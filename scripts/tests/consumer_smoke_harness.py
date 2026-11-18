"""Stage the workspace, downloads, and probe every smoke suite shares.

Three suites drive the consumer smoke: the CLI end to end, the archive and
probe adapters, and the decisions taken from in-memory values. All three need
the same release-shaped fixture — a package record, a trusted digest record,
the release declaration, a tarball carrying one binary, its published sidecar,
and an injected ``--version`` probe — so one copy lives here and each suite
asserts its own behaviour against it.

The probe stub is the load-bearing part. It answers with a fixed status and
banner and never launches anything, so no test executes a downloaded file and
none reaches the network. The workspace is a real directory tree, because the
CLI reads files rather than values and a fake filesystem would stop exercising
the reads that can fail.

Nothing here reaches the network.
"""

import dataclasses
import io
import json
import tarfile
import typing as typ

from consumer_smoke_cli import main
from release_digest_files import digest_of_file

if typ.TYPE_CHECKING:
    import pathlib

    from release_binstall_probe import ProbeLauncher
    from release_consumer_payload import VerdictRecord

VERSION = "1.2.3"
COMMIT = "0" * 40
TARGET = "x86_64-unknown-linux-gnu"
PACKAGE = "netsuke-build"
REPOSITORY = "leynos/netsuke"
CRATE_NAME = f"{PACKAGE}-{VERSION}.crate"
ARCHIVE_NAME = f"{PACKAGE}-{VERSION}-{TARGET}.tar.gz"
SIDECAR_NAME = f"{ARCHIVE_NAME}.sha256"
BINARY_BYTES = b"netsuke binary payload"
BANNER = f"netsuke {VERSION}\n"


@dataclasses.dataclass(slots=True)
class FakeChild:
    """Stand in for a launched child, recording how the probe drove it.

    Attributes
    ----------
    returncode
        The status the child reports once it is reaped.
    stream
        What the child wrote to standard output, read at the probe's request.
    stopped
        Whether the probe stopped the child. A refusal that stops its child
        before reaping it leaves this true while :attr:`reads` shows the
        ceiling was reached.
    reads
        The size of every read the probe asked for, so a test can prove the
        ceiling was requested rather than only described.
    reaped
        Whether the probe waited for the child, so a test can prove that no
        refusal left a process behind.
    """

    returncode: int
    stream: io.TextIOBase
    stopped: bool = False
    reads: list[int] = dataclasses.field(default_factory=list)
    reaped: bool = False
    closed: bool = False

    def read_stdout(self, size: int) -> str:
        """Read up to ``size`` characters, recording the bound asked for."""
        self.reads.append(size)
        return self.stream.read(size)

    def wait_until(self, deadline: float) -> int | None:
        """Report the child as exited, which a fake does the moment it is asked.

        Returns
        -------
        int
            The status the child exited with. A fake that must stay running
            overrides this rather than the probe growing a second code path.
        """
        _ = deadline
        self.reaped = True
        return self.returncode

    def stop(self) -> int:
        """Stop and reap the child, reporting the status it exited with."""
        self.stopped = True
        self.reaped = True
        return self.returncode

    def close(self) -> None:
        """Release the standard output, which nothing else holds."""
        self.closed = True


@dataclasses.dataclass(frozen=True, slots=True)
class Workspace:
    """Name every file one smoke run reads or writes.

    Attributes
    ----------
    root
        The temporary directory holding every other path.
    identity
        The package record the packaging stage wrote.
    trusted
        The pre-upload digest record, keyed by published asset name.
    staging
        The release staging configuration naming every target.
    manifest
        The Cargo manifest naming the package.
    plan
        The plan document ``plan`` writes and ``verify`` re-checks.
    verdict
        The bounded verdict record ``verify`` rewrites per transition.
    phase_log
        The bounded per-phase outcome and timing record.
    crate
        The saved copy of the crate download.
    crate_status
        The HTTP status curl recorded for the crate.
    archive
        The saved cargo-binstall archive.
    archive_status
        The HTTP status curl recorded for the archive.
    sidecar
        The saved checksum sidecar.
    sidecar_status
        The HTTP status curl recorded for the sidecar.
    extract
        The fresh, empty directory the binary is extracted into.
    """

    root: pathlib.Path
    identity: pathlib.Path
    trusted: pathlib.Path
    staging: pathlib.Path
    manifest: pathlib.Path
    plan: pathlib.Path
    verdict: pathlib.Path
    phase_log: pathlib.Path
    crate: pathlib.Path
    crate_status: pathlib.Path
    archive: pathlib.Path
    archive_status: pathlib.Path
    sidecar: pathlib.Path
    sidecar_status: pathlib.Path
    extract: pathlib.Path


def tarball(path: pathlib.Path, *, symlink: bool) -> pathlib.Path:
    """Write the published archive, optionally carrying a symlink member.

    Returns
    -------
    pathlib.Path
        The archive's path, once written.
    """
    with tarfile.open(path, "w:gz") as archive:
        entry = tarfile.TarInfo("netsuke")
        entry.size = 0 if symlink else len(BINARY_BYTES)
        if symlink:
            entry.type = tarfile.SYMTYPE
            entry.linkname = "/etc/passwd"
        _ = archive.addfile(entry, io.BytesIO(b"" if symlink else BINARY_BYTES))
    return path


def write_json(path: pathlib.Path, value: object) -> None:
    """Write one JSON document the CLI reads."""
    _ = path.write_text(json.dumps(value), encoding="utf-8")


def workspace(tmp_path: pathlib.Path, *, symlink: bool = False) -> Workspace:
    """Build a verified-release workspace the CLI can plan and verify.

    Returns
    -------
    Workspace
        Every path a run reads or writes, under ``tmp_path``.
    """
    archive = tarball(tmp_path / ARCHIVE_NAME, symlink=symlink)
    sidecar = tmp_path / SIDECAR_NAME
    _ = sidecar.write_text(
        f"{digest_of_file(archive)}  {ARCHIVE_NAME}\n", encoding="utf-8"
    )
    crate = tmp_path / CRATE_NAME
    _ = crate.write_bytes(b"crate archive bytes")
    status = tmp_path / "status"
    _ = status.write_text("200\n", encoding="utf-8")
    identity = tmp_path / "identity.json"
    write_json(
        identity,
        {
            "crate": CRATE_NAME,
            "identity": {
                "tag": f"v{VERSION}",
                "commit": COMMIT,
                "package_name": PACKAGE,
                "version": VERSION,
                "bin_name": "netsuke",
                "is_prerelease": False,
            },
            "sha256": digest_of_file(crate),
        },
    )
    trusted = tmp_path / "trusted.json"
    write_json(
        trusted,
        {
            "assets": {
                ARCHIVE_NAME: digest_of_file(archive),
                SIDECAR_NAME: digest_of_file(sidecar),
            }
        },
    )
    staging = tmp_path / "release-staging.toml"
    _ = staging.write_text(f'[targets.linux]\ntarget = "{TARGET}"\n', encoding="utf-8")
    manifest = tmp_path / "Cargo.toml"
    _ = manifest.write_text(f'[package]\nname = "{PACKAGE}"\n', encoding="utf-8")
    extract = tmp_path / "extract"
    extract.mkdir()
    return Workspace(
        root=tmp_path,
        identity=identity,
        trusted=trusted,
        staging=staging,
        manifest=manifest,
        plan=tmp_path / "plan.json",
        verdict=tmp_path / "verdict.json",
        phase_log=tmp_path / "phase-log.json",
        crate=crate,
        crate_status=status,
        archive=archive,
        archive_status=status,
        sidecar=sidecar,
        sidecar_status=status,
        extract=extract,
    )


def evidence_arguments(target: Workspace) -> list[str]:
    """Return the arguments both subcommands share.

    Returns
    -------
    list[str]
        The evidence flags and their values.
    """
    return [
        "--identity",
        str(target.identity),
        "--trusted",
        str(target.trusted),
        "--staging-config",
        str(target.staging),
        "--manifest",
        str(target.manifest),
        "--repository",
        REPOSITORY,
        "--target",
        TARGET,
        "--commit",
        COMMIT,
    ]


def run_plan(target: Workspace) -> int:
    """Run the ``plan`` subcommand for the workspace.

    Returns
    -------
    int
        The exit status ``plan`` reported.
    """
    return main(["plan", *evidence_arguments(target), "--output", str(target.plan)])


def run_verify(
    target: Workspace,
    *,
    launcher: ProbeLauncher | None = None,
    overrides: dict[str, pathlib.Path] | None = None,
) -> int:
    """Run the ``verify`` subcommand for the workspace.

    Returns
    -------
    int
        The exit status ``verify`` reported.
    """
    paths = {
        "plan": target.plan,
        "crate": target.crate,
        "crate-status": target.crate_status,
        "archive": target.archive,
        "archive-status": target.archive_status,
        "sidecar": target.sidecar,
        "sidecar-status": target.sidecar_status,
        "extract-dir": target.extract,
        "verdict": target.verdict,
        "phase-log": target.phase_log,
    }
    paths.update(overrides or {})
    arguments = ["verify", *evidence_arguments(target)]
    for flag, path in paths.items():
        arguments.extend([f"--{flag}", str(path)])
    return main(arguments, launcher=launcher)


def fake_probe(*, returncode: int = 0, stdout: str = BANNER) -> ProbeLauncher:
    """Return a launcher whose child reports one fixed outcome.

    Returns
    -------
    ProbeLauncher
        A launcher that starts a :class:`FakeChild` instead of a process.
    """

    def launcher(command: list[str], **_kwargs: object) -> FakeChild:
        """Answer the version probe without launching anything."""
        _ = command
        return FakeChild(returncode=returncode, stream=io.StringIO(stdout))

    return launcher


def read_verdict(target: Workspace) -> VerdictRecord:
    """Read the verdict artefact the last run wrote.

    Returns
    -------
    VerdictRecord
        The decoded verdict, carrying its phase table and its overall verdict.

    Notes
    -----
    The artefact is JSON, so every member of it arrives as ``object``. Both
    members are established here, once, so a caller can read
    ``verdict["phases"][label]`` without re-checking the shape at each site; an
    artefact missing either fails the assertions below rather than handing back
    a value the caller would have to re-check. The top-level member set is
    matched exactly rather than sampled, so a run that starts reporting
    something new to a consumer fails the suite that reads this artefact.
    """
    decoded = json.loads(target.verdict.read_text(encoding="utf-8"))
    assert isinstance(decoded, dict), "the verdict artefact must be a JSON object"
    assert set(decoded) == {"phases", "verdict"}, (
        "the verdict artefact must carry exactly its phase table and its verdict, "
        f"not {sorted(decoded)}"
    )
    phases = decoded.get("phases")
    assert isinstance(phases, dict), "the verdict artefact must record its phases"
    verdict = decoded.get("verdict")
    assert isinstance(verdict, str), "the verdict artefact must record a verdict"
    return {"phases": phases, "verdict": verdict}


def read_phase_log(target: Workspace) -> dict[str, dict[str, object]]:
    """Read the phase log the last run wrote.

    Returns
    -------
    dict[str, dict[str, object]]
        One entry per phase the run reached.

    Notes
    -----
    An artefact whose members are not all JSON objects fails the assertions
    below rather than returning a shape the caller would have to re-check.
    """
    decoded = json.loads(target.phase_log.read_text(encoding="utf-8"))
    assert isinstance(decoded, dict), "the phase log must be a JSON object"
    assert all(isinstance(entry, dict) for entry in decoded.values()), (
        "every phase log entry must be a JSON object"
    )
    return decoded
