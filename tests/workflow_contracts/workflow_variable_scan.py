"""Find the expression references a workflow's steps actually name.

A GitHub Actions expression naming a repository variable that the repository
has not declared does not fail. It interpolates to the empty string and the
step runs with a value the author did not intend, which is worse than a
missing feature: the workflow reads as though it configured something. The
observed instance was an upload step handed
``${{ vars.CODESCENE_CLI_SHA256 }}`` as an installer checksum, where the empty
value made the check a no-op that looked like verification.

These predicates answer the question for one parsed step, over every string it
holds. The scan is deliberately not restricted to the ``with`` block: an ``if``
condition decides whether a step runs at all, so a ``vars.`` reference there is
the most consequential place one can appear, and an ``env`` entry is where a
credential would be bound. It is also not restricted to the start of an
expression, because a compound condition or a function argument names a
variable just as directly as a bare one — see `VARIABLE_REFERENCE`.

`expression_references` is the mechanism underneath, and is general to any
name: it enumerates the dotted identifiers an expression names, so a caller
holding a step to an exact reference — a credential, say — asks about the
identifier rather than about a substring of the text around it. Text that
merely spells a name, in a value that is not an expression at all, is not a
reference and is not enumerated.

They read parsed values rather than files, so the callers can hold the
repository's own workflows to the contract and drive shapes the repository does
not have.

Run via ``make test-workflow-contracts``.
"""

import re
import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: Matches one `${{ ... }}` expression and captures its body. A `vars.`
#: reference is only meaningful inside one of these, so the regions are located
#: first and scanned second: matching `${{` and `vars.` as one pattern would
#: only ever find a reference that immediately follows the opening delimiter.
#:
#: `DOTALL` is required rather than cosmetic. A YAML literal block (`|`) keeps
#: its newlines after parsing, so an expression a step breaks across two lines
#: is scanned as text containing a newline; without the flag `.` stops at it,
#: the region never closes, and the reference inside goes unreported.
EXPRESSION_REGION: typ.Final[re.Pattern[str]] = re.compile(
    r"\$\{\{(?P<body>.*?)\}\}", re.DOTALL
)

#: Matches a `vars.` reference anywhere within an expression body. The
#: repository declares no variables, so every one of them resolves to the empty
#: string.
#:
#: This is deliberately anchored to neither end of the body. A compound
#: condition such as `github.event_name == 'push' && vars.SECRET != ''`, or a
#: call such as `contains(vars.FOO, 'x')`, names a variable exactly as the bare
#: form does, and resolves to the empty string just the same; an earlier
#: version of this scan matched only the position immediately after `${{`, so
#: it reported nothing for either.
VARIABLE_REFERENCE: typ.Final[re.Pattern[str]] = re.compile(r"\bvars\.")

#: Matches one dotted identifier in an expression body, capturing its
#: namespace and its name. A GitHub Actions expression addresses a value as
#: `<namespace>.<name>` — `env.CI`, `secrets.TOKEN`, `vars.FOO`, `github.ref` —
#: so the pair is the unit a caller can compare against a name it expects.
#:
#: The namespace is part of the match, not decoration. Asking whether a value
#: merely contains `CS_ACCESS_TOKEN` answers a question about the text, and
#: `env.NOT_CS_ACCESS_TOKEN` contains it; asking whether some identifier's name
#: *is* `CS_ACCESS_TOKEN` answers the question about the reference.
REFERENCE_IDENTIFIER: typ.Final[re.Pattern[str]] = re.compile(
    r"\b(?P<namespace>[A-Za-z_][A-Za-z0-9_]*)\.(?P<name>[A-Za-z_][A-Za-z0-9_]*)"
)

#: The one repository variable the workflows legitimately read, and the one
#: place a `vars.` expression is allowed to appear in the coverage lane: it
#: selects between the two sccache backends. Every other `vars.` reference is a
#: value that silently resolves to the empty string.
PERMITTED_VARIABLE_NAME: typ.Final[str] = "NETSUKE_SCCACHE_LOCAL_DIR"


def unbound_variable_references(step: cabc.Mapping[str, object]) -> list[str]:
    """Return every ``vars.`` reference in a step but the permitted one.

    Each ``${{ ... }}`` region is located first and then scanned for references,
    so a reference is found anywhere inside an expression — a bare condition, a
    compound one, or a function argument — while text that merely spells
    ``vars.`` outside an expression is ignored.

    Parameters
    ----------
    step
        One parsed workflow step. Every string it holds is scanned, so a
        reference in ``if``, ``env``, or ``with`` is found alike. The parameter
        is a `Mapping` rather than a `dict` because the scan only reads, and a
        mapping's value type is covariant: a literal like ``{"if": "..."}``
        infers ``dict[str, str]``, which is not a `dict[str, object]` but is a
        ``Mapping[str, object]``.

    Returns
    -------
    list[str]
        One entry per offending value, empty when the step reads no repository
        variable or reads only the one this repository defines. A value
        carrying several references is reported once, since the caller needs to
        know which values to look at rather than how many names each holds.
    """
    return [
        value for value in _iter_strings(step) if _names_an_unpermitted_variable(value)
    ]


def _names_an_unpermitted_variable(value: str) -> bool:
    """Return whether ``value`` reads an unpermitted repository variable."""
    return any(
        _variable_name(region.group("body"), match.end()) != PERMITTED_VARIABLE_NAME
        for region in EXPRESSION_REGION.finditer(value)
        for match in VARIABLE_REFERENCE.finditer(region.group("body"))
    )


def expression_references(value: str, *, bare: bool = False) -> list[tuple[str, str]]:
    """Return the ``(namespace, name)`` pairs an expression names.

    Only text inside a ``${{ ... }}`` region is scanned by default, so a value
    that spells a name as literal text — a message, a comparison against a
    string — names nothing and returns an empty list.

    Parameters
    ----------
    value
        One string from a parsed workflow step.
    bare
        Whether the value is itself an expression, in addition to any regions
        it holds. GitHub evaluates a step's ``if`` as an expression whether or
        not it is written with the delimiters, so a caller holding a gate to an
        exact reference must scan the whole value: the delimited form is what
        the rest of the workflow uses, and the bare form is what the gate
        happens to be written with.

    Returns
    -------
    list[tuple[str, str]]
        One pair per identifier, in the order it appears, with repeats kept:
        a caller asking whether a name is *used* is served by membership, and a
        caller asking how often is served by the length.
    """
    bodies = [region.group("body") for region in EXPRESSION_REGION.finditer(value)]
    if bare:
        bodies.append(value)
    return [
        (match.group("namespace"), match.group("name"))
        for body in bodies
        for match in REFERENCE_IDENTIFIER.finditer(body)
    ]


def _variable_name(body: str, start: int) -> str:
    """Return the variable name beginning at ``start`` in an expression body."""
    match = re.match(r"[A-Za-z_][A-Za-z0-9_]*", body[start:])
    return match.group(0) if match else ""


def _iter_strings(value: object) -> cabc.Iterator[str]:
    """Yield every string nested anywhere in a parsed YAML value."""
    match value:
        case str() as text:
            yield text
        case dict() as mapping:
            for key, item in mapping.items():
                yield from _iter_strings(key)
                yield from _iter_strings(item)
        case list() as sequence:
            for item in sequence:
                yield from _iter_strings(item)
        case _:
            return
