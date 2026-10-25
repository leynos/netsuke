"""Exercise checksum-verified download and extraction for the CI scripts."""

import contextlib
import hashlib
import http.server
import os
import threading
import typing as typ

import pytest
from ci_script_support import file_url, load_ci_script, write_tarball

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    import pathlib

ci_archive = load_ci_script("ci_archive")

REDIRECTING_BODY = b"redirected-payload"


@contextlib.contextmanager
def redirecting_server(location: str, header: str = "Location") -> cabc.Iterator[str]:
    """Serve a 302 to ``location`` at ``/redirect`` and a body at ``/target``.

    Parameters
    ----------
    location
        The redirect target, written into ``header``.
    header
        The header naming the target. CPython's ``http_error_302`` reads
        ``URI`` when ``Location`` is absent, so both spellings must be
        refused.

    Yields
    ------
    str
        The ``http://`` base URL. A real socket is the only way to exercise
        the redirect path: ``urllib`` decides whether to follow before
        :func:`download` sees a response, so a mocked opener would test the
        mock.
    """

    class Handler(http.server.BaseHTTPRequestHandler):
        """Answer ``/redirect`` with the 302 and everything else with a body."""

        def do_GET(self) -> None:
            """Answer the redirect, or the payload the redirect points at."""
            if self.path == "/redirect":
                self.send_response(302)
                self.send_header(header, location)
                self.end_headers()
                return
            self.send_response(200)
            self.send_header("Content-Length", str(len(REDIRECTING_BODY)))
            self.end_headers()
            self.wfile.write(REDIRECTING_BODY)

        def log_message(self, format: str, *args: object) -> None:  # ruff: ignore[builtin-argument-shadowing] - http.server's parameter name
            """Stay silent; the test's own assertions are the output."""

    server = http.server.HTTPServer(("127.0.0.1", 0), Handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    try:
        yield f"http://127.0.0.1:{server.server_address[1]}"
    finally:
        server.shutdown()
        server.server_close()


def test_download_copies_a_file_url(tmp_path: pathlib.Path) -> None:
    """A ``file://`` fixture is fetched through the real download path."""
    source = tmp_path / "source.bin"
    source.write_bytes(b"payload")

    ci_archive.download(file_url(source), tmp_path / "dest.bin")

    assert (tmp_path / "dest.bin").read_bytes() == b"payload", (
        "expected: (tmp_path / 'dest.bin').read_bytes() == b'payload'"
    )


@pytest.mark.parametrize("url", ["http://example.invalid/x", "ftp://example.invalid/x"])
def test_download_refuses_insecure_schemes(url: str, tmp_path: pathlib.Path) -> None:
    """Only https and file URLs are ever opened."""
    with pytest.raises(ci_archive.CiScriptError, match="URLs are permitted"):
        ci_archive.download(url, tmp_path / "dest.bin")
    assert not (tmp_path / "dest.bin").exists(), (
        "expected: not (tmp_path / 'dest.bin').exists()"
    )


def test_download_reports_a_missing_source(tmp_path: pathlib.Path) -> None:
    """A failed fetch is an actionable error naming the URL."""
    url = file_url(tmp_path / "absent.bin")
    with pytest.raises(ci_archive.CiScriptError, match=r"downloading .* failed"):
        ci_archive.download(url, tmp_path / "dest.bin")


@pytest.mark.parametrize("header", ["Location", "URI"])
def test_download_refuses_a_redirect_outside_the_allowed_schemes(
    header: str, tmp_path: pathlib.Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """A redirect target is allowlisted too, not only the URL asked for.

    CPython's own guard admits ``http``, ``https`` and ``ftp``, so on its own
    it would let a release URL be answered with a redirect to plain ``ftp://``
    and the archive would be fetched from there. The scheme set is narrowed
    here only so the loopback server below can be the origin.

    Both header spellings are covered because ``http_error_302`` accepts
    ``URI`` when ``Location`` is absent, so a check that read only ``Location``
    would let the second case through.
    """
    monkeypatch.setattr(ci_archive, "ALLOWED_URL_SCHEMES", frozenset({"http", "file"}))

    with (
        redirecting_server("ftp://example.invalid/archive.tar.gz", header) as base,
        pytest.raises(ci_archive.CiScriptError, match="refusing to open"),
    ):
        ci_archive.download(f"{base}/redirect", tmp_path / "dest.bin")

    assert not (tmp_path / "dest.bin").exists(), (
        "expected: not (tmp_path / 'dest.bin').exists()"
    )


def test_download_follows_a_redirect_within_the_allowed_schemes(
    tmp_path: pathlib.Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """The redirect check admits an ordinary in-scheme redirect."""
    monkeypatch.setattr(ci_archive, "ALLOWED_URL_SCHEMES", frozenset({"http", "file"}))

    with redirecting_server("/target") as base:
        ci_archive.download(f"{base}/redirect", tmp_path / "dest.bin")

    assert (tmp_path / "dest.bin").read_bytes() == REDIRECTING_BODY, (
        "expected: (tmp_path / 'dest.bin').read_bytes() == REDIRECTING_BODY"
    )


def test_verify_sha256_accepts_the_matching_digest(tmp_path: pathlib.Path) -> None:
    """A matching pin, in either case, passes silently."""
    archive = tmp_path / "a.tar.gz"
    archive.write_bytes(b"archive")
    digest = hashlib.sha256(b"archive").hexdigest()

    ci_archive.verify_sha256(archive, digest)
    ci_archive.verify_sha256(archive, digest.upper())


def test_verify_sha256_names_both_digests_on_mismatch(tmp_path: pathlib.Path) -> None:
    """The failure states the pinned and actual digests so drift is diagnosable."""
    archive = tmp_path / "a.tar.gz"
    archive.write_bytes(b"archive")
    actual = hashlib.sha256(b"archive").hexdigest()

    with pytest.raises(ci_archive.CiScriptError) as caught:
        ci_archive.verify_sha256(archive, "0" * 64)
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

    extracted = ci_archive.extract_members(
        archive, destination, ("tool", "nested/other")
    )

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

    with pytest.raises(ci_archive.CiScriptError, match="has no member 'tool'"):
        ci_archive.extract_members(archive, tmp_path, ("tool",))


def test_extract_members_refuses_a_symlink_member(tmp_path: pathlib.Path) -> None:
    """A link is never followed or written, so an archive cannot escape."""
    archive = write_tarball(
        tmp_path / "a.tar.gz", {"README": b"doc"}, symlinks={"tool": "/etc/passwd"}
    )

    with pytest.raises(ci_archive.CiScriptError, match="is not a regular file"):
        ci_archive.extract_members(archive, tmp_path, ("tool",))
    assert not (tmp_path / "tool").exists(), (
        "expected: not (tmp_path / 'tool').exists()"
    )
