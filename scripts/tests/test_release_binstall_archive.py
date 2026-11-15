"""Test reading, verifying, extracting, and probing a downloaded archive.

Archives are built in memory and written to a temporary directory, and the
probe is driven through an injected runner, so no test here reaches the
network or executes a downloaded file. One case launches a shell script the
test itself writes, because decoding of a child's streams belongs to
``subprocess`` and an injected runner cannot reproduce it.

Three adapters share the work: :mod:`release_binstall_archive` reads and
verifies the bytes, :mod:`release_binstall_extract` writes the binary out, and
:mod:`release_binstall_probe` launches it. The layout decisions they rely on
live in :mod:`release_consumer_archive_policy`.

Run via ``make test-release-publication``.
"""

import io
import subprocess  # ruff: ignore[suspicious-subprocess-import] - the seam under test.
import sys
import tarfile
import typing as typ

import pytest
import release_binstall_archive as adapter
import release_binstall_extract as extract
import release_binstall_probe as probe
import release_consumer_archive_policy as policy
import release_digest_files

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    import pathlib

BINARY_BYTES = b"netsuke binary payload"
SIDECAR_NAME = "netsuke-build-1.2.3-x86_64-unknown-linux-gnu.tar.gz.sha256"


def _tarball(
    path: pathlib.Path,
    *,
    binary: bytes = BINARY_BYTES,
    extra: cabc.Sequence[tuple[str, bytes]] = (),
    kind: policy.MemberKind | None = None,
) -> pathlib.Path:
    """Write a gzip-compressed tar archive holding a binary and extras.

    Returns
    -------
    pathlib.Path
        The archive's path, once written.
    """
    with tarfile.open(path, "w:gz") as archive:
        entry = tarfile.TarInfo(policy.BINARY_NAME)
        entry.size = len(binary)
        if kind is policy.MemberKind.SYMLINK:
            entry.type = tarfile.SYMTYPE
            entry.linkname = "/etc/passwd"
            entry.size = 0
        _ = archive.addfile(entry, io.BytesIO(binary))
        for name, payload in extra:
            other = tarfile.TarInfo(name)
            other.size = len(payload)
            _ = archive.addfile(other, io.BytesIO(payload))
    return path


def _digest(path: pathlib.Path) -> str:
    """Return the archive's digest as the trusted record would hold it.

    Returns
    -------
    str
        The lower-case hexadecimal SHA-256 of the file.
    """
    return release_digest_files.digest_of_file(path)


def test_scan_archive_accepts_a_well_formed_archive(tmp_path: pathlib.Path) -> None:
    """Return the validated layout of an archive whose digest matches."""
    archive = _tarball(tmp_path / "archive.tar.gz", extra=[("LICENSE", b"MIT")])

    layout = adapter.scan_archive(archive, expected_sha256=_digest(archive))

    assert (layout.binary_name, layout.binary_size) == (
        policy.BINARY_NAME,
        len(BINARY_BYTES),
    ), "the scan must report the root binary and its declared size"
    assert layout.member_count == 2, "the scan must count every member"


def test_scan_archive_refuses_an_empty_file(tmp_path: pathlib.Path) -> None:
    """Refuse a download that carries no bytes."""
    archive = tmp_path / "empty.tar.gz"
    _ = archive.write_bytes(b"")

    with pytest.raises(adapter.ArchiveEmptyError):
        adapter.scan_archive(archive, expected_sha256=_digest(archive))


def test_scan_archive_refuses_an_oversized_file(tmp_path: pathlib.Path) -> None:
    """Refuse a file larger than the fixed read cap, without reading it."""
    archive = tmp_path / "huge.tar.gz"
    with archive.open("wb") as handle:
        handle.seek(adapter.MAX_ARCHIVE_BYTES)
        _ = handle.write(b"x")

    with pytest.raises(adapter.ArchiveFileTooLargeError):
        adapter.scan_archive(archive, expected_sha256=_digest(archive))


def test_scan_archive_refuses_a_different_digest(tmp_path: pathlib.Path) -> None:
    """Refuse bytes the trusted record does not name."""
    archive = _tarball(tmp_path / "archive.tar.gz")

    with pytest.raises(adapter.ArchiveDigestMismatchError):
        adapter.scan_archive(archive, expected_sha256="ab" * 32)


def test_scan_archive_refuses_bytes_that_are_not_a_tarball(
    tmp_path: pathlib.Path,
) -> None:
    """Translate a gzip or tar failure into one corrupt-archive refusal."""
    archive = tmp_path / "archive.tar.gz"
    _ = archive.write_bytes(b"not a gzip stream at all")

    with pytest.raises(adapter.ArchiveCorruptError):
        adapter.scan_archive(archive, expected_sha256=_digest(archive))


def test_scan_archive_refuses_a_truncated_archive(tmp_path: pathlib.Path) -> None:
    """Refuse a stream that ends before the archive does."""
    archive = _tarball(tmp_path / "archive.tar.gz")
    truncated = tmp_path / "truncated.tar.gz"
    _ = truncated.write_bytes(archive.read_bytes()[: len(archive.read_bytes()) // 2])

    with pytest.raises(adapter.ArchiveCorruptError):
        adapter.scan_archive(truncated, expected_sha256=_digest(truncated))


def test_scan_archive_refuses_a_link_member(tmp_path: pathlib.Path) -> None:
    """Refuse an archive whose binary member is a symlink."""
    archive = _tarball(tmp_path / "archive.tar.gz", kind=policy.MemberKind.SYMLINK)

    with pytest.raises(policy.UnsafeMemberTypeError):
        adapter.scan_archive(archive, expected_sha256=_digest(archive))


def test_verify_sidecar_accepts_a_matching_checksum(tmp_path: pathlib.Path) -> None:
    """Return the sidecar's digest when it describes the archive."""
    archive = _tarball(tmp_path / "archive.tar.gz")
    sidecar = tmp_path / SIDECAR_NAME
    digest = _digest(archive)
    _ = sidecar.write_text(f"{digest}  {archive.name}\n", encoding="utf-8")

    assert adapter.verify_sidecar(archive, sidecar) == digest, (
        "a matching sidecar must return the archive's digest"
    )


@pytest.mark.parametrize(
    "text",
    [
        "",
        "not a checksum\n",
        "ab" * 32 + "  another-asset.tar.gz\n",
        "cd" * 32 + "  archive.tar.gz\n",
    ],
    ids=["empty", "malformed", "other-name", "other-digest"],
)
def test_verify_sidecar_refuses_a_sidecar_that_does_not_describe_the_archive(
    tmp_path: pathlib.Path, text: str
) -> None:
    """Refuse a sidecar that is empty, malformed, or names other bytes."""
    archive = _tarball(tmp_path / "archive.tar.gz")
    sidecar = tmp_path / SIDECAR_NAME
    _ = sidecar.write_text(text, encoding="utf-8")

    with pytest.raises(adapter.SidecarVerificationError):
        adapter.verify_sidecar(archive, sidecar)


def test_verify_sidecar_refuses_an_oversized_body(tmp_path: pathlib.Path) -> None:
    """Refuse a sidecar larger than one checksum line."""
    archive = _tarball(tmp_path / "archive.tar.gz")
    sidecar = tmp_path / SIDECAR_NAME
    _ = sidecar.write_bytes(b"a" * (adapter.MAX_SIDECAR_BYTES + 1))

    with pytest.raises(adapter.SidecarVerificationError):
        adapter.verify_sidecar(archive, sidecar)


def test_verify_sidecar_refuses_a_non_utf8_body(tmp_path: pathlib.Path) -> None:
    """Refuse bytes that are not text at all."""
    archive = _tarball(tmp_path / "archive.tar.gz")
    sidecar = tmp_path / SIDECAR_NAME
    _ = sidecar.write_bytes(b"\xff\xfe\x00")

    with pytest.raises(adapter.SidecarVerificationError):
        adapter.verify_sidecar(archive, sidecar)


def test_verify_sidecar_refuses_an_absent_sidecar(tmp_path: pathlib.Path) -> None:
    """Refuse a download that never arrived."""
    archive = _tarball(tmp_path / "archive.tar.gz")

    with pytest.raises(adapter.SidecarVerificationError):
        adapter.verify_sidecar(archive, tmp_path / SIDECAR_NAME)


def test_extract_binary_writes_only_the_root_binary(
    tmp_path: pathlib.Path,
) -> None:
    """Write the binary executable and leave every other member unwritten."""
    archive = _tarball(tmp_path / "archive.tar.gz", extra=[("LICENSE", b"MIT")])
    layout = adapter.scan_archive(archive, expected_sha256=_digest(archive))
    destination = tmp_path / "out"
    destination.mkdir()

    binary = extract.extract_binary(
        archive, layout, destination, expected_sha256=_digest(archive)
    )

    assert binary.read_bytes() == BINARY_BYTES, (
        "the extracted bytes must be the member's bytes"
    )
    assert binary.stat().st_mode & 0o777 == 0o755, (
        "the extracted binary must be executable"
    )
    assert [path.name for path in destination.iterdir()] == [policy.BINARY_NAME], (
        "only the root binary may be written"
    )


def test_extract_binary_refuses_a_destination_holding_a_symlink(
    tmp_path: pathlib.Path,
) -> None:
    """Refuse to write into a directory that already holds an entry."""
    archive = _tarball(tmp_path / "archive.tar.gz")
    layout = adapter.scan_archive(archive, expected_sha256=_digest(archive))
    destination = tmp_path / "out"
    destination.mkdir()
    elsewhere = tmp_path / "elsewhere"
    _ = elsewhere.write_bytes(b"untouched")
    (destination / policy.BINARY_NAME).symlink_to(elsewhere)

    with pytest.raises(adapter.ExtractionError):
        extract.extract_binary(
            archive, layout, destination, expected_sha256=_digest(archive)
        )

    assert elsewhere.read_bytes() == b"untouched", (
        "a planted symlink must not redirect the write"
    )


def test_extract_binary_refuses_a_missing_destination(tmp_path: pathlib.Path) -> None:
    """Refuse a destination the caller never created."""
    archive = _tarball(tmp_path / "archive.tar.gz")
    layout = adapter.scan_archive(archive, expected_sha256=_digest(archive))

    with pytest.raises(adapter.ExtractionError):
        extract.extract_binary(
            archive, layout, tmp_path / "absent", expected_sha256=_digest(archive)
        )


def test_extract_binary_refuses_a_layout_from_other_bytes(
    tmp_path: pathlib.Path,
) -> None:
    """Refuse an archive that changed after it was scanned."""
    archive = _tarball(tmp_path / "archive.tar.gz")
    layout = adapter.scan_archive(archive, expected_sha256=_digest(archive))
    stale = policy.ArchiveLayout(
        binary_name=layout.binary_name,
        binary_size=layout.binary_size + 1,
        member_count=layout.member_count,
    )
    destination = tmp_path / "out"
    destination.mkdir()

    with pytest.raises(adapter.ExtractionError):
        extract.extract_binary(
            archive, stale, destination, expected_sha256=_digest(archive)
        )

    assert not list(destination.iterdir()), (
        "a refused extraction must leave the directory empty"
    )


def test_extract_binary_refuses_a_replacement_that_keeps_the_layout(
    tmp_path: pathlib.Path,
) -> None:
    """Refuse a replacement archive whose members still match the scan.

    ``ArchiveLayout`` records the binary's name, its declared size and the
    member count, not the content, so a replacement of equal size passes the
    re-scan. The trusted digest is what separates the two archives, and this
    test changes only the bytes: the file is replaced, the layout is not.
    """
    archive = _tarball(tmp_path / "archive.tar.gz")
    trusted = _digest(archive)
    layout = adapter.scan_archive(archive, expected_sha256=trusted)
    replacement = _tarball(
        tmp_path / "replacement.tar.gz", binary=b"netsuke binary payloaD"
    )
    _ = replacement.replace(archive)

    assert adapter.scan_archive(archive, expected_sha256=_digest(archive)) == layout, (
        "the fixture must be one the layout re-scan accepts, or it proves nothing"
    )
    destination = tmp_path / "out"
    destination.mkdir()

    with pytest.raises(adapter.ExtractionError):
        extract.extract_binary(archive, layout, destination, expected_sha256=trusted)

    assert not list(destination.iterdir()), (
        "a refused extraction must leave the directory empty"
    )


def test_extract_binary_removes_a_partial_file_when_the_copy_fails(
    tmp_path: pathlib.Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Delete the partial output when the copy cannot finish.

    A gzip stream that fails part-way through cannot be built without also
    breaking the member index the scan reads first, so the copy step is failed
    directly: the property under test is the cleanup, not how tar reports a
    damaged stream.
    """

    def explode(*_args: object, **_kwargs: object) -> None:
        """Fail the copy the way a damaged stream would."""
        msg = "stream failed part-way through"
        raise adapter.ArchiveCorruptError(msg)

    archive = _tarball(tmp_path / "archive.tar.gz")
    layout = adapter.scan_archive(archive, expected_sha256=_digest(archive))
    destination = tmp_path / "out"
    destination.mkdir()
    monkeypatch.setattr(extract, "_copy_member", explode)

    with pytest.raises(adapter.ArchiveCorruptError):
        extract.extract_binary(
            archive, layout, destination, expected_sha256=_digest(archive)
        )

    assert not list(destination.iterdir()), (
        "a failed extraction must not leave a partial binary behind"
    )


def test_probe_version_launches_the_binary_with_its_version_flag(
    tmp_path: pathlib.Path,
) -> None:
    """Pass the absolute path, ``--version``, and a minimal environment."""
    observed: dict[str, object] = {}

    def runner(
        command: list[str], **kwargs: object
    ) -> subprocess.CompletedProcess[str]:
        """Record the launch, then answer as a healthy binary would."""
        observed["command"] = command
        observed["env"] = kwargs["env"]
        return subprocess.CompletedProcess(command, 0, "netsuke 1.2.3\n", "")

    binary = tmp_path / "netsuke"
    result = probe.probe_version(binary, runner=runner)

    assert observed["command"] == [str(binary), "--version"], (
        "the probe must pass the binary path and its version flag as argv"
    )
    assert observed["env"] == probe.PROBE_ENVIRONMENT, (
        "the probe must run with its own fixed environment"
    )
    assert "GITHUB_TOKEN" not in typ.cast("dict[str, str]", observed["env"]), (
        "the probe must not inherit a workflow credential"
    )
    assert (result.returncode, result.stdout) == (0, "netsuke 1.2.3\n"), (
        "the probe must report the status and standard output it observed"
    )


def test_probe_version_reports_a_timeout(tmp_path: pathlib.Path) -> None:
    """Translate a killed probe into its own refusal."""

    def runner(
        command: list[str], **_kwargs: object
    ) -> subprocess.CompletedProcess[str]:
        """Fail the launch the way a probe that ran too long would."""
        raise subprocess.TimeoutExpired(command, probe.PROBE_TIMEOUT_SECONDS)

    with pytest.raises(probe.ProbeTimeoutError):
        probe.probe_version(tmp_path / "netsuke", runner=runner)


def test_probe_version_reports_a_launch_failure(tmp_path: pathlib.Path) -> None:
    """Translate a binary that never started into its own refusal."""

    def runner(
        command: list[str], **_kwargs: object
    ) -> subprocess.CompletedProcess[str]:
        """Fail the launch the way a missing binary would."""
        raise FileNotFoundError(command[0])

    with pytest.raises(probe.ProbeLaunchError):
        probe.probe_version(tmp_path / "netsuke", runner=runner)


@pytest.mark.skipif(sys.platform == "win32", reason="needs a POSIX shell script")
def test_probe_version_ignores_a_non_utf8_diagnostic(tmp_path: pathlib.Path) -> None:
    """Accept a correct banner whatever the child writes to standard error.

    The child is a script this test writes, not a downloaded file. It is
    launched for real because the defect is in ``subprocess``: text-mode
    capture decodes both streams, so one stray byte on standard error would
    otherwise refuse a run whose banner is exactly right.
    """
    binary = tmp_path / policy.BINARY_NAME
    _ = binary.write_text(
        "#!/bin/sh\nprintf 'netsuke 1.2.3\\n'\nprintf '\\377' >&2\n",
        encoding="utf-8",
    )
    binary.chmod(0o755)

    result = probe.probe_version(binary)

    assert result.stdout == "netsuke 1.2.3\n", (
        "a diagnostic on standard error must not decide the banner's verdict"
    )


def test_probe_version_reports_output_that_is_not_utf8(
    tmp_path: pathlib.Path,
) -> None:
    """Translate undecodable standard output into its own refusal."""

    def runner(
        command: list[str], **_kwargs: object
    ) -> subprocess.CompletedProcess[str]:
        """Fail the way text-mode capture does on undecodable bytes."""
        raise UnicodeDecodeError("utf-8", b"\xff", 0, 1, "invalid start byte")

    with pytest.raises(probe.ProbeOutputError):
        probe.probe_version(tmp_path / "netsuke", runner=runner)
