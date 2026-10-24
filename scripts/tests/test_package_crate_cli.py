"""Exercise the crate-packaging CLI the packaging job invokes.

``cargo package`` is replaced with a stand-in that writes a known file, so the
suite proves the ordering and the recorded digest without compiling anything.
The real command's own verification is not this module's subject: what is
tested here is that the seam refuses a disagreeing identity *before* spending
build time, and that the digest it records is the digest of the file it
produced.

The stand-in and the argument shapes live in :mod:`package_crate_support`, so
this module holds only behaviour and stays inside the repository's 400-line
cap. Both modules are shipped together by ``make test-release-publication``.
"""

import hashlib
import json
import pathlib
import subprocess  # ruff: ignore[suspicious-subprocess-import] - the stub under test.

import pytest
from package_crate_cli import (
    EXIT_IDENTITY_REJECTED,
    EXIT_OK,
    EXIT_PACKAGING_FAILED,
    PACKAGE_SUBDIR,
    PACKAGING_TIMEOUT_SECONDS,
    CratePackage,
    main,
    package_crate,
)
from package_crate_support import (
    COMMIT,
    CRATE_NAME,
    TAG,
    VERSION,
    completed,
    packaging_argv,
    parse_argv,
    read_record,
    record_identity,
    stub_cargo,
    stub_runner,
    tagless_argv,
)


def test_the_cli_records_the_crate_and_its_digest(
    tmp_path: pathlib.Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Record the file name and the digest of the bytes cargo produced."""
    destination = tmp_path / "package.json"
    body = b"netsuke crate bytes"
    _ = stub_cargo(monkeypatch, body)

    status = main(packaging_argv(destination, target_dir=tmp_path / "target"))

    assert status == EXIT_OK, f"packaging must succeed, got {status}"
    record = read_record(destination)
    assert record["crate"] == CRATE_NAME, "the record must name the crate"
    assert record["sha256"] == hashlib.sha256(body).hexdigest(), (
        "the record must carry the digest of the bytes cargo produced"
    )


def test_the_cli_nests_the_identity_beside_the_digest(
    tmp_path: pathlib.Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Carry the whole identity forward, so later stages need no second source."""
    destination = tmp_path / "package.json"
    _ = stub_cargo(monkeypatch, b"netsuke")

    status = main(packaging_argv(destination, target_dir=tmp_path / "target"))

    assert status == EXIT_OK, f"packaging must succeed, got {status}"
    identity = read_record(destination)["identity"]
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
    _ = stub_cargo(monkeypatch, b"netsuke")
    prerelease = "1.2.3-beta1"
    argv = packaging_argv(
        destination, tag=f"v{prerelease}", target_dir=tmp_path / "target"
    )
    argv[argv.index("--version") + 1] = prerelease

    status = main(argv)

    assert status == EXIT_OK, f"a prerelease must be packaged, got {status}"
    identity = read_record(destination)["identity"]
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
    calls = stub_cargo(monkeypatch, b"netsuke")

    status = main(
        packaging_argv(destination, tag="v2.0.0", target_dir=tmp_path / "target")
    )

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

    def _fail(
        _arguments: list[str], **_kwargs: object
    ) -> subprocess.CompletedProcess[str]:
        """Stand in for a cargo run that reports a compile failure."""
        return completed(101, "error: failed to package")

    monkeypatch.setattr("package_crate_cli.subprocess.run", _fail)
    destination = tmp_path / "package.json"

    status = main(packaging_argv(destination, target_dir=tmp_path / "target"))

    assert status == EXIT_PACKAGING_FAILED, (
        f"a failed cargo run must report as packaging failure, got {status}"
    )
    assert not destination.exists(), "a failed run must leave no record"


def test_the_packager_refuses_a_destination_holding_a_stale_crate(
    tmp_path: pathlib.Path,
) -> None:
    """Refuse ambiguity rather than publishing whichever file sorted first.

    A stale ``.crate`` has a digest belonging to different bytes; publishing it
    under this run's identity is what the digest record exists to prevent.
    """
    target_dir = tmp_path / "target"
    calls: list[list[str]] = []
    stale = target_dir / PACKAGE_SUBDIR / "netsuke-build-0.0.1.crate"
    stale.parent.mkdir(parents=True, exist_ok=True)
    _ = stale.write_bytes(b"stale")

    with pytest.raises(RuntimeError, match="exactly one"):
        _ = package_crate(
            pathlib.Path("Cargo.toml"),
            target_dir,
            runner=stub_runner(b"fresh", calls),
        )


def test_the_packager_reads_the_cargo_staging_subdirectory(
    tmp_path: pathlib.Path,
) -> None:
    """Read the crate from where ``cargo package`` actually writes it.

    ``--target-dir`` nests its output: the archive lands in
    ``<target-dir>/package/``, beside the ``tmp-crate/`` tree verification
    unpacks into. A reader globbing the target directory itself would find
    nothing and report a successful run as a failure. The stub writes only to
    the nested path, so a reader that looked at the root would raise.
    """
    target_dir = tmp_path / "target"
    body = b"nested crate bytes"
    calls: list[list[str]] = []

    crate = package_crate(
        pathlib.Path("Cargo.toml"),
        target_dir,
        runner=stub_runner(body, calls),
    )

    assert crate == target_dir / PACKAGE_SUBDIR / CRATE_NAME, (
        "the packager must read the crate from cargo's staging subdirectory"
    )
    assert crate.read_bytes() == body, "the packager must return the staged bytes"


def test_the_packager_reports_cargo_stderr_on_failure(
    tmp_path: pathlib.Path,
) -> None:
    """Carry cargo's own diagnosis, so the operator sees the real cause."""

    def _fail(
        _arguments: list[str], **_kwargs: object
    ) -> subprocess.CompletedProcess[str]:
        """Stand in for a cargo run whose own diagnosis must reach the operator."""
        return completed(101, "error: package metadata is missing")

    with pytest.raises(RuntimeError, match="package metadata is missing"):
        _ = package_crate(pathlib.Path("Cargo.toml"), tmp_path / "target", runner=_fail)


def test_the_packager_reports_an_unlaunchable_cargo_as_a_packaging_failure(
    tmp_path: pathlib.Path,
) -> None:
    """Translate a failed spawn, so a PATH fault is a message and not a traceback.

    ``cargo`` missing from the workflow PATH raises ``OSError`` from the spawn
    itself, before any exit code exists. Nothing in the module had a branch for
    that case, so it escaped past :func:`main` and surfaced as an unhandled
    exception from a CLI whose contract is a code and a diagnosis.
    """

    def _unlaunchable(
        _arguments: list[str], **_kwargs: object
    ) -> subprocess.CompletedProcess[str]:
        """Stand in for a spawn that never reached the child process."""
        message = "[Errno 2] No such file or directory: 'cargo'"
        raise FileNotFoundError(message)

    with pytest.raises(RuntimeError, match="could not be launched"):
        _ = package_crate(
            pathlib.Path("Cargo.toml"), tmp_path / "target", runner=_unlaunchable
        )


def test_the_cli_bounds_cargo_and_reports_a_timeout(
    tmp_path: pathlib.Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Bound the command, and turn an expiry into a packaging failure.

    Without a ``timeout`` a stuck cargo holds the job until the runner kills
    it, and the log reports a step timeout with no cause. Asserted twice over:
    the bound reaches the call, and an expiry becomes this module's exit code
    rather than a ``TimeoutExpired`` escaping past its handling.
    """
    seen: dict[str, object] = {}

    def _hang(_arguments: list[str], **kwargs: object) -> object:
        """Record the call's keyword arguments, then stand in for a hang."""
        seen.update(kwargs)
        raise subprocess.TimeoutExpired(cmd="cargo package", timeout=1)

    monkeypatch.setattr("package_crate_cli.subprocess.run", _hang)
    destination = tmp_path / "package.json"

    status = main(packaging_argv(destination, target_dir=tmp_path / "target"))

    assert seen.get("timeout") == PACKAGING_TIMEOUT_SECONDS, (
        f"cargo must be given a wall-clock bound, got {seen.get('timeout')!r}"
    )
    assert status == EXIT_PACKAGING_FAILED, (
        f"a timed-out packaging run must report as packaging failure, got {status}"
    )
    assert not destination.exists(), "a timed-out run must leave no record"


def test_the_record_is_stable_across_runs() -> None:
    """Render the same package identically, so two records diff only by content."""
    package = CratePackage(record_identity(), pathlib.Path("p.crate"), "ab")

    first = json.dumps(package.as_record(), indent=2, sort_keys=True)
    second = json.dumps(package.as_record(), indent=2, sort_keys=True)

    assert first == second, "the record rendering must be deterministic"


def test_the_cli_exposes_every_required_argument() -> None:
    """Name each input explicitly, so a missing one is refused at parse time."""
    chosen = pathlib.Path("package.json")
    target_dir = pathlib.Path("target")

    parsed = parse_argv(packaging_argv(chosen, target_dir=target_dir))

    assert parsed.output == chosen, "the caller must choose where the record lands"
    assert parsed.target_dir == target_dir, "the caller must choose the target dir"
    assert parsed.manifest == pathlib.Path("Cargo.toml"), (
        "the caller must name the manifest"
    )


def test_the_cli_requires_a_target_directory() -> None:
    """Refuse to guess where cargo writes, which would scatter build output."""
    with pytest.raises(SystemExit):
        _ = parse_argv(["--tag", TAG])


def test_the_cli_records_a_rehearsal_with_no_tag(
    tmp_path: pathlib.Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Record a tagless identity: the pull-request lane has no tag to record.

    ``github.ref_name`` there is a merge ref, so a caller forwarding it would
    fail the tag validator. ``--dry-run`` records the absence instead, and the
    archive is still built and digested — what the rehearsal exists to prove.
    """
    destination = tmp_path / "package.json"
    body = b"rehearsed crate bytes"
    _ = stub_cargo(monkeypatch, body)
    status = main(tagless_argv(destination, rehearsal=True))

    assert status == EXIT_OK, f"a rehearsal must package, got {status}"
    record = read_record(destination)
    assert record["crate"] == CRATE_NAME, "a rehearsal must still name the crate"
    assert record["sha256"] == hashlib.sha256(body).hexdigest(), (
        "a rehearsal must still record the digest of the bytes it produced"
    )
    identity = record["identity"]
    assert isinstance(identity, dict), "the identity must be recorded as a mapping"
    assert identity["tag"] is None, "a rehearsal must record the absence of a tag"


@pytest.mark.parametrize(
    "selector",
    [
        pytest.param("both", id="both"),
        pytest.param("neither", id="neither"),
    ],
)
def test_the_cli_requires_exactly_one_selector(
    tmp_path: pathlib.Path, selector: str
) -> None:
    """Refuse an ambiguous or mode-less call rather than assuming one.

    Two failures share a rule. A publishing call site that kept its ``--tag``
    and also set the rehearsal flag would record an identity with no tag, and
    the publication stage would have nothing to publish under. In the other
    direction, absence of a tag is what a rehearsal *records*, so a publishing
    caller that forgot its tag must not be silently reinterpreted as one.
    """
    destination = tmp_path / "package.json"
    argv = (
        ["--dry-run", *packaging_argv(destination)]
        if selector == "both"
        else tagless_argv(destination, rehearsal=False)
    )

    with pytest.raises(SystemExit):
        _ = parse_argv(argv)
