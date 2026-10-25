"""Test actionlint cache reuse and unusable cached binaries."""

import typing as typ

import pytest
from ci_script_support import file_url, invoke, load_ci_script, write_executable

if typ.TYPE_CHECKING:
    import pathlib

script = load_ci_script("install_actionlint")
VERSION = "1.7.12"


@pytest.fixture
def install_dir(
    monkeypatch: pytest.MonkeyPatch, tmp_path: pathlib.Path
) -> pathlib.Path:
    """Provide an actionlint install directory and its required pins."""
    install_dir = tmp_path / "workspace"
    install_dir.mkdir()
    monkeypatch.setenv("INPUT_INSTALL_DIR", str(install_dir))
    monkeypatch.setenv("INPUT_ACTIONLINT_VERSION", VERSION)
    monkeypatch.setenv("RUNNER_TEMP", str(tmp_path))
    return install_dir


@pytest.fixture
def no_releases_available(
    monkeypatch: pytest.MonkeyPatch, tmp_path: pathlib.Path
) -> None:
    """Make every download impossible, so only the cache probe can decide."""
    monkeypatch.setenv("INPUT_RELEASE_ROOT", file_url(tmp_path / "absent"))
    monkeypatch.setenv("INPUT_ACTIONLINT_SHA256", "0" * 64)


def test_warm_cache_reuses_a_binary_reporting_the_pinned_version(
    install_dir: pathlib.Path,
    no_releases_available: None,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Keep a restored binary at the pin without attempting a download."""
    cached = write_executable(install_dir / "actionlint", f"echo '{VERSION}'")
    before = cached.read_bytes()

    exit_code = invoke(script.app)

    assert exit_code == 0, "expected: exit_code == 0"
    assert cached.read_bytes() == before, "expected: cached.read_bytes() == before"
    assert "restored from the cache volume" in capsys.readouterr().out, (
        "expected: 'restored from the cache volume' in capsys.readouterr().out"
    )


def test_cached_symlink_is_rejected_without_probing_its_target(
    install_dir: pathlib.Path,
    no_releases_available: None,
    tmp_path: pathlib.Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Never reuse a cache entry that redirects the install path elsewhere."""
    outside = write_executable(tmp_path / "outside" / "actionlint", f"echo '{VERSION}'")
    outside_before = outside.read_bytes()
    cached = install_dir / "actionlint"
    cached.symlink_to(outside)

    exit_code = invoke(script.app)

    assert exit_code == 1, "expected: exit_code == 1"
    err = capsys.readouterr().err
    assert "symbolic link" in err, "expected the diagnostic to reject the link"
    assert cached.is_symlink(), "expected the cache symlink to remain untouched"
    assert outside.read_bytes() == outside_before, (
        "expected the external executable to remain untouched"
    )
    assert "does not match its pinned SHA-256" not in err, (
        "expected rejection before attempting a release download"
    )


def test_a_cached_binary_that_cannot_report_a_version_fails_the_step(
    install_dir: pathlib.Path,
    no_releases_available: None,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Treat a cached binary that exits non-zero as broken, not as a cache miss."""
    write_executable(install_dir / "actionlint", "exit 7")

    exit_code = invoke(script.app)

    assert exit_code == 1, "expected: exit_code == 1"
    err = capsys.readouterr().err
    missing = [
        fragment
        for fragment in ("the cached actionlint cannot be used", "exited 7")
        if fragment not in err
    ]
    assert not missing, f"the cache diagnostic is missing {missing}; got: {err!r}"
    assert "does not match its pinned SHA-256" not in err, (
        "the step must fail at the cache probe, not at a download it should "
        "never have attempted"
    )


UNUSABLE_CACHE_FRAGMENTS = (
    "the cached actionlint cannot be used",
    "exited 0",
    "printed no version",
)
"""Identify the cache diagnostic and the reason a clean probe failed."""


def _assert_unusable_cache_diagnostic(err: str) -> None:
    """Assert that ``err`` reports an unusable cache entry and its reason."""
    missing = [fragment for fragment in UNUSABLE_CACHE_FRAGMENTS if fragment not in err]
    assert not missing, f"the cache diagnostic is missing {missing}; got: {err!r}"


def test_a_cached_binary_printing_nothing_fails_the_step(
    install_dir: pathlib.Path,
    no_releases_available: None,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Treat a clean probe with no output as an unusable cached binary."""
    write_executable(install_dir / "actionlint", "true")

    exit_code = invoke(script.app)

    assert exit_code == 1, "expected: exit_code == 1"
    _assert_unusable_cache_diagnostic(capsys.readouterr().err)


def test_a_cached_binary_with_a_blank_first_line_does_not_skip_to_the_next(
    install_dir: pathlib.Path,
    no_releases_available: None,
    capsys: pytest.CaptureFixture[str],
) -> None:
    r"""Keep the cache probe tied to actionlint's first output line."""
    binary = write_executable(install_dir / "actionlint", rf"printf '\n{VERSION}\n'")
    before = binary.read_bytes()

    exit_code = invoke(script.app)

    assert exit_code == 1, "expected: exit_code == 1"
    err = capsys.readouterr().err
    _assert_unusable_cache_diagnostic(err)
    assert "does not match its pinned SHA-256" not in err, (
        "the cache probe must reject the blank first line before downloading"
    )
    assert binary.read_bytes() == before, (
        "the cached executable must be left exactly as it was found"
    )


@pytest.mark.parametrize(
    "body",
    [r"printf '   \n'", r"printf '\t\n'", r"printf '\n'"],
    ids=["spaces", "tabs", "newline-only"],
)
def test_a_cached_binary_printing_only_whitespace_fails_the_step(
    body: str,
    install_dir: pathlib.Path,
    no_releases_available: None,
    capsys: pytest.CaptureFixture[str],
) -> None:
    r"""Reject output that normalizes to an empty version string.

    Whitespace is truthy, so a check on the raw stream would accept it before
    stripping it to the empty string. With no matching pin, that path would
    silently replace the broken cached executable. The impossible release and
    byte-for-byte comparison ensure this fails at the probe and preserves the
    cache entry.
    """
    binary = write_executable(install_dir / "actionlint", body)
    before = binary.read_bytes()

    exit_code = invoke(script.app)

    assert exit_code == 1, "expected: exit_code == 1"
    err = capsys.readouterr().err
    _assert_unusable_cache_diagnostic(err)
    assert "does not match its pinned SHA-256" not in err, (
        "the step must fail at the cache probe, not at a download it should "
        "never have attempted"
    )
    assert binary.read_bytes() == before, (
        "the cached executable must be left exactly as it was found"
    )
