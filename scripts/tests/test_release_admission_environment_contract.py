r"""Pin the release-admission gate's environment and artefact-order contract.

Three orderings in the shell are observable from outside it, and each is a
position rather than a value:

1. ``GITHUB_REPOSITORY`` and ``GITHUB_SHA`` are refused before any artefact
   exists, so a refusal on either of them leaves *no* metrics file and *no*
   trace file behind.
2. Both artefact parents are created before *either* file is truncated, so a
   refused trace parent leaves a previous metrics file exactly as it was.
3. ``GITHUB_OUTPUT`` is refused after both artefacts have been created, so that
   refusal leaves two empty artefacts and no ``gate_complete`` trace, because
   the shell armed its exit trap only after the check.

The cases here are the ones no other module can express. Everything else drives
a gate whose artefacts are already readable, and a run that correctly creates
nothing is only assertable once the harness tolerates an absent file. The
non-canonical path spelling is asserted in
:mod:`test_release_admission_default_forms` instead, because it is a statement
about the *value* a configured input resolves to rather than about a refusal
position, and one case with two homes is one case too many.

The two Bash default forms are the other half of this contract. They are pinned
in :mod:`test_release_admission_default_forms`, because they are about the
*values* an input resolves to rather than the *order* the gate refuses them in.

Example (run from the repository root)::

    PYTHONPATH=scripts uv run --no-project --python 3.14 \
        --with pytest==9.0.2 --with 'cmd-mox==0.2.0' --with 'cuprum==0.1.0' \
        --with 'cyclopts==4.25.3' \
        python -m pytest \
        scripts/tests/test_release_admission_environment_contract.py \
        -c /dev/null --rootdir=. -p no:cacheprovider
"""

import os
import typing as typ

import pytest
from release_admission_test_support import (
    REVISION,
    install_boundaries,
    run_gate,
)

if typ.TYPE_CHECKING:
    import pathlib

    from cmd_mox import CmdMox

pytest_plugins = ("cmd_mox.pytest_plugin",)

REPOSITORY_VARIABLE = "GITHUB_REPOSITORY"
REVISION_VARIABLE = "GITHUB_SHA"
OUTPUT_VARIABLE = "GITHUB_OUTPUT"
METRICS_FILE_VARIABLE = "NETSUKE_RELEASE_ADMISSION_METRICS_FILE"
TRACE_FILE_VARIABLE = "NETSUKE_RELEASE_ADMISSION_TRACE_FILE"

#: The artefact names the harness *configures*, which is what a
#: refusal-position case asserts the absence of: the harness names both files
#: explicitly, so neither is the default the gate would otherwise derive.
CONFIGURED_METRICS_NAME = "release-admission-metrics.jsonl"
CONFIGURED_TRACE_NAME = "release-admission-traces.jsonl"
#: The artefact names the gate derives when no path variable is supplied. The
#: ``netsuke-`` prefix distinguishes them from the configured pair above, and
#: using one where the other belongs is the mistake these two constants exist
#: to make visible.
DEFAULT_METRICS_NAME = "netsuke-release-admission-metrics.jsonl"
DEFAULT_TRACE_NAME = "netsuke-release-admission-traces.jsonl"
FALLBACK_DIRECTORY = "/tmp"  # ruff: ignore[hardcoded-temp-file] - the documented fallback, never a temporary file.

#: Required-variable refusals, each naming the variable and the state supplied
#: for it. ``None`` removes the variable; the empty string exports it empty.
REQUIRED_REFUSALS = [
    pytest.param(REPOSITORY_VARIABLE, None, id="repository-unset"),
    pytest.param(REPOSITORY_VARIABLE, "", id="repository-empty"),
    pytest.param(REVISION_VARIABLE, None, id="revision-unset"),
    pytest.param(REVISION_VARIABLE, "", id="revision-empty"),
]


@pytest.mark.parametrize(("variable", "state"), REQUIRED_REFUSALS)
def test_required_variable_refusal_precedes_every_artefact(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
    variable: str,
    state: str | None,
) -> None:
    """Verify a refused required variable leaves no artefact behind.

    Parameters
    ----------
    cmd_mox
        Active controller whose shims stand in for every adapter.
    tmp_path
        Isolated directory that must stay empty of gate files.
    variable
        The required variable this case removes or empties.
    state
        The value the variable is given, or ``None`` to remove it.

    Notes
    -----
    The shell expanded ``GITHUB_REPOSITORY`` and ``GITHUB_SHA`` with ``:?``
    *before* it computed either artefact path, so a refusal on either left the
    filesystem untouched. That is a position, not a side effect: this is the
    only run in the suite that correctly creates nothing, and asserting it
    requires a reader that treats an absent artefact as no records rather than
    as a fault.

    Both states are pinned because the ``:?`` form refuses them alike. A case
    that only removed the variable would also pass for an implementation that
    read an exported empty value as present.
    """
    boundaries = install_boundaries(cmd_mox)
    run = run_gate(cmd_mox, tmp_path, extra_environment={variable: state})

    assert run.result.returncode == 1, "a refused required variable must fail closed"
    assert variable in run.result.stderr, (
        "the refusal must name the variable the operator has to supply"
    )
    for name in (CONFIGURED_METRICS_NAME, CONFIGURED_TRACE_NAME, "github-output"):
        path = tmp_path / name
        assert not path.exists(), (
            f"no artefact may exist after a required-variable refusal; found {path}"
        )
    assert boundaries.gh.calls == [], (
        "a refused configuration must not reach the GitHub adapter"
    )
    assert boundaries.git.calls == [], (
        "a refused configuration must not reach the Git adapter"
    )


def test_trace_parent_refusal_preserves_a_previous_metrics_file(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
) -> None:
    """Verify both artefact parents are created before either file is truncated.

    Notes
    -----
    The shell ran one ``mkdir -p`` naming both parents and only then truncated
    the two files. Interleaving the two would truncate the metrics file before
    discovering that the trace parent could not be created, destroying a
    previous run's records on a configuration the gate then refuses.

    The refusal is induced through the filesystem rather than the environment:
    the trace path's parent is a regular file, so no directory can be made
    there. The metrics file starts with a previous run's bytes and must still
    hold exactly those bytes once the gate has exited.

    The preserved content is compared as *bytes* rather than read back through
    the suite's record reader. Any non-empty text stands in for the previous
    run here, and non-JSON text is the stricter fixture: a reader that parsed
    the file would fail on the content rather than on the truncation this case
    is about.
    """
    install_boundaries(cmd_mox)
    blocker = tmp_path / "blocker"
    blocker.write_text("not a directory\n", encoding="utf-8")
    previous = tmp_path / CONFIGURED_METRICS_NAME
    preserved = b'{"name": "a previous run left this here"}\n'
    previous.write_bytes(preserved)
    refused = blocker / CONFIGURED_TRACE_NAME
    run = run_gate(
        cmd_mox,
        tmp_path,
        extra_environment={
            METRICS_FILE_VARIABLE: str(previous),
            TRACE_FILE_VARIABLE: str(refused),
        },
    )

    assert run.result.returncode == 1, "a refused artefact path must fail closed"
    assert "release-admission could not create the directory" in run.result.stderr, (
        "the refusal must say which artefact it could not create"
    )
    assert str(refused) in run.result.stderr, (
        "the diagnostic must carry the configured path the operator supplied"
    )
    assert "Traceback" not in run.result.stderr, (
        "a filesystem refusal must be reported, not left as an uncaught traceback"
    )
    assert previous.read_bytes() == preserved, (
        "a refused trace parent must leave the metrics file untouched"
    )


def test_missing_output_refusal_leaves_both_artefacts_empty(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
) -> None:
    """Verify the output refusal follows creation and precedes the exit trap.

    Notes
    -----
    ``GITHUB_OUTPUT`` is required, but at a different position from the other
    two: the shell created both artefacts first and armed its exit trap only
    after the check. A refusal here therefore leaves the metrics file and the
    trace file *created and empty*, writes no output, and records no
    ``gate_complete`` trace -- the trap that would have written the delivery
    traces was never armed.
    """
    install_boundaries(cmd_mox)
    run = run_gate(cmd_mox, tmp_path, extra_environment={OUTPUT_VARIABLE: None})

    assert run.result.returncode == 1, "a missing output file must fail closed"
    assert OUTPUT_VARIABLE in run.result.stderr, (
        "the refusal must name the workflow output variable"
    )
    for name in (CONFIGURED_METRICS_NAME, CONFIGURED_TRACE_NAME):
        path = tmp_path / name
        assert path.exists(), f"the output refusal is reached after {name} is created"
        assert path.read_bytes() == b"", f"{name} must be created empty"
    assert not (tmp_path / "github-output").exists(), (
        "a refused output must not be written"
    )
    assert run.outputs == {}, "a refusal before the trap writes no workflow output"
    assert run.traces == [], "a refusal before the trap records no delivery traces"


def test_absent_path_variables_derive_their_defaults_from_runner_temp(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
) -> None:
    """Verify the artefact defaults are derived from the temporary directory.

    Notes
    -----
    The shell read ``${RUNNER_TEMP:-/tmp}`` once and built both default paths
    from it, so the two names are the only fixed part of the default and the
    directory comes from the variable. Pointing ``RUNNER_TEMP`` at a directory
    that is neither the isolated one the rest of the suite uses nor ``/tmp`` is
    what makes this a test of the derivation rather than of a constant: a gate
    that hardcoded either directory would publish the wrong path.

    Both path variables are removed as well as the directory being moved,
    because the harness names an artefact path for every other case and a
    variable that is *set* is what the gate reads first. Moving ``RUNNER_TEMP``
    alone would leave the configured paths in place and prove nothing.

    The published paths are compared as text, because that is what the shell
    published: the workflow output carried the string, not a resolved path.
    The ``/tmp`` fallback itself is asserted separately, as a named constant,
    because reaching it end to end means writing both artefacts outside any
    directory a test owns.
    """
    install_boundaries(cmd_mox)
    runner_temp = tmp_path / "runner-temp"
    run = run_gate(
        cmd_mox,
        tmp_path,
        extra_environment={
            "RUNNER_TEMP": str(runner_temp),
            METRICS_FILE_VARIABLE: None,
            TRACE_FILE_VARIABLE: None,
        },
    )

    assert run.outputs["metrics-file"] == str(runner_temp / DEFAULT_METRICS_NAME), (
        "an absent metrics path must default inside RUNNER_TEMP"
    )
    assert run.outputs["trace-file"] == str(runner_temp / DEFAULT_TRACE_NAME), (
        "an absent trace path must default inside RUNNER_TEMP"
    )
    assert (runner_temp / DEFAULT_METRICS_NAME).exists(), (
        "the default metric artefact must be the file the gate wrote"
    )


def test_required_input_refusal_keeps_the_shells_diagnostic_sentence(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
) -> None:
    """Verify the required-input diagnostics keep the shell's own sentences.

    Notes
    -----
    The shell's ``:?`` form printed ``<path>: line N: <VAR>: <message>``. The
    path and line number are properties of the Bash source and cannot be
    reproduced, so the port keeps the message and drops the prefix; this case
    pins the message so a later edit cannot quietly reword what an operator
    greps for, and pins the absence of anything the prefix used to carry.
    """
    install_boundaries(cmd_mox)
    run = run_gate(cmd_mox, tmp_path, extra_environment={REPOSITORY_VARIABLE: None})

    assert "GITHUB_REPOSITORY must identify the release repository" in (
        run.result.stderr
    ), "the refusal must keep the shell's own diagnostic sentence"
    assert os.sep not in run.result.stderr, (
        "the port must not prefix the sentence with a script path"
    )
    assert REVISION not in run.result.stderr, "the refusal must not carry the revision"
