"""Hold boundary naming to the meaning of a call rather than its history.

Four ``gh`` invocations reach GitHub during an upload, but only three command
shapes describe them: two of the four are the *same* ``gh release view``, and
which figure they belong to depends on whether a transfer has already happened.
The cases here pin that distinction, the subcommands that name themselves, and
the commands that are not boundaries at all.

The function is total and stateless, so a case states a phase and a command and
asserts a name. Nothing here needs a subprocess, a clock, or a staged payload;
:class:`release_upload_timing.TimedRunner` composes this naming with those and
is covered by ``scripts/tests/test_release_upload_timing.py``.

Run via ``make test-release-publication``.
"""

import pytest
from release_upload_boundaries import (
    PHASE_CONFIRMATION,
    PHASE_RECONCILIATION,
    UPLOAD_BOUNDARY,
    boundary_of,
    is_release_command,
)

#: The listing command, which is the one whose meaning the phase decides.
VIEW = ["gh", "release", "view", "v1", "--json", "assets"]

#: Why a repeated listing keeps its name, restated in each message.
REPEATED = "a repeated listing in one phase must keep its name"


@pytest.mark.parametrize(
    ("arguments", "expected"),
    [
        (["gh", "release", "download", "v1", "--pattern", "a.deb"], "download"),
        (["gh", "release", "upload", "v1", "a.deb"], "upload"),
        (["gh", "release", "delete-asset", "v1", "a.deb"], None),
        (["gh", "release", "create", "v1"], None),
        (["gh", "release", "list"], None),
        (["gh", "pr", "view"], None),
        (["gh", "auth", "status"], None),
        (["gh"], None),
        ([], None),
        (["python3", "-c", "pass"], None),
    ],
)
def test_a_command_names_the_boundary_it_crosses(
    arguments: list[str], expected: str | None
) -> None:
    """Name the two transferring subcommands and nothing else.

    ``download`` and ``upload`` mean one thing each and name themselves. The
    neighbouring release subcommands -- creating, listing, deleting an asset --
    are not one of the four boundaries, and neither is a non-release command:
    measuring them would attribute a duration to a figure that does not describe
    it.
    """
    assert boundary_of(arguments, phase=PHASE_RECONCILIATION) == expected, arguments


def test_the_first_listing_reconciles() -> None:
    """Name the listing taken before any transfer after its purpose."""
    assert boundary_of(VIEW, phase=PHASE_RECONCILIATION) == "listing", VIEW


def test_a_listing_after_a_transfer_verifies() -> None:
    """Name the same command by the later moment it is asked in.

    This is the whole reason the phase is an argument: a run's period is over
    once the transfer is sent, so a listing taken afterwards answers "what does
    the release hold" rather than "what is it missing".
    """
    assert boundary_of(VIEW, phase=PHASE_CONFIRMATION) == "verification", VIEW


def test_a_transfer_names_itself_in_either_phase() -> None:
    """Keep a transfer's name independent of when it happens.

    A retried transfer after a first one crossed the phase boundary is still an
    upload. Naming it anything else would leave the two attempts in different
    figures and hide the retry's cost.
    """
    upload = ["gh", "release", "upload", "v1", "a.deb"]
    download = ["gh", "release", "download", "v1", "--pattern", "a.deb"]
    for phase in (PHASE_RECONCILIATION, PHASE_CONFIRMATION):
        assert boundary_of(upload, phase=phase) == "upload", phase
        assert boundary_of(download, phase=phase) == "download", phase


def test_a_listing_is_named_the_same_way_every_time_in_one_phase() -> None:
    """Answer repeated calls alike, so a caller may list as often as it needs.

    Reconciliation lists once per undigested name in some runs. A naming that
    advanced on its own would call the second of those a verification, so the
    same call in the same phase has to keep its name.
    """
    for _ in range(3):
        reconciled = boundary_of(VIEW, phase=PHASE_RECONCILIATION)
        confirmed = boundary_of(VIEW, phase=PHASE_CONFIRMATION)
        assert reconciled == "listing", REPEATED
        assert confirmed == "verification", REPEATED


def test_an_unknown_phase_is_refused_rather_than_guessed() -> None:
    """Refuse a phase with no listing boundary instead of inventing one."""
    with pytest.raises(KeyError):
        boundary_of(VIEW, phase="retry")


def test_the_phase_names_are_the_two_the_runner_moves_between() -> None:
    """Keep the constants distinct and the upload the one that flips them.

    The upload boundary doubles as the phase transition, so it must not also
    be the name of a listing: a phase that changed on a listing would move the
    run on before the transfer it exists to bracket.
    """
    assert PHASE_RECONCILIATION != PHASE_CONFIRMATION, "the two phases must differ"
    assert UPLOAD_BOUNDARY not in {PHASE_RECONCILIATION, PHASE_CONFIRMATION}, (
        "a transfer must not also name a phase"
    )
    assert boundary_of(["gh", "release", "upload", "v1"], phase="x") == "upload", (
        "a transfer must ignore the phase"
    )


@pytest.mark.parametrize(
    ("arguments", "expected"),
    [
        (["gh", "release", "view"], True),
        (["gh", "pr", "view"], False),
        (["gh", "release"], False),
        (["release", "view"], False),
        ([], False),
    ],
)
def test_only_a_complete_gh_release_command_qualifies(
    arguments: list[str], *, expected: bool
) -> None:
    """Require the executable, the verb, and a subcommand to follow.

    ``gh release`` alone carries no subcommand, so indexing past it would read
    a missing element rather than name a boundary.
    """
    assert is_release_command(arguments) is expected, arguments
