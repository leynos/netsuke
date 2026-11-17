"""Execute the consumer-smoke action over a published release.

Each case runs the action's real shell bodies through the checked-in CLI, over
bytes a stubbed host serves. The harness that stubs ``uv`` and ``curl`` and
reads the manifest lives in ``consumer_smoke_action_harness.py``; this module
supplies the candidates and asserts what the run reported.

Read together, the cases hold four properties to account: a published
candidate verifies and reports every phase successful; a download that never
arrived, arrived empty, or arrived with other bytes is refused in the phase
that needed it; an archive that cannot be extracted safely, and a binary that
reports another version, are refused before the run claims a verdict; and a
work directory that already holds an earlier attempt's files is refused at
validation while the report step still publishes what it observed.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

from composite_action_execution import interpreter_baseline
from consumer_smoke_action_harness import (
    ARCHIVE_NAME,
    CRATE_NAME,
    SIDECAR_NAME,
    Smoke,
    execute,
)

if typ.TYPE_CHECKING:
    import pathlib

    from consumer_smoke_action_harness import ActionRun

PHASES = (
    "crate_download",
    "crate_verification",
    "archive_download",
    "archive_verification",
    "executable_probe",
)
#: The exit status the CLI documents for a download that is missing, empty, or
#: not the published bytes.
EXIT_DOWNLOAD_REJECTED = 2
#: The exit status for an archive that is corrupt, unsafe, or unwritable.
EXIT_ARCHIVE_REJECTED = 3
#: The exit status for a binary that did not report the candidate version.
EXIT_PROBE_REJECTED = 4


def _refusal_status(run: ActionRun) -> int:
    """Return the verification step's exit status.

    The verification step is the last one before the report step, which the
    manifest runs even after a refusal, so a run that reached verification is
    one step longer than its verdict path suggests.

    Returns
    -------
    int
        The status the verifier exited with.
    """
    return run.completed[-2].returncode


def _require_phases(run: ActionRun, failed: str) -> None:
    """Require every phase to succeed, and ``failed`` to have failed.

    The report step emits one outcome per label, so a phase that never ran is
    published as ``skipped`` rather than omitted. Asserting that at the
    action's own output boundary is what proves the ledger the verifier wrote
    was parsed and labelled correctly.
    """
    assert run.outputs.get(f"{failed.replace('_', '-')}-outcome") == "failure", (
        f"{failed} must be reported as the phase that refused, got {run.outputs!r}"
    )
    others = [phase for phase in PHASES if phase != failed]
    for phase in others:
        outcome = run.outputs.get(f"{phase.replace('_', '-')}-outcome")
        assert outcome in {"success", "skipped"}, (
            f"{phase} must not be reported as {outcome!r} when {failed} refused"
        )


def _require_every_phase_succeeded(run: ActionRun) -> None:
    """Require the report step to have published a successful outcome per phase."""
    for phase in PHASES:
        key = f"{phase.replace('_', '-')}-outcome"
        assert run.outputs.get(key) == "success", (
            f"{phase} must be reported successful, got {run.outputs.get(key)!r}"
        )


def test_the_action_verifies_a_published_candidate(tmp_path: pathlib.Path) -> None:
    """Download, verify, extract, and probe a release the host serves exactly."""
    run = execute(tmp_path)

    assert len(run.completed) == 5, run.completed[-1].stderr
    assert all(result.returncode == 0 for result in run.completed), run.completed[
        -1
    ].stderr
    assert run.verdict() == {
        "phases": dict.fromkeys(PHASES, "success"),
        "verdict": "verified",
    }, "a verified candidate must leave a verified verdict"
    _require_every_phase_succeeded(run)
    # The three downloads are the public ones: the crate from the registry host
    # and the archive with its sidecar from the release's own download URL.
    assert any("static.crates.io" in url for url in run.curl_log), run.curl_log
    assert all(
        name in "\n".join(run.curl_log)
        for name in (CRATE_NAME, ARCHIVE_NAME, SIDECAR_NAME)
    ), "each published name must be downloaded once"
    assert set(run.baselines) == {interpreter_baseline()}, run.baselines
    extracted = run.work_dir / "verdict.json"
    assert extracted.is_file(), "the verdict belongs in the work directory"


def test_a_download_the_host_never_delivered_refuses_the_crate_phase(
    tmp_path: pathlib.Path,
) -> None:
    """Refuse a crate whose transfer failed, and still report every phase."""
    run = execute(tmp_path, smoke=Smoke(refused=frozenset({CRATE_NAME})))

    assert _refusal_status(run) == EXIT_DOWNLOAD_REJECTED, run.completed[-2].stderr
    assert len(run.completed) == 5, "the report step must run after a refusal"
    assert run.completed[-1].returncode == 0, run.completed[-1].stderr
    _require_phases(run, "crate_download")
    assert run.verdict() is not None, "a refused smoke must still publish a verdict"
    assert run.verdict()["verdict"] == "failed", run.verdict()


def test_an_empty_download_refuses_the_archive_phase(tmp_path: pathlib.Path) -> None:
    """Refuse an archive the host answered with a 404 and an empty body."""
    run = execute(tmp_path, smoke=Smoke(not_found=frozenset({ARCHIVE_NAME})))

    assert _refusal_status(run) == EXIT_DOWNLOAD_REJECTED, run.completed[-2].stderr
    _require_phases(run, "archive_download")


def test_bytes_that_are_not_the_published_ones_refuse_the_verification(
    tmp_path: pathlib.Path,
) -> None:
    """Refuse a crate whose bytes are not the digest the package record names."""
    run = execute(
        tmp_path, smoke=Smoke(served={CRATE_NAME: b"not the published crate\n"})
    )

    assert _refusal_status(run) == EXIT_DOWNLOAD_REJECTED, run.completed[-2].stderr
    _require_phases(run, "crate_verification")
    assert run.outputs.get("crate-download-outcome") == "success", (
        "the bytes arrived; it is the digest that disagrees"
    )


def test_an_unsafe_archive_is_refused_before_the_probe(
    tmp_path: pathlib.Path,
) -> None:
    """Refuse an archive whose member would escape the extraction directory."""
    run = execute(tmp_path, smoke=Smoke(members=("../escape",)))

    assert _refusal_status(run) == EXIT_ARCHIVE_REJECTED, run.completed[-2].stderr
    _require_phases(run, "archive_verification")
    assert run.outputs.get("executable-probe-outcome") == "skipped", (
        "an archive that cannot be extracted must not be probed"
    )


def test_a_binary_reporting_another_version_refuses_the_probe(
    tmp_path: pathlib.Path,
) -> None:
    """Refuse a shipped binary whose banner is not the candidate's version."""
    run = execute(
        tmp_path,
        smoke=Smoke(binary=b"#!/bin/sh\nprintf '%s\\n' 'netsuke 9.9.9'\n"),
    )

    assert _refusal_status(run) == EXIT_PROBE_REJECTED, run.completed[-2].stderr
    _require_phases(run, "executable_probe")


def test_a_populated_work_directory_is_refused_at_validation(
    tmp_path: pathlib.Path,
) -> None:
    """Refuse an earlier attempt's leftovers, and report every phase as skipped."""

    def leave_files(step: int, work_dir: pathlib.Path) -> None:
        """Create the work directory with a file an earlier attempt left.

        Runs before the validation step, which is the only moment the directory
        can hold anything: after that the action creates it itself.
        """
        if step != 0:
            return
        work_dir.mkdir(parents=True)
        (work_dir / "stale.json").write_text("{}", encoding="utf-8")

    run = execute(tmp_path, smoke=Smoke(before_step=leave_files))

    assert len(run.completed) == 2, (
        "only validation and the always-run report step may execute, "
        f"got {[result.returncode for result in run.completed]}"
    )
    assert run.completed[0].returncode != 0, (
        "a populated work directory must be refused"
    )
    assert run.outputs.get("crate-download-outcome") == "skipped", (
        "a refused validation must report every phase as skipped"
    )
    assert "verdict-path" not in run.outputs, (
        "a refused validation must not name an earlier run's verdict"
    )
    assert run.verdict() is None, "no verdict may be attributed to this attempt"


def test_an_empty_repository_is_refused_at_validation(tmp_path: pathlib.Path) -> None:
    """Refuse an empty required input before any plan or download is attempted."""
    run = execute(tmp_path, smoke=Smoke(inputs={"repository": ""}))

    assert len(run.completed) == 2, (
        "an empty input must stop before planning, "
        f"got {[result.returncode for result in run.completed]}"
    )
    assert run.completed[0].returncode != 0, "an empty input must be refused"
    assert "RELEASE_REPOSITORY must not be empty" in run.completed[0].stderr, (
        run.completed[0].stderr
    )
    assert not run.curl_log, "a refused validation must not reach the network"
