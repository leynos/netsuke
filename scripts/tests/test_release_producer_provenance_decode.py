"""Hold the producer record's wire boundary to its own refusals.

A producer record arrives as JSON written by another job. Between those bytes
and the claim publication reasons about there are three questions about the
document rather than about the release: is it a string-keyed object, does it
carry every member, and is each member the type the policy expects. Those are
answered by :mod:`release_producer_provenance_decode`, and each failure gets
its own diagnosis here.

The *claim* refusals -- a commit that is not hexadecimal, a platform with no
package format, an unsafe asset name -- are deliberately not re-tested here.
They belong to ``scripts/tests/test_release_producer_provenance.py``, and a
case duplicated in both files would leave two places to update and no way to
tell which boundary actually refuses.

Run via ``make test-release-publication``.
"""

import pytest
import release_producer_provenance_decode
from release_producer_provenance_decode import decode_provenance
from release_producer_provenance_errors import (
    EmptyAssetsError,
    MalformedCommitError,
    MalformedRecordError,
    MissingMemberError,
    NonMappingRecordError,
    SchemaNotIntegerError,
    UnknownSchemaError,
    UnsafeAssetNameError,
)

COMMIT = "0123456789abcdef0123456789abcdef01234567"
DIGEST = "b" * 64

#: Distinguish "leave this member out" from "set this member to None".
_ABSENT = object()


def _document(**overrides: object) -> dict[str, object]:
    """Return a well-formed record with the requested members overridden."""
    document: dict[str, object] = {
        "schema": 1,
        "commit": COMMIT,
        "run_id": "1234",
        "run_attempt": "1",
        "target": "linux-x86_64",
        "platform": "linux",
        "assets": {"netsuke_1_amd64.deb": DIGEST},
    }
    for member, value in overrides.items():
        if value is _ABSENT:
            del document[member]
        else:
            document[member] = value
    return document


def test_a_complete_document_decodes_to_the_claim_it_describes() -> None:
    """Carry every member through to the claim, changing none of them."""
    claim = decode_provenance(_document())

    assert claim.schema == 1, claim
    assert claim.commit == COMMIT, claim
    assert claim.run_id == "1234", claim
    assert claim.run_attempt == "1", claim
    assert claim.target == "linux-x86_64", claim
    assert claim.platform == "linux", claim
    assert claim.assets == {"netsuke_1_amd64.deb": DIGEST}, claim


@pytest.mark.parametrize("document", [None, [], "record", 7, 1.0], ids=repr)
def test_a_document_that_is_not_an_object_is_refused(document: object) -> None:
    """Refuse to read members out of something that is not an object.

    A JSON array, a bare string, and a number all parse successfully, so the
    wrong type is only detectable here rather than by the parser.
    """
    with pytest.raises(NonMappingRecordError):
        decode_provenance(document)


def test_an_object_with_non_string_keys_is_refused() -> None:
    """Refuse a document JSON cannot have produced.

    Keys that are not strings cannot come from a JSON object, so this is a
    caller handing over a Python mapping rather than decoded JSON. Refusing
    keeps the reader's contract "decoded JSON" rather than "something
    mapping-shaped".
    """
    with pytest.raises(NonMappingRecordError):
        decode_provenance({1: "x", "commit": COMMIT})


@pytest.mark.parametrize(
    "member",
    ["schema", "commit", "run_id", "run_attempt", "target", "platform", "assets"],
)
def test_a_truncated_document_names_every_absent_member(member: str) -> None:
    """Report exactly which member is missing, for each member in turn."""
    with pytest.raises(MissingMemberError) as refusal:
        decode_provenance(_document(**{member: _ABSENT}))

    assert member in str(refusal.value), "the diagnosis must name the member"


def test_a_document_missing_several_members_names_all_of_them() -> None:
    """Report the whole absent set, so one edit fixes every gap.

    Reporting only the first would make a template with two omissions need
    two runs to diagnose.
    """
    document = _document(run_id=_ABSENT, platform=_ABSENT)

    with pytest.raises(MissingMemberError) as refusal:
        decode_provenance(document)

    assert "run_id" in str(refusal.value), (refusal.value, "must name run_id")
    assert "platform" in str(refusal.value), (refusal.value, "must name platform")


@pytest.mark.parametrize("schema", ["1", True, 1.0, None, []], ids=repr)
def test_a_schema_of_the_wrong_type_is_refused_at_the_wire(
    schema: object,
) -> None:
    """Refuse a schema that is not an integer before the policy sees it.

    ``True`` is the case worth naming: it is an ``int`` to Python, so a check
    that only asked ``isinstance(value, int)`` would read a template's
    ``true`` as a schema version.
    """
    with pytest.raises(SchemaNotIntegerError):
        decode_provenance(_document(schema=schema))


@pytest.mark.parametrize(
    "member", ["commit", "run_id", "run_attempt", "target", "platform"]
)
@pytest.mark.parametrize("value", [7, None, [], {}, ""], ids=repr)
def test_a_text_member_of_the_wrong_type_is_refused_at_the_wire(
    member: str, value: object
) -> None:
    """Refuse a non-text member before the policy tries to compare it.

    An empty string is refused too: every one of these members is compared
    against the release later, and an empty value would make that comparison
    report a stale candidate when the real fault is a truncated document.
    """
    with pytest.raises(MalformedRecordError) as refusal:
        decode_provenance(_document(**{member: value}))

    assert member in str(refusal.value), "the diagnosis must name the member"


@pytest.mark.parametrize("assets", [[], "assets", 3, None], ids=repr)
def test_an_asset_map_that_is_not_an_object_is_refused_by_the_policy(
    assets: object,
) -> None:
    """Pass the asset map through, and let the policy refuse its shape.

    The map's keys are data rather than members, so judging it is the policy's
    business. That the refusal is still the policy's own is what this case
    pins: the decoder must not half-validate it here.
    """
    with pytest.raises(MalformedRecordError):
        decode_provenance(_document(assets=assets))


def test_an_empty_asset_map_is_refused_by_the_policy() -> None:
    """Propagate the policy's refusal of a producer that claims nothing."""
    with pytest.raises(EmptyAssetsError):
        decode_provenance(_document(assets={}))


def test_an_unsafe_asset_name_is_refused_by_the_policy() -> None:
    """Propagate the policy's refusal of a name that escapes its directory."""
    with pytest.raises(UnsafeAssetNameError):
        decode_provenance(_document(assets={"../escape.deb": DIGEST}))


def test_a_malformed_commit_is_refused_by_the_policy_not_the_wire() -> None:
    """Separate "wrong type" from "wrong value" with two distinct diagnoses.

    The commit below is text, so the wire boundary accepts it; its *shape* is
    the policy's to refuse. Keeping the two apart is what lets an operator
    tell a corrupt file from a producer that built something unexpected.
    """
    with pytest.raises(MalformedCommitError):
        decode_provenance(_document(commit="not-a-commit"))


def test_an_unknown_schema_is_refused_by_the_policy_not_the_wire() -> None:
    """Accept schema ``2`` at the wire and refuse it as a claim.

    A newer producer writing a format this reader does not understand is a
    version-skew diagnosis, not a malformed document, and the two must not
    collapse into one message.
    """
    with pytest.raises(UnknownSchemaError):
        decode_provenance(_document(schema=2))


def test_the_decoder_is_the_one_name_the_tree_adapter_imports() -> None:
    """Require the tree adapter to decode through this module's one entry.

    ``release_payload_tree`` reads the file and hands the parsed value over;
    if it reached the policy's ``validated_provenance`` directly it would be
    responsible for checking members itself, and the two call paths would
    drift.
    """
    assert release_producer_provenance_decode.__all__ == ["decode_provenance"], (
        "the decoder must expose exactly one entry point"
    )
