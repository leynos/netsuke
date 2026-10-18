"""Hold the single-execution rule for Linux tests.

One instrumented run measures coverage and executes the suite together, so the
lane compiles the workspace once per pull request instead of twice. That only
stays honest while the instrumented invocation is as broad as an
uninstrumented one would have been, so these checks pin its flags, pin the
doctest pass that `cargo llvm-cov nextest` cannot perform, and reject a second
Linux job quietly reintroducing `cargo nextest` or `cargo test`.

The Makefile reading those checks lean on lives in ``makefile_recipes.py``, so
this module states the coverage contract and that one owns the file access.

Run via ``make test-workflow-contracts``.
"""

import re
import tomllib
import typing as typ

import makefile_recipes
import pytest
from cache_contract_data import WORKFLOW_DIR
from makefile_recipes import load_makefile, makefile_recipe
from makefile_variables import expand_makefile_variables, makefile_definitions
from workflow_loading import (
    REPO_ROOT,
    job_steps,
    load_workflow,
    named_step,
    require_mapping,
    workflow_job,
)

if typ.TYPE_CHECKING:
    from pathlib import Path

#: Inputs that make the coverage run as broad as `make test` was. Without
#: `all-features` the `legacy-digests` tests in `src/stdlib/path/hash_utils.rs`
#: and `tests/std_filter_tests/hash_filters.rs` stop running; without
#: `all-targets` the two `benches/` targets stop compiling; without `doctests`
#: nothing runs the doctests, which `cargo llvm-cov nextest` cannot execute.
REQUIRED_COVERAGE_INPUTS = {
    "all-features": "true",
    "all-targets": "true",
    "doctests": "true",
}

#: Ambient-target UI checks select only the library whose artefacts they use.
#: Isolated fixture and packaging builds intentionally retain shipped defaults.
GATE_FEATURES_MODULE: Path = REPO_ROOT / "tests" / "support" / "cargo_features.rs"
TEST_SUPPORT_MANIFEST: Path = REPO_ROOT / "test_support" / "Cargo.toml"
AMBIENT_TARGET_NESTED_BUILD_PACKAGES: tuple[tuple[str, Path], ...] = (
    ("test_support", REPO_ROOT / "tests" / "support" / "test_support_rlib.rs"),
    ("netsuke-build", REPO_ROOT / "tests" / "command_env_ui_tests.rs"),
)
AMBIENT_TARGET_NESTED_BUILD_SOURCES: tuple[Path, ...] = tuple(
    source for _, source in AMBIENT_TARGET_NESTED_BUILD_PACKAGES
)

#: Every job that measures coverage, and the event it is restricted to. Two
#: coverage runs against one commit pay twice and give the ratchet baseline
#: two writers.
COVERAGE_PRODUCERS = {
    ("ci.yml", "build-test"): "github.event_name == 'pull_request'",
    ("coverage-main.yml", "coverage-upload"): None,
}


def test_definitions_are_substituted_one_level_and_unknown_names_left_alone() -> None:
    """Check the pure expansion against supplied definitions, with no file read.

    The helper is deliberately a plain substitution so that its behaviour can
    be pinned here, and so a recipe's flag is never asserted against a value
    that came from somewhere the test did not name.
    """
    definitions = {"GATE_RUSTFLAGS": 'RUSTFLAGS="$(RUSTFLAGS:+$RUSTFLAGS )-D warnings"'}
    text = "$(GATE_RUSTFLAGS) $(CARGO) nextest run $(UNDEFINED)"

    expanded = expand_makefile_variables(text, definitions)

    assert expanded == (
        'RUSTFLAGS="$(RUSTFLAGS:+$RUSTFLAGS )-D warnings" '
        "$(CARGO) nextest run $(UNDEFINED)"
    ), "one level only: nested references survive for the shell or Make to resolve"
    assert expand_makefile_variables("nothing to do", {}) == "nothing to do", (
        "text naming no variable must pass through untouched"
    )
    assert makefile_definitions("FOO ?= bar\nBAZ = qux\nnot a definition\n") == {
        "FOO": "bar",
        "BAZ": "qux",
    }, "both assignment spellings count, and prose does not"


def test_the_loader_reports_an_unreadable_makefile_with_its_path(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    """Fail the loader test, not the assertion, when the Makefile is unreadable.

    The pure helpers take text, so the fallible read is the whole of the
    boundary, and this is where it owes the caller a diagnosis. A missing file
    and undecodable bytes are the two ways in; the second is the one worth
    driving, because it is what a UTF-8 read of a file written by a non-UTF-8
    editor looks like.

    Both readers are driven, not just :func:`load_makefile`: the recipe reader
    reaches the same path, and a contract that only exercised the loader would
    report the recipe reader's own read as covered while it failed with a bare
    ``OSError`` beside an assertion about a gate's flags.
    """
    # Patch the reader's own module: that is where the path is looked up, and a
    # patch against this module's namespace would silently do nothing.
    broken = tmp_path / "Makefile"
    broken.write_bytes(b"\xff\xfe not utf-8")
    monkeypatch.setattr(makefile_recipes, "MAKEFILE_PATH", broken)
    with pytest.raises(
        pytest.fail.Exception, match=re.escape(f"could not read {broken}")
    ):
        load_makefile()
    with pytest.raises(
        pytest.fail.Exception, match=re.escape(f"could not read {broken}")
    ):
        makefile_recipe("test-nextest")

    missing = tmp_path / "absent" / "Makefile"
    monkeypatch.setattr(makefile_recipes, "MAKEFILE_PATH", missing)
    with pytest.raises(
        pytest.fail.Exception, match=re.escape(f"could not read {missing}")
    ):
        load_makefile()
    with pytest.raises(
        pytest.fail.Exception, match=re.escape(f"could not read {missing}")
    ):
        makefile_recipe("test-nextest")


@pytest.mark.parametrize(
    ("workflow_name", "job_name"), sorted(COVERAGE_PRODUCERS, key=str)
)
def test_the_instrumented_run_is_as_broad_as_an_uninstrumented_one(
    workflow_name: str, job_name: str
) -> None:
    """Require every coverage run to execute the whole suite.

    The instrumented run replaced a separate `cargo nextest` execution, so
    narrowing its features or targets would silently retire tests rather than
    remove duplicated work.
    """
    workflow = load_workflow(WORKFLOW_DIR / workflow_name)
    step = named_step(job_steps(workflow, job_name), "Test and Measure Coverage")
    inputs = step.get("with")
    assert isinstance(inputs, dict), f"{workflow_name} {job_name} must pass inputs"
    for name, expected in REQUIRED_COVERAGE_INPUTS.items():
        assert inputs.get(name) == expected, (
            f"{workflow_name} {job_name} must pass {name}={expected!r}, "
            f"got {inputs.get(name)!r}"
        )


@pytest.mark.parametrize(
    ("workflow_name", "job_name"), sorted(COVERAGE_PRODUCERS, key=str)
)
def test_warnings_are_denied_through_the_toolchain_setup(
    workflow_name: str, job_name: str
) -> None:
    """Require `-D warnings` to reach the instrumented run.

    The job declares no `env.RUSTFLAGS`; `tests/polonius_toolchain_contract.rs`
    holds that. `setup-rust` exports the flag instead, and `cargo llvm-cov`
    appends its instrumentation to whatever it finds, so the setting survives.
    """
    workflow = load_workflow(WORKFLOW_DIR / workflow_name)
    setup = named_step(job_steps(workflow, job_name), "Setup Rust")
    inputs = setup.get("with")
    assert isinstance(inputs, dict), "Setup Rust must declare inputs"
    assert inputs.get("rustflags") == "-D warnings", (
        f"{workflow_name} {job_name} must deny warnings through setup-rust"
    )


def test_no_bespoke_doctest_step_shadows_the_action() -> None:
    """Require the doctest pass to come from the coverage action.

    The action runs `cargo test --doc --workspace` under the same feature
    selection as the instrumented run, so a hand-rolled step beside it would
    state that selection twice and let the two drift apart.
    """
    steps = job_steps(load_workflow(WORKFLOW_DIR / "ci.yml"), "build-test")
    assert not [step for step in steps if step.get("name") == "Doctests"], (
        "the coverage action's doctests input replaced the bespoke step"
    )


def test_the_local_test_target_still_runs_both_passes() -> None:
    """Keep `make test` running what CI runs, for local parity.

    CI drives the doctests through the coverage action, but a contributor runs
    `make test`, so the target must still compose both passes with the same
    breadth.
    """
    makefile = load_makefile()
    assert "test: test-nextest doctest" in makefile, (
        "`make test` must compose the nextest and doctest passes"
    )
    definitions = makefile_definitions(makefile)
    nextest = expand_makefile_variables(makefile_recipe("test-nextest"), definitions)
    for flag in ("--workspace", "--all-targets", "--all-features"):
        assert flag in nextest, f"the local nextest pass must pass {flag}"
    doctest = expand_makefile_variables(makefile_recipe("doctest"), definitions)
    for flag in ("--workspace", "--doc", "--all-features"):
        assert flag in doctest, f"the local doctest pass must pass {flag}"
    for recipe, label in ((nextest, "nextest"), (doctest, "doctest")):
        assert "-D warnings" in recipe, f"the local {label} pass must deny warnings"


def test_ambient_target_harnesses_check_only_the_library_they_consume() -> None:
    """Keep direct-rustc preparation within each fixture's dependency graph.

    The support fixture selects its library; the production fixtures select
    the application library, which also runs its build script and prepares its
    build dependencies. None needs Cargo to check every workspace target
    before compiling a small direct-rustc fixture.
    """
    # `lint` marks the all-features lane; the default-features lane passes no
    # feature selection, so a nested check must not add `--all-features` there
    # or Cargo recompiles the graph.
    lanes = dict(
        re.findall(
            r'#\[cfg\((feature = "lint"|not\(feature = "lint"\))\)\]\s*'
            r"pub const LIBRARY_BUILD_ARGUMENTS:\s*&\[&str\]\s*=\s*&\[(.*?)\];",
            GATE_FEATURES_MODULE.read_text(encoding="utf-8"),
            re.DOTALL,
        )
    )
    for lane, expected in (
        ('feature = "lint"', ["--lib", "--all-features"]),
        ('not(feature = "lint")', ["--lib"]),
    ):
        arguments = re.findall(r'"([^"]+)"', lanes.get(lane, ""))
        assert arguments == expected, f"cfg({lane}) must select exactly {expected}"
    for package, source in AMBIENT_TARGET_NESTED_BUILD_PACKAGES:
        text = source.read_text(encoding="utf-8")
        required_composition = (
            f'args(["--package", "{package}"])',
            "cargo_features::LIBRARY_BUILD_ARGUMENTS",
            'manifest_dir().join("Cargo.toml")',
            '.arg("check")',
        )
        missing = [
            fragment for fragment in required_composition if fragment not in text
        ]
        assert not missing, (
            f"{source.relative_to(REPO_ROOT)} must compose Cargo check with "
            f"the {package} library selector and root manifest; missing {missing!r}"
        )
    manifest = tomllib.loads(TEST_SUPPORT_MANIFEST.read_text(encoding="utf-8"))
    features = manifest.get("features")
    assert isinstance(features, dict), "test_support must declare its features"
    assert features.get("default") == [], "test_support defaults must stay empty"
    assert features.get("legacy-digests") == ["netsuke/legacy-digests"], (
        "test_support must forward legacy-digests exactly to netsuke-build"
    )




@pytest.mark.parametrize(
    "source",
    AMBIENT_TARGET_NESTED_BUILD_SOURCES,
    ids=[source.name for source in AMBIENT_TARGET_NESTED_BUILD_SOURCES],
)
def test_ambient_ui_harnesses_do_not_rebuild_in_isolated_targets(source: Path) -> None:
    """Keep public API checks on the ambient target directory.

    The cached CLI configuration fixture previously selected a dedicated
    target directory and rebuilt Netsuke's dependencies, exceeding Nextest's
    300-second budget on a shared build host.
    """
    text = source.read_text(encoding="utf-8")
    assert not re.search(r'\.env\(\s*"CARGO_TARGET_DIR"', text), (
        f"{source.relative_to(REPO_ROOT)} must reuse ambient build artefacts "
        "rather than compile into an isolated Cargo target directory"
    )


def test_cold_packaging_verification_has_a_scoped_budget() -> None:
    """Allow measured cold package compilation without relaxing ordinary tests."""
    config = tomllib.loads(
        (REPO_ROOT / ".config" / "nextest.toml").read_text(encoding="utf-8")
    )
    profile = config["profile"]["default"]
    assert profile["slow-timeout"] == {"period": "60s", "terminate-after": 5}, (
        "ordinary tests must retain their five-minute budget"
    )
    timed_overrides = [
        override for override in profile["overrides"] if "slow-timeout" in override
    ]
    assert len(timed_overrides) == 4, (
        "only remaining nested builds may need extended test budgets"
    )
    configured = {
        override["filter"]: override["slow-timeout"] for override in timed_overrides
    }
    expected = {
        "test(/^packaged_manifest_retains_build_script_sources($|::)/)": {
            "period": "60s",
            "terminate-after": 15,
        },
        (
            "test(/^compile_guard::"
            "every_patched_tree_compiles_under_denied_warnings($|::)/)"
        ): {
            "period": "60s",
            "terminate-after": 10,
        },
    }
    expected.update({
        f"test(/^{name}($|::)/)": {"period": "60s", "terminate-after": periods}
        for name, periods in (
            ("split_build_fixture_compiles_through_the_direct_rustc_harness", 15),
            ("stub_env_default_does_not_compile", 15),
        )
    })
    assert configured == expected, (
        "cold-build budgets must retain current-main allowances"
    )


def test_packaging_verification_preserves_cargo_build_configuration() -> None:
    """Reuse Cargo build configuration while retaining package verification."""
    source = (REPO_ROOT / "tests" / "packaging_smoke_tests.rs").read_text(
        encoding="utf-8"
    )
    forbidden = (
        r"\b(?:TempDir|tempdir(?:_in)?)\b|"
        r'\.env(?:_remove)?\(\s*"CARGO_(?:TARGET_DIR|BUILD_TARGET_DIR|BUILD_BUILD_DIR)"'
    )
    assert not re.search(forbidden, source), "preserve Cargo's build placement"
    required = (
        ".args(PUBLISH_DRY_RUN_ARGS)",
        '.args(["package", "--list", "--allow-dirty", "-p", "netsuke-build"])',
        "assert_required_paths_present(&packaged_paths, &readme_paths, &source_paths);",
        "assert_forbidden_roots_absent(&packaged_paths);",
    )
    assert all(fragment in source for fragment in required), (
        "package verification and source-list assertions must remain intact"
    )
    linux = source.split("} else {", maxsplit=1)[1].split("};", maxsplit=1)[0]
    assert '"--no-verify"' not in linux, "Linux must verify the extracted package"


@pytest.mark.parametrize(
    ("workflow_name", "job_name", "expected_condition"),
    [
        (workflow, job, condition)
        for (workflow, job), condition in COVERAGE_PRODUCERS.items()
    ],
)
def test_one_coverage_producer_per_event(
    workflow_name: str, job_name: str, expected_condition: str | None
) -> None:
    """Keep exactly one job measuring coverage for any given commit."""
    workflow = load_workflow(WORKFLOW_DIR / workflow_name)
    step = named_step(job_steps(workflow, job_name), "Test and Measure Coverage")
    assert step.get("if") == expected_condition, (
        f"{workflow_name} {job_name} must measure coverage under "
        f"{expected_condition!r}, got {step.get('if')!r}"
    )


#: The instrumented lanes' budget for one `cargo llvm-cov nextest` invocation,
#: in seconds. The shared coverage action defaults to 600, which was sized
#: against a lane that restored a `target` archive and so never paid for a cold
#: compile.
CARGO_WAIT_TIMEOUT = "1980"


@pytest.mark.parametrize(
    ("workflow_name", "job_name"), sorted(COVERAGE_PRODUCERS, key=str)
)
def test_instrumented_lanes_budget_for_a_cold_build(
    workflow_name: str, job_name: str
) -> None:
    """Require both instrumented lanes to raise the cargo watchdog.

    The shared coverage action wraps `cargo llvm-cov nextest` in a watchdog
    that defaults to 600 seconds. That budget assumed a restored `target`
    archive; this repository archives no build tree, so a cold sccache store
    leaves the whole instrumented build to do inside it. The first trunk run
    after the runner migration failed exactly there: all 2,790 tests passed,
    taking about 512 seconds at 19.42% sccache hits, and the watchdog killed
    cargo 88 seconds later during report generation.

    Held for both producers, not only the lane that failed. The merge gate
    runs the same action on pull requests and meets the same wall whenever its
    store is cold, which is the case a green trunk run would otherwise hide.
    """
    job = workflow_job(load_workflow(WORKFLOW_DIR / workflow_name), job_name)
    env = require_mapping(job.get("env"), f"{job_name} env")
    assert env.get("RUN_RUST_CARGO_WAIT_TIMEOUT") == CARGO_WAIT_TIMEOUT, (
        f"{workflow_name} {job_name} must budget "
        f"{CARGO_WAIT_TIMEOUT}s for one instrumented cargo run, got "
        f"{env.get('RUN_RUST_CARGO_WAIT_TIMEOUT')!r}"
    )
