"""Exercise the crate-packaging CLI the packaging job invokes.

``cargo package`` is replaced with a stand-in that writes a known file, so the
suite proves the ordering and the recorded digest without compiling anything.
The real command's own verification is not this module's subject: what is
tested here is that the seam refuses a disagreeing identity *before* spending
build time, and that the digest it records is the digest of the file it
produced.
"""

import dataclasses
import hashlib
import json
import pathlib
import typing as typ

import pytest
import release_candidate
from package_crate_cli import (
    EXIT_IDENTITY_REJECTED,
    EXIT_OK,
    EXIT_PACKAGING_FAILED,
    PACKAGE_SUBDIR,
    CratePackage,
    build_parser,
    main,
    package_crate,
)

if typ.TYPE_CHECKING:
    import argparse

COMMIT = "0123456789abcdef0123456789abcdef01234567"
PACKAGE = "netsuke-build"
BIN_NAME = "netsuke"
TAG = "v1.2.3"
VERSION = "1.2.3"
CRATE_NAME = f"{PACKAGE}-{VERSION}.crate"


def _argv(
    destination: pathlib.Path,
    *,
    tag: str = TAG,
    manifest: pathlib.Path = pathlib.Path("Cargo.toml"),
    target_dir: pathlib.Path = pathlib.Path("target"),
) -> list[str]:
    """Return the packaging arguments, overriding the fields under test."""
    return [
        "--tag",
        tag,
        "--commit",
        COMMIT,
        "--package-name",
        PACKAGE,
        "--version",
        VERSION,
        "--bin-name",
        BIN_NAME,
        "--manifest",
        str(manifest),
        "--target-dir",
        str(target_dir),
        "--output",
        str(destination),
    ]


def _record(destination: pathlib.Path) -> dict[str, object]:
    """Read back the package record the CLI wrote."""
    return json.loads(destination.read_text(encoding="utf-8"))


def _parse(argv: list[str]) -> argparse.Namespace:
    """Parse ``argv`` and return the accepted namespace."""
    return build_parser().parse_args(argv)


def _stub_cargo(monkeypatch: pytest.MonkeyPatch, body: bytes) -> list[list[str]]:
    """Replace ``cargo package`` with one that writes ``body`` as the crate."""
    calls: list[list[str]] = []

    def _run(arguments: list[str], **_kwargs: object) -> object:
        """Record the invocation and stage the crate it would have produced."""
        calls.append(arguments)
        target_dir = pathlib.Path(arguments[arguments.index("--target-dir") + 1])
        staging = target_dir / PACKAGE_SUBDIR
        staging.mkdir(parents=True, exist_ok=True)
        (staging / CRATE_NAME).write_bytes(body)
        return _Completed(0, "")

    monkeypatch.setattr("package_crate_cli.subprocess.run", _run)
    return calls


@dataclasses.dataclass(frozen=True, slots=True)
class _Completed:
    """Stand in for ``subprocess.CompletedProcess`` with the fields read."""

    returncode: int
    stderr: str


def test_the_cli_records_the_crate_and_its_digest(
    tmp_path: pathlib.Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Record the file name and the digest of the bytes cargo produced."""
    destination = tmp_path / "package.json"
    body = b"netsuke crate bytes"
    _ = _stub_cargo(monkeypatch, body)
    target_dir = tmp_path / "target"

    status = main(_argv(destination, target_dir=target_dir))

    assert status == EXIT_OK, f"packaging must succeed, got {status}"
    record = _record(destination)
    assert record["crate"] == CRATE_NAME, "the record must name the crate"
    assert record["sha256"] == _expected_digest(body), (
        "the record must carry the digest of the bytes cargo produced"
    )


def test_the_cli_nests_the_identity_beside_the_digest(
    tmp_path: pathlib.Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Carry the whole identity forward, so later stages need no second source."""
    destination = tmp_path / "package.json"
    _ = _stub_cargo(monkeypatch, b"netsuke")
    target_dir = tmp_path / "target"

    status = main(_argv(destination, target_dir=target_dir))

    assert status == EXIT_OK, f"packaging must succeed, got {status}"
    identity = _record(destination)["identity"]
    assert isinstance(identity, dict), "the identity must be recorded as a mapping"
    assert identity["tag"] == TAG, "the identity must carry the tag"
    assert identity["commit"] == COMMIT, "the identity must carry the commit"
    assert identity["version"] == VERSION, "the identity must carry the version"
    assert identity["is_prerelease"] is False, "a plain release is not a prerelease"


def test_the_cli_records_a_prerelease_identity(
    tmp_path: pathlib.Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Carry the prerelease flag through, since the registry treats it differently."""
    destination = tmp_path / "package.json"
    _ = _stub_cargo(monkeypatch, b"netsuke")
    prerelease = "1.2.3-beta1"
    argv = _argv(destination, tag=f"v{prerelease}", target_dir=tmp_path / "target")
    argv[argv.index("--version") + 1] = prerelease

    status = main(argv)

    assert status == EXIT_OK, f"a prerelease must be packaged, got {status}"
    identity = _record(destination)["identity"]
    assert isinstance(identity, dict), "the identity must be recorded as a mapping"
    assert identity["is_prerelease"] is True, (
        "a prerelease version must be recorded as a prerelease"
    )


def test_the_cli_rejects_a_disagreeing_tag_before_cargo_runs(
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Spend no build time on a candidate the rules already refused."""
    destination = tmp_path / "package.json"
    calls = _stub_cargo(monkeypatch, b"netsuke")

    status = main(_argv(destination, tag="v2.0.0", target_dir=tmp_path / "target"))

    assert status == EXIT_IDENTITY_REJECTED, (
        f"a disagreeing tag must be refused, got {status}"
    )
    assert not calls, "the identity must be resolved before cargo is invoked"
    assert not destination.exists(), "a rejected identity must leave no record"
    assert "v2.0.0" in capsys.readouterr().err, "the diagnostic must name the tag"


def test_the_cli_reports_a_failed_packaging_run(
    tmp_path: pathlib.Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Fail with the packaging code, and write no record for bytes that do not exist."""

    def _fail(_arguments: list[str], **_kwargs: object) -> object:
        """Stand in for a cargo run that reports a compile failure."""
        return _Completed(101, "error: failed to package")

    monkeypatch.setattr("package_crate_cli.subprocess.run", _fail)
    destination = tmp_path / "package.json"

    status = main(_argv(destination, target_dir=tmp_path / "target"))

    assert status == EXIT_PACKAGING_FAILED, (
        f"a failed cargo run must report as packaging failure, got {status}"
    )
    assert not destination.exists(), "a failed run must leave no record"


def test_the_packager_refuses_a_destination_holding_a_stale_crate(
    tmp_path: pathlib.Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Refuse ambiguity rather than publishing whichever file sorted first.

    A stale ``.crate`` left from an earlier run has a digest that belongs to
    different bytes. Publishing it under this run's identity is exactly what
    the digest record exists to prevent, so two candidates are an error.
    """
    target_dir = tmp_path / "target"
    _ = _stub_cargo(monkeypatch, b"fresh")
    stale = target_dir / PACKAGE_SUBDIR / "netsuke-build-0.0.1.crate"
    stale.parent.mkdir(parents=True, exist_ok=True)
    _ = stale.write_bytes(b"stale")

    with pytest.raises(RuntimeError, match="exactly one"):
        _ = package_crate(pathlib.Path("Cargo.toml"), target_dir)


def test_the_packager_reads_the_cargo_staging_subdirectory(
    tmp_path: pathlib.Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Read the crate from where ``cargo package`` actually writes it.

    ``cargo package --target-dir`` nests its output: the archive lands in
    ``<target-dir>/package/``, beside the ``tmp-crate/`` tree verification
    unpacks into. A reader that globbed the target directory itself would find
    nothing and report a successful run as a failure, which is the defect this
    pins. The stub writes only to the nested path, so a reader that looked at
    the root would raise instead of returning the crate.
    """
    target_dir = tmp_path / "target"
    body = b"nested crate bytes"
    _ = _stub_cargo(monkeypatch, body)

    crate = package_crate(pathlib.Path("Cargo.toml"), target_dir)

    assert crate == target_dir / PACKAGE_SUBDIR / CRATE_NAME, (
        "the packager must read the crate from cargo's staging subdirectory"
    )
    assert crate.read_bytes() == body, "the packager must return the staged bytes"


def test_the_packager_reports_cargo_stderr_on_failure(
    tmp_path: pathlib.Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Carry cargo's own diagnosis, so the operator sees the real cause."""

    def _fail(_arguments: list[str], **_kwargs: object) -> object:
        """Stand in for a cargo run whose own diagnosis must reach the operator."""
        return _Completed(101, "error: package metadata is missing")

    monkeypatch.setattr("package_crate_cli.subprocess.run", _fail)

    with pytest.raises(RuntimeError, match="package metadata is missing"):
        _ = package_crate(pathlib.Path("Cargo.toml"), tmp_path / "target")


def test_the_record_is_stable_across_runs(tmp_path: pathlib.Path) -> None:
    """Render the same package identically, so two records diff only by content."""
    identity = _record_identity()
    package = CratePackage(identity, pathlib.Path("p.crate"), "ab")

    first = json.dumps(package.as_record(), indent=2, sort_keys=True)
    second = json.dumps(package.as_record(), indent=2, sort_keys=True)

    assert first == second, "the record rendering must be deterministic"


def test_the_cli_exposes_every_required_argument() -> None:
    """Name each input explicitly, so a missing one is refused at parse time."""
    chosen = pathlib.Path("package.json")
    target_dir = pathlib.Path("target")

    parsed = _parse(_argv(chosen, target_dir=target_dir))

    assert parsed.output == chosen, "the caller must choose where the record lands"
    assert parsed.target_dir == target_dir, "the caller must choose the target dir"
    assert parsed.manifest == pathlib.Path("Cargo.toml"), (
        "the caller must name the manifest"
    )


def test_the_cli_requires_a_target_directory() -> None:
    """Refuse to guess where cargo writes, which would scatter build output."""
    with pytest.raises(SystemExit):
        _ = _parse(["--tag", TAG])


def _expected_digest(body: bytes) -> str:
    """Return the digest of ``body`` as the digest module computes it."""
    return hashlib.sha256(body).hexdigest()


def _record_identity() -> release_candidate.ReleaseIdentity:
    """Return a valid identity for record-rendering tests."""
    return release_candidate.resolve_identity(
        release_candidate.CandidateFacts(
            tag=TAG,
            commit=COMMIT,
            package_name=PACKAGE,
            version=VERSION,
            bin_name=BIN_NAME,
        )
    )
