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
    assert script.probe_version(install_dir / "actionlint").version == VERSION, (
        "expected: the replaced binary to report the pinned version"
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
    # The group is the parameter name, so the message names the field beneath it.
    assert "--release.version" in capsys.readouterr().err, (
        "expected: '--release.version' in capsys.readouterr().err"
    )
    assert not (tmp_path / "actionlint").exists(), (
        "expected: not (tmp_path / 'actionlint').exists()"
    )


def test_a_missing_install_directory_is_created_on_extraction(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: pathlib.Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """``INPUT_INSTALL_DIR`` may name a directory that does not exist yet.

    The workflow uses the existing working directory, so this only bites when
    the input overrides it. Extraction writes into the destination rather than
    creating it, so without the mkdir the cold path fails with a
    ``FileNotFoundError`` from deep inside the archive write, naming neither
    the input nor the directory it was told to use.
    """
    install_dir = tmp_path / "workspace" / "nested"
    monkeypatch.setenv("INPUT_INSTALL_DIR", str(install_dir))
    monkeypatch.setenv("INPUT_ACTIONLINT_VERSION", VERSION)
    monkeypatch.setenv("RUNNER_TEMP", str(tmp_path))
    _publish_release(tmp_path, monkeypatch, {"actionlint": script_reporting(VERSION)})

    exit_code = invoke(script.app)

    assert exit_code == 0, "expected: exit_code == 0"
    binary = install_dir / "actionlint"
    assert binary.is_file(), "the binary must land in the directory that was made"
    assert os.access(binary, os.X_OK), "expected: os.access(binary, os.X_OK)"
    assert f"installed actionlint {VERSION}" in capsys.readouterr().out, (
        "the install must be reported with its version"
    )


def test_a_cached_binary_that_cannot_report_a_version_fails_the_step(
    install_dir: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: pathlib.Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """A cached binary that runs but cannot answer is a failure, not a miss.

    A reinstall would replace it and the step would pass, so a broken cache
    would read as a healthy one and the entry would stay broken until the
    version pin changed. The failure has to surface here, where the diagnostic
    can name the binary and its exit status. ``INPUT_RELEASE_ROOT`` points at
    a directory with no releases, so a step that treated this as a cache miss
    would fail on the download instead and the assertions below would catch
    the wrong failure.
    """
    write_executable(install_dir / "actionlint", "exit 7")
    monkeypatch.setenv("INPUT_RELEASE_ROOT", file_url(tmp_path / "absent"))
    monkeypatch.setenv("INPUT_ACTIONLINT_SHA256", "0" * 64)

    exit_code = invoke(script.app)

    assert exit_code == 1, "expected: exit_code == 1"
    err = capsys.readouterr().err
    assert "the cached actionlint cannot be used" in err, (
        "expected: the diagnostic to name the unusable cache entry"
    )
    assert "exited 7" in err, "expected: the probe's exit status in the diagnostic"
    assert "does not match its pinned SHA-256" not in err, (
        "the step must fail at the cache probe, not at a download it should "
        "never have attempted"
    )


def test_a_cached_binary_printing_nothing_fails_the_step(
    install_dir: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: pathlib.Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """A probe that exits 0 while printing nothing is still not an answer.

    This is the other half of the same failure: the exit status is clean, so
    only the empty output marks the binary as unusable. Without that check the
    empty string would be compared against the pin, read as a version
    mismatch, and the binary would be replaced.
    """
    write_executable(install_dir / "actionlint", "true")
    monkeypatch.setenv("INPUT_RELEASE_ROOT", file_url(tmp_path / "absent"))
    monkeypatch.setenv("INPUT_ACTIONLINT_SHA256", "0" * 64)

    exit_code = invoke(script.app)

    assert exit_code == 1, "expected: exit_code == 1"
    err = capsys.readouterr().err
    assert "the cached actionlint cannot be used" in err, (
        "expected: the diagnostic to name the unusable cache entry"
    )
    assert "exited 0" in err, "expected: the probe's exit status in the diagnostic"


def test_a_failed_checksum_leaves_no_install_directory_behind(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: pathlib.Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """The install directory is made after verification, not before it.

    Creating it first would leave an empty directory behind on a checksum
    mismatch, which reads as a partially completed install to anyone looking
    at the cache volume afterwards.
    """
    install_dir = tmp_path / "workspace" / "nested"
    monkeypatch.setenv("INPUT_INSTALL_DIR", str(install_dir))
    monkeypatch.setenv("INPUT_ACTIONLINT_VERSION", VERSION)
    monkeypatch.setenv("RUNNER_TEMP", str(tmp_path))
    _publish_release(
        tmp_path,
        monkeypatch,
        {"actionlint": script_reporting(VERSION)},
        pin_matches=False,
    )

    exit_code = invoke(script.app)

    assert exit_code == 1, "expected: exit_code == 1"
    assert not install_dir.exists(), (
        "a rejected archive must not leave an install directory behind"
    )
    assert "does not match its pinned SHA-256" in capsys.readouterr().err, (
        "expected: the checksum failure to be reported"
    )


def test_archive_url_follows_the_release_layout() -> None:
    """The URL is the versioned release path with the platform suffix."""
    release = script.Release(
        version="1.2.3",
        sha256="0" * 64,
        release_root="https://example.invalid/dl",
        platform="linux_amd64",
    )

    assert release.url == (
        "https://example.invalid/dl/v1.2.3/actionlint_1.2.3_linux_amd64.tar.gz"
    ), release.url
