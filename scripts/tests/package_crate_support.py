"""Build the fixtures the crate-packaging suites share.

Two things every test in that suite needs, and neither belongs to the module
under test: a stand-in for ``cargo package``, and the argument shapes the CLI
accepts. Keeping them here is what lets the behaviour modules stay inside the
repository's 400-line cap without either of them re-deriving a fixture the
other already has.

The stub is deliberately not a mock of ``subprocess``. It is a runner of the
same shape :data:`package_crate_cli.SubprocessRunner` declares: it returns a
real :class:`subprocess.CompletedProcess`, so the fixture satisfies the
production annotation by construction rather than by a cast, and a test that
exercises :func:`package_crate_cli.package_crate` passes it as an argument.
Only a test that goes through ``main`` needs to install it where the CLI edge
looks it up. That split is the point: the module's process-table access is one
call site, and a test can replace it without reaching into the standard
library.

Nothing here reaches the network or the process table.
"""

import json
import pathlib
import subprocess  # ruff: ignore[suspicious-subprocess-import] - the seam under test.
import typing as typ

import package_crate_cli
import release_candidate
from package_crate_cli import PACKAGE_SUBDIR, build_parser

if typ.TYPE_CHECKING:
    import argparse

    import pytest

COMMIT = "0123456789abcdef0123456789abcdef01234567"
PACKAGE = "netsuke-build"
BIN_NAME = "netsuke"
TAG = "v1.2.3"
VERSION = "1.2.3"

#: The archive name ``cargo package`` produces for :data:`VERSION`.
CRATE_NAME = f"{PACKAGE}-{VERSION}.crate"


#: The callable shape :func:`package_crate_cli.package_crate` accepts. Aliased
#: from the production module's own name so the two cannot drift: a fixture
#: typed against a look-alike would satisfy ``ty`` while failing the call.
type Runner = package_crate_cli.SubprocessRunner


def completed(returncode: int, stderr: str) -> subprocess.CompletedProcess[str]:
    """Return a finished-process record carrying only the fields the CLI reads.

    :func:`package_crate_cli._run_package` reads ``returncode`` and ``stderr``
    and nothing else, so the record is built with those two set and the rest
    left empty. Returning the real type rather than a stand-in keeps the
    fixture assignable to :data:`Runner` without a cast.

    Returns
    -------
    subprocess.CompletedProcess[str]
        A completed process with the given exit code and standard error.
    """
    return subprocess.CompletedProcess([], returncode, "", stderr)


def packaging_argv(
    destination: pathlib.Path,
    *,
    tag: str = TAG,
    manifest: pathlib.Path = pathlib.Path("Cargo.toml"),
    target_dir: pathlib.Path = pathlib.Path("target"),
) -> list[str]:
    """Return the packaging arguments, overriding the fields under test.

    Returns
    -------
    list[str]
        A complete argument list that parses against
        :func:`package_crate_cli.build_parser`.
    """
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


def tagless_argv(destination: pathlib.Path, *, rehearsal: bool) -> list[str]:
    """Return the packaging arguments with the tag pair removed.

    ``rehearsal`` decides whether ``--dry-run`` replaces the tag. Both shapes
    are produced from the same base, so the only difference between the two is
    the selector under test rather than some unrelated argument drifting.

    Returns
    -------
    list[str]
        The packaging arguments, with ``--dry-run`` in place of the tag pair
        when ``rehearsal`` is true.
    """
    argv = packaging_argv(destination)
    del argv[argv.index("--tag") : argv.index("--tag") + 2]
    return ["--dry-run", *argv] if rehearsal else argv


def read_record(destination: pathlib.Path) -> dict[str, object]:
    """Read back the package record the CLI wrote."""
    return json.loads(destination.read_text(encoding="utf-8"))


def parse_argv(argv: list[str]) -> argparse.Namespace:
    """Parse ``argv`` and return the accepted namespace."""
    return build_parser().parse_args(argv)


def stub_runner(body: bytes, calls: list[list[str]]) -> Runner:
    """Return a runner that stages ``body`` as the crate and records the call.

    The staged file lands where ``cargo package --target-dir`` puts it, so a
    caller reading the wrong directory fails the same way it would in the
    workflow rather than passing against a convenient stub.

    Returns
    -------
    Runner
        A callable of the shape :func:`package_crate_cli.package_crate`
        accepts, appending each invocation's argument list to ``calls``.
    """

    def _run(
        arguments: list[str], **_kwargs: object
    ) -> subprocess.CompletedProcess[str]:
        """Record the invocation and stage the crate it would have produced."""
        calls.append(arguments)
        target_dir = pathlib.Path(arguments[arguments.index("--target-dir") + 1])
        staging = target_dir / PACKAGE_SUBDIR
        staging.mkdir(parents=True, exist_ok=True)
        (staging / CRATE_NAME).write_bytes(body)
        return completed(0, "")

    return _run


def stub_cargo(monkeypatch: pytest.MonkeyPatch, body: bytes) -> list[list[str]]:
    """Replace ``cargo package`` with one that writes ``body`` as the crate.

    The replacement is installed where the CLI edge looks it up, so the whole
    ``main`` path runs without the process table being touched. A test that
    calls :func:`package_crate_cli.package_crate` directly passes
    :func:`stub_runner` instead, because the runner is a parameter there.

    Returns
    -------
    list[list[str]]
        The list the stub appends each invocation to. Empty until the CLI
        actually reaches the packaging call, which is what lets a test assert
        that a refused identity spent no build time.
    """
    calls: list[list[str]] = []
    monkeypatch.setattr("package_crate_cli.subprocess.run", stub_runner(body, calls))
    return calls


def record_identity() -> release_candidate.ReleaseIdentity:
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


__all__ = [
    "BIN_NAME",
    "COMMIT",
    "CRATE_NAME",
    "PACKAGE",
    "TAG",
    "VERSION",
    "Runner",
    "completed",
    "packaging_argv",
    "parse_argv",
    "read_record",
    "record_identity",
    "stub_cargo",
    "stub_runner",
    "tagless_argv",
]
