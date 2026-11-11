"""Generated properties for the containment that bounds a payload copy.

The examples in ``test_release_payload_regular_files.py`` state one shape at a
time. These cases state the *relation* the guard must hold across every shape:
a source the guard accepts must resolve inside the staged root, and a source
that resolves outside it must be refused. Depth, how many components are
climbed and where, the position of a symlinked directory, and a root whose own
spelling carries a ``..`` are generated rather than enumerated.

The oracle is independent of the implementation on purpose. The guard decides
by walking path components and counting depth; the oracle asks the operating
system's resolver where the path ends up and compares that with the resolved
root. Two different mechanisms agreeing is evidence; the guard comparing
against itself would not be.

Run via ``make test-release-publication``.
"""

import itertools
import os
import pathlib

import pytest
import release_payload_tree
from hypothesis import HealthCheck, given, settings
from hypothesis import strategies as st
from release_payload_errors import StagedFileMissingError
from release_payload_install_support import write_asset

#: Generated depths stay small so every case builds a real tree quickly and a
#: failure names a shape a reader can follow.
MAX_DEPTH = 4

_COUNTER = itertools.count()


@pytest.fixture
def counter() -> itertools.count:
    """Yield one fresh case number per call, across a test function's runs.

    Returns
    -------
    itertools.count
        An iterator of distinct integers for this test function.
    """
    return _COUNTER


_SETTINGS = settings(
    max_examples=60,
    deadline=None,
    derandomize=True,
    suppress_health_check=[HealthCheck.function_scoped_fixture],
)


def _case_directory(tmp_path: pathlib.Path, counter: itertools.count) -> pathlib.Path:
    """Give each generated example a directory of its own.

    Hypothesis replays a failing example into the same ``tmp_path``, so a tree
    left by the previous attempt would make the replay fail on ``mkdir`` and
    the case would report a flake instead of the defect it found. The counter
    is drawn from the function's own sequence rather than from the example, so
    a replay still gets a directory nobody has used.

    Returns
    -------
    pathlib.Path
        A fresh directory below ``tmp_path``.
    """
    base = tmp_path / f"case{next(counter)}"
    base.mkdir()
    return base


def _build_chain(root: pathlib.Path, depth: int) -> tuple[pathlib.Path, ...]:
    """Create a nested directory chain below ``root``.

    Returns
    -------
    tuple[pathlib.Path, ...]
        Every directory in the chain, outermost first, excluding ``root``
        itself. The tuple is empty when ``depth`` is zero.
    """
    chain: list[pathlib.Path] = []
    current = root
    for index in range(depth):
        current /= f"d{index}"
        current.mkdir()
        chain.append(current)
    return tuple(chain)


def _resolves_inside(path: pathlib.Path, root: pathlib.Path) -> bool:
    """Report whether ``path`` resolves below ``root``, asking the OS.

    Returns
    -------
    bool
        ``True`` when the resolved path is below the resolved root. The
        resolver is deliberately not the component walk the guard uses, so
        agreement between them is evidence rather than restatement.
    """
    resolved_root = pathlib.Path(os.path.realpath(root))
    return pathlib.Path(os.path.realpath(path)).is_relative_to(resolved_root)


@_SETTINGS
@given(data=st.data())
def test_an_accepted_source_always_resolves_inside_the_root(
    data: st.DataObject, tmp_path: pathlib.Path, counter: itertools.count
) -> None:
    """Accept only sources whose bytes really are the downloaded tree's.

    The generated spelling climbs ``climb`` components out of the chain and
    descends the same components again, so ``..`` appears in the path and
    cancels before the leaf. That is a file producers staged, whatever the
    depth, and refusing it on the spelling alone would fail a good download.
    """
    depth = data.draw(st.integers(min_value=0, max_value=MAX_DEPTH))
    climb = data.draw(st.integers(min_value=0, max_value=depth))
    base = _case_directory(tmp_path, counter)
    root = base / "dist"
    root.mkdir()
    chain = _build_chain(root, depth)
    deepest = chain[-1] if chain else root
    asset = write_asset(deepest, "a.deb")

    descent = chain[depth - climb :]
    candidate = deepest.joinpath(
        *(("..",) * climb), *(part.name for part in descent), asset.name
    )
    assert _resolves_inside(candidate, root), "the generated shape must stay in"

    assert release_payload_tree.require_regular(candidate, staged_root=root), (
        "a source that resolves inside the root must be accepted"
    )
    assert candidate.resolve() == asset.resolve(), (
        "the accepted source must name the file producers staged"
    )


@_SETTINGS
@given(data=st.data())
def test_a_source_that_resolves_outside_the_root_is_always_refused(
    data: st.DataObject, tmp_path: pathlib.Path, counter: itertools.count
) -> None:
    """Refuse every spelling whose bytes are outside the downloaded tree.

    The shape cancels part of the chain *inside* the root, descends again, and
    then climbs past the root to a real file one directory above it. That is
    the case a purely lexical containment misses: ``is_relative_to`` compares
    components as written and the ``..`` components cancel before the final
    climb, so the comparison answers ``True`` while the bytes are outside.
    """
    depth = data.draw(st.integers(min_value=1, max_value=MAX_DEPTH))
    cancel = data.draw(st.integers(min_value=1, max_value=depth))
    overrun = data.draw(st.integers(min_value=1, max_value=3))
    base = _case_directory(tmp_path, counter)
    root = base / "dist"
    root.mkdir()
    chain = _build_chain(root, depth)
    deepest = chain[-1]
    asset = write_asset(base / "outside", "a.deb")

    descent = chain[depth - cancel :]
    candidate = deepest.joinpath(
        *(("..",) * cancel),
        *(part.name for part in descent),
        *(("..",) * (depth + overrun)),
        "outside",
        asset.name,
    )
    assert not _resolves_inside(candidate, root), "the generated shape must escape"

    with pytest.raises(StagedFileMissingError, match="inside the staged tree"):
        release_payload_tree.require_regular(candidate, staged_root=root)


@_SETTINGS
@given(data=st.data())
def test_a_dotdot_in_the_roots_own_spelling_never_causes_a_refusal(
    data: st.DataObject, tmp_path: pathlib.Path, counter: itertools.count
) -> None:
    """Accept a staged file however the caller spelled the root.

    ``os.walk`` does not resolve and ``walk_files`` builds each result from the
    spelling it was handed, so a root of ``dist/d0/..`` puts that ``..`` into
    every source the walk yields. The file is inside the root, so refusing it
    would fail a good download over the caller's phrasing.
    """
    depth = data.draw(st.integers(min_value=1, max_value=MAX_DEPTH))
    base = _case_directory(tmp_path, counter)
    root = base / "dist"
    root.mkdir()
    chain = _build_chain(root, depth)
    deepest = chain[-1]
    asset = write_asset(deepest, "a.deb")

    # `relative_to` compares components as written, so the leaf's name is
    # spelled directly: the point is the root's spelling, not a re-derivation.
    dotted_root = deepest / ".."
    candidate = dotted_root / deepest.name / asset.name
    assert _resolves_inside(candidate, dotted_root), "the source must stay in"

    assert release_payload_tree.require_regular(candidate, staged_root=dotted_root), (
        "a root's own ``..`` must not refuse an in-root source"
    )
    assert candidate.resolve() == asset.resolve(), (
        "the accepted source must name the file producers staged"
    )


@_SETTINGS
@given(data=st.data())
def test_a_symlinked_directory_anywhere_below_the_root_is_refused(
    data: st.DataObject, tmp_path: pathlib.Path, counter: itertools.count
) -> None:
    """Refuse a source reached through a link at any depth.

    The generated position decides which directory in the chain is the link,
    so the case covers the leaf's own parent and every ancestor above it. Each
    is a link to a directory outside the tree holding a real file of the
    requested name: the bytes exist, and only judging the link as itself
    refuses them.
    """
    depth = data.draw(st.integers(min_value=1, max_value=MAX_DEPTH))
    link_at = data.draw(st.integers(min_value=0, max_value=depth - 1))
    base = _case_directory(tmp_path, counter)
    root = base / "dist"
    root.mkdir()
    elsewhere = base / "elsewhere"
    _ = write_asset(elsewhere, "a.deb")

    current = root
    for index in range(depth):
        current /= f"d{index}"
        if index == link_at:
            _ = current.symlink_to(elsewhere, target_is_directory=True)
        else:
            current.mkdir()

    candidate = current / "outside" / ".." / "a.deb"
    with pytest.raises(StagedFileMissingError, match=r"directory|regular file"):
        release_payload_tree.require_regular(candidate, staged_root=root)
