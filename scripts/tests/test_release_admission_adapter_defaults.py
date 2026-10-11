r"""Pin the colon-form defaults that are only visible in what the gate runs.

Four optional inputs were read with Bash's ``${VAR:-default}`` form -- the
colon form, which substitutes for an *unset or empty* value -- but their
defaults are not published anywhere an operator can read them back. The three
adapter names decide which *program* the gate executes, and the operation
timeout decides the bound it executes it under. An empty value that reached the
program name would fail a launch; one that reached the timeout would fail the
gate's own configuration check. Both are therefore observed by running the gate
and reading what it did, not by reading a record.

The three adapter defaults are the shell's own: ``gh``, ``git``, and
``python3``, measured from ``require-release-admission-canaries.sh`` at base
``5ee29b3e``. The timeout defaults to ``30``, and a value is accepted only as a
decimal integer in ``1``..``300``; the frozen measurement is recorded in
:mod:`release_admission_test_support`.

Each of the four is exercised in *both* states the colon form substitutes for:
an exported empty value and a variable that is not set at all. Both are stated
rather than assumed, because the harness builds its child environment as
``{**os.environ, ...}`` -- an *omitted* override therefore leaves whatever the
host happens to export in place, and a case that is silently satisfied by the
host's own value would stop testing the default without failing. The unset arm
is the one only the harness can express: it passes ``None``, which removes the
name from the child environment instead of setting it.

This module is apart from :mod:`test_release_admission_default_forms` for two
reasons. The first is scope: that module pins defaults whose consequences are
*published* -- an artefact path appears in a workflow output, an evidence state
in a category -- while these four are read back from the run's own execution.
The second is the module ceiling: that module is the natural home for them, and
adding four documented cases would push it past the 400-line limit the
repository enforces on Python modules.

Example (run from the repository root)::

    PYTHONPATH=scripts uv run --no-project --python 3.14 \
        --with pytest==9.0.2 --with 'cmd-mox==0.2.0' --with 'cuprum==0.1.0' \
        --with 'cyclopts==4.25.3' \
        python -m pytest \
        scripts/tests/test_release_admission_adapter_defaults.py \
        -c /dev/null --rootdir=. -p no:cacheprovider
"""

import typing as typ

import pytest
from release_admission_test_support import (
    install_adapted,
    install_boundaries,
    run_gate,
)

if typ.TYPE_CHECKING:
    import pathlib

    from cmd_mox import CmdMox

pytest_plugins = ("cmd_mox.pytest_plugin",)

GH_ADAPTER_VARIABLE = "NETSUKE_RELEASE_ADMISSION_GH_ADAPTER"
GIT_ADAPTER_VARIABLE = "NETSUKE_RELEASE_ADMISSION_GIT_ADAPTER"
CLOCK_ADAPTER_VARIABLE = "NETSUKE_RELEASE_ADMISSION_CLOCK_ADAPTER"
TIMEOUT_VARIABLE = "NETSUKE_RELEASE_ADMISSION_OPERATION_TIMEOUT_SECONDS"

#: The two command adapters whose default names are registered as doubles by
#: :func:`install_boundaries`, each with the name the shell defaulted to. The
#: clock is absent deliberately: its default is ``python3``, which that module
#: refuses to shim because a shim named ``python3`` would resolve its own
#: interpreter back to itself, so it gets a case of its own below.
COMMAND_ADAPTERS = [
    pytest.param(GH_ADAPTER_VARIABLE, id="gh-adapter-empty"),
    pytest.param(GIT_ADAPTER_VARIABLE, id="git-adapter-empty"),
]

#: The same two adapters, *not set* rather than exported empty. ``None`` is the
#: harness's own vocabulary for "remove this name from the child environment",
#: which is the only way to state the arm the colon form's unset branch serves.
UNSET_COMMAND_ADAPTERS = [
    pytest.param(GH_ADAPTER_VARIABLE, id="gh-adapter-unset"),
    pytest.param(GIT_ADAPTER_VARIABLE, id="git-adapter-unset"),
]

#: The category a well-formed default run publishes when the fixture state is
#: the harness's default ``missing``. A clock read that failed would replace it
#: with ``unknown``, which is what makes it the observable for the clock case.
DEFAULT_RUN_CATEGORY = "missing_evidence"


@pytest.mark.parametrize("variable", COMMAND_ADAPTERS)
def test_empty_command_adapter_falls_back_to_its_default_name(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
    variable: str,
) -> None:
    """Verify an exported *empty* adapter name runs the documented default.

    Parameters
    ----------
    cmd_mox
        Active controller whose shims stand in for every adapter.
    tmp_path
        Isolated output directory for the gate.
    variable
        The command-adapter variable that is exported empty.

    Notes
    -----
    Both sets of doubles are registered: the six adapted ones and the three
    default-named ones. That is what makes the assertion a *discrimination*
    rather than a presence check. If the colon form substituted the default,
    the call lands on the double registered under the default name and the
    adapted double stays empty; if the empty value were passed through as a
    program name, no double would be reached at all and the launch would fail.

    Asserting only that the run succeeded would pass for a gate that resolved
    *something* else entirely, so the pair of assertions -- default reached,
    adapted untouched -- is what pins which name was used.
    """
    _assert_command_adapter_default(cmd_mox, tmp_path, variable, value="")


@pytest.mark.parametrize("variable", UNSET_COMMAND_ADAPTERS)
def test_unset_command_adapter_falls_back_to_its_default_name(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
    variable: str,
) -> None:
    """Verify an adapter the child never sees runs the documented default.

    Parameters
    ----------
    cmd_mox
        Active controller whose shims stand in for every adapter.
    tmp_path
        Isolated output directory for the gate.
    variable
        The command-adapter variable that is removed from the child.

    Notes
    -----
    The empty case's twin, and not a duplicate of it: the colon form's two
    substitution arms are ``unset`` and ``empty``, and only an absent name
    exercises the first. The child environment is built from ``os.environ``, so
    an override the case simply *omits* would leave a host value in place and
    the case would silently test something else; passing ``None`` removes the
    name instead, which is what makes the state the case names the state the
    gate sees.
    """
    _assert_command_adapter_default(cmd_mox, tmp_path, variable, value=None)


def _assert_command_adapter_default(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
    variable: str,
    value: str | None,
) -> None:
    """Run one command-adapter case and assert which double was reached.

    ``value`` is the override the harness receives: an empty string for the
    exported-empty arm, ``None`` for the unset one. Both arms assert the same
    pair, because the gate cannot tell them apart -- that is what the colon
    form means -- and the assertion is what would catch a port that treated
    them differently.
    """
    boundaries = install_boundaries(cmd_mox)
    adapted = install_adapted(cmd_mox)
    run = run_gate(cmd_mox, tmp_path, extra_environment={variable: value})

    assert run.result.returncode == 0, run.result.stderr
    reached, untouched = (
        (boundaries.gh, adapted.gh)
        if variable == GH_ADAPTER_VARIABLE
        else (boundaries.git, adapted.git)
    )
    assert reached.calls, (
        "an unset or empty adapter name must fall back to the default name on PATH"
    )
    assert untouched.calls == [], (
        "an unset or empty adapter name must not reach the adapter shim it displaced"
    )


def test_empty_clock_adapter_falls_back_to_the_default_interpreter(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
) -> None:
    """Verify an empty clock adapter still reads a monotonic clock.

    Notes
    -----
    The clock's default is ``python3``, and that name is deliberately never
    shimmed, so this case cannot assert which double was reached. Its
    observable is the consequence instead: an empty value that *reached* the
    program name would fail the launch, and a failed clock read forces the
    operation to ``failure``/``unknown`` whatever it did. The run therefore
    ends in the ordinary missing-evidence category only if the default
    interpreter ran.

    The adapted clock double is still asserted empty, which rules out the
    clock having been resolved to the shim the operator did not name.

    The two *command* adapters are redirected at their doubles through the
    adapted variables, because the case is about the clock alone: leaving them
    to resolve on ``PATH`` would reach the host's real ``gh`` and ``git`` and
    the category under assertion would report that failure instead. The clock
    variable is overridden *after* the merge, so the empty value is the one the
    gate reads.
    """
    _assert_clock_adapter_default(cmd_mox, tmp_path, value="")


def test_unset_clock_adapter_falls_back_to_the_default_interpreter(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
) -> None:
    """Verify a clock adapter the child never sees still reads a clock.

    Notes
    -----
    The unset twin of the empty case, and the same observable: a value of
    ``None`` removes the name from the child, which the harness documents as
    the way a case expresses *unset*. The reading's consequence is checked
    rather than the program name, for the reason the empty case gives -- the
    default is ``python3``, which is never shimmed.

    Removing the name matters here for the same reason as in the command
    adapters: the child environment is built from ``os.environ``, so a host
    that exported this variable would decide which interpreter ran unless the
    case removed it.
    """
    _assert_clock_adapter_default(cmd_mox, tmp_path, value=None)


def _assert_clock_adapter_default(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
    value: str | None,
) -> None:
    """Run one clock-adapter case and assert the default interpreter ran.

    Both arms share this body because the gate cannot distinguish them, which
    is the whole content of the colon form.
    """
    adapted = install_adapted(cmd_mox)
    run = run_gate(
        cmd_mox,
        tmp_path,
        extra_environment={
            **adapted.variables(cmd_mox),
            CLOCK_ADAPTER_VARIABLE: value,
        },
    )

    assert run.result.returncode == 0, run.result.stderr
    assert run.outputs["gate-error-category"] == DEFAULT_RUN_CATEGORY, (
        "an unset or empty clock adapter must read the real clock, not fail a launch"
    )
    assert adapted.clock.calls == [], (
        "an unset or empty clock adapter must not reach the adapter shim it displaced"
    )


def test_empty_timeout_takes_the_documented_default(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
) -> None:
    """Verify an exported empty timeout is accepted, not passed through.

    Notes
    -----
    The resolved bound reaches no record, so the default is observable through
    its *acceptance* rather than through its value: ``is_operation_timeout``
    accepts only a decimal integer in ``1``..``300``, and an empty string is
    outside that vocabulary. A port that passed the empty value through would
    refuse the whole configuration -- ``outcome=failure`` with
    ``error_category=unknown`` and exit ``1``, before any operation ran -- while
    the colon form admits it as the measured default of ``30`` and the gate
    proceeds normally.

    The case does not distinguish ``30`` from another accepted bound, and does
    not claim to: what it pins is that an empty value substituted the default
    rather than being forwarded. The measured constant itself is recorded in
    :mod:`release_admission_test_support`.

    Every adapter is redirected at a double, so the run's category reports the
    bound's *acceptance* rather than a fault in one of the operations; the
    timeout is overridden after the merge so the empty value is what the gate
    reads.
    """
    _assert_timeout_default(cmd_mox, tmp_path, value="")


def test_unset_timeout_takes_the_documented_default(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
) -> None:
    """Verify a timeout the child never sees is accepted as the default.

    Notes
    -----
    The unset twin of the empty case. Its observable is identical -- the run's
    category reports the bound's acceptance, not its value -- because the gate
    substitutes the same default in both states.

    Stating it separately is not redundant bookkeeping. The harness environment
    is built from ``os.environ``, so a case that merely *omitted* the override
    would inherit a host value: on a machine that exported this variable the
    case would be exercising that value while claiming to exercise the default,
    and it would pass. ``None`` removes the name, so the state the case names is
    the state the gate sees.
    """
    _assert_timeout_default(cmd_mox, tmp_path, value=None)


def _assert_timeout_default(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
    value: str | None,
) -> None:
    """Run one timeout case and assert the default bound was accepted."""
    adapted = install_adapted(cmd_mox)
    run = run_gate(
        cmd_mox,
        tmp_path,
        extra_environment={
            **adapted.variables(cmd_mox),
            TIMEOUT_VARIABLE: value,
        },
    )

    assert run.result.returncode == 0, run.result.stderr
    assert run.outputs["gate-error-category"] == DEFAULT_RUN_CATEGORY, (
        "an unset or empty timeout must take the default rather than be refused"
    )
