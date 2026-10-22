"""Keep the production build slice in Cargo's graph, outside timed test setup.

The slice used to be compiled by ``tests/build_module_slice_ui_tests.rs``,
which ran a package-scoped ``cargo check`` and two ``rustc`` invocations
inside one Nextest test body. ``build_slice_fixture/`` now compiles the same
sources during Cargo's ordinary build phase, and its doctests are the
compiler controls. This contract holds the fixture to that role: the same
build dependencies as ``build.rs``, real production sources rather than a
stand-in, and no compiler work back inside the timed test.

Run via ``make test-workflow-contracts``.
"""

import re
import tomllib

import pytest
from workflow_loading import REPO_ROOT

FIXTURE = REPO_ROOT / "build_slice_fixture"

#: `cfg(feature = "…")` as written in Rust, including the `cfg!(…)` form.
FEATURE_GATE = re.compile(r'cfg!?\(\s*feature\s*=\s*"([^"]+)"')

#: Markers each fixture source must carry, and the behaviour losing one would
#: reintroduce. ``lib.rs`` must include the shared slice composition and hold
#: both doctest controls; ``slice.rs`` must compose the production module by
#: path so the fixture compiles the real sources rather than a copy.
REQUIRED_MARKERS: dict[str, dict[str, str]] = {
    "lib.rs": {
        'include!("slice.rs")': (
            "lib.rs must include the shared slice composition, or the fixture "
            "compiles no production module at all"
        ),
        "use netsuke_build_slice_fixture::cli::Cli;": (
            "the positive control must name the supported CLI surface, or "
            "nothing proves that surface compiles"
        ),
        "```compile_fail,E0432": (
            "the negative control must stay a compile-fail example, or it "
            "passes by compiling and asserts nothing"
        ),
        "use netsuke_build_slice_fixture::cli::discovery;": (
            "the negative control must probe the runtime-only module, or it "
            "proves only that some unrelated path is absent"
        ),
    },
    "slice.rs": {
        '#[path = "../src/cli"]': (
            "slice.rs must reach the production CLI module by path, or the "
            "fixture stops standing in for build.rs"
        ),
        "pub mod cli {": ("slice.rs must publish the cli module the doctests import"),
    },
}


def test_build_slice_fixture_uses_the_production_build_dependencies() -> None:
    """Reject drift in versions, features, or dependency sources for the slice."""
    root = tomllib.loads((REPO_ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    fixture = tomllib.loads((FIXTURE / "Cargo.toml").read_text(encoding="utf-8"))
    assert "build_slice_fixture" in root["workspace"]["members"], (
        "the fixture must stay a workspace member, or no ordinary build "
        "compiles the production slice"
    )
    assert fixture["package"]["publish"] is False, (
        "the fixture must never be published; it exists only to compile the "
        "slice alongside the crate"
    )
    expected_names = {"camino", "clap", "ortho_config", "serde", "thiserror", "tracing"}
    assert fixture["dependencies"] == {
        name: root["build-dependencies"][name] for name in expected_names
    }, (
        "the fixture's dependencies must match build.rs exactly in version, "
        "features, and source, or it can compile a slice the real build "
        "script cannot"
    )
    assert fixture["lib"].get("doctest", True), (
        "the boundary doctests must execute; disabling doctests would remove "
        "both compiler controls"
    )


def test_build_slice_fixture_declares_every_gated_feature_name() -> None:
    """Keep the fixture's feature names in step with the sources it reaches.

    The fixture compiles production modules by path, so their
    `cfg(feature = …)` sites are evaluated in a package whose feature list
    Cargo reads from *this* manifest. The workspace denies `unexpected_cfgs`,
    so a name the host package declares but the fixture does not fails
    `cargo doc` and `cargo clippy` with an error that names the included
    source rather than the missing declaration.

    Deriving the expected set from `src/` rather than hard-coding it keeps the
    contract honest when a future feature gate lands in an included module.
    """
    root = tomllib.loads((REPO_ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    fixture = tomllib.loads((FIXTURE / "Cargo.toml").read_text(encoding="utf-8"))
    gated = {
        name
        for path in (REPO_ROOT / "src").rglob("*.rs")
        for name in FEATURE_GATE.findall(path.read_text(encoding="utf-8"))
    }
    declared = set(fixture.get("features", {}))
    missing = sorted(gated - declared)
    assert not missing, (
        "the fixture must declare every feature name `src/` gates on, or its "
        f"build fails on `unexpected_cfgs` for {missing}"
    )
    unknown = sorted(declared - set(root["features"]))
    assert not unknown, (
        "a fixture-only feature name would let the slice compile under a gate "
        f"the host package never sets: {unknown}"
    )


@pytest.mark.parametrize(("name", "markers"), REQUIRED_MARKERS.items())
def test_build_slice_fixture_is_part_of_the_normal_library_build(
    name: str, markers: dict[str, str]
) -> None:
    """Keep real production sources and both compiler controls in the fixture."""
    source = (FIXTURE / name).read_text(encoding="utf-8")
    missing = {
        marker: reason for marker, reason in markers.items() if marker not in source
    }
    assert not missing, f"{name} is missing required markers: {missing}"


def test_build_slice_runtime_contract_cannot_start_a_compiler() -> None:
    """Prevent reintroducing Cargo preparation inside the Nextest time budget."""
    source = (REPO_ROOT / "tests/build_module_slice_ui_tests.rs").read_text(
        encoding="utf-8"
    )
    forbidden = {
        "Command": "spawning a child process puts preparation back in the test",
        "BuildSliceDependencies": (
            "the retired artefact resolver must not return; its compiler work "
            "belongs to the build phase now"
        ),
    }
    present = {token: reason for token, reason in forbidden.items() if token in source}
    assert not present, f"the timed test reintroduced compiler work: {present}"
    assert 'join("build_slice_fixture/slice.rs")' in source, (
        "the source-only test must read the fixture composition it compares "
        "against build.rs"
    )
