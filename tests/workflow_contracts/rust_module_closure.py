"""Close over the source files a set of Rust files can depend on.

`kani-smoke` skips its proofs on a pull request that changes none of their
inputs, so the proof-scope contract must know every file a harness can depend
on. Starting from the files ``kani_seeds`` names, this module follows the
references each reached file makes, over the tree ``rust_module_graph`` reads:

- a `crate::`, `super::`, `self::` or `$crate::` path reaches the module it
  names, together with every compiled module nested beneath it, and a `{…}`
  use group reaches each module it names;
- a bare `child::` path reaches a module declared in the same file;
- a `name!` invocation reaches every file defining `macro_rules! name`;
- an `impl` item whose header names a type or trait the closure defines
  reaches the file holding it, since coherence lets an impl live anywhere in
  the crate;
- `include_str!`, `include_bytes!` and `include!` reach the file a literal
  argument names, or the directory of a `concat!`'s leading literal.

A reached module's declaring ancestors are added at the end, since the
attributes on a `mod` line decide whether and how it is compiled. Every rule
over-approximates; the crate root alone is reached as a file rather than a
subtree, because every module sits beneath it.

Test-only: production code and workflows must not import this module.
"""

import re
import typing as typ

from rust_module_graph import STRING_LITERAL, ModuleGraphError, closing_bracket

if typ.TYPE_CHECKING:
    from pathlib import Path

    from rust_module_graph import CrateSource, RustModule

ROOTED_PATH = re.compile(r"(?<![\w$])(?P<head>\$crate|crate|super|self)\s*::")
PATH_SEGMENT = re.compile(r"\s*(?P<name>[A-Za-z_]\w*)\s*::")
GROUP_OPEN = re.compile(r"\s*\{")
FINAL_SEGMENT = re.compile(r"\s*(?P<name>[A-Za-z_]\w*)")
GROUP_NAME = re.compile(r"(?:^|[{,])\s*(?P<name>[A-Za-z_]\w*)")
BARE_PATH = re.compile(r"(?<![\w:$])(?P<name>[A-Za-z_]\w*)\s*::")
MACRO_DEFINITION = re.compile(r"\bmacro_rules!\s*(?P<name>[A-Za-z_]\w*)")
MACRO_CALL = re.compile(r"(?<![\w:])(?P<name>[A-Za-z_]\w*)!")
TYPE_DEFINITION = re.compile(
    r"\b(?:struct|enum|union|trait|type)\s+(?P<name>[A-Za-z_]\w*)"
)
#: An `impl` item at the start of a line; `impl Trait` in argument or return
#: position is a type, not an item, and never starts a line.
IMPL_HEADER = re.compile(r"(?m)^[ \t]*(?:unsafe\s+)?impl\b(?P<header>[^{;]*)\{")
IDENTIFIER = re.compile(r"[A-Za-z_]\w*")
INCLUDE_CALL = re.compile(r"\binclude(?:_str|_bytes)?!\s*\(")
ASSEMBLED_PATH = re.compile(r'concat!\s*\(\s*"(?P<value>[^"\\]*)"')
KANI_MARKER = re.compile(r"\bkani\b|#\[\s*global_allocator\s*\]")


def _rooted_targets(
    crate: CrateSource, module: RustModule, code: str
) -> set[tuple[str, ...]]:
    """Return the modules the rooted paths in ``code`` name."""
    targets = set()
    for match in ROOTED_PATH.finditer(code):
        base, index = _walk_path(code, match.end(), _path_base(module, match["head"]))
        prefix = crate.resolve(base)
        targets.add(prefix)
        targets.add(_final_target(crate, base, code, index))
        if group := GROUP_OPEN.match(code, index):
            targets |= _grouped_targets(crate, prefix, code, group.end())
    return targets


def _final_target(
    crate: CrateSource, base: tuple[str, ...], code: str, index: int
) -> tuple[str, ...]:
    """Return the module a path's final segment names, or ``base`` itself.

    `use crate::hasher;` names the module `hasher` in a segment no `::`
    follows, and the file then reaches it through a bare `hasher::` path that
    ``_bare_targets`` cannot see from a nested module. When the final segment
    is an item rather than a module, resolving falls back to ``base``.

    Returns
    -------
    tuple[str, ...]
        The longest module prefix of the path including its final segment.
    """
    final = FINAL_SEGMENT.match(code, index)
    return crate.resolve((*base, final["name"]) if final else base)


def _path_base(module: RustModule, head: str) -> tuple[str, ...]:
    """Return the module a rooted path's ``head`` keyword starts from."""
    match head:
        case "self":
            return module.path
        case "super":
            return module.path[:-1]
        case _:
            return ()


def _walk_path(
    code: str, index: int, base: tuple[str, ...]
) -> tuple[tuple[str, ...], int]:
    """Return the module a path names from ``base``, and where the walk stopped.

    The walk consumes further `super::` hops and module names, stopping at a
    brace group, a glob or an item.

    Returns
    -------
    tuple[tuple[str, ...], int]
        The named module path and the offset just past its last segment.
    """
    while segment := PATH_SEGMENT.match(code, index):
        base = base[:-1] if segment["name"] == "super" else (*base, segment["name"])
        index = segment.end()
    return base, index


def _grouped_targets(
    crate: CrateSource, prefix: tuple[str, ...], code: str, start: int
) -> set[tuple[str, ...]]:
    """Return the modules a `{a::b, c}` use group names beneath ``prefix``.

    Nested groups are flattened to their first segment beneath ``prefix``,
    which reaches the whole subtree and so over-approximates.

    Returns
    -------
    set[tuple[str, ...]]
        The longest module prefix of each name the group starts with.
    """
    group = code[start : closing_bracket(code, start) - 1]
    names = {match["name"] for match in GROUP_NAME.finditer(group)}
    return {crate.resolve((*prefix, name)) for name in names - {"self"}}


def _bare_targets(
    crate: CrateSource, module: RustModule, code: str
) -> set[tuple[str, ...]]:
    """Return the child modules of ``module`` named by a bare `child::` path."""
    children = {child.path[-1]: child.path for child in crate.children(module.path)}
    return {
        children[match["name"]]
        for match in BARE_PATH.finditer(code)
        if match["name"] in children
    }


def _include_targets(file: Path, code: str, text: str) -> set[Path]:
    """Return the files and directories the `include*!` calls in a file read.

    The argument list is delimited on the fully masked ``code``, so a
    parenthesis inside a literal cannot end it early, and read from ``text``
    at the same offsets. A literal argument names one file. A `concat!` whose
    first argument is a literal names the directory of that literal, which
    covers every file the assembled path can name there. Any other argument is
    refused by ``_include_target``.

    Returns
    -------
    set[Path]
        The resolved file and directory targets.
    """
    return {
        _include_target(
            file, text[match.end() : closing_bracket(code, match.end(), "()") - 1]
        )
        for match in INCLUDE_CALL.finditer(code)
    }


def _include_target(file: Path, arguments: str) -> Path:
    """Return the file or directory one include's ``arguments`` name.

    Returns
    -------
    Path
        The resolved target.

    Raises
    ------
    ModuleGraphError
        When the path is neither a literal nor a `concat!` led by one.
    """
    if whole := STRING_LITERAL.fullmatch(arguments.strip()):
        return (file.parent / whole["value"]).resolve()
    if assembled := ASSEMBLED_PATH.match(arguments.strip()):
        directory = assembled["value"].rpartition("/")[0] or "."
        return (file.parent / directory).resolve()
    msg = f"{file}: an include without a literal path cannot be resolved"
    raise ModuleGraphError(msg)


def _defined_names(crate: CrateSource, files: set[Path]) -> set[str]:
    """Return every type and trait name defined in ``files``."""
    return {
        name for file in files for name in TYPE_DEFINITION.findall(crate.code[file])
    }


def _implementing_files(crate: CrateSource, names: set[str]) -> set[Path]:
    """Return every compiled file with an `impl` header naming one of ``names``."""
    return {
        module.file
        for module in crate.modules.values()
        if not module.is_test_only
        and any(
            names & set(IDENTIFIER.findall(match["header"]))
            for match in IMPL_HEADER.finditer(crate.code[module.file])
        )
    }


def _macro_files(crate: CrateSource) -> dict[str, set[Path]]:
    """Return the files defining each `macro_rules!` name in the crate."""
    definitions: dict[str, set[Path]] = {}
    for module in crate.modules.values():
        for name in MACRO_DEFINITION.findall(crate.code[module.file]):
            definitions.setdefault(name, set()).add(module.file)
    return definitions


def _file_references(
    crate: CrateSource, file: Path, macros: dict[str, set[Path]]
) -> set[Path]:
    """Return the files one reached file references directly."""
    module = crate.module_of(file)
    code = crate.code[file]
    modules = _rooted_targets(crate, module, code) | _bare_targets(crate, module, code)
    # The crate root names the root file alone: every module sits beneath it,
    # so reaching its subtree would reach the whole crate for any root item.
    referenced = {crate.modules[()].file for path in modules if not path}
    referenced |= {
        nested.file
        for path in modules
        if path
        for nested in crate.subtree(path)
        if not nested.is_test_only
    }
    return referenced | {
        definition
        for call in MACRO_CALL.findall(code)
        for definition in macros.get(call, set())
    }


def reachable_files(crate: CrateSource, seeds: set[Path]) -> set[Path]:
    """Return the source files and include targets ``seeds`` can depend on.

    Each reached module's declaring ancestors are included too, since the
    attributes on a `mod` line (`cfg`, `path`) decide whether and how the
    module is compiled at all.

    Returns
    -------
    set[Path]
        The reached files, their declaring ancestors, and include targets.
    """
    macros = _macro_files(crate)
    reached: set[Path] = set()
    pending = set(seeds)
    while pending:
        reached |= pending
        frontier = {
            reference
            for file in pending
            for reference in _file_references(crate, file, macros)
        }
        frontier |= _implementing_files(crate, _defined_names(crate, reached))
        pending = frontier - reached
    ancestors = _declaring_ancestors(crate, reached)
    return reached | ancestors | _all_includes(crate, reached)


def _declaring_ancestors(crate: CrateSource, files: set[Path]) -> set[Path]:
    """Return the files declaring each module on the way down to ``files``."""
    return {
        crate.modules[crate.module_of(file).path[:length]].file
        for file in files
        for length in range(len(crate.module_of(file).path))
    }


def _all_includes(crate: CrateSource, files: set[Path]) -> set[Path]:
    """Return every include target the `include*!` calls in ``files`` read."""
    return {
        target
        for file in files
        for target in _include_targets(file, crate.code[file], crate.text[file])
    }


def kani_seeds(crate: CrateSource) -> set[Path]:
    """Return every compiled file whose code names Kani or a global allocator.

    A `#[kani::proof]` harness is a seed, and so is any `cfg(kani)` or
    `kani::` site, since it changes what the verifier compiles even where no
    harness reaches it by path. A `#[global_allocator]` changes every
    allocation a harness makes.

    Returns
    -------
    set[Path]
        The seed files.
    """
    return {
        module.file
        for module in crate.modules.values()
        if not module.is_test_only and KANI_MARKER.search(crate.code[module.file])
    }
