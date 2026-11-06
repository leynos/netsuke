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

#: Manifest-like names the reader must never select, because their path does
#: not normalise to the two-component packaged shape.
_WRONG_DEPTH_MANIFEST = st.one_of(
    st.just("Cargo.toml"),
    st.builds(
        lambda top, middle: f"{top}/{middle}/Cargo.toml",
        _COMPONENT,
        _COMPONENT,
    ),
    st.builds(
        lambda top, middle, leaf: f"{top}/{middle}/{leaf}/Cargo.toml",
        _COMPONENT,
        _COMPONENT,
        _COMPONENT,
    ),
)

#: Any member the reader cannot select, in either file kind. Adding one of
#: these to an archive is the metamorphic step: it must change nothing.
_UNSELECTABLE_MEMBER = st.tuples(
    st.one_of(_UNRELATED_NAME, _WRONG_DEPTH_MANIFEST), st.booleans()
)

#: Spellings a POSIX path library normalises to ``<root>/Cargo.toml``. The
#: reader compares normalised components, so each must select the same root.
_NORMALISED_MANIFEST = (
    "./{root}/Cargo.toml",
    "{root}/./Cargo.toml",
    "{root}//Cargo.toml",
)

_POLICY_NAME = st.text(
    alphabet="abcdefghijklmnopqrstuvwxyz0123456789_-", min_size=1, max_size=8
)
_POLICY_VERSION = st.sampled_from(("0.9.0", "1.0.0", "1.2.3"))


@dc.dataclass(frozen=True, slots=True)
class ArchiveLayout:
    """Describe the generated members of one candidate archive.

    ``manifests`` pairs each matching-shape ``<root>/Cargo.toml`` member with
    whether it is a regular file; the tuple's length is the selection count the
    reader must classify. ``unrelated`` holds members that can never be
    selected, including manifest-like names at the wrong depth. ``position``
    places the manifest members among the unrelated ones.
    """

    manifests: tuple[tuple[str, bool], ...]
    unrelated: tuple[tuple[str, bool], ...]
    position: int

    @property
    def roots(self) -> tuple[str, ...]:
        """The archive roots the matching-shape members name."""
        return tuple(
            pathlib.PurePosixPath(name).parent.name for name, _ in self.manifests
        )


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
    """Generate zero to three matching manifests at any position."""
    count = draw(st.integers(min_value=0, max_value=3))
    roots = draw(st.lists(_COMPONENT, min_size=count, max_size=count, unique=True))
    manifests = tuple((f"{root}/Cargo.toml", draw(st.booleans())) for root in roots)
    unrelated = draw(
        st.lists(
            st.tuples(st.one_of(_UNRELATED_NAME, _WRONG_DEPTH_MANIFEST), st.booleans()),
            max_size=4,
        )
    )
    return ArchiveLayout(
        manifests=manifests,
        unrelated=tuple(unrelated),
        position=draw(st.integers(min_value=0, max_value=len(unrelated))),
    )


def _members(layout: ArchiveLayout) -> tuple[tuple[str, bytes | None], ...]:
    """Build the archive members the layout describes, in their stated order."""
    manifests = tuple(
        (name, manifest_text().encode() if is_file else None)
        for name, is_file in layout.manifests
    )
    unrelated = tuple(
        (name, b"payload" if is_file else None) for name, is_file in layout.unrelated
    )
    return (
        *unrelated[: layout.position],
        *manifests,
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
def test_reader_accepts_exactly_one_regular_packaged_manifest(
    layout: ArchiveLayout,
) -> None:
    """Accept one regular manifest and classify every other selection count."""
    outcome = _read(_members(layout))

    if len(layout.manifests) != 1:
        assert outcome.issue is CandidateIssue.MANIFEST_MEMBER, (
            "zero or several matching members is a selection rejection"
        )
        return
    _, is_file = layout.manifests[0]
    if not is_file:
        assert outcome.issue is CandidateIssue.MANIFEST_FILE, (
            "a directory carrying the manifest name is a file-kind rejection"
        )
        return
    assert outcome.issue is None, "a unique regular manifest must be accepted"
    assert outcome.root == layout.roots[0], "report the root the archive holds"
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


@settings(max_examples=100, deadline=None, derandomize=True)
@given(
    layout=_layouts(),
    extra=st.lists(_UNSELECTABLE_MEMBER, min_size=1, max_size=4),
    insert_at=st.integers(min_value=0, max_value=8),
)
def test_adding_unselectable_members_does_not_change_the_reader_result(
    layout: ArchiveLayout,
    extra: list[tuple[str, bool]],
    insert_at: int,
) -> None:
    """Keep acceptance, rejection class, and reported facts as strangers appear.

    Unrelated members are what a real archive carries besides the packaged
    manifest: build output, extra files, and manifest-shaped names at the wrong
    depth. None of them can be selected, so adding them anywhere must leave the
    reader's answer untouched. ``insert_at`` clamps to the end so the final
    position stays reachable when the layout is short.
    """
    members = _members(layout)
    additions = tuple(
        (name, b"payload" if is_file else None) for name, is_file in extra
    )
    at = min(insert_at, len(members))
    augmented = (*members[:at], *additions, *members[at:])

    assert _read(augmented) == _read(members), (
        "a member the reader cannot select must not change what it reports"
    )


@settings(max_examples=100, deadline=None, derandomize=True)
@given(
    root=_COMPONENT,
    template=st.sampled_from(_NORMALISED_MANIFEST),
    is_file=st.booleans(),
)
def test_normalised_manifest_paths_select_the_same_root(
    root: str, template: str, *, is_file: bool
) -> None:
    """Select a manifest whose spelling normalises to the packaged shape."""
    name = template.format(root=root)
    outcome = _read(((name, manifest_text().encode() if is_file else None),))

    if not is_file:
        assert outcome.issue is CandidateIssue.MANIFEST_FILE, (
            "normalisation must not change the file-kind rejection"
        )
        return
    assert outcome.issue is None, "a normalised regular manifest must be accepted"
    assert outcome.root == root, "report the root the normalised path names"


@settings(max_examples=100, deadline=None, derandomize=True)
@given(names=st.lists(_WRONG_DEPTH_MANIFEST, min_size=1, max_size=4))
def test_manifest_like_names_at_other_depths_never_select_a_manifest(
    names: list[str],
) -> None:
    """Reject an archive whose manifest-like names sit at another depth."""
    members = tuple((name, manifest_text().encode()) for name in names)

    assert _read(members).issue is CandidateIssue.MANIFEST_MEMBER, (
        "a manifest name outside the packaged depth is not a selection"
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
