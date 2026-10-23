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

import typing as typ

import pytest
from makefile_recipes import load_makefile, makefile_recipe
from release_publication_imports import (
    any_offender,
    imported_modules,
    network_offenders,
)
from release_publication_wiring import (
    TEST_MODULE,
    collected_modules,
    docstring_example_count,
    suite_module_names,
)
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

#: The test modules this target owns. Named individually rather than by a
#: glob, so a module silently dropped from the recipe is a failure here and
#: not a smaller test run that still reports success.
SECTION_MODULES = (
    "scripts/tests/test_release_candidate.py",
    "scripts/tests/test_release_candidate_cli.py",
    "scripts/tests/test_package_crate_cli.py",
    "scripts/tests/test_publish_crates_io_cli.py",
    "scripts/tests/test_verify_release_inventory_cli.py",
    "scripts/tests/test_finalize_github_release_cli.py",
    "scripts/tests/test_release_inventory.py",
    "scripts/tests/test_release_semver.py",
    "scripts/tests/test_release_digest.py",
    "scripts/tests/test_verify_crates_io_bytes.py",
    "scripts/tests/test_release_reconcile_crates.py",
    "scripts/tests/test_release_reconcile_github.py",
    "scripts/tests/test_release_declaration.py",
    "scripts/tests/test_release_declaration_cli.py",
)

#: The modules under test, named on the command line so `--doctest-modules`
#: has something to collect. `pytest` with the flag but no module argument
#: collects examples only in what it already walks, so dropping these leaves
#: the flag present and every example unrun — a state that looks identical to
#: success from the outside.
SOURCE_MODULES = (
    "scripts/release_candidate.py",
    "scripts/release_candidate_cli.py",
    "scripts/release_identity_record.py",
    "scripts/package_crate_cli.py",
    "scripts/publish_crates_io_cli.py",
    "scripts/verify_release_inventory_cli.py",
    "scripts/finalize_github_release_cli.py",
    "scripts/release_inventory.py",
    "scripts/release_semver.py",
    "scripts/release_digest.py",
    "scripts/verify_crates_io_bytes.py",
    "scripts/release_registry_response.py",
    "scripts/release_reconcile_crates.py",
    "scripts/release_reconcile_github.py",
    "scripts/release_asset_names.py",
    "scripts/release_package_formats.py",
    "scripts/release_declaration.py",
    "scripts/release_declaration_cli.py",
)

#: Fragments any sibling script-test target carries. The three that matter
#: are the baseline interpreter, the pinned pytest, and the isolated
#: configuration: without `-c /dev/null` a root `pyproject.toml` silently
#: supplies plugins and ini options, and the command stops describing what it
#: runs. `--doctest-modules` is here because the source modules' examples are
#: evidence the plan claims; without the flag they are prose nothing executes.
REQUIRED_RECIPE_FRAGMENTS = (
    "PYTHONPATH=scripts",
    "--python $(PYTHON_BASELINE)",
    "--with pytest==9.0.2",
    "--with hypothesis==6.151.9",
    "python -m pytest",
    "-c /dev/null",
    "--rootdir=.",
    "-p no:cacheprovider",
    "--doctest-modules",
)

_TESTS_DIRECTORY: Path = REPO_ROOT / "scripts" / "tests"


def test_the_target_recipe_pins_its_interpreter_and_modules() -> None:
    """Require the recipe to pin its command and name its own modules."""
    recipe = makefile_recipe(TARGET)
    for fragment in REQUIRED_RECIPE_FRAGMENTS:
        assert fragment in recipe, f"{TARGET} must retain {fragment!r} in its command"
    collected = TEST_MODULE.findall(recipe)
    assert [f"scripts/tests/{name}" for name in collected] == list(SECTION_MODULES), (
        f"{TARGET} must execute exactly the release-publication modules, "
        f"got {collected!r}"
    )
    for module in SOURCE_MODULES:
        assert module in recipe, (
            f"{TARGET} must name {module} on its command line, or "
            f"--doctest-modules has no source to collect and the examples in "
            f"that module are prose nothing executes"
        )


@pytest.mark.parametrize("module", SOURCE_MODULES)
def test_each_source_module_carries_docstring_examples(module: str) -> None:
    """Require the named sources to hold examples, so the flag is not vacuous.

    ``--doctest-modules`` over a module with no ``>>>`` line collects nothing
    and reports success either way, so the flag's presence alone does not show
    the examples ran. This asserts there is something for it to find, which
    makes the target's doctest count a fixed number rather than a value that
    can quietly fall to zero.

    Counted over the parsed docstrings rather than over the raw text. A module
    whose ``>>>`` appears in its own module docstring's *prose* — the paragraph
    introducing the convention, say — would satisfy a substring probe while
    pytest collects no example at all, and the check would report coverage
    that does not exist.
    """
    source = (REPO_ROOT / module).read_text(encoding="utf-8")
    examples = docstring_example_count(source)
    assert examples, (
        f"{module} carries no docstring example, so naming it on the "
        f"{TARGET} command line adds no coverage; either restore an example "
        f"or drop the module from SOURCE_MODULES"
    )


def test_a_prompt_outside_a_string_constant_does_not_count() -> None:
    """Reject a prompt that is not in a docstring, so the check is not textual.

    The control for :func:`test_each_source_module_carries_docstring_examples`.
    A prompt in a comment is prose to pytest and an example to a substring
    probe, and the two sources differ in exactly the way this check exists to
    notice. Both halves are needed: the comment must count zero *and* a real
    docstring prompt must still count one, or the function returns zero for
    everything and the parameterised test fails for every module instead of
    catching one.
    """
    assert docstring_example_count("# >>> 1 + 1\n") == 0, (
        "a prompt in a comment must not count as a docstring example"
    )
    documented = 'def f():\n    """Add.\n\n    >>> 1 + 1\n    2\n    """\n'
    assert docstring_example_count(documented) == 1, (
        "a prompt inside a docstring must still count"
    )


@pytest.mark.parametrize("module", SOURCE_MODULES)
def test_policy_modules_import_no_network_client(module: str) -> None:
    """Require every source module to be testable with no network at all.

    EP-M2 states the policy modules perform no network I/O and each is runnable
    locally without one. The statement is only worth making if something
    checks it: an import added later would otherwise leave the claim in prose
    while the suite silently needed a socket. The verifier is the module most
    at risk — its name says "download" — so it takes a local path and returns
    a digest, and the workflow step that writes that file is what downloads.

    The CLI seams are held to the same rule deliberately, even though they do
    perform I/O. A CLI writes the record its job needs, but the *network* half
    of that job belongs to the workflow step, which is where GitHub's own
    credentials and OIDC exchange live. Keeping the seam socket-free is what
    makes the boundary legible: anything that reaches the network is a step,
    never a Python module.
    """
    source = (REPO_ROOT / module).read_text(encoding="utf-8")
    offenders = network_offenders(source)
    assert not offenders, (
        f"{module} imports {offenders!r}, so the module is no longer pure "
        f"policy; move the request into the workflow step and pass the "
        f"resulting path in"
    )


def test_the_no_network_check_would_notice_an_offender() -> None:
    """Exercise the detector on a module that does open a connection.

    Without this, :func:`test_policy_modules_import_no_network_client` could
    pass because :func:`imported_modules` parses nothing or because the banned
    set matches no spelling. The positive control is a synthetic source, not a
    fixture file, so it cannot be mistaken for a real module.
    """
    positives = (
        "import urllib.request",
        "from urllib.request import urlopen",
        "import http.client as client",
        "from socket import create_connection",
    )
    for source in positives:
        assert any_offender(imported_modules(source)), (
            f"the detector must flag {source!r}, or the no-network check is vacuous"
        )
    negatives = (
        "import release_digest",
        "from pathlib import Path",
        "import typing as typ",
    )
    for source in negatives:
        assert not any_offender(imported_modules(source)), (
            f"the detector must not flag {source!r}, a non-network import"
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
    collected = collected_modules(makefile)
    orphaned = sorted(suite_module_names(_TESTS_DIRECTORY) - collected)
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
    collected = collected_modules(makefile)
    unwired = "test_deliberately_uncoupled.py"
    assert unwired not in collected, (
        "the synthetic module must be absent, or this check proves nothing"
    )
    assert TEST_MODULE.fullmatch(f"scripts/tests/{unwired}"), (
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


def test_a_module_named_only_in_a_comment_is_not_collected() -> None:
    """Reject a comment mention, so the orphan check cannot be satisfied by prose.

    The scan reads the Makefile through :func:`collected_modules`, which
    blanks comments before matching. Without that, a recipe could lose its
    module argument and still pass: the explanatory comment beside it would
    name the module and the check would report it as wired. The synthetic
    text here is the exact shape that hides the fault — a live target line
    and a comment line that mentions a module the target no longer runs.
    """
    synthetic = (
        "test-synthetic:\n"
        "\tpython -m pytest scripts/tests/test_live.py\n"
        "# test-retired.py was dropped when the suite moved\n"
    )
    assert collected_modules(synthetic) == {"test_live.py"}, (
        "a module named only in a comment must not count as collected; the "
        "scan must read the recipe, not the prose around it"
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
