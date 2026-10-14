"""Verify the CI helper scripts stay inside the directory they are handed.

Two of the helpers name filesystem entries from outside the process.
``ci_archive.extract_members`` writes archive members whose names come from a
downloaded release artifact, and ``discard_instrumented_tree.discard`` removes
subtrees whose names come from a workflow input. Both join that name to a
directory the caller chose, and both are destructive or attacker-influenced, so
"the name is one entry beneath that directory" is a safety property rather than
an input-validation nicety.

The examples in :mod:`test_ci_archive` and
:mod:`test_ci_discard_instrumented_tree` pin the documented cases -- one nested
member, one missing member, one symlink, the two default subtree names. Those
are the shapes a reader can hold in their head. They cannot cover the shapes an
archive or a workflow can actually carry, so the properties here generate
names across the whole space and assert the containment claim directly:

* a member either raises ``CiScriptError`` or is written below the destination,
  never beside it;
* a subtree name either raises ``CiScriptError`` or removes something below the
  target, never outside it.

The oracle is a fixed tree outside the operated-on directory, snapshotted
before and compared after. It is deliberately independent of the scripts' own
notion of a safe name: a property whose expectation is computed by the code
under test proves nothing.

Run via ``make test-ci-scripts``.
"""

import contextlib
import typing as typ

import pytest
from ci_script_support import load_ci_script, write_tarball
from hypothesis import given, settings
from hypothesis import strategies as st

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    import pathlib

ci_archive = load_ci_script("ci_archive")
script = load_ci_script("discard_instrumented_tree")

ORDINARY_COMPONENT = st.text(alphabet="ab-_019.", min_size=1, max_size=8)

#: Shapes that name something other than a single ordinary entry. ``..`` and
#: ``.`` are their own base names, so a naive flattening keeps them and lands on
#: the destination's parent or on the destination itself; the absolute and
#: multi-component shapes cover the other two ways a join can leave its base.
#: ``./x``, ``dir/`` and ``x/.`` are included although they are safe -- they
#: normalise to one plain component -- because a guard that rejected them would
#: be over-strict, and the property below is what distinguishes the two.
HOSTILE_SHAPE = st.sampled_from([
    "",
    ".",
    "..",
    "/",
    "//",
    "./x",
    "dir/",
    "x/.",
    "a/../b",
    "a/./b",
    "a/b",
    "nested/other",
    "/nonexistent-ci-containment-probe",
    "/nonexistent-ci-containment-probe/nested",
])

#: Names an archive member can carry. Extraction only ever opens a file below
#: the destination, so every shape is safe to generate here.
MEMBER_NAME = st.one_of(
    ORDINARY_COMPONENT,
    HOSTILE_SHAPE,
    st.lists(ORDINARY_COMPONENT, min_size=2, max_size=3).map("/".join),
)

#: Names a subtree removal can carry. These are bounded to three components,
#: each ``.``, ``..``, or a plain name, and the target in the test below sits
#: four levels under ``tmp_path``. The deepest path a removal can therefore
#: reach is ``tmp_path/a``, inside the directory this test owns. That bound is
#: what makes the property safe to run against a build whose guard has been
#: removed: the escape it would then demonstrate stays within the fixture, and
#: the assertion below reports it rather than the suite destroying another
#: test's scratch space. Absolute shapes are confined to paths that do not
#: exist, for the same reason.
SUBTREE_NAME = st.one_of(
    ORDINARY_COMPONENT,
    HOSTILE_SHAPE,
    st.lists(st.sampled_from(["..", ".", "a", "b"]), min_size=1, max_size=3).map(
        "/".join
    ),
)


def _outside_structure(root: pathlib.Path) -> pathlib.Path:
    """Build the fixed tree that must survive, and return its root.

    It carries a plain file, a nested file, and an empty directory so that a
    removal which reached it would have to leave a trace at every depth, and an
    empty directory so that a removal that only emptied rather than deleted it
    is caught too.

    Returns
    -------
    pathlib.Path
        The ``outside`` directory, the root of the tree to compare against.
    """
    outside = root / "outside"
    (outside / "nested").mkdir(parents=True)
    (outside / "sentinel.txt").write_bytes(b"sentinel")
    (outside / "nested" / "deep.txt").write_bytes(b"deep")
    (outside / "empty").mkdir()
    return outside


def _workspace(factory: pytest.TempPathFactory) -> pathlib.Path:
    """Return a fresh scratch directory, for one generated input.

    Hypothesis runs many inputs through one test invocation, and a fixture is
    not reset between them -- which its ``function_scoped_fixture`` health check
    reports, and rightly: reusing one directory would let an earlier input's
    leftovers decide a later input's outcome, which is exactly the interference
    these properties exist to detect. The directory is therefore created from
    the session-scoped factory on every call, so each input starts empty.

    Returns
    -------
    pathlib.Path
        A newly created, empty scratch directory.
    """
    return factory.mktemp("containment")


def _snapshot(root: pathlib.Path) -> dict[str, bytes | None]:
    """Return every path under ``root``, directories mapped to ``None``."""
    snapshot: dict[str, bytes | None] = {}
    for path in sorted(root.rglob("*")):
        snapshot[str(path.relative_to(root))] = (
            None if path.is_dir() else path.read_bytes()
        )
    return snapshot


def _assert_destination_holds_only(
    destination: pathlib.Path, extracted: cabc.Sequence[pathlib.Path]
) -> None:
    """Assert each extracted path is one plain file directly in ``destination``."""
    assert len(extracted) == 1, "one named member must produce exactly one file"
    for path in extracted:
        assert path.resolve().parent == destination.resolve(), (
            f"{path} must be written directly into {destination}, not beside it"
        )
    assert sorted(destination.iterdir()) == sorted(extracted), (
        "extraction must write nothing the caller did not name"
    )


@given(name=MEMBER_NAME)
@settings(deadline=None, max_examples=50)
def test_extract_members_never_writes_outside_the_destination(
    name: str, tmp_path_factory: pytest.TempPathFactory
) -> None:
    """A member name is refused, or written below the destination, never beside.

    The archive holds the generated name as a regular file, so the property
    exercises the write path rather than a lookup miss: a name that is simply
    absent would take the "has no member" branch and prove nothing about
    containment.
    """
    workspace = _workspace(tmp_path_factory)
    destination = workspace / "dest"
    destination.mkdir()
    outside = _outside_structure(workspace)
    before = _snapshot(outside)
    archive = write_tarball(workspace / "a.tar.gz", {name: b"payload"})

    try:
        extracted = ci_archive.extract_members(archive, destination, (name,))
    except ci_archive.CiScriptError:
        assert not list(destination.iterdir()), (
            f"a refused member {name!r} must leave the destination empty"
        )
    else:
        _assert_destination_holds_only(destination, extracted)

    assert _snapshot(outside) == before, (
        f"extracting {name!r} must not touch anything outside {destination}"
    )


@given(name=SUBTREE_NAME)
@settings(deadline=None, max_examples=50)
def test_discard_never_removes_outside_the_target(
    name: str, tmp_path_factory: pytest.TempPathFactory
) -> None:
    """A subtree name is refused, or removes below the target, never outside.

    The target holds two children so that a removal naming one of them is
    observable as a real deletion rather than as an absent path, and the
    sibling outside the target carries the tree that must survive.
    """
    workspace = _workspace(tmp_path_factory)
    outside = _outside_structure(workspace)
    before = _snapshot(outside)
    target = workspace / "a" / "b" / "target"
    children = ("child-a", "child-b")
    for child in children:
        (target / child).mkdir(parents=True)
        (target / child / "obj").write_bytes(b"x")

    with contextlib.suppress(script.CiScriptError):
        script.discard(target, (name,))

    assert target.is_dir(), f"discarding {name!r} must not remove the target itself"
    assert _snapshot(outside) == before, (
        f"discarding {name!r} must not touch anything outside {target}"
    )
    for child in children:
        if child != name:
            assert (target / child).is_dir(), (
                f"discarding {name!r} must leave {child} alone"
            )


@pytest.mark.parametrize(
    "name",
    [
        "",
        ".",
        "..",
        "/",
        "//",
        "/etc/passwd",
        "a/b",
        "nested/other",
        "a/../b",
        "a/./b",
        "../..",
    ],
)
def test_discard_refuses_a_name_that_is_not_one_plain_component(
    name: str, tmp_path: pathlib.Path
) -> None:
    """Every unsafe shape is refused by name, before any filesystem call.

    The property above proves containment for these shapes; this pins the
    refusal itself, so a guard that quietly stopped rejecting them -- and
    instead relied on the path happening not to exist -- is still caught.
    """
    target = tmp_path / "target"
    target.mkdir()

    with pytest.raises(script.CiScriptError, match="single plain path component"):
        script.discard(target, (name,))


@pytest.mark.parametrize("name", ["./x", "dir/", "x/."])
def test_discard_accepts_a_name_that_normalises_to_one_component(
    name: str, tmp_path: pathlib.Path
) -> None:
    """A redundant separator or ``.`` is normalised away, not refused.

    These spell one ordinary component, so refusing them would reject a name
    that cannot escape. The distinction matters because the guard is a shape
    test on the resolved path, not a denylist of substrings: ``dir/`` and
    ``a/../b`` both contain a separator, and only the second one leaves the
    directory.
    """
    target = tmp_path / "target"
    (target / "x").mkdir(parents=True)
    (target / "dir").mkdir()

    removed = script.discard(target, (name,))

    assert len(removed) == 1, "the normalised name must resolve to one subtree"
    assert not removed[0].exists(), "the named subtree must be removed"
    assert removed[0].parent == target, "the removal must stay inside the target"


def test_discard_refuses_an_absolute_name_naming_a_real_directory(
    tmp_path: pathlib.Path,
) -> None:
    """An absolute name cannot reach a directory outside the target.

    The shapes above are all paths that happen not to exist, so a guard that
    merely skipped absent paths would still pass them. This one names a
    directory the test owns, which a join would have removed outright.
    """
    outside = _outside_structure(tmp_path)
    before = _snapshot(outside)
    target = tmp_path / "target"
    target.mkdir()

    with pytest.raises(script.CiScriptError, match="single plain path component"):
        script.discard(target, (str(outside),))

    assert _snapshot(outside) == before, (
        "the refused removal must leave the named directory untouched"
    )


def test_extract_members_flattens_two_members_sharing_a_base_name(
    tmp_path: pathlib.Path,
) -> None:
    """Two members with one base name resolve to one path; the last one wins.

    The archive is a release artifact, so this is reachable rather than
    hypothetical. Flattening is the documented behaviour -- a member is written
    under its own base name -- and the collision is silent, so the property
    pins which of the two survives rather than leaving it to dictionary order.
    """
    destination = tmp_path / "dest"
    destination.mkdir()
    archive = write_tarball(
        tmp_path / "a.tar.gz", {"first/tool": b"first", "second/tool": b"second"}
    )

    extracted = ci_archive.extract_members(
        archive, destination, ("first/tool", "second/tool")
    )

    assert extracted == [destination / "tool", destination / "tool"], (
        "both members must flatten to the same path"
    )
    assert (destination / "tool").read_bytes() == b"second", (
        "the member named last must be the one left in place"
    )


def test_extract_members_refuses_a_directory_member(tmp_path: pathlib.Path) -> None:
    """A member that is a directory is refused, not created or descended into.

    The symlink case has its own example; a directory is the other non-regular
    type a hostile archive can carry, and it is the one a naive extraction
    would create rather than refuse.
    """
    import io
    import tarfile

    archive = tmp_path / "a.tar.gz"
    with tarfile.open(archive, "w:gz") as tar:
        info = tarfile.TarInfo("tool")
        info.type = tarfile.DIRTYPE
        tar.addfile(info, io.BytesIO())

    with pytest.raises(ci_archive.CiScriptError, match="is not a regular file"):
        ci_archive.extract_members(archive, tmp_path, ("tool",))
    assert not (tmp_path / "tool").exists(), (
        "a refused directory member must not be created"
    )
