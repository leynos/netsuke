"""Exercise the producer provenance recorder against real staged bytes.

The policy module is proved separately. These tests pin the adapter the build
lane runs: which bytes it hashes, which names it refuses, and what it leaves on
disk when it refuses. Every case uses a real temporary directory so a symlink,
a missing package, or a clobbered output is the thing actually being tested.

Run via ``make test-release-publication``.
"""

import hashlib
import json
import os
import typing as typ

from record_producer_provenance_cli import (
    EXIT_OK,
    EXIT_OUTPUT_UNWRITABLE,
    EXIT_UNUSABLE_INPUT,
    RECORD_NAME,
    main,
)

if typ.TYPE_CHECKING:
    import pathlib

    import pytest

COMMIT = "0123456789abcdef0123456789abcdef01234567"


def _argv(
    destination: pathlib.Path,
    assets: list[pathlib.Path],
    *,
    commit: str = COMMIT,
    run_id: str = "1234",
    run_attempt: str = "1",
    target: str = "linux-x86_64",
    platform: str = "linux",
) -> list[str]:
    """Return a complete argument list naming ``assets``."""
    argv = [
        "--commit",
        commit,
        "--run-id",
        run_id,
        "--run-attempt",
        run_attempt,
        "--target",
        target,
        "--platform",
        platform,
    ]
    for asset in assets:
        argv.extend(["--asset", str(asset)])
    argv.extend(["--output", str(destination)])
    return argv


def _staged_linux(directory: pathlib.Path) -> list[pathlib.Path]:
    """Write one ``.deb`` and one ``.rpm``, the formats linux must record."""
    deb = directory / "netsuke_1.2.3-1_amd64.deb"
    rpm = directory / "netsuke-1.2.3-1.x86_64.rpm"
    _ = deb.write_bytes(b"deb bytes")
    _ = rpm.write_bytes(b"rpm bytes")
    return [deb, rpm]


def _record(destination: pathlib.Path) -> dict[str, object]:
    """Read back the record the CLI wrote."""
    return json.loads(destination.read_text(encoding="utf-8"))


def test_the_cli_records_the_candidate_and_every_staged_digest(
    tmp_path: pathlib.Path,
) -> None:
    """Write the identity and one digest per staged artefact."""
    destination = tmp_path / "provenance.json"
    deb, rpm = _staged_linux(tmp_path)

    status = main(_argv(destination, [deb, rpm]))

    assert status == EXIT_OK, (
        f"a complete producer record must be written, got {status}"
    )
    record = _record(destination)
    assert record["commit"] == COMMIT, "the record must name the candidate"
    assert record["run_id"] == "1234", "the record must name the run"
    assert record["run_attempt"] == "1", "the record must name the attempt"
    assert record["target"] == "linux-x86_64", "the record must name the target"
    assert record["platform"] == "linux", "the record must name the platform"
    assets = record["assets"]
    assert isinstance(assets, dict), "the record must carry a digest table"
    assert set(assets) == {deb.name, rpm.name}, (
        "the record must name exactly the artefacts the producer staged"
    )
    assert assets[deb.name] != assets[rpm.name], (
        "each artefact must carry its own digest"
    )


def test_the_recorded_digest_is_the_digest_of_the_bytes(
    tmp_path: pathlib.Path,
) -> None:
    """Hash the bytes a consumer will download, not the path or the metadata."""
    destination = tmp_path / "provenance.json"
    deb, rpm = _staged_linux(tmp_path)
    expected = hashlib.sha256(deb.read_bytes()).hexdigest()

    status = main(_argv(destination, [deb, rpm]))

    assert status == EXIT_OK, f"the record must be written, got {status}"
    assets = _record(destination)["assets"]
    assert isinstance(assets, dict), "the record must carry a digest table"
    assert assets[deb.name] == expected, (
        "the recorded digest must be the SHA-256 of the staged bytes"
    )


def test_a_package_the_platform_owes_but_lacks_is_refused(
    tmp_path: pathlib.Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Refuse a linux producer that recorded only one of its two formats."""
    destination = tmp_path / "provenance.json"
    deb, _rpm = _staged_linux(tmp_path)

    status = main(_argv(destination, [deb]))

    assert status == EXIT_UNUSABLE_INPUT, (
        f"a missing format must be refused, got {status}"
    )
    assert not destination.exists(), "a refused producer must leave no record"
    assert "rpm" in capsys.readouterr().err, "the diagnosis must name the format"


def test_two_packages_of_one_format_are_refused(tmp_path: pathlib.Path) -> None:
    """Refuse a producer that cannot say which package it built."""
    destination = tmp_path / "provenance.json"
    deb, rpm = _staged_linux(tmp_path)
    second_deb = tmp_path / "netsuke-1.2.3-1-x86_64.deb"
    _ = second_deb.write_bytes(b"second deb")

    status = main(_argv(destination, [deb, rpm, second_deb]))

    assert status == EXIT_UNUSABLE_INPUT, (
        f"a duplicated format must be refused, got {status}"
    )
    assert not destination.exists(), "a refused producer must leave no record"


def test_a_macos_producer_records_its_package(tmp_path: pathlib.Path) -> None:
    """Accept a platform whose single format is the one package staged."""
    destination = tmp_path / "provenance.json"
    package = tmp_path / "netsuke-1.2.3.pkg"
    _ = package.write_bytes(b"pkg bytes")

    status = main(_argv(destination, [package], target="macos-arm64", platform="macos"))

    assert status == EXIT_OK, f"a macos package must be recorded, got {status}"
    assets = _record(destination)["assets"]
    assert isinstance(assets, dict), "the record must carry a digest table"
    assert set(assets) == {package.name}, "the package name must be the asset name"


def test_a_windows_producer_records_its_installer(tmp_path: pathlib.Path) -> None:
    """Accept the one format Windows stages."""
    destination = tmp_path / "provenance.json"
    installer = tmp_path / "netsuke-1.2.3-x86_64.msi"
    _ = installer.write_bytes(b"msi bytes")

    status = main(
        _argv(destination, [installer], target="windows-x86_64", platform="windows")
    )

    assert status == EXIT_OK, f"an msi must be recorded, got {status}"


def test_a_missing_artefact_is_refused(tmp_path: pathlib.Path) -> None:
    """Refuse to hash a path the producer did not stage."""
    destination = tmp_path / "provenance.json"

    status = main(_argv(destination, [tmp_path / "netsuke_1_amd64.deb"]))

    assert status == EXIT_UNUSABLE_INPUT, (
        f"a missing path must be refused, got {status}"
    )
    assert not destination.exists(), "a refused producer must leave no record"


def test_a_symlinked_artefact_is_refused(tmp_path: pathlib.Path) -> None:
    """Refuse bytes that are not in the directory the producer is handing on."""
    destination = tmp_path / "provenance.json"
    deb, rpm = _staged_linux(tmp_path)
    outside = tmp_path / "outside"
    outside.mkdir()
    target = outside / "netsuke_1.2.3-1_amd64.deb"
    _ = target.write_bytes(b"elsewhere")
    _ = deb.unlink()
    deb.symlink_to(target)

    status = main(_argv(destination, [deb, rpm]))

    assert status == EXIT_UNUSABLE_INPUT, (
        f"a symlinked artefact must be refused, got {status}"
    )
    assert not destination.exists(), "a refused producer must leave no record"


def test_a_directory_named_like_a_package_is_refused(tmp_path: pathlib.Path) -> None:
    """Refuse a path that is not a regular file, whatever its suffix says."""
    destination = tmp_path / "provenance.json"
    deb, rpm = _staged_linux(tmp_path)
    _ = deb.unlink()
    deb.mkdir()

    status = main(_argv(destination, [deb, rpm]))

    assert status == EXIT_UNUSABLE_INPUT, f"a directory must be refused, got {status}"


def test_a_fifo_named_like_a_package_is_refused(tmp_path: pathlib.Path) -> None:
    """Refuse a special file, which has no stable bytes to vouch for."""
    destination = tmp_path / "provenance.json"
    deb, rpm = _staged_linux(tmp_path)
    _ = deb.unlink()
    os.mkfifo(deb)

    status = main(_argv(destination, [deb, rpm]))

    assert status == EXIT_UNUSABLE_INPUT, f"a fifo must be refused, got {status}"
    assert not destination.exists(), "a refused producer must leave no record"


def test_a_short_commit_is_refused(tmp_path: pathlib.Path) -> None:
    """Refuse an identity that cannot name one commit unambiguously."""
    destination = tmp_path / "provenance.json"
    deb, rpm = _staged_linux(tmp_path)

    status = main(_argv(destination, [deb, rpm], commit=COMMIT[:12]))

    assert status == EXIT_UNUSABLE_INPUT, (
        f"a short commit must be refused, got {status}"
    )
    assert not destination.exists(), "a refused producer must leave no record"


def test_an_unpackageable_platform_is_refused(tmp_path: pathlib.Path) -> None:
    """Refuse a platform whose package names this release cannot state."""
    destination = tmp_path / "provenance.json"
    package = tmp_path / "netsuke-1.2.3.pkg"
    _ = package.write_bytes(b"pkg bytes")

    status = main(_argv(destination, [package], platform="solaris"))

    assert status == EXIT_UNUSABLE_INPUT, (
        f"an unknown platform must be refused, got {status}"
    )


def test_two_paths_publishing_under_one_name_are_refused(
    tmp_path: pathlib.Path,
) -> None:
    """Refuse two artefacts the release would publish as one asset."""
    destination = tmp_path / "provenance.json"
    deb, rpm = _staged_linux(tmp_path)
    duplicate_dir = tmp_path / "nested" / "netsuke_1.2.3-1_amd64.deb"
    duplicate_dir.parent.mkdir()
    _ = duplicate_dir.write_bytes(b"duplicate name")

    status = main(_argv(destination, [deb, rpm, duplicate_dir]))

    assert status == EXIT_UNUSABLE_INPUT, (
        f"a duplicated published name must be refused, got {status}"
    )


def test_the_record_is_written_atomically(tmp_path: pathlib.Path) -> None:
    """Install the record whole, leaving no staging directory behind."""
    destination = tmp_path / "provenance.json"
    deb, rpm = _staged_linux(tmp_path)

    status = main(_argv(destination, [deb, rpm]))

    assert status == EXIT_OK, f"the record must be written, got {status}"
    leftovers = [
        entry.name for entry in tmp_path.iterdir() if entry.name != destination.name
    ]
    assert all(not name.startswith(".provenance.json") for name in leftovers), (
        "an atomic write must clean up its staging directory"
    )


def test_a_previous_record_survives_a_refused_run(tmp_path: pathlib.Path) -> None:
    """Leave the previous record intact when this run cannot replace it."""
    destination = tmp_path / "provenance.json"
    deb, rpm = _staged_linux(tmp_path)
    required = main(_argv(destination, [deb, rpm]))
    original = destination.read_bytes()

    status = main(_argv(destination, [deb], run_attempt="2"))

    assert required == EXIT_OK, "the fixture must start from a written record"
    assert status == EXIT_UNUSABLE_INPUT, (
        f"the second run must be refused, got {status}"
    )
    assert destination.read_bytes() == original, (
        "a refused run must not disturb the record it could not replace"
    )


def test_an_unwritable_destination_is_reported_separately(
    tmp_path: pathlib.Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Distinguish an unwritable output from an artefact this producer rejects."""
    destination = tmp_path / "absent-directory" / "provenance.json"
    deb, rpm = _staged_linux(tmp_path)

    status = main(_argv(destination, [deb, rpm]))

    assert status == EXIT_OUTPUT_UNWRITABLE, (
        f"an unwritable destination must be reported, got {status}"
    )
    assert "output" in capsys.readouterr().err, (
        "the diagnosis must say the output could not be written"
    )


def test_the_record_name_is_stable() -> None:
    """Keep the hand-off file name stable for the publication lane."""
    assert RECORD_NAME == "producer-provenance.json", (
        "publication discovers the record by this name"
    )
