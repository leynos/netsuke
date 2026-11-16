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
search for one suspicious line. The script may prepare the invocation with
``set -…`` and plain assignments and nothing else: a conditional, a loop, a
bare command, or a ``set +e`` above it can skip the invocation or stop it
failing, and a success exit below it can absorb the refusal even when the
invocation itself is byte-for-byte correct.

Run via ``make test-workflow-contracts``.
"""

import re
import shlex

from release_publish_path_consumer_shape import (
    CONSUMER_SMOKE_ACTION_PATH,
    CONSUMER_SMOKE_JOB,
    action_steps,
    command_lines,
    steps_of,
)

#: The CLI both invocations must run, and the evidence flags both pass.
SMOKE_CLI = "scripts/consumer_smoke_cli.py"
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
#: The only lines a step may run before the reviewed invocation: an assignment
#: of one variable, or a ``set`` that enables options. Shell separators and
#: substitutions that run a command are excluded from an assignment's value, so
#: a line cannot both assign and run something.
_ASSIGNMENT = re.compile(r"[A-Za-z_][A-Za-z0-9_]*=[^;&|`]*")
_ENABLING_SET = re.compile(r"set -[A-Za-z]+(?: [A-Za-z][A-Za-z0-9_]*)*")


def smoke_command_line(step: dict[str, object]) -> tuple[str, ...] | None:
    """Return the complete command line a step runs for the smoke CLI.

    Returns
    -------
    tuple[str, ...] | None
        Every token of the first line that runs :data:`SMOKE_CLI`, launcher
        included, or ``None`` when no line does. A line that cannot be
        tokenized is not a command and is skipped.
    """
    for line in command_lines(step.get("run")):
        try:
            tokens = tuple(shlex.split(line))
        except ValueError:
            continue
        if SMOKE_CLI in tokens:
            return tokens
    return None


def invoked_phase(tokens: tuple[str, ...] | None) -> str | None:
    """Return the subcommand one tokenised command line runs, or ``None``.

    Returns
    -------
    str | None
        The token after :data:`SMOKE_CLI`, which names the phase the CLI is
        asked to run, or ``None`` when the line does not run the CLI or names
        no phase after it.
    """
    if tokens is None:
        return None
    for index, token in enumerate(tokens):
        if token == SMOKE_CLI:
            return tokens[index + 1] if index + 1 < len(tokens) else None
    return None


def ends_successfully(line: str) -> bool:
    """Return whether one shell command line exits the step successfully.

    Returns
    -------
    bool
        True for a bare ``exit`` or ``return``, or one naming status ``0``.
        A non-zero status fails the step instead of skipping the rest of it,
        so it makes no command unreachable.

    Examples
    --------
    >>> ends_successfully("exit 0"), ends_successfully("exit 1")
    (True, False)
    """
    try:
        tokens = tuple(shlex.split(line))
    except ValueError:
        return False
    return tokens[:1] in {("exit",), ("return",)} and tokens[1:] in {(), ("0",)}


def _is_prologue(line: str) -> bool:
    """Return whether one line only prepares the step for the invocation.

    Returns
    -------
    bool
        True for an assignment of one variable or a ``set`` that enables
        options, which are the two things a step needs before the command it
        exists to run. Everything else — a conditional, a loop, a bare command,
        or a ``set +e`` — can decide whether the invocation runs at all.
    """
    return (
        _ASSIGNMENT.fullmatch(line) is not None
        or _ENABLING_SET.fullmatch(line) is not None
    )


def unsupported_prologue(step: dict[str, object]) -> str | None:
    r"""Return the first line before the CLI that could skip or disarm it.

    The reviewed invocation is one command inside a script, so an exact match
    on that line says nothing about the script around it. ``exit 0`` above it
    makes it unreachable, ``if false; then`` above it confines it to a branch
    bash never takes, and ``set +e`` above it lets a refusal fall through to an
    ``exit 0`` below. Only a prologue that plainly prepares the step is
    accepted, so a construct nobody reviewed is refused rather than assumed
    harmless.

    Returns
    -------
    str | None
        The first line ahead of :data:`SMOKE_CLI` that is not a plain prologue,
        or ``None`` when every line ahead of the invocation prepares the step.

    Examples
    --------
    >>> step = {"run": "set -e\nexit 0\nuv run x scripts/consumer_smoke_cli.py"}
    >>> unsupported_prologue(step)
    'exit 0'
    """
    for line in command_lines(step.get("run")):
        if SMOKE_CLI in line:
            return None
        if not _is_prologue(line):
            return line
    return None


def ends_after_smoke_cli(step: dict[str, object]) -> str | None:
    """Return the line that ends a step successfully after the CLI runs.

    A success exit below the invocation absorbs the refusal: with the shell's
    fail-fast disabled, or never enabled, a verifier that exits non-zero is
    followed by ``exit 0`` and the step reports success.

    Returns
    -------
    str | None
        The first success exit that follows :data:`SMOKE_CLI`, or ``None`` when
        nothing after the invocation would end the step successfully.
    """
    lines = command_lines(step.get("run"))
    position = next(
        (index for index, line in enumerate(lines) if SMOKE_CLI in line), None
    )
    if position is None:
        return None
    return next(
        (line for line in lines[position + 1 :] if ends_successfully(line)), None
    )


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


def _require_invocation(
    phase: str,
    expected: tuple[str, ...],
    steps: list[dict[str, object]],
    violations: list[str],
) -> None:
    """Require one subcommand to run once, unconditionally, exactly, and so it fails."""
    label = f"{CONSUMER_SMOKE_JOB}.action: the {phase} invocation"
    found = [
        (step, tokens)
        for step in steps
        if (tokens := smoke_command_line(step)) is not None
        and invoked_phase(tokens) == phase
    ]
    if len(found) != 1:
        violations.append(f"{label} must appear exactly once, found {len(found)}")
        return
    step, tokens = found[0]
    if "if" in step:
        violations.append(
            f"{label} must run whenever the action does, got if: {step['if']!r}"
        )
    if tokens != expected:
        violations.append(
            f"{label} must run exactly {list(expected)}, got {list(tokens)}"
        )
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
    "ends_after_smoke_cli",
    "ends_successfully",
    "invoked_phase",
    "smoke_command_line",
    "unsupported_prologue",
]
