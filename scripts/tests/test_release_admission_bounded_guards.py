"""Pin the two guards that narrow a value before anything is published on it.

Both functions decide whether one string is safe to carry further, and both fail
closed by refusing rather than by repairing. :func:`evidence.is_evidence_max_age`
holds the freshness window's maximum to ASCII digits, no leading zero, a bounded
length, and the policy's one-week ceiling: a mistyped bound must not widen the
window, and a value the gate cannot read must not be read as "no bound". The
maximum age is the only input that decides how long evidence stays admissible,
so it is checked before any operation runs.

:func:`policy.publishable_revision` decides what reaches ``GITHUB_OUTPUT``, a
line-oriented file: a revision containing a newline would forge a second
``key=value`` line and invent an output the publication job then trusts. A
revision that is not a full object ID therefore publishes as the empty string,
which the publication guard's comparison with ``github.sha`` refuses.

These are named tables rather than examples because each row is a distinct
contract: the boundary value that is accepted, the first value past it, and the
spellings that look close enough to be mistaken for it. Run via
``make test-release-admission``.
"""

import pytest
from _release_admission import evidence, policy

#: A full SHA-1 object ID, spelled in lowercase hexadecimal.
SHA1_REVISION = "0123456789abcdef0123456789abcdef01234567"
#: A full SHA-256 object ID, which a repository may be created with.
SHA256_REVISION = "0123456789abcdef" * 4


@pytest.mark.parametrize(
    "value",
    [
        pytest.param("1", id="one-second"),
        pytest.param("86400", id="one-day"),
        pytest.param(
            str(evidence.MAX_EVIDENCE_MAX_AGE_SECONDS), id="the-ceiling-itself"
        ),
    ],
)
def test_a_bounded_maximum_age_is_accepted(value: str) -> None:
    """Accept every positive age up to and including the policy ceiling."""
    assert evidence.is_evidence_max_age(value), (
        f"{value!r} is inside the bounded window and must be accepted"
    )


@pytest.mark.parametrize(
    "value",
    [
        pytest.param("", id="empty"),
        pytest.param("0", id="zero"),
        pytest.param("00", id="two-zeros"),
        pytest.param("0604800", id="leading-zero"),
        pytest.param(
            str(evidence.MAX_EVIDENCE_MAX_AGE_SECONDS + 1), id="one-past-the-ceiling"
        ),
        pytest.param("1000000", id="more-digits-than-the-bound-allows"),
        pytest.param(" 86400", id="leading-space"),
        pytest.param("86400 ", id="trailing-space"),
        pytest.param("86400\n", id="trailing-newline"),
        pytest.param("-1", id="negative"),
        pytest.param("+1", id="explicit-sign"),
        pytest.param("8.64e4", id="exponent-form"),
        pytest.param("٨٦٤٠٠", id="non-ascii-digits"),
    ],
)
def test_an_unbounded_maximum_age_is_refused(value: str) -> None:
    """Refuse every spelling that is not a plain bounded decimal age.

    Each row is either a value outside the ceiling or a string that a reader
    which merely tried ``int`` would accept: Python parses the sign, the
    surrounding whitespace, the newline, and the non-ASCII digits, so the guard
    has to judge the spelling rather than the parsed number.
    """
    assert not evidence.is_evidence_max_age(value), (
        f"{value!r} is not a bounded maximum age and must be refused"
    )


@pytest.mark.parametrize(
    "revision",
    [
        pytest.param(SHA1_REVISION, id="sha1"),
        pytest.param(SHA256_REVISION, id="sha256"),
    ],
)
def test_a_full_object_id_is_publishable(revision: str) -> None:
    """Publish a full lowercase object ID exactly as it was supplied."""
    assert policy.publishable_revision(revision) == revision, (
        "a full object ID must reach the workflow output unchanged"
    )


@pytest.mark.parametrize(
    "revision",
    [
        pytest.param("", id="empty"),
        pytest.param("a" * 39, id="one-short"),
        pytest.param("a" * 41, id="one-long"),
        pytest.param("a" * 63, id="short-of-sha256"),
        pytest.param("a" * 65, id="past-sha256"),
        pytest.param("A" * 40, id="uppercase"),
        pytest.param("g" * 40, id="not-hexadecimal"),
        pytest.param(f"{SHA1_REVISION}\n", id="trailing-newline"),
    ],
)
def test_anything_but_a_full_object_id_publishes_as_empty(revision: str) -> None:
    """Refuse to publish a value that could forge a workflow-output line.

    The newline row is the one that matters most: the revision is interpolated
    into a ``key=value`` file, so a revision that ends in a newline would write
    a second line the publication job reads as another output. The width rows
    refuse a truncated or overlong value, and the case rows refuse an object ID
    spelled in a form Git does not produce.
    """
    assert not policy.publishable_revision(revision), (
        f"{revision!r} is not a full lowercase object ID and must publish as empty"
    )
