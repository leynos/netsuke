"""Render a workflow's concurrency group for a given run, as Actions would.

``pr_concurrency_test`` decides whether a group keeps together the runs that
should cancel one another and keeps every other run apart. Searching the
group's text for a context name cannot answer that: ``github.head_ref`` names
the pull request's branch and still collides across forks, and a group can
name the pull-request number and then discard it. So the contract renders each
group against the run contexts below and compares the strings.

Runs that must share a group: two pushes to one pull request, so the newer
cancels the older. Runs that must not: that pull request, a fork's pull
request from a branch of the same name, two pushes to ``main``, two
dispatches of another branch, and two scheduled runs. A ref-keyed fallback
would put the trunk pushes in one group, where a third push replaces a
still-pending second one and that commit never gets CI. So the fallback is
``github.run_id`` (estate rule "PR-lane concurrency fallback").

Only context paths joined by ``||`` are modelled, which is every form the
estate's groups use. Anything else is refused rather than guessed at.

Run via ``make test-workflow-contracts``.
"""

import re
import typing as typ

#: One ``${{ ... }}`` expression inside a group template.
_EXPRESSION: typ.Final[re.Pattern[str]] = re.compile(r"\$\{\{\s*(.*?)\s*\}\}")

#: A bare context path such as ``github.event.pull_request.number``.
_CONTEXT_PATH: typ.Final[re.Pattern[str]] = re.compile(r"[A-Za-z_][A-Za-z0-9_.-]*")

#: The one expression a run-unique value may appear in: the pull-request
#: number first, the run identifier as its fallback.
ESTATE_FALLBACK: typ.Final[tuple[str, ...]] = (
    "github.event.pull_request.number",
    "github.run_id",
)

#: Context values unique to one run. The estate rule allows one only as the
#: fallback behind the pull-request number.
RUN_UNIQUE: typ.Final[frozenset[str]] = frozenset({
    "github.run_id",
    "github.run_number",
    "github.run_attempt",
    "github.sha",
})


class UnmodelledGroupError(AssertionError):
    """Raised when a group uses an expression the renderer does not model."""


def pull_request_run(number: str, head_ref: str, run_id: str) -> dict[str, str]:
    """Return the context of one run of a pull request's workflow.

    Parameters
    ----------
    number : str
        The pull request number, or ``""`` for a run with no pull request.
    head_ref : str
        The source branch name.
    run_id : str
        The run identifier, which also stands in for the run number and SHA.

    Returns
    -------
    dict[str, str]
        Context paths mapped to the values Actions would supply.

    Examples
    --------
    >>> pull_request_run("7", "patch-1", "101")["github.ref"]
    'refs/pull/7/merge'
    """
    return {
        "github.workflow": "CI",
        "github.event_name": "pull_request",
        "github.event.pull_request.number": number,
        "github.ref": f"refs/pull/{number}/merge",
        "github.head_ref": head_ref,
        "github.base_ref": "main",
        "github.run_id": run_id,
        "github.run_number": run_id,
        "github.run_attempt": "1",
        "github.sha": f"sha-{run_id}",
    }


def branch_run(event_name: str, ref: str, run_id: str) -> dict[str, str]:
    """Return the context of one run with no pull request.

    Parameters
    ----------
    event_name : str
        The triggering event, such as ``push`` or ``schedule``.
    ref : str
        The ref the run is for.
    run_id : str
        The run identifier.

    Returns
    -------
    dict[str, str]
        Context paths mapped to the values Actions would supply; the
        pull-request fields, ``head_ref`` and ``base_ref`` are empty.

    Examples
    --------
    >>> branch_run("push", "refs/heads/main", "104")["github.base_ref"]
    ''
    """
    return {
        **pull_request_run("", "", run_id),
        "github.event_name": event_name,
        "github.ref": ref,
        "github.base_ref": "",
    }


FIRST_PUSH: typ.Final = pull_request_run("7", "patch-1", "101")
SECOND_PUSH: typ.Final = pull_request_run("7", "patch-1", "102")

#: Runs that must render one group: the newer cancels the older.
MUST_SHARE: typ.Final[tuple[tuple[dict[str, str], dict[str, str]], ...]] = (
    (FIRST_PUSH, SECOND_PUSH),
)

#: Runs that must render distinct groups: none may cancel or replace another.
MUST_PART: typ.Final[tuple[dict[str, str], ...]] = (
    FIRST_PUSH,
    pull_request_run("8", "patch-1", "103"),
    branch_run("push", "refs/heads/main", "104"),
    branch_run("push", "refs/heads/main", "105"),
    branch_run("workflow_dispatch", "refs/heads/feature", "106"),
    branch_run("workflow_dispatch", "refs/heads/feature", "107"),
    branch_run("schedule", "refs/heads/main", "108"),
    branch_run("schedule", "refs/heads/main", "109"),
)


def expressions(template: str) -> list[tuple[str, ...]]:
    """Return the ``||`` operands of each ``${{ ... }}`` expression in a group.

    Parameters
    ----------
    template : str
        The group as written in the workflow.

    Returns
    -------
    list[tuple[str, ...]]
        One tuple of stripped operands per expression, in order.

    Examples
    --------
    >>> expressions("${{ github.workflow }}-${{ a || b }}")
    [('github.workflow',), ('a', 'b')]
    """
    return [
        tuple(operand.strip() for operand in match.group(1).split("||"))
        for match in _EXPRESSION.finditer(template)
    ]


def render_group(template: str, context: dict[str, str]) -> str:
    """Render a concurrency group for ``context``.

    ``||`` yields its first non-empty operand, as in Actions, where a missing
    pull-request number is null and falls through. Every operand is checked
    before any is chosen, so an unmodelled right-hand operand is refused even
    when the left one would have been used.

    Parameters
    ----------
    template : str
        The group as written in the workflow.
    context : dict[str, str]
        Context paths mapped to their values for one run.

    Returns
    -------
    str
        The group Actions would compute.

    Raises
    ------
    UnmodelledGroupError
        If an expression holds anything but known context paths joined by
        ``||``, or a ``${{`` opener is left unclosed.

    Examples
    --------
    >>> template = "pr-${{ github.event.pull_request.number || github.ref }}"
    >>> render_group(template, FIRST_PUSH)
    'pr-7'
    >>> render_group(template, MUST_PART[2])
    'pr-refs/heads/main'
    """

    def evaluate(match: re.Match[str]) -> str:
        """Return the first non-empty operand of one expression."""
        paths = [operand.strip() for operand in match.group(1).split("||")]
        if any(
            not _CONTEXT_PATH.fullmatch(path) or path not in context for path in paths
        ):
            message = f"unmodelled expression {match.group(0)!r} in {template!r}"
            raise UnmodelledGroupError(message)
        return next((context[path] for path in paths if context[path]), "")

    rendered = _EXPRESSION.sub(evaluate, template)
    if "${{" in rendered:
        message = f"unclosed expression in {template!r}"
        raise UnmodelledGroupError(message)
    return rendered


def shared_groups(rendered: dict[str, str]) -> dict[str, list[str]]:
    """Return each group that more than one run or workflow renders.

    GitHub compares concurrency group names case-insensitively, so ``CI-7``
    and ``ci-7`` are one group and either run can cancel the other. Groups
    are therefore compared casefolded.

    Parameters
    ----------
    rendered : dict[str, str]
        Each run or workflow's label mapped to the group it renders.

    Returns
    -------
    dict[str, list[str]]
        Each casefolded group claimed more than once, mapped to the labels
        claiming it; empty when every group is distinct.

    Examples
    --------
    >>> shared_groups({"a.yml": "CI-7", "b.yml": "ci-7", "c.yml": "Lint-7"})
    {'ci-7': ['a.yml', 'b.yml']}
    """
    owners: dict[str, list[str]] = {}
    for label, group in rendered.items():
        owners.setdefault(group.casefold(), []).append(label)
    return {group: labels for group, labels in owners.items() if len(labels) > 1}


def fallback_problems(group: str) -> list[str]:
    """Return why a group breaks the run-identifier rule, or nothing.

    The estate rule allows a run-unique value exactly once, as the fallback
    behind the pull-request number. Anywhere else it either splits one pull
    request's pushes or hides a ref-keyed fallback.

    Parameters
    ----------
    group : str
        The group as written in the workflow.

    Returns
    -------
    list[str]
        One reason per broken clause; empty when the group complies.

    Examples
    --------
    >>> fallback_problems(
    ...     "${{ github.workflow }}-"
    ...     "${{ github.event.pull_request.number || github.run_id }}"
    ... )
    []
    >>> len(fallback_problems("${{ github.workflow }}-${{ github.run_id }}"))
    2
    """
    found = expressions(group)
    problems: list[str] = []
    if found.count(ESTATE_FALLBACK) != 1:
        expected = " || ".join(ESTATE_FALLBACK)
        problems.append(f"must contain `${{{{ {expected} }}}}` once, found {found}")
    misplaced = [
        operands
        for operands in found
        if operands != ESTATE_FALLBACK and any(name in RUN_UNIQUE for name in operands)
    ]
    if misplaced:
        problems.append(f"uses a run-unique value outside the fallback: {misplaced}")
    return problems


def keeps_runs_together_and_apart(template: str) -> bool:
    """Report whether a group joins the ``MUST_SHARE`` runs and parts the rest.

    Parameters
    ----------
    template : str
        The group as written in the workflow.

    Returns
    -------
    bool
        True when every ``MUST_SHARE`` pair renders one group and the
        ``MUST_PART`` runs render distinct ones, compared casefolded.

    Examples
    --------
    >>> keeps_runs_together_and_apart(
    ...     "${{ github.event.pull_request.number || github.run_id }}"
    ... )
    True
    >>> keeps_runs_together_and_apart("release-${{ github.ref }}")
    False
    """
    shares = all(
        render_group(template, first).casefold()
        == render_group(template, second).casefold()
        for first, second in MUST_SHARE
    )
    parted = {
        str(index): render_group(template, run) for index, run in enumerate(MUST_PART)
    }
    return shares and not shared_groups(parted)
