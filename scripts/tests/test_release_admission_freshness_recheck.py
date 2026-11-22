r"""Pin that the evidence window holds at the decision, not only at the check.

The gate asks about evidence freshness twice: once when it reads the manifest,
and again when it is about to admit on it. The second reading is not redundant.
Four clock reads and three bounded commands run in between, so a document whose
oldest moment is just inside the window can be outside it by the time the gate
decides, and admitting on the earlier reading would let the evidence outlive the
bound the gate publishes.

Both readings are served by one controlled clock, so the boundary is exercised
deterministically rather than by racing a real clock: a case names the wall
clock readings in order, and the adapter returns them in that order while
answering the monotonic reads from a rising counter. Every case also asserts
which readings the gate actually took, so a gate that stopped re-reading the
window fails here rather than passing for want of a second reading.

Example (run from the repository root)::

    PYTHONPATH=scripts uv run --no-project --python 3.14 \
        --with pytest==9.0.2 --with hypothesis==6.151.9 \
        --with 'cmd-mox==0.2.0' --with 'cuprum==0.1.0' --with 'cyclopts==4.25.3' \
        python -m pytest \
        scripts/tests/test_release_admission_freshness_recheck.py \
        -c /dev/null --rootdir=. -p no:cacheprovider
"""

import datetime as dt
import hashlib
import itertools
import json
import typing as typ

from release_admission_evidence_fixtures import (
    REQUIRED_RESULT_NAMES as REQUIRED_RESULTS,
)
from release_admission_test_support import (
    CANARY_BY_OPERATION,
    GITHUB_REPOSITORY,
    METRICS_VALIDATOR,
    MISSING,
    RELEASE_TAG,
    REVISION,
    WORKFLOW_RUN_ID,
    GateRun,
    expected_gate_labels,
    expected_operation_labels,
    install_adapted,
    operation_records,
    run_gate,
)

pytest_plugins = ("cmd_mox.pytest_plugin",)

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    import pathlib

    from cmd_mox import CmdMox
    from cmd_mox.ipc import Invocation
    from release_admission_test_doubles import Handler, Wrapper

#: The window each case configures, stated here rather than left to the gate's
#: default so that the two readings can be placed either side of it exactly.
EVIDENCE_AGE_SECONDS = 600

#: The instant the manifest names, chosen far from the present so that neither
#: reading is near a boundary the real clock could decide.
ISSUED_AT = 1_000_000_000.0

#: A reading under which the document is one second inside its window.
INSIDE_WINDOW = ISSUED_AT + EVIDENCE_AGE_SECONDS - 1

#: A reading under which the same document is one second outside it.
OUTSIDE_WINDOW = ISSUED_AT + EVIDENCE_AGE_SECONDS + 1

#: The variable naming the window, which every case sets.
MAX_AGE_VARIABLE = "NETSUKE_RELEASE_ADMISSION_MAX_EVIDENCE_AGE_SECONDS"
#: The variable naming the manifest, which every case points at its own file.
EVIDENCE_FILE_VARIABLE = "NETSUKE_RELEASE_ADMISSION_EVIDENCE_FILE"
#: The variable selecting admission mode.
ENFORCEMENT_VARIABLE = "NETSUKE_RELEASE_ADMISSION_ENFORCE"
#: The variable naming the tag a publishing run is about to release.
TAG_VARIABLE = "NETSUKE_RELEASE_ADMISSION_TAG"
#: The variable carrying the digest the producer job published. These cases
#: are about the window, so an enforcing run receives the digest of the
#: document it reads; attribution is covered by
#: ``test_release_admission_producer_trust``.
EVIDENCE_DIGEST_VARIABLE = "NETSUKE_RELEASE_ADMISSION_EVIDENCE_SHA256"


def test_evidence_that_expires_before_the_decision_is_refused(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
) -> None:
    """Refuse a manifest that was fresh when read and stale when admitted.

    Notes
    -----
    The freshness operation must succeed, or the case would be proving the
    first check rather than the recheck. The two recorded readings are asserted
    in order: the first is served to the freshness check and the second to the
    admission decision.
    """
    run, readings = _run(
        cmd_mox, tmp_path, [INSIDE_WINDOW, OUTSIDE_WINDOW], is_enforced=True
    )

    assert readings == [INSIDE_WINDOW, OUTSIDE_WINDOW], (
        "the gate must read the wall clock once at the check and once at the decision"
    )
    assert run.result.returncode == 1, (
        "a document that expired before admission must fail closed; "
        f"{run.result.stderr}"
    )
    METRICS_VALIDATOR.validate_metrics(run.metrics)
    METRICS_VALIDATOR.validate_traces(run.traces)
    assert operation_records(run.metrics, "check_scan_freshness")[-1]["labels"] == (
        expected_operation_labels(
            CANARY_BY_OPERATION["check_scan_freshness"],
            "check_scan_freshness",
            "success",
            "none",
        )
    ), "the case is only about the recheck if the freshness check itself passed"
    assert operation_records(run.metrics, "verify_evidence")[-1]["labels"] == (
        expected_operation_labels(
            CANARY_BY_OPERATION["verify_evidence"],
            "verify_evidence",
            "failure",
            "stale_evidence",
        )
    ), "expiry at the decision must be reported against the admitting operation"
    assert run.metrics[-1]["labels"] == expected_gate_labels(
        "failure", "stale_evidence"
    ), "the gate must publish the expiry it refused on"
    assert run.outputs["gate-outcome"] == "failure", (
        "an enforced expiry must reach the workflow outcome"
    )
    assert run.outputs["gate-error-category"] == "stale_evidence", (
        "an enforced expiry must retain the stale-evidence category"
    )


def test_evidence_inside_its_window_at_both_readings_is_admitted(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
) -> None:
    """Admit the same document when the second reading is still inside.

    Notes
    -----
    This is the negative control for the refusal above: the document, the
    window, and the first reading are identical, so only the second reading
    separates an admission from a refusal.
    """
    run, readings = _run(
        cmd_mox, tmp_path, [INSIDE_WINDOW, INSIDE_WINDOW], is_enforced=True
    )

    assert readings == [INSIDE_WINDOW, INSIDE_WINDOW], (
        "the admitting path must take both readings too"
    )
    assert run.result.returncode == 0, (
        f"a document inside its window at both readings must be admitted; "
        f"{run.result.stderr}"
    )
    METRICS_VALIDATOR.validate_metrics(run.metrics)
    METRICS_VALIDATOR.validate_traces(run.traces)
    assert operation_records(run.metrics, "verify_evidence")[-1]["labels"] == (
        expected_operation_labels(
            CANARY_BY_OPERATION["verify_evidence"],
            "verify_evidence",
            "success",
            "none",
        )
    ), "the recheck must not refuse a document that is still fresh"
    assert run.metrics[-1]["labels"] == expected_gate_labels("success", "none"), (
        "an admitted candidate must leave a successful gate record"
    )
    assert run.outputs["gate-outcome"] == "success", (
        "an admitted candidate must publish a successful outcome"
    )


def test_expiry_at_the_decision_is_observed_in_observation_mode(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
) -> None:
    """Report the expiry without failing the workflow when observing.

    Notes
    -----
    Observation mode changes the exit status and nothing else, so the expiry
    the recheck finds must still be recorded and published: a rehearsal that
    reported success would hide exactly the condition the enforcement will act
    on.
    """
    run, readings = _run(
        cmd_mox, tmp_path, [INSIDE_WINDOW, OUTSIDE_WINDOW], is_enforced=False
    )

    assert readings == [INSIDE_WINDOW, OUTSIDE_WINDOW], (
        "observation must reach the decision and re-read the window"
    )
    assert run.result.returncode == 0, (
        f"observation must retain the expiry's diagnostics; {run.result.stderr}"
    )
    METRICS_VALIDATOR.validate_metrics(run.metrics)
    METRICS_VALIDATOR.validate_traces(run.traces)
    assert run.outputs["gate-outcome"] == "failure", (
        "observation must still publish the failed gate outcome"
    )
    assert run.outputs["gate-error-category"] == "stale_evidence", (
        "observation must retain the stale-evidence category"
    )
    assert run.outputs["enforcement"] == "false", (
        "the gate must publish the mode it actually ran in"
    )


def _run(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
    readings: cabc.Sequence[float],
    *,
    is_enforced: bool,
) -> tuple[GateRun, list[float]]:
    """Run the production gate with the wall clock answering *readings* in order.

    Parameters
    ----------
    cmd_mox
        Active controller whose shims stand in for every adapter.
    tmp_path
        Isolated output directory for the gate.
    readings
        The wall-clock readings to serve, in the order the gate asks for them.
    is_enforced
        Whether the run enforces admission, which also decides whether it names
        a tag the way a publishing run does.

    Returns
    -------
    tuple
        The gate run and the wall-clock readings the gate actually took.
    """
    adapted = install_adapted(cmd_mox)
    wrapper, served = _scripted_clock(readings)
    adapted.clock.wraps(wrapper)
    evidence = tmp_path / "expiring-evidence.json"
    document = _manifest()
    evidence.write_bytes(document)
    environment = {
        **adapted.variables(cmd_mox),
        ENFORCEMENT_VARIABLE: str(is_enforced).lower(),
        MAX_AGE_VARIABLE: str(EVIDENCE_AGE_SECONDS),
        EVIDENCE_FILE_VARIABLE: str(evidence),
    }
    if is_enforced:
        environment[TAG_VARIABLE] = RELEASE_TAG
        environment[EVIDENCE_DIGEST_VARIABLE] = hashlib.sha256(document).hexdigest()
    return (
        run_gate(
            cmd_mox,
            tmp_path,
            evidence_state=MISSING,
            extra_environment=environment,
        ),
        served,
    )


def _scripted_clock(
    readings: cabc.Sequence[float],
) -> tuple[Wrapper, list[float]]:
    """Return a clock double serving *readings*, and the record of what it served.

    The wall clock answers the named readings in order; the monotonic clock
    answers a rising counter, so every duration is a usable positive number and
    the duration contract does not decide a case about freshness.

    Returns
    -------
    tuple
        The wrapper to install over the clock double, and the list it appends
        each wall-clock reading to.
    """
    remaining = list(readings)
    served: list[float] = []
    monotonic = itertools.count(1)

    def wrapper(invocation: Invocation, _handler: Handler) -> tuple[str, str, int]:
        """Answer one clock read from the reading this case is scripting."""
        if "time.time()" in " ".join(invocation.args):
            # The last reading is repeated rather than exhausted, so a gate that
            # reads the clock more often than a case names is visible in
            # ``served`` instead of failing inside the double.
            reading = remaining.pop(0) if len(remaining) > 1 else remaining[0]
            served.append(reading)
            return f"{reading}\n", "", 0
        return f"{next(monotonic)}\n", "", 0

    return wrapper, served


def _manifest() -> bytes:
    """Return a document that admits the candidate and is stamped at its issue time.

    The schema is written out here rather than built from the gate's own parser,
    for the reason the fixtures module gives: a document built from the verifier
    agrees with a broken verifier.

    Returns
    -------
    bytes
        The encoded manifest document.
    """
    moment = (
        dt.datetime.fromtimestamp(ISSUED_AT, dt.UTC).isoformat().replace("+00:00", "Z")
    )
    document = {
        "schema_version": 1,
        "repository": GITHUB_REPOSITORY,
        "commit": REVISION,
        "tag": RELEASE_TAG,
        "workflow_run_id": WORKFLOW_RUN_ID,
        "issued_at": moment,
        "results": {
            name: {"outcome": "success", "observed_at": moment}
            for name in REQUIRED_RESULTS
        },
    }
    return json.dumps(document).encode("utf-8")
