"""Guard immediate passing output for the RFC coverage progress count.

A partial split can pass the coverage assertions. The count therefore needs
immediate output, so progress is visible before Nextest's final report.
Run these contracts with ``make test-workflow-contracts``.
"""

import typing as typ

import pytest
from nextest_child_cargo_group_invariants import nextest_config
from nextest_rust_test_discovery import declared_test_names
from nextest_success_output import (
    IMMEDIATE_OUTPUT_TESTS,
    NEXTEST_CONFIG,
    immediate_output_offences,
)
from nextest_success_output_mutations import (
    EXPECTED_OFFENCE,
    GUARDED_TEST,
    MUTATIONS,
    mutate_success_output,
)

if typ.TYPE_CHECKING:
    from pathlib import Path

BREAKING = tuple(EXPECTED_OFFENCE)


def _parsed(tmp_path: Path, document: str) -> dict[str, object]:
    """Load an isolated synthetic configuration through the shared TOML reader."""
    source = tmp_path / "nextest.toml"
    source.write_text(document, encoding="utf-8")
    return nextest_config(source)


def test_the_repository_configuration_keeps_its_evidence_channel() -> None:
    """The checked-in default profile preserves immediate progress output."""
    offences = immediate_output_offences(nextest_config(NEXTEST_CONFIG))
    assert not offences, f"the {GUARDED_TEST} output override is invalid: {offences}"


def test_every_guarded_test_is_declared_by_the_test_tree() -> None:
    """The guarded names resolve through the existing Rust discovery helper."""
    declared = declared_test_names()
    unresolved = [test for test in IMMEDIATE_OUTPUT_TESTS if test not in declared]
    assert not unresolved, f"guarded Rust tests are undeclared: {unresolved}"


def test_the_identity_configuration_is_accepted(tmp_path: Path) -> None:
    """A valid fixture passes before its individual conditions are mutated."""
    valid = _parsed(tmp_path, mutate_success_output("valid"))
    offences = immediate_output_offences(valid)
    assert not offences, f"valid override rejected: {offences}"


@pytest.mark.parametrize("mutation", BREAKING)
def test_every_mutation_is_reported_for_the_reason_it_names(
    tmp_path: Path, mutation: str
) -> None:
    """Every one-condition mutation fails for its intended diagnostic."""
    config = _parsed(tmp_path, mutate_success_output(mutation))
    offences = immediate_output_offences(config)
    expected = EXPECTED_OFFENCE[mutation]
    assert len(offences) == 1, f"{mutation}: expected one offence, found {offences}"
    assert GUARDED_TEST in offences[0], offences
    assert expected in offences[0], f"{mutation}: expected {expected!r} in {offences}"


def test_the_identity_is_the_only_accepted_mutation(tmp_path: Path) -> None:
    """Every declared negative fixture changes an enforced condition."""
    accepted = [
        mutation
        for mutation in MUTATIONS
        if not immediate_output_offences(
            _parsed(tmp_path, mutate_success_output(mutation))
        )
    ]
    assert accepted == ["valid"], f"unexpected accepted mutations: {accepted}"
