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
    from pathlib import Path

TESTS_DIR = REPO_ROOT / "tests"

#: This package's name, which is the first half of every `binary_id`.
PACKAGE_NAME = "netsuke"

#: The manifest key that would declare a test target by hand. Cargo's
#: auto-discovery is what the derivation here models, and an explicit
#: `[[test]]` can name a target that has nothing to do with its filename, so
#: the absence of the key is asserted rather than assumed.
TEST_TARGET_KEY = "test"


def declared_test_targets() -> list[object]:
    """Return every `[[test]]` section this workspace's manifests declare."""
    declared: list[object] = []
    for manifest in [REPO_ROOT / "Cargo.toml", *REPO_ROOT.glob("*/Cargo.toml")]:
        parsed = tomllib.loads(manifest.read_text(encoding="utf-8"))
        declared.extend(parsed.get(TEST_TARGET_KEY) or [])
    return declared


def target_sources() -> dict[str, list[Path]]:
    """Return each integration-test target's name and the sources it compiles."""
    # Cargo's auto-discovery, modelled: `tests/<name>.rs` is a target called
    # `<name>`, and `tests/<name>/main.rs` is a target called `<name>` that
    # compiles every module beneath it. Anything else under `tests/` is a
    # module of one of those, not a target of its own. Globbing every `.rs`
    # instead reported a module file as a target and named it after its own
    # file, so `tests/<name>/main.rs` read as `main`.
    targets: dict[str, list[Path]] = {}
    for path in sorted(TESTS_DIR.glob("*.rs")):
        targets[path.stem] = [path]
    for entry in sorted(TESTS_DIR.iterdir()):
        if entry.is_dir() and (entry / "main.rs").exists():
            targets[entry.name] = sorted(entry.rglob("*.rs"))
    return targets
