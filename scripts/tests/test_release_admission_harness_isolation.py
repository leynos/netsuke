r"""Pin the isolated environment the runtime harness hands each gate run.

Every other runtime module starts its gate through
:func:`release_admission_test_harness.run_gate`, which builds the child's
environment from the harness's own inputs. That construction is the one place
the suite could stop testing what it declares without any case noticing, so it
is asserted here directly rather than through a run: a child that inherited a
control the harness never set would still produce a plausible run, and the
failure would then read as a defect in the gate.

Which values the harness sets is pinned where they are used -- the configured
artefact paths in :mod:`test_release_admission_environment_contract`, the
documented defaults in :mod:`test_release_admission_default_forms`. What is
left, and what only this module states, is that nothing else under the
admission prefix survives into the child.

Example (run from the repository root)::

    PYTHONPATH=scripts uv run --no-project --python 3.14 \
        --with pytest==9.0.2 --with 'cmd-mox==0.2.0' --with 'cuprum==0.1.0' \
        --with 'cyclopts==4.25.3' \
        python -m pytest \
        scripts/tests/test_release_admission_harness_isolation.py \
        -c /dev/null --rootdir=. -p no:cacheprovider
"""

import os
import typing as typ

from release_admission_test_support import (
    MISSING,
    GateInputs,
    gate_environment,
    gate_paths,
)

if typ.TYPE_CHECKING:
    import pathlib

    from cmd_mox import CmdMox

pytest_plugins = ("cmd_mox.pytest_plugin",)

TAG_VARIABLE = "NETSUKE_RELEASE_ADMISSION_TAG"
ENFORCE_VARIABLE = "NETSUKE_RELEASE_ADMISSION_ENFORCE"
TIMEOUT_VARIABLE = "NETSUKE_RELEASE_ADMISSION_OPERATION_TIMEOUT_SECONDS"
METRICS_FILE_VARIABLE = "NETSUKE_RELEASE_ADMISSION_METRICS_FILE"

#: An unrelated variable, which must survive: the harness drops two prefixes
#: rather than emptying the environment a case inherits.
KEPT_VARIABLE = "NETSUKE_HARNESS_TEST_KEPT"
INHERITED_TAG = "v1.2.3-inherited"
CASE_TAG = "v9.9.9-case"

#: The Git variables a hook-run suite exports for the repository that started
#: it, which name a repository the case never chose.
GIT_LOCATION_VARIABLES = ("GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE")


def test_inherited_admission_controls_do_not_reach_the_child(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
) -> None:
    """Verify an exported admission control cannot decide what a case runs.

    Notes
    -----
    The three inherited values are chosen for what each would change rather
    than for coverage: the tag turns the default rehearsal into a tagged run,
    the mode turns it into an *enforcing* one whose evidence then has to match,
    and the timeout shortens every operation's deadline. None of the three is
    the harness's to accept from its own parent, and the case's own tag arriving
    intact is what shows the filter drops the inherited values alone.
    """
    inherited = {
        KEPT_VARIABLE: "kept",
        "PATH": os.environ["PATH"],
        TAG_VARIABLE: INHERITED_TAG,
        ENFORCE_VARIABLE: "true",
        TIMEOUT_VARIABLE: "1",
    }
    paths = gate_paths(tmp_path)
    environment = gate_environment(
        cmd_mox,
        GateInputs(tmp_path, MISSING, {TAG_VARIABLE: CASE_TAG}, paths),
        inherited,
    )

    assert environment[KEPT_VARIABLE] == "kept", (
        "the harness must drop admission controls only, not the whole environment"
    )
    assert environment.get(TAG_VARIABLE) == CASE_TAG, (
        "the case's own tag must still reach the child"
    )
    assert ENFORCE_VARIABLE not in environment, (
        "an inherited mode must not decide whether the child enforces admission"
    )
    assert TIMEOUT_VARIABLE not in environment, (
        "an inherited timeout must not decide the child's operation bounds"
    )
    assert environment[METRICS_FILE_VARIABLE] == str(paths["metrics"]), (
        "the harness's own artefact path must survive its filter"
    )
    shim_dir = cmd_mox.environment.shim_dir
    assert shim_dir is not None, "CmdMox must create its command shim directory"
    assert environment["PATH"].split(os.pathsep)[0] == str(shim_dir), (
        "the controller's shims must still lead the child's PATH"
    )


def test_an_inherited_git_repository_location_does_not_reach_the_child(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
) -> None:
    """Verify the child resolves Git from its working directory alone.

    Notes
    -----
    These cases are the ones that choose a working directory, and Git resolves
    ``GIT_DIR`` and its companions before it looks at one. A hook-run suite
    exports them for the repository that started the run, so an inherited value
    would decide which repository the gate's fetch reads while every assertion
    in the case still named the checkout it had chosen.
    """
    inherited = {"PATH": os.environ["PATH"]}
    inherited.update(dict.fromkeys(GIT_LOCATION_VARIABLES, "/inherited/elsewhere"))
    paths = gate_paths(tmp_path)
    environment = gate_environment(
        cmd_mox,
        GateInputs(tmp_path, MISSING, None, paths),
        inherited,
    )

    surviving = [name for name in GIT_LOCATION_VARIABLES if name in environment]
    assert not surviving, (
        f"an inherited Git repository location must not reach the child: {surviving}"
    )
    assert environment["PATH"].split(os.pathsep)[0] == str(
        cmd_mox.environment.shim_dir
    ), "the controller's shims must still lead the child's PATH"
