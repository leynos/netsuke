"""Hold the consumer-smoke action to the commands it is allowed to run.

The composite action is the only thing that downloads the published bytes and
the only thing that runs the verifier, and no other contract reads its command
lines. Without the checks here an action that kept its report step and dropped
or weakened its verification would satisfy every other shape check, and the
step that was supposed to fail would quietly pass.

Two properties are asserted: the verifier is invoked exactly as reviewed, and
nothing in the action builds from source. Both read what bash would run rather
than what a comment says, so documentation naming a compiler or a flag cannot
be mistaken for an invocation.

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
#: A whole-word Rust build command. A comment may name the archive format, so
#: only executable lines are read; a command line naming any of these would let
#: the smoke build what it is supposed to download.
COMPILATION_COMMAND = re.compile(r"\b(?:cargo|rustc|rustup)\b")


def action_cli_invocation(step: dict[str, object]) -> tuple[str, ...] | None:
    """Return the smoke-CLI command line a step runs, or ``None``.

    Returns
    -------
    tuple[str, ...] | None
        The tokens from :data:`SMOKE_CLI` to the end of the first command line
        that runs it, or ``None`` when no line does. The path is stripped of
        the wrapper that launches it — ``timeout`` and ``uv run`` — so the
        comparison is of the invocation itself. A line that cannot be tokenized
        is not a valid invocation and is skipped.
    """
    for line in command_lines(step.get("run")):
        try:
            tokens = tuple(shlex.split(line))
        except ValueError:
            continue
        if SMOKE_CLI in tokens:
            return tokens[tokens.index(SMOKE_CLI) :]
    return None


def check_smoke_action_commands(
    action: dict[str, object] | None, violations: list[str]
) -> None:
    """Require both invocations to be the reviewed ones, from unconditioned steps.

    A step-level ``if`` can stop the verifier while the job guard, the
    dependencies, and the inputs all still read as correct, and a missing or
    rewritten invocation can leave the smoke reporting a release it never
    checked. Each subcommand must therefore appear exactly once, carry no
    condition, and pass exactly the arguments the review approved.
    """
    if action is None:
        return
    for phase, expected in SMOKE_ACTION_ARGUMENTS.items():
        _require_invocation(phase, expected, action_steps(action), violations)


def _require_invocation(
    phase: str,
    expected: tuple[str, ...],
    steps: list[dict[str, object]],
    violations: list[str],
) -> None:
    """Require one subcommand to be invoked once, unconditionally and exactly."""
    label = f"{CONSUMER_SMOKE_JOB}.action: the {phase} invocation"
    found = [
        (step, tokens)
        for step, tokens in ((step, action_cli_invocation(step)) for step in steps)
        if tokens is not None and tokens[:2] == (SMOKE_CLI, phase)
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
            f"{label} must pass exactly {list(expected)}, got {list(tokens)}"
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
    "SMOKE_CLI",
    "action_cli_invocation",
    "check_smoke_action_commands",
    "check_smoke_compilation",
]
