"""Prove a write job's guard is judged the same wrapped or bare.

GitHub accepts a job-level ``if`` either as a bare expression or wrapped in
``${{ }}``, and the two spellings mean the same guard. The publication gate
reads the guard structurally, so this module holds a wrapped guard to exactly
the verdict the bare one receives: every required clause counts as present,
and a disjunction or ``always()`` is still refused. The mutation suite proves
the gate rejects regressions in the committed workflow; these cases prove it
reads a guard GitHub would honour rather than only the spelling it happens to
be written in today.
"""

import pytest
from release_write_authority_contract import REQUIRED_GUARD_CLAUSES, check_write_guard

#: A guard carrying every clause the publication gate requires.
COMPLETE_GUARD = " && ".join(("!cancelled()", *REQUIRED_GUARD_CLAUSES))


def _violations(guard: str) -> list[str]:
    """Return the findings one guard attracts for a single write job."""
    violations: list[str] = []
    check_write_guard("publish-packages", {"if": guard}, violations)
    return violations


@pytest.mark.parametrize(
    "guard",
    [COMPLETE_GUARD, f"${{{{ {COMPLETE_GUARD} }}}}"],
    ids=["bare", "wrapped"],
)
def test_complete_guard_is_accepted_in_either_spelling(guard: str) -> None:
    """Accept a guard carrying every clause, wrapped or bare.

    The wrapped spelling is the case a naive reader fails: the final clause
    keeps its ``}}`` and every earlier clause is unaffected, so the report is
    a single missing check that the guard does carry.
    """
    violations = _violations(guard)

    assert not [v for v in violations if ".guard:" in v], (
        f"a complete guard should attract no guard finding; got {violations!r}"
    )


@pytest.mark.parametrize(
    "suffix",
    [" || true", " || github.event_name == 'push'"],
    ids=["always-true", "alternate-trigger"],
)
def test_wrapped_disjunction_is_still_refused(suffix: str) -> None:
    """Refuse a wrapped guard carrying a disjunct appended after the gate."""
    guard = f"${{{{ {COMPLETE_GUARD}{suffix} }}}}"

    violations = _violations(guard)

    assert any("must be a conjunction" in v for v in violations), (
        f"a disjunctive guard should be refused; got {violations!r}"
    )


def test_wrapped_always_call_is_still_refused() -> None:
    """Refuse a wrapped guard that reaches for a status call over the gate."""
    guard = f"${{{{ always() && {COMPLETE_GUARD} }}}}"

    violations = _violations(guard)

    assert any("always()" in v for v in violations), (
        f"an unconditional status call should be refused; got {violations!r}"
    )


def test_wrapped_guard_missing_a_clause_still_reports_it() -> None:
    """Report the absent clause, not the wrapper, when a check is dropped."""
    dropped = REQUIRED_GUARD_CLAUSES[0]
    kept = " && ".join(c for c in COMPLETE_GUARD.split(" && ") if c != dropped)
    guard = f"${{{{ {kept} }}}}"

    violations = _violations(guard)

    assert any(dropped in v for v in violations), (
        f"the missing clause should be named; got {violations!r}"
    )
