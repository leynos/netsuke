"""Exercise the registry-exchange CLI the publisher job invokes.

The classification and the archive verification are both proved in their own
modules. These tests pin the seam: that the CLI reaches the right policy
function with the arguments a workflow supplies, that it records a decision
before it refuses, and that its exit-code table covers every verdict the
policy module can return. No test here opens a socket.
"""

import json
import typing as typ

import pytest
import verify_crates_io_bytes
from publish_crates_io_cli import (
    EXIT_ACTION_REQUIRED,
    EXIT_OK,
    EXIT_VERIFICATION_FAILED,
    VERDICT_EXITS,
    build_parser,
    main,
)

if typ.TYPE_CHECKING:
    import pathlib

DIGEST = "ab" * 32
CRATE = "netsuke-build"
VERSION = "0.1.0"


def _classify_argv(
    destination: pathlib.Path, *, status: int, body: pathlib.Path | None = None
) -> list[str]:
    """Return a classify invocation, optionally naming a recorded body."""
    argv = [
        "classify",
        "--status",
        str(status),
        "--sha256",
        DIGEST,
        "--output",
        str(destination),
    ]
    if body is not None:
        argv.extend(["--body", str(body)])
    return argv


def _verify_argv(archive: pathlib.Path, destination: pathlib.Path) -> list[str]:
    """Return a verify invocation for ``archive``."""
    return [
        "verify",
        "--archive",
        str(archive),
        "--crate",
        CRATE,
        "--version",
        VERSION,
        "--sha256",
        DIGEST,
        "--output",
        str(destination),
    ]


def _decision(destination: pathlib.Path) -> dict[str, object]:
    """Read back the decision record the CLI wrote."""
    return json.loads(destination.read_text(encoding="utf-8"))


def _archive(tmp_path: pathlib.Path, body: bytes) -> pathlib.Path:
    """Write a downloaded archive under the name the registry would serve."""
    archive = tmp_path / f"{CRATE}-{VERSION}.crate"
    _ = archive.write_bytes(body)
    return archive


@pytest.mark.parametrize(
    ("status", "state", "action"),
    [
        (404, "absent", "publish"),
        (200, "identical", "skip"),
    ],
)
def test_the_cli_records_the_action_the_registry_state_dictates(
    tmp_path: pathlib.Path, status: int, state: str, action: str
) -> None:
    """Record both the state and the action, so the log explains the decision."""
    destination = tmp_path / "decision.json"
    body = tmp_path / "body.json"
    _ = body.write_text(
        json.dumps({"versions": [{"checksum": DIGEST, "yanked": False}]}),
        encoding="utf-8",
    )

    code = main(_classify_argv(destination, status=status, body=body))

    assert code == EXIT_OK, f"a decidable state must not fail, got {code}"
    record = _decision(destination)
    assert record["state"] == state, f"the record must name the state, got {record}"
    assert record["action"] == action, f"the record must name the action, got {record}"


def test_the_cli_refuses_a_conflicting_registry_entry(tmp_path: pathlib.Path) -> None:
    """Stop on a digest that disagrees, and record the state that stopped it."""
    destination = tmp_path / "decision.json"
    body = tmp_path / "body.json"
    _ = body.write_text(
        json.dumps({"versions": [{"checksum": "cd" * 32, "yanked": False}]}),
        encoding="utf-8",
    )

    code = main(_classify_argv(destination, status=200, body=body))

    assert code == EXIT_ACTION_REQUIRED, f"a conflict must stop the run, got {code}"
    assert _decision(destination)["action"] == "fail", (
        "the refusal must still be recorded, so the artefact names the state"
    )


def test_the_cli_refuses_a_yanked_version(
    tmp_path: pathlib.Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Stop on a yanked version even when its checksum matches."""
    destination = tmp_path / "decision.json"
    body = tmp_path / "body.json"
    _ = body.write_text(
        json.dumps({"versions": [{"checksum": DIGEST, "yanked": True}]}),
        encoding="utf-8",
    )

    code = main(_classify_argv(destination, status=200, body=body))

    assert code == EXIT_ACTION_REQUIRED, f"a yanked version must stop, got {code}"
    assert "yanked" in capsys.readouterr().err, (
        "the diagnostic must name the state that stopped the run"
    )


def test_the_cli_refuses_an_indeterminate_answer(tmp_path: pathlib.Path) -> None:
    """Stop on a status that proves nothing, rather than guessing absence."""
    destination = tmp_path / "decision.json"

    code = main(_classify_argv(destination, status=429))

    assert code == EXIT_ACTION_REQUIRED, (
        f"an unreadable answer must not be treated as absent, got {code}"
    )
    assert _decision(destination)["state"] == "indeterminate", (
        "the record must say the answer was unreadable"
    )


def test_the_cli_treats_a_missing_body_as_absent_only_for_404(
    tmp_path: pathlib.Path,
) -> None:
    """Read absence from the status alone, which is what the policy module does."""
    destination = tmp_path / "decision.json"

    code = main(_classify_argv(destination, status=404))

    assert code == EXIT_OK, f"a 404 with no body means absent, got {code}"
    assert _decision(destination)["action"] == "publish", (
        "an absent version is the one case that proceeds to publication"
    )


def test_the_cli_verifies_a_matching_archive(tmp_path: pathlib.Path) -> None:
    """Accept the bytes the identity record describes, and record the digest."""
    archive = _archive(tmp_path, b"netsuke crate")
    import release_digest

    digest = release_digest.digest_of_file(archive)
    destination = tmp_path / "verdict.json"
    argv = _verify_argv(archive, destination)
    argv[argv.index("--sha256") + 1] = digest

    code = main(argv)

    assert code == EXIT_OK, f"matching bytes must verify, got {code}"
    record = json.loads(destination.read_text(encoding="utf-8"))
    assert record["verdict"] == "verified", "the verdict must say the bytes verified"
    assert record["sha256"] == digest, "the record must carry the digest it read"


def test_the_cli_refuses_bytes_that_disagree_with_the_record(
    tmp_path: pathlib.Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Stop on a mismatch, and write no verdict file."""
    archive = _archive(tmp_path, b"netsuke crate")
    destination = tmp_path / "verdict.json"

    code = main(_verify_argv(archive, destination))

    assert code == EXIT_VERIFICATION_FAILED, f"a mismatch must fail, got {code}"
    assert not destination.exists(), "a failed verification has no verdict to record"
    assert "mismatch" in capsys.readouterr().err, (
        "the diagnostic must name the mismatch"
    )


def test_the_cli_refuses_an_absent_archive(tmp_path: pathlib.Path) -> None:
    """Report an empty download as absence, not as corrupted bytes."""
    archive = _archive(tmp_path, b"")
    destination = tmp_path / "verdict.json"

    code = main(_verify_argv(archive, destination))

    assert code == EXIT_VERIFICATION_FAILED, f"absent bytes must fail, got {code}"
    assert not destination.exists(), "an absent archive has no verdict to record"


def test_the_exit_table_covers_every_verdict_the_policy_module_declares() -> None:
    """Require the table to name every verdict, so no case falls to a default.

    The policy module exposes its verdict vocabulary for exactly this check. A
    verdict added there without a row here would reach a dispatch that had no
    branch for it, and the run would proceed on an unhandled answer.
    """
    declared = set(verify_crates_io_bytes.declared_verdicts())
    assert set(VERDICT_EXITS) == declared, (
        f"the exit table must cover exactly the declared verdicts; "
        f"missing {sorted(declared - set(VERDICT_EXITS))!r}, "
        f"extra {sorted(set(VERDICT_EXITS) - declared)!r}"
    )


def test_the_exit_table_treats_only_verified_as_success() -> None:
    """Require every non-verified verdict to stop the release."""
    successes = sorted(verdict for verdict, code in VERDICT_EXITS.items() if code == 0)
    assert successes == ["verified"], (
        f"only a verified archive may proceed, found {successes!r}"
    )


def test_the_classify_mode_requires_a_status_and_a_digest() -> None:
    """Refuse to guess either value, since each decides what the run does."""
    with pytest.raises(SystemExit):
        _ = build_parser().parse_args(["classify", "--status", "404"])


def test_the_cli_requires_a_mode() -> None:
    """Refuse an invocation that names neither half of the exchange."""
    with pytest.raises(SystemExit):
        _ = build_parser().parse_args([])
