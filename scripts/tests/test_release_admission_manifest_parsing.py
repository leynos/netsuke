"""Pin the refusals the manifest reader owes an unreadable document.

:func:`~_release_admission.manifest.parse_manifest` is the one place untrusted
bytes become a manifest. Every way the document can be unusable has to arrive
as :attr:`~_release_admission.policy.ErrorCategory.MALFORMED_EVIDENCE`, because
the gate classifies refusals into a fixed vocabulary; an exception escaping the
decoder would instead surface as an unexplained failure with no category, and a
publishing run would fail closed for a reason the records do not name.

Two decoder failures are worth distinguishing here. A document that decodes to
something which is not a manifest is refused because the shape is wrong. A
decoder that gives up on the document at all -- as ``json`` does with
``RecursionError`` on deeply nested input, which is not a ``ValueError`` -- is
refused for the same reason. The pinned interpreter decodes the deepest
document the byte cap admits (32,768 nested arrays, 65,536 bytes) without
recursion, so the second case supplies the decoder failure itself and says so:
the branch is real on other interpreters, and this one cannot produce it from a
file.

Example (run from the repository root)::

    PYTHONPATH=scripts uv run --no-project --python 3.14 \
        --with pytest==9.0.2 --with 'cuprum==0.1.0' \
        python -m pytest scripts/tests/test_release_admission_manifest_parsing.py \
        -c /dev/null --rootdir=. -p no:cacheprovider
"""

from unittest import mock

import pytest
from _release_admission import evidence, manifest, policy

#: The deepest nesting a 65,536-byte document can hold: one bracket pair per
#: two bytes, around one scalar, with the scalar replacing a bracket pair.
MAXIMUM_DEPTH = evidence.MAX_EVIDENCE_BYTES // 2 - 1


def test_document_at_the_deepest_admitted_nesting_is_refused() -> None:
    """Refuse the deepest document the byte cap admits without recursing.

    Notes
    -----
    The premise is asserted alongside the verdict: if the document were larger
    than the cap, the reader would refuse it before the decoder saw it, and the
    case would prove nothing about nesting. What the decoder returns here is a
    list of lists, which is not a manifest, so the refusal is the shape rule at
    the extreme of the size bound.
    """
    raw = b"[" * MAXIMUM_DEPTH + b"0" + b"]" * MAXIMUM_DEPTH

    assert len(raw) <= evidence.MAX_EVIDENCE_BYTES, (
        "the case must fit inside the byte cap the reader enforces"
    )

    parsed = manifest.parse_manifest(raw)

    assert parsed.manifest is None, "a list of lists is not a manifest"
    assert parsed.error_category is policy.ErrorCategory.MALFORMED_EVIDENCE, (
        "an unreadable shape must be refused as malformed evidence"
    )


def test_decoder_recursion_is_refused_as_malformed_evidence() -> None:
    """Refuse a decoder that gives up on a deeply nested document.

    Notes
    -----
    ``json.loads`` reaches for ``RecursionError`` when its own nesting limit is
    passed rather than reporting a ``ValueError``, so a reader that catches
    only ``ValueError`` lets it escape. The interpreter this suite pins decodes
    the deepest document inside the byte cap, so the failure is supplied
    directly: the precondition the case needs is that the decoder raises it,
    and what is asserted is the category the gate reports.
    """
    fault = RecursionError("maximum recursion depth exceeded")

    with mock.patch.object(manifest.json, "loads", side_effect=fault):
        parsed = manifest.parse_manifest(b"{}")

    assert parsed.manifest is None, "a refused document must yield no manifest"
    assert parsed.error_category is policy.ErrorCategory.MALFORMED_EVIDENCE, (
        "an exhausted decoder must be refused as malformed evidence"
    )


#: A commit in the form the schema requires: the identity fields below are not
#: the subject of these cases, so they only have to be readable.
COMMIT = "a" * 40

#: One result entry in the shape the decoder accepts, named so a case can
#: repeat it or perturb one member of it.
_HISTORY_SCAN = (
    '"history_scan": {"outcome": "success", "observed_at": "2026-11-18T00:00:00+00:00"}'
)


def _manifest_text(*, results: str, extra: str = "") -> bytes:
    """Return a complete manifest carrying the supplied members as written.

    The members are supplied as JSON text rather than as Python values because
    a duplicate member is not expressible as a mapping: the point of these
    cases is the document a producer could send, which ``json`` accepts and a
    mapping cannot represent. The extra members are inserted whole, separator
    included, so a case that repeats a top-level name still produces a document
    that only the repetition makes unacceptable rather than one that is merely
    ill-formed.

    Returns
    -------
    bytes
        The encoded document.
    """
    inserted = f"{extra}," if extra else ""
    return (
        "{"
        '"schema_version": 1,'
        '"repository": "leynos/netsuke",'
        f'"commit": "{COMMIT}",'
        '"tag": "v1.2.3",'
        '"workflow_run_id": "12345678901",'
        '"issued_at": "2026-11-18T00:00:00+00:00",'
        f"{inserted}"
        f'"results": {{{results}}}'
        "}"
    ).encode()


def test_a_manifest_without_duplicate_members_is_accepted() -> None:
    """Accept the undisturbed document these cases perturb.

    Notes
    -----
    Without this control a refusal could be attributed to the duplicate member
    when it was really the surrounding document that never parsed. The
    assertion names the result the document declares, so acceptance means the
    decoder produced the content the cases below try to hide.
    """
    parsed = manifest.parse_manifest(_manifest_text(results=_HISTORY_SCAN))

    assert parsed.error_category is policy.ErrorCategory.NONE, (
        "a manifest with unique members must be accepted, or the duplicate "
        "cases below cannot attribute their refusal"
    )
    assert parsed.manifest is not None, "an accepted document must yield a manifest"
    assert set(parsed.manifest.results) == {"history_scan"}, (
        "the decoder must hand on the results the document declares"
    )


@pytest.mark.parametrize(
    ("label", "extra", "results"),
    [
        pytest.param(
            "repeated-result-name",
            "",
            f"{_HISTORY_SCAN}, {_HISTORY_SCAN}",
            id="repeated-result-name",
        ),
        pytest.param(
            "repeated-outcome",
            "",
            '"history_scan": {"outcome": "failure", "outcome": "success", '
            '"observed_at": "2026-11-18T00:00:00+00:00"}',
            id="repeated-outcome",
        ),
        pytest.param(
            "repeated-top-level-member",
            '"schema_version": 1',
            _HISTORY_SCAN,
            id="repeated-top-level-member",
        ),
    ],
)
def test_a_member_declared_twice_is_refused_as_malformed_evidence(
    label: str,
    extra: str,
    results: str,
) -> None:
    """Refuse a document that declares one member name twice in an object.

    Notes
    -----
    Each case is the accepted document with exactly one repeated member, so
    the refusal is attributable to the repetition. ``json`` keeps the last of
    the two, which is what makes acceptance dangerous: the earlier declaration
    is a failure the admission rules would never see.
    """
    parsed = manifest.parse_manifest(_manifest_text(results=results, extra=extra))

    assert parsed.manifest is None, (
        f"{label}: a document with a repeated member must yield no manifest"
    )
    assert parsed.error_category is policy.ErrorCategory.MALFORMED_EVIDENCE, (
        f"{label}: a repeated member leaves the document ambiguous, so it must "
        "be refused as malformed evidence"
    )
