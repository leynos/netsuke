"""Generated tests for the proof-scope matching and decision rules.

``kani_proof_scope_decision_test.py`` drives the script over real repositories
and named cases. This module generates scope entries and changed paths and
checks ``is_in_scope`` and ``decide`` against an oracle that compares path
segments by hand, so a prefix test that matches `src/irony.rs` against
`src/ir/` fails here whatever the examples happen to name.

Run via ``make test-workflow-contracts``.
"""

import sys

from hypothesis import given, settings
from hypothesis import strategies as st
from workflow_loading import REPO_ROOT

sys.path.insert(0, str(REPO_ROOT / "scripts"))

# The module under test lives in scripts/, outside any package, so the import
# cannot precede the sys.path insertion above.
import kani_proof_scope as scope_mod

PROPERTY = settings(max_examples=200, deadline=None)
#: A small alphabet makes a generated path collide with an entry's prefix, or
#: share a stem with its directory name, often enough to exercise the boundary.
SEGMENT = st.sampled_from(["src", "ir", "irony", "a", "b", "Cargo.toml"])
PATHS = st.lists(SEGMENT, min_size=1, max_size=4).map("/".join)
ENTRIES = st.one_of(
    PATHS,
    st.lists(SEGMENT, min_size=1, max_size=3).map(lambda s: "/".join(s) + "/"),
)
SCOPES = st.lists(ENTRIES, min_size=1, max_size=5).map(tuple)
EVENTS = st.sampled_from(["push", "schedule", "workflow_dispatch", "anything"])


def matches(path: str, entry: str) -> bool:
    """Match by whole segments: a directory entry is a proper segment prefix."""
    if not entry.endswith("/"):
        return path == entry
    directory = entry.rstrip("/").split("/")
    parts = path.split("/")
    return len(parts) > len(directory) and parts[: len(directory)] == directory


@PROPERTY
@given(path=PATHS, scope=SCOPES)
def test_scope_matching_agrees_with_segment_comparison(
    path: str, scope: tuple[str, ...]
) -> None:
    """Match a file entry exactly and a directory entry on a segment boundary."""
    expected = any(matches(path, entry) for entry in scope)
    assert scope_mod.is_in_scope(path, scope) is expected, f"{path} against {scope}"


@PROPERTY
@given(directory=st.lists(SEGMENT, min_size=1, max_size=3), tail=SEGMENT)
def test_a_directory_entry_never_matches_a_sibling_sharing_its_stem(
    directory: list[str], tail: str
) -> None:
    """Refuse `src/irony.rs` for `src/ir/`: the boundary is the slash, not a prefix."""
    entry = "/".join(directory) + "/"
    sibling = "/".join(directory) + tail
    inside = "/".join(directory) + "/" + tail
    assert not scope_mod.is_in_scope(sibling, (entry,)), f"{sibling} matched {entry}"
    assert scope_mod.is_in_scope(inside, (entry,)), f"{inside} missed {entry}"


@PROPERTY
@given(scope=SCOPES, changes=st.lists(PATHS, max_size=6), data=st.data())
def test_a_pull_request_runs_exactly_when_a_change_is_in_scope(
    scope: tuple[str, ...], changes: list[str], data: st.DataObject
) -> None:
    """Run the proofs on a pull request iff a change matches, in any order."""
    expected = any(matches(path, entry) for path in changes for entry in scope)
    shuffled = data.draw(st.permutations(changes))
    reordered = tuple(data.draw(st.permutations(scope)))
    decision = scope_mod.decide("pull_request", scope, changes)
    assert decision.run_proofs is expected, f"{changes} against {scope}"
    again = scope_mod.decide("pull_request", reordered, shuffled)
    assert again.run_proofs is expected, f"order changed the decision for {scope}"


@PROPERTY
@given(scope=SCOPES)
def test_an_unreadable_change_set_runs_the_proofs(scope: tuple[str, ...]) -> None:
    """Run every harness when the diff is unreadable, never skip on a guess."""
    decision = scope_mod.decide("pull_request", scope, None)
    assert decision.run_proofs is True, f"an unreadable diff skipped for {scope}"


@PROPERTY
@given(event=EVENTS, scope=SCOPES, changes=st.one_of(st.none(), st.lists(PATHS)))
def test_every_event_but_a_pull_request_runs_the_proofs(
    event: str, scope: tuple[str, ...], changes: list[str] | None
) -> None:
    """Run every harness on a push, schedule or dispatch whatever it changes."""
    decision = scope_mod.decide(event, scope, changes)
    assert decision.run_proofs is True, f"a `{event}` run skipped the proofs"
