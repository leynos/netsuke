"""Read the one `runs-on` expression this estate writes.

A pull request from a fork cannot obtain a Ubicloud runner, so a lane that
serves pull requests names two runners and chooses between them when the
workflow is evaluated. Without the arm the lane never starts on a fork's pull
request, and a required check that never reports presents as a pull request
waiting on a check rather than as a placement fault.

This module reads that declaration and nothing else. It asserts nothing: a
reader that fails closed returns ``None`` and leaves each caller to decide
whether that is an offence, so a declaration this module cannot classify is
refused by the assertion written for it rather than skipped.

One spelling is read. A negated guard or a comparison says the same thing with
the arms the other way round, and reading either as the prescribed form would
let two spellings of one placement drift apart while both satisfied the
contract.

Examples
--------
>>> read_placement("${{ a.b && 'x' || 'y' }}")
Placement(guard='a.b', fork='x', owned='y')
>>> read_placement("ubuntu-latest") is None
True
>>> owned_runner("ubuntu-latest")
'ubuntu-latest'
"""

import re
import typing as typ

if typ.TYPE_CHECKING:  # pragma: no cover - typing only
    import collections.abc as cabc

#: The field that tells a fork's pull request from this repository's own.
#:
#: Named rather than matched by shape. ``private`` and ``archived`` sit in the
#: same position, parse identically and evaluate, and either would send every
#: pull request down one arm while the declaration still looked right.
FORK_GUARD: typ.Final[str] = "github.event.pull_request.head.repo.fork"

#: The runner a fork's pull request falls back to. The point of the arm is that
#: it names a runner GitHub hosts, so this is the only value it may take.
FORK_FALLBACK_RUNNER: typ.Final[str] = "ubuntu-latest"

#: The prefix every Ubicloud label carries. The owned arm must name one: an
#: arm that fell back to a hosted runner on both sides would satisfy a check
#: written only about the fork arm while the lane stopped using Ubicloud at all.
UBICLOUD_PREFIX: typ.Final[str] = "ubicloud-"


#: A single-quoted literal with no quote inside it. GitHub's expression
#: syntax has no escape other than a doubled quote, so a value containing one
#: is not the simple literal this reader accepts.
_SIMPLE_LITERAL = re.compile(r"'([^']*)'")


class Placement(typ.NamedTuple):
    """One `runs-on` expression read as a guard and its two arms.

    Attributes
    ----------
    guard
        The context field the expression branches on.
    fork
        The runner a fork's pull request is sent to.
    owned
        The runner this repository's own branches are sent to.
    """

    guard: str
    fork: str
    owned: str


def _expression_body(text: str) -> str | None:
    """Return the text inside ``${{`` and ``}}``.

    Returns
    -------
    str or None
        The body, or None when the text is not wrapped in an expression.
    """
    stripped = text.strip()
    if not (stripped.startswith("${{") and stripped.endswith("}}")):
        return None
    return stripped[3:-2]


def _quoted_literal(text: str) -> str | None:
    """Return a single-quoted literal's contents, or None for anything else.

    GitHub's expression syntax has no escape inside a single-quoted literal
    other than a doubled quote, so a value containing one is not the simple
    literal this reader accepts and is refused rather than guessed at.

    Returns
    -------
    str or None
        The literal's contents, or None when the text is not a simple literal.
    """
    found = _SIMPLE_LITERAL.fullmatch(text.strip())
    return None if found is None else found.group(1)


def _is_context_path(text: str) -> bool:
    """Return whether the text is a bare context path such as `github.event.x`.

    A guard has to be one field reference. A negation, a call or a comparison
    is a different question about the pull request, and is refused here so the
    assertion naming the expected field reports it.

    Returns
    -------
    bool
        True when the text is one bare context path.
    """
    stripped = text.strip()
    return bool(stripped) and all(
        character.isalnum() or character in "_." for character in stripped
    )


def _split_operands(declaration: str) -> tuple[str, str, str] | None:
    """Split an expression into its guard and its two unread arms.

    Returns
    -------
    tuple of str or None
        The guard, the truthy arm's text and the falsy arm's text, or None
        when the value is not one guarded choice between two operands.
    """
    body = _expression_body(declaration)
    if body is None or "&&" not in body:
        return None
    guard, arms = body.split("&&", 1)
    if "||" not in arms or not _is_context_path(guard):
        return None
    fork_text, owned_text = arms.split("||", 1)
    return guard.strip(), fork_text, owned_text


def read_placement(declaration: object) -> Placement | None:
    """Return the guard and arms of a placement expression, or None.

    A literal label, a matrix reference and a declaration carrying a line break
    all read as None. The line break matters most: a folded scalar whose
    continuation is indented deeper than its key keeps the break, GitHub
    evaluates the value regardless, and a green run is therefore no evidence
    that the declaration is well formed.

    Parameters
    ----------
    declaration
        A job's parsed ``runs-on`` value, of any type.

    Returns
    -------
    Placement or None
        The reading, or None when the value is not this one expression.
    """
    if not isinstance(declaration, str) or "\n" in declaration:
        return None
    parts = _split_operands(declaration)
    if parts is None:
        return None
    guard, fork_text, owned_text = parts
    fork = _quoted_literal(fork_text)
    owned = _quoted_literal(owned_text)
    if fork is None or owned is None:
        return None
    return Placement(guard=guard, fork=fork, owned=owned)


def owned_runner(declaration: object) -> str:
    """Return the runner this repository's own branches get.

    Every rule that sizes a lane, or pins its shape, asks about the runner the
    repository's own branches use. A fork's run is a GitHub-hosted fallback
    whose shape those rules deliberately do not govern, so normalising here
    keeps one reading of the declaration rather than one per caller.

    Parameters
    ----------
    declaration
        A job's parsed ``runs-on`` value, of any type.

    Returns
    -------
    str
        The owned arm of a placement expression, or the declaration rendered
        as text when it is not one.
    """
    placement = read_placement(declaration)
    return placement.owned if placement is not None else str(declaration)


#: Assignment keys whose lane serves pull requests and therefore needs the
#: fork arm. Every other Ubicloud lane must name its runner outright: an arm
#: nothing takes is a branch to keep correct for nothing, and asserting the
#: absence stops the expression spreading by imitation.
#:
#: `coverage-pr-submit` is not here on purpose. Its jobs trigger on
#: `workflow_run`, which runs in this repository's context whatever the
#: originating pull request was, so no fork ever selects their runner.
FORK_FALLBACK_KEYS = (
    "ci.build-test",
    "ci.kani-smoke",
    "netsukefile-test.netsukefile",
)


def _is_well_formed_fallback(placement: Placement | None) -> bool:
    """Return whether a reading is the placement a pull-request lane needs.

    Four things have to hold and each fails differently. There is no placement
    at all. It branches on a sibling field that parses and evaluates just the
    same. Its fork arm names a runner a fork cannot use, or one of the wrong
    platform. Its owned arm is not Ubicloud, which takes the lane off the
    measured shape while still looking like a fallback.

    Returns
    -------
    bool
        True when the reading is the prescribed placement.
    """
    return (
        placement is not None
        and placement.guard == FORK_GUARD
        and placement.fork == FORK_FALLBACK_RUNNER
        and placement.owned.startswith(UBICLOUD_PREFIX)
    )


def fork_fallback_offences(declarations: cabc.Mapping[str, object]) -> list[str]:
    """Return every lane whose `runs-on` declaration breaks the fork policy.

    Parameters
    ----------
    declarations
        Assignment key to the job's raw ``runs-on`` value, as written. Typed
        loosely because a job may declare a sequence, a mapping or nothing, and
        each of those is an offence to report rather than a shape to exclude.

    Returns
    -------
    list[str]
        One entry per offending key, named so a fix that corrects one lane and
        leaves another still fails.

    Notes
    -----
    Both directions are offences. A pull-request lane without the arm never
    starts on a fork's pull request, and a lane no fork reaches that carries
    one has a branch nothing takes. A lane whose arms or guard are wrong is
    worse than either, because the declaration looks right.

    Examples
    --------
    >>> owned = "ubicloud-standard-2-ubuntu-2404"
    >>> ok = f"${{{{ {FORK_GUARD} && '{FORK_FALLBACK_RUNNER}' || '{owned}' }}}}"
    >>> fork_fallback_offences({"ci.build-test": ok})
    []
    >>> fork_fallback_offences({"ci.build-test": owned})
    ['ci.build-test']
    >>> fork_fallback_offences({"coverage-main.coverage-upload": ok})
    ['coverage-main.coverage-upload']
    """
    offences: list[str] = []
    for key, declaration in declarations.items():
        placement = read_placement(declaration)
        if key not in FORK_FALLBACK_KEYS:
            if placement is not None:
                offences.append(key)
            continue
        if not _is_well_formed_fallback(placement):
            offences.append(key)
    return offences
