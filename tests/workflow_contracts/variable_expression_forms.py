"""Expressions and step values, composed from the fragment vocabulary.

A fragment says what one piece of text names; these types say what *the whole*
names, which is what the scan is asked about. An expression holds fragments and
renders them in either of the two spellings GitHub evaluates a step's condition
in — wrapped in ``${{ }}`` delimiters, or bare — and a value holds expressions
among the unrelated text a step entry legitimately carries.

The composition is the direction the dependency runs: this module builds on
``variable_reference_forms`` and that one knows nothing of it. The split is also
what keeps each module within the size the linters allow.

Nothing here runs a workflow. A generated expression is text, and the properties
are stated over what the scan should *read* in it.

Run via ``make test-workflow-contracts``.
"""

import dataclasses as dc

from hypothesis import strategies as st
from variable_reference_forms import (
    PERMITTED_FRAGMENTS,
    PROSE_WORDS,
    REFERENCE_FRAGMENTS,
    SILENT_FRAGMENTS,
    Fragment,
    Reference,
    is_undeclared,
)


def _undeclared_references(
    references: tuple[Reference, ...],
) -> tuple[Reference, ...]:
    """Return the undeclared references in their input order.

    The filter keeps what the caller gave it: repeats survive, because a body
    naming the same undeclared variable twice is two occurrences to report, and
    so does the order, which is what a caller comparing the result against the
    text it was read from needs. Both model classes read their undeclared
    references through here, so an expression and a value cannot come to
    disagree about which references are the offending ones.

    Returns
    -------
    tuple[Reference, ...]
        The references ``is_undeclared`` accepts, in the order they were given.
    """
    return tuple(reference for reference in references if is_undeclared(reference))


@dc.dataclass(frozen=True, slots=True)
class Expression:
    """An expression body, and the references it names.

    One body, rendered in either of the two spellings GitHub evaluates a step's
    condition in: wrapped in the delimiters, or bare. The references are the
    same either way, which is exactly what makes the pair worth distinguishing —
    a scan that read only one spelling would report the other clean.
    """

    #: The body, without any `${{ }}` delimiters.
    body: str
    #: The references the body names, in the order it names them.
    references: tuple[Reference, ...] = ()

    @classmethod
    def of(cls, *fragments: Fragment) -> Expression:
        """Return the expression those fragments are written as.

        Returns
        -------
        Expression
            The fragments joined into one body, carrying their references in
            order.
        """
        return cls(
            " ".join(fragment.text for fragment in fragments),
            tuple(
                reference for fragment in fragments for reference in fragment.references
            ),
        )

    def render(self, *, delimited: bool = True) -> str:
        """Return the expression as a workflow writes it.

        Returns
        -------
        str
            The body in the delimiters, or bare when ``delimited`` is false.
        """
        if delimited:
            return f"${{{{ {self.body} }}}}"
        return self.body

    def undeclared(self) -> tuple[Reference, ...]:
        """Return the references naming an undeclared repository variable.

        Returns
        -------
        tuple[Reference, ...]
            The offending references, in the order the body names them.
        """
        return _undeclared_references(self.references)


@dc.dataclass(frozen=True, slots=True)
class Value:
    """One string a step holds: prose, and the expression regions written in it.

    The pieces are kept together rather than as two lists because a step's value
    is one string and the scan reads it as one: a region's position within the
    value is what its reported offsets are into, so a model that knew only which
    regions it wrote could not say where their references were read.
    """

    #: The regions and prose the value is written from, in order.
    pieces: tuple[Expression | str, ...] = ()

    def render(self) -> str:
        """Return the value as a workflow writes it.

        Prose is padded with a space on either side, so that a region never
        abuts the text beside it. The padding is layout rather than meaning —
        prose names nothing wherever it sits — but it keeps a generated value
        readable, and it keeps the delimiters from running into a word.

        Returns
        -------
        str
            The value, regions and prose joined in the order they were drawn.
        """
        parts = [
            piece.render() if isinstance(piece, Expression) else f" {piece} "
            for piece in self.pieces
        ]
        return "".join(parts)

    def references(self) -> tuple[Reference, ...]:
        """Return every reference the value's expressions name, in order.

        Returns
        -------
        tuple[Reference, ...]
            The references, in the order the value names them.
        """
        return tuple(
            reference
            for piece in self.pieces
            if isinstance(piece, Expression)
            for reference in piece.references
        )

    def undeclared(self) -> tuple[Reference, ...]:
        """Return the references naming an undeclared repository variable.

        Returns
        -------
        tuple[Reference, ...]
            The offending references, in the order the value names them.
        """
        return _undeclared_references(self.references())


@st.composite
def _expressions(
    draw: st.DrawFn,
    required: st.SearchStrategy[Fragment],
    surrounding: st.SearchStrategy[Fragment],
) -> Expression:
    """Draw an expression holding one ``required`` fragment among others.

    The required fragment is placed at a drawn position rather than always at
    the front, because where a reference sits is a case in its own right: the
    observed failure this scan was written for was a reference that did not
    follow the opening delimiter, and one that led every generated expression
    would leave that position untested.

    Returns
    -------
    Expression
        Strategies for expressions built from the drawn fragments.
    """
    fragments = draw(st.lists(surrounding, max_size=3))
    position = draw(st.integers(min_value=0, max_value=len(fragments)))
    fragments.insert(position, draw(required))
    return Expression.of(*fragments)


def conditions() -> st.SearchStrategy[Expression]:
    """Return a strategy for a step's condition.

    At least one reference is guaranteed, so a property stated over the result
    is never comparing two empty collections; the fragments beside it may name
    references of their own, which is how repeated and multiple references are
    reached.

    Returns
    -------
    st.SearchStrategy
        Strategies for expression bodies a condition may hold.
    """
    return _expressions(
        REFERENCE_FRAGMENTS, st.one_of(REFERENCE_FRAGMENTS, SILENT_FRAGMENTS)
    )


def permitted_conditions() -> st.SearchStrategy[Expression]:
    """Return a strategy for conditions naming only the declared variable.

    Every `vars.` reference the strategy writes is the permitted name defined
    beside the fragment vocabulary, and at least one reference is guaranteed —
    so a property asserting that nothing is reported is asserting it about a
    condition that does name something, rather than about an empty expression.

    Returns
    -------
    st.SearchStrategy
        Strategies for conditions the repository has declared what they read.
    """
    return _expressions(
        PERMITTED_FRAGMENTS, st.one_of(PERMITTED_FRAGMENTS, SILENT_FRAGMENTS)
    )


def silent_expressions() -> st.SearchStrategy[Expression]:
    """Return a strategy for expressions that name nothing at all.

    At least one fragment is guaranteed, so the expressions are text a workflow
    could have written rather than the empty string, and the property stated
    over them is about prose that *looks* like a reference being left alone
    rather than about nothing being reported for nothing.

    Returns
    -------
    st.SearchStrategy
        Strategies for expressions built only from literals and unrelated text.
    """

    @st.composite
    def build(draw: st.DrawFn) -> Expression:
        """Draw an expression from the fragments that name nothing."""
        return Expression.of(*draw(st.lists(SILENT_FRAGMENTS, min_size=1, max_size=3)))

    return build()


@st.composite
def values(draw: st.DrawFn) -> Value:
    """Draw a step value: one or more regions, and prose around them.

    At least one region is guaranteed, so the value names something; the prose
    pieces are the unrelated text a step legitimately holds, and they appear
    between regions as readily as beside them.

    Returns
    -------
    Value
        Strategies for values a non-condition step entry may hold.
    """
    piece = st.one_of(conditions(), PROSE_WORDS)
    pieces: list[Expression | str] = [draw(conditions())]
    pieces.extend(draw(st.lists(piece, max_size=3)))
    return Value(tuple(pieces))
