"""Prove the decision-record contract refuses a guarded write.

The contract in ``release_publish_path_summary`` reads the authorization
script's syntax tree. These cases give it scripts it must accept and scripts it
must refuse, so the check is shown to distinguish an unconditional record from
one a branch can skip rather than merely to pass on the script of the day.
"""

import typing as typ

import pytest
import release_publish_path_summary as summary_contract

if typ.TYPE_CHECKING:
    import pathlib

_ENTRY = "def export_authorization():\n"
_UNCONDITIONAL = _ENTRY + "    write_summary(path, decision)\n"
_GUARDED = (
    _ENTRY + "    if authorized(decision):\n        write_summary(path, decision)\n"
)
_LOOPED = _ENTRY + "    for path in paths:\n        write_summary(path, decision)\n"
_NO_ENTRY = "def another_entry_point():\n    write_summary(path, decision)\n"


@pytest.mark.parametrize(
    ("source", "expected"),
    [
        pytest.param(_UNCONDITIONAL, None, id="unconditional-call"),
        pytest.param(
            _GUARDED,
            "authorization.summary: the decision record must be written "
            "unconditionally",
            id="call-inside-branch",
        ),
        pytest.param(
            _LOOPED,
            "authorization.summary: the decision record must be written "
            "unconditionally",
            id="call-inside-loop",
        ),
        pytest.param(
            _NO_ENTRY,
            "authorization.summary: script.py defines no export_authorization",
            id="missing-entry-point",
        ),
    ],
)
def test_summary_record_contract(
    tmp_path: pathlib.Path, *, source: str, expected: str | None
) -> None:
    """Accept an unconditional record and refuse one a construct can skip."""
    script = tmp_path / "script.py"
    script.write_text(source, encoding="utf-8")
    violations: list[str] = []

    summary_contract.check_summary_record(violations, script=script)

    if expected is None:
        assert not violations, f"an unconditional call must pass; got {violations!r}"
    else:
        assert expected in violations, (
            f"the contract must report {expected!r}; got {violations!r}"
        )


def test_summary_record_contract_reports_an_unreadable_script(
    tmp_path: pathlib.Path,
) -> None:
    """Report a missing script instead of treating it as a passing one."""
    violations: list[str] = []

    summary_contract.check_summary_record(violations, script=tmp_path / "absent.py")

    assert len(violations) == 1, (
        f"a missing script is one violation; got {violations!r}"
    )
    assert violations[0].startswith("authorization.summary: cannot read "), (
        f"the violation must name the unreadable script; got {violations[0]!r}"
    )


def test_repository_script_satisfies_the_contract() -> None:
    """Hold the script the workflow actually runs to the contract."""
    violations: list[str] = []

    summary_contract.check_summary_record(violations)

    assert not violations, (
        f"the shipped script must record every decision; got {violations!r}"
    )
