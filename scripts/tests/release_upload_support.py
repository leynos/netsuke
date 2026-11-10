"""Stage the release and the ``gh`` exchange the uploader's suites share.

Two suites drive the uploader: one asserts the decision it reaches, the other
the exit status it reports. Both need the same three things -- a payload with a
trusted record, a release listing a case chooses, and a stub that answers the
invocations a real ``gh`` would receive -- and keeping one copy here is what
lets them disagree about behaviour while agreeing about fixtures.

The stub is the load-bearing part. It answers by *subcommand*, at the argument
position a real ``gh`` puts it in, and it records every invocation it receives.
A stub that answered by position alone would let an upload whose arguments were
misassembled pass, which is the failure these suites exist to catch.

Nothing here reaches the network.
"""

import dataclasses
import json
import pathlib
import subprocess  # ruff: ignore[suspicious-subprocess-import] - the seam under test.
import typing as typ

import release_digest_files
import release_digest_record_files
import release_upload_plan

if typ.TYPE_CHECKING:
    import pytest

TAG = "v1.0.0"
DEB = "netsuke_1.0.0_amd64.deb"
RPM = "netsuke-1.0.0-1.x86_64.rpm"


@dataclasses.dataclass(frozen=True, slots=True)
class Destinations:
    """Name the three files one run reads and writes.

    Attributes
    ----------
    payload
        Directory holding the selected assets. Defaults to the scenario's.
    record
        The declaration record to compare against. Defaults to the scenario's.
    selection
        The selection record. Defaults to the scenario's.
    plan
        Where the run writes its plan. Defaults to the scenario's.

    Notes
    -----
    A field left as ``None`` means "whatever the scenario already staged", so a
    case overrides only the one destination it is making hostile.
    """

    payload: pathlib.Path | None = None
    record: pathlib.Path | None = None
    selection: pathlib.Path | None = None
    plan: pathlib.Path | None = None


@dataclasses.dataclass(frozen=True, slots=True)
class Invocation:
    """Name how one run is launched, as distinct from what it acts on.

    Attributes
    ----------
    destinations
        The files the run reads and writes. Defaults to the scenario's own.
    dry_run
        Whether the run plans without transferring. Defaults to a real run.

    Notes
    -----
    Grouped rather than passed loose because the two travel together: a case
    rehearses a run by choosing both the mode and, usually, the destinations,
    and the repository keeps call surfaces to four arguments.
    """

    destinations: Destinations | None = None
    dry_run: bool = False


class Scenario:
    """Hold one run's payload, trusted record, and plan destination."""

    def __init__(self, root: pathlib.Path, *, contents: dict[str, bytes]) -> None:
        """Write ``contents`` as the payload and record their digests beside it."""
        self.root = root
        self.payload = root / "payload"
        self.payload.mkdir()
        self.contents = contents
        for name, content in contents.items():
            _ = (self.payload / name).write_bytes(content)
        self.record = root / "trusted.json"
        self.selection = root / "selection.json"
        release_digest_record_files.write_digest_record(self.digests(), self.record)
        release_digest_record_files.write_digest_record(self.digests(), self.selection)
        self.plan = root / release_upload_plan.PLAN_NAME

    def digests(self) -> dict[str, str]:
        """Return the digest of each payload file.

        Returns
        -------
        dict[str, str]
            Published name mapped to the SHA-256 of its bytes.
        """
        return {
            name: release_digest_files.digest_of_file(self.payload / name)
            for name in self.contents
        }

    def request(self, *, is_dry_run: bool = False) -> release_upload_plan.UploadRequest:
        """Return the request this scenario runs.

        Returns
        -------
        release_upload_plan.UploadRequest
            A request naming the payload, record, and plan.
        """
        return release_upload_plan.UploadRequest(
            tag=TAG,
            payload_dir=self.payload,
            digest_record=self.record,
            selection_record=self.selection,
            plan_path=self.plan,
            is_dry_run=is_dry_run,
        )

    def stored_plan(self) -> release_upload_plan.PlanRecord:
        """Return the decoded plan the run wrote.

        Returns
        -------
        release_upload_plan.PlanRecord
            The plan, decoded through the writer's own record so a case reads
            the fields the run actually wrote rather than re-describing them.
        """
        return release_upload_plan.read_plan(self.plan)


def listing(**attached: str | None) -> str:
    """Return a release listing naming each ``attached`` asset.

    Returns
    -------
    str
        A ``gh release view --json assets`` payload.
    """
    assets = [
        {"name": name, "digest": digest} if digest is not None else {"name": name}
        for name, digest in attached.items()
    ]
    return json.dumps({"assets": assets})


def completed(
    arguments: list[str],
    *,
    returncode: int = 0,
    stdout: str = "",
    stderr: str = "",
) -> subprocess.CompletedProcess[str]:
    """Return the result of one invocation for the stub to hand back.

    Parameters
    ----------
    arguments
        The command line the stub received, echoed into the result.
    returncode
        The status a real ``gh`` would exit with.
    stdout
        The body to answer with, for a listing.
    stderr
        The diagnostic to answer with, for a refusal.

    Returns
    -------
    subprocess.CompletedProcess
        The fabricated result.

    Notes
    -----
    One builder rather than a ``succeeded``/``failed`` pair. The pair differed
    only in which stream carried the body and in the status, and every new
    answer the stub needed -- accepted but dropped, transient 502, 404 -- cost
    two more nearly identical functions.

    Examples
    --------
    >>> completed(["gh", "release", "view"], stdout='{"assets": []}').returncode
    0
    >>> completed(["gh", "release", "upload"], returncode=1, stderr="502").stderr
    '502'
    """
    return subprocess.CompletedProcess(
        args=list(arguments), returncode=returncode, stdout=stdout, stderr=stderr
    )


def digest_of(path: pathlib.Path) -> str:
    """Return the digest of a file the stub was sent.

    Returns
    -------
    str
        The lowercase SHA-256 of the bytes, or an empty string when absent.
    """
    try:
        return release_digest_files.digest_of_file(path)
    except OSError:
        return ""


class Exchange:
    """Answer ``gh`` for a release whose attached assets a case chooses."""

    def __init__(
        self,
        *,
        attached: dict[str, str | None],
        downloads: dict[str, bytes] | None = None,
        fail_upload: bool = False,
        drop_uploads: bool = False,
    ) -> None:
        """Hold the release's attached digests, staged downloads, and failure mode."""
        self.attached = attached
        self.downloads = downloads or {}
        self.fail_upload = fail_upload
        # Models the failure the status check exists for: `gh` reports the
        # upload as accepted, but the asset never appears on the release.
        self.drop_uploads = drop_uploads
        self.calls: list[list[str]] = []

    def __call__(
        self, arguments: list[str], **_keywords: object
    ) -> subprocess.CompletedProcess[str]:
        """Record the invocation and answer it from the scenario's state."""
        self.calls.append(list(arguments))
        return self._answer(arguments)

    def _answer(self, arguments: list[str]) -> subprocess.CompletedProcess[str]:
        """Return the result of one ``gh`` subcommand.

        Returns
        -------
        subprocess.CompletedProcess
            The stubbed exchange, carrying either a listing or an empty body.

        Notes
        -----
        ``arguments`` is ``["gh", "release", <subcommand>, ...]``, so the
        subcommand is at index 2 and an upload's files follow the tag at 4.
        """
        subcommand = arguments[2]
        if subcommand == "upload":
            return self._upload(arguments)
        if subcommand == "download":
            return self._download(arguments)
        return completed(arguments, stdout=listing(**self.attached))

    def _upload(self, arguments: list[str]) -> subprocess.CompletedProcess[str]:
        """Attach each sent file's bytes, or fail as a rejected transfer would."""
        if self.fail_upload:
            return completed(arguments, returncode=1, stderr="HTTP 502 Bad Gateway")
        if self.drop_uploads:
            # Accepted, but nothing is attached. The caller only learns this
            # from the re-listing.
            return completed(arguments)
        self.attached |= {
            pathlib.Path(name).name: f"sha256:{digest_of(pathlib.Path(name))}"
            for name in arguments[4:]
        }
        return completed(arguments)

    def _download(self, arguments: list[str]) -> subprocess.CompletedProcess[str]:
        """Write the requested asset's bytes, or fail as a 404 would."""
        name = arguments[arguments.index("--pattern") + 1]
        if name not in self.downloads:
            return completed(arguments, returncode=1, stderr="HTTP 404 Not Found")
        directory = pathlib.Path(arguments[arguments.index("--dir") + 1])
        _ = (directory / name).write_bytes(self.downloads[name])
        return completed(arguments)

    def uploads(self) -> list[list[str]]:
        """Return every upload invocation the exchange received.

        Returns
        -------
        list[list[str]]
            One argument list per ``gh release upload``.
        """
        return [call for call in self.calls if call[2] == "upload"]

    def clobbered(self) -> bool:
        """Report whether any upload passed ``--clobber``.

        Returns
        -------
        bool
            ``True`` if any upload invocation carried the flag.
        """
        return any("--clobber" in call for call in self.calls)


def main_argv(
    scenario: Scenario,
    *,
    destinations: Destinations | None = None,
    dry_run: bool = False,
) -> list[str]:
    """Return the command line a run of ``scenario`` passes to ``main``.

    Returns
    -------
    list[str]
        The arguments, with any destination replaced by the one given.

    Notes
    -----
    Kept here rather than in each suite so a case that makes one destination
    hostile still passes the other two exactly as a real run would. The three
    overrides travel together as :class:`Destinations` rather than as three
    separate parameters, which keeps this call surface inside the repository's
    four-argument ceiling and matches how a case names the one it is making
    hostile.
    """
    chosen = destinations or Destinations()
    mode = ["--dry-run"] if dry_run else []
    return [
        "--release-tag",
        TAG,
        "--payload-dir",
        str(scenario.payload if chosen.payload is None else chosen.payload),
        "--digest-record",
        str(scenario.record if chosen.record is None else chosen.record),
        "--selection-record",
        str(scenario.selection if chosen.selection is None else chosen.selection),
        "--plan",
        str(scenario.plan if chosen.plan is None else chosen.plan),
        *mode,
    ]


def run_cli(
    scenario: Scenario,
    exchange: Exchange,
    monkeypatch: pytest.MonkeyPatch,
    *,
    invocation: Invocation | None = None,
) -> int:
    """Run ``main`` over ``scenario`` with ``exchange`` answering ``gh``.

    Returns
    -------
    int
        The exit status ``main`` reported.

    Notes
    -----
    ``invocation`` groups the two choices a case makes about *how* the run is
    launched — which destinations it reads and writes, and whether it is a dry
    run — so the call surface stays within the repository's argument ceiling
    and a case names only the one it is varying.
    """
    import upload_declared_assets_cli as cli

    chosen = invocation or Invocation()
    monkeypatch.setattr(cli, "PRODUCTION_RUNNER", exchange)
    return cli.main(
        main_argv(scenario, destinations=chosen.destinations, dry_run=chosen.dry_run)
    )


__all__ = [
    "DEB",
    "RPM",
    "TAG",
    "Destinations",
    "Exchange",
    "Invocation",
    "Scenario",
    "completed",
    "digest_of",
    "listing",
    "main_argv",
    "run_cli",
]
