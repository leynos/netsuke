"""Specify Cargo package metadata normalization and workflow output."""

import json
import typing as typ

import pytest
from conftest import load_script_module

if typ.TYPE_CHECKING:
    import pathlib
    import types

READER_MODULE_NAME = "cargo_package_metadata_test"
PACKAGE_VALUES = {
    "homepage": "https://example.test/project",
    "license": "ISC",
    "description": "A useful package description.",
}
EXPECTED_GITHUB_OUTPUT = (
    "maintainer=First Author <first@example.test>\n"
    "homepage=https://example.test/project\n"
    "license=ISC\n"
    "description=A useful package description.\n"
)


def _manifest_text(overrides: dict[str, str | None] | None = None) -> str:
    """Build a minimal Cargo manifest from raw TOML field values."""
    fields = {
        "authors": (
            '["First Author <first@example.test>", '
            '"Second Author <second@example.test>"]'
        ),
        **{key: json.dumps(value) for key, value in PACKAGE_VALUES.items()},
    }
    if overrides:
        fields.update(overrides)
    package_lines = [
        f"{key} = {value}" for key, value in fields.items() if value is not None
    ]
    return "[package]\n" + "\n".join(package_lines) + "\n"


def _write_manifest(
    tmp_path: pathlib.Path, overrides: dict[str, str | None] | None = None
) -> pathlib.Path:
    """Write one temporary manifest with optional raw TOML overrides."""
    manifest = tmp_path / "Cargo.toml"
    manifest.write_text(_manifest_text(overrides), encoding="utf-8")
    return manifest


def _reader() -> types.ModuleType:
    """Load the production reader through the shared script test seam."""
    return load_script_module(READER_MODULE_NAME, "cargo_package_metadata.py")


def test_main_writes_normalized_package_values_in_stable_order(
    tmp_path: pathlib.Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Write one value for each package field using the workflow output format."""
    manifest = _write_manifest(tmp_path)
    output = tmp_path / "github-output"
    monkeypatch.setenv("GITHUB_OUTPUT", str(output))

    status = _reader().main(["--manifest", str(manifest)])

    assert status == 0, "valid package metadata should be written"
    actual_output = output.read_text(encoding="utf-8")
    assert actual_output == EXPECTED_GITHUB_OUTPUT, (
        "the reader should write all fields in stable order"
    )


def test_reader_selects_the_first_author(
    tmp_path: pathlib.Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Use the first Cargo author even when later authors are also present."""
    manifest = _write_manifest(
        tmp_path,
        {
            "authors": json.dumps([
                "  Release Maintainer <release@example.test>  ",
                "Other Author",
            ])
        },
    )
    output = tmp_path / "github-output"
    monkeypatch.setenv("GITHUB_OUTPUT", str(output))

    status = _reader().main(["--manifest", str(manifest)])

    assert status == 0, "valid package metadata should be written"
    assert output.read_text(encoding="utf-8").splitlines()[0] == (
        "maintainer=Release Maintainer <release@example.test>"
    ), "the first Cargo author should become the maintainer"


@pytest.mark.parametrize(
    "invalid_value",
    [
        pytest.param(
            ("homepage", None, "package.homepage must be a string"), id="missing"
        ),
        pytest.param(
            ("homepage", '"  "', "package.homepage must not be empty"), id="empty"
        ),
        pytest.param(
            ("license", "42", "package.license must be a string"), id="non-string"
        ),
        pytest.param(
            (
                "description",
                '"first line\\nsecond line"',
                "package.description must not contain control characters",
            ),
            id="control-character",
        ),
        pytest.param(
            (
                "authors",
                '["", "Later Author <later@example.test>"]',
                "package.authors[0] must not be empty",
            ),
            id="empty-first-author",
        ),
        pytest.param(
            ("authors", "[]", "package.authors must be a non-empty array"),
            id="no-authors",
        ),
        pytest.param(
            (
                "authors",
                '"not-an-array"',
                "package.authors must be a non-empty array",
            ),
            id="authors-not-array",
        ),
    ],
)
def test_main_rejects_invalid_package_values_without_output(
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
    invalid_value: tuple[str, str | None, str],
) -> None:
    """Reject missing, empty, non-string, control, and author-list values."""
    field, raw_value, message = invalid_value
    manifest = _write_manifest(tmp_path, {field: raw_value})
    output = tmp_path / "github-output"
    monkeypatch.setenv("GITHUB_OUTPUT", str(output))

    status = _reader().main(["--manifest", str(manifest)])

    captured = capsys.readouterr()
    assert status == 1, "invalid package metadata should fail"
    assert captured.err == f"error: {message}\n", (
        "the error should name the invalid field"
    )
    assert not captured.out, "metadata errors should not write standard output"
    assert not output.exists(), "metadata errors should not write workflow outputs"


@pytest.mark.parametrize(
    "invalid_manifest",
    [
        pytest.param(
            (
                "[workspace]\nmembers = []\n",
                "Cargo manifest must define a [package] table",
            ),
            id="missing-package-table",
        ),
        pytest.param(
            (
                '[package]\nauthors = "not-an-array"\n',
                "package.authors must be a non-empty array",
            ),
            id="invalid-authors-table",
        ),
    ],
)
def test_main_rejects_manifests_without_a_valid_package_table(
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
    invalid_manifest: tuple[str, str],
) -> None:
    """Reject manifests that omit package authors before writing outputs."""
    manifest_text, message = invalid_manifest
    manifest = tmp_path / "Cargo.toml"
    manifest.write_text(manifest_text, encoding="utf-8")
    output = tmp_path / "github-output"
    monkeypatch.setenv("GITHUB_OUTPUT", str(output))

    status = _reader().main(["--manifest", str(manifest)])

    captured = capsys.readouterr()
    assert status == 1, "incomplete package metadata should fail"
    assert captured.err == f"error: {message}\n", (
        "the error should describe the missing package field"
    )
    assert not captured.out, "metadata errors should not write standard output"
    assert not output.exists(), "metadata errors should not write workflow outputs"
