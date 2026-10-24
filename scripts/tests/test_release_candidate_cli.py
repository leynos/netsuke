"""Exercise the identity-recording CLI the packaging job invokes.

The policy module is proved separately; these tests pin the thin seam between
workflow-provided values and that module. They run the parser in-process so a
failure names the argument that was rejected, and they never touch the
network.
"""

import json
import pathlib
import typing as typ

import pytest
from release_candidate_cli import (
    EXIT_IDENTITY_REJECTED,
    EXIT_OK,
    build_parser,
    main,
)

if typ.TYPE_CHECKING:
    import argparse

COMMIT = "0123456789abcdef0123456789abcdef01234567"
PACKAGE = "netsuke-build"
BIN_NAME = "netsuke"
TAG = "v1.2.3"
VERSION = "1.2.3"


def _argv(
    destination: pathlib.Path, *, tag: str = TAG, version: str = VERSION
) -> list[str]:
    """Return the identity arguments, overriding the tag and version."""
    return [
        "--tag",
        tag,
        "--commit",
        COMMIT,
        "--package-name",
        PACKAGE,
        "--version",
        version,
        "--bin-name",
        BIN_NAME,
        "--output",
        str(destination),
    ]


def _record(destination: pathlib.Path) -> dict[str, object]:
    """Read back the identity record the CLI wrote."""
    return json.loads(destination.read_text(encoding="utf-8"))


def _parse(argv: list[str]) -> argparse.Namespace:
    """Parse ``argv`` and return the accepted namespace."""
    return build_parser().parse_args(argv)


def test_the_cli_records_a_valid_identity(tmp_path: pathlib.Path) -> None:
    """Write every field the later stages compare against."""
    destination = tmp_path / "identity.json"

    status = main(_argv(destination))

    assert status == EXIT_OK, f"a valid identity must be recorded, got {status}"
    record = _record(destination)
    assert record["tag"] == TAG, "the record must name the tag"
    assert record["commit"] == COMMIT, "the record must name the commit"
    assert record["package_name"] == PACKAGE, "the record must name the package"
    assert record["version"] == VERSION, "the record must name the version"
    assert record["bin_name"] == BIN_NAME, "the record must name the binary"
    assert record["is_prerelease"] is False, "a plain release is not a prerelease"


def test_the_cli_marks_a_prerelease_from_the_version(tmp_path: pathlib.Path) -> None:
    """Derive the prerelease flag from the version, not from the tag's spelling."""
    destination = tmp_path / "identity.json"
    prerelease = "1.2.3-beta1"

    status = main(_argv(destination, tag=f"v{prerelease}", version=prerelease))

    assert status == EXIT_OK, f"a prerelease identity must be recorded, got {status}"
    assert _record(destination)["is_prerelease"] is True, (
        "the flag must follow the version, not the tag's spelling"
    )


def test_the_cli_rejects_a_tag_that_disagrees_with_the_version(
    tmp_path: pathlib.Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Fail closed, naming the disagreement, and write no record."""
    destination = tmp_path / "identity.json"

    status = main(_argv(destination, tag="v2.0.0"))

    assert status == EXIT_IDENTITY_REJECTED, (
        f"a disagreeing tag must be refused, got {status}"
    )
    assert not destination.exists(), "a rejected identity must leave no record"
    assert "v2.0.0" in capsys.readouterr().err, "the diagnostic must name the tag"


def test_the_cli_requires_every_identity_field() -> None:
    """Refuse to guess a missing field rather than defaulting it."""
    with pytest.raises(SystemExit):
        _parse(["--tag", TAG])


def test_the_cli_exposes_the_output_path() -> None:
    """Name the destination explicitly, so the caller owns where the record lands."""
    chosen = pathlib.Path("identity.json")

    parsed = _parse(_argv(chosen))

    assert parsed.output == chosen, "the caller must choose where the record lands"
