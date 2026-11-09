"""Test the ``gh`` boundary the uploader talks to and nothing else does.

Every case here drives a recording stub in place of ``subprocess.run``, so the
suite asserts on the *command* a caller would have launched. That is the only
way to pin the property this module exists for: no invocation may carry
``--clobber``, whatever the payload or the listing says.

Run via ``make test-release-publication``.
"""

import pathlib
import subprocess  # ruff: ignore[suspicious-subprocess-import] - the seam under test.

import pytest
import release_digest_files
import release_upload_remote
from release_upload_digests import downloaded_digests
from release_upload_failures import DigestFailure
from release_upload_remote import (
    GhCommandFailedError,
    RemoteListingError,
    StaticRunner,
    UploadError,
    attached_digests,
    upload,
)

DIGEST = "ab" * 32


class RecordingRunner:
    """Record every invocation and answer with a caller-chosen result."""

    def __init__(
        self,
        *,
        stdout: str = "",
        returncode: int = 0,
        stderr: str = "",
        writes: dict[str, bytes] | None = None,
    ) -> None:
        """Hold the canned answer and start an empty record of invocations."""
        self.stdout = stdout
        self.returncode = returncode
        self.stderr = stderr
        self.writes = writes or {}
        self.links: dict[str, pathlib.Path] = {}
        self.calls: list[list[str]] = []

    def link(self, name: str, target: pathlib.Path) -> None:
        """Install ``name`` as a symlink to ``target`` when a download runs.

        Model a download resolving to something that is not the asset. The
        staging is kept out of the constructor because it is a second concern
        the answer itself does not have, and every other case leaves it empty.
        """
        self.links[name] = target

    def __call__(
        self, arguments: list[str], **_keywords: object
    ) -> subprocess.CompletedProcess[str]:
        """Record the call, write any staged download, and return the result."""
        self.calls.append(list(arguments))
        if "--dir" in arguments:
            directory = pathlib.Path(arguments[arguments.index("--dir") + 1])
            for name, content in self.writes.items():
                _ = (directory / name).write_bytes(content)
            for name, target in self.links.items():
                (directory / name).symlink_to(target)
        return subprocess.CompletedProcess(
            args=list(arguments),
            returncode=self.returncode,
            stdout=self.stdout,
            stderr=self.stderr,
        )


def asset_listing(*assets: tuple[str, object]) -> str:
    """Return a ``gh release view --json assets`` payload.

    Returns
    -------
    str
        JSON naming each asset and the digest GitHub reported for it.
    """
    import json

    entries = [
        {"name": name, "digest": digest} if digest is not None else {"name": name}
        for name, digest in assets
    ]
    return json.dumps({"assets": entries})


def test_attached_digests_reads_the_asset_listing() -> None:
    """Normalise the vendor digest into the bare value reconciliation compares."""
    runner = RecordingRunner(stdout=asset_listing(("a.deb", f"sha256:{DIGEST}")))
    assert attached_digests("v1", runner=runner) == {"a.deb": DIGEST}, (
        "the vendor digest must be normalised to its bare value"
    )


def test_attached_digests_lists_the_release_with_json_assets() -> None:
    """Ask ``gh`` for the asset member, not for a human-readable table."""
    runner = RecordingRunner(stdout=asset_listing())
    _ = attached_digests("v1", runner=runner)
    assert runner.calls == [["gh", "release", "view", "v1", "--json", "assets"]], (
        "the listing must ask for the assets member as JSON"
    )


def test_an_absent_digest_is_unknown_rather_than_empty() -> None:
    """Report ``None`` where GitHub supplied nothing comparable.

    Folding that into an empty string would let "nothing verifiable" read as
    "nothing wrong".
    """
    runner = RecordingRunner(stdout=asset_listing(("a.deb", None)))
    assert attached_digests("v1", runner=runner) == {"a.deb": None}, (
        "an absent digest must be unknown, not empty"
    )


def test_an_unusable_digest_is_unknown_rather_than_empty() -> None:
    """Treat a digest in another algorithm the same way as none at all."""
    runner = RecordingRunner(stdout=asset_listing(("a.deb", "md5:abcd")))
    assert attached_digests("v1", runner=runner) == {"a.deb": None}, (
        "an absent digest must be unknown, not empty"
    )


def test_entries_without_a_name_are_skipped() -> None:
    """Ignore a listing entry no release asset could be reconciled by."""
    import json

    stdout = json.dumps({"assets": [{"digest": DIGEST}, {"name": 7}, "junk"]})
    runner = RecordingRunner(stdout=stdout)
    assert attached_digests("v1", runner=runner) == {}, (
        "a nameless entry must not be reconciled"
    )


def test_a_listing_that_is_not_json_is_refused() -> None:
    """Report output no reconciliation can read as a listing defect."""
    runner = RecordingRunner(stdout="not json")
    with pytest.raises(RemoteListingError, match="not JSON"):
        attached_digests("v1", runner=runner)


def test_a_listing_without_an_asset_member_is_refused() -> None:
    """Refuse JSON that does not answer the question that was asked."""
    runner = RecordingRunner(stdout='{"tagName": "v1"}')
    with pytest.raises(RemoteListingError, match="no asset list"):
        attached_digests("v1", runner=runner)


def test_a_failed_listing_reports_the_command_and_status() -> None:
    """Name the exact invocation that failed so an operator can re-run it."""
    runner = RecordingRunner(returncode=1, stderr="gh: not authenticated")
    with pytest.raises(GhCommandFailedError, match="not authenticated"):
        attached_digests("v1", runner=runner)


def test_a_failed_listing_names_the_arguments_it_used() -> None:
    """Include the arguments so a mistyped tag is visible in the log."""
    runner = RecordingRunner(returncode=4, stderr="boom")
    with pytest.raises(GhCommandFailedError, match="release view v1"):
        attached_digests("v1", runner=runner)


def test_upload_sends_one_call_for_every_pending_name(tmp_path: pathlib.Path) -> None:
    """Send every missing asset in one invocation, so a partial send is visible."""
    runner = RecordingRunner()
    upload("v1", ["b.rpm", "a.deb"], tmp_path, runner=runner)
    expected = [
        [
            "gh",
            "release",
            "upload",
            "v1",
            str(tmp_path / "a.deb"),
            str(tmp_path / "b.rpm"),
        ]
    ]
    assert runner.calls == expected, "one call, with the names in sorted order"


def test_upload_never_passes_clobber(tmp_path: pathlib.Path) -> None:
    """Refuse to replace attached bytes, whatever the payload holds.

    ``--clobber`` deletes the existing asset before transferring, so a failed
    upload loses the original; and a release that replaced its own bytes could
    publish a second commit's build under the first commit's tag.
    """
    runner = RecordingRunner()
    upload("v1", ["a.deb"], tmp_path, runner=runner)
    assert "--clobber" not in runner.calls[0], (
        "the uploader must never delete an attached asset before transfer"
    )


def test_upload_of_nothing_asks_gh_for_nothing(tmp_path: pathlib.Path) -> None:
    """Refuse to invoke ``gh`` with no file, which uploads nothing and may error."""
    runner = RecordingRunner()
    upload("v1", [], tmp_path, runner=runner)
    assert not runner.calls, (
        f"an empty payload must not reach gh at all, got {runner.calls}"
    )


def test_upload_reports_a_transport_failure(tmp_path: pathlib.Path) -> None:
    """Surface a failed transfer rather than returning as though it worked."""
    runner = RecordingRunner(returncode=1, stderr="HTTP 502")
    with pytest.raises(GhCommandFailedError, match="HTTP 502"):
        upload("v1", ["a.deb"], tmp_path, runner=runner)


def test_upload_failure_is_an_upload_error(tmp_path: pathlib.Path) -> None:
    """Keep the failure in the family the CLI maps to one exit status."""
    runner = RecordingRunner(returncode=1)
    with pytest.raises(UploadError):
        upload("v1", ["a.deb"], tmp_path, runner=runner)


def test_downloaded_digests_hashes_the_exact_named_asset(
    tmp_path: pathlib.Path,
) -> None:
    """Establish a digest from the downloaded bytes when the listing had none."""
    content = b"attached bytes"
    runner = RecordingRunner(writes={"a.deb": content})
    evidence = downloaded_digests("v1", ["a.deb"], runner=runner)
    assert evidence.digests["a.deb"] == release_digest_files.digest_of_file(
        _written(tmp_path, content)
    ), "the digest must come from the downloaded bytes"
    assert not evidence.failures, "a hash that succeeded needs no reason"


def _written(tmp_path: pathlib.Path, content: bytes) -> pathlib.Path:
    """Write ``content`` so a digest can be computed from the same bytes.

    Returns
    -------
    pathlib.Path
        The written path.
    """
    path = tmp_path / "expected.bin"
    _ = path.write_bytes(content)
    return path


def test_downloaded_digests_asks_for_the_exact_name() -> None:
    """Pin the pattern to the asset name so a glob cannot fetch another asset."""
    runner = RecordingRunner()
    _ = downloaded_digests("v1", ["a.deb"], runner=runner)
    call = runner.calls[0]
    assert call[:4] == ["gh", "release", "download", "v1"], call[:4]
    assert call[call.index("--pattern") + 1] == "a.deb", (
        "the pattern must be the exact asset name"
    )


def test_downloaded_digests_downloads_into_its_own_directory() -> None:
    """Pass an explicit destination so a download cannot land in the checkout."""
    runner = RecordingRunner()
    _ = downloaded_digests("v1", ["a.deb"], runner=runner)
    assert "--dir" in runner.calls[0], "the download must name its own destination"


def test_a_failed_download_leaves_the_digest_unknown() -> None:
    """Answer ``None`` rather than raising: reconciliation turns it into a conflict."""
    runner = RecordingRunner(returncode=1, stderr="404")
    evidence = downloaded_digests("v1", ["a.deb"], runner=runner)
    assert evidence.digests == {"a.deb": None}, (
        "a failed download must leave the digest unknown"
    )
    assert evidence.failures["a.deb"] is DigestFailure.DOWNLOAD_FAILED, (
        "a refused transfer must be reported as one, not as a missing file"
    )


def test_a_download_that_writes_nothing_leaves_the_digest_unknown() -> None:
    """Refuse to hash an absence as though it were agreement."""
    runner = RecordingRunner()
    evidence = downloaded_digests("v1", ["a.deb"], runner=runner)
    assert evidence.digests == {"a.deb": None}, (
        "an absent file must not be hashed as agreement"
    )
    assert evidence.failures["a.deb"] is DigestFailure.MISSING_FILE, (
        "a transfer that exited zero and wrote nothing is its own finding"
    )


def test_a_failed_download_is_not_hidden_by_the_file_it_wrote() -> None:
    """Report the refused transfer rather than hashing bytes it never sent.

    A transfer can exit non-zero having written a partial file -- a truncated
    body is exactly what a dropped connection leaves. Hashing it would answer
    a question about the release with bytes no complete download produced, and
    the resulting digest would differ from the record for a reason that says
    nothing about GitHub. The status is therefore consulted first.
    """
    runner = RecordingRunner(returncode=1, writes={"a.deb": b"half a body"})
    evidence = downloaded_digests("v1", ["a.deb"], runner=runner)
    assert evidence.digests == {"a.deb": None}, (
        "a refused download must not be answered from the file it left behind"
    )
    assert evidence.failures["a.deb"] is DigestFailure.DOWNLOAD_FAILED, (
        evidence.failures
    )


def test_a_symlink_at_the_asset_name_is_not_followed(tmp_path: pathlib.Path) -> None:
    """Refuse to hash whatever a symlink points at.

    The name arrives from the release's listing, and following it would let a
    download hand the hash the contents of a file outside the directory it
    wrote into -- which is not evidence about the release at all.
    """
    outside = tmp_path / "outside.bin"
    _ = outside.write_bytes(b"not this asset")
    runner = RecordingRunner()
    runner.link("a.deb", outside)
    evidence = downloaded_digests("v1", ["a.deb"], runner=runner)
    assert evidence.digests == {"a.deb": None}, "a symlink supplies no digest"
    assert evidence.failures["a.deb"] is DigestFailure.NOT_A_REGULAR_FILE, (
        evidence.failures
    )


def test_downloaded_digests_never_reports_a_digest_it_did_not_compute() -> None:
    """Leave every name it could not establish at ``None``.

    The stub writes one asset and not the other, which is what a partial
    download looks like to a caller.
    """
    runner = RecordingRunner(writes={"a.deb": b"x"})
    evidence = downloaded_digests("v1", ["a.deb", "b.rpm"], runner=runner)
    assert evidence.digests["b.rpm"] is None, "the missing asset must stay unknown"
    assert evidence.digests["a.deb"] is not None, "the written asset must be hashed"
    assert set(evidence.failures) == {"b.rpm"}, (
        "only the name without a digest may carry a reason"
    )


def test_downloaded_digests_of_nothing_launches_nothing() -> None:
    """Establish no digest for no name without paying for a download."""
    runner = RecordingRunner()
    evidence = downloaded_digests("v1", [], runner=runner)
    assert (evidence.digests, evidence.failures) == ({}, {}), (
        "no name means no digest and no reason"
    )
    assert not runner.calls, "no name means no subprocess"


def test_a_static_runner_records_what_a_caller_ran() -> None:
    """Record the exchange so a dry-run case can be asserted after the fact."""
    runner = StaticRunner('{"assets": []}')
    _ = attached_digests("v1", runner=runner)
    assert runner.calls == [["gh", "release", "view", "v1", "--json", "assets"]], (
        "the listing must ask for the assets member as JSON"
    )


def test_the_static_runner_always_succeeds() -> None:
    """Pin the stub's contract so a case using it is testing what it thinks."""
    completed = StaticRunner('{"assets": []}')(["gh", "release", "view"])
    assert completed.returncode == 0, completed.stderr
    assert not completed.stderr, completed.stderr


def test_the_production_runner_is_subprocess_run() -> None:
    """Name the one production entry point the boundary installs."""
    assert release_upload_remote.PRODUCTION_RUNNER is subprocess.run, (
        "the production runner must be the real subprocess boundary"
    )
