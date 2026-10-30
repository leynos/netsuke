"""Package the release crate and record the identity of the bytes produced.

Publication, inventory, and consumer smoke compare against this record's
archive digest; recording it once keeps their claims aligned.

Identity policy stays in :mod:`release_candidate`, digest I/O in
:mod:`release_digest_files`; this subprocess seam needs no network.

A pull request's ``github.ref_name`` is a merge ref such as ``903/merge``.
The caller therefore passes :option:`--dry-run`, which records ``tag: null``.
Requiring exactly one of :option:`--tag` and :option:`--dry-run` stops a
publishing call that forgot its tag from silently producing a tagless identity.
"""

import argparse
import collections.abc as cabc
import contextlib
import dataclasses
import json
import pathlib
import subprocess  # ruff: ignore[suspicious-subprocess-import] - packaging boundary.
import sys
import tempfile

import release_candidate
import release_digest_files

#: The shape of ``subprocess.run`` this module needs. Declared so a test can
#: supply its own runner instead of patching the global ``subprocess`` module,
#: which keeps the failure modes of *launching* a command reachable without
#: actually launching one.
type SubprocessRunner = cabc.Callable[..., subprocess.CompletedProcess[str]]

EXIT_OK = 0
EXIT_PACKAGING_FAILED = 1
EXIT_IDENTITY_REJECTED = 2
EXIT_ARTIFACT_ERROR = 3

#: Wall-clock seconds ``cargo package`` may run before it is killed.
#:
#: Without a bound, a cargo invocation that waits on a lock or on a hung
#: network fetch holds the job until the runner's own limit kills it, and the
#: job's log says only that the step timed out — no crate, no diagnosis, and no
#: indication of which command was stuck. The packaging step itself is minutes,
#: not hours: it compiles the packaged sources once inside
#: ``<target>/package/tmp-crate/``. Fifteen minutes leaves generous room for a
#: cold cache while still failing with a message rather than a runner eviction.
PACKAGING_TIMEOUT_SECONDS = 15 * 60

#: Subdirectory of ``cargo package --target-dir`` the finished archive lands
#: in. Cargo writes the crate here and unpacks its verification copy into the
#: sibling ``tmp-crate/`` tree, so this is the only place a caller may look for
#: exactly one crate.
PACKAGE_SUBDIR = "package"


@dataclasses.dataclass(frozen=True, slots=True)
class CratePackage:
    """Record the archive one packaging run produced.

    Parameters
    ----------
    identity
        The candidate the archive was built from.
    crate
        Path to the ``.crate`` file.
    sha256
        Lowercase hexadecimal digest of the crate's bytes.
    """

    identity: release_candidate.ReleaseIdentity
    crate: pathlib.Path
    sha256: str

    def as_record(self) -> dict[str, object]:
        """Return the package as a JSON-serializable mapping with sorted keys.

        Returns
        -------
        dict
            The identity record nested under ``identity``, beside the crate's
            name and digest.

        Examples
        --------
        >>> identity = release_candidate.ReleaseIdentity(
        ...     "v1.2.3", "0" * 40, "p", "1.2.3", "b", False
        ... )
        >>> package = CratePackage(identity, pathlib.Path("p.crate"), "ab")
        >>> sorted(package.as_record())
        ['crate', 'identity', 'sha256']
        """
        return {
            "crate": self.crate.name,
            "identity": self.identity.as_record(),
            "sha256": self.sha256,
        }


def _write_package_record(output: pathlib.Path, package: CratePackage) -> None:
    """Atomically write the package identity and digest record.

    The temporary file is a sibling of ``output``, so replacement stays on
    one filesystem and readers never observe a partial JSON record.

    Raises
    ------
    OSError
        If the temporary record cannot be written, removed, or replaced.

    Examples
    --------
    >>> identity = release_candidate.ReleaseIdentity("v1.2.3", "0" * 40,
    ...     "netsuke-build", "1.2.3", "netsuke", False)
    >>> package = CratePackage(identity, pathlib.Path("x.crate"), "ab")
    >>> with tempfile.TemporaryDirectory() as directory:
    ...     output = pathlib.Path(directory) / "package.json"
    ...     _write_package_record(output, package)
    ...     json.loads(output.read_text(encoding="utf-8"))["crate"]
    'x.crate'
    """
    temporary_path: pathlib.Path | None = None
    try:
        with tempfile.NamedTemporaryFile(
            mode="w",
            encoding="utf-8",
            dir=output.parent,
            prefix=f".{output.name}.",
            suffix=".tmp",
            delete=False,
        ) as temporary:
            temporary_path = pathlib.Path(temporary.name)
            _ = temporary.write(
                json.dumps(package.as_record(), indent=2, sort_keys=True) + "\n"
            )
        temporary_path.replace(output)
    except OSError:
        if temporary_path is not None:
            with contextlib.suppress(OSError):
                temporary_path.unlink(missing_ok=True)
        raise


def package_crate(
    manifest: pathlib.Path,
    destination: pathlib.Path,
    *,
    runner: SubprocessRunner,
) -> pathlib.Path:
    """Build the ``.crate`` for the package the manifest declares.

    Parameters
    ----------
    manifest
        Path to the Cargo manifest naming the package.
    destination
        Target directory ``cargo package`` is given.
    runner
        The callable that launches the command. The CLI edge supplies
        :func:`subprocess.run`; a test supplies its own so the packaging
        boundary can be exercised without compiling anything. Required rather
        than defaulted, so the one place this module reaches the process table
        is :func:`main`.

    Returns
    -------
    pathlib.Path
        The single ``.crate`` file the command produced.

    Notes
    -----
    ``--no-verify`` is deliberately not passed: the command's own verification
    compiles the packaged sources, which is the check that the archive builds
    from exactly the files it carries rather than from the working tree.

    The crate is read from :data:`PACKAGE_SUBDIR`, not from ``destination``
    itself, because ``cargo package --target-dir`` nests its output one level
    down. Measured on the pinned toolchain at the 2026-10-20 probe, a bare
    ``glob("*.crate")`` over the target directory returns nothing, so a caller
    that looked there would report a successful run as a failure.

    Exactly one crate is required rather than "take the first": a stale file
    from an earlier run would otherwise be published under this run's
    identity, which is the failure the digest record exists to prevent.

    Both helpers raise ``RuntimeError`` — if ``cargo package`` cannot be
    launched, exits non-zero, exceeds :data:`PACKAGING_TIMEOUT_SECONDS`, or if
    the directory does not hold exactly one crate afterwards — so :func:`main`
    translates them into ``EXIT_PACKAGING_FAILED``. A ``TimeoutExpired`` or an
    ``OSError`` escaping instead would surface as a traceback from a module
    whose whole contract is "exits with a code and a message".
    """
    _run_package(manifest, destination, runner=runner)
    return _the_only_crate(destination / PACKAGE_SUBDIR)


def _run_package(
    manifest: pathlib.Path,
    destination: pathlib.Path,
    *,
    runner: SubprocessRunner,
) -> None:
    """Run ``cargo package``, translating every way it can fail.

    Raises
    ------
    RuntimeError
        If the command cannot be launched, exits non-zero, or exceeds
        :data:`PACKAGING_TIMEOUT_SECONDS`.
    """
    try:
        # `runner` is the injected seam, not the `subprocess` module, so the
        # bandit process-launch rules do not reach this call: the production
        # runner installs them at its own `subprocess.run` and the stub never
        # launches anything. Suppressing them here would be an unused noqa.
        completed = runner(
            [
                "cargo",
                "package",
                "--manifest-path",
                str(manifest),
                "--target-dir",
                str(destination),
            ],
            check=False,
            capture_output=True,
            text=True,
            timeout=PACKAGING_TIMEOUT_SECONDS,
        )
    except subprocess.TimeoutExpired as error:
        msg = (
            f"cargo package did not finish within {PACKAGING_TIMEOUT_SECONDS}s "
            f"and was killed; the packaging step cannot be left to the "
            f"runner's own limit, which reports a step timeout and no cause"
        )
        raise RuntimeError(msg) from error
    except OSError as error:
        # An `OSError` here means the command never started: `cargo` is not on
        # PATH, or the directory it names is not executable. Nothing ran, so
        # there is no return code and no standard error to report, and letting
        # the error escape would turn a wiring fault into a traceback.
        msg = (
            f"cargo package could not be launched: {error}; the workflow "
            f"step's own PATH and working directory are what decide this"
        )
        raise RuntimeError(msg) from error
    if completed.returncode != 0:
        msg = (
            f"cargo package failed with exit {completed.returncode}:\n"
            f"{completed.stderr}"
        )
        raise RuntimeError(msg)


def _the_only_crate(staging: pathlib.Path) -> pathlib.Path:
    """Return the single crate in ``staging``, refusing any other count.

    Returns
    -------
    pathlib.Path
        The one ``.crate`` file ``staging`` holds.

    Raises
    ------
    RuntimeError
        If ``staging`` does not hold exactly one ``.crate`` file.
    """
    produced = sorted(staging.glob("*.crate"))
    if len(produced) != 1:
        msg = (
            f"expected exactly one .crate in {staging}, found "
            f"{[path.name for path in produced]}"
        )
        raise RuntimeError(msg)
    return produced[0]


def build_parser() -> argparse.ArgumentParser:
    """Build the parser for the crate-packaging CLI.

    Returns
    -------
    argparse.ArgumentParser
        A parser requiring the identity facts, the manifest, and a destination.
    """
    parser = argparse.ArgumentParser(
        prog="package_crate_cli",
        description="Package the release crate and record its identity.",
    )
    release = parser.add_mutually_exclusive_group(required=True)
    release.add_argument("--tag", help="the release tag, e.g. v1.2.3")
    release.add_argument(
        "--dry-run",
        action="store_true",
        help=(
            "record that this run has no release tag, as a pull-request "
            "rehearsal does; the archive is still packaged and digested"
        ),
    )
    parser.add_argument("--commit", required=True, help="the full commit SHA")
    parser.add_argument("--package-name", required=True, help="the Cargo package name")
    parser.add_argument("--version", required=True, help="the manifest version")
    parser.add_argument("--bin-name", required=True, help="the installed binary name")
    parser.add_argument(
        "--manifest", required=True, type=pathlib.Path, help="the Cargo manifest"
    )
    parser.add_argument(
        "--target-dir",
        required=True,
        type=pathlib.Path,
        help="the target directory cargo package writes beneath",
    )
    parser.add_argument(
        "--output",
        required=True,
        type=pathlib.Path,
        help="where to write the package record",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    """Package the crate and write the record describing it.

    Parameters
    ----------
    argv
        Command-line arguments. Defaults to ``sys.argv[1:]``.

    Returns
    -------
    int
        ``EXIT_OK`` when the archive and its record exist,
        ``EXIT_IDENTITY_REJECTED`` when the identity facts disagree, or
        ``EXIT_PACKAGING_FAILED`` when ``cargo package`` refuses, or
        ``EXIT_ARTIFACT_ERROR`` when archive discovery, hashing, or record
        persistence fails.

    Notes
    -----
    The identity is resolved before cargo runs, so a tag that disagrees with
    the manifest spends no build time and leaves no half-written record.

    Examples
    --------
    A tag that does not spell the manifest version is refused before cargo is
    invoked, so the run exits with the identity code rather than with whatever
    ``cargo package`` would have reported. Rehearsing instead of tagging takes
    ``--dry-run``, which records the absence of a tag. The diagnosis goes to
    standard error, which a doctest does not compare:

    >>> main(["--tag", "v2.0.0", "--commit", "0" * 40, "--package-name", "p",
    ...       "--version", "1.2.3", "--bin-name", "b",
    ...       "--manifest", "/nonexistent/Cargo.toml",
    ...       "--target-dir", "/nonexistent/target",
    ...       "--output", "/nonexistent/package.json"])
    2
    """
    arguments = build_parser().parse_args(argv)
    facts = release_candidate.CandidateFacts(
        tag=None if arguments.dry_run else arguments.tag,
        commit=arguments.commit,
        package_name=arguments.package_name,
        version=arguments.version,
        bin_name=arguments.bin_name,
    )
    try:
        identity = release_candidate.resolve_identity(facts)
    except release_candidate.IdentityError as error:
        print(f"release identity rejected: {error}", file=sys.stderr)
        return EXIT_IDENTITY_REJECTED
    try:
        crate = package_crate(
            arguments.manifest, arguments.target_dir, runner=subprocess.run
        )
    except RuntimeError as error:
        print(f"packaging failed: {error}", file=sys.stderr)
        return EXIT_PACKAGING_FAILED
    except OSError as error:
        print(f"could not inspect the packaged archive: {error}", file=sys.stderr)
        return EXIT_ARTIFACT_ERROR
    try:
        digest = release_digest_files.digest_of_file(crate)
    except OSError as error:
        print(f"could not read the packaged archive {crate}: {error}", file=sys.stderr)
        return EXIT_ARTIFACT_ERROR
    package = CratePackage(identity, crate, digest)
    try:
        _write_package_record(arguments.output, package)
    except OSError as error:
        print("package record write failed:", arguments.output, error, file=sys.stderr)
        return EXIT_ARTIFACT_ERROR
    return EXIT_OK


if __name__ == "__main__":
    raise SystemExit(main())
