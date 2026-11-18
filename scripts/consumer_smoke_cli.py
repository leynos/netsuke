#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# dependencies = ["cyclopts>=4.25.3,<5"]
# ///
"""Plan and verify the public-consumer smoke for one release candidate.

``plan`` binds the trusted evidence to the exact public downloads a smoke run
must exercise: the crate on ``static.crates.io`` and the cargo-binstall archive
with its checksum sidecar on the GitHub release. ``verify`` re-derives that plan
from the same evidence, refuses a saved plan that disagrees, and then checks the
bytes the workflow downloaded.

Nothing here fetches anything. The workflow performs the downloads with a
credential-free ``curl`` and this CLI reads what was saved, so the smoke
exercises the same public URLs a consumer uses while the Python side stays
network-free and testable.

The command line is Cyclopts's. Each command takes one dataclass from
:mod:`consumer_smoke_options`, which Cyclopts flattens into one flag per field;
the flag the action passes and the value the stage below reads are the same
name, so neither can move without the other.

Decisions live in :mod:`release_consumer_smoke` and
:mod:`release_consumer_payload`, the trusted-record reads in
:mod:`consumer_smoke_evidence`, and the phase walk in
:mod:`consumer_smoke_verify`, which also owns the exit codes each refusal
reports:

- ``0`` every phase verified.
- ``1`` the identity, trusted record, declared archive set, or saved plan is
  unusable, or the verdict artefact cannot be written.
- ``2`` a public download is missing, empty, or not the bytes we published.
- ``3`` the archive itself is refused: corrupt, unsafe to extract, or
  unwritable.
- ``4`` the shipped executable could not report the candidate's exact version:
  it could not be launched, did not answer in time, exited non-zero, or printed
  something else.

Run from ``.github/actions/consumer-smoke/action.yml``; behavioural coverage
lives in ``scripts/tests/test_consumer_smoke_cli.py``.

Examples
--------
A rehearsal identity records no tag, so planning against it is refused before
any download is named. The diagnosis goes to standard error, which a doctest
does not compare:

>>> import json, tempfile
>>> from pathlib import Path
>>> with tempfile.TemporaryDirectory() as tmp:
...     root = Path(tmp)
...     identity = root / "identity.json"
...     record = {
...         "crate": "netsuke-build-1.2.3.crate",
...         "identity": {
...             "tag": None, "commit": "0" * 40,
...             "package_name": "netsuke-build", "version": "1.2.3",
...             "bin_name": "netsuke", "is_prerelease": False,
...         },
...         "sha256": "ab" * 32,
...     }
...     _ = identity.write_text(json.dumps(record), encoding="utf-8")
...     trusted = root / "trusted.json"
...     _ = trusted.write_text(
...         json.dumps({"assets": {"a": "ab" * 32}}), encoding="utf-8"
...     )
...     staging = root / "staging.toml"
...     _ = staging.write_text(
...         '[targets.linux]' + chr(10)
...         + 'target = "x86_64-unknown-linux-gnu"' + chr(10),
...         encoding="utf-8",
...     )
...     manifest = root / "Cargo.toml"
...     _ = manifest.write_text(
...         '[package]' + chr(10) + 'name = "netsuke-build"' + chr(10),
...         encoding="utf-8",
...     )
...     main([
...         "plan",
...         "--identity", str(identity),
...         "--trusted", str(trusted),
...         "--staging-config", str(staging),
...         "--manifest", str(manifest),
...         "--repository", "leynos/netsuke",
...         "--target", "x86_64-unknown-linux-gnu",
...         "--commit", "0" * 40,
...         "--output", str(root / "plan.json"),
...     ])
1
"""

import sys
import typing as typ

import consumer_smoke_evidence as evidence
import consumer_smoke_verify as verify_stage
import release_binstall_child as child_adapter
import release_consumer_smoke as smoke
from consumer_smoke_options import PlanOptions, VerifyOptions
from consumer_smoke_verify import EXIT_EVIDENCE_REJECTED, EXIT_OK

from cyclopts import App, Parameter

if typ.TYPE_CHECKING:
    import collections.abc as cabc

    import release_binstall_probe as probe_adapter


def run_plan(options: PlanOptions) -> int:
    """Write the plan document for this run's evidence.

    Returns
    -------
    int
        :data:`consumer_smoke_verify.EXIT_OK` once the plan is written, or the
        evidence-rejected status when the evidence is unreadable or unusable.
    """
    try:
        plan = smoke.plan_smoke(evidence.gather_evidence(options))
        evidence.write_json(options.output, evidence.plan_document(plan))
    except (smoke.EvidenceRejectedError, OSError) as error:
        print(f"consumer smoke evidence rejected: {error}", file=sys.stderr)
        return EXIT_EVIDENCE_REJECTED
    return EXIT_OK


def build_app(launcher: probe_adapter.ProbeLauncher | None = None) -> App:
    """Build the command app, binding the launcher this run will use.

    Parameters
    ----------
    launcher
        The callable that starts the extracted binary. ``None`` starts a real
        child, which is why this is the composition root for the probe; a test
        supplies its own so the probe boundary is exercised without executing a
        downloaded file.

    Returns
    -------
    App
        An app whose ``plan`` and ``verify`` commands are the ones the
        composite action invokes. Building it per call rather than once at
        import keeps the injected launcher out of module state.
    """
    chosen = child_adapter.launch if launcher is None else launcher
    app = App(
        name="consumer_smoke_cli",
        help="Plan and verify the public-consumer release smoke.",
    )

    @app.command
    def plan(options: typ.Annotated[PlanOptions, Parameter(name="*")]) -> int:
        """Derive the public downloads this run must perform."""
        return run_plan(options)

    @app.command
    def verify(options: typ.Annotated[VerifyOptions, Parameter(name="*")]) -> int:
        """Check the saved downloads and the shipped executable."""
        return verify_stage.run_verify(options, launcher=chosen)

    return app


def main(
    argv: cabc.Sequence[str] | None = None,
    *,
    launcher: probe_adapter.ProbeLauncher | None = None,
) -> int:
    """Run the requested command and report its outcome.

    Parameters
    ----------
    argv
        Command-line arguments. Defaults to ``sys.argv[1:]``.
    launcher
        The callable that starts the extracted binary; see :func:`build_app`.

    Returns
    -------
    int
        One of the exit codes stated in the module docstring.

    Notes
    -----
    The module docstring's example walks a rehearsal identity to its refusal;
    behavioural coverage of both commands lives in
    ``scripts/tests/test_consumer_smoke_cli.py``.
    """
    return build_app(launcher)(argv, result_action="return_value")


if __name__ == "__main__":
    raise SystemExit(main())
