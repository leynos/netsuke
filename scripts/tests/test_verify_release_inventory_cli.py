"""Exercise the inventory-verification CLI the verifier job invokes.

The two agreements it checks are proved in their own modules. These tests pin
the composition: that the declared set is read from the file the packaging
stage wrote, that a missing archive is reported as a missing archive rather
than as a malformed sidecar, and that a sidecar describing different bytes
stops the run. Everything runs against a temporary directory; no test here
opens a socket.
"""

import json
import typing as typ

import pytest
import release_digest
import release_inventory
from verify_release_inventory_cli import (
    EXIT_INPUT_UNREADABLE,
    EXIT_INVENTORY_REJECTED,
    EXIT_OK,
    build_parser,
    main,
    verify,
)

if typ.TYPE_CHECKING:
    import pathlib

ARCHIVE = "netsuke-build-0.1.0-x86_64-unknown-linux-gnu.tar.gz"
SIDECAR = f"{ARCHIVE}.sha256"
BODY = b"netsuke archive bytes"


def _stage(directory: pathlib.Path, *, sidecar_body: bytes | None = None) -> None:
    """Write the archive and a sidecar describing it."""
    directory.mkdir(parents=True, exist_ok=True)
    archive = directory / ARCHIVE
    _ = archive.write_bytes(BODY)
    digest = release_digest.digest_of_file(archive).encode()
    recorded = sidecar_body if sidecar_body is not None else digest
    _ = (directory / SIDECAR).write_text(
        f"{recorded.decode()}  dist/{ARCHIVE}\n", encoding="utf-8"
    )


def _declaration(destination: pathlib.Path, names: list[str]) -> pathlib.Path:
    """Write the declared asset list the packaging stage produces."""
    _ = destination.write_text("\n".join(names) + "\n", encoding="utf-8")
    return destination


def test_verify_accepts_the_declared_assets(tmp_path: pathlib.Path) -> None:
    """Accept an archive with a matching sidecar and report each asset's own digest.

    The sidecar entry is asserted separately and deliberately. Returning the
    archive's digest under the sidecar's name type-checks and looks right in a
    two-key mapping, but GitHub reports a digest for the sidecar file itself,
    so the finalizer would compare the wrong value. Computing the two digests
    from the staged files rather than from the CLI's own output keeps the
    assertion independent of the code under test.
    """
    staging = tmp_path / "dist"
    _stage(staging)

    verified = verify(staging, expected=[ARCHIVE, SIDECAR])

    assert sorted(verified) == [ARCHIVE, SIDECAR], (
        f"both assets must be keyed by name, got {sorted(verified)}"
    )
    assert verified[ARCHIVE] == release_digest.digest_of_file(staging / ARCHIVE), (
        "the archive's digest must be the digest of its bytes"
    )
    assert verified[SIDECAR] == release_digest.digest_of_file(staging / SIDECAR), (
        "the sidecar's digest must be the digest of the sidecar file, not of "
        "the archive it describes"
    )
    assert verified[SIDECAR] != verified[ARCHIVE], (
        "the two entries must differ, or this assertion cannot tell the bug "
        "from the fix"
    )


def test_verify_reports_a_missing_archive(tmp_path: pathlib.Path) -> None:
    """Reject a release carrying a sidecar whose archive never arrived.

    This is the 2026-10-12 failure: every staged file passed inspection, and
    the release was unusable because the archives were the files that were
    missing. The declaration is what makes the absence visible.
    """
    staging = tmp_path / "dist"
    staging.mkdir()
    _ = (staging / SIDECAR).write_text("ab  dist/x\n", encoding="utf-8")

    with pytest.raises(release_inventory.MissingAssetError):
        _ = verify(staging, expected=[ARCHIVE, SIDECAR])


def test_verify_reports_an_unexpected_archive(tmp_path: pathlib.Path) -> None:
    """Reject an asset the release never declared."""
    staging = tmp_path / "dist"
    _stage(staging)
    _ = (staging / "extra.tar.gz").write_bytes(b"extra")

    with pytest.raises(release_inventory.UnexpectedAssetError):
        _ = verify(staging, expected=[ARCHIVE, SIDECAR])


def test_verify_rejects_a_sidecar_describing_different_bytes(
    tmp_path: pathlib.Path,
) -> None:
    """Stop when the sidecar's digest is not the archive's, not merely well formed."""
    staging = tmp_path / "dist"
    _stage(staging, sidecar_body=b"cd" * 32)

    with pytest.raises(release_digest.DigestMismatchError):
        _ = verify(staging, expected=[ARCHIVE, SIDECAR])


def test_the_cli_records_the_verified_digests(tmp_path: pathlib.Path) -> None:
    """Write the digests the release was verified against."""
    staging = tmp_path / "dist"
    _stage(staging)
    declaration = _declaration(tmp_path / "declared.txt", [ARCHIVE, SIDECAR])
    destination = tmp_path / "verified.json"

    code = main([
        "--directory",
        str(staging),
        "--expected",
        str(declaration),
        "--output",
        str(destination),
    ])

    assert code == EXIT_OK, f"a complete release must verify, got {code}"
    record = json.loads(destination.read_text(encoding="utf-8"))
    assert sorted(record["assets"]) == [ARCHIVE, SIDECAR], (
        f"both assets must be recorded by name, got {sorted(record['assets'])}"
    )


def test_the_cli_rejects_an_incomplete_release_and_writes_no_record(
    tmp_path: pathlib.Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Fail closed, naming the disagreement, and leave no record behind."""
    staging = tmp_path / "dist"
    staging.mkdir()
    _ = (staging / SIDECAR).write_text("ab  dist/x\n", encoding="utf-8")
    declaration = _declaration(tmp_path / "declared.txt", [ARCHIVE, SIDECAR])
    destination = tmp_path / "verified.json"

    code = main([
        "--directory",
        str(staging),
        "--expected",
        str(declaration),
        "--output",
        str(destination),
    ])

    assert code == EXIT_INVENTORY_REJECTED, (
        f"an incomplete release must be refused, got {code}"
    )
    assert not destination.exists(), "a rejected inventory must leave no record"
    assert ARCHIVE in capsys.readouterr().err, (
        "the diagnostic must name the missing asset"
    )


def test_the_declaration_ignores_blank_lines(tmp_path: pathlib.Path) -> None:
    """Read the declaration leniently, so a trailing newline is not an asset."""
    staging = tmp_path / "dist"
    _stage(staging)
    declaration = tmp_path / "declared.txt"
    _ = declaration.write_text(f"\n{ARCHIVE}\n\n{SIDECAR}\n\n", encoding="utf-8")
    destination = tmp_path / "verified.json"

    code = main([
        "--directory",
        str(staging),
        "--expected",
        str(declaration),
        "--output",
        str(destination),
    ])

    assert code == EXIT_OK, f"blank lines must not be read as asset names, got {code}"


def test_the_cli_reports_an_unreadable_declaration_separately(
    tmp_path: pathlib.Path,
) -> None:
    """Distinguish a bad path from a bad release, which sends an operator elsewhere."""
    destination = tmp_path / "verified.json"

    code = main([
        "--directory",
        str(tmp_path / "dist"),
        "--expected",
        str(tmp_path / "absent.txt"),
        "--output",
        str(destination),
    ])

    assert code == EXIT_INPUT_UNREADABLE, (
        f"an unreadable declaration must not be reported as a rejected "
        f"release, got {code}"
    )
    assert not destination.exists(), "nothing was verified, so nothing is recorded"


@pytest.mark.parametrize("missing", ["--directory", "--expected", "--output"])
def test_the_cli_requires_a_directory_a_declaration_and_an_output(missing: str) -> None:
    """Refuse to guess any of the three, since each names a different file."""
    complete = {
        "--directory": "dist",
        "--expected": "declared.txt",
        "--output": "verified.json",
    }
    argv = [
        part
        for flag, value in complete.items()
        if flag != missing
        for part in (flag, value)
    ]

    with pytest.raises(SystemExit):
        _ = build_parser().parse_args(argv)
