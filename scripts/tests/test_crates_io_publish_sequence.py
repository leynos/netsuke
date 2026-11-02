"""Pin the registry observations to the permitted publication decisions."""

import dataclasses
import hashlib
import json
import typing as typ

import pytest
from publish_crates_io_cli import (
    EXIT_ACTION_REQUIRED,
    EXIT_OK,
    EXIT_RESPONSE_ERROR,
    main,
)

if typ.TYPE_CHECKING:
    import pathlib

VERSION = "1.2.3"
PACKAGE = "netsuke-build"
SHA256 = "ab" * 32


@dataclasses.dataclass(frozen=True, slots=True)
class RegistryScenario:
    """Describe one recorded registry response and its permitted sequence."""

    status: int
    response: bytes
    expected_state: str
    expected_action: str
    expected_exit: int


def _response(*, checksum: str = SHA256, yanked: bool = False) -> bytes:
    """Encode one crates.io version observation for the intended release."""
    return json.dumps({
        "version": {"num": VERSION, "checksum": checksum, "yanked": yanked}
    }).encode("utf-8")


def _classify_record(
    output: pathlib.Path, *, status: int, response: bytes, sha256: str = SHA256
) -> dict[str, object]:
    """Classify one recorded registry response and return its decision record."""
    body = output.with_suffix(".probe")
    _ = body.write_bytes(response)

    result = main([
        "classify",
        "--status",
        str(status),
        "--body",
        str(body),
        "--version",
        VERSION,
        "--sha256",
        sha256,
        "--output",
        str(output),
    ])

    assert result == EXIT_OK, "a classified response must produce a decision record"
    return json.loads(output.read_text(encoding="utf-8"))


@pytest.mark.parametrize(
    "scenario",
    [
        pytest.param(
            RegistryScenario(404, b"", "absent", "publish", EXIT_OK),
            id="absent",
        ),
        pytest.param(
            RegistryScenario(200, _response(), "identical", "skip", EXIT_OK),
            id="identical",
        ),
        pytest.param(
            RegistryScenario(
                200,
                _response(checksum="cd" * 32),
                "conflict",
                "fail",
                EXIT_ACTION_REQUIRED,
            ),
            id="conflicting",
        ),
        pytest.param(
            RegistryScenario(
                200, _response(yanked=True), "yanked", "fail", EXIT_ACTION_REQUIRED
            ),
            id="yanked",
        ),
        pytest.param(
            RegistryScenario(200, b"[]", "indeterminate", "fail", EXIT_RESPONSE_ERROR),
            id="malformed-response",
        ),
        pytest.param(
            RegistryScenario(
                503, b"busy", "indeterminate", "fail", EXIT_ACTION_REQUIRED
            ),
            id="unavailable-response",
        ),
    ],
)
def test_state_selects_only_the_permitted_registry_action(
    tmp_path: pathlib.Path, scenario: RegistryScenario
) -> None:
    """Only absent publishes; both safe states verify, and refusals stop early."""
    body = tmp_path / "probe.json"
    decision = tmp_path / "decision.json"
    _ = body.write_bytes(scenario.response)

    result = main([
        "classify",
        "--status",
        str(scenario.status),
        "--body",
        str(body),
        "--version",
        VERSION,
        "--sha256",
        SHA256,
        "--output",
        str(decision),
    ])

    assert result == scenario.expected_exit, "registry state must select its exit code"
    record = json.loads(decision.read_text(encoding="utf-8"))
    assert record["state"] == scenario.expected_state, (
        "record the observed registry state"
    )
    assert record["action"] == scenario.expected_action, "record its permitted action"


def test_interrupted_publication_resumes_without_republishing(
    tmp_path: pathlib.Path,
) -> None:
    """A rerun after an interrupted publication skips, then verifies the bytes.

    The first attempt observes an absent version and decides to publish, but
    the run stops before the download is verified. The rerun must find the
    version already published with the same digest, stand down rather than
    publish again, and then confirm that the bytes now served match the
    packaged digest. Re-publishing here would be rejected by the registry for
    an immutable version, so the skip is what makes the retry converge.
    """
    archive_body = b"packaged crate bytes"
    digest = hashlib.sha256(archive_body).hexdigest()

    first = _classify_record(
        tmp_path / "first.json", status=404, response=b"", sha256=digest
    )
    assert first["state"] == "absent", "the first attempt observes no publication"
    assert first["action"] == "publish", "the first attempt must publish"

    second = _classify_record(
        tmp_path / "second.json",
        status=200,
        response=_response(checksum=digest),
        sha256=digest,
    )
    assert second["state"] == "identical", "the rerun observes its own publication"
    assert second["action"] == "skip", (
        "the rerun must not publish the same version twice"
    )

    downloaded = tmp_path / f"{PACKAGE}-{VERSION}.crate"
    _ = downloaded.write_bytes(archive_body)
    verdict = tmp_path / "verdict.json"
    result = main([
        "verify",
        "--archive",
        str(downloaded),
        "--crate",
        PACKAGE,
        "--version",
        VERSION,
        "--sha256",
        digest,
        "--output",
        str(verdict),
    ])

    assert result == EXIT_OK, "the published bytes must match the packaged digest"
    assert json.loads(verdict.read_text(encoding="utf-8")) == {
        "sha256": digest,
        "verdict": "verified",
    }, "record the digest observed from the registry"
