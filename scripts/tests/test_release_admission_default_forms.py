r"""Pin the two Bash default forms the gate resolves its inputs with.

The shell read its inputs with two different expansions, and neither is
interchangeable with the other:

- ``${VAR:-default}``, the *colon* form, substitutes when the variable is unset
  **or empty**. Every adapter, artefact path, and evidence value was read that
  way, so an exported empty path became the default path.
- ``${VAR-default}``, the *no-colon* form, substitutes only when the variable is
  **unset**. The shell wrote it for ``NETSUKE_RELEASE_ADMISSION_ENFORCE`` alone,
  so an exported empty mode stayed empty and was refused by the gate's own mode
  check rather than quietly becoming observation mode.

Both forms are exercised against the real gate, because the difference decides
whether a refused configuration admits a release. The cases live apart from
:mod:`test_release_admission_environment_contract` because they are about the
*values* an input resolves to rather than the *order* the gate refuses them in.
The non-canonical path spelling is one of those values, so it is asserted here
alongside the defaults rather than in the refusal-order module.

Example (run from the repository root)::

    PYTHONPATH=scripts uv run --no-project --python 3.14 \
        --with pytest==9.0.2 --with 'cmd-mox==0.2.0' --with 'cuprum==0.1.0' \
        --with 'cyclopts==4.25.3' \
        python -m pytest scripts/tests/test_release_admission_default_forms.py \
        -c /dev/null --rootdir=. -p no:cacheprovider
"""

import dataclasses
import typing as typ

import pytest
from release_admission_test_support import (
    METRICS_VALIDATOR,
    install_adapted,
    install_boundaries,
    run_gate,
)

if typ.TYPE_CHECKING:
    import pathlib

    from cmd_mox import CmdMox

pytest_plugins = ("cmd_mox.pytest_plugin",)

EVIDENCE_VARIABLE = "NETSUKE_RELEASE_ADMISSION_EVIDENCE_STATE"
METRICS_FILE_VARIABLE = "NETSUKE_RELEASE_ADMISSION_METRICS_FILE"
TRACE_FILE_VARIABLE = "NETSUKE_RELEASE_ADMISSION_TRACE_FILE"
ENFORCE_VARIABLE = "NETSUKE_RELEASE_ADMISSION_ENFORCE"

#: The artefact names the gate derives when no path variable is supplied.
#: They are not the names the harness configures: the harness names its own
#: files explicitly, and the ``netsuke-`` prefix is what distinguishes the
#: default from the configured pair.
DEFAULT_METRICS_NAME = "netsuke-release-admission-metrics.jsonl"
DEFAULT_TRACE_NAME = "netsuke-release-admission-traces.jsonl"


@dataclasses.dataclass(frozen=True, slots=True)
class EnforcementCase:
    """Describe one state of the gate's no-colon input and what it must produce.

    The two states travel with the outcome each must reach, because the pair
    only means something together: the *value* supplied and the exit status and
    category it produces are one statement about the no-colon form, and a case
    that carried only the value would have nothing to assert it against.

    Attributes
    ----------
    state
        The value the enforcement variable is given, or ``None`` to remove it.
    status
        The exit status the state must produce.
    category
        The gate error category the state must publish.
    """

    state: str | None
    status: int
    category: str


#: The two states of the one no-colon input, and the category each publishes.
#: ``None`` removes the variable; the empty string exports it empty.
ENFORCEMENT_STATES = [
    pytest.param(
        EnforcementCase(None, 0, "missing_evidence"),
        id="unset-takes-the-default",
    ),
    pytest.param(
        EnforcementCase("", 1, "unknown"),
        id="empty-is-kept-and-refused",
    ),
]


@pytest.mark.parametrize("case", ENFORCEMENT_STATES)
def test_enforcement_default_distinguishes_unset_from_empty(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
    case: EnforcementCase,
) -> None:
    """Verify only an *unset* enforcement variable takes the documented default.

    Parameters
    ----------
    cmd_mox
        Active controller whose shims stand in for every adapter.
    tmp_path
        Isolated output directory for the gate.
    case
        The state under test and the status and category it must produce.

    Notes
    -----
    This is the gate's no-colon input. An unset mode takes the documented
    ``false`` default, observes the admission, and exits ``0``; an empty mode is
    kept, refused by the gate's own check, and exits ``1`` with the ``unknown``
    category its refusal arm records.

    The pair is what makes the helper load-bearing rather than decorative:
    substituting the default for an exported empty value -- which is what the
    colon form does, and what the two cases below confirm for every other input
    -- would turn a refused configuration into a successful observation. A case
    that only supplied valid modes could not tell the two implementations apart.
    """
    install_boundaries(cmd_mox)
    run = run_gate(cmd_mox, tmp_path, extra_environment={ENFORCE_VARIABLE: case.state})

    assert run.result.returncode == case.status, (
        "only an unset enforcement variable may take the observation default"
    )
    assert run.outputs["gate-error-category"] == case.category, (
        "the state must publish the category its refusal or observation produces"
    )


def test_optional_values_are_read_with_the_colon_default_form(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
) -> None:
    """Verify an *empty* optional value substitutes its default.

    Notes
    -----
    Every optional input other than the enforcement mode was read with the colon
    form, so an empty value substituted the default exactly as an unset one did.
    Two of them are supplied empty here: the artefact paths, whose default is
    observable in the workflow outputs the gate publishes, and the evidence
    state, whose default is observable in the category it reports.

    The evidence state's default is ``missing``, so an empty value must reach
    the freshness check as a missing-evidence refusal rather than as the
    ``unknown`` category an unrecognised state produces. Those two categories
    are the whole difference between "the shell substituted" and "the shell
    passed the empty string through", and asserting the category rather than
    only the exit status is what pins it.
    """
    install_boundaries(cmd_mox)
    run = run_gate(
        cmd_mox,
        tmp_path,
        extra_environment={
            METRICS_FILE_VARIABLE: "",
            TRACE_FILE_VARIABLE: "",
            EVIDENCE_VARIABLE: "",
        },
    )

    assert run.outputs["metrics-file"] == str(tmp_path / DEFAULT_METRICS_NAME), (
        "an empty metrics path must take the default, as the colon form does"
    )
    assert run.outputs["trace-file"] == str(tmp_path / DEFAULT_TRACE_NAME), (
        "an empty trace path must take the default"
    )
    METRICS_VALIDATOR.validate_metrics(run.metrics)
    METRICS_VALIDATOR.validate_traces(run.traces)
    assert run.outputs["gate-error-category"] == "missing_evidence", (
        "an empty evidence state must take the documented missing default"
    )


def test_gate_publishes_the_operators_own_path_spelling(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
) -> None:
    """Verify a non-canonical configured path reaches both observable boundaries.

    Notes
    -----
    The shell passed the operator's string to its sink adapter and published it
    verbatim in the ``metrics-file`` and ``trace-file`` workflow outputs; only
    ``mkdir`` and the redirection saw a normalised form. Rendering a
    :class:`~pathlib.Path` back to text would rewrite ``//`` and ``./`` at both
    boundaries, so the configured spelling is retained.

    The case is a colon-form case as well: the two paths are supplied *set*, so
    a gate that defaulted them would publish the temporary directory instead of
    the operator's own spelling, and the assertions would fail rather than pass
    by coincidence.
    """
    install_boundaries(cmd_mox)
    spelled = f"{tmp_path}//observations/./metrics.jsonl"
    traces_spelled = f"{tmp_path}/traces/../traces.jsonl"
    run = run_gate(
        cmd_mox,
        tmp_path,
        extra_environment={
            METRICS_FILE_VARIABLE: spelled,
            TRACE_FILE_VARIABLE: traces_spelled,
        },
    )

    assert run.result.returncode == 0, run.result.stderr
    assert run.outputs["metrics-file"] == spelled, (
        "a workflow output must carry the configured spelling, not a normalised one"
    )
    assert run.outputs["trace-file"] == traces_spelled, (
        "both published paths must keep the operator's own spelling"
    )


def test_artefact_paths_are_created_at_the_spelling_they_were_given(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
) -> None:
    """Verify the configured spelling is also the file the gate wrote.

    Notes
    -----
    Retaining the configured string at the published boundaries must not change
    *which file* the gate writes: the shell's redirection resolved its operand
    through the filesystem, so ``./observations//metrics.jsonl`` was the same
    file as ``observations/metrics.jsonl``. A port that kept the string but
    resolved nothing would publish a path it never wrote, which is why this is
    asserted against the filesystem rather than against the outputs alone.
    """
    install_boundaries(cmd_mox)
    spelled = f"{tmp_path}//observations/./metrics.jsonl"
    resolved = tmp_path / "observations" / "metrics.jsonl"
    run = run_gate(
        cmd_mox,
        tmp_path,
        extra_environment={METRICS_FILE_VARIABLE: spelled},
    )

    assert resolved.exists(), (
        "the gate must write the file the operator's own spelling resolves to"
    )
    assert run.outputs["metrics-file"] == spelled, (
        "the published path must still be the spelling the operator configured"
    )
    METRICS_VALIDATOR.validate_metrics(run.metrics)


def test_external_metrics_sink_receives_the_configured_spelling(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
) -> None:
    r"""Verify the sink adapter is named with the operator's own path spelling.

    Notes
    -----
    The shell fed each record to its adapter with
    ``printf '%s\n' "$record" | "$adapter" "$file"``, where ``file`` was the
    configured string rather than a resolved path, so
    ``./observations//metrics.jsonl`` reached the adapter under exactly that
    spelling. Retaining the string at the published boundaries is a different
    statement from retaining it here: this is the one boundary where the
    spelling is an argument to a program the operator supplied, and rendering
    the path back to text would rewrite ``//`` and ``./`` before the adapter
    ever saw them.

    An external sink has to be configured for that argument to exist at all.
    With no sink the gate appends in process, so a case that asserted only the
    published outputs would leave this boundary unobserved; the sink double is
    installed in the metrics position and records the argv it was handed.
    """
    adapted = install_adapted(cmd_mox)
    spelled = f"{tmp_path}//observations/./metrics.jsonl"
    normalised = f"{tmp_path}/observations/metrics.jsonl"
    run = run_gate(
        cmd_mox,
        tmp_path,
        extra_environment={
            **adapted.variables(cmd_mox),
            METRICS_FILE_VARIABLE: spelled,
        },
    )

    assert run.result.returncode == 0, run.result.stderr
    assert adapted.metrics.calls, (
        "an external sink must have been invoked, or the argv assertion is vacuous"
    )
    assert set(map(tuple, adapted.metrics.calls)) == {(spelled,)}, (
        "every sink invocation must name the target with the configured spelling"
    )
    handed = {argument for call in adapted.metrics.calls for argument in call}
    assert normalised not in handed, (
        "the adapter must not be handed a normalised spelling of the target"
    )
