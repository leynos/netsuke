"""Specify missing-file and filesystem-error behaviour for optional evidence."""

from pathlib import Path

import ci_test_evidence_files
import pytest
from ci_test_evidence_files import read_inventories, read_logs


def test_missing_optional_files_are_omitted(
    attempt: tuple[dict, dict], tmp_path: Path
) -> None:
    """Treat only absent optional logs and inventories as unavailable."""
    run, jobs = attempt
    log_dir, telemetry_dir = tmp_path / "logs", tmp_path / "telemetry"
    log_dir.mkdir()
    telemetry_dir.mkdir()

    assert not read_logs(log_dir, jobs["jobs"]), (
        "An absent log must remain an optional observation"
    )
    assert not read_inventories(telemetry_dir, run, jobs["jobs"]), (
        "An absent inventory must remain an optional observation"
    )


def test_log_read_permission_error_is_not_hidden(
    attempt: tuple[dict, dict], tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Propagate inaccessible existing logs instead of marking them absent."""
    _, jobs = attempt
    log_dir = tmp_path / "logs"
    log_dir.mkdir()
    log_path = log_dir / "456.log"
    log_path.write_text("saved log", encoding="utf-8")
    original_read_text = Path.read_text

    monkeypatch.setattr(Path, "is_file", lambda _path: False)

    def denied_read_text(
        path: Path, encoding: str | None = None, errors: str | None = None
    ) -> str:
        """Deny reads of the fixture log and delegate other paths."""
        if path == log_path:
            raise PermissionError
        return original_read_text(path, encoding=encoding, errors=errors)

    monkeypatch.setattr(Path, "read_text", denied_read_text)

    with pytest.raises(PermissionError):
        read_logs(log_dir, jobs["jobs"])


def test_inventory_read_permission_error_is_not_hidden(
    attempt: tuple[dict, dict], tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Propagate inaccessible existing inventories instead of omitting them."""
    run, jobs = attempt
    telemetry_dir = tmp_path / "telemetry"
    telemetry_dir.mkdir()
    inventory_path = telemetry_dir / "456.json"
    inventory_path.write_text("{}", encoding="utf-8")
    monkeypatch.setattr(Path, "is_file", lambda _path: False)

    def denied_inventory(path: Path, identity: tuple[int, int, int]) -> dict:
        """Deny loading the fixture inventory to expose filesystem errors."""
        if path == inventory_path:
            raise PermissionError
        raise AssertionError

    monkeypatch.setattr(ci_test_evidence_files, "load_inventory", denied_inventory)

    with pytest.raises(PermissionError):
        read_inventories(telemetry_dir, run, jobs["jobs"])
