"""Hold the runtime filter oracle's entry point to what it can verify.

`verify_nextest_anchored_filters.py` replays every Nextest filter and then
checks each anchored filter that names a parameterized test against that test's
case instances. ADR-0059 removed the last grouped parameterized test, which left
the second check with no subject. The entry point then failed the coverage lane
on a healthy configuration, because it treated an empty intersection of the two
corpora as an empty read.

These tests drive `main` with the discovery and Nextest helpers replaced, so
they pin the decision itself: an empty corpus is the script's own fault and
fails, while an empty intersection of two non-empty corpora still replays every
filter and succeeds.

Run via ``make test-workflow-contracts``.
"""

import sys
import typing as typ
from contextlib import ExitStack
from pathlib import Path
from unittest import mock

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
# The script lives beside the coverage lane it serves, outside any package the
# test tree imports by name, so its directory is inserted rather than the
# module copied; the test exercises the code that actually runs.
sys.path.insert(0, str(REPO_ROOT / ".github" / "scripts"))

import verify_nextest_anchored_filters as oracle  # ruff: ignore[module-import-not-at-top-of-file] - needs the sys.path insertion above.

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: An anchored filter naming a test that is not parameterized.
UNPARAMETERIZED_SELECTOR = "test(/^coverage_map_status_is_reported($|::)/)"


@pytest.fixture
def replays() -> cabc.Iterator[list[object]]:
    """Replace the Nextest-facing helpers and record each replay request.

    Yields
    ------
    list[object]
        One entry per call to the whole-configuration replay, so a test can
        assert that the replay ran rather than being skipped.
    """
    calls: list[object] = []
    with ExitStack() as patches:
        patches.enter_context(
            mock.patch.object(oracle, "instrumented_environment", return_value={})
        )
        patches.enter_context(
            mock.patch.object(
                oracle,
                "_check_every_filter_selects_something",
                side_effect=calls.append,
            )
        )
        patches.enter_context(
            mock.patch.object(
                oracle,
                "anchored_selectors",
                return_value=[
                    (UNPARAMETERIZED_SELECTOR, "coverage_map_status_is_reported")
                ],
            )
        )
        patches.enter_context(
            mock.patch.object(
                oracle,
                "configured_names",
                return_value=({"coverage_map_status_is_reported"}, set()),
            )
        )
        yield calls


def test_no_parameterized_filter_still_replays_every_filter(
    replays: list[object],
) -> None:
    """Pass when no filter names a parameterized test, after the replay runs."""
    with mock.patch.object(
        oracle, "parameterized_tests", return_value={"some_case": 2}
    ):
        assert oracle.main([]) == 0, (
            "an empty intersection of two non-empty corpora is a legitimate "
            "configuration, not a fault"
        )
    assert len(replays) == 1, (
        "the whole-configuration replay must still run when the per-instance "
        "check has no subject"
    )


def test_an_empty_parameterized_corpus_is_refused(
    replays: list[object],
) -> None:
    """Fail when discovery read no parameterized test at all."""
    with (
        mock.patch.object(oracle, "parameterized_tests", return_value={}),
        pytest.raises(SystemExit) as raised,
    ):
        oracle.main([])
    assert raised.value.code == 1, "an empty read must fail the lane"
    assert not replays, "a failed read must stop before replaying any filter"


def test_an_empty_anchored_corpus_is_refused(
    replays: list[object],
) -> None:
    """Fail when discovery finds parameterized tests but no anchored filters."""
    with (
        mock.patch.object(oracle, "anchored_selectors", return_value=[]),
        mock.patch.object(oracle, "configured_names", return_value=(set(), set())),
        mock.patch.object(oracle, "parameterized_tests", return_value={"some_case": 2}),
        pytest.raises(SystemExit) as raised,
    ):
        oracle.main([])
    assert raised.value.code == 1, "an empty filter read must fail the lane"
    assert not replays, "a failed read must stop before replaying any filter"
