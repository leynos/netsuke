"""Mutate the prologue and epilogue around the consumer smoke's verifier.

Each applier names one plausible regression in the script that surrounds the
reviewed invocation, and applies it to a deep copy. They live apart from the
wiring mutations because they answer a different question: those ask whether the
job depends on, waits for, and guards the right stages, while these ask whether
the invocation the action runs is the one that decides the step's outcome.

The regressions are chosen to defeat a reader that searches the script for a
suspicious line rather than reading its shape. ``PLACEHOLDER=1 exit 0`` has an
assignment's spelling and bash's ``exit 0`` semantics, ``set -n`` looks like the
reviewed ``set`` prologue and disables execution, and an assignment whose value
spells the CLI's path sits above the real invocation and once ended the search
for it. Each one leaves the invocation line byte-for-byte intact.

What each regression must break is stated in
``release_publish_path_consumer_expectations``.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

from release_publish_path_consumer_mutation_support import (
    ConsumerSmokeMutationError,
    ConsumerSmokeMutationIssue,
    Job,
    cli_invocation,
)
from release_publish_path_consumer_script import runs_smoke_cli
from release_publish_path_consumer_shape import command_lines

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: The reference the verifier passes as its identity evidence. Only its quoting
#: differs between the reviewed line and the regression below.
IDENTITY_REFERENCE = '"$IDENTITY_PATH"'


def _verify_script(action: Job) -> tuple[dict[str, object], str]:
    """Return the verify step and its script, ready for one rewrite.

    Returns
    -------
    tuple[dict[str, object], str]
        The step and the script it currently runs, so an applier can replace
        the script with a rewritten copy without re-reading the document.

    Raises
    ------
    ConsumerSmokeMutationError
        If the verify step carries no script to rewrite, so the mutation would
        change nothing the contract reads.
    """
    declared, index = cli_invocation(action, "verify")
    step = declared[index]
    run = step.get("run")
    if not isinstance(run, str):
        raise ConsumerSmokeMutationError(ConsumerSmokeMutationIssue.MISSING_ACTION_STEP)
    return step, run


def _invocation_place(run: str) -> tuple[list[str], int]:
    """Return the script's executable lines and where the invocation sits.

    Returns
    -------
    tuple[list[str], int]
        The joined command lines and the index of the one that runs the CLI.

    Raises
    ------
    ConsumerSmokeMutationError
        If the script runs no line the contract reads as the invocation, so a
        rewrite would change nothing.

    Notes
    -----
    The lines are taken from :func:`command_lines`, which joins a command split
    across backslash continuations and drops comments. A rewrite made in the
    raw text would land inside the invocation's continuation chain and become
    one of its arguments, which would change the command line the contract
    compares rather than the script around it.
    """
    lines = command_lines(run)
    for index, line in enumerate(lines):
        if runs_smoke_cli(line):
            return lines, index
    raise ConsumerSmokeMutationError(ConsumerSmokeMutationIssue.MISSING_ACTION_STEP)


def _insert_before_invocation(run: str, added: str) -> str:
    """Return the script with ``added`` on its own line before the invocation.

    Returns
    -------
    str
        The rewritten script, holding only the executable lines of the original
        with the addition spliced in.

    Notes
    -----
    A script with no invocation refuses through :func:`_invocation_place`
    rather than being returned unchanged, so an applier built on this cannot
    silently mutate nothing.
    """
    lines, index = _invocation_place(run)
    return "\n".join([*lines[:index], added, *lines[index:]])


def _repeat_invocation(run: str) -> str:
    """Return the script with the invocation line repeated directly below it.

    Returns
    -------
    str
        The rewritten script, whose only addition is a second copy of the
        reviewed command.

    Notes
    -----
    A script with no invocation refuses through :func:`_invocation_place`
    rather than being returned unchanged.
    """
    lines, index = _invocation_place(run)
    return "\n".join([*lines[: index + 1], lines[index], *lines[index + 1 :]])


def _short_circuit_verifier(_release: Job, action: Job) -> None:
    """End the verify step successfully before the verifier can run.

    Every reviewed argument stays in place, so only a contract that reads the
    script's shape rather than searching it for an exit notices that the
    verification is unreachable.
    """
    step, run = _verify_script(action)
    step["run"] = f"exit 0\n{run}"


def _hide_verifier_failure(_release: Job, action: Job) -> None:
    """Let the verifier fail without failing the step.

    ``set +e`` is inserted immediately before the invocation, after the
    fail-fast the script starts with, so this is not a prologue the reviewed
    ``set -euo pipefail`` can be confused with. ``exit 0`` below the invocation
    then ends the step successfully, so a refused verification reaches the job
    as a pass. The invocation line itself is untouched.
    """
    step, run = _verify_script(action)
    step["run"] = f"{_insert_before_invocation(run, 'set +e')}\nexit 0"


def _enclose_verifier_in_false_branch(_release: Job, action: Job) -> None:
    """Confine the verifier to a branch bash never takes.

    The invocation stays on its own line with every reviewed argument, so only
    a contract that reads the script's control flow notices that the branch's
    condition is false and nothing verified the release.
    """
    step, run = _verify_script(action)
    step["run"] = f"if false; then\n{run}\nfi"


def _prefix_with_assignment_exit(_release: Job, action: Job) -> None:
    """Hide an ``exit 0`` behind an assignment's spelling.

    ``PLACEHOLDER=1 exit 0`` matches the shape of a plain assignment and bash
    runs it as ``exit 0``, so a reader that pattern-matches the line rather than
    tokenizing it accepts a prologue that ends the step before the verifier
    runs.
    """
    step, run = _verify_script(action)
    step["run"] = _insert_before_invocation(run, "PLACEHOLDER=1 exit 0")


def _disable_execution(_release: Job, action: Job) -> None:
    """Turn the shell's execution off before the verifier runs.

    ``set -n`` looks like the reviewed ``set -euo pipefail`` and makes bash read
    the invocation without running it, so only a reader that checks which
    letters a prologue's ``set`` turns on refuses it.
    """
    step, run = _verify_script(action)
    step["run"] = _insert_before_invocation(run, "set -n")


def _single_quote_reference(_release: Job, action: Job) -> None:
    """Make bash pass a reference's text instead of its value.

    Single quotes stop expansion, so ``'$IDENTITY_PATH'`` reaches the CLI as a
    literal path and every evidence check fails at run time. Tokenizing cannot
    see the difference, because both spellings produce the same word.

    Raises
    ------
    ConsumerSmokeMutationError
        If the reviewed line no longer carries the reference to requote, so the
        mutation would change nothing.
    """
    step, run = _verify_script(action)
    if IDENTITY_REFERENCE not in run:
        raise ConsumerSmokeMutationError(ConsumerSmokeMutationIssue.MISSING_ACTION_STEP)
    step["run"] = run.replace(IDENTITY_REFERENCE, "'$IDENTITY_PATH'", 1)


def _mask_with_cli_named_assignment(_release: Job, action: Job) -> None:
    """End the step early behind an assignment that spells the CLI's path.

    The assignment is a legitimate prologue line and the ``exit 0`` below it is
    not, so a reader that stops at the first line mentioning the CLI's path
    reports the prologue as clean and lets the verifier be skipped.
    """
    step, run = _verify_script(action)
    step["run"] = _insert_before_invocation(
        run, 'smoke_script="scripts/consumer_smoke_cli.py"\nexit 0'
    )


def _duplicate_verifier(_release: Job, action: Job) -> None:
    """Run the reviewed verifier twice inside the step that already runs it.

    Both lines are byte-for-byte the reviewed command, so a contract that
    compares the command line, and counts only the first invocation of each
    step, sees exactly one reviewed execution while the action runs two. The
    second run reads downloads the first has already consumed, so it fails for
    a reason that says nothing about the release and hides the verdict the
    step exists to report.
    """
    step, run = _verify_script(action)
    step["run"] = _repeat_invocation(run)


PROLOGUE_MUTATIONS: dict[str, cabc.Callable[[Job, Job], None]] = {
    "short-circuit-consumer-smoke-verifier": _short_circuit_verifier,
    "hide-consumer-smoke-verifier-failure": _hide_verifier_failure,
    "enclose-consumer-smoke-verifier-in-false-branch": (
        _enclose_verifier_in_false_branch
    ),
    "prefix-consumer-smoke-verifier-with-assignment-exit": (
        _prefix_with_assignment_exit
    ),
    "disable-consumer-smoke-verifier-execution": _disable_execution,
    "single-quote-consumer-smoke-verifier-reference": _single_quote_reference,
    "mask-consumer-smoke-verifier-with-cli-named-assignment": (
        _mask_with_cli_named_assignment
    ),
    "duplicate-consumer-smoke-verifier": _duplicate_verifier,
}


__all__ = ["PROLOGUE_MUTATIONS"]
