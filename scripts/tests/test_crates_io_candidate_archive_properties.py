"""State the archive manifest selection rule over generated member layouts.

The focused examples in ``test_crates_io_candidate_guards`` pin one layout
each: a foreign root, a duplicate manifest, a directory carrying the manifest
name, and an absent manifest. This module states the rule those examples
sample: the reader accepts an archive exactly when it holds one regular
``<root>/Cargo.toml`` member, wherever that member sits among unrelated
entries, and the policy accepts the reported facts only when both the root and
the packaged identity agree with the release.

Run via ``make test-release-publication``.
"""

import dataclasses as dc
import pathlib
import tempfile

from crates_io_candidate_archive import read_packaged_manifest
from crates_io_candidate_errors import CandidateError, CandidateIssue
from crates_io_candidate_policy import require_manifest_identity
from crates_io_candidate_support import (
    PACKAGE_NAME,
    VERSION,
    manifest_text,
    tar_bytes,
)
from hypothesis import given, settings
from hypothesis import strategies as st

#: A single POSIX path segment, excluding the manifest's own file name.
_COMPONENT = st.text(
    alphabet="abcdefghijklmnopqrstuvwxyz0123456789_-", min_size=1, max_size=8
).filter(lambda part: part != "Cargo.toml")

#: Member names that can never select a package manifest, at any depth.
_UNRELATED_NAME = st.one_of(
    _COMPONENT,
    st.builds(lambda parent, leaf: f"{parent}/{leaf}", _COMPONENT, _COMPONENT),
    st.builds(
        lambda top, middle, leaf: f"{top}/{middle}/{leaf}",
        _COMPONENT,
        _COMPONENT,
        _COMPONENT,
    ),
)

_POLICY_NAME = st.text(
    alphabet="abcdefghijklmnopqrstuvwxyz0123456789_-", min_size=1, max_size=8
)
_POLICY_VERSION = st.sampled_from(("0.9.0", "1.0.0", "1.2.3"))


@dc.dataclass(frozen=True, slots=True)
class ArchiveLayout:
    """Describe the generated members of one candidate archive.

    ``unrelated`` pairs a member name with whether the entry is a regular file;
    a directory entry lets the property exercise the file-kind guard away from
    the manifest itself. ``position`` places the manifest member among them.
    """

    root: str
    manifest_is_file: bool
    unrelated: tuple[tuple[str, bool], ...]
    position: int


@dc.dataclass(frozen=True, slots=True)
class ReadOutcome:
    """Report what the archive reader returned or the issue it classified."""

    root: str | None = None
    manifest: dict[str, object] | None = None
    issue: CandidateIssue | None = None


@dc.dataclass(frozen=True, slots=True)
class PolicyExpectation:
    """Pair the reader's facts with the release identity they are compared to.

    Each fact is generated either to agree with the release or to differ from
    it. Five independent draws would almost never agree, so the accept branch
    and both rejection classes would go unexercised.
    """

    root: str
    name: str
    version: str
    expected_name: str
    expected_version: str

    @property
    def expected_root(self) -> str:
        """The root the release identity requires."""
        return f"{self.expected_name}-{self.expected_version}"


@st.composite
def _layouts(draw: st.DrawFn) -> ArchiveLayout:
    """Generate an archive layout with the manifest at any position."""
    unrelated = draw(st.lists(st.tuples(_UNRELATED_NAME, st.booleans()), max_size=4))
    return ArchiveLayout(
        root=draw(_COMPONENT),
        manifest_is_file=draw(st.booleans()),
        unrelated=tuple(unrelated),
        position=draw(st.integers(min_value=0, max_value=len(unrelated))),
    )


def _members(layout: ArchiveLayout) -> tuple[tuple[str, bytes | None], ...]:
    """Build the archive members the layout describes, in their stated order."""
    manifest = (
        (
            f"{layout.root}/Cargo.toml",
            manifest_text().encode() if layout.manifest_is_file else None,
        ),
    )
    unrelated = tuple(
        (name, b"payload" if is_file else None) for name, is_file in layout.unrelated
    )
    return (
        *unrelated[: layout.position],
        *manifest,
        *unrelated[layout.position :],
    )


def _read(members: tuple[tuple[str, bytes | None], ...]) -> ReadOutcome:
    """Read one generated archive and report the result or classified issue."""
    with tempfile.TemporaryDirectory() as directory:
        archive = pathlib.Path(directory) / "candidate.crate"
        _ = archive.write_bytes(tar_bytes(members))
        try:
            root, manifest = read_packaged_manifest(archive)
        except CandidateError as error:
            return ReadOutcome(issue=error.issue)
        return ReadOutcome(root=root, manifest=manifest)


def _package_identity(manifest: dict[str, object] | None) -> tuple[object, object]:
    """Return the packaged name and version from a parsed manifest."""
    assert manifest is not None, "an accepted archive reports its manifest"
    package = manifest["package"]
    assert isinstance(package, dict), "an accepted manifest holds a package table"
    return package["name"], package["version"]


@settings(max_examples=100, deadline=None, derandomize=True)
@given(layout=_layouts())
def test_reader_selects_one_regular_manifest_wherever_it_sits(
    layout: ArchiveLayout,
) -> None:
    """Accept exactly one regular packaged manifest among unrelated entries."""
    outcome = _read(_members(layout))

    if not layout.manifest_is_file:
        assert outcome.issue is CandidateIssue.MANIFEST_FILE, (
            "a directory carrying the manifest name is a file-kind rejection"
        )
        return
    assert outcome.issue is None, "a unique regular manifest must be accepted"
    assert outcome.root == layout.root, "report the root the archive holds"
    assert _package_identity(outcome.manifest) == (PACKAGE_NAME, VERSION), (
        "report the packaged identity the policy compares"
    )


@settings(max_examples=100, deadline=None, derandomize=True)
@given(layout=_layouts(), reverse=st.booleans())
def test_unrelated_member_order_does_not_change_the_reader_result(
    layout: ArchiveLayout, *, reverse: bool
) -> None:
    """Keep the reader result independent of unrelated member order."""
    members = _members(layout)
    reordered = members if not reverse else (*reversed(members),)

    assert _read(reordered) == _read(members), (
        "reordering unrelated members must not change acceptance or its facts"
    )


def _policy_issue(
    root: str,
    manifest: dict[str, object],
    expected_name: str,
    expected_version: str,
) -> CandidateIssue | None:
    """Return the issue the policy classifies, or ``None`` when it accepts."""
    try:
        require_manifest_identity(root, manifest, expected_name, expected_version)
    except CandidateError as error:
        return error.issue
    return None


@st.composite
def _policy_expectations(draw: st.DrawFn) -> PolicyExpectation:
    """Generate reader facts that agree with the release or differ from it."""
    expected_name = draw(_POLICY_NAME)
    expected_version = draw(_POLICY_VERSION)
    expected_root = f"{expected_name}-{expected_version}"
    return PolicyExpectation(
        root=draw(st.sampled_from((expected_root, f"{expected_root}-other"))),
        name=draw(st.sampled_from((expected_name, f"{expected_name}-other"))),
        version=draw(st.sampled_from((expected_version, "9.9.9"))),
        expected_name=expected_name,
        expected_version=expected_version,
    )


@settings(max_examples=100, deadline=None, derandomize=True)
@given(case=_policy_expectations())
def test_policy_accepts_only_a_matching_root_and_packaged_identity(
    case: PolicyExpectation,
) -> None:
    """Accept the reader facts exactly when root and identity both agree."""
    manifest: dict[str, object] = {
        "package": {"name": case.name, "version": case.version}
    }
    agrees = (case.root, case.name, case.version) == (
        case.expected_root,
        case.expected_name,
        case.expected_version,
    )

    issue = _policy_issue(
        case.root, manifest, case.expected_name, case.expected_version
    )

    if agrees:
        assert issue is None, "a fully agreeing candidate must be accepted"
        return
    expected_issue = (
        CandidateIssue.MANIFEST_ROOT
        if case.root != case.expected_root
        else CandidateIssue.MANIFEST_IDENTITY
    )
    assert issue is expected_issue, "classify the disagreement precisely"
