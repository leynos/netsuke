"""Check that evidence attribution admits exactly the producer's own digest.

``evidence.is_attributed`` decides whether a manifest the gate read is the one
the producer job published. The rule is narrow: the published digest must be
well-formed lowercase SHA-256 hexadecimal, and it must equal the digest of the
exact bytes. These properties generate documents and digests, including
digests that are one character away from the correct value or differ only in
case, and compare the verdict with that rule.

The generated documents are arbitrary bytes rather than manifests, because the
attribution check is a byte-level comparison and must not depend on the
manifest's shape.
"""

import hashlib
import re

from _release_admission import evidence
from hypothesis import event, given, settings
from hypothesis import strategies as st

#: The one spelling of a published digest the producer emits.
SHA256_SPELLING = r"[0-9a-f]{64}"
#: Upper-case spelling of the same digest shape, which must not attribute.
UPPER_CASE_SPELLING = r"[0-9A-F]{64}"


def _perturbed(digest: str) -> str:
    """Return a digest of the same shape that differs in its final character."""
    last = "1" if digest[-1] == "0" else "0"
    return digest[:-1] + last


@st.composite
def _document_with_digest(draw: st.DrawFn) -> tuple[bytes, str]:
    """Draw a document together with a digest that is correct, near, or arbitrary."""
    document = draw(st.binary(max_size=512))
    correct = hashlib.sha256(document).hexdigest()
    digest = draw(
        st.one_of(
            st.just(correct),
            st.just(correct.upper()),
            st.just(_perturbed(correct)),
            st.from_regex(SHA256_SPELLING, fullmatch=True),
            st.from_regex(UPPER_CASE_SPELLING, fullmatch=True),
            st.text(max_size=80),
        )
    )
    return document, digest


@settings(max_examples=300, deadline=None, derandomize=True)
@given(pair=_document_with_digest())
def test_attribution_matches_the_well_formed_correct_digest_rule(
    pair: tuple[bytes, str],
) -> None:
    """Attribution holds exactly when the digest is well formed and names the bytes."""
    document, digest = pair
    is_well_formed = re.fullmatch(SHA256_SPELLING, digest) is not None
    names_document = digest == hashlib.sha256(document).hexdigest()
    expected = is_well_formed and names_document
    event(f"expected attribution: {expected}")
    actual = evidence.is_attributed(document, digest)
    assert actual is expected, (
        f"is_attributed must be {expected} for digest {digest!r} over "
        f"{len(document)} bytes; well formed={is_well_formed}, "
        f"names document={names_document}"
    )


@settings(max_examples=200, deadline=None, derandomize=True)
@given(document=st.binary(max_size=512))
def test_the_producers_own_digest_always_attributes(document: bytes) -> None:
    """The digest the producer computes over a document attributes that document."""
    published = hashlib.sha256(document).hexdigest()
    assert evidence.is_attributed(document, published) is True, (
        f"the SHA-256 of {len(document)} bytes must attribute those bytes"
    )


@settings(max_examples=200, deadline=None, derandomize=True)
@given(document=st.binary(max_size=512), suffix=st.binary(min_size=1, max_size=8))
def test_a_digest_of_other_bytes_never_attributes(
    document: bytes, suffix: bytes
) -> None:
    """A digest computed over one document never attributes a different document."""
    published = hashlib.sha256(document).hexdigest()
    other = document + suffix
    assert evidence.is_attributed(other, published) is False, (
        "the digest of one document must not attribute a longer document"
    )
