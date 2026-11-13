"""Specify the cross-record agreement publication requires before upload.

Selection and declaration write independent records from the same staged tree,
so neither proves the copy on its own. The upload run therefore requires the
two records to agree before it compares the payload with either. These cases
pin both directions of that refusal: a digest that differs under one name, and
a name one record never saw.

Run via ``make test-release-publication``.
"""

import pytest
import release_digest_record
import release_digest_record_agreement

DIGEST = "ab" * 32
OTHER = "cd" * 32


def test_two_records_naming_the_same_bytes_agree() -> None:
    """Accept records written independently from one verified tree."""
    release_digest_record_agreement.require_records_agree(
        {"a": DIGEST}, declaration={"a": DIGEST}
    )


def test_records_that_differ_about_one_asset_are_refused() -> None:
    """Refuse records that describe different bytes under the same name."""
    with pytest.raises(release_digest_record.RecordedDigestMismatchError) as caught:
        release_digest_record_agreement.require_records_agree(
            {"a": DIGEST}, declaration={"a": OTHER}
        )

    assert "'a'" in str(caught.value), (
        f"the disagreeing asset must be named, got {caught.value}"
    )


@pytest.mark.parametrize(
    ("selection", "declaration"),
    [
        pytest.param({"a": DIGEST, "b": OTHER}, {"a": DIGEST}, id="selection-only"),
        pytest.param({"a": DIGEST}, {"a": DIGEST, "b": OTHER}, id="declaration-only"),
    ],
)
def test_a_record_naming_an_asset_the_other_omits_is_refused(
    selection: dict[str, str], declaration: dict[str, str]
) -> None:
    """Refuse either side naming an asset the other never saw.

    Absence is not agreement: a selection that hashed a file the declaration
    never covered, or the reverse, means the two stages looked at different
    trees and neither record speaks for the payload. Both directions are
    generated, so a comparison that only walks one record cannot pass by
    treating the other as authoritative.
    """
    with pytest.raises(release_digest_record.RecordedDigestMismatchError) as caught:
        release_digest_record_agreement.require_records_agree(
            selection, declaration=declaration
        )

    assert "['b']" in str(caught.value), (
        f"the absent asset must be named, got {caught.value}"
    )
