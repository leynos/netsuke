"""Which proof-only code the mutation caller may exclude, and how.

``exclude-globs`` is *path*-scoped. That is the right granularity for a module
gated behind ``#[cfg(kani)] mod``, because every item in it is absent from the
ordinary test binaries. It is the wrong granularity for a file that mixes
proof-only implementations with the ``cfg(not(kani))`` production counterparts,
because excluding the path would take both halves out of the lane.

Issue #1038 is exactly that second case: ``src/ir/graph/mod.rs`` holds four
``cfg(kani)`` implementations beside the production ones, and the ordinary lane
reported six false survivors from them. Those items carry per-item
``#[mutants::skip]`` attributes instead, and
``tests/mutation_selection_contract_tests.rs`` holds the source side.

These assertions hold the workflow side: the excludes stay a whole-file set, and
the mixed file is never added to it. They live here rather than in
``mutation_testing_test`` so that module stays inside the repository's 400-line
limit. The two accessors both contracts need — the parsed ``with:`` block and
the string narrowing — live in ``mutation_caller_support``, so there is one
definition of where ``exclude-globs`` is read from and neither module imports a
private helper from the other.

Run via ``make test-workflow-contracts``.
"""

from mutation_caller_support import mutation_with_block, require_str
from mutation_testing_test import KANI_EXCLUDES, MIXED_KANI_FILE


def _excluded_globs() -> tuple[str, ...]:
    """Return the caller's declared ``exclude-globs`` entries, in order.

    Reads the parsed ``with:`` block rather than the raw document, so the
    explanatory prose around the key — which names the mixed file on purpose —
    cannot be mistaken for configuration.

    Returns
    -------
    tuple[str, ...]
        The comma-separated globs with surrounding whitespace removed.
    """
    exclude_globs = require_str(
        mutation_with_block().get("exclude-globs"),
        "jobs.mutation.with.exclude-globs",
    )
    return tuple(glob.strip() for glob in exclude_globs.split(",") if glob.strip())


def test_mixed_kani_file_is_not_excluded_wholesale() -> None:
    """The mixed production/Kani file is never added to ``exclude-globs``.

    Naming it here would remove the ``cfg(not(kani))`` counterparts from the
    lane along with the proof-only items, turning a narrow repair into a
    whole-file exclusion. The proof-only items are excluded one at a time by
    ``#[mutants::skip]`` instead.
    """
    globs = _excluded_globs()
    assert MIXED_KANI_FILE not in globs, (
        f"jobs.mutation.with.exclude-globs must not name {MIXED_KANI_FILE}: it "
        "holds both proof-only and production implementations, so excluding "
        "the file would remove the production counterparts from the lane too. "
        f"got {sorted(globs)}"
    )


def test_excluded_globs_are_whole_kani_only_files() -> None:
    """Each excluded path is one of the known whole-file Kani modules.

    Asserting the exact set keeps a new exclusion from being *added* without
    review — including the mixed file above, but equally any ordinary source
    that a future edit would rather filter out than test.
    """
    globs = _excluded_globs()
    assert set(globs) == set(KANI_EXCLUDES), (
        "jobs.mutation.with.exclude-globs must be exactly the whole-file "
        f"Kani-only modules; expected {sorted(KANI_EXCLUDES)}, got {sorted(globs)}"
    )


def test_the_exclusion_set_is_deduplicated() -> None:
    """No glob is listed twice, so the set assertion is not hiding a repeat.

    A duplicate would satisfy the set comparison above while making the caller's
    value say something the reviewed set does not.
    """
    globs = _excluded_globs()
    assert len(globs) == len(set(globs)), (
        "jobs.mutation.with.exclude-globs must not repeat an entry; "
        f"got {sorted(globs)}"
    )
