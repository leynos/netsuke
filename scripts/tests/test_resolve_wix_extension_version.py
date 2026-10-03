"""Test WiX version validation and GitHub output export."""

import typing as typ

import pytest
import resolve_wix_extension_version as resolver

if typ.TYPE_CHECKING:
    from pathlib import Path

INVALID_VERSION_CASES = (
    pytest.param(("", "WIX_EXTENSION_VERSION must be set"), id="empty"),
    pytest.param(
        ("7\nvalue=8", "WIX_EXTENSION_VERSION must not contain line breaks"),
        id="line-feed",
    ),
    pytest.param(
        ("7\rvalue=8", "WIX_EXTENSION_VERSION must not contain line breaks"),
        id="carriage-return",
    ),
)


@pytest.mark.parametrize("version", ["7", "8.1.2"])
def test_exports_version_and_reports_it(
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
    tmp_path: Path,
    version: str,
) -> None:
    """Append the supplied version and report the value to the workflow log."""
    output_path = tmp_path / "github-output"
    output_path.write_text("existing=value\n", encoding="utf-8")
    monkeypatch.setenv("INPUT_WIX_EXTENSION_VERSION", version)
    monkeypatch.setenv("GITHUB_OUTPUT", str(output_path))

    resolver.app([], result_action="return_value")

    assert output_path.read_text(encoding="utf-8") == (
        f"existing=value\nvalue={version}\n"
    ), "the workflow output must append without replacing existing values"
    assert capsys.readouterr().out == (
        f"Resolved WiX extension version: {version}\n"
    ), "the workflow log must report the resolved version"


@pytest.mark.parametrize("case", INVALID_VERSION_CASES)
def test_rejects_values_that_cannot_be_written_safely(
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
    tmp_path: Path,
    case: tuple[str, str],
) -> None:
    """Reject empty and multiline values before writing workflow output."""
    version, message = case
    output_path = tmp_path / "github-output"
    monkeypatch.setenv("INPUT_WIX_EXTENSION_VERSION", version)
    monkeypatch.setenv("GITHUB_OUTPUT", str(output_path))

    with pytest.raises(SystemExit) as error:
        resolver.app([], result_action="return_value")

    assert error.value.code == 1, "an invalid version must fail the workflow step"
    assert capsys.readouterr().err == f"{message}\n", (
        "the resolver must report the validation failure"
    )
    assert not output_path.exists(), "invalid input must not create workflow output"


@pytest.mark.parametrize("case", INVALID_VERSION_CASES)
def test_version_validation_reports_specific_errors(case: tuple[str, str]) -> None:
    """Expose distinct diagnostics for empty and multiline values."""
    version, message = case
    with pytest.raises(resolver.ExtensionVersionError, match=message):
        resolver.resolve_extension_version(version)
