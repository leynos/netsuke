"""Specify the release-inventory exactness rule.

LEM-2 holds that the files a release presents for publication are exactly the
files it declared: nothing missing, nothing extra, no empty file, no duplicate
name, and no name that could reach outside the directory it is staged in. Each
class is a named row below, and the accept rows make the predicate two-sided,
so a validator that refuses everything and one that refuses nothing each fail.
Generated path cases also exercise both path flavours and the order-preserving
partition that separates escaping names from other malformed names.

Stating the rule as exact set equality matters because the failure it catches
is otherwise invisible. On 2026-10-12 every published release was found to
carry the six ``netsuke-build-{version}-{target}.tar.gz.sha256`` sidecars and
none of the six archives they describe. A rule that asked only whether each
present file looked acceptable accepted that release; a rule that asks whether
the declared set is present does not, and the ``a-binstall-archive`` row below
is that release's verdict written down.
"""

import dataclasses

import pytest
import release_asset_names
import release_inventory
from hypothesis import example, given, settings
from hypothesis import strategies as st

ARCHIVE = "netsuke-build-0.1.0-beta4-x86_64-unknown-linux-gnu.tar.gz"
SIDECAR = f"{ARCHIVE}.sha256"
BIN = "netsuke"
DECLARED = (ARCHIVE, SIDECAR, BIN)


def _published(name: str, size: int = 1) -> release_inventory.PublishedAsset:
    """Return a published asset carrying the given name and byte size."""
    return release_inventory.PublishedAsset(name=name, size=size)


def test_the_declared_inventory_is_accepted() -> None:
    """Accept a published set that holds exactly the declared names."""
    present = [_published(name) for name in DECLARED]
    release_inventory.validate_inventory(present, expected=DECLARED)


def test_the_order_of_the_published_set_does_not_change_the_verdict() -> None:
    """Compare as a set, so the order assets were listed in cannot decide it."""
    present = [_published(name) for name in reversed(DECLARED)]
    release_inventory.validate_inventory(present, expected=DECLARED)


@pytest.mark.parametrize(
    "absent",
    [
        pytest.param(ARCHIVE, id="a-binstall-archive"),
        pytest.param(SIDECAR, id="its-checksum-sidecar"),
        pytest.param(BIN, id="the-linux-binary"),
    ],
)
def test_a_declared_asset_that_was_not_published_is_rejected(absent: str) -> None:
    """Reject a declared asset the release never published.

    The ``a-binstall-archive`` row is the live defect recorded in the module
    docstring: releases carry the sidecar and not the archive, so this is the
    verdict a check of the published listing must reach.
    """
    present = [_published(name) for name in DECLARED if name != absent]
    with pytest.raises(release_inventory.MissingAssetError) as caught:
        release_inventory.validate_inventory(present, expected=DECLARED)
    assert absent in str(caught.value), "the diagnostic must name the absent asset"


def test_an_undeclared_asset_is_rejected() -> None:
    """Reject an asset that was published but never declared."""
    present = [_published(name) for name in (*DECLARED, "netsuke.1")]
    with pytest.raises(release_inventory.UnexpectedAssetError) as caught:
        release_inventory.validate_inventory(present, expected=DECLARED)
    assert "netsuke.1" in str(caught.value), (
        "the diagnostic must name the undeclared asset"
    )


def test_an_empty_asset_is_rejected() -> None:
    """Reject a zero-byte asset, which uploads but cannot be used."""
    present = [_published(name, size=0) for name in DECLARED]
    with pytest.raises(release_inventory.EmptyAssetError) as caught:
        release_inventory.validate_inventory(present, expected=DECLARED)
    assert ARCHIVE in str(caught.value), "the diagnostic must name the empty asset"


def test_a_duplicated_name_is_rejected() -> None:
    """Reject two entries claiming the same asset name."""
    present = [_published(name) for name in (*DECLARED, BIN)]
    with pytest.raises(release_inventory.DuplicateAssetError) as caught:
        release_inventory.validate_inventory(present, expected=DECLARED)
    assert BIN in str(caught.value), "the diagnostic must name the duplicate"


@pytest.mark.parametrize(
    "name",
    [
        pytest.param("..", id="a-bare-parent"),
        pytest.param("../netsuke", id="a-leading-parent"),
        pytest.param("a/../../b", id="a-parent-below-a-component"),
        pytest.param("/etc/passwd", id="an-absolute-posix-path"),
        pytest.param(r"C:\Windows\netsuke.exe", id="a-windows-drive-path"),
    ],
)
def test_a_name_that_could_escape_the_staging_directory_is_rejected(name: str) -> None:
    """Reject a name that could resolve outside the directory it is staged in."""
    present = [_published(name) for name in (*DECLARED, name)]
    with pytest.raises(release_inventory.EscapingAssetNameError) as caught:
        release_inventory.validate_inventory(present, expected=DECLARED)
    # `repr` rather than the bare name: the diagnostic renders it with `repr`
    # too, so that a name differing only by an invisible character stays
    # legible. The Windows row is why this matters here — its backslashes are
    # escaped in the message, and a bare-substring check would miss them.
    assert repr(name) in str(caught.value), "the diagnostic must name the entry"


@pytest.mark.parametrize(
    "name",
    [
        pytest.param("", id="an-empty-name"),
        pytest.param(".", id="the-current-directory"),
        pytest.param("sub/netsuke", id="a-nested-path"),
        pytest.param("sub\\netsuke", id="a-backslash-nested-path"),
        pytest.param("a//b", id="an-empty-component"),
    ],
)
def test_a_name_that_is_not_a_bare_file_name_is_rejected(name: str) -> None:
    """Require a flat file name, however the entry was staged.

    This is a different diagnosis from escaping: ``sub/netsuke`` stays inside
    the directory, but it is not the name the uploader would publish, so a
    listing that carried it is describing the wrong thing.
    """
    present = [_published(name) for name in (*DECLARED, name)]
    with pytest.raises(release_inventory.MalformedAssetNameError) as caught:
        release_inventory.validate_inventory(present, expected=DECLARED)
    assert repr(name) in str(caught.value), "the diagnostic must name the entry"


def test_a_published_asset_is_immutable() -> None:
    """Reject mutation, so a validator cannot rewrite an entry it is judging."""
    asset = _published(BIN)
    rewritten = "name"
    with pytest.raises(dataclasses.FrozenInstanceError):
        setattr(asset, rewritten, "other")


_SAFE_NAME = st.from_regex(r"[A-Za-z0-9][A-Za-z0-9._-]{0,20}", fullmatch=True)
_PATH_COMPONENT = st.one_of(
    st.sampled_from((".", "..")),
    st.from_regex(r"[A-Za-z0-9_-]{1,8}", fullmatch=True),
)


@st.composite
def _asset_name_cases(draw: st.DrawFn) -> tuple[str, bool, bool]:
    """Generate a name with expected escape and bare-file outcomes.

    Cases vary separators, absolute prefixes, current and parent components,
    drive-like names, and trailing separators. Keep this strategy local to the
    inventory suite because it models this policy's two-platform contract.

    Returns
    -------
    tuple[str, bool, bool]
        The generated name and its expected escape and bare-name results.
    """
    flavour = draw(st.sampled_from(("posix", "windows")))
    separator = draw(st.sampled_from(("/", "\\")))
    components = draw(st.lists(_PATH_COMPONENT, min_size=1, max_size=5))
    is_absolute = draw(st.booleans())
    has_trailing_separator = draw(st.booleans())
    name = separator.join(components)
    if is_absolute:
        prefix = "/" if flavour == "posix" else f"C:{separator}"
        name = prefix + name
    if has_trailing_separator:
        name += separator

    depth = 0
    escapes = is_absolute
    for component in components:
        if component == "..":
            depth -= 1
            escapes = escapes or depth < 0
        elif component not in {"", "."}:
            depth += 1
    is_bare = (
        not is_absolute
        and not has_trailing_separator
        and len(components) == 1
        and components[0] not in {"", ".", ".."}
        and ":" not in components[0]
    )
    return name, escapes, is_bare


@settings(max_examples=200, deadline=None, derandomize=True)
@example(case=("../netsuke", True, False))
@example(case=("C:\\Windows\\netsuke.exe", True, False))
@example(case=("C:/Windows/netsuke.exe", True, False))
@example(case=("C:", False, False))
@example(case=("", False, False))
@example(case=("./netsuke", False, False))
@example(case=("netsuke/", False, False))
@example(case=("netsuke\\", False, False))
@example(case=(r"nested\file", False, False))
@given(case=_asset_name_cases())
def test_asset_name_predicates_cover_generated_posix_and_windows_paths(
    case: tuple[str, bool, bool],
) -> None:
    """Match generated path facts, including normalization edge cases."""
    name, expected_escape, expected_bare = case

    assert release_asset_names.escapes_its_directory(name) is expected_escape, (
        f"the escape predicate must match generated path {name!r}"
    )
    assert release_asset_names.is_bare_file_name(name) is expected_bare, (
        f"the bare-name predicate must match generated path {name!r}"
    )


@settings(max_examples=200, deadline=None, derandomize=True)
@given(cases=st.lists(_asset_name_cases(), max_size=8))
def test_rejected_path_partition_preserves_generated_input_order(
    cases: list[tuple[str, bool, bool]],
) -> None:
    """Partition generated names without sorting or losing duplicates."""
    names = [name for name, _, _ in cases]
    expected_escaping = [name for name, escapes, _ in cases if escapes]
    expected_malformed = [
        name for name, escapes, bare in cases if not escapes and not bare
    ]

    assert release_asset_names.rejected_names(names) == (
        expected_escaping,
        expected_malformed,
    ), "both rejection classes must preserve source order"


@settings(max_examples=200, deadline=None, derandomize=True)
@example(names=list(DECLARED))
@example(names=[*DECLARED, BIN])
@given(
    names=st.lists(
        st.one_of(st.sampled_from(DECLARED), _SAFE_NAME), min_size=0, max_size=6
    )
)
def test_acceptance_is_exactly_set_equality(names: list[str]) -> None:
    """Accept a published set exactly when it holds each declared name once.

    The two ``example`` rows are the witnesses that make this property
    two-sided rather than vacuous: the first is the accepted case, which a
    validator that always raised would fail, and the second carries a
    duplicated name, which a validator that never raised would pass.
    """
    present = [_published(name) for name in names]
    matches = set(names) == set(DECLARED) and len(names) == len(DECLARED)
    try:
        release_inventory.validate_inventory(present, expected=DECLARED)
    except release_inventory.InventoryError:
        accepted = False
    else:
        accepted = True
    assert accepted is matches, (
        f"an inventory of {names!r} must be accepted exactly when it is the "
        f"declared set; the validator said {accepted}"
    )
