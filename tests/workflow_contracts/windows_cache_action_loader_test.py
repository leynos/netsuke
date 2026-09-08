"""Failure-path tests for the Windows cache action loader.

`windows_cache_action.load_cache_action` is the only filesystem and YAML touch
in the Windows cache contracts, and it runs from a fixture, so its failures are
what a maintainer sees when the action is missing or half-edited. Each test
here drives a temporary file rather than the real action, so the paths are
exercised without the repository having to be broken.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import pytest
import yaml
from windows_cache_action import load_cache_action

if typ.TYPE_CHECKING:  # pragma: no cover - imported for annotations only
    import pathlib as pl


def test_loader_reports_the_file_it_could_not_read(tmp_path: pl.Path) -> None:
    """A missing action must name itself, not surface as a bare OSError.

    Scenario: the loader is the only filesystem touch in this module, and it
    runs from a fixture. Invariant: an unreadable file raises with the path and
    the failed operation, so a reader is not left deducing which of several
    YAML inputs went missing. Uses a temporary path, so it does not depend on
    the real action being absent.
    """
    missing = tmp_path / "absent" / "action.yml"
    with pytest.raises(RuntimeError, match="could not read") as caught:
        load_cache_action(missing)
    assert str(missing) in str(caught.value), (
        f"the failure must name the file it could not read, got {caught.value!r}"
    )
    assert isinstance(caught.value.__cause__, OSError), (
        "the underlying OSError must be chained rather than discarded, got "
        f"{caught.value.__cause__!r}"
    )


def test_loader_reports_the_file_it_could_not_parse(tmp_path: pl.Path) -> None:
    """Malformed YAML must name the file and the parse step.

    Scenario: a half-edited action is a likelier failure than a missing one,
    and `yaml.YAMLError` alone says nothing about which file it came from.
    Invariant: the loader raises naming the path and the parse operation, with
    the parser error chained. Uses a temporary file holding deliberately
    invalid YAML.
    """
    broken = tmp_path / "action.yml"
    broken.write_text("runs: [unclosed\n", encoding="utf-8")
    with pytest.raises(RuntimeError, match="could not parse") as caught:
        load_cache_action(broken)
    assert str(broken) in str(caught.value), (
        f"the failure must name the file it could not parse, got {caught.value!r}"
    )
    assert isinstance(caught.value.__cause__, yaml.YAMLError), (
        "the underlying YAMLError must be chained rather than discarded, got "
        f"{caught.value.__cause__!r}"
    )


def test_loader_rejects_a_document_that_is_not_a_mapping(tmp_path: pl.Path) -> None:
    """A well-formed but wrongly shaped action must not reach the helpers.

    Scenario: valid YAML that is a list or a scalar parses cleanly and would
    then fail somewhere inside a query helper. Invariant: the loader refuses it
    at the boundary, keeping the helpers' `dict[str, object]` contract true.
    """
    scalar = tmp_path / "action.yml"
    scalar.write_text("just a string\n", encoding="utf-8")
    with pytest.raises(pytest.fail.Exception, match="must be a mapping"):
        load_cache_action(scalar)
