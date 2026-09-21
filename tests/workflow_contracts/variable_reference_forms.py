"""The fragments a GitHub Actions expression is written from.

One fragment is one of the four things a workflow actually writes inside an
expression: a property de-reference, an index, a quoted literal, or unrelated
text. Each records what it names at the moment it is built, so a test consuming
this module compares the scan against an independent account of the text rather
than against a second reading of the same text.

The vocabulary lives here and the composition lives in
``variable_expression_forms``, which is the direction the dependency runs: an
expression is made of fragments, and a fragment knows nothing of the expression
it will sit in. The split is also what keeps each module within the size the
linters allow.

The model is deliberately shallow rather than a rival implementation of GitHub's
expression grammar. Its single decision is which fragments name something, which
is the decision the scan makes, so the two are comparable; a deeper model would
be a rival parser whose disagreements would be about the disagreement rather
than about the contract under test.

Nothing here runs a workflow. A generated expression is text, and the properties
are stated over what the scan should *read* in it.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import dataclasses as dc
import typing as typ

from hypothesis import strategies as st

#: The namespace repository variables are read through.
VARIABLE_NAMESPACE: typ.Final[str] = "vars"

#: The one repository variable the workflows legitimately read, spelled exactly
#: as the scan module spells it. The model states the permission itself, so the
#: property that reads it is a comparison between two accounts of the rule
#: rather than a restatement of one; ``workflow_variable_scan_properties_test``
#: holds the two spellings equal, so a rename on either side is caught.
PERMITTED_VARIABLE_NAME: typ.Final[str] = "NETSUKE_SCCACHE_LOCAL_DIR"

#: The contexts a generated expression may address, the variable namespace
#: first. More than one is present because the scan enumerates *any* dotted or
#: indexed pair, and a model that only ever wrote `vars.` would leave the
#: namespace half of the offending-reference rule untested.
NAMESPACES: typ.Final[tuple[str, ...]] = (
    VARIABLE_NAMESPACE,
    "env",
    "secrets",
    "github",
    "steps",
)

#: The contexts that are not the variable namespace, which name nothing the
#: repository has to declare.
OTHER_NAMESPACES: typ.Final[tuple[str, ...]] = tuple(
    namespace for namespace in NAMESPACES if namespace != VARIABLE_NAMESPACE
)

#: The texts a generated quoted literal may hold.
#:
#: Several of them are spelled exactly as a reference is, which is the point:
#: GitHub's grammar gives a name meaning only when it is written unquoted, so a
#: literal that spells one is text and hides whatever it spells. A literal
#: holding an index spelling is the sharpest of them — it contains a quote, so a
#: scan that read the literal's contents would also have to decide where the
#: literal ended.
LITERAL_TEXTS: typ.Final[tuple[str, ...]] = (
    "",
    "push",
    "true",
    "vars.UNDECLARED",
    "env.CS_ACCESS_TOKEN",
    "vars['FOO']",
    "secrets.TOKEN",
)

#: A context a reference fragment may be addressed through.
ANY_NAMESPACE: typ.Final[st.SearchStrategy[str]] = st.sampled_from(NAMESPACES)

#: A context other than the variable namespace.
OTHER_NAMESPACE: typ.Final[st.SearchStrategy[str]] = st.sampled_from(OTHER_NAMESPACES)

#: A name a reference fragment may address.
#:
#: The alphabet is GitHub's identifier one, and it excludes the quote, the
#: bracket, and the dot — so a generated name cannot end a fragment early or
#: spell a second reference inside the first.
VARIABLE_NAMES: typ.Final[st.SearchStrategy[str]] = st.from_regex(
    r"[A-Za-z_][A-Za-z0-9_]{0,10}", fullmatch=True
)

#: A word or operator of unrelated text.
#:
#: No `/`, `.`, `[`, `'`, or `$` appears here: the first would spell a path, the
#: second and third would let two prose words join into a reference the model
#: never intended, the fourth would open a string literal, and the fifth the
#: expression delimiters. Prose is the control — text that names nothing — so it
#: must not be able to become something that does.
PROSE_WORDS: typ.Final[st.SearchStrategy[str]] = st.one_of(
    st.from_regex(r"[A-Za-z_][A-Za-z0-9_]{0,8}", fullmatch=True),
    st.sampled_from(("==", "!=", "&&", "||", "(", ")", ",", ">", "<")),
)


@dc.dataclass(frozen=True)
class Reference:
    """One reference an expression names, and the text it was written as.

    ``rendered`` is what makes the model answerable about *position* as well as
    about identity: the scan reports the span it read each reference from, and
    the only independent way to say that span is right is to compare the text it
    covers against the text the model wrote.
    """

    #: The context the reference is addressed through.
    namespace: str
    #: The name addressed within that context.
    name: str
    #: The reference exactly as it was rendered.
    rendered: str


@dc.dataclass(frozen=True)
class Fragment:
    """One piece of an expression body, and the references it names."""

    #: The text the fragment is written as.
    text: str
    #: The references that text names, empty when it names none.
    references: tuple[Reference, ...] = ()


def is_undeclared(reference: Reference) -> bool:
    """Return whether ``reference`` reads an undeclared repository variable.

    The policy, written out here as well as in the scan module on purpose. A
    property comparing the two is then comparing a reading against a rule rather
    than a reading against itself — which is the whole reason the rule is stated
    twice.

    Returns
    -------
    bool
        Whether the reference reads a repository variable that is not declared.
    """
    return (
        reference.namespace == VARIABLE_NAMESPACE
        and reference.name != PERMITTED_VARIABLE_NAME
    )


def dotted(namespace: str, name: str) -> Fragment:
    """Return a `namespace.name` fragment, naming that reference.

    Returns
    -------
    Fragment
        The property de-reference, carrying the reference it names.
    """
    rendered = f"{namespace}.{name}"
    return Fragment(rendered, (Reference(namespace, name, rendered),))


def indexed(namespace: str, name: str, *, spacing: str = "") -> Fragment:
    """Return a `namespace['name']` fragment, naming that reference.

    The index syntax is a second spelling of the same reference, so the model
    records the same pair for it. ``spacing`` is the whitespace a workflow may
    write inside the brackets, which GitHub's grammar admits either way — the
    reference is the same one, and only its rendering differs.

    Returns
    -------
    Fragment
        The index spelling, carrying the reference it names.
    """
    rendered = f"{namespace}[{spacing}'{name}'{spacing}]"
    return Fragment(rendered, (Reference(namespace, name, rendered),))


def literal(text: str) -> Fragment:
    """Return a quoted literal fragment, which names nothing.

    Returns
    -------
    Fragment
        The literal, naming no reference.
    """
    return Fragment(f"'{text}'")


def _references_in_contexts(
    namespaces: st.SearchStrategy[str],
    names: st.SearchStrategy[str],
) -> st.SearchStrategy[Fragment]:
    """Return reference fragments drawn from the given contexts and names.

    Returns
    -------
    st.SearchStrategy
        Strategies for references in either spelling GitHub accepts.
    """
    return st.one_of(
        st.builds(dotted, namespace=namespaces, name=names),
        st.builds(
            indexed,
            namespace=namespaces,
            name=names,
            spacing=st.sampled_from(("", " ")),
        ),
    )


#: Fragments naming a reference in an arbitrary context, in either syntax.
REFERENCE_FRAGMENTS: typ.Final[st.SearchStrategy[Fragment]] = _references_in_contexts(
    ANY_NAMESPACE, VARIABLE_NAMES
)

#: Fragments a condition may hold when the only repository variable it names is
#: the declared one. Every `vars.` reference drawn here is the permitted name,
#: and the contexts that are not the variable namespace are unrestricted — so a
#: value built from these names something without naming anything undeclared.
PERMITTED_FRAGMENTS: typ.Final[st.SearchStrategy[Fragment]] = st.one_of(
    _references_in_contexts(
        st.just(VARIABLE_NAMESPACE), st.just(PERMITTED_VARIABLE_NAME)
    ),
    _references_in_contexts(OTHER_NAMESPACE, VARIABLE_NAMES),
)

#: Fragments that name nothing: quoted literals, and unrelated text.
SILENT_FRAGMENTS: typ.Final[st.SearchStrategy[Fragment]] = st.one_of(
    st.builds(literal, text=st.sampled_from(LITERAL_TEXTS)),
    st.builds(Fragment, text=PROSE_WORDS),
)
