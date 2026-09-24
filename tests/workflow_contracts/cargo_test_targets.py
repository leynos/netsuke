"""Cargo's integration-test targets, derived from this workspace's layout.

A contract written about test targets has to know what a target *is*. A target
is not a file: `tests/<name>/main.rs` compiles every module beneath it into one
binary called `<name>`, and a reader that treated each source as a target of
its own would name that binary `main` and report every module as something
nextest runs.

Auto-discovery is modelled rather than read from `cargo metadata`, which would
need a toolchain and a build directory to answer a question about file layout.
The premise that makes the model sound, that no target is declared by hand, is
asserted by `cargo_test_targets_test` rather than assumed.
"""

import tomllib
import typing as typ

from workflow_loading import REPO_ROOT

if typ.TYPE_CHECKING:  # pragma: no cover - imported for annotations only
    import collections.abc as cabc
    from pathlib import Path

TESTS_DIR = REPO_ROOT / "tests"

#: This package's name, which is the first half of every `binary_id`.
PACKAGE_NAME = "netsuke"

#: The manifest key that would declare a test target by hand. Cargo's
#: auto-discovery is what the derivation here models, and an explicit
#: `[[test]]` can name a target that has nothing to do with its filename, so
#: the absence of the key is asserted rather than assumed.
TEST_TARGET_KEY = "test"


class RepositoryFileError(OSError):
    """Raised when a repository file or directory cannot be read or parsed.

    A contract that reads fewer files than the tree holds answers for a
    smaller tree, and an empty answer passes every "each member is covered"
    rule. So a missing or unreadable file, bytes that do not decode, and TOML
    that does not parse all arrive here with the path, rather than as whatever
    exception the failure produced or as an empty discovery.
    """


def read_repository_file(path: Path) -> str:
    """Return one repository file's text, or raise naming it.

    Returns
    -------
    str
        The file's text.

    Raises
    ------
    RepositoryFileError
        If the file is missing, unreadable, or not UTF-8.
    """
    try:
        return path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as error:
        message = f"cannot read {path}: {error}"
        raise RepositoryFileError(message) from error


def parse_toml(text: str, path: Path) -> dict[str, typ.Any]:
    """Return a TOML document parsed, or raise naming the file it came from.

    Returns
    -------
    dict[str, typ.Any]
        The parsed document.

    Raises
    ------
    RepositoryFileError
        If the text is not valid TOML.
    """
    try:
        return tomllib.loads(text)
    except tomllib.TOMLDecodeError as error:
        message = f"{path} is not valid TOML: {error}"
        raise RepositoryFileError(message) from error


def manifest_texts(root: Path = REPO_ROOT) -> dict[Path, str]:
    """Return the workspace root manifest and each member's, read at the boundary."""
    manifests = [root / "Cargo.toml", *sorted(root.glob("*/Cargo.toml"))]
    return {manifest: read_repository_file(manifest) for manifest in manifests}


def declared_test_targets(manifests: cabc.Mapping[Path, str]) -> list[object]:
    """Return every `[[test]]` section the given manifests declare.

    Returns
    -------
    list[object]
        One entry per declared section. Empty is the expected answer here, and
        the contract beside this module asserts it: Cargo's auto-discovery is
        what `target_sources` models, and an explicit section would give a
        target a name unrelated to its path. A manifest that is not valid
        TOML raises `RepositoryFileError` through `parse_toml`.
    """
    declared: list[object] = []
    for path, text in manifests.items():
        declared.extend(parse_toml(text, path).get(TEST_TARGET_KEY) or [])
    return declared


def target_sources(tests_dir: Path = TESTS_DIR) -> dict[str, list[Path]]:
    """Return each integration-test target's name and the sources it compiles.

    Returns
    -------
    dict[str, list[Path]]
        Target name to the sources Cargo compiles into it. A
        `tests/<name>.rs` target has one source; a `tests/<name>/main.rs`
        target has every `.rs` beneath its directory.

    Raises
    ------
    RepositoryFileError
        If the directory is missing or cannot be listed. A missing directory
        would otherwise glob to nothing and read as a tree with no targets.
    """
    # Cargo's auto-discovery, modelled: `tests/<name>.rs` is a target called
    # `<name>`, and `tests/<name>/main.rs` is a target called `<name>` that
    # compiles every module beneath it. Anything else under `tests/` is a
    # module of one of those, not a target of its own.
    if not tests_dir.is_dir():
        message = f"{tests_dir} is not a directory, so no test target was read"
        raise RepositoryFileError(message)
    try:
        return _file_targets(tests_dir) | _directory_targets(tests_dir)
    except OSError as error:
        message = f"cannot list {tests_dir}: {error}"
        raise RepositoryFileError(message) from error


def _file_targets(tests_dir: Path) -> dict[str, list[Path]]:
    """Return each `tests/<name>.rs` target with its one source."""
    return {path.stem: [path] for path in sorted(tests_dir.glob("*.rs"))}


def _directory_targets(tests_dir: Path) -> dict[str, list[Path]]:
    """Return each `tests/<name>/main.rs` target with every source beneath it."""
    return {
        entry.name: sorted(entry.rglob("*.rs"))
        for entry in sorted(tests_dir.iterdir())
        if entry.is_dir() and (entry / "main.rs").exists()
    }


def target_texts(targets: cabc.Mapping[str, list[Path]]) -> dict[str, list[str]]:
    """Return each target's source texts, read at the boundary."""
    return {
        name: [read_repository_file(path) for path in paths]
        for name, paths in targets.items()
    }
