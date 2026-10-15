"""Wire the release-publication modules into the gates that run them.

A suite that nothing collects is worse than a suite that does not exist. It
passes `make test`, it passes review, and it reports nothing; the failure it
was written to catch goes on happening. That is the shape Cuprum #499 records,
and it is the specific hazard here: the release-candidate, semver, and
inventory modules arrived as new files under ``scripts/tests``, which
``make test`` does not reach at all — it runs the Rust suite. Until
``test-release-publication`` exists and CI invokes it, those modules are inert.

So this contract holds the wiring, not the behaviour. It pins the Makefile
target's command line, the CI step that runs it, the step's position relative
to the workflow-contract gate it must follow, and — the check that actually
closes the hazard — that no ``scripts/tests`` module is left uncollected by any
recipe. The last one is general on purpose: it is what catches the *next*
module, long after this branch is merged.

Run via ``make test-workflow-contracts``.
"""

import re
import typing as typ

import pytest
from makefile_recipes import load_makefile, makefile_recipe
from workflow_loading import (
    CI_WORKFLOW_PATH,
    MAKEFILE_PATH,
    REPO_ROOT,
    job_steps,
    load_workflow,
    named_step,
    step_index_by_key,
)

if typ.TYPE_CHECKING:
    from pathlib import Path

TARGET = "test-release-publication"
CI_STEP_NAME = "Release publication tests"

#: The modules this target owns. Named individually rather than by a glob, so
#: a module silently dropped from the recipe is a failure here and not a
#: smaller test run that still reports success.
SECTION_MODULES = (
    "scripts/tests/test_release_candidate.py",
    "scripts/tests/test_release_inventory.py",
    "scripts/tests/test_release_semver.py",
)

#: Fragments any sibling script-test target carries. The three that matter
#: are the baseline interpreter, the pinned pytest, and the isolated
#: configuration: without `-c /dev/null` a root `pyproject.toml` silently
#: supplies plugins and ini options, and the command stops describing what it
#: runs.
REQUIRED_RECIPE_FRAGMENTS = (
    "PYTHONPATH=scripts",
    "--python $(PYTHON_BASELINE)",
    "--with pytest==9.0.2",
    "--with hypothesis==6.151.9",
    "python -m pytest",
    "-c /dev/null",
    "--rootdir=.",
    "-p no:cacheprovider",
)

#: Matches a test module path in a recipe. Deliberately anchored to the
#: ``scripts/tests/`` directory rather than to any ``test_*.py`` argument:
#: targets such as ``test-workflow-contracts`` also run ``tests/`` and
#: ``.github/`` suites, and a pattern loose enough to catch those would make
#: the orphan check below report them as uncollected.
_TEST_MODULE = re.compile(r"scripts/tests/(test_[A-Za-z0-9_]+\.py)")

_TESTS_DIRECTORY: Path = REPO_ROOT / "scripts" / "tests"


def _collected_modules(makefile: str) -> set[str]:
    """Return every ``scripts/tests`` module name some recipe names.

    Parameters
    ----------
    makefile
        Whole Makefile text to scan.

    Returns
    -------
    set[str]
        Bare module file names, such as ``test_release_semver.py``.

    Notes
    -----
    The whole file is scanned rather than one recipe, because a module may be
    legitimately collected by any target — a new one here, or an existing
    target that grew a module. The question this answers is whether the suite
    is reachable at all, not which target owns it. Mentions inside comments
    count, which is intentional: a comment naming a module still means a
    maintainer knows it is there, and the check's purpose is to catch modules
    nobody wired up, not to audit comment style.
    """
    return set(_TEST_MODULE.findall(makefile))


def _test_module_names(directory: Path) -> set[str]:
    """Return the bare file name of each test module in ``directory``."""
    return {path.name for path in directory.glob("test_*.py")}


def test_the_target_recipe_pins_its_interpreter_and_modules() -> None:
    """Require the recipe to pin its command and name its own modules."""
    recipe = makefile_recipe(TARGET)
    for fragment in REQUIRED_RECIPE_FRAGMENTS:
        assert fragment in recipe, f"{TARGET} must retain {fragment!r} in its command"
    collected = _TEST_MODULE.findall(recipe)
    assert [f"scripts/tests/{name}" for name in collected] == list(SECTION_MODULES), (
        f"{TARGET} must execute exactly the release-publication modules, "
        f"got {collected!r}"
    )


def test_the_target_is_declared_phony() -> None:
    """Require the target to be phony, so a file of that name cannot mask it.

    A target absent from `.PHONY` still runs when its name matches no file, so
    the failure this prevents is specific and quiet: create a directory or file
    called `test-release-publication` and make reports the gate as up to date
    without running a single test.
    """
    phony = next(
        (line for line in load_makefile().splitlines() if line.startswith(".PHONY:")),
        None,
    )
    assert phony is not None, "the Makefile must declare a .PHONY line"
    assert TARGET in phony.split(), (
        f"{TARGET} must be declared phony so a file of that name cannot shadow it"
    )


def test_ci_runs_the_target_after_the_workflow_contract_gate() -> None:
    """Require the pull-request lane to run the target, after its contracts.

    Order is load-bearing and not cosmetic. The workflow-contract suite is
    what asserts this wiring; running the module suites first would mean a
    broken coupling reports as a test failure rather than as a wiring failure,
    which is a harder thing to diagnose.
    """
    steps = job_steps(load_workflow(CI_WORKFLOW_PATH), "build-test")
    contracts = step_index_by_key(steps, "name", "Workflow contract tests")
    publication = step_index_by_key(steps, "name", CI_STEP_NAME)
    assert publication > contracts, (
        f"{CI_STEP_NAME} must run after the workflow-contract gate"
    )
    step = named_step(steps, CI_STEP_NAME)
    assert step.get("run") == f"make {TARGET}", (
        f"CI must invoke the dedicated target with `make {TARGET}`, "
        f"got {step.get('run')!r}"
    )


def test_every_scripts_test_module_is_collected_by_a_recipe() -> None:
    """Reject a test module that no Makefile recipe names.

    This is the orphan check, and it is the reason this module exists. It
    fails when a new module lands under ``scripts/tests`` without a target to
    run it — the state the release-publication modules were in before
    ``test-release-publication`` was written.

    It is deliberately blind to *which* recipe does the collecting. The
    question is reachability, and a module named by any target is reachable.
    """
    makefile = load_makefile()
    collected = _collected_modules(makefile)
    orphaned = sorted(_test_module_names(_TESTS_DIRECTORY) - collected)
    assert not orphaned, (
        "these test modules are not run by any Makefile recipe, so their "
        f"assertions never execute in CI: {orphaned}"
    )


def test_the_orphan_check_would_notice_an_uncoupled_module() -> None:
    """Exercise the orphan check against a module no recipe could name.

    Without this, :func:`test_every_scripts_test_module_is_collected_by_a_recipe`
    could pass because its set arithmetic is wrong — both sides empty, or the
    pattern matching nothing — rather than because every module is wired. The
    check is composed here exactly as that test composes it, against a
    synthetic module name that cannot be in the Makefile.
    """
    makefile = load_makefile()
    collected = _collected_modules(makefile)
    unwired = "test_deliberately_uncoupled.py"
    assert unwired not in collected, (
        "the synthetic module must be absent, or this check proves nothing"
    )
    assert _TEST_MODULE.fullmatch(f"scripts/tests/{unwired}"), (
        "the pattern must match a well-formed module path"
    )
    assert {unwired} - collected == {unwired}, (
        "a module named by no recipe must survive set subtraction, so the "
        "orphan check reports it"
    )
    # Pin the other direction too: a module that IS wired must be subtracted
    # away, or the check would report every module and be equally useless.
    wired = "test_release_semver.py"
    assert wired in collected, "a wired module must be found in the Makefile"
    assert {wired} - collected == set(), (
        "a wired module must be subtracted away, not reported"
    )


@pytest.mark.parametrize("module", SECTION_MODULES)
def test_each_section_module_exists_on_disk(module: str) -> None:
    """Require the recipe's named modules to be files that are actually there.

    A recipe naming a deleted module still exits non-zero, so this is not the
    failure it prevents. It prevents the reverse: pytest reports "no tests
    ran" as a successful exit when a file argument matches nothing under some
    configurations, and a recipe pointing at a module that was renamed away
    would then look like a passing gate.
    """
    path = REPO_ROOT / module
    assert path.is_file(), f"{module} must exist, or the {TARGET} gate is vacuous"


def test_the_makefile_is_the_one_this_check_reads() -> None:
    """Require the checked Makefile to be the repository's own.

    Every assertion above reads a file, and a mis-resolved path would have
    them all read a fixture or fail obscurely. Asserted once, here, so the
    rest can rely on it.
    """
    assert MAKEFILE_PATH == REPO_ROOT / "Makefile", (
        "the Makefile path must resolve to the repository root"
    )
    assert MAKEFILE_PATH.is_file(), f"{MAKEFILE_PATH} must exist"
