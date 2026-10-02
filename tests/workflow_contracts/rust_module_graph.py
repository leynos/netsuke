"""Read a Rust crate's module tree from its source text.

The Kani proof-scope contract needs each file-backed module of the library
crate: its path from the crate root, the file that backs it, and whether it
is compiled only under `cfg(test)`. This module reads that tree from the crate
root, following `mod name;` declarations and `#[path]` attributes, and keeps
each file's text twice: fully masked, for code queries, and with only comments
masked, where string literals such as `#[path]` values must stay readable.
``rust_module_closure`` closes over the references in that text.

A module declared under `#[cfg(test)]`, and an inline `#[cfg(test)] mod … {
… }` body, are marked or masked because `cargo kani` compiles without
`cfg(test)`. A `mod name;` nested inside an inline module body is refused
rather than resolved by guesswork, so an unsupported layout fails the contract
instead of silently shrinking the scope.

Test-only: production code and workflows must not import this module.
"""

import dataclasses
import functools
import re
import typing as typ

from rust_source_scan import mask_non_code

if typ.TYPE_CHECKING:
    from pathlib import Path

#: A file-backed `mod name;` declaration with the attribute block before it.
MOD_DECLARATION = re.compile(
    r"(?P<attributes>(?:#\[[^\]]*\]\s*)*)"
    r"(?:pub(?:\s*\([^)]*\))?\s+)?mod\s+(?P<name>[A-Za-z_]\w*)\s*(?P<end>[;{])"
)
PATH_ATTRIBUTE = re.compile(r'#\[\s*path\s*=\s*"(?P<path>[^"]+)"\s*\]')
TEST_ONLY_CFG = re.compile(r"#\[\s*cfg\s*\(\s*(?:test\s*\)|all\s*\(\s*test\b)")
STRING_LITERAL = re.compile(r'"(?P<value>[^"\\]*)"')
STRING_PREFIXES = ('"', 'r"', "r#", 'b"', "br")
MOD_RS_NAMES = frozenset({"lib.rs", "main.rs", "mod.rs"})


class ModuleGraphError(Exception):
    """Report a module layout the reader refuses to guess about."""


@dataclasses.dataclass(frozen=True, slots=True)
class RustModule:
    """One file-backed module: its path from the crate root and its file."""

    path: tuple[str, ...]
    file: Path
    is_test_only: bool


@dataclasses.dataclass(frozen=True)
class CrateSource:
    """The module tree of one crate and two views of each file's text.

    ``code`` masks comments and literals, so every brace and path in it is
    real, and blanks inline `#[cfg(test)]` module bodies; ``text`` masks only
    comments, so literals stay readable at the same offsets. Every query finds
    its match in ``code`` and reads literals from ``text`` at that offset, so
    nothing in a blanked body is ever read.
    """

    modules: dict[tuple[str, ...], RustModule]
    code: dict[Path, str]
    text: dict[Path, str]

    @functools.cached_property
    def _by_file(self) -> dict[Path, RustModule]:
        """Index the modules by the file that backs each."""
        return {module.file: module for module in self.modules.values()}

    def module_of(self, file: Path) -> RustModule:
        """Return the module a file backs."""
        return self._by_file[file]

    def children(self, path: tuple[str, ...]) -> list[RustModule]:
        """Return the modules declared directly inside ``path``."""
        return [m for m in self.modules.values() if m.path[:-1] == path and m.path]

    def subtree(self, path: tuple[str, ...]) -> list[RustModule]:
        """Return ``path`` and every module nested beneath it."""
        return [m for m in self.modules.values() if m.path[: len(path)] == path]

    def resolve(self, path: tuple[str, ...]) -> tuple[str, ...]:
        """Return the longest prefix of ``path`` that names a module."""
        for length in range(len(path), -1, -1):
            if path[:length] in self.modules:
                return path[:length]
        return ()


def closing_bracket(code: str, start: int, pair: str = "{}") -> int:
    """Return the offset just past the bracket closing the one before ``start``.

    ``code`` must be fully masked, so that no bracket inside a literal or a
    comment is counted.

    Returns
    -------
    int
        The offset after the matching closing bracket, or the end of ``code``
        when the brackets never balance.
    """
    depth, index = 1, start
    while depth and index < len(code):
        depth += {pair[0]: 1, pair[1]: -1}.get(code[index], 0)
        index += 1
    return index


def _test_only_spans(code: str) -> list[tuple[int, int]]:
    """Return the span of every inline `#[cfg(test)] mod … { … }` in ``code``."""
    return [
        (match.start(), closing_bracket(code, match.end()))
        for match in MOD_DECLARATION.finditer(code)
        if match["end"] == "{" and TEST_ONLY_CFG.search(match["attributes"])
    ]


def _blank(text: str, spans: list[tuple[int, int]]) -> str:
    """Replace each span of ``text`` with spaces, keeping its line breaks."""
    characters = list(text)
    for start, end in spans:
        characters[start:end] = (
            "\n" if character == "\n" else " " for character in text[start:end]
        )
    return "".join(characters)


def _declared_file(
    parent: Path, name: str, path_attribute: str | None, *, is_mod_rs: bool
) -> Path:
    """Return the file a `mod name;` declared in ``parent`` loads."""
    if path_attribute is not None:
        return (parent.parent / path_attribute).resolve()
    directory = parent.parent if is_mod_rs else parent.parent / parent.stem
    flat = directory / f"{name}.rs"
    return flat if flat.is_file() else directory / name / "mod.rs"


def _file_declarations(
    file: Path, code: str, text: str
) -> list[tuple[str, str | None, bool]]:
    """Return each file-backed module declared in ``file``, refusing nested ones.

    ``code`` is the fully masked source, whose braces are all real, and
    ``text`` the same source with only comments masked, from which the
    `#[path]` literal is read at the same offsets.

    Returns
    -------
    list[tuple[str, str | None, bool]]
        Each module's name, its `#[path]` value if any, and whether it is
        declared under `#[cfg(test)]`.

    Raises
    ------
    ModuleGraphError
        When a `mod name;` sits inside a brace block, where its file would
        resolve relative to an inline module the reader does not model.
    """
    declarations = []
    for match in MOD_DECLARATION.finditer(code):
        if match["end"] != ";":
            continue
        if code[: match.start()].count("{") != code[: match.start()].count("}"):
            msg = f"{file}: `mod {match['name']};` is nested in a block"
            raise ModuleGraphError(msg)
        attributes = text[match.start("attributes") : match.end("attributes")]
        path_match = PATH_ATTRIBUTE.search(attributes)
        declarations.append((
            match["name"],
            path_match["path"] if path_match else None,
            bool(TEST_ONLY_CFG.search(attributes)),
        ))
    return declarations


class _EveryStringLiteral(set[str]):
    """Claim to contain every string literal, so masking keeps them all.

    ``mask_non_code`` retains the literals its caller names; naming them in
    advance would need a second tokenizer, and a regular expression pairing
    quotes falls out of step at the first escaped quote.
    """

    def __contains__(self, literal: object) -> bool:
        """Return whether ``literal`` is a string rather than a comment."""
        return isinstance(literal, str) and literal.startswith(STRING_PREFIXES)


def comment_free(source: str) -> str:
    """Mask Rust comments while keeping every string literal readable."""
    return mask_non_code(source, _EveryStringLiteral())


def _file_views(file: Path) -> tuple[str, str]:
    """Return a file's masked code, test bodies blanked, and comment-free text."""
    raw = file.read_text(encoding="utf-8")
    masked = mask_non_code(raw, set())
    return _blank(masked, _test_only_spans(masked)), comment_free(raw)


def read_crate(crate_root: Path) -> CrateSource:
    """Read every file-backed module reachable from ``crate_root``.

    Returns
    -------
    CrateSource
        The module tree with each file's masked views.

    Raises
    ------
    ModuleGraphError
        When a declared module's file is missing or the layout is refused.
    """
    modules: dict[tuple[str, ...], RustModule] = {}
    code: dict[Path, str] = {}
    text: dict[Path, str] = {}
    pending = [(RustModule((), crate_root.resolve(), is_test_only=False), True)]
    while pending:
        module, is_mod_rs = pending.pop()
        if not module.file.is_file():
            msg = f"module {'::'.join(module.path)} has no file at {module.file}"
            raise ModuleGraphError(msg)
        modules[module.path] = module
        code[module.file], text[module.file] = _file_views(module.file)
        for name, path_attribute, is_test_only in _file_declarations(
            module.file, code[module.file], text[module.file]
        ):
            child = _declared_file(
                module.file, name, path_attribute, is_mod_rs=is_mod_rs
            )
            pending.append((
                RustModule(
                    (*module.path, name), child, module.is_test_only or is_test_only
                ),
                path_attribute is not None or child.name in MOD_RS_NAMES,
            ))
    return CrateSource(modules, code, text)
