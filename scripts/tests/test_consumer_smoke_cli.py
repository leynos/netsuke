"""Test the consumer-smoke CLI end to end over a temporary workspace.

Every input is a real file written into ``tmp_path``: a package record, a
trusted digest record, the release declaration, and the saved downloads. The
probe is the only injected seam, so the suite never launches a downloaded
binary, and nothing here reaches the network.

Each case asserts both artefacts the run leaves behind. The verdict says what a
consumer is told; the phase log says which phase was in flight, which is the
fact an operator needs when a run dies rather than returns.

Run via ``make test-release-publication``.
"""

import json
import typing as typ

import consumer_smoke_evidence
import pytest
from consumer_smoke_harness import (
    ARCHIVE_NAME,
    BANNER,
    BINARY_BYTES,
    CRATE_NAME,
    PACKAGE,
    SIDECAR_NAME,
    VERSION,
    Workspace,
    fake_probe,
    read_phase_log,
    read_verdict,
    run_plan,
    run_verify,
    workspace,
    write_json,
)
from release_digest_files import digest_of_file

if typ.TYPE_CHECKING:
    import pathlib
    import subprocess

PHASES = (
    "crate_download",
    "crate_verification",
    "archive_download",
    "archive_verification",
    "executable_probe",
)


def _prepared(tmp_path: pathlib.Path, *, symlink: bool = False) -> Workspace:
    """Return a workspace whose plan has already been written.

    Returns
    -------
    Workspace
        The workspace, with the plan document on disk.
    """
    staged = workspace(tmp_path, symlink=symlink)
    assert run_plan(staged) == 0, "the plan must exist before verification"
    return staged


def test_plan_names_the_public_downloads(
    tmp_path: pathlib.Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Write the plan with the crate and GitHub release URLs and digests."""
    staged = workspace(tmp_path)

    assert run_plan(staged) == 0, "planning for a verified release must succeed"
    assert not capsys.readouterr().out, "a successful plan prints nothing"

    plan = json.loads(staged.plan.read_text(encoding="utf-8"))
    assert plan["crate"]["url"] == (
        f"https://static.crates.io/crates/{PACKAGE}/{CRATE_NAME}"
    ), "the crate must come from the public registry host"
    assert plan["archive"]["url"] == (
        f"https://github.com/leynos/netsuke/releases/download/v{VERSION}/{ARCHIVE_NAME}"
    ), "the archive must come from the public release URL"
    assert plan["sidecar"]["url"].endswith(SIDECAR_NAME), (
        "the sidecar is downloaded from the release beside the archive"
    )
    assert plan["archive"]["sha256"] == digest_of_file(staged.archive), (
        "each download must carry its trusted digest"
    )
    assert plan["user_agent"].startswith("netsuke-release-verifier"), (
        "the plan must carry the request identity the workflow curl uses"
    )


def test_plan_refuses_evidence_from_another_commit(tmp_path: pathlib.Path) -> None:
    """Refuse a package record that was built from a different revision."""
    staged = workspace(tmp_path)
    recorded = json.loads(staged.identity.read_text(encoding="utf-8"))
    recorded["identity"]["commit"] = "1" * 40
    write_json(staged.identity, recorded)

    assert run_plan(staged) == 1, "another run's candidate must be refused"
    assert not staged.plan.exists(), "a refused plan must write no document"


def test_verify_accepts_the_published_candidate(
    tmp_path: pathlib.Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Verify every phase and leave the extracted binary behind."""
    staged = _prepared(tmp_path)

    assert run_verify(staged, runner=fake_probe()) == 0, (
        "a matching download and banner must verify"
    )
    assert not capsys.readouterr().out, "a successful verify prints nothing"
    assert read_verdict(staged) == {
        "phases": dict.fromkeys(PHASES, "success"),
        "verdict": "verified",
    }, "every phase must be recorded as successful"
    binary = staged.extract / "netsuke"
    assert binary.read_bytes() == BINARY_BYTES, (
        "the shipped binary must be the archive's bytes"
    )


def test_verify_timestamps_every_phase_it_completed(tmp_path: pathlib.Path) -> None:
    """Record a start and an end for each phase the run finished."""
    staged = _prepared(tmp_path)

    assert run_verify(staged, runner=fake_probe()) == 0, "the smoke must verify"
    log = read_phase_log(staged)
    assert set(log) == set(PHASES), "the log must name every contract phase"
    for label in PHASES:
        started = log[label]["started_ns"]
        ended = log[label]["ended_ns"]
        assert log[label]["outcome"] == "success", f"{label} must have succeeded"
        assert isinstance(started, int), (
            f"{label} must record its start as an integer nanosecond stamp"
        )
        assert isinstance(ended, int), (
            f"{label} must record its end as an integer nanosecond stamp"
        )
        assert started <= ended, f"{label}'s end must not precede its start"


def test_verify_reports_a_download_the_host_did_not_deliver(
    tmp_path: pathlib.Path,
) -> None:
    """Stop at the download phase and record the phases that never ran."""
    staged = _prepared(tmp_path)
    absent = tmp_path / "absent.status"
    _ = absent.write_text("404\n", encoding="utf-8")

    assert (
        run_verify(staged, runner=fake_probe(), overrides={"crate-status": absent}) == 2
    ), "an undelivered download must be refused"

    verdict = read_verdict(staged)
    assert verdict["phases"]["crate_download"] == "failure", (
        "the phase that observed the missing download must be recorded as failed"
    )
    assert verdict["phases"]["crate_verification"] == "skipped", (
        "a phase after the failure must be recorded as skipped"
    )
    assert verdict["verdict"] == "failed", (
        "a refused run must not report a verified verdict"
    )
    failure = read_phase_log(staged)["crate_download"]
    assert failure["outcome"] == "failure", "the phase in flight must be recorded"
    assert failure["ended_ns"] is None, (
        "a phase that never finished must record no end timestamp"
    )


def test_verify_reports_a_saved_download_it_cannot_read(
    tmp_path: pathlib.Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Record an unreadable saved download as that phase's own refusal.

    The file is present, so this is not "the host did not deliver it": the run
    listed the file and then a stat or a hash of it failed. That belongs to the
    download phase, which must record the failure and return the download
    status rather than letting the host's ``OSError`` end the run with no
    verdict at all.
    """
    staged = _prepared(tmp_path)

    def unreadable(**_kwargs: object) -> object:
        """Fail the way a stat or a hash of the saved body would."""
        msg = "permission denied"
        raise OSError(msg)

    monkeypatch.setattr(consumer_smoke_evidence, "payload_facts", unreadable)

    assert run_verify(staged, runner=fake_probe()) == 2, (
        "a download the smoke cannot read must be refused as a download"
    )
    verdict = read_verdict(staged)
    assert verdict["phases"]["crate_download"] == "failure", (
        "the phase that needed the facts must be recorded as failed"
    )
    assert verdict["phases"]["crate_verification"] == "skipped", (
        "no later phase may run once the download is refused"
    )


def test_verify_reports_bytes_that_are_not_the_published_ones(
    tmp_path: pathlib.Path,
) -> None:
    """Refuse an archive whose bytes differ from the trusted digest."""
    staged = _prepared(tmp_path)
    with staged.archive.open("ab") as handle:
        _ = handle.write(b"tampered")

    assert run_verify(staged, runner=fake_probe()) == 2, (
        "bytes the trusted record does not name must be refused"
    )
    phases = read_verdict(staged)["phases"]
    assert (phases["archive_download"], phases["archive_verification"]) == (
        "success",
        "failure",
    ), "the download arrived, and its verification is what failed"


def test_verify_reports_an_archive_that_is_unsafe_to_extract(
    tmp_path: pathlib.Path,
) -> None:
    """Refuse an archive carrying a symlink instead of a binary."""
    staged = _prepared(tmp_path, symlink=True)

    assert run_verify(staged, runner=fake_probe()) == 3, (
        "an archive with an unsafe member must be refused"
    )
    assert read_verdict(staged)["phases"]["archive_verification"] == "failure", (
        "the archive phase must record the refusal"
    )
    assert not list(staged.extract.iterdir()), (
        "an archive refused before extraction must write nothing"
    )


@pytest.mark.parametrize(
    ("returncode", "stdout"),
    [(0, "netsuke 9.9.9\n"), (1, BANNER)],
    ids=["wrong-banner", "nonzero-exit"],
)
def test_verify_reports_an_executable_that_disagrees_with_the_candidate(
    tmp_path: pathlib.Path, returncode: int, stdout: str
) -> None:
    """Refuse a binary that does not report exactly the candidate version."""
    staged = _prepared(tmp_path)

    assert (
        run_verify(staged, runner=fake_probe(returncode=returncode, stdout=stdout)) == 4
    ), "a binary that is not the candidate must be refused"
    assert read_verdict(staged)["phases"]["executable_probe"] == "failure", (
        "the probe phase must record the refusal"
    )


def test_verify_reports_a_probe_that_never_ran(tmp_path: pathlib.Path) -> None:
    """Refuse an executable that could not be launched at all."""

    def runner(
        command: list[str], **_kwargs: object
    ) -> subprocess.CompletedProcess[str]:
        """Fail the launch the way a missing binary would."""
        raise FileNotFoundError(command[0])

    staged = _prepared(tmp_path)

    assert run_verify(staged, runner=runner) == 4, (
        "an unlaunchable executable is the probe phase's failure"
    )


def test_verify_refuses_a_plan_another_run_wrote(tmp_path: pathlib.Path) -> None:
    """Refuse a saved plan that is not the one this evidence derives."""
    staged = _prepared(tmp_path)
    plan = json.loads(staged.plan.read_text(encoding="utf-8"))
    plan["archive"]["url"] += "?redirected"
    write_json(staged.plan, plan)

    assert run_verify(staged, runner=fake_probe()) == 1, (
        "a plan naming other downloads must be refused before anything is read"
    )
    assert not staged.verdict.exists(), (
        "a run refused before its first phase writes no verdict artefact"
    )
    assert not staged.phase_log.exists(), (
        "a run refused before its first phase writes no phase log"
    )


def test_verify_reports_an_unreadable_trusted_record(tmp_path: pathlib.Path) -> None:
    """Refuse evidence that is not a digest record at all."""
    staged = workspace(tmp_path)
    _ = staged.trusted.write_text('{"assets": []}', encoding="utf-8")

    assert run_verify(staged, runner=fake_probe()) == 1, (
        "an unusable trusted record must be refused as evidence"
    )
