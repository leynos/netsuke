"""Test the name policy that judges a staged tree against a declaration.

Every refusal selection can make over a name is reachable from a dictionary
here, which is the point of the split: the policy is exercised without a
filesystem, so a case that would need a carefully broken workspace instead
costs one literal.

Run via ``make test-release-publication``.
"""

import pytest
from release_payload_names import (
    ARCHIVE_SUFFIX,
    PACKAGE_SUFFIXES,
    describe_offenders,
    is_release_shaped,
)

#: A sidecar name, derived the way the policy derives it so the two cannot
#: silently disagree about which suffix carries a checksum.
SIDECAR_NAME = f"netsuke-1.0.0-x86_64-unknown-linux-gnu{ARCHIVE_SUFFIX}.sha256"


@pytest.mark.parametrize(
    ("name", "expected"),
    [
        ("netsuke-1.0.0-x86_64-unknown-linux-gnu.tar.gz", True),
        ("netsuke-1.0.0-x86_64-unknown-linux-gnu.tar.gz.sha256", True),
        ("netsuke-1.0.0-1.x86_64.rpm", True),
        ("netsuke_1.0.0_amd64.deb", True),
        ("netsuke", False),
        ("LICENSE", False),
        ("completions/bash/netsuke", False),
        ("netsuke.1", False),
    ],
)
def test_is_release_shaped_classifies_publishable_names(
    name: str, *, expected: bool
) -> None:
    """Separate names a release could publish from the material inside it."""
    assert is_release_shaped(name) is expected, name


def test_package_suffixes_are_derived_from_the_platform_table() -> None:
    """Take the package suffixes from configuration rather than restating them."""
    assert PACKAGE_SUFFIXES, "a release with no package suffix publishes nothing"
    ordered = list(PACKAGE_SUFFIXES)
    assert ordered == sorted(ordered), (
        "the suffix tuple is reported in refusals, so its order must be stable"
    )
    assert ".tar.gz" not in PACKAGE_SUFFIXES, (
        "the archive suffix is handled separately; folding it in would make a "
        "sidecar-shaped check depend on a platform package format"
    )


def test_a_complete_tree_reports_nothing() -> None:
    """Accept a tree that stages exactly one file per declared name."""
    staged = {"a.deb": ["x"], "b.rpm": ["y"]}
    assert describe_offenders(("a.deb", "b.rpm"), staged) == [], "nothing to report"


def test_build_material_inside_the_packages_is_excluded_silently() -> None:
    """Ignore staged files no release publishes, such as binaries and licences."""
    staged = {"a.deb": ["x"], "netsuke": ["bin"], "LICENSE": ["licence"]}
    assert describe_offenders(("a.deb",), staged) == [], (
        "retained build material must not be reported as a defect"
    )


def test_a_missing_declared_asset_is_reported() -> None:
    """Refuse a tree that omits a declared name."""
    assert describe_offenders(("a.deb", "b.rpm"), {"a.deb": ["x"]}) == [
        "'b.rpm' was not staged"
    ], "the missing name must be the whole report"


def test_a_duplicated_declared_asset_is_reported() -> None:
    """Refuse a tree that stages one declared name more than once."""
    assert describe_offenders(("a.deb",), {"a.deb": ["x", "y"]}) == [
        "'a.deb' was staged 2 times"
    ], "the count must name the duplicated asset"


def test_a_release_shaped_stray_is_refused_rather_than_guessed_about() -> None:
    """Refuse an undeclared name that would publish if it were uploaded.

    A stray archive is either this release's asset under another name or
    retained material from an earlier one. Selection has no way to tell, so it
    refuses instead of choosing.
    """
    staged = {"a.deb": ["x"], "stale-1.0.0-x86_64.tar.gz": ["y"]}
    assert describe_offenders(("a.deb",), staged) == [
        "'stale-1.0.0-x86_64.tar.gz' is release-shaped but not declared"
    ], "a stray must be refused, not silently dropped"


def test_a_name_that_escapes_its_directory_is_reported_before_its_file() -> None:
    """Report an unsafe name as an escape, not as a missing asset.

    Reporting it as missing would describe the symptom and hide the reason an
    operator needs: the path left the downloaded tree.
    """
    offenders = describe_offenders(("a.deb",), {"../a.deb": ["x"]})
    assert offenders[0].startswith("staged name escapes its directory:"), (
        f"the escape must lead the report, not follow it: {offenders!r}"
    )
    assert "'a.deb' was not staged" in offenders, (
        "the missing declared asset is still a symptom worth reporting, but it "
        "must not be the headline"
    )


def test_a_name_that_is_not_a_bare_file_name_is_reported() -> None:
    """Refuse a staged name carrying a path separator."""
    offenders = describe_offenders((), {"nested/a.deb": ["x"]})
    assert any("not a bare file name" in offender for offender in offenders), (
        f"a nested name must be reported as malformed: {offenders!r}"
    )


def test_offenders_are_sorted_so_a_rerun_reports_the_same_list() -> None:
    """Order the report so two runs over one tree compare line for line."""
    staged = {
        "z.rpm": ["z"],
        "m.rpm": ["m"],
        "a.rpm": ["a"],
    }
    assert describe_offenders(("b.rpm",), staged) == [
        "'a.rpm' is release-shaped but not declared",
        "'b.rpm' was not staged",
        "'m.rpm' is release-shaped but not declared",
        "'z.rpm' is release-shaped but not declared",
    ], "the report must be sorted by the offending name"


def test_the_sidecar_of_a_real_archive_is_release_shaped() -> None:
    """Recognise an archive checksum, which is published beside the archive."""
    assert is_release_shaped(SIDECAR_NAME), SIDECAR_NAME
