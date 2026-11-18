"""Hold the consumer-smoke action to the commands it is allowed to run.

The composite action is the only thing that downloads the published bytes and
the only thing that runs the verifier, and no other contract reads its command
lines. Without the checks here an action that kept its report step and dropped
or weakened its verification would satisfy every other shape check, and the
step that was supposed to fail would quietly pass.

Four properties are asserted: the verifier is invoked exactly as reviewed, the
invocation is reachable, its refusal reaches the step as a failure, and nothing
in the action builds from source. All four read what bash would run rather than
what a comment says, so documentation naming a compiler or a flag cannot be
mistaken for an invocation.

Reachability and failure propagation are read as a shape rather than as a
search for one suspicious line, using the readers in
``release_publish_path_consumer_script``: a conditional, a loop, a bare
command, or a ``set +e`` above the invocation can skip it or stop it failing,
and a success exit below it can absorb the refusal even when the invocation
itself is byte-for-byte correct.

Run via ``make test-workflow-contracts``.
"""

import re
import typing as typ

from release_publish_path_consumer_script import (
    ends_after_smoke_cli,
    hides_a_reference,
    invocation,
    invoked_phase,
    unsupported_prologue,
)
from release_publish_path_consumer_shape import (
    CONSUMER_SMOKE_ACTION_PATH,
    CONSUMER_SMOKE_JOB,
    SMOKE_CLI,
    action_steps,
    command_lines,
    steps_of,
)

if typ.TYPE_CHECKING:
    from release_publish_path_consumer_script import Invocation

#: The evidence flags both invocations must pass, in the reviewed order.
_EVIDENCE_ARGUMENTS = (
    "--identity",
    "$IDENTITY_PATH",
    "--trusted",
    "$TRUSTED_RECORD_PATH",
    "--repository",
    "$RELEASE_REPOSITORY",
    "--target",
    "$RELEASE_TARGET",
    "--commit",
    "$RELEASE_COMMIT",
)
#: The exact tokens each subcommand must be given, compared literally. A
#: dropped bound, a swapped phase, or an added flag fails the contract, because
#: the arguments are what decide which bytes the smoke judges.
SMOKE_ACTION_ARGUMENTS = {
    "plan": (
        SMOKE_CLI,
        "plan",
        *_EVIDENCE_ARGUMENTS,
        "--output",
        "$WORK_DIRECTORY/plan.json",
    ),
    "verify": (
        SMOKE_CLI,
        "verify",
        *_EVIDENCE_ARGUMENTS,
        "--plan",
        "$WORK_DIRECTORY/plan.json",
        "--crate",
        "$downloads/$CRATE_NAME",
        "--crate-status",
        "$downloads/$CRATE_NAME.status",
        "--archive",
        "$downloads/$ARCHIVE_NAME",
        "--archive-status",
        "$downloads/$ARCHIVE_NAME.status",
        "--sidecar",
        "$downloads/$SIDECAR_NAME",
        "--sidecar-status",
        "$downloads/$SIDECAR_NAME.status",
        "--extract-dir",
        "$EXTRACT_DIRECTORY",
        "--verdict",
        "$WORK_DIRECTORY/verdict.json",
        "--phase-log",
        "$WORK_DIRECTORY/phase-log.json",
    ),
}
#: How each invocation is launched: the step bounds it with ``timeout`` and
#: reaches the PEP 723 script through ``uv run``. The launcher is part of the
#: contract because ``echo`` in front of the reviewed arguments runs nothing,
#: so a check that compared only the arguments would accept a step that merely
#: printed its invocation.
_UV_LAUNCHER = ("uv", "run", "--no-project", "--python", "3.14")
SMOKE_ACTION_LAUNCHERS = {
    "plan": ("timeout", "--kill-after=30s", "300s", *_UV_LAUNCHER),
    "verify": ("timeout", "--kill-after=30s", "600s", *_UV_LAUNCHER),
}
#: A whole-word Rust build command. A comment may name the archive format, so
#: only executable lines are read; a command line naming any of these would let
#: the smoke build what it is supposed to download.
COMPILATION_COMMAND = re.compile(r"\b(?:cargo|rustc|rustup)\b")


def check_smoke_action_commands(
    action: dict[str, object] | None, violations: list[str]
) -> None:
    """Require both invocations to be the reviewed ones, from unconditioned steps.

    A step-level ``if`` can stop the verifier while the job guard, the
    dependencies, and the inputs all still read as correct, and a missing or
    rewritten invocation can leave the smoke reporting a release it never
    checked. Each subcommand must therefore appear exactly once, carry no
    condition, and run exactly the reviewed command line, launcher included.
    """
    if action is None:
        return
    for phase, arguments in SMOKE_ACTION_ARGUMENTS.items():
        expected = (*SMOKE_ACTION_LAUNCHERS[phase], *arguments)
        _require_invocation(phase, expected, action_steps(action), violations)


def _calls_of_phase(
    phase: str, steps: list[dict[str, object]]
) -> list[tuple[dict[str, object], Invocation]]:
    """Return every step whose invocation runs one phase of the smoke CLI.

    Returns
    -------
    list[tuple[dict[str, object], Invocation]]
        One entry per step that runs the CLI with this phase, so the caller can
        require the count without re-reading the steps.
    """
    return [
        (step, call)
        for step in steps
        if (call := invocation(step)) is not None
        and invoked_phase(call.tokens) == phase
    ]


def _require_command_shape(
    label: str,
    expected: tuple[str, ...],
    call: Invocation,
    violations: list[str],
) -> None:
    """Require the invocation's words to be exact and expansion-safe."""
    if call.tokens != expected:
        violations.append(
            f"{label} must run exactly {list(expected)}, got {list(call.tokens)}"
        )
    if hides_a_reference(call.line):
        violations.append(
            f"{label} must let bash expand its evidence, but {call.line!r} "
            "single-quotes a reference"
        )


def _require_script_shape(
    label: str, step: dict[str, object], violations: list[str]
) -> None:
    """Require the script around the invocation to reach it and propagate it."""
    unreachable = unsupported_prologue(step)
    if unreachable is not None:
        violations.append(
            f"{label} is unreachable: {unreachable!r} may skip it or disarm it"
        )
    absorbed = ends_after_smoke_cli(step)
    if absorbed is not None:
        violations.append(
            f"{label} must propagate its refusal, but {absorbed!r} ends the "
            "step successfully afterwards"
        )


def _require_invocation(
    phase: str,
    expected: tuple[str, ...],
    steps: list[dict[str, object]],
    violations: list[str],
) -> None:
    """Require one subcommand to run once, unconditionally, exactly, and so it fails."""
    label = f"{CONSUMER_SMOKE_JOB}.action: the {phase} invocation"
    found = _calls_of_phase(phase, steps)
    if len(found) != 1:
        violations.append(f"{label} must appear exactly once, found {len(found)}")
        return
    step, call = found[0]
    if "if" in step:
        violations.append(
            f"{label} must run whenever the action does, got if: {step['if']!r}"
        )
    _require_command_shape(label, expected, call, violations)
    _require_script_shape(label, step, violations)


def check_smoke_compilation(
    job: dict[str, object],
    action: dict[str, object] | None,
    violations: list[str],
) -> None:
    """Refuse a build command anywhere in the job or the action it calls.

    The smoke exists to run the bytes a consumer receives. A compiler here
    would replace the download with a build, and ``cargo install netsuke-build``
    is the specific substitution this check was written for.
    """
    sources: list[tuple[str, object]] = [
        (f"{CONSUMER_SMOKE_JOB} step {index}", step.get("run"))
        for index, step in enumerate(steps_of(job))
    ]
    if action is not None:
        sources.extend(
            (f"{CONSUMER_SMOKE_ACTION_PATH} step {index}", step.get("run"))
            for index, step in enumerate(action_steps(action))
        )
    for label, run in sources:
        offending = [
            line for line in command_lines(run) if COMPILATION_COMMAND.search(line)
        ]
        if offending:
            violations.append(
                f"{CONSUMER_SMOKE_JOB}.compilation: {label} builds instead of "
                f"downloading, got {offending[0]!r}"
            )


__all__ = [
    "COMPILATION_COMMAND",
    "SMOKE_ACTION_ARGUMENTS",
    "SMOKE_ACTION_LAUNCHERS",
    "SMOKE_CLI",
    "check_smoke_action_commands",
    "check_smoke_compilation",
]
