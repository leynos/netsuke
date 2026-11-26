"""Verify the release-admission evidence producer against the real verifier.

The producer turns the downstream canaries' provenance records into the
manifest the admission gate reads. Each case writes records with the canary
runner's own record builder and reads the manifest back through the gate's own
parser, so the producer cannot drift from either schema unnoticed. The cases
pin the producer's one obligation: every downstream canary is declared, and
only a canary that passed against this release's commit is declared a success.
"""

import hashlib
import importlib.util
import json
import sys
import typing as typ
from pathlib import Path

import pytest
from downstream_canary_provenance import CanaryIdentity, provenance_record

if typ.TYPE_CHECKING:
    import collections.abc as cabc

SCRIPT = (
    Path(__file__).resolve().parents[2]
    / ".github"
    / "scripts"
    / "write_release_admission_evidence.py"
)
_SPEC = importlib.util.spec_from_file_location(
    "write_release_admission_evidence", SCRIPT
)
if _SPEC is None or _SPEC.loader is None:
    message = f"cannot load {SCRIPT}"
    raise ImportError(message)
producer = importlib.util.module_from_spec(_SPEC)
sys.modules[_SPEC.name] = producer
_SPEC.loader.exec_module(producer)

# Loading the producer put its directory on the path; these are the verifier's
# own modules, imported after it on purpose.
from _release_admission import (  # ruff: ignore[module-import-not-at-top-of-file] - needs the producer's path.
    downstream,
    evidence,
    manifest,
    policy,
)

COMMIT = "a" * 40
OTHER_COMMIT = "b" * 40
DOWNSTREAM_REVISION = "c" * 40
RELEASE = {
    "repository": "leynos/netsuke",
    "commit": COMMIT,
    "tag": "v0.1.0",
    "run-id": "1234",
}


def _record(canary: str, *, commit: str = COMMIT, passed: bool = True) -> object:
    """Build the record the canary runner writes for one canary."""
    identity = CanaryIdentity(
        canary=canary,
        repository="leynos/example",
        downstream_revision=DOWNSTREAM_REVISION,
        netsuke_revision=commit,
        netsuke_version="0.1.0",
        platform="Linux",
        selectors={},
        targets=("test",),
    )
    status = "passed" if passed else "failed"
    state = {"generate": "passed", "isolation": "passed", "targets": {"test": status}}
    return provenance_record(identity, state, DOWNSTREAM_REVISION)


def _write_records(directory: Path, records: cabc.Mapping[str, object]) -> None:
    """Lay records out as ``actions/download-artifact`` extracts a pattern."""
    for canary, record in records.items():
        artefact = directory / f"downstream-canary-provenance-{canary}"
        artefact.mkdir(parents=True)
        payload = record if isinstance(record, str) else json.dumps(record)
        (artefact / "downstream-canary-provenance.json").write_text(
            payload, encoding="utf-8"
        )


def _produce(
    tmp_path: Path,
    records: cabc.Mapping[str, object],
    *,
    linux: str = "success",
    windows: str = "success",
) -> tuple[manifest.Manifest, bytes, str]:
    """Run the producer and return the parsed manifest, its bytes and digest."""
    provenance = tmp_path / "provenance"
    _write_records(provenance, records)
    output = tmp_path / "evidence" / "evidence.json"
    github_output = tmp_path / "github-output"
    arguments = [f"--{name}={value}" for name, value in RELEASE.items()]
    arguments += [
        f"--provenance-dir={provenance}",
        f"--linux-result={linux}",
        f"--windows-result={windows}",
        f"--output={output}",
        f"--github-output={github_output}",
    ]
    assert producer.main(arguments) == 0, "the producer must report success"
    document = output.read_bytes()
    parsed = manifest.parse_manifest(document)
    assert parsed.manifest is not None, "the verifier must parse the manifest"
    published = github_output.read_text(encoding="utf-8")
    assert published.startswith("evidence-sha256="), published
    return parsed.manifest, document, published.removeprefix("evidence-sha256=").strip()


def _passing_records() -> dict[str, object]:
    """Return a passing record for every downstream canary."""
    return {canary: _record(canary) for canary in downstream.CANARIES}


def _outcomes(parsed: manifest.Manifest) -> dict[str, str]:
    """Return each declared result's outcome."""
    return {name: result.outcome for name, result in parsed.results.items()}


def test_passing_canaries_are_declared_successes_and_attributed(
    tmp_path: Path,
) -> None:
    """Every passing canary is a success, and the digest names the file.

    The producer declares only the downstream results it observed: the RFC
    0005 results have producers of their own, so the verifier still reports
    them missing, which keeps publication closed until those producers exist.
    """
    parsed, document, digest = _produce(tmp_path, _passing_records())

    assert _outcomes(parsed) == dict.fromkeys(downstream.RESULTS, "success"), (
        "every passing canary must be declared a success"
    )
    assert evidence.is_attributed(document, digest), (
        "the published digest must attribute exactly the bytes written"
    )
    assert digest == hashlib.sha256(document).hexdigest(), (
        "the published digest must be the manifest's SHA-256"
    )
    candidate = evidence.Candidate(
        repository=RELEASE["repository"],
        commit=COMMIT,
        tag=RELEASE["tag"],
        run_id=RELEASE["run-id"],
    )
    assert evidence.verify(parsed, candidate).error_category == (
        policy.ErrorCategory.MISSING_EVIDENCE
    ), "the history scan and release-candidate results are not this producer's"


@pytest.mark.parametrize(
    ("records", "linux", "failed"),
    [
        pytest.param(
            {"mxd-sqlite": None}, "success", "mxd-sqlite", id="record-missing"
        ),
        pytest.param(
            {"mxd-sqlite": "not json"}, "success", "mxd-sqlite", id="record-unreadable"
        ),
        pytest.param(
            {"mxd-sqlite": _record("mxd-sqlite", commit=OTHER_COMMIT)},
            "success",
            "mxd-sqlite",
            id="another-candidate",
        ),
        pytest.param(
            {"mxd-sqlite": _record("mxd-sqlite", passed=False)},
            "success",
            "mxd-sqlite",
            id="target-failed",
        ),
        pytest.param(
            {"mxd-sqlite": _record("mxd-postgres")},
            "success",
            "mxd-sqlite",
            id="another-canary's-record",
        ),
    ],
)
def test_an_unproven_canary_is_declared_a_failure(
    tmp_path: Path,
    records: dict[str, object],
    linux: str,
    failed: str,
) -> None:
    """A canary that did not pass against this commit is a declared failure.

    Omitting it would leave the verifier to notice an absence; declaring it a
    failure makes the refusal explicit, and every other canary is unaffected.
    """
    merged: dict[str, object] = {
        canary: record
        for canary, record in {**_passing_records(), **records}.items()
        if record is not None
    }

    parsed, _, _ = _produce(tmp_path, merged, linux=linux)

    expected = dict.fromkeys(downstream.RESULTS, "success")
    expected[downstream.result_name(failed)] = "failure"
    assert _outcomes(parsed) == expected, f"only {failed} must be declared a failure"


@pytest.mark.parametrize(
    ("linux", "windows"),
    [
        pytest.param("failure", "success", id="linux-job-failed"),
        pytest.param("success", "cancelled", id="windows-job-cancelled"),
        pytest.param("skipped", "skipped", id="canaries-skipped"),
    ],
)
def test_a_job_that_did_not_succeed_fails_its_canaries(
    tmp_path: Path, linux: str, windows: str
) -> None:
    """A passing record does not outweigh the job that ran it.

    A record can be uploaded before a later step fails the job, so the job's
    own result is required as well.
    """
    parsed, _, _ = _produce(tmp_path, _passing_records(), linux=linux, windows=windows)

    expected = {
        downstream.result_name(canary): (
            "success"
            if (windows if canary in downstream.WINDOWS_CANARIES else linux)
            == "success"
            else "failure"
        )
        for canary in downstream.CANARIES
    }
    assert _outcomes(parsed) == expected, (
        "each canary must follow the result of the job that ran it"
    )


def test_the_manifest_names_the_release(tmp_path: Path) -> None:
    """The identity fields are the ones the gate compares with the run."""
    parsed, _, _ = _produce(tmp_path, _passing_records())

    assert (
        parsed.repository,
        parsed.commit,
        parsed.tag,
        parsed.workflow_run_id,
    ) == (RELEASE["repository"], COMMIT, RELEASE["tag"], RELEASE["run-id"]), (
        "the manifest must name the release the gate compares it with"
    )


def test_every_downstream_result_is_required_by_the_verifier() -> None:
    """The gate refuses a manifest that omits any downstream canary.

    The matrix contract ties the workflow's rows to ``downstream.CANARIES``;
    this ties those names to the verifier, so dropping one from
    ``policy.REQUIRED_RESULTS`` cannot pass unnoticed.
    """
    missing = sorted(set(downstream.RESULTS) - set(policy.REQUIRED_RESULTS))
    assert not missing, f"the verifier must require every downstream result: {missing}"
