"""The trybuild rule end to end, over a tree built for it, and its boundary.

This repository declares no trybuild target, so the contract over its own tree
asserts an empty set and its coverage loop never runs. A discovery replaced by
a constant empty answer would pass it. These cases drive the whole path, from
a `tests/` directory through discovery to the allowance check, with targets
the tree does not have; and they drive the file boundary with inputs that
must fail loudly rather than read as an empty discovery.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import pytest
from cargo_test_targets import (
    RepositoryFileError,
    declared_test_targets,
    parse_toml,
    read_repository_file,
    target_sources,
    target_texts,
)
from trybuild_override_test import trybuild_targets_in, uncovered_targets

if typ.TYPE_CHECKING:  # pragma: no cover - imported for annotations only
    import pathlib

#: A target that constructs the harness.
_CONSTRUCTS = "#[test]\nfn ui() {\n    let t = trybuild::TestCases::new();\n}\n"

#: A target that only mentions it, in a comment and in a string literal.
_MENTIONS = (
    "// trybuild::TestCases::new() was removed from here.\n"
    '#[test]\nfn plain() {\n    let s = "trybuild::TestCases::new()";\n}\n'
)


@pytest.fixture(name="tree")
def tree_fixture(tmp_path: pathlib.Path) -> pathlib.Path:
    """Return a `tests/` directory with one target of each kind.

    `ui.rs` constructs a harness, `mention.rs` only names one, and `suite/`
    is a directory target whose construction sits in a module, not `main.rs`.

    Returns
    -------
    pathlib.Path
        The `tests/` directory.
    """
    tests = tmp_path / "tests"
    (tests / "suite").mkdir(parents=True)
    (tests / "ui.rs").write_text(_CONSTRUCTS, encoding="utf-8")
    (tests / "mention.rs").write_text(_MENTIONS, encoding="utf-8")
    (tests / "suite" / "main.rs").write_text("mod cases;\n", encoding="utf-8")
    (tests / "suite" / "cases.rs").write_text(_CONSTRUCTS, encoding="utf-8")
    return tests


def test_discovery_finds_constructions_and_not_mentions(tree: pathlib.Path) -> None:
    """Both constructing targets are found, the mention is not."""
    found = trybuild_targets_in(target_texts(target_sources(tree)))
    assert found == ["suite", "ui"], (
        f"discovery must find exactly the two builders: {found}"
    )


@pytest.mark.parametrize(
    ("filters", "uncovered"),
    [
        pytest.param([], ["suite", "ui"], id="no-override"),
        pytest.param(["binary(=ui)"], ["suite"], id="one-exact-override"),
        pytest.param(["binary(=ui) | binary(=suite)"], [], id="both-covered-exactly"),
        pytest.param(
            ["binary(ui) | binary(suite)"], ["suite", "ui"], id="substring-only"
        ),
    ],
)
def test_the_allowance_check_runs_over_what_discovery_found(
    tree: pathlib.Path, filters: list[str], uncovered: list[str]
) -> None:
    """The coverage loop the repository's empty set never reaches."""
    targets = trybuild_targets_in(target_texts(target_sources(tree)))
    got = uncovered_targets(targets, filters)
    assert got == uncovered, (
        f"filters {filters} must leave {uncovered} uncovered: {got}"
    )


def test_a_missing_tests_directory_is_an_error_not_an_empty_tree(
    tmp_path: pathlib.Path,
) -> None:
    """A directory that is not there must not read as one with no targets."""
    with pytest.raises(RepositoryFileError, match="not a directory"):
        target_sources(tmp_path / "tests")


@pytest.mark.parametrize(
    ("content", "expected"),
    [
        pytest.param(None, "cannot read", id="missing"),
        pytest.param(b"\xff\xfe", "cannot read", id="not-utf-8"),
    ],
)
def test_an_unreadable_file_is_a_named_error(
    tmp_path: pathlib.Path, content: bytes | None, expected: str
) -> None:
    """Each failure names the file it could not read."""
    path = tmp_path / "source.rs"
    if content is not None:
        path.write_bytes(content)
    with pytest.raises(RepositoryFileError, match=expected) as raised:
        read_repository_file(path)
    assert "source.rs" in str(raised.value), (
        f"the error must name the file: {raised.value}"
    )


def test_malformed_toml_is_a_named_error(tmp_path: pathlib.Path) -> None:
    """A manifest that does not parse names itself rather than reading as empty."""
    path = tmp_path / "Cargo.toml"
    with pytest.raises(RepositoryFileError, match=r"Cargo\.toml is not valid TOML"):
        declared_test_targets({path: "[package\n"})
    parsed = parse_toml('[[test]]\nname = "x"\n', path)
    assert parsed == {"test": [{"name": "x"}]}, f"valid TOML must parse: {parsed}"
