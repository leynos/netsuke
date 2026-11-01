"""Pin the registry observations to the permitted publication decisions."""

import dataclasses
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
