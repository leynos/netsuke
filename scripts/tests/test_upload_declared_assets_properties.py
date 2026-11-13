"""Generated properties over mixed release states for the uploader.

``test_upload_declared_assets_cli.py`` names one finding per case and is the
specification: each of its cases says *why* a particular mixture matters. This
module holds the relation those cases compose, over mixtures nobody wrote
down. A release can present any combination of declared assets -- absent,
attached and matching, attached and disagreeing, attached with unreadable
bytes -- in any order, and the run must do exactly one of two things: refuse
before it transfers anything, or upload precisely the declared assets GitHub
does not have.

The two branches are generated rather than filtered. A property written as one
``assume``-guarded case would spend its example budget rejecting mixtures and
would quietly stop covering a branch if the generator drifted; forcing one
blocking name into one strategy and one usable name into the other keeps both
branches populated by construction.

Run via ``make test-release-publication``.
"""

import dataclasses
import enum
import itertools
import pathlib

# Module scope rather than `TYPE_CHECKING` because Hypothesis reads each
# `@given` test's signature as the decorator is applied, and under PEP 649 that
# evaluates the annotations then and there: a name bound only for type checking
# is unbound at exactly the moment it is needed, and the whole module fails to
# collect.
import pytest  # ruff: ignore[typing-only-third-party-import] - see note.
from hypothesis import HealthCheck, given, settings
from hypothesis import strategies as st
from release_upload_support import Exchange, Invocation, Scenario, run_cli
from upload_declared_assets_refusals import EXIT_CONFLICT, EXIT_OK

#: Asset names the payload and the plan can both carry. Generated rather than
#: drawn from a fixed list so a case cannot pass by being in a lucky order, and
#: constrained to filename-safe shapes so a name is never the reason a case
#: fails.
_NAME = st.from_regex(r"[a-z0-9][a-z0-9._-]{0,9}", fullmatch=True)

_SETTINGS = settings(
    max_examples=50,
    deadline=None,
    derandomize=True,
    # `tmp_path` and `monkeypatch` are function-scoped, so every example needs
    # a fresh directory. That is the point: a case must not inherit the files
    # the previous one left behind.
    suppress_health_check=[HealthCheck.function_scoped_fixture],
)


class State(enum.Enum):
    """Name one declared asset's state on the release under test."""

    #: GitHub has not attached the name, so the run must send it.
    ABSENT = "absent"

    #: Attached, and its digest agrees with the trusted record.
    MATCHING = "matching"

    #: Attached, and its digest was read and disagrees.
    CONFLICTING = "conflicting"

    #: Attached, but neither the listing nor a download could yield a digest.
    UNVERIFIABLE = "unverifiable"


#: The two states that stop a run from transferring anything, for the reasons
#: `release_upload_observation.should_upload` records.
BLOCKING = (State.CONFLICTING, State.UNVERIFIABLE)


@dataclasses.dataclass(frozen=True, slots=True)
class Staged:
    """Hold one generated release, the stub answering it, and what was asked.

    Attributes
    ----------
    scenario
        The payload, trusted record, and plan destination the run reads.
    exchange
        The stub standing in for ``gh``, holding the release's attached assets.
    states
        Declared name mapped to the state the case gave it. The assertions
        compare the run's plan against this, so it is the case's own account of
        what the release looked like.
    """

    scenario: Scenario
    exchange: Exchange
    states: dict[str, State]

    def names_in(self, *wanted: State) -> set[str]:
        """Return the declared names the case gave any of ``wanted``.

        Returns
        -------
        set[str]
            Names whose generated state is one of ``wanted``.
        """
        return {name for name, state in self.states.items() if state in wanted}

    def uploaded(self) -> list[str]:
        """Return the asset names the run sent to ``gh``, in invocation order.

        Returns
        -------
        list[str]
            One entry per file argument across every upload invocation.
        """
        return [
            pathlib.Path(argument).name
            for call in self.exchange.uploads()
            for argument in call[4:]
        ]

    def recorded_blockers(self) -> tuple[set[str], set[str]]:
        """Return the conflicts and unverifiable names the plan recorded.

        Returns
        -------
        tuple[set[str], set[str]]
            The plan's ``conflicts`` names and its ``unverifiable`` names.
        """
        plan = self.scenario.stored_plan()
        return set(plan["conflicts"]), set(plan["unverifiable"])


def _ordered(draw: st.DrawFn, states: dict[str, State]) -> dict[str, State]:
    """Return ``states`` in an order the case chooses.

    Returns
    -------
    dict[str, State]
        The same pairs, in their own order or the reverse of it.

    Notes
    -----
    The uploader sorts names before deciding, so the answer must not depend on
    the order a release happens to present them in. Rebuilding the mapping in a
    drawn order is what gives an order-independent assertion something to be
    independent of.
    """
    pairs = list(states.items())
    if draw(st.booleans()):
        pairs.reverse()
    return dict(pairs)


@st.composite
def _any_release(draw: st.DrawFn) -> dict[str, State]:
    """Generate a release with any mixture of the four states.

    Returns
    -------
    dict[str, State]
        Declared name mapped to its generated state.
    """
    return _ordered(
        draw,
        dict(
            draw(
                st.dictionaries(
                    _NAME, st.sampled_from(tuple(State)), min_size=1, max_size=5
                )
            )
        ),
    )


@st.composite
def _blocked_release(draw: st.DrawFn) -> dict[str, State]:
    """Generate a release that holds at least one blocker.

    Returns
    -------
    dict[str, State]
        Declared name mapped to its generated state, one of which is
        conflicting or unverifiable.
    """
    states = dict(
        draw(st.dictionaries(_NAME, st.sampled_from(tuple(State)), max_size=4))
    )
    states[draw(_NAME)] = draw(st.sampled_from(BLOCKING))
    return _ordered(draw, states)


@st.composite
def _unblocked_release(draw: st.DrawFn) -> dict[str, State]:
    """Generate a release with nothing that blocks an upload.

    Returns
    -------
    dict[str, State]
        Declared name mapped to its generated state, none of which is
        conflicting or unverifiable.
    """
    states = dict(
        draw(
            st.dictionaries(
                _NAME, st.sampled_from(tuple(State)), min_size=1, max_size=5
            )
        )
    )
    usable = (State.ABSENT, State.MATCHING)
    return _ordered(
        draw,
        {
            name: state if state not in BLOCKING else draw(st.sampled_from(usable))
            for name, state in states.items()
        },
    )


def _different(digest: str) -> str:
    """Return a hexadecimal digest that differs from ``digest``.

    Returns
    -------
    str
        The same digest with its last digit changed, so the two cannot compare
        equal whatever the payload's bytes hashed to.
    """
    return digest[:-1] + ("0" if digest[-1] != "0" else "1")


_ROOT = itertools.count()


def _stage(tmp_path: pathlib.Path, states: dict[str, State]) -> Staged:
    """Write a payload for ``states`` and a stub that presents them.

    Parameters
    ----------
    tmp_path
        The case's scratch root. Each call takes a fresh directory below it, so
        one example cannot read another's files.
    states
        Declared name mapped to the state the release should present.

    Returns
    -------
    Staged
        The scenario, the stub, and the states that produced both.

    Notes
    -----
    A name generated as unverifiable is attached without a listing digest and
    with no bytes staged for its download, which is exactly the pair of
    shortcomings the run cannot resolve: the transfer 404s and the digest stays
    unknown.
    """
    root = tmp_path / f"case-{next(_ROOT)}"
    root.mkdir()
    scenario = Scenario(
        root, contents={name: f"bytes for {name}".encode() for name in states}
    )
    expected = scenario.digests()
    attached: dict[str, str | None] = {}
    for name, state in states.items():
        if state is State.MATCHING:
            attached[name] = f"sha256:{expected[name]}"
        elif state is State.CONFLICTING:
            attached[name] = f"sha256:{_different(expected[name])}"
        elif state is State.UNVERIFIABLE:
            attached[name] = None
    return Staged(
        scenario=scenario,
        exchange=Exchange(attached=attached),
        states=states,
    )


@_SETTINGS
@given(states=_blocked_release())
def test_a_blocked_release_transfers_nothing(
    states: dict[str, State],
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Refuse before the first transfer, whatever else the release holds.

    A run that sent the missing names first would leave the release holding a
    mixture of this run's bytes and an earlier run's, which is harder to unwind
    than a refusal. So the blocked branch asserts two things at once: the exit
    status names a conflict, and ``gh`` was never asked to upload anything --
    not even the assets that were merely absent.
    """
    staged = _stage(tmp_path, states)
    assert run_cli(staged.scenario, staged.exchange, monkeypatch) == EXIT_CONFLICT, (
        "a release holding a blocker must be refused"
    )
    assert not staged.exchange.uploads(), (
        f"a blocked run must transfer nothing, sent {staged.uploaded()}"
    )
    assert not staged.exchange.clobbered(), "no refusal may reach for --clobber"


@_SETTINGS
@given(states=_blocked_release())
def test_the_plan_reports_each_blocker_under_its_own_kind(
    states: dict[str, State],
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Record conflicts and unreadable digests apart, naming every one of each.

    The two need different words and different next steps from an operator, so
    a run that folded them together would report a transfer fault as though the
    release held demonstrably wrong bytes.
    """
    staged = _stage(tmp_path, states)
    _ = run_cli(staged.scenario, staged.exchange, monkeypatch)
    conflicts, unreadable = staged.recorded_blockers()
    assert conflicts == staged.names_in(State.CONFLICTING), (
        "every asset whose bytes disagreed must be recorded as a conflict"
    )
    assert unreadable == staged.names_in(State.UNVERIFIABLE), (
        "every asset whose digest could not be read must be recorded apart"
    )
    reasons = staged.scenario.stored_plan()["unverifiable_reasons"]
    assert set(reasons) == unreadable, (
        "each unverifiable asset must carry the bounded category explaining it"
    )
    assert set(reasons.values()) <= {
        "download-failed",
        "missing-file",
        "not-a-regular-file",
        "read-failed",
    }, "a recorded reason must be one of the published categories"


@_SETTINGS
@given(states=_unblocked_release())
def test_an_unblocked_release_uploads_exactly_the_missing_assets(
    states: dict[str, State],
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Send the absent declared names and nothing else, in one invocation.

    "Exactly" is the whole property: a name the release already holds must not
    be re-sent, because the upload never carries ``--clobber`` and so a second
    send would fail; and a name the payload does not declare is not this run's
    to send at all.
    """
    staged = _stage(tmp_path, states)
    assert run_cli(staged.scenario, staged.exchange, monkeypatch) == EXIT_OK, (
        "an unblocked run must settle the release"
    )
    missing = staged.names_in(State.ABSENT)
    assert sorted(staged.uploaded()) == sorted(missing), (
        f"the run must send exactly the absent names, sent {staged.uploaded()}"
    )
    assert len(staged.exchange.uploads()) <= 1, (
        "every pending name belongs in one invocation, so a partial send is visible"
    )
    assert not staged.exchange.clobbered(), "no upload may reach for --clobber"


@_SETTINGS
@given(states=_any_release())
def test_a_dry_run_never_transfers_whatever_the_release_holds(
    states: dict[str, State],
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Plan against the release in every mixture and send nothing.

    A rehearsal that uploaded on the way would not be a rehearsal, and it would
    spend a release's assets on a run nobody meant to publish. The exit status
    is still allowed to refuse -- a dry run reports what a real one would --
    but no branch of it may reach ``gh release upload``.
    """
    staged = _stage(tmp_path, states)
    exit_code = run_cli(
        staged.scenario,
        staged.exchange,
        monkeypatch,
        invocation=Invocation(dry_run=True),
    )
    blockers = staged.names_in(*BLOCKING)
    assert exit_code == (EXIT_CONFLICT if blockers else EXIT_OK), (
        "a dry run must report the status a real run of the same release would"
    )
    assert not staged.exchange.uploads(), (
        f"a dry run must transfer nothing, sent {staged.uploaded()}"
    )
