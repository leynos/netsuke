"""Exercise the helpers shared by the CI helper scripts."""

import hashlib
import os
import typing as typ

import pytest
from ci_script_support import file_url, load_ci_script, write_executable, write_tarball

if typ.TYPE_CHECKING:
    import pathlib

    from cmd_mox import CmdMox

support = load_ci_script("ci_support")


def test_run_returns_captured_output_for_an_allowlisted_program(
    cmd_mox: CmdMox,
) -> None:
    """A mocked program's stdout and exit code come back on the result."""
    cmd_mox.mock("frob").with_args("--flag", "value").returns(stdout="done\n")

    result = support.run(
        "frob", "--flag", "value", allowed=support.catalogue("frob"), echo=False
    )

    assert result.ok, result
    assert result.stdout == "done\n", "expected: result.stdout == 'done\n'"


def test_run_refuses_a_program_outside_the_catalogue() -> None:
    """The catalogue is the allowlist: an unlisted program never spawns."""
    from cuprum import UnknownProgramError

    with pytest.raises(UnknownProgramError):
        support.run("frob", allowed=support.catalogue("other"), echo=False)


def test_run_overlays_env_on_the_child(cmd_mox: CmdMox) -> None:
    """A per-run env overlay reaches the child without touching os.environ."""
    seen: list[str | None] = []
    cmd_mox.mock("frob").runs(
        lambda inv: (seen.append(inv.env.get("CI_PROBE")) or "", "", 0)
    )

    support.run(
        "frob", allowed=support.catalogue("frob"), env={"CI_PROBE": "yes"}, echo=False
    )

    assert seen == ["yes"], "expected: seen == ['yes']"
    assert "CI_PROBE" not in os.environ, "expected: 'CI_PROBE' not in os.environ"


def test_describe_failure_names_the_command_and_last_stderr_line(
    cmd_mox: CmdMox,
) -> None:
    """Failure text carries the argv, exit code, and the final stderr line."""
    cmd_mox.mock("frob").with_args("x").returns(
        stderr="first\nlast line\n", exit_code=3
    )

    result = support.run("frob", "x", allowed=support.catalogue("frob"), echo=False)

    assert support.describe_failure(result) == "`frob x` exited 3: last line", (
        "expected: support.describe_failure(result) == '`frob x` exited 3: l..."
    )


def test_append_github_path_appends_one_line(tmp_path: pathlib.Path) -> None:
    """Each published directory is one line, appended after existing content."""
    github_path = tmp_path / "path"
    github_path.write_text("/existing\n", encoding="utf-8")

    support.append_github_path(tmp_path / "bin", github_path)

    assert (
        github_path.read_text(encoding="utf-8") == f"/existing\n{tmp_path / 'bin'}\n"
    ), "expected: github_path.read_text(encoding='utf-8') == f'/existing\n{..."


def test_prepend_path_puts_the_directory_first(
    monkeypatch: pytest.MonkeyPatch, tmp_path: pathlib.Path
) -> None:
    """The process PATH gains the directory ahead of everything else."""
    monkeypatch.setenv("PATH", "/usr/bin")

    support.prepend_path(tmp_path)

    assert os.environ["PATH"] == f"{tmp_path}{os.pathsep}/usr/bin", (
        "the directory must be first on PATH, ahead of the existing entries"
    )


def test_is_executable_file_rejects_directories_and_plain_files(
    tmp_path: pathlib.Path,
) -> None:
    """Only a regular file with the execute bit counts."""
    plain = tmp_path / "plain"
    plain.write_text("x", encoding="utf-8")
    executable = write_executable(tmp_path / "exe", "true")

    assert not support.is_executable_file(tmp_path), (
        "expected: not support.is_executable_file(tmp_path)"
    )
    assert not support.is_executable_file(plain), (
        "expected: not support.is_executable_file(plain)"
    )
    assert not support.is_executable_file(tmp_path / "missing"), (
        "expected: not support.is_executable_file(tmp_path / 'missing')"
    )
    assert support.is_executable_file(executable), (
        "expected: support.is_executable_file(executable)"
    )


def test_download_copies_a_file_url(tmp_path: pathlib.Path) -> None:
    """A ``file://`` fixture is fetched through the real download path."""
    source = tmp_path / "source.bin"
    source.write_bytes(b"payload")

    support.download(file_url(source), tmp_path / "dest.bin")

    assert (tmp_path / "dest.bin").read_bytes() == b"payload", (
        "expected: (tmp_path / 'dest.bin').read_bytes() == b'payload'"
    )


@pytest.mark.parametrize("url", ["http://example.invalid/x", "ftp://example.invalid/x"])
def test_download_refuses_insecure_schemes(url: str, tmp_path: pathlib.Path) -> None:
    """Only https and file URLs are ever opened."""
    with pytest.raises(support.CiScriptError, match="URLs are permitted"):
        support.download(url, tmp_path / "dest.bin")
    assert not (tmp_path / "dest.bin").exists(), (
        "expected: not (tmp_path / 'dest.bin').exists()"
    )


def test_download_reports_a_missing_source(tmp_path: pathlib.Path) -> None:
    """A failed fetch is an actionable error naming the URL."""
    url = file_url(tmp_path / "absent.bin")
    with pytest.raises(support.CiScriptError, match=r"downloading .* failed"):
        support.download(url, tmp_path / "dest.bin")


def test_verify_sha256_accepts_the_matching_digest(tmp_path: pathlib.Path) -> None:
    """A matching pin, in either case, passes silently."""
    archive = tmp_path / "a.tar.gz"
    archive.write_bytes(b"archive")
    digest = hashlib.sha256(b"archive").hexdigest()

    support.verify_sha256(archive, digest)
    support.verify_sha256(archive, digest.upper())


def test_verify_sha256_names_both_digests_on_mismatch(tmp_path: pathlib.Path) -> None:
    """The failure states the pinned and actual digests so drift is diagnosable."""
    archive = tmp_path / "a.tar.gz"
    archive.write_bytes(b"archive")
    actual = hashlib.sha256(b"archive").hexdigest()

    with pytest.raises(support.CiScriptError) as caught:
        support.verify_sha256(archive, "0" * 64)
    message = str(caught.value)
    assert "0" * 64 in message, "expected: '0' * 64 in message"
    assert actual in message, "expected: actual in message"
    assert "ci.yml" in message, "expected: 'ci.yml' in message"


def test_extract_members_writes_named_files_as_executables(
    tmp_path: pathlib.Path,
) -> None:
    """Named members land flat under the destination with the execute bit set."""
    archive = write_tarball(
        tmp_path / "a.tar.gz",
        {"tool": b"#!/bin/sh\necho tool\n", "nested/other": b"other", "README": b"doc"},
    )
    destination = tmp_path / "bin"
    destination.mkdir()

    extracted = support.extract_members(archive, destination, ("tool", "nested/other"))

    assert extracted == [destination / "tool", destination / "other"], (
        "expected: extracted == [destination / 'tool', destination / 'other']"
    )
    assert (destination / "tool").read_bytes().startswith(b"#!/bin/sh"), (
        "expected: (destination / 'tool').read_bytes().startswith(b'#!/bin/sh')"
    )
    assert os.access(destination / "tool", os.X_OK), (
        "expected: os.access(destination / 'tool', os.X_OK)"
    )
    assert not (destination / "README").exists(), (
        "expected: not (destination / 'README').exists()"
    )


def test_extract_members_refuses_a_missing_member(tmp_path: pathlib.Path) -> None:
    """An archive without the expected member is rejected by name."""
    archive = write_tarball(tmp_path / "a.tar.gz", {"README": b"doc"})

    with pytest.raises(support.CiScriptError, match="has no member 'tool'"):
        support.extract_members(archive, tmp_path, ("tool",))


def test_extract_members_refuses_a_symlink_member(tmp_path: pathlib.Path) -> None:
    """A link is never followed or written, so an archive cannot escape."""
    archive = write_tarball(
        tmp_path / "a.tar.gz", {"README": b"doc"}, symlinks={"tool": "/etc/passwd"}
    )

    with pytest.raises(support.CiScriptError, match="is not a regular file"):
        support.extract_members(archive, tmp_path, ("tool",))
    assert not (tmp_path / "tool").exists(), (
        "expected: not (tmp_path / 'tool').exists()"
    )
