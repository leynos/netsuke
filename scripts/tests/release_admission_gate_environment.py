"""Build the isolated environment one real release-admission run starts with.

The gate is exercised as a real process, so everything it reads from its
environment has to be handed to it deliberately: the shim directory that makes
the doubles reachable, the file paths it writes its records and outputs to, and
the case's own overrides. This module owns that construction, and
:mod:`release_admission_test_harness` owns launching the process with the
result.

The isolation is the point. Every admission variable the parent exported is
dropped rather than inherited, because a developer with
``NETSUKE_RELEASE_ADMISSION_TAG`` exported would otherwise turn the default
rehearsal into a tagged run, and the resulting failure would read as a defect in
the gate rather than as a contaminated environment.
"""

import dataclasses
import os
import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    from pathlib import Path

    from cmd_mox import CmdMox

from release_admission_test_doubles import GITHUB_REPOSITORY, REVISION, WORKFLOW_RUN_ID

#: The variable the child reads its evidence manifest from. The harness owns
#: this spelling because it must write the fixture wherever the child is told
#: to look, and a case may point the child somewhere else entirely.
EVIDENCE_FILE_VARIABLE = "NETSUKE_RELEASE_ADMISSION_EVIDENCE_FILE"

#: The prefix every admission control shares. An inherited name under it is
#: dropped before the case's own values are applied: the harness runs one
#: declared scenario, and a developer's exported tag, mode, bound, or adapter
#: would otherwise decide what a case exercises.
ADMISSION_VARIABLE_PREFIX = "NETSUKE_RELEASE_ADMISSION_"


@dataclasses.dataclass(frozen=True, slots=True)
class GateInputs:
    """Hold the runner-local inputs one gate invocation is configured from.

    These four values travel together -- the isolated directory, the evidence
    state the fixture supplies, the case's own overrides, and the named output
    paths -- so they are one value rather than a five-argument call.
    """

    tmp_path: Path
    evidence_state: str
    extra_environment: cabc.Mapping[str, str | None] | None
    paths: dict[str, Path]


def gate_environment(
    cmd_mox: CmdMox,
    inputs: GateInputs,
    inherited: cabc.Mapping[str, str] | None = None,
) -> dict[str, str]:
    """Build an isolated environment for one real gate invocation.

    The PATH is the controller's own: cmd-mox prepends its shim directory when
    it enters, so this copy puts the shims ahead of any real ``gh`` or ``git``
    on the host, and a scenario must never replace it. A shim's shebang
    resolves ``python3`` through the child's PATH, so a child started without
    one cannot run the shim at all -- which reads as an unexplained missing
    invocation rather than as the configuration error it is.

    Every admission variable the parent exported is dropped rather than
    inherited, because the child reads its tag, mode, bounds, adapters, and
    sinks from this harness and from the case itself: a developer with
    ``NETSUKE_RELEASE_ADMISSION_TAG`` exported would otherwise turn the default
    rehearsal into a tagged run, and the resulting failure would read as a
    defect in the gate.

    Parameters
    ----------
    cmd_mox
        Active controller whose shim directory leads the child PATH.
    inputs
        Runner-local inputs: the isolated directory, the fixture state, the
        case's own overrides, and the named output paths.
    inherited
        The environment to inherit from before the fixed inputs and the case's
        overrides are applied; the process environment by default.

    Returns
    -------
    dict[str, str]
        Environment mapping passed to the admission subprocess.

    Notes
    -----
    Contract invariants: the child inherits the controller's environment, so
    the shim directory and the IPC socket reach it, and production values are
    preserved unless overridden. An absent shim directory means this helper
    ran outside a live controller: a test fault, not a condition to report.
    """
    shim_dir = cmd_mox.environment.shim_dir
    assert shim_dir is not None, "CmdMox must create its command shim directory"
    base = os.environ if inherited is None else inherited
    paths = inputs.paths
    # The overrides are merged below, where a ``None`` *removes* a name rather
    # than being stored: the returned mapping therefore holds only ``str``
    # values, and the annotation is what says so rather than a narrowed view a
    # reader would have to reconstruct.
    environment: dict[str, str] = {
        name: value
        for name, value in base.items()
        if not name.startswith(ADMISSION_VARIABLE_PREFIX)
    }
    environment.update({
        "GITHUB_OUTPUT": str(paths["output"]),
        "GITHUB_REPOSITORY": GITHUB_REPOSITORY,
        "GITHUB_RUN_ID": WORKFLOW_RUN_ID,
        "GITHUB_SHA": REVISION,
        EVIDENCE_FILE_VARIABLE: str(paths["evidence"]),
        "NETSUKE_RELEASE_ADMISSION_METRICS_FILE": str(paths["metrics"]),
        "NETSUKE_RELEASE_ADMISSION_TRACE_FILE": str(paths["trace"]),
        "PATH": os.pathsep.join([str(shim_dir), base["PATH"]]),
        "RUNNER_TEMP": str(inputs.tmp_path),
    })
    # Mapping a name to ``None`` *unsets* it, which is not the same as mapping
    # it to ``""``: the gate's colon-form default substitutes for an empty
    # value, so only an unset variable reaches the earlier arm.
    for name, value in (inputs.extra_environment or {}).items():
        if value is None:
            environment.pop(name, None)
        else:
            environment[name] = value
    return environment
