"""Run the release-admission gate and decode the records one run produced.

This module owns the subprocess boundary the release-admission runtime tests
exercise: it invokes the gate under test and returns the completed process
alongside the three artefact sets it wrote. The environment that process starts
with is built by :mod:`release_admission_gate_environment`. The frozen
measurements those tests are built on live in
:mod:`release_admission_test_support`, which re-exports everything here as the
facade the runtime modules import from, and the doubles the child reaches are
registered by :mod:`release_admission_test_doubles`.

The gate is a real process because only a process exercises the whole contract:
the exit status the workflow reads, the artefact files the gate creates for
itself, and the redirections it performs. Most executables it runs are cmd-mox
shims, so the boundary under test is the one production reaches while every
external effect stays inside the test.

The candidate fetch is the exception, and :func:`run_gate` supports it through
``cwd`` and an absolute ``GIT_ADAPTER``. No shim can model which repository
``origin`` names -- a shim's recorded invocation has no working directory -- so
a test that must distinguish the candidate checkout from the verifier checkout
builds real repositories in its own temporary tree and points the adapter at the
host's own ``git``. The mutation stays inside that tree: the repositories are
bare remotes and clones the test created for the run.

The *shebang* is not one of those properties. :func:`run_gate` launches
``[str(PYTHON_PATH), str(SCRIPT_PATH)]`` -- the interpreter by absolute path,
with the script as its first argument -- so the shebang ``uv`` honours is never
consulted and this harness cannot vouch for it. That contract is asserted
statically instead, by
:func:`test_release_admission_metrics.test_entry_point_declares_the_runtime_contract_the_workflow_needs`,
which reads the first line of the file and its executable bit.
"""

import dataclasses
import hashlib
import subprocess  # ruff: ignore[suspicious-subprocess-import] - the script boundary is under test.
import typing as typ
from pathlib import Path

if typ.TYPE_CHECKING:
    import collections.abc as cabc

from release_admission_evidence_fixtures import (
    ATTRIBUTED,
    EVIDENCE_DIGEST_VARIABLE,
    MISSING,
    PRODUCER_TRUST_VARIABLE,
    candidate_for,
    write_evidence,
)
from release_admission_gate_environment import (
    EVIDENCE_FILE_VARIABLE,
    GateInputs,
    gate_environment,
)
from release_admission_test_doubles import PYTHON_PATH, SCRIPT_PATH, GateRun
from release_admission_validator import METRICS_VALIDATOR

if typ.TYPE_CHECKING:
    from cmd_mox import CmdMox


@dataclasses.dataclass(frozen=True, slots=True)
class GateRunRequest:
    """Hold the values one gate invocation is launched from.

    Most cases vary only the temporary directory, the evidence state, and their
    own environment overrides, and :func:`run_gate` takes exactly those. The
    working directory is the fourth choice, and only a case that models two
    checkouts makes it: it is what decides which repository ``origin`` names
    for the gate's candidate fetch, so it must be a value that case states
    rather than an ambient property of the test process.
    """

    tmp_path: Path
    evidence_state: str = MISSING
    extra_environment: cabc.Mapping[str, str | None] | None = None
    cwd: Path | None = None


def run_gate(
    cmd_mox: CmdMox,
    tmp_path: Path,
    *,
    evidence_state: str = MISSING,
    extra_environment: cabc.Mapping[str, str | None] | None = None,
) -> GateRun:
    """Run the production gate from the test process's own directory.

    Parameters
    ----------
    cmd_mox
        Active controller, already in replay, whose shims are on the child
        ``PATH``.
    tmp_path
        Isolated directory for inputs and outputs.
    evidence_state
        Which evidence manifest to write at the path the child is told to read,
        named from :mod:`release_admission_evidence_fixtures`. The default
        writes nothing, which is the state before a producer exists.
    extra_environment
        Optional child-process environment overrides for a test scenario. A
        value of ``None`` removes the variable instead of setting it, which is
        how a case expresses *unset* rather than *empty*.

    Returns
    -------
    GateRun
        The completed process and the records decoded from the files it wrote.

    Notes
    -----
    This is :func:`run_gate_request` for the common case, which keeps the test
    process's working directory. A case that must choose the working directory
    -- because it models a second checkout -- builds the request itself.
    """
    return run_gate_request(
        cmd_mox,
        GateRunRequest(
            tmp_path,
            evidence_state=evidence_state,
            extra_environment=extra_environment,
        ),
    )


def run_gate_request(cmd_mox: CmdMox, request: GateRunRequest) -> GateRun:
    """Run the production gate for one request and collect what it wrote.

    Parameters
    ----------
    cmd_mox
        Active controller, already in replay, whose shims are on the child
        ``PATH``.
    request
        The temporary directory, evidence state, environment overrides, and
        working directory the run uses.

    Returns
    -------
    GateRun
        The completed process and the records decoded from the files it wrote.

    Notes
    -----
    Contract invariants: the child receives an isolated environment, the only
    ``gh`` and ``git`` it can reach are the shims, its standard input is closed,
    and every returned record is decoded from a file the gate itself produced.
    The tag is left unset unless a case supplies it, so the default run is the
    rehearsal form: the revision resolves and compares with itself.
    """
    paths = gate_paths(request.tmp_path)
    environment = gate_environment(
        cmd_mox,
        GateInputs(
            request.tmp_path,
            request.evidence_state,
            request.extra_environment,
            paths,
        ),
    )
    document = write_evidence(
        _evidence_path(environment, paths, request.cwd),
        request.evidence_state,
        candidate_for(environment),
    )
    if environment.get(EVIDENCE_DIGEST_VARIABLE) == ATTRIBUTED:
        environment[EVIDENCE_DIGEST_VARIABLE] = hashlib.sha256(document).hexdigest()
        # An attributed run is one the block no longer withholds; a case about
        # the block itself states the variable, which this leaves alone.
        environment.setdefault(PRODUCER_TRUST_VARIABLE, "true")
    result = subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - fixed test target.
        [str(PYTHON_PATH), str(SCRIPT_PATH)],
        capture_output=True,
        check=False,
        cwd=request.cwd,
        env=environment,
        stdin=subprocess.DEVNULL,
        text=True,
    )
    metrics, traces = _read_gate_records(paths)
    outputs = _read_workflow_outputs(paths)
    return GateRun(result, metrics, traces, outputs, paths)


def _evidence_path(
    environment: dict[str, str],
    paths: dict[str, Path],
    cwd: Path | None,
) -> Path:
    """Return the evidence path the child under test will actually read.

    A case may redirect the child's evidence file, and the fixture has to be
    written where that case pointed it. Writing to the harness's own default
    instead leaves the redirected path absent, so a state the case asked for
    arrives as a *missing evidence* refusal -- a failure that names the
    document rather than the harness defect producing it.

    Parameters
    ----------
    environment
        The merged child environment, after the case's overrides.
    paths
        Named output paths returned by :func:`gate_paths`.
    cwd
        The working directory the child starts in, or ``None`` when it
        inherits this process's. A relative configured value is resolved
        against it, because that is where the child resolves it.

    Returns
    -------
    Path
        The configured evidence path, or the harness default when the
        variable is unset or empty.

    Notes
    -----
    Neither remaining case can be decided by the fixture's location: an
    enforcing run refuses the configuration outright without an evidence file,
    and a rehearsal reads no evidence at all.
    """
    configured = environment.get(EVIDENCE_FILE_VARIABLE, "")
    if not configured:
        return paths["evidence"]
    return Path.cwd() / configured if cwd is None else cwd / configured


def _read_workflow_outputs(paths: dict[str, Path]) -> dict[str, str]:
    """Decode the workflow outputs one run wrote, empty when it wrote none.

    Parameters
    ----------
    paths
        Named output paths returned by :func:`gate_paths`.

    Returns
    -------
    dict[str, str]
        The ``key=value`` lines in the order the gate wrote them.

    Notes
    -----
    An absent file is a meaningful state, not a fault: the runner creates
    ``GITHUB_OUTPUT`` before the step, so a run that stops earlier leaves it
    exactly as it found it. In this harness nothing pre-creates it, so such a
    run leaves nothing to read, and a case asserting no outputs were written
    asserts on this empty mapping.
    """
    path = paths["output"]
    if not path.exists():
        return {}
    return dict(
        line.split("=", maxsplit=1)
        for line in path.read_text(encoding="utf-8").splitlines()
    )


def gate_paths(tmp_path: Path) -> dict[str, Path]:
    """Name the files one gate run writes into.

    Parameters
    ----------
    tmp_path
        Isolated directory that owns the gate's files.

    Returns
    -------
    dict[str, Path]
        Named paths for the JSON Lines outputs and the workflow output.

    Notes
    -----
    Contract invariant: every returned path is confined to ``tmp_path``.
    """
    return {
        "metrics": tmp_path / "release-admission-metrics.jsonl",
        "trace": tmp_path / "release-admission-traces.jsonl",
        "evidence": tmp_path / "release-admission-evidence.json",
        "output": tmp_path / "github-output",
    }


def _read_gate_records(
    paths: dict[str, Path],
) -> tuple[list[dict[str, object]], list[dict[str, object]]]:
    """Read parsed metric and trace records from one run.

    Parameters
    ----------
    paths
        Named output paths returned by :func:`gate_paths`.

    Returns
    -------
    tuple
        Parsed metric records and parsed trace records.

    Notes
    -----
    Contract invariant: JSON Lines are decoded through the shared validator so
    runtime tests exercise the same schema as workflow-contract tests.

    An absent file reads as no records. Neither artefact exists when a required
    variable aborts the gate before the two paths are computed, a state the
    tests assert on deliberately: the shell refused ``GITHUB_REPOSITORY`` and
    ``GITHUB_SHA`` before any artefact was created. Raising
    :class:`FileNotFoundError` would make that run unpresentable rather than
    assertable, so the file's existence is a separate, explicit assertion.
    """
    return (
        _parse_records(paths["metrics"], METRICS_VALIDATOR.parse_metrics),
        _parse_records(paths["trace"], METRICS_VALIDATOR.parse_traces),
    )


def _parse_records(
    path: Path,
    parse: cabc.Callable[[list[str]], list[dict[str, object]]],
) -> list[dict[str, object]]:
    """Parse one JSON Lines artefact, or report it as empty when absent.

    Parameters
    ----------
    path
        The artefact file one run may or may not have created.
    parse
        The shared validator's parser for this artefact's schema.

    Returns
    -------
    list[dict[str, object]]
        The records, or an empty list when the file does not exist.
    """
    if not path.exists():
        return []
    return parse(path.read_text(encoding="utf-8").splitlines())


__all__ = (
    "EVIDENCE_FILE_VARIABLE",
    "METRICS_VALIDATOR",
    "PYTHON_PATH",
    "SCRIPT_PATH",
    "GateInputs",
    "GateRun",
    "GateRunRequest",
    "gate_environment",
    "gate_paths",
    "run_gate",
    "run_gate_request",
)
