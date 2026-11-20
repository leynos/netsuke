r"""Pin the interim block that refuses evidence no producer attests.

The admission gate cannot authenticate the producer of the manifest it reads.
``actions/download-artifact`` resolves the current run by default, and a
reusable-workflow caller defines jobs in that same run, so a caller that uploads
a matching document under ``release-admission-evidence`` reaches the verifier
indistinguishably from a producer. Identity checks compare the document with the
candidate; they do not establish who wrote it.

The workflow therefore ships ``NETSUKE_RELEASE_ADMISSION_PRODUCER_TRUSTED`` as
``false``, and an enforcing run refuses every manifest -- including a fresh,
matching one -- with ``untrusted_evidence`` until #598 supplies a producer that
attests its upload. These cases run the production gate against a manifest that
would otherwise be admitted, so the refusal is attributable to the absent
attribution and nothing else:

- every spelling other than ``true`` refuses, including unset and empty, so a
  configuration the gate half-reads cannot admit;
- ``true`` admits the same document, which is the positive control that makes
  the block the deciding input rather than an incidental defect;
- observation mode still reports what it finds, because the block governs
  publication and must not make the read-only lane unreachable.

Example (run from the repository root)::

    PYTHONPATH=scripts uv run --no-project --python 3.14 \
        --with pytest==9.0.2 --with hypothesis==6.151.9 \
        --with 'cmd-mox==0.2.0' --with 'cuprum==0.1.0' --with 'cyclopts==4.25.3' \
        python -m pytest scripts/tests/test_release_admission_producer_trust.py \
        -c /dev/null --rootdir=. -p no:cacheprovider
"""

import typing as typ

import pytest
from release_admission_test_support import (
    CANARY_BY_OPERATION,
    METRICS_VALIDATOR,
    RELEASE_TAG,
    GateRun,
    expected_gate_labels,
    expected_operation_labels,
    install_adapted,
    operation_records,
    run_gate,
)

pytest_plugins = ("cmd_mox.pytest_plugin",)

if typ.TYPE_CHECKING:
    import pathlib

    from cmd_mox import CmdMox

#: The variable selecting admission mode; every case states it explicitly so
#: the block is exercised against a known mode rather than the harness default.
ENFORCEMENT_VARIABLE = "NETSUKE_RELEASE_ADMISSION_ENFORCE"
#: The variable reporting whether the run can attribute the evidence to a
#: trusted producer. The workflow ships it as ``false`` until #598 lands.
PRODUCER_TRUST_VARIABLE = "NETSUKE_RELEASE_ADMISSION_PRODUCER_TRUSTED"
#: The variable naming the tag an enforcing run is about to release.
TAG_VARIABLE = "NETSUKE_RELEASE_ADMISSION_TAG"
#: A manifest state the verifier admits on its own terms: fresh, matching, and
#: reporting both required canaries as successful.
ADMITTABLE_EVIDENCE = "fresh"


def _run(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
    *,
    mode: str,
    producer_trusted: str | None,
) -> GateRun:
    """Run the gate with an admittable manifest and the named producer state.

    Parameters
    ----------
    cmd_mox
        Active controller whose shims stand in for every adapter.
    tmp_path
        Isolated output directory for the gate.
    mode
        The enforcement mode the child runs under, spelled as the workflow
        spells it.
    producer_trusted
        The producer-attribution value to set, or ``None`` to leave the
        variable unset, which is the state a caller cannot alter.

    Returns
    -------
    GateRun
        The completed process and the records decoded from the files it wrote.
    """
    adapted = install_adapted(cmd_mox)
    environment = {
        **adapted.variables(cmd_mox),
        ENFORCEMENT_VARIABLE: mode,
        TAG_VARIABLE: RELEASE_TAG,
    }
    if producer_trusted is not None:
        environment[PRODUCER_TRUST_VARIABLE] = producer_trusted
    return run_gate(
        cmd_mox,
        tmp_path,
        evidence_state=ADMITTABLE_EVIDENCE,
        extra_environment=environment,
    )


def _assert_untrusted_refusal(run: GateRun) -> None:
    """Require the run to refuse the manifest for its absent attribution."""
    assert run.result.returncode == 1, (
        f"an enforcing run without producer attribution must fail closed; "
        f"{run.result.stderr}"
    )
    METRICS_VALIDATOR.validate_metrics(run.metrics)
    METRICS_VALIDATOR.validate_traces(run.traces)
    assert operation_records(run.metrics, "verify_evidence")[-1]["labels"] == (
        expected_operation_labels(
            CANARY_BY_OPERATION["verify_evidence"],
            "verify_evidence",
            "failure",
            "untrusted_evidence",
        )
    ), "the refusal must be recorded against the evidence decision that made it"
    assert run.metrics[-1]["labels"] == expected_gate_labels(
        "failure", "untrusted_evidence"
    ), "the gate record must carry the refusal a publisher reads"
    assert run.outputs["gate-outcome"] == "failure", (
        "a write job reading this run must not see a successful gate"
    )
    assert run.outputs["gate-error-category"] == "untrusted_evidence", (
        "the published category must name the absent attribution, not the "
        "document's contents"
    )
    assert run.outputs["enforcement"] == "true", (
        "the refusal must be published as an enforced one"
    )


@pytest.mark.parametrize(
    "producer_trusted",
    [
        pytest.param(None, id="unset"),
        pytest.param("", id="empty"),
        pytest.param("false", id="false"),
        pytest.param("True", id="capitalised"),
        pytest.param("yes", id="other-word"),
    ],
)
def test_matching_evidence_without_attribution_is_refused(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
    producer_trusted: str | None,
) -> None:
    """Refuse a manifest the verifier would otherwise admit.

    The document is fresh, names this run's repository, commit, tag and run
    identifier, and reports both required canaries as successful, so every
    content check passes. Only the attribution is absent, which is exactly the
    same-run supplier the download cannot distinguish from a producer.
    """
    run = _run(cmd_mox, tmp_path, mode="true", producer_trusted=producer_trusted)

    _assert_untrusted_refusal(run)


def test_attributed_evidence_is_admitted(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
) -> None:
    """Admit the same document once the run reports a trusted producer.

    This is the positive control for the refusal above: the document, the mode
    and the tag are identical, so only the attribution separates them. Without
    it, a gate that refused everything would satisfy the negative cases.
    """
    run = _run(cmd_mox, tmp_path, mode="true", producer_trusted="true")

    assert run.result.returncode == 0, (
        f"an attributed manifest must be admitted; {run.result.stderr}"
    )
    METRICS_VALIDATOR.validate_metrics(run.metrics)
    METRICS_VALIDATOR.validate_traces(run.traces)
    assert run.metrics[-1]["labels"] == expected_gate_labels("success", "none"), (
        "an admitted candidate must leave a successful gate record"
    )
    assert run.outputs["gate-outcome"] == "success", (
        "an admitted candidate must publish a successful outcome"
    )


def test_observation_mode_still_reports_the_manifest(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
) -> None:
    """Keep observing on a rehearsal, where nothing is published either way.

    The block governs publication rather than reporting, so a dry run must
    still reach a verdict on the document. Failing the rehearsal here would
    make the read-only lane report an admission problem it does not have.
    """
    run = _run(cmd_mox, tmp_path, mode="false", producer_trusted=None)

    assert run.result.returncode == 0, (
        f"a rehearsal must observe rather than fail; {run.result.stderr}"
    )
    METRICS_VALIDATOR.validate_metrics(run.metrics)
    METRICS_VALIDATOR.validate_traces(run.traces)
    assert run.metrics[-1]["labels"] == expected_gate_labels("success", "none"), (
        "a rehearsal of an admittable document must record what it observed"
    )
    assert run.outputs["enforcement"] == "false", (
        "the rehearsal must publish the mode it actually ran in"
    )
