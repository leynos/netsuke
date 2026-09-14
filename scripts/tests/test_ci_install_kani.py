"""Behavioural tests for ``scripts/ci/install_kani.py``."""

import dataclasses
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

    from cmd_mox import CmdMox

script = load_ci_script("install_kani")

VERSION = "0.67.0"
TARGET = "x86_64-unknown-linux-gnu"


class Homes(typ.NamedTuple):
    """The job's home directories and the expected payload locations."""

    cargo_home: pathlib.Path
    kani_home: pathlib.Path
    rustup_home: pathlib.Path
    github_path: pathlib.Path

    @property
    def frontend_bin(self) -> pathlib.Path:
        """The version-qualified front-end directory."""
        return self.cargo_home / "frontend" / f"kani-{VERSION}"

    @property
    def driver(self) -> pathlib.Path:
        """The verifier driver the bundle install produces."""
        return self.kani_home / f"kani-{VERSION}" / "bin" / "kani-driver"


@dataclasses.dataclass(frozen=True, slots=True)
class Harness:
    """The job's directories plus the means to publish release fixtures."""

    homes: Homes
    tmp_path: pathlib.Path
    monkeypatch: pytest.MonkeyPatch

    def publish(
        self, *, frontend_ok: bool = True, bundle_ok: bool = True
    ) -> tuple[pathlib.Path, pathlib.Path]:
        """Serve both archives under their real URL layouts and pin their digests."""
        quick = self.tmp_path / "quickinstall"
        upstream = self.tmp_path / "upstream"
        frontend = write_tarball(
            quick
            / "releases"
            / "download"
            / f"kani-verifier-{VERSION}"
            / f"kani-verifier-{VERSION}-{TARGET}.tar.gz",
            {
                "cargo-kani": script_reporting("cargo-kani"),
                "kani": script_reporting("kani"),
            },
        )
        bundle = write_tarball(
            upstream
            / "releases"
            / "download"
            / f"kani-{VERSION}"
            / f"kani-{VERSION}-{TARGET}.tar.gz",
            {"bin/kani-driver": script_reporting("driver")},
        )
        self.monkeypatch.setenv("INPUT_QUICKINSTALL_ROOT", file_url(quick))
        self.monkeypatch.setenv("INPUT_UPSTREAM_ROOT", file_url(upstream))
        self.monkeypatch.setenv(
            "INPUT_FRONTEND_SHA256",
            hashlib.sha256(frontend.read_bytes()).hexdigest()
            if frontend_ok
            else "0" * 64,
        )
        self.monkeypatch.setenv(
            "INPUT_BUNDLE_SHA256",
            hashlib.sha256(bundle.read_bytes()).hexdigest() if bundle_ok else "0" * 64,
        )
        return frontend, bundle

    def restore_everything(self) -> None:
        """Pretend the cache restored both payloads and point the roots nowhere."""
        for name in ("cargo-kani", "kani"):
            write_executable(self.homes.frontend_bin / name, "true")
        write_executable(self.homes.driver, "true")
        absent = file_url(self.tmp_path / "absent")
        self.monkeypatch.setenv("INPUT_QUICKINSTALL_ROOT", absent)
        self.monkeypatch.setenv("INPUT_UPSTREAM_ROOT", absent)
        self.monkeypatch.setenv("INPUT_FRONTEND_SHA256", "0" * 64)
        self.monkeypatch.setenv("INPUT_BUNDLE_SHA256", "0" * 64)


@pytest.fixture
def kani(monkeypatch: pytest.MonkeyPatch, tmp_path: pathlib.Path) -> Harness:
    """Provide the ambient job variables, a pinned version file, and fixtures."""
    homes = Homes(
        tmp_path / "cargo",
        tmp_path / "kani",
        tmp_path / "rustup",
        tmp_path / "github-path",
    )
    homes.github_path.touch()
    version_file = tmp_path / "VERSION"
    version_file.write_text(f"{VERSION}\n", encoding="utf-8")
    monkeypatch.setenv("CARGO_HOME", str(homes.cargo_home))
    monkeypatch.setenv("KANI_HOME", str(homes.kani_home))
    monkeypatch.setenv("RUSTUP_HOME", str(homes.rustup_home))
    monkeypatch.setenv("RUNNER_TEMP", str(tmp_path))
    monkeypatch.setenv("GITHUB_PATH", str(homes.github_path))
    monkeypatch.setenv("INPUT_KANI_VERSION", VERSION)
    monkeypatch.setenv("INPUT_VERSION_FILE", str(version_file))
    monkeypatch.setenv("PATH", os.environ["PATH"])
    return Harness(homes, tmp_path, monkeypatch)


def _mock_setup(cmd_mox: CmdMox, bundle: pathlib.Path, *, exit_code: int = 0) -> None:
    cmd_mox.mock("cargo").with_args(
        "kani", "setup", "--use-local-bundle", str(bundle)
    ).returns(exit_code=exit_code)


def test_cold_cache_installs_both_payloads(cmd_mox: CmdMox, kani: Harness) -> None:
    """Front-end binaries are extracted and the bundle is handed to cargo kani."""
    _, bundle = kani.publish()
    _mock_setup(cmd_mox, kani.tmp_path / bundle.name)

    exit_code = invoke(script.app)

    assert exit_code == 0, "expected: exit_code == 0"
    for name in ("cargo-kani", "kani"):
        assert os.access(kani.homes.frontend_bin / name, os.X_OK), (
            "expected: os.access(kani.homes.frontend_bin / name, os.X_OK)"
        )
    assert kani.homes.rustup_home.is_dir(), "expected: kani.homes.rustup_home.is_dir()"
    assert kani.homes.github_path.read_text(encoding="utf-8") == (
        f"{kani.homes.frontend_bin}\n"
    ), "the front-end directory must be published through GITHUB_PATH exactly once"
    assert os.environ["PATH"].split(os.pathsep)[0] == str(kani.homes.frontend_bin), (
        "expected: os.environ['PATH'].split(os.pathsep)[0] == str(kani.homes..."
    )


def test_warm_cache_downloads_nothing_and_skips_setup(
    cmd_mox: CmdMox, kani: Harness, capsys: pytest.CaptureFixture[str]
) -> None:
    """Restored payloads are detected by their executables; no network, no cargo."""
    kani.restore_everything()
    cargo = cmd_mox.spy("cargo").returns(exit_code=0)

    exit_code = invoke(script.app)

    assert exit_code == 0, "expected: exit_code == 0"
    cargo.assert_not_called()
    out = capsys.readouterr().out
    assert "front-end 0.67.0 present" in out, (
        "expected: 'front-end 0.67.0 present' in out"
    )
    assert "verifier bundle 0.67.0 present" in out, (
        "expected: 'verifier bundle 0.67.0 present' in out"
    )
    assert kani.homes.github_path.read_text(encoding="utf-8") == (
        f"{kani.homes.frontend_bin}\n"
    ), "the front-end directory must be published through GITHUB_PATH exactly once"


def test_version_file_disagreeing_with_the_pin_stops_everything(
    cmd_mox: CmdMox, kani: Harness, capsys: pytest.CaptureFixture[str]
) -> None:
    """tools/kani/VERSION and the workflow pin must agree before any download."""
    kani.publish()
    kani.monkeypatch.setenv("INPUT_KANI_VERSION", "0.66.0")
    cargo = cmd_mox.spy("cargo").returns(exit_code=0)

    exit_code = invoke(script.app)

    assert exit_code == 1, "expected: exit_code == 1"
    cargo.assert_not_called()
    err = capsys.readouterr().err
    assert "pins Kani '0.67.0' but the workflow passed '0.66.0'" in err, err
    assert not kani.homes.cargo_home.exists(), (
        "expected: not kani.homes.cargo_home.exists()"
    )


def test_front_end_checksum_mismatch_extracts_nothing(
    cmd_mox: CmdMox, kani: Harness, capsys: pytest.CaptureFixture[str]
) -> None:
    """The front-end archive is verified before a single member is extracted."""
    kani.publish(frontend_ok=False)
    cargo = cmd_mox.spy("cargo").returns(exit_code=0)

    exit_code = invoke(script.app)

    assert exit_code == 1, "expected: exit_code == 1"
    cargo.assert_not_called()
    assert not (kani.homes.frontend_bin / "cargo-kani").exists(), (
        "expected: not (kani.homes.frontend_bin / 'cargo-kani').exists()"
    )
    assert "does not match its pinned SHA-256" in capsys.readouterr().err, (
        "expected: 'does not match its pinned SHA-256' in capsys.readouterr(..."
    )


def test_bundle_checksum_mismatch_never_reaches_cargo_kani_setup(
    cmd_mox: CmdMox, kani: Harness
) -> None:
    """The bundle is verified before it is handed to the installer."""
    kani.publish(bundle_ok=False)
    cargo = cmd_mox.spy("cargo").returns(exit_code=0)

    exit_code = invoke(script.app)

    assert exit_code == 1, "expected: exit_code == 1"
    cargo.assert_not_called()
    assert os.access(kani.homes.frontend_bin / "kani", os.X_OK), (
        "the front-end still installs"
    )


def test_failed_bundle_setup_is_reported(
    cmd_mox: CmdMox, kani: Harness, capsys: pytest.CaptureFixture[str]
) -> None:
    """A non-zero cargo kani setup fails the step with its exit code."""
    _, bundle = kani.publish()
    _mock_setup(cmd_mox, kani.tmp_path / bundle.name, exit_code=7)

    exit_code = invoke(script.app)

    assert exit_code == 1, "expected: exit_code == 1"
    assert "installing the verifier bundle failed" in capsys.readouterr().err, (
        "expected: 'installing the verifier bundle failed' in capsys.readout..."
    )


def test_urls_follow_the_release_layouts() -> None:
    """Both archive URLs are the versioned release paths for the target."""
    assert script.frontend_url("https://q.invalid", "1.0.0", "t") == (
        "https://q.invalid/releases/download/kani-verifier-1.0.0/kani-verifier-1.0.0-t.tar.gz"
    ), "expected: script.frontend_url('https://q.invalid', '1.0.0', 't') ==..."
    assert script.bundle_url("https://u.invalid", "1.0.0", "t") == (
        "https://u.invalid/releases/download/kani-1.0.0/kani-1.0.0-t.tar.gz"
    ), "expected: script.bundle_url('https://u.invalid', '1.0.0', 't') == (..."
