"""Specify built Linux package metadata checks at the release boundary."""

import hashlib
import io
import itertools
import json
import pathlib
import subprocess  # ruff: ignore[suspicious-subprocess-import] - test seam.
import tarfile
import typing as typ

import pytest
from conftest import load_script_module

if typ.TYPE_CHECKING:
    import types

VALID_METADATA = {
    "maintainer": "Cargo Fixture Maintainer <cargo@example.test>",
    "homepage": "https://cargo.example.test/linux-package-fixture",
    "license": "Apache-2.0",
    "description": "A manifest-sourced Linux package fixture.",
}
# Keep package-command values literal so tests compare independent sources.
DEBIAN_FIELDS = {
    "Maintainer": "Cargo Fixture Maintainer <cargo@example.test>",
    "Homepage": "https://cargo.example.test/linux-package-fixture",
    "Description": "A manifest-sourced Linux package fixture.",
}
RPM_TAGS = {
    "PACKAGER": "Cargo Fixture Maintainer <cargo@example.test>",
    "URL": "https://cargo.example.test/linux-package-fixture",
    "LICENSE": "Apache-2.0",
    "SUMMARY": "A manifest-sourced Linux package fixture.",
    "DESCRIPTION": "A manifest-sourced Linux package fixture.",
}
LICENCE_CONTENTS = b"Independent package fixture licence bytes.\n"
VALIDATOR_MODULE_NAME = "validate_linux_package_metadata_test"


class FakePackageCommands:
    """Return configured package headers and a generated Debian file list."""

    def __init__(
        self,
        *,
        debian_fields: dict[str, str] | None = None,
        rpm_tags: dict[str, str] | None = None,
        copyright_contents: bytes | None = LICENCE_CONTENTS,
        copyright_member_name: str = "./usr/share/doc/netsuke/copyright",
    ) -> None:
        """Configure package fields and optional copyright-file contents."""
        self.debian_fields = dict(
            DEBIAN_FIELDS if debian_fields is None else debian_fields
        )
        self.rpm_tags = dict(RPM_TAGS if rpm_tags is None else rpm_tags)
        self.copyright_contents = copyright_contents
        self.copyright_member_name = copyright_member_name
        self.calls: list[list[str]] = []

    def __call__(
        self, argv: list[str], **kwargs: object
    ) -> subprocess.CompletedProcess[bytes]:
        """Answer one dpkg-deb or RPM query with captured byte output."""
        del kwargs
        self.calls.append(argv)
        if pathlib.Path(argv[0]).name == "dpkg-deb":
            if "--field" in argv:
                value = self.debian_fields.get(argv[-1], "")
                return _completed(argv, value.encode("utf-8"))
            return _completed(
                argv,
                _copyright_archive(self.copyright_contents, self.copyright_member_name),
            )

        query = argv[argv.index("--queryformat") + 1]
        tag = query.removeprefix("%{").removesuffix("}")
        value = self.rpm_tags.get(tag, "")
        return _completed(argv, value.encode("utf-8"))


def _completed(argv: list[str], stdout: bytes) -> subprocess.CompletedProcess[bytes]:
    """Build a successful result carrying the supplied stdout bytes."""
    return subprocess.CompletedProcess(argv, 0, stdout, b"")


def _copyright_archive(contents: bytes | None, member_name: str) -> bytes:
    """Build a tar archive with one packaged copyright member."""
    output = io.BytesIO()
    with tarfile.open(fileobj=output, mode="w") as archive:
        if contents is not None:
            member = tarfile.TarInfo(member_name)
            member.size = len(contents)
            archive.addfile(member, io.BytesIO(contents))
    return output.getvalue()


def _module() -> types.ModuleType:
    """Load the validator through the shared script test seam."""
    return load_script_module(
        VALIDATOR_MODULE_NAME, "validate_linux_package_metadata.py"
    )


def _prepare_inputs(
    tmp_path: pathlib.Path, monkeypatch: pytest.MonkeyPatch
) -> tuple[pathlib.Path, pathlib.Path, pathlib.Path, list[str]]:
    """Create CLI inputs for a fake Debian and RPM package."""
    module = _module()
    monkeypatch.setattr(module.shutil, "which", lambda name: f"/usr/bin/{name}")
    dist = tmp_path / "dist"
    dist.mkdir()
    (dist / "netsuke.deb").write_bytes(b"fake Debian package")
    (dist / "netsuke.rpm").write_bytes(b"fake RPM package")

    manifest = tmp_path / "Cargo.toml"
    package = {
        "authors": [VALID_METADATA["maintainer"], "Other Author"],
        "homepage": VALID_METADATA["homepage"],
        "license": VALID_METADATA["license"],
        "description": VALID_METADATA["description"],
    }
    manifest.write_text(
        "[package]\n"
        + "\n".join(f"{key} = {json.dumps(value)}" for key, value in package.items())
        + "\n",
        encoding="utf-8",
    )
    licence_file = tmp_path / "LICENSE"
    licence_file.write_bytes(LICENCE_CONTENTS)
    arguments = [
        "--dist",
        str(dist),
        "--manifest",
        str(manifest),
        "--package-name",
        "netsuke",
        "--license-file",
        str(licence_file),
    ]
    return dist, manifest, licence_file, arguments


@pytest.mark.parametrize(
    "copyright_member_name",
    [
        pytest.param("./usr/share/doc/netsuke/copyright", id="canonical"),
        pytest.param("usr//share/doc/./netsuke/copyright", id="equivalent-spelling"),
    ],
)
def test_main_accepts_matching_headers_and_packaged_debian_copyright(
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
    copyright_member_name: str,
) -> None:
    """Accept independent sources and normalized Debian copyright paths."""
    _, manifest, _, arguments = _prepare_inputs(tmp_path, monkeypatch)
    runner = FakePackageCommands(copyright_member_name=copyright_member_name)

    status = _module().main(arguments, runner=runner)

    captured = capsys.readouterr()
    assert status == 0, "matching package metadata should pass"
    assert captured.out == "ok: Debian and RPM package metadata match Cargo.toml\n", (
        "successful package validation should print its status"
    )
    assert not captured.err, "matching package metadata should not report errors"
    debian_fields = [call[-1] for call in runner.calls if "--field" in call]
    assert debian_fields == ["Maintainer", "Homepage", "Description"], (
        "Debian header queries should cover the three available fields"
    )
    rpm_queries = [call for call in runner.calls if "--queryformat" in call]
    assert [call[call.index("--queryformat") + 1] for call in rpm_queries] == [
        "%{PACKAGER}",
        "%{URL}",
        "%{LICENSE}",
        "%{SUMMARY}",
        "%{DESCRIPTION}",
    ], "RPM metadata should use one query per required tag"

    manifest_contents = manifest.read_text(encoding="utf-8")
    manifest.write_text(
        manifest_contents.replace(
            VALID_METADATA["homepage"], "https://changed.example.test"
        ),
        encoding="utf-8",
    )
    status = _module().main(arguments, runner=runner)
    captured = capsys.readouterr()
    assert status == 1, "changed manifest metadata should fail validation"
    assert (
        "field Homepage does not match: expected 'https://changed.example.test'; "
        "actual 'https://cargo.example.test/linux-package-fixture'"
    ) in captured.err, "package metadata should be compared with the manifest"


def test_main_reports_every_empty_or_differing_package_field(
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Report each empty or mismatching Debian and RPM metadata field."""
    _, _, _, arguments = _prepare_inputs(tmp_path, monkeypatch)
    runner = FakePackageCommands(
        debian_fields={
            "Maintainer": "",
            "Homepage": "wrong",
            "Description": "wrong",
        },
        rpm_tags={
            "PACKAGER": "wrong",
            "URL": "",
            "LICENSE": "MIT",
            "SUMMARY": "wrong",
            "DESCRIPTION": "",
        },
    )

    status = _module().main(arguments, runner=runner)

    captured = capsys.readouterr()
    assert status == 1, "mismatching package metadata should fail"
    metadata = VALID_METADATA
    expected_fields = [
        ("netsuke.deb", "Maintainer", "is empty", metadata["maintainer"], ""),
        ("netsuke.deb", "Homepage", "does not match", metadata["homepage"], "wrong"),
        (
            "netsuke.deb",
            "Description",
            "does not match",
            metadata["description"],
            "wrong",
        ),
        ("netsuke.rpm", "PACKAGER", "does not match", metadata["maintainer"], "wrong"),
        ("netsuke.rpm", "URL", "is empty", metadata["homepage"], ""),
        (
            "netsuke.rpm",
            "LICENSE",
            "does not match",
            metadata["license"],
            "MIT",
        ),
        ("netsuke.rpm", "SUMMARY", "does not match", metadata["description"], "wrong"),
        ("netsuke.rpm", "DESCRIPTION", "is empty", metadata["description"], ""),
    ]
    diagnostic_format = "error: {}: field {} {}: expected {!r}; actual {!r}"
    actual_diagnostics = list(
        itertools.starmap(diagnostic_format.format, expected_fields)
    )
    assert captured.err.splitlines() == actual_diagnostics, (
        "diagnostics should name each empty or mismatching field and its values"
    )


@pytest.mark.parametrize(
    "copyright_contents",
    [
        pytest.param(None, id="missing-copyright"),
        pytest.param(b"different licence", id="different-copyright"),
    ],
)
def test_main_rejects_missing_or_differing_debian_copyright(
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
    copyright_contents: bytes | None,
) -> None:
    """Reject an absent or differing Debian copyright member."""
    _, _, _, arguments = _prepare_inputs(tmp_path, monkeypatch)
    runner = FakePackageCommands(copyright_contents=copyright_contents)

    status = _module().main(arguments, runner=runner)

    captured = capsys.readouterr()
    assert status == 1, "invalid Debian copyright contents should fail"
    outcome = "is missing" if copyright_contents is None else "does not match"
    expected_fingerprint = (
        f"sha256:{hashlib.sha256(LICENCE_CONTENTS).hexdigest()} "
        f"({len(LICENCE_CONTENTS)} bytes)"
    )
    actual_value = (
        "missing"
        if copyright_contents is None
        else (
            f"sha256:{hashlib.sha256(copyright_contents).hexdigest()} "
            f"({len(copyright_contents)} bytes)"
        )
    )
    expected_error = (
        "error: netsuke.deb: field Debian copyright "
        f"{outcome}: expected {expected_fingerprint}; actual {actual_value}\n"
    )
    assert captured.err == expected_error, (
        "copyright diagnostics should report the complete expected and actual values"
    )


@pytest.mark.parametrize(
    "case",
    [
        pytest.param((".deb", 0), id="missing-deb"),
        pytest.param((".rpm", 0), id="missing-rpm"),
        pytest.param((".deb", 1), id="duplicate-deb"),
        pytest.param((".rpm", 1), id="duplicate-rpm"),
    ],
)
def test_main_requires_one_direct_package_of_each_format(
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
    case: tuple[str, int],
) -> None:
    """Reject a missing or duplicate direct package for either format."""
    suffix, extra_count = case
    dist, _, _, arguments = _prepare_inputs(tmp_path, monkeypatch)
    original = dist / f"netsuke{suffix}"
    if extra_count == 0:
        original.unlink()
    else:
        for number in range(extra_count):
            (dist / f"copy-{number}{suffix}").write_bytes(b"extra package")

    status = _module().main(arguments, runner=FakePackageCommands())

    captured = capsys.readouterr()
    assert status == 1, "ambiguous package formats should fail"
    found_count = extra_count if extra_count == 0 else extra_count + 1
    expected_error = (
        f"error: {dist}: expected exactly one direct {suffix} package, "
        f"found {found_count}\n"
    )
    assert captured.err == expected_error, (
        "package-count diagnostics should name the format and matching-file count"
    )


def test_main_rejects_a_symlink_as_the_only_package_file(
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Reject a package symlink even when it is the sole suffix match."""
    dist, _, _, arguments = _prepare_inputs(tmp_path, monkeypatch)
    (dist / "netsuke.deb").unlink()
    (dist / "source.blob").write_bytes(b"not the package")
    (dist / "netsuke.deb").symlink_to("source.blob")

    status = _module().main(arguments, runner=FakePackageCommands())

    captured = capsys.readouterr()
    assert status == 1, "a package symlink should fail validation"
    assert (
        "netsuke.deb: expected a regular, non-symlink package file" in captured.err
    ), "symlink diagnostics should name the regular-file requirement"


@pytest.mark.parametrize("tool", ["dpkg-deb", "rpm"])
def test_main_reports_each_missing_package_inspection_tool(
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
    tool: str,
) -> None:
    """Fail clearly when either package inspection executable is unavailable."""
    _prepare_inputs(tmp_path, monkeypatch)
    module = _module()
    monkeypatch.setattr(
        module.shutil, "which", lambda name: None if name == tool else name
    )

    status = module.main(
        [
            "--dist",
            str(tmp_path / "dist"),
            "--manifest",
            str(tmp_path / "Cargo.toml"),
            "--package-name",
            "netsuke",
            "--license-file",
            str(tmp_path / "LICENSE"),
        ],
        runner=FakePackageCommands(),
    )

    captured = capsys.readouterr()
    assert status == 1, "missing package tooling should fail"
    assert f"tool {tool!r} was not found on PATH" in captured.err, (
        "missing-tool diagnostics should name the required executable"
    )
