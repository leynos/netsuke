"""Execute the consumer-smoke action's own shell steps outside GitHub.

The suites around ``.github/actions/consumer-smoke`` read the manifest as text:
they assert that the job carries no credential, that each step names the CLI,
and that the script around each invocation has the accepted shape. None of them
runs a step, so a flag spelled wrongly, an input read under a name the action
never exports, or a report step that reads a log it cannot parse can satisfy
every structural check and still fail on a runner.

This module closes that gap. Each ``run`` body is taken from the manifest rather
than restated here, so the command lines, the environment each step is handed,
and the phase-reporting loop are held to account as written. ``uv`` is stubbed
at the path boundary and hands the remainder to the real interpreter, so the
checked-in CLI runs for real; ``curl`` is stubbed so no request leaves the host,
and the bytes it serves are the fixtures a case published. The package record,
the trusted digest record, the downloaded bodies, the extracted binary, and the
two artefacts the verifier writes are all real files.

The manifest readers and the expression resolver live in
``consumer_smoke_action_manifest.py``; the cases that use this harness live in
``consumer_smoke_action_execution_test.py``.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import dataclasses
import hashlib
import io
import json
import sys
import tarfile
import typing as typ

from composite_action_execution import (
    output_file,
    read_step_outputs,
    run_shell,
    write_stub,
)
from composite_action_stubs import UV_STUB
from consumer_smoke_action_manifest import (
    REPORT_STEP,
    manifest_step_id,
    should_run,
    smoke_steps,
    step_environment,
)
from release_publish_path_consumer_shape import SMOKE_ACTION_TARGET
from workflow_loading import REPO_ROOT

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    import pathlib
    import subprocess

    type BeforeStep = cabc.Callable[[int, pathlib.Path], None]

#: The candidate version every fixture describes unless a case says otherwise.
VERSION = "1.2.3"
#: The crate whose package record is the smoke's identity evidence.
PACKAGE = "netsuke-build"
#: The target the action exercises, read from the job's contract rather than
#: restated, so a workflow that changes its matrix target changes this too.
TARGET = SMOKE_ACTION_TARGET
#: The published names the plan must derive for this candidate.
CRATE_NAME = f"{PACKAGE}-{VERSION}.crate"
ARCHIVE_NAME = f"{PACKAGE}-{VERSION}-{TARGET}.tar.gz"
SIDECAR_NAME = f"{ARCHIVE_NAME}.sha256"
#: The banner the extracted binary prints when it is the candidate's.
BANNER = f"netsuke {VERSION}"
#: The commit the package record and the workflow both name.
COMMIT = "0" * 40
#: The shipped binary, as the archive carries it. A shell script rather than an
#: ELF file because the probe only asks it to answer ``--version``.
BINARY = f"#!/bin/sh\nprintf '%s\\n' '{BANNER}'\n".encode()

#: Stands in for ``curl``. It serves the fixture named by the URL's last path
#: segment, and refuses the way a host does: an ``.absent`` marker is a
#: transport failure with no status, an ``.http404`` marker is a delivered 404
#: with an empty body. A name with neither marker is a fixture a case forgot to
#: publish, so it fails as a transport error rather than serving other bytes.
CURL_STUB = """\
#!/bin/sh
set -eu
url=""
output=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --url) url="$2"; shift 2 ;;
    --output) output="$2"; shift 2 ;;
    --write-out) shift 2 ;;
    --connect-timeout|--max-time|--max-filesize|--max-redirs|--proto) shift 2 ;;
    --proto-redir|--retry|--retry-delay|--retry-max-time|--user-agent) shift 2 ;;
    *) shift ;;
  esac
done
printf '%s\\n' "$url" >> "$CURL_LOG"
name="${url##*/}"
if [ -f "$CURL_FIXTURES/$name" ]; then
  cp "$CURL_FIXTURES/$name" "$output"
  printf '200\\n'
  exit 0
fi
if [ -f "$CURL_FIXTURES/$name.http404" ]; then
  : > "$output"
  printf '404\\n'
  exit 0
fi
rm -f "$output"
exit 7
"""

#: The file suffix a case uses to have one asset answered with a 404.
HTTP404 = ".http404"
#: The file suffix a case uses to have one asset never reach the host.
ABSENT = ".absent"


def _digest(data: bytes) -> str:
    """Return the lowercase SHA-256 of ``data``."""
    return hashlib.sha256(data).hexdigest()


def archive_bytes(
    binary: bytes = BINARY, members: cabc.Sequence[str] = ("netsuke",)
) -> bytes:
    """Build a cargo-binstall archive holding ``binary`` at its first member.

    Parameters
    ----------
    binary
        The bytes the first member holds.
    members
        The member names to write, so a case can publish a layout the policy
        must refuse.

    Returns
    -------
    bytes
        The gzipped tar archive.
    """
    buffer = io.BytesIO()
    with tarfile.open(fileobj=buffer, mode="w:gz") as archive:
        for index, name in enumerate(members):
            member = tarfile.TarInfo(name)
            payload = binary if index == 0 else b""
            member.size = len(payload)
            member.mode = 0o755
            archive.addfile(member, io.BytesIO(payload))
    return buffer.getvalue()


def sidecar_bytes(archive: bytes, name: str = ARCHIVE_NAME) -> bytes:
    """Build the checksum sidecar the release publishes beside ``archive``."""
    return f"{_digest(archive)}  {name}\n".encode()


@dataclasses.dataclass(frozen=True, slots=True)
class Smoke:
    """Describe what a case hands the action and what the host publishes.

    Attributes
    ----------
    version
        The candidate version the package record and the archive name carry.
    binary
        The bytes the archive publishes as its first member.
    members
        The member names the published archive carries.
    served
        Asset names the host replaces, mapped to the bytes it serves instead.
        A replacement digest is not what the trusted record names, which is how
        a case models a substituted download.
    refused
        Asset names the host never delivers at all. Each gets an ``.absent``
        marker, so ``curl`` fails as a transport error with no status.
    not_found
        Asset names the host answers with a 404 and an empty body.
    inputs
        Action inputs a case overrides, keyed by the name the manifest declares.
    before_step
        Called with the next step's index and the work directory before that
        step runs, so a case can populate the directory an earlier attempt left
        behind.
    """

    version: str = VERSION
    binary: bytes = BINARY
    members: tuple[str, ...] = ("netsuke",)
    served: dict[str, bytes] = dataclasses.field(default_factory=dict)
    refused: frozenset[str] = frozenset()
    not_found: frozenset[str] = frozenset()
    inputs: dict[str, str] = dataclasses.field(default_factory=dict)
    before_step: BeforeStep | None = None


#: The candidate a case that names none publishes.
VERIFIED = Smoke()


@dataclasses.dataclass(frozen=True, slots=True)
class Published:
    """Hold the records a case wrote and the bytes the host serves.

    Attributes
    ----------
    identity
        The package record the packaging stage retained.
    trusted
        The pre-upload digest record, keyed by published asset name.
    fixtures
        The directory the ``curl`` stand-in serves from.
    work_dir
        The directory the action is told to keep its records in.
    """

    identity: pathlib.Path
    trusted: pathlib.Path
    fixtures: pathlib.Path
    work_dir: pathlib.Path


@dataclasses.dataclass(frozen=True, slots=True)
class Sandbox:
    """Hold the paths one executed run writes and reads.

    Attributes
    ----------
    stubs
        The directory holding the ``uv`` and ``curl`` stand-ins.
    scratch
        A writable home for the child processes, also holding their scripts.
    outputs_directory
        One ``GITHUB_OUTPUT`` file per step, so a step's entries can be read
        back as that step's alone.
    curl_log
        Every URL the stand-in was asked for, one per line.
    baseline_log
        Every interpreter the ``uv`` stand-in was told to use.
    """

    stubs: pathlib.Path
    scratch: pathlib.Path
    outputs_directory: pathlib.Path
    curl_log: pathlib.Path
    baseline_log: pathlib.Path


@dataclasses.dataclass(frozen=True, slots=True)
class ActionRun:
    """Report what executing the action's steps did.

    Attributes
    ----------
    completed
        Each step's completed process, in execution order. A step a runner
        would have skipped is absent, so the count names where the run stopped.
    step_outputs
        The ``GITHUB_OUTPUT`` entries each step wrote, keyed by the step's
        manifest id.
    work_dir
        The directory the action was told to keep its records in.
    curl_log
        Every URL the ``curl`` stand-in was asked for.
    baselines
        The interpreter each ``uv`` invocation was told to use.
    """

    completed: tuple[subprocess.CompletedProcess[str], ...]
    step_outputs: dict[str, dict[str, str]]
    work_dir: pathlib.Path
    curl_log: tuple[str, ...]
    baselines: tuple[str, ...]

    @property
    def outputs(self) -> dict[str, str]:
        """The entries the report step wrote, which are the action's outputs."""
        return self.step_outputs[manifest_step_id(REPORT_STEP)]

    def verdict(self) -> dict[str, object] | None:
        """Return the verdict record, or ``None`` when the run wrote none."""
        return _read_json(self.work_dir / "verdict.json")


def _read_json(path: pathlib.Path) -> dict[str, object] | None:
    """Return a JSON object a run wrote, or ``None`` when it is absent."""
    if not path.is_file():
        return None
    return json.loads(path.read_text(encoding="utf-8"))


def _publish(root: pathlib.Path, smoke: Smoke) -> Published:
    """Write the evidence records and the bytes a run publishes.

    Returns
    -------
    Published
        The two record paths, the fixture directory, and the work directory.
    """
    archive = archive_bytes(smoke.binary, smoke.members)
    sidecar = sidecar_bytes(archive)
    crate = f"the {PACKAGE} {smoke.version} crate\n".encode()
    fixtures = root / "fixtures"
    fixtures.mkdir()
    for name, body in (
        (CRATE_NAME, smoke.served.get(CRATE_NAME, crate)),
        (ARCHIVE_NAME, smoke.served.get(ARCHIVE_NAME, archive)),
        (SIDECAR_NAME, smoke.served.get(SIDECAR_NAME, sidecar)),
    ):
        _stage_fixture(fixtures, name, body, smoke)
    identity = root / "identity.json"
    identity.write_text(
        json.dumps({
            "crate": CRATE_NAME,
            "identity": {
                "tag": f"v{smoke.version}",
                "commit": COMMIT,
                "package_name": PACKAGE,
                "version": smoke.version,
                "bin_name": "netsuke",
                "is_prerelease": False,
            },
            "sha256": _digest(crate),
        }),
        encoding="utf-8",
    )
    trusted = root / "trusted.json"
    trusted.write_text(
        json.dumps({
            "assets": {ARCHIVE_NAME: _digest(archive), SIDECAR_NAME: _digest(sidecar)}
        }),
        encoding="utf-8",
    )
    return Published(
        identity=identity,
        trusted=trusted,
        fixtures=fixtures,
        work_dir=root / "runner" / "work",
    )


def _stage_fixture(
    fixtures: pathlib.Path, name: str, body: bytes, smoke: Smoke
) -> None:
    """Write one asset's bytes, or the marker that stands in for a refusal."""
    if name in smoke.refused:
        (fixtures / f"{name}{ABSENT}").touch()
    elif name in smoke.not_found:
        (fixtures / f"{name}{HTTP404}").touch()
    else:
        (fixtures / name).write_bytes(body)


def _sandbox(root: pathlib.Path) -> Sandbox:
    """Create the stubs and logs one run needs.

    Returns
    -------
    Sandbox
        The paths the run writes.
    """
    stubs = root / "stubs"
    stubs.mkdir()
    write_stub(stubs / "uv", UV_STUB)
    write_stub(stubs / "curl", CURL_STUB)
    scratch = root / "scratch"
    scratch.mkdir()
    baseline_log = root / "uv-baselines.txt"
    baseline_log.touch()
    curl_log = root / "curl.log"
    curl_log.touch()
    outputs_directory = root / "github-output"
    outputs_directory.mkdir()
    return Sandbox(
        stubs=stubs,
        scratch=scratch,
        outputs_directory=outputs_directory,
        curl_log=curl_log,
        baseline_log=baseline_log,
    )


def _environment(sandbox: Sandbox, published: Published) -> dict[str, str]:
    """Return the environment every step inherits.

    ``GITHUB_OUTPUT`` is deliberately absent: a step's output file is that
    step's own, and :func:`execute` supplies it per step.

    Returns
    -------
    dict[str, str]
        The base environment.
    """
    return {
        "PATH": f"{sandbox.stubs}:/usr/bin:/bin",
        "HOME": str(sandbox.scratch),
        "TMPDIR": str(sandbox.scratch),
        "PYTHONDONTWRITEBYTECODE": "1",
        "UV_BASELINE_LOG": str(sandbox.baseline_log),
        "UV_PYTHON": sys.executable,
        "CURL_LOG": str(sandbox.curl_log),
        "CURL_FIXTURES": str(published.fixtures),
    }


def _default_inputs(published: Published) -> dict[str, str]:
    """Return the inputs a case that overrides none supplies.

    Returns
    -------
    dict[str, str]
        The six declared inputs, naming this run's evidence and workspace.
    """
    return {
        "identity-path": str(published.identity),
        "asset-digest-record": str(published.trusted),
        "repository": "leynos/netsuke",
        "commit": COMMIT,
        "target": TARGET,
        "work-dir": str(published.work_dir),
    }


@dataclasses.dataclass(slots=True)
class _Attempt:
    """Hold what one executed run carries from one step to the next."""

    sandbox: Sandbox
    published: Published
    inputs: dict[str, str]
    base_environment: dict[str, str]
    steps_dir: pathlib.Path
    outputs: dict[str, dict[str, str]] = dataclasses.field(default_factory=dict)
    outcomes: dict[str, str] = dataclasses.field(default_factory=dict)
    completed: list[subprocess.CompletedProcess[str]] = dataclasses.field(
        default_factory=list
    )


def _start_step(step: dict[str, object], number: int, attempt: _Attempt) -> None:
    """Run one step, or record that a runner would skip it."""
    step_id = manifest_step_id(str(step.get("name", "")))
    outputs_path = output_file(attempt.sandbox.outputs_directory, step_id)
    if not should_run(
        step, outcomes=attempt.outcomes, previous=list(attempt.outcomes.values())
    ):
        attempt.outcomes[step_id] = "skipped"
        return
    script = attempt.steps_dir / f"{number}.sh"
    script.write_text(str(step.get("run", "")), encoding="utf-8")
    environment = attempt.base_environment | step_environment(
        step,
        inputs=attempt.inputs,
        outputs=attempt.outputs,
        outcomes=attempt.outcomes,
    )
    environment["GITHUB_OUTPUT"] = str(outputs_path)
    result = run_shell(REPO_ROOT, script, environment)
    attempt.outcomes[step_id] = "success" if result.returncode == 0 else "failure"
    attempt.completed.append(result)
    attempt.outputs = read_step_outputs(attempt.sandbox.outputs_directory)


def execute(tmp_path: pathlib.Path, *, smoke: Smoke = VERIFIED) -> ActionRun:
    """Run the action's steps in order, stopping where a runner would.

    Parameters
    ----------
    tmp_path
        The directory the evidence, the served bytes, and the run's records
        are written under.
    smoke
        The candidate the host publishes and the inputs a case overrides.

    Returns
    -------
    ActionRun
        The completed steps, the outputs each step wrote, and the calls the
        stand-ins recorded.

    Notes
    -----
    Each step runs with the repository root as its working directory, because
    that is the tree a runner's checkout puts the action in and the checked-in
    CLI resolves its default staging configuration and manifest from there.
    Nothing the steps write lands in that tree: the work directory, the
    downloads, and the extracted binary are all under ``tmp_path``.
    """
    published = _publish(tmp_path, smoke)
    sandbox = _sandbox(tmp_path)
    steps_dir = sandbox.scratch / "steps"
    steps_dir.mkdir()
    attempt = _Attempt(
        sandbox=sandbox,
        published=published,
        inputs=_default_inputs(published) | smoke.inputs,
        base_environment=_environment(sandbox, published),
        steps_dir=steps_dir,
    )
    for number, step in enumerate(smoke_steps()):
        if smoke.before_step is not None:
            smoke.before_step(number, published.work_dir)
        _start_step(step, number, attempt)
    return ActionRun(
        completed=tuple(attempt.completed),
        step_outputs=read_step_outputs(sandbox.outputs_directory),
        work_dir=published.work_dir,
        curl_log=tuple(sandbox.curl_log.read_text(encoding="utf-8").splitlines()),
        baselines=tuple(sandbox.baseline_log.read_text(encoding="utf-8").split()),
    )


__all__ = [
    "ABSENT",
    "ARCHIVE_NAME",
    "BANNER",
    "BINARY",
    "COMMIT",
    "CRATE_NAME",
    "HTTP404",
    "PACKAGE",
    "SIDECAR_NAME",
    "TARGET",
    "VERIFIED",
    "VERSION",
    "ActionRun",
    "Published",
    "Sandbox",
    "Smoke",
    "archive_bytes",
    "execute",
    "sidecar_bytes",
]
