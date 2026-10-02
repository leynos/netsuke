"""Specify Cargo package metadata normalization and workflow output."""

import copy
import json
import typing as typ
import unicodedata

import pytest
from conftest import load_script_module
from hypothesis import given
from hypothesis import strategies as st

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
EXPECTED_CONTROL_CHARACTER_ERROR = (
    "package.description must not contain control characters or line separators"
)
SAFE_METADATA_CHARACTERS = st.characters(blacklist_categories=("Cc", "Zl", "Zp"))
METADATA_WHITESPACE = st.characters(whitelist_categories=("Zs",))


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


def test_metadata_error_preserves_message_when_copied_or_pickled() -> None:
    """Retain the formatted message and issue details across copies."""
    reader = _reader()
    issue = reader.PackageMetadataIssue.FIELD_EMPTY
    error = reader.PackageMetadataError(issue, "package.homepage")
    expected_message = "package.homepage must not be empty"

    assert error.args == (expected_message,), (
        "the exception args should expose its message"
    )
    for copied in (copy.copy(error), copy.deepcopy(error)):
        assert copied.issue is issue, "copies should retain the classified issue"
        assert copied.detail == "package.homepage", "copies should retain field context"
        assert str(copied) == expected_message, "copies should retain the diagnostic"


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
                "\t  Release Maintainer <release@example.test>  \r\n",
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


@given(
    leading=st.text(alphabet=METADATA_WHITESPACE, max_size=8),
    core=st.text(alphabet=SAFE_METADATA_CHARACTERS, min_size=1, max_size=64).filter(
        lambda value: bool(value.strip())
    ),
    trailing=st.text(alphabet=METADATA_WHITESPACE, max_size=8),
)
def test_required_string_trims_arbitrary_safe_unicode(
    leading: str, core: str, trailing: str
) -> None:
    """Preserve every safe Unicode string after trimming its edges."""
    reader = _reader()
    value = leading + core + trailing
    normalized = value.strip()

    assert reader._required_string(value, "package.description") == normalized, (
        "valid Unicode metadata should be trimmed without changing its content"
    )
    assert all(
        unicodedata.category(character) not in reader.INVALID_CHARACTER_CATEGORIES
        for character in normalized
    ), "normalized values should contain no forbidden Unicode categories"


@given(character=st.characters(categories=("Cc", "Zl", "Zp")))
def test_required_string_rejects_arbitrary_forbidden_unicode(character: str) -> None:
    """Reject every generated control or Unicode line-separator category."""
    reader = _reader()

    with pytest.raises(reader.PackageMetadataError) as error:
        reader._required_string(f"left{character}right", "package.description")

    assert error.value.issue is reader.PackageMetadataIssue.FIELD_CONTROL_CHARACTER, (
        "forbidden Unicode categories should use the control-character issue"
    )


@pytest.mark.parametrize(
    "invalid_case",
    [
        pytest.param(
            (_manifest_text({"homepage": None}), "package.homepage must be a string"),
            id="missing",
        ),
        pytest.param(
            (
                _manifest_text({"homepage": '"  "'}),
                "package.homepage must not be empty",
            ),
            id="empty",
        ),
        pytest.param(
            (_manifest_text({"license": "42"}), "package.license must be a string"),
            id="non-string",
        ),
        pytest.param(
            (
                _manifest_text({"description": '"first line\\nsecond line"'}),
                EXPECTED_CONTROL_CHARACTER_ERROR,
            ),
            id="control-character",
        ),
        pytest.param(
            (
                _manifest_text({"description": '"first line\\u2028second line"'}),
                EXPECTED_CONTROL_CHARACTER_ERROR,
            ),
            id="line-separator",
        ),
        pytest.param(
            (
                _manifest_text({"description": '"first line\\u2029second line"'}),
                EXPECTED_CONTROL_CHARACTER_ERROR,
            ),
            id="paragraph-separator",
        ),
        pytest.param(
            (
                _manifest_text({
                    "authors": '["", "Later Author <later@example.test>"]'
                }),
                "package.authors[0] must not be empty",
            ),
            id="empty-first-author",
        ),
        pytest.param(
            (
                _manifest_text({"authors": "[]"}),
                "package.authors must be a non-empty array",
            ),
            id="no-authors",
        ),
        pytest.param(
            (
                _manifest_text({"authors": '"not-an-array"'}),
                "package.authors must be a non-empty array",
            ),
            id="authors-not-array",
        ),
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
def test_main_rejects_invalid_manifests_without_output(
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
    invalid_case: tuple[str, str],
) -> None:
    """Reject invalid package values and package tables before writing outputs."""
    manifest_contents, message = invalid_case
    manifest = tmp_path / "Cargo.toml"
    manifest.write_text(manifest_contents, encoding="utf-8")
    output = tmp_path / "github-output"
    monkeypatch.setenv("GITHUB_OUTPUT", str(output))

    status = _reader().main(["--manifest", str(manifest)])

    captured = capsys.readouterr()
    assert status == 1, "invalid package metadata should fail"
    assert captured.err == f"error: {message}\n", (
        "the error should describe the invalid package metadata"
    )
    assert not captured.out, "metadata errors should not write standard output"
    assert not output.exists(), "metadata errors should not write workflow outputs"
