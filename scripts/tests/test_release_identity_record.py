"""Exercise the persisted release-identity validation boundary.

The candidate resolver validates facts gathered for one run; this suite checks
the separate mapping reader that reconstructs and re-validates saved facts.
The generated records and mutations stay private to this reader's suite.
"""

import pytest
import release_candidate
import release_identity_record
from hypothesis import given, settings
from hypothesis import strategies as st

_IDENTITY_FIELDS = (
    "tag",
    "commit",
    "package_name",
    "version",
    "bin_name",
    "is_prerelease",
)
_NUMERIC = st.integers(min_value=0, max_value=999).map(str)
_PRERELEASE_IDENTIFIER = st.one_of(
    _NUMERIC,
    st.from_regex(r"[A-Za-z][A-Za-z0-9-]{0,5}", fullmatch=True),
)
_METADATA_IDENTIFIER = st.from_regex(r"[A-Za-z][A-Za-z0-9-]{0,5}", fullmatch=True)
_CARGO_NAME = st.from_regex(r"[A-Za-z0-9][A-Za-z0-9_-]{0,10}", fullmatch=True)
_COMMIT = st.text(alphabet="0123456789abcdef", min_size=40, max_size=40)


@st.composite
def _identity_records(draw: st.DrawFn) -> dict[str, object]:
    """Generate valid records across the reader's independent field classes.

    Versions vary release or prerelease form and optional metadata; candidates
    vary tagged and tagless form, commit, package name, and binary name.

    Returns
    -------
    dict[str, object]
        A serialized identity accepted by the reader.
    """
    core = ".".join(draw(st.tuples(_NUMERIC, _NUMERIC, _NUMERIC)))
    prerelease = draw(
        st.one_of(
            st.none(),
            st.lists(_PRERELEASE_IDENTIFIER, min_size=1, max_size=2).map(".".join),
        )
    )
    metadata = draw(
        st.one_of(
            st.none(),
            st.lists(_METADATA_IDENTIFIER, min_size=1, max_size=2).map(".".join),
        )
    )
    version = core
    if prerelease is not None:
        version += f"-{prerelease}"
    if metadata is not None:
        version += f"+{metadata}"
    tag = f"v{version}" if draw(st.booleans()) else None
    facts = release_candidate.CandidateFacts(
        tag=tag,
        commit=draw(_COMMIT),
        package_name=draw(_CARGO_NAME),
        version=version,
        bin_name=draw(_CARGO_NAME),
    )
    return release_candidate.resolve_identity(facts).as_record()


@settings(max_examples=150, deadline=None, derandomize=True)
@given(record=_identity_records())
def test_valid_identity_records_round_trip(record: dict[str, object]) -> None:
    """Preserve every valid field when rebuilding a saved identity mapping."""
    rebuilt = release_identity_record.identity_from_mapping(record)

    assert rebuilt.as_record() == record, (
        "reading a valid identity record must preserve all its fields"
    )


@settings(max_examples=100, deadline=None, derandomize=True)
@given(record=_identity_records(), missing=st.sampled_from(_IDENTITY_FIELDS))
def test_each_required_identity_field_is_rejected_when_missing(
    record: dict[str, object], missing: str
) -> None:
    """Name the missing field instead of leaking an incidental key failure."""
    incomplete = dict(record)
    del incomplete[missing]

    with pytest.raises(release_identity_record.MissingFieldError) as caught:
        release_identity_record.identity_from_mapping(incomplete)

    assert missing in str(caught.value), (
        f"the missing-field diagnosis must identify {missing!r}"
    )


@settings(max_examples=100, deadline=None, derandomize=True)
@given(
    record=_identity_records(),
    invalid=st.one_of(st.none(), st.integers(), st.text(max_size=12)),
)
def test_a_non_boolean_prerelease_flag_is_rejected(
    record: dict[str, object], invalid: object
) -> None:
    """Require a real boolean so strings and integers cannot set release mode."""
    record["is_prerelease"] = invalid

    with pytest.raises(release_identity_record.RecordedFlagError):
        release_identity_record.identity_from_mapping(record)


@settings(max_examples=100, deadline=None, derandomize=True)
@given(record=_identity_records())
def test_a_prerelease_flag_that_disagrees_with_the_version_is_rejected(
    record: dict[str, object],
) -> None:
    """Reject either direction of a stale release-mode flag."""
    record["is_prerelease"] = not record["is_prerelease"]

    with pytest.raises(release_identity_record.RecordedFlagError):
        release_identity_record.identity_from_mapping(record)
