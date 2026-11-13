"""Hold the upload decision to reading an observation it is handed.

An upload run is two jobs: an effect, and a decision about what the effect
found. The effect lists the release and, for the names the listing left
undigested, downloads and hashes them. The decision turns that observation into
a per-asset plan and reaches nothing -- no network call, no file opened.

These cases pin the second half's emptiness rather than describing it. A stub
runner that fails the assertion if it is called at all proves the decision
cannot have reached GitHub, and a refused write proves it cannot have recorded
anything; neither claim is visible from a plan that merely came out right.

Run via ``make test-release-publication``.
"""

import typing as typ

import pytest
import release_reconcile_github
import release_upload_observation
import release_upload_remote

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    import pathlib


def _refuse(message: str) -> typ.NoReturn:
    """Fail the test from inside a call that the decision must not make."""
    raise AssertionError(message)


def _refusing(message: str) -> cabc.Callable[..., typ.NoReturn]:
    """Return a callable that refuses when it is finally called.

    ``monkeypatch.setattr`` takes the replacement, not its result, so the
    refusal has to be deferred: written as ``_refuse(...)`` it would fire
    while the test was still arranging itself, and every case would fail for
    the same reason whether or not the decision ever reached anything.

    Returns
    -------
    collections.abc.Callable[..., typing.NoReturn]
        A stand-in that fails the case when invoked.
    """

    def refuse(*_args: object, **_keywords: object) -> typ.NoReturn:
        """Fail the case from the substituted call."""
        return _refuse(message)

    return refuse


@pytest.mark.parametrize(
    ("observed", "expected", "actions"),
    [
        # Attached with the recorded digest: nothing to send.
        (
            {"a": "aa", "b": "bb"},
            {"a": "aa", "b": "bb"},
            [("a", "skip-verified"), ("b", "skip-verified")],
        ),
        # Attached with different bytes: a conflict, and nothing is clobbered.
        ({"a": "ff"}, {"a": "aa"}, [("a", "conflict")]),
        # Absent from the release entirely.
        ({}, {"a": "aa"}, [("a", "upload")]),
        # Attached, but neither the listing nor a download established a digest.
        ({"a": None}, {"a": "aa"}, [("a", "unverifiable")]),
    ],
)
def test_settle_observation_decides_without_reaching_anything(
    monkeypatch: pytest.MonkeyPatch,
    observed: dict[str, str | None],
    expected: dict[str, str],
    actions: list[tuple[str, str]],
) -> None:
    """Decide from a handwritten observation, with every effect made fatal.

    The parameter table is the reconciliation's own vocabulary, so a decision
    that needed the network to reach one of these answers would show up as a
    failed call rather than as a plan that happened to agree.
    """
    monkeypatch.setattr(
        release_upload_remote,
        "attached_digests",
        _refusing("the decision must not list the release"),
    )
    settlement = release_upload_observation.settle_observation(
        release_upload_observation.Observation(digests=observed, failures={}),
        expected=expected,
    )
    assert [(name, action.value) for name, action in settlement.actions] == actions, (
        "the plan must be decided from the observation alone"
    )


def test_settle_observation_writes_nothing(
    monkeypatch: pytest.MonkeyPatch, tmp_path: pathlib.Path
) -> None:
    """Refuse every write the decision could make, and decide anyway."""
    for member in ("write_text", "write_bytes", "mkdir", "touch"):
        monkeypatch.setattr(
            tmp_path.__class__,
            member,
            _refusing(f"the decision must not call Path.{member}"),
        )
    settlement = release_upload_observation.settle_observation(
        release_upload_observation.Observation(digests={"a": "aa"}, failures={}),
        expected={"a": "aa"},
    )
    assert [action for _name, action in settlement.actions] == [
        release_reconcile_github.AssetAction.SKIP_VERIFIED
    ], "the decision must be taken without writing anything"


def test_observe_release_is_the_half_that_reaches_the_release() -> None:
    """Take the effect through its injected runner, and record what it asked.

    The listing already carries every digest here, so the download half is
    skipped entirely: the observation is one ``view`` and nothing else. That
    is the boundary the cases above hold the decision away from.
    """
    runner = release_upload_remote.StaticRunner(
        '{"assets": [{"name": "a", "digest": "sha256:aa"}]}'
    )
    observed = release_upload_observation.observe_release(
        "v1", {"a": "aa"}, runner=runner
    )
    assert observed.digests == {"a": "aa"}, observed.digests
    assert [call[2] for call in runner.calls] == ["view"], (
        "a fully digested listing must be observed without a download"
    )
