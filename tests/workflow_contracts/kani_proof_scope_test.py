"""Hold `tools/kani/proof-scope.toml` to what the Kani harnesses depend on.

`kani-smoke` skips its proofs on a pull request that changes nothing in the
proof scope, so a scope missing one input lets a pull request that breaks a
proof merge green. These contracts recompute the harnesses' module closure
from the Rust source (see ``rust_module_closure``) and fail when the scope
misses any of it, when a `#[kani::proof]` file anywhere in the repository is
outside it, or when a toolchain input is dropped. They also fail when the
scope grows past the closure, since a scope covering the whole crate would
satisfy sufficiency and quietly run the proofs on every pull request again.

Run via ``make test-workflow-contracts``.
"""

import re
import tomllib
import typing as typ

import pytest
from rust_module_closure import kani_seeds, reachable_files
from rust_module_graph import CrateSource, read_crate
from rust_source_scan import mask_non_code
from workflow_loading import MAKEFILE_PATH, REPO_ROOT

if typ.TYPE_CHECKING:
    from pathlib import Path

SCOPE_FILE = REPO_ROOT / "tools" / "kani" / "proof-scope.toml"
CRATE_ROOT = REPO_ROOT / "src" / "lib.rs"
#: What builds and runs the proofs, which no source closure can find: each is
#: an input `cargo kani` or the `kani-smoke` job reads. ADR-039 records why.
REQUIRED_INFRASTRUCTURE = (
    ".cargo/",  # Cargo configuration read by every cargo invocation
    ".github/actions/kani-cache/",  # the cached verifier payloads
    ".github/workflows/ci.yml",  # the job, its installer and its pins
    "Cargo.lock",  # the resolved dependencies the harnesses compile against
    "Cargo.toml",  # features, dependencies and `[package.metadata.kani]`
    "Makefile",  # the `kani-ir` and `kani-full` targets and their flags
    "build.rs",  # the build script `cargo kani` runs before compiling
    "rust-toolchain.toml",  # the toolchain pin
    "scripts/kani_proof_scope.py",  # the decision itself
    "tools/kani/",  # the verifier version and this scope
)
PROOF_ATTRIBUTE = re.compile(r"#\[\s*kani\s*::\s*proof\s*\]")
SKIPPED_DIRECTORIES = frozenset({"target", "node_modules"})


def _scope() -> dict[str, list[str]]:
    """Return the `[scope]` table of the proof-scope file."""
    return tomllib.loads(SCOPE_FILE.read_text(encoding="utf-8"))["scope"]


def _relative(path: Path) -> str:
    """Return ``path`` relative to the repository root, POSIX-style."""
    return path.relative_to(REPO_ROOT).as_posix()


def _covers(entry: str, path: str) -> bool:
    """Return whether one scope entry covers a repository-relative path."""
    if entry.endswith("/"):
        return path == entry.removesuffix("/") or path.startswith(entry)
    return path == entry


def _covered_files(entry: str) -> list[Path]:
    """Return every file on disk one scope entry covers."""
    target = REPO_ROOT / entry
    if entry.endswith("/"):
        return sorted(path for path in target.rglob("*") if path.is_file())
    return [target] if target.is_file() else []


def _repository_rust_files() -> list[Path]:
    """Return every Rust source in the repository outside build and hidden trees."""
    return sorted(
        path
        for path in REPO_ROOT.rglob("*.rs")
        if not any(
            part in SKIPPED_DIRECTORIES or part.startswith(".")
            for part in path.relative_to(REPO_ROOT).parts
        )
    )


@pytest.fixture(scope="module")
def crate() -> CrateSource:
    """Return the library crate's module tree."""
    return read_crate(CRATE_ROOT)


@pytest.fixture(scope="module")
def closure(crate: CrateSource) -> set[str]:
    """Return the harness closure as repository-relative paths."""
    return {_relative(path) for path in reachable_files(crate, kani_seeds(crate))}


def test_every_proof_harness_file_is_in_scope() -> None:
    """Require every file declaring `#[kani::proof]` to be a proof input.

    The search covers the whole repository, not just the library, so a
    harness added to a test tree or another crate fails here until the scope
    and the Kani invocation are extended to it. Comments and string literals
    are masked, so prose quoting the attribute does not count.
    """
    harnesses = [
        _relative(path)
        for path in _repository_rust_files()
        if PROOF_ATTRIBUTE.search(
            mask_non_code(path.read_text(encoding="utf-8"), set())
        )
    ]
    assert harnesses, "no `#[kani::proof]` harness found; the discovery is broken"
    sources = _scope()["sources"]
    outside = [path for path in harnesses if not any(_covers(e, path) for e in sources)]
    assert not outside, (
        f"these harness files are outside tools/kani/proof-scope.toml "
        f"`sources`: {outside}"
    )


def test_scope_covers_the_harness_closure(closure: set[str]) -> None:
    """Require every file the harnesses can reach to be a proof input."""
    sources = _scope()["sources"]
    missing = sorted(
        path for path in closure if not any(_covers(e, path) for e in sources)
    )
    assert not missing, (
        f"the harnesses reach these paths, which tools/kani/proof-scope.toml "
        f"`sources` does not cover: {missing}"
    )


def _is_beneath(path: str, directories: set[str]) -> bool:
    """Return whether ``path`` lies inside one of ``directories``."""
    return any(path.startswith(f"{directory}/") for directory in directories)


def _unreached_files(entry: str, allowed: set[str], directories: set[str]) -> list[str]:
    """Return the files one scope entry covers that no harness reaches.

    A file is reached when ``allowed`` names it or it lies beneath one of the
    ``directories`` the closure reaches (an include target).

    Returns
    -------
    list[str]
        The covered files outside the closure, repository-relative.
    """
    return [
        path
        for path in map(_relative, _covered_files(entry))
        if path not in allowed and not _is_beneath(path, directories)
    ]


def test_scope_reaches_no_further_than_the_closure(
    crate: CrateSource, closure: set[str]
) -> None:
    """Require each source entry to cover only the closure and its test modules.

    A test-only module is compiled out under `cargo kani`, so a directory
    entry may cover one harmlessly. Anything else means the entry is broader
    than what the proofs depend on.
    """
    test_only = {_relative(m.file) for m in crate.modules.values() if m.is_test_only}
    directories = {path for path in closure if (REPO_ROOT / path).is_dir()}
    broad = {
        entry: unreached
        for entry in _scope()["sources"]
        if (unreached := _unreached_files(entry, closure | test_only, directories))
    }
    assert not broad, f"these entries cover files no harness reaches: {broad}"


def test_every_scope_entry_covers_the_closure(closure: set[str]) -> None:
    """Require every source entry to cover at least one path in the closure.

    An entry covering nothing a harness reaches is stale, so it is removed
    rather than kept.
    """
    dead = [
        e for e in _scope()["sources"] if not any(_covers(e, path) for path in closure)
    ]
    assert not dead, f"these entries cover nothing a harness reaches: {dead}"


def test_scope_names_every_toolchain_input() -> None:
    """Require the inputs no source closure can find, each present on disk."""
    infrastructure = _scope()["infrastructure"]
    missing = [
        entry for entry in REQUIRED_INFRASTRUCTURE if entry not in infrastructure
    ]
    assert not missing, f"the proof scope must name these inputs: {missing}"
    absent = [entry for entry in infrastructure if not (REPO_ROOT / entry).exists()]
    assert not absent, f"these proof-scope entries do not exist: {absent}"


def test_kani_compiles_without_the_test_configuration() -> None:
    """Require `cargo kani` to stay off `--tests`.

    The closure leaves out `#[cfg(test)]` modules because `cargo kani` does
    not compile them. Passing `--tests` would compile them, so the closure
    would silently miss every test module a harness could then reach.
    """
    makefile = MAKEFILE_PATH.read_text(encoding="utf-8")
    kani_lines = [line for line in makefile.splitlines() if "KANI" in line]
    assert not [line for line in kani_lines if "--tests" in line], (
        "the Makefile's Kani invocation must not pass `--tests`"
    )
    manifest = tomllib.loads((REPO_ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    flags = manifest["package"]["metadata"]["kani"]["flags"]
    assert "tests" not in flags, "`[package.metadata.kani.flags]` must not set `tests`"
