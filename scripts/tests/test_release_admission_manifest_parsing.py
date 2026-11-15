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

import sys
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / ".github" / "scripts"))
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
