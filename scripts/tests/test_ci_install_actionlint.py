"""Behavioural tests for ``scripts/ci/install_actionlint.py``."""

import hashlib
import os
import typing as typ

import pytest
from ci_script_support import (
    file_url,
    invoke,
    load_ci_script,
    script_reporting,
    write_executable,
    write_tarball,
)

if typ.TYPE_CHECKING:
    import pathlib

script = load_ci_script("install_actionlint")

VERSION = "1.7.12"


@pytest.fixture
def install_dir(
    monkeypatch: pytest.MonkeyPatch, tmp_path: pathlib.Path
) -> pathlib.Path:
    """Provide the install directory and the required INPUT_* pins."""
    install_dir = tmp_path / "workspace"
    install_dir.mkdir()
    monkeypatch.setenv("INPUT_INSTALL_DIR", str(install_dir))
    monkeypatch.setenv("INPUT_ACTIONLINT_VERSION", VERSION)
    monkeypatch.setenv("RUNNER_TEMP", str(tmp_path))
    return install_dir


def _publish_release(
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
    members: dict[str, bytes],
    *,
    pin_matches: bool = True,
) -> pathlib.Path:
    """Serve a release archive under the real URL layout and pin its digest."""
    root = tmp_path / "releases"
    archive = write_tarball(
        root / f"v{VERSION}" / f"actionlint_{VERSION}_linux_amd64.tar.gz", members
    )
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    monkeypatch.setenv("INPUT_RELEASE_ROOT", file_url(root))
    monkeypatch.setenv("INPUT_ACTIONLINT_SHA256", digest if pin_matches else "0" * 64)
    return archive


def test_cold_cache_downloads_verifies_and_extracts_the_binary(
    install_dir: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: pathlib.Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """With no binary present, the pinned archive is fetched and installed."""
    _publish_release(
        tmp_path,
        monkeypatch,
        {"actionlint": script_reporting(VERSION), "LICENSE": b"x"},
    )

    exit_code = invoke(script.app)

    assert exit_code == 0, "expected: exit_code == 0"
    binary = install_dir / "actionlint"
    assert binary.is_file(), "expected: binary.is_file()"
    assert os.access(binary, os.X_OK), "expected: os.access(binary, os.X_OK)"
    assert not (install_dir / "LICENSE").exists(), (
        "expected: not (install_dir / 'LICENSE').exists()"
    )
    assert f"installed actionlint {VERSION}" in capsys.readouterr().out, (
        "the install must be reported with its version"
    )


def test_warm_cache_reuses_a_binary_reporting_the_pinned_version(
    install_dir: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: pathlib.Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """A restored binary at the pin is kept and no download is attempted."""
    cached = write_executable(install_dir / "actionlint", f"echo '{VERSION}'")
    before = cached.read_bytes()
    monkeypatch.setenv("INPUT_RELEASE_ROOT", file_url(tmp_path / "absent"))
    monkeypatch.setenv("INPUT_ACTIONLINT_SHA256", "0" * 64)

    exit_code = invoke(script.app)

    assert exit_code == 0, "expected: exit_code == 0"
    assert cached.read_bytes() == before, "expected: cached.read_bytes() == before"
    assert "restored from the cache volume" in capsys.readouterr().out, (
        "expected: 'restored from the cache volume' in capsys.readouterr().out"
    )


def test_stale_cached_binary_is_replaced_by_the_pinned_release(
    install_dir: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: pathlib.Path,
) -> None:
    """A cached binary at another version is overwritten, not trusted."""
    write_executable(install_dir / "actionlint", "echo '1.6.0'")
    _publish_release(tmp_path, monkeypatch, {"actionlint": script_reporting(VERSION)})

    exit_code = invoke(script.app)

    assert exit_code == 0, "expected: exit_code == 0"
    assert script.reported_version(install_dir / "actionlint") == VERSION, (
        "expected: script.reported_version(install_dir / 'actionlint') == VE..."
    )


def test_checksum_mismatch_installs_nothing(
    install_dir: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: pathlib.Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """An archive that does not match its pin never reaches the install dir."""
    _publish_release(
        tmp_path,
        monkeypatch,
        {"actionlint": script_reporting(VERSION)},
        pin_matches=False,
    )

    exit_code = invoke(script.app)

    assert exit_code == 1, "expected: exit_code == 1"
    assert not (install_dir / "actionlint").exists(), (
        "expected: not (install_dir / 'actionlint').exists()"
    )
    assert "does not match its pinned SHA-256" in capsys.readouterr().err, (
        "expected: 'does not match its pinned SHA-256' in capsys.readouterr(..."
    )


def test_archive_without_the_binary_is_rejected(
    install_dir: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: pathlib.Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """A verified archive still has to contain the one member we extract."""
    _publish_release(tmp_path, monkeypatch, {"README": b"nothing here"})

    exit_code = invoke(script.app)

    assert exit_code == 1, "expected: exit_code == 1"
    assert "has no member 'actionlint'" in capsys.readouterr().err, (
        "expected: 'has no member 'actionlint'' in capsys.readouterr().err"
    )


def test_installed_binary_reporting_the_wrong_version_fails(
    install_dir: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: pathlib.Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """The post-install version probe guards against a mislabelled release."""
    _publish_release(tmp_path, monkeypatch, {"actionlint": script_reporting("9.9.9")})

    exit_code = invoke(script.app)

    assert exit_code == 1, "expected: exit_code == 1"
    err = capsys.readouterr().err
    assert "reports '9.9.9'" in err, "expected: 'reports '9.9.9'' in err"
    assert f"expected '{VERSION}'" in err, "the message must name the pinned version"


def test_missing_pins_are_reported_before_anything_runs(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: pathlib.Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Without the version pin the app fails at parameter parsing, exit 1."""
    monkeypatch.setenv("INPUT_INSTALL_DIR", str(tmp_path))
    monkeypatch.delenv("INPUT_ACTIONLINT_VERSION", raising=False)
    monkeypatch.setenv("INPUT_ACTIONLINT_SHA256", "0" * 64)

    with pytest.raises(SystemExit) as caught:
        script.app([])

    assert caught.value.code == 1, "expected: caught.value.code == 1"
    assert "actionlint-version" in capsys.readouterr().err, (
        "expected: 'actionlint-version' in capsys.readouterr().err"
    )
    assert not (tmp_path / "actionlint").exists(), (
        "expected: not (tmp_path / 'actionlint').exists()"
    )


def test_archive_url_follows_the_release_layout() -> None:
    """The URL is the versioned release path with the platform suffix."""
    url = script.archive_url("https://example.invalid/dl", "1.2.3", "linux_amd64")

    assert url == (
        "https://example.invalid/dl/v1.2.3/actionlint_1.2.3_linux_amd64.tar.gz"
    ), url
