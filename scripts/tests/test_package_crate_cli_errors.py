"""Keep packaging discovery, digest, and record failures inside the CLI contract.

The command may have produced a valid archive before one of these later steps
fails. Each failure must still return a stable status and a useful diagnosis,
without leaving a package record that downstream stages could mistake as valid.
"""

import pathlib
import typing as typ

from package_crate_cli import EXIT_ARTIFACT_ERROR, main
from package_crate_support import packaging_argv, stub_cargo

if typ.TYPE_CHECKING:
    import pytest


def test_the_cli_reports_archive_discovery_io_errors(
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Report staging lookup failures instead of exposing a traceback."""
    destination = tmp_path / "package.json"
    _ = stub_cargo(monkeypatch, b"netsuke crate")

    def _fail_glob(_path: pathlib.Path, _pattern: str) -> list[pathlib.Path]:
        """Represent a permission failure while listing cargo's archive."""
        raise PermissionError

    monkeypatch.setattr(pathlib.Path, "glob", _fail_glob)
    status = main(packaging_argv(destination, target_dir=tmp_path / "target"))

    assert status == EXIT_ARTIFACT_ERROR, (
        f"discovery failure must return 3, got {status}"
    )
    assert "could not inspect" in capsys.readouterr().err, (
        "the failure must identify archive discovery"
    )
    assert not destination.exists(), "a missing archive must not leave a record"


def test_the_cli_reports_archive_hashing_io_errors(
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Report a read failure after discovery without writing a package record."""
    destination = tmp_path / "package.json"
    _ = stub_cargo(monkeypatch, b"netsuke crate")

    def _fail_hash(_path: pathlib.Path) -> str:
        """Represent an archive that cannot be read for hashing."""
        raise PermissionError

    monkeypatch.setattr(
        "package_crate_cli.release_digest_files.digest_of_file", _fail_hash
    )
    status = main(packaging_argv(destination, target_dir=tmp_path / "target"))

    assert status == EXIT_ARTIFACT_ERROR, f"hash failure must return 3, got {status}"
    assert "could not read the packaged archive" in capsys.readouterr().err, (
        "the failure must identify archive hashing"
    )
    assert not destination.exists(), "an unreadable archive must not leave a record"


def test_the_cli_cleans_up_when_package_record_persistence_fails(
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Return a controlled failure and remove the temporary record on replace error."""
    destination = tmp_path / "package.json"
    _ = stub_cargo(monkeypatch, b"netsuke crate")

    def _fail_replace(_source: pathlib.Path, _target: pathlib.Path) -> pathlib.Path:
        """Represent a filesystem that refuses to persist the package record."""
        raise PermissionError

    monkeypatch.setattr(pathlib.Path, "replace", _fail_replace)
    status = main(packaging_argv(destination, target_dir=tmp_path / "target"))

    assert status == EXIT_ARTIFACT_ERROR, f"record failure must return 3, got {status}"
    assert "could not write package record" in capsys.readouterr().err, (
        "the failure must identify package record persistence"
    )
    assert not destination.exists(), "failed persistence must not expose a record"
    leftovers = [
        path.name
        for path in tmp_path.iterdir()
        if path.name.startswith(".package.json.")
    ]
    assert not leftovers, (
        f"failed persistence must clean its temporary file: {leftovers}"
    )
