"""Every trybuild target is covered by an allowance, or there are none.

A per-test `terminate-after` and a name-based override list are a pair that
rots apart. The list is written once against the names of the day and is never
re-derived, and neither a passing run nor a green gate notices a target that
has fallen out of it, because the cost only appears on a cold cache. A trybuild
target builds a scratch crate against this workspace's dependency graph, so it
is the cost that overruns first.

This repository has no trybuild target today, and the contract pins that. An
empty set is not a reason to omit the rule: it is the state the rule must
notice leaving. A trybuild harness added tomorrow inherits the base allowance
of `[profile.default]`, which is sized for a test that compiles nothing, and
would be terminated on the first cold run rather than reported.

The discovery reads what the file constructs, not what it mentions. This
repository has a module whose documentation explains at length why a trybuild
harness was removed, and a text match would report it as a target that exists.

Run via ``make test-workflow-contracts``.
"""

import re
import tomllib
from pathlib import Path

import pytest
from rust_source_reading import code_only
from workflow_loading import REPO_ROOT

TESTS_DIR = REPO_ROOT / "tests"
NEXTEST_CONFIG = REPO_ROOT / ".config" / "nextest.toml"

#: A construction of the harness, not a mention of the crate. The crate may be
#: named in a comment, an import that is never used, or a paragraph explaining
#: why it is absent; only a constructed `TestCases` runs a scratch build.
TRYBUILD_CONSTRUCTION = re.compile(r"\bTestCases::new\(")

#: Whitespace around a path separator or before a call's parenthesis. Rust
#: permits it and this repository's formatter removes it, so the reader
#: normalizes rather than depending on the formatter having run.
_SPACED_SYNTAX = re.compile(r"\s*(::)\s*|\s+(\()")


def _constructs_trybuild(text: str) -> bool:
    """Return whether the Rust source constructs a trybuild harness."""
    # Comments and literals first: a paragraph explaining why a harness was
    # removed writes the construction it removed, and a search of raw source
    # cannot tell that sentence from the harness.
    normalized = _SPACED_SYNTAX.sub(
        lambda found: found.group(1) or found.group(2), code_only(text)
    )
    return TRYBUILD_CONSTRUCTION.search(normalized) is not None


#: A nextest binary selector naming one binary exactly. `binary(name)` matches
#: by substring and `binary(=name)` by equality; only the second is read as
#: coverage, because the first would accept a filter written for a different
#: binary whose name happens to contain this one.
_BINARY_SELECTOR = re.compile(r"\bbinary(?:_id)?\s*\(\s*=\s*([^)\s]+)\s*\)")

#: Any negation in a filterset. `not test(ui)` names the binary and excludes
#: it, so a reader that looks for the name in the text reports coverage where
#: nextest applies none. A filter carrying one is read as covering nothing
#: rather than evaluated, because evaluating a filterset is nextest's job.
_NEGATION = re.compile(r"(?:^|[^\w])(?:not\b|!)")


def _filter_covers(filter_text: str, target: str) -> bool:
    """Return whether one override filter selects `target`'s binary exactly."""
    if _NEGATION.search(filter_text):
        return False
    stem = Path(target).stem
    return any(
        stem in {selector, selector.rsplit("::", 1)[-1]}
        for selector in _BINARY_SELECTOR.findall(filter_text)
    )


def _trybuild_targets() -> list[str]:
    """Return every integration-test target that constructs a trybuild harness."""
    return sorted(
        path.relative_to(REPO_ROOT).as_posix()
        for path in TESTS_DIR.rglob("*.rs")
        if _constructs_trybuild(path.read_text(encoding="utf-8"))
    )


def _base_terminates() -> bool:
    """Return whether the default profile terminates a test on its allowance."""
    config = tomllib.loads(NEXTEST_CONFIG.read_text(encoding="utf-8"))
    match config.get("profile", {}).get("default", {}).get("slow-timeout"):
        case {"terminate-after": _}:
            return True
        case _:
            # A bare duration sets a warning period and terminates nothing, and
            # so does an absent key; both leave a trybuild target unbounded.
            return False


def _override_filters() -> list[str]:
    """Return every override filter declared in any profile."""
    config = tomllib.loads(NEXTEST_CONFIG.read_text(encoding="utf-8"))
    return [
        str(override.get("filter", ""))
        for profile in config.get("profile", {}).values()
        for override in profile.get("overrides", [])
    ]


def test_the_base_allowance_terminates() -> None:
    """The premise this contract rests on, asserted rather than assumed.

    Without `terminate-after` nothing is killed, so an uncovered trybuild
    target would merely be reported slow and the rule below would be guarding
    a hazard that does not exist. If this ever fails, the rule needs rewriting
    rather than relaxing.
    """
    assert _base_terminates(), (
        "[profile.default] must declare slow-timeout with terminate-after; "
        "this contract exists because a trybuild target inherits it"
    )


def test_every_trybuild_target_has_an_allowance_of_its_own() -> None:
    """The set is discovered from the tree, and the empty set is pinned too.

    Listing the set here would be the same defect one level up: a list written
    once against the names of the day. Discovery fails when a target is added,
    which is the moment the allowance has to be decided.

    The emptiness is asserted separately because coverage alone cannot see it.
    Every member of an empty set is covered, so an assertion that each
    discovered target has an allowance passes when discovery finds nothing,
    whether the repository has no trybuild target or the reader has stopped
    finding them.
    """
    targets = _trybuild_targets()
    assert targets == [], (
        f"this repository declared no trybuild target when this contract was "
        f"written and now declares {targets}; decide the allowance for each "
        f"and pin the new set here, because a set asserted only through its "
        f"coverage is satisfied by an empty discovery"
    )
    uncovered = [
        target
        for target in targets
        if not any(
            _filter_covers(filter_text, target) for filter_text in _override_filters()
        )
    ]
    assert not uncovered, (
        f"these trybuild targets inherit the base allowance: {uncovered}; a "
        f"trybuild target builds a scratch crate against this workspace's "
        f"dependency graph and will be terminated on a cold run. Give each an "
        f"override whose filter selects its binary exactly, sized for that "
        f"build"
    )


@pytest.mark.parametrize(
    ("filter_text", "target", "covers"),
    [
        pytest.param("binary(=ui)", "tests/ui.rs", True, id="an-exact-binary"),
        pytest.param(
            "binary_id(=netsuke::ui)", "tests/ui.rs", True, id="an-exact-binary-id"
        ),
        pytest.param(
            "binary(=ui) | test(=other)", "tests/ui.rs", True, id="one-arm-of-a-union"
        ),
        pytest.param("binary(ui)", "tests/ui.rs", False, id="a-substring-binary"),
        pytest.param("not binary(=ui)", "tests/ui.rs", False, id="a-negated-selector"),
        pytest.param(
            "!binary(=ui)", "tests/ui.rs", False, id="a-negated-selector-in-symbols"
        ),
        pytest.param(
            "test(=harness_compiles_under_a_split_build_dir)",
            "tests/ui.rs",
            False,
            id="a-test-name-that-contains-the-stem",
        ),
        pytest.param(
            "binary(=user_interface)",
            "tests/ui.rs",
            False,
            id="a-binary-whose-name-contains-the-stem",
        ),
    ],
)
def test_coverage_needs_an_exact_selector_that_is_not_negated(
    filter_text: str, target: str, *, covers: bool
) -> None:
    """Containment is not selection, and a name inside a filter is not either.

    `test(=harness_compiles_under_a_split_build_dir)` contains `ui`, inside
    `build`, so a `tests/ui.rs` harness read as covered by a filter that
    selects a different test entirely. `not binary(=ui)` names the binary and
    excludes it, which containment reads as coverage with the sign inverted.
    `binary(ui)` matches by substring in nextest, so it is not evidence that
    this binary is the one the override was written for.

    A filter carrying a negation covers nothing here rather than being
    evaluated: evaluating a filterset is nextest's work, and a reader that
    guessed would be the defect one layer down.
    """
    assert _filter_covers(filter_text, target) is covers, (
        f"{filter_text!r} must read as covers={covers} for {target}"
    )


@pytest.mark.parametrize(
    ("source", "constructs"),
    [
        pytest.param("let t = trybuild::TestCases::new();", True, id="fully-qualified"),
        pytest.param(
            "use trybuild::TestCases;\nlet t = TestCases::new();",
            True,
            id="imported-then-constructed",
        ),
        pytest.param(
            "let t = trybuild :: TestCases :: new ();", True, id="generously-spaced"
        ),
        pytest.param(
            "//! This replaced a `trybuild` harness during the migration.",
            False,
            id="a-paragraph-about-its-absence",
        ),
        pytest.param("use trybuild::TestCases;", False, id="imported-but-unused"),
        pytest.param("// trybuild::TestCases::new", False, id="named-without-a-call"),
        pytest.param(
            "// let t = trybuild::TestCases::new();",
            False,
            id="a-commented-out-construction",
        ),
        pytest.param(
            "/* let t = trybuild::TestCases::new(); */",
            False,
            id="a-block-commented-construction",
        ),
        # Rust nests block comments, so the inner `*/` closes the inner one and
        # the code after the outer one is code. A reader that stopped at the
        # first `*/` would blank a construction; one that never stopped would
        # keep the construction it should have blanked.
        pytest.param(
            "/* a /* nested */ x */ let t = trybuild::TestCases::new();",
            True,
            id="code-after-a-nested-block-comment",
        ),
        pytest.param(
            "/* a /* nested */ trybuild::TestCases::new() */",
            False,
            id="a-construction-inside-a-nested-block-comment",
        ),
        pytest.param(
            "/* unterminated trybuild::TestCases::new()",
            False,
            id="an-unterminated-block-comment",
        ),
        pytest.param(
            'let note = "trybuild::TestCases::new()";',
            False,
            id="a-construction-inside-a-string",
        ),
        pytest.param(
            'let note = r#"a " and trybuild::TestCases::new()"#;',
            False,
            id="a-construction-inside-a-raw-string",
        ),
        pytest.param(
            "let quote = '\"'; let t = trybuild::TestCases::new();",
            True,
            id="a-quote-character-does-not-open-a-string",
        ),
        pytest.param(
            "fn f<'a>(x: &'a str) { let t = trybuild::TestCases::new(); }",
            True,
            id="a-lifetime-is-not-a-character-literal",
        ),
        pytest.param(
            "let cases = OtherTestCases::new();", False, id="a-different-type"
        ),
    ],
)
def test_discovery_reads_construction_rather_than_mention(
    source: str, *, constructs: bool
) -> None:
    """The discrimination the rule rests on, driven directly.

    This repository declares no trybuild target, so parameterized over its own
    files the reader agrees with the tree whether it reads construction,
    mention, or nothing at all. These cases are what separate the three.
    """
    assert _constructs_trybuild(source) is constructs, (
        f"{source!r} must read as constructs={constructs}; a mention of the "
        f"crate is not a target and a construction of it is"
    )
