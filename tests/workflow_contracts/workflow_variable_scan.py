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
variable just as directly as a bare one — see `REFERENCE_TOKEN`.

Reading an expression is one job, and both predicates are stated over the same
reading of it: `_references_in` enumerates the identifiers a body names, in
either of the two syntaxes GitHub accepts for addressing one, and each predicate
is then a question about that list. `expression_references` is the general form,
so a caller holding a step to an exact reference — a credential, say — asks
about the identifier rather than about a substring of the text around it. A
caller that has to ask about the text *beside* a reference — whether the
comparison next to it is the right one — uses `reference_occurrences`, which is
the same list with the position each reference was read from, so the question is
answered against the offsets rather than by reading the grammar a second time.
Text that merely spells a name, in a value that is not an expression at all, is
not a reference and is not enumerated.

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

#: The namespace repository variables are read through. Named so the predicate
#: below reads as the question it asks — is this reference a repository
#: variable — rather than as a comparison against a word.
VARIABLE_NAMESPACE: typ.Final[str] = "vars"

#: Matches one single-quoted string literal in an expression body, as GitHub
#: Actions spells them. Double quotes are not string delimiters in that
#: grammar, so they are deliberately not matched.
#:
#: This is a branch of `REFERENCE_TOKEN` rather than a pattern applied on its
#: own, and that is what keeps the two readings of a quoted run apart: the
#: `'env.TOKEN'` in `${{ 'env.TOKEN' != '' }}` is a literal that names nothing,
#: while the quotes in `env['TOKEN']` are an index's delimiters with the name
#: sitting inside them. A separate pass over the body could not tell the two
#: apart — it would have to strip one and keep the other, and it does not know
#: which until it has read what precedes the opening quote.
EXPRESSION_STRING: typ.Final[str] = r"'[^']*'"

#: Matches one reference to a context value in an expression body, and one
#: string literal, in the order a scan must consider them.
#:
#: GitHub's contexts reference gives an expression two syntaxes for addressing
#: a value: property de-reference, `env.CI`, and index, `env['CI']`. Both are
#: read, because the question a caller asks of an expression — is this value
#: the reference I am holding the step to — is the same of either spelling.
#: Reading only the dotted form reported nothing for
#: `vars['CODESCENE_CLI_SHA256']` while it resolved to the empty string exactly
#: as the dotted spelling did, which is the failure this scan exists to
#: prevent, in a spelling a `.`-only pattern cannot see.
#:
#: The alternatives are ordered, and the order is what the scan means. A quoted
#: run is a string literal that names nothing *unless* the quotes are an
#: index's delimiters, so the index is tried first: `env['TOKEN']` is a
#: reference, and must not be read as the text `env` followed by a run that the
#: literal branch then swallows. The literal is tried next, ahead of any bare
#: identifier, so the contents of a quoted run are consumed by it and can never
#: be re-read as a reference — which is the whole of what
#: `${{ 'env.TOKEN' != '' }}` depends on.
#:
#: The index branch requires its bracket to hold a quoted name, so a
#: non-literal index — `vars[name]`, or `vars[format('{0}', name)]` — is
#: deliberately not matched: there is no name in the text to compare against a
#: declared one, and the scan states that bound rather than guessing at a value
#: only the runner can reach. The name is required to be non-empty so that the
#: two spellings a caller reads it from cannot disagree: `vars['']` reads the
#: empty name, which `indexed_name or dotted_name` would discard as though the
#: branch had not matched. The `\b` anchors keep a match from starting
#: part-way through a longer word.
REFERENCE_TOKEN: typ.Final[re.Pattern[str]] = re.compile(
    r"\b(?P<indexed_ns>[A-Za-z_][A-Za-z0-9_]*)"
    r"\[\s*'(?P<indexed_name>[^']+)'\s*\]"
    rf"|{EXPRESSION_STRING}"
    r"|\b(?P<dotted_ns>[A-Za-z_][A-Za-z0-9_]*)"
    r"\.(?P<dotted_name>[A-Za-z_][A-Za-z0-9_]*)"
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
    """Return whether ``value`` reads an unpermitted repository variable.

    The same reading as [`expression_references`], asked the narrower question:
    a reference is offending when it is a repository variable and is not the
    one this repository declares. Reading the namespace here rather than in the
    pattern is what keeps `env.NETSUKE_SCCACHE_LOCAL_DIR` — a different
    namespace, and a value that is not this repository's variable at all — from
    being permitted on the strength of its name.

    Returns
    -------
    bool
        Whether some reference in the value reads an undeclared variable.
    """
    return any(
        namespace == VARIABLE_NAMESPACE and name != PERMITTED_VARIABLE_NAME
        for namespace, name, _, _ in reference_occurrences(value)
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
        caller asking how often is served by the length. An identifier inside a
        quoted string is not enumerated: GitHub's expression grammar only
        treats a name as a reference when it is written unquoted, so
        ``${{ 'env.TOKEN' != '' }}`` names no variable and is not a gate.
    """
    return [
        (namespace, name)
        for namespace, name, _, _ in reference_occurrences(value, bare=bare)
    ]


def reference_occurrences(
    value: str, *, bare: bool = False
) -> list[tuple[str, str, int, int]]:
    """Return ``(namespace, name, start, end)`` for each reference in ``value``.

    The pairs are [`expression_references`] with the position each was read
    from, for a caller that has to ask something about the text *around* a
    reference rather than about the reference alone. A condition, for instance,
    is a gate on the credential only when the comparison beside it is the right
    one, and where the reference sits is what says which text that is.

    The offsets are into ``value`` itself, not into the body of the region a
    reference was found in, so a caller can slice ``value`` with them.

    Parameters
    ----------
    value
        One string from a parsed workflow step.
    bare
        Whether ``value`` is itself an expression, in addition to any regions
        it holds.

    Returns
    -------
    list[tuple[str, str, int, int]]
        One entry per reference, in the order it appears, with repeats kept.
    """
    occurrences: list[tuple[str, str, int, int]] = []
    for region in EXPRESSION_REGION.finditer(value):
        offset = region.start("body")
        occurrences.extend(
            (namespace, name, start + offset, end + offset)
            for namespace, name, start, end in _references_in(region.group("body"))
        )
    if bare:
        occurrences.extend(_references_in(value))
    return occurrences


def _references_in(body: str) -> list[tuple[str, str, int, int]]:
    """Return each reference ``body`` reads, in order, with its span.

    The body is read once, left to right, against [`REFERENCE_TOKEN`] — the
    only reading of the expression grammar in this module. Every branch of that
    pattern is a match, so the scan has to ask what each one *found* rather than
    whether it matched: a string literal is consumed here precisely so that the
    name it spells is never enumerated, and the branch that consumed it is what
    makes that so.

    Returns
    -------
    list[tuple[str, str, int, int]]
        One entry per reference, with repeats kept and literals omitted. The
        offsets are into ``body``.
    """
    references: list[tuple[str, str, int, int]] = []
    for match in REFERENCE_TOKEN.finditer(body):
        reference = _reference_of(match)
        if reference is not None:
            references.append((*reference, match.start(), match.end()))
    return references


def _reference_of(match: re.Match[str]) -> tuple[str, str] | None:
    """Return the reference one token matched, or `None` when it matched text.

    Which branch matched is the answer, so the groups are read rather than a
    separate test applied: the literal branch carries no groups at all, which
    is how a quoted run that names nothing is told apart from an index that
    names something.

    Returns
    -------
    tuple[str, str] | None
        The ``(namespace, name)`` pair, or `None` for a string literal.
    """
    namespace = match.group("indexed_ns") or match.group("dotted_ns")
    if namespace is None:
        return None
    return (namespace, match.group("indexed_name") or match.group("dotted_name"))


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
