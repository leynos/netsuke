#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# dependencies = []
# ///
r"""Write the release-admission evidence manifest from the downstream canaries.

This is the producer RFC 0005's admission gate reads. Each downstream canary
job uploads one provenance record (``scripts/downstream_canary_provenance.py``
owns its schema); this script reads them, decides one result per canary, and
writes the schema-1 manifest ``_release_admission.manifest`` parses. A canary
succeeds only when its job succeeded, its record is the expected schema and
names that canary, it built this release's commit, and it passed. Anything
else -- a missing or unreadable record, another candidate, a failed target --
is declared as a failure, never omitted, so the verifier refuses it.

The script then publishes the manifest's SHA-256 to ``GITHUB_OUTPUT``. The job
re-exports that value as a job output, and the admission job hands it to the
gate, which admits only a document whose bytes hash to it. That digest is the
binding from the uploaded artefact to this producer.

It declares only what it observed: the downstream results. ``history_scan``
and ``release_candidate`` have producers of their own to come, and until they
exist the gate refuses the manifest as missing evidence.

Usage
-----
From the verifier checkout's parent directory::

    uv run --no-config --no-project --script \
        gate/.github/scripts/write_release_admission_evidence.py \
        --repository leynos/netsuke --commit <sha> --tag v0.1.0 \
        --run-id 1234 --provenance-dir provenance \
        --linux-result success --windows-result success \
        --output evidence/evidence.json --github-output "$GITHUB_OUTPUT"
"""

import argparse
import collections.abc as cabc
import dataclasses
import datetime as dt
import hashlib
import json
import sys
from pathlib import Path

# The workflow runs this file by path, so the package beside it is not
# importable by default; see ``release_admission.py`` for the same insertion.
sys.path.insert(0, str(Path(__file__).resolve().parent))

from _release_admission import (
    downstream,
    manifest,
    policy,
)

#: The provenance schema the canary records are written in.
PROVENANCE_SCHEMA = "netsuke.downstream-canary.provenance/v1"
#: The provenance outcome a passing canary reports.
PASSED = "passed"
#: The artefact name prefix each canary job uploads its record under.
PROVENANCE_ARTEFACT_PREFIX = "downstream-canary-provenance-"
#: The record's file name inside each artefact.
PROVENANCE_FILE = "downstream-canary-provenance.json"
#: The job result GitHub reports for a job whose every matrix row passed.
JOB_SUCCESS = "success"


@dataclasses.dataclass(frozen=True, slots=True)
class Release:
    """Identify the release the manifest is evidence for.

    Attributes
    ----------
    repository
        The ``owner/name`` the workflow runs in.
    commit
        The candidate commit every canary must have built.
    tag
        The release tag, or the ref name a rehearsal runs on.
    run_id
        The workflow run the manifest belongs to.
    """

    repository: str
    commit: str
    tag: str
    run_id: str


def canary_succeeded(
    canary: str, record: object, *, commit: str, job_result: str
) -> bool:
    """Return whether one canary's record admits it.

    Returns
    -------
    bool
        Whether the job passed and the record names this canary, the expected
        schema and commit, and a passing outcome.

    Examples
    --------
    >>> record = {"schema": PROVENANCE_SCHEMA, "canary": "mxd-sqlite",
    ...           "netsuke_revision": "a" * 40, "outcome": "passed"}
    >>> canary_succeeded("mxd-sqlite", record, commit="a" * 40, job_result="success")
    True
    >>> canary_succeeded("mxd-sqlite", record, commit="b" * 40, job_result="success")
    False
    """
    if job_result != JOB_SUCCESS or not isinstance(record, cabc.Mapping):
        return False
    expected = {
        "schema": PROVENANCE_SCHEMA,
        "canary": canary,
        "netsuke_revision": commit,
        "outcome": PASSED,
    }
    return all(record.get(key) == value for key, value in expected.items())


def build_manifest(
    release: Release,
    records: cabc.Mapping[str, object],
    job_results: cabc.Mapping[str, str],
    issued_at: dt.datetime,
) -> dict[str, object]:
    """Build the schema-1 evidence manifest for every downstream canary.

    Parameters
    ----------
    release
        The release the manifest describes.
    records
        Each canary's decoded provenance record, keyed by canary; an absent
        key is a canary that left no readable record.
    job_results
        Each canary's job result, keyed by canary.
    issued_at
        The instant the manifest is issued, in UTC.

    Returns
    -------
    dict[str, object]
        The manifest, ready to serialize.
    """
    moment = issued_at.astimezone(dt.UTC).isoformat().replace("+00:00", "Z")
    results = {
        downstream.result_name(canary): {
            "outcome": (
                policy.Outcome.SUCCESS
                if canary_succeeded(
                    canary,
                    records.get(canary),
                    commit=release.commit,
                    job_result=job_results.get(canary, ""),
                )
                else policy.Outcome.FAILURE
            ).value,
            "observed_at": moment,
        }
        for canary in downstream.CANARIES
    }
    return {
        "schema_version": manifest.SCHEMA_VERSION,
        "repository": release.repository,
        "commit": release.commit,
        "tag": release.tag,
        "workflow_run_id": release.run_id,
        "issued_at": moment,
        "results": results,
    }


def read_records(directory: Path) -> dict[str, object]:
    """Read every canary's provenance record that can be decoded.

    ``actions/download-artifact`` extracts each artefact into a directory named
    after it. A record that is absent or not JSON is left out, which the
    manifest then reports as that canary's failure.

    Returns
    -------
    dict[str, object]
        Each decodable record, keyed by canary.
    """
    records: dict[str, object] = {}
    for canary in downstream.CANARIES:
        path = directory / f"{PROVENANCE_ARTEFACT_PREFIX}{canary}" / PROVENANCE_FILE
        try:
            records[canary] = json.loads(path.read_text(encoding="utf-8"))
        except OSError, ValueError:
            continue
    return records


def job_results(linux: str, windows: str) -> dict[str, str]:
    """Map each canary to the result of the job that ran it.

    Returns
    -------
    dict[str, str]
        The Linux or Windows job result, keyed by canary.

    Examples
    --------
    >>> job_results("success", "failure")["ortho-config-windows"]
    'failure'
    """
    return {
        canary: windows if canary in downstream.WINDOWS_CANARIES else linux
        for canary in downstream.CANARIES
    }


def serialize(document: cabc.Mapping[str, object]) -> bytes:
    """Render the manifest as the exact bytes the digest is taken over.

    Returns
    -------
    bytes
        Sorted, indented UTF-8 JSON with a trailing newline.
    """
    return (json.dumps(document, indent=2, sort_keys=True) + "\n").encode("utf-8")


def _parse_arguments(argv: cabc.Sequence[str] | None) -> argparse.Namespace:
    """Parse the command line."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    for name in ("repository", "commit", "tag", "run-id", "linux-result"):
        parser.add_argument(f"--{name}", required=True)
    parser.add_argument("--windows-result", required=True)
    parser.add_argument("--provenance-dir", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--github-output", required=True, type=Path)
    return parser.parse_args(argv)


def main(argv: cabc.Sequence[str] | None = None) -> int:
    """Write the manifest and publish its digest.

    Returns
    -------
    int
        Zero once both files are written; an unwritable path raises.
    """
    arguments = _parse_arguments(argv)
    release = Release(
        repository=arguments.repository,
        commit=arguments.commit,
        tag=arguments.tag,
        run_id=arguments.run_id,
    )
    document = serialize(
        build_manifest(
            release,
            read_records(arguments.provenance_dir),
            job_results(arguments.linux_result, arguments.windows_result),
            dt.datetime.now(dt.UTC),
        )
    )
    arguments.output.parent.mkdir(parents=True, exist_ok=True)
    arguments.output.write_bytes(document)
    with arguments.github_output.open("a", encoding="utf-8") as stream:
        stream.write(f"evidence-sha256={hashlib.sha256(document).hexdigest()}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
