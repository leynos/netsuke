"""Exercise registry transport boundaries and byte-preserving invariants."""

import dataclasses as dc
import http.client
import json
import string
import struct
import typing as typ
import urllib.error
import urllib.request

import crates_io_upload_metadata
import pytest
from crates_io_transport_cli import (
    DOWNLOAD_ATTEMPTS,
    EXIT_OK,
    EXIT_TRANSPORT_ERROR,
    EXIT_UPLOAD_FAILED,
    _upload_frame,
    main,
)
from crates_io_transport_http import RegistryUrlError, _AllowlistedRequestHandler
from crates_io_transport_support import FakeOpener
from crates_io_upload_metadata import MetadataError, MetadataIssue
from hypothesis import given, settings
from hypothesis import strategies as st

if typ.TYPE_CHECKING:
    import pathlib

CRATE = "netsuke-build"
VERSION = "1.2.3"
TEST_CREDENTIAL = "short-lived-test-value"
IDENTIFIER = st.text(
    alphabet=string.ascii_letters + string.digits + "_-.", min_size=1, max_size=16
)
README_COMPONENT = st.text(
    alphabet=string.ascii_letters + string.digits + "_-", min_size=1, max_size=16
)
EXPECTED_DEPENDENCY_SOURCES = (
    ("normal", None),
    ("dev", None),
    ("build", None),
    ("normal", "target"),
    ("dev", "target"),
    ("build", "target"),
)


@dc.dataclass(frozen=True, slots=True)
class DependencyProperties:
    """Bundle generated values for one normalized Cargo dependency."""

    name: str
    version: str
    features: list[str]
    optional: bool
    default_features: bool
    target: str | None
    kind: str


DEPENDENCY_PROPERTIES = st.builds(
    DependencyProperties,
    name=IDENTIFIER,
    version=st.text(
        alphabet=string.ascii_letters + string.digits + ".^~<>=+-",
        min_size=1,
        max_size=20,
    ),
    features=st.lists(IDENTIFIER, unique=True, max_size=6),
    optional=st.booleans(),
    default_features=st.booleans(),
    target=st.one_of(st.none(), IDENTIFIER),
    kind=st.sampled_from(("normal", "dev", "build")),
)


def _probe_args(status: pathlib.Path, body: pathlib.Path) -> list[str]:
    """Build a single-response registry probe invocation."""
    return [
        "probe",
        "--crate",
        CRATE,
        "--version",
        VERSION,
        "--status-file",
        str(status),
        "--body-file",
        str(body),
    ]


def _download_args(
    archive: pathlib.Path, status: pathlib.Path, report: pathlib.Path | None = None
) -> list[str]:
    """Build a bounded archive polling invocation."""
    arguments = [
        "download",
        "--crate",
        CRATE,
        "--version",
        VERSION,
        "--archive",
        str(archive),
        "--status-file",
        str(status),
    ]
    if report is not None:
        arguments.extend(("--report-file", str(report)))
    return arguments


@pytest.mark.parametrize(
    ("url", "hook"),
    [
        pytest.param(
            "https://attacker.example/forged-response",
            "https_request",
            id="unallowlisted-host",
        ),
        pytest.param("ftp://crates.io/forged-response", "ftp_request", id="ftp"),
    ],
)
def test_request_handler_rejects_an_unallowlisted_redirect_target(
    url: str, hook: str
) -> None:
    """Recheck the URL urllib opens after following a registry redirect."""
    handler = _AllowlistedRequestHandler(frozenset({"crates.io"}))
    request = urllib.request.Request(  # ruff: ignore[suspicious-url-open-usage] - test a rejected redirect target
        url
    )

    with pytest.raises(RegistryUrlError):
        getattr(handler, hook)(request)


def test_rejected_redirect_fails_closed(tmp_path: pathlib.Path) -> None:
    """Treat rejected redirect targets as an unavailable registry observation."""
    opener = FakeOpener(RegistryUrlError())

    result = main(_probe_args(tmp_path / "status", tmp_path / "body"), opener=opener)

    assert result == EXIT_TRANSPORT_ERROR, "reject a forged registry probe"
    assert len(opener.requests) == 1, "do not retry a rejected probe redirect"


def test_probe_fails_closed_when_no_http_status_exists(tmp_path: pathlib.Path) -> None:
    """Do not turn a connection failure into an absent registry version."""
    status = tmp_path / "status.txt"
    body = tmp_path / "body.json"
    opener = FakeOpener(urllib.error.URLError("offline"))

    result = main(_probe_args(status, body), opener=opener)

    assert result == EXIT_TRANSPORT_ERROR, "a probe without status must fail closed"
    assert len(opener.requests) == 1, "the failed probe must not retry"
    assert not status.exists(), "a missing status must not be written as HTTP 404"


def test_incomplete_probe_fails_closed(tmp_path: pathlib.Path) -> None:
    """Do not classify a truncated probe body as a registry observation."""
    status = tmp_path / "status"
    body = tmp_path / "body"
    opener = FakeOpener(http.client.IncompleteRead(b"", 1))

    result = main(_probe_args(status, body), opener=opener)

    assert result == EXIT_TRANSPORT_ERROR, "an incomplete probe has no usable status"
    assert len(opener.requests) == 1, "a registry probe is single-shot"
    assert not status.exists(), "a partial body must not create a status observation"


def test_download_retries_incomplete_http_responses_to_its_budget(
    tmp_path: pathlib.Path,
) -> None:
    """Write the unavailable sentinel after bounded retries of truncated bodies."""
    archive = tmp_path / "archive.crate"
    status = tmp_path / "status"
    report = tmp_path / "download-report.json"
    delays: list[float] = []
    opener = FakeOpener(*[
        http.client.IncompleteRead(b"", 1) for _ in range(DOWNLOAD_ATTEMPTS)
    ])

    result = main(
        _download_args(archive, status, report), opener=opener, sleeper=delays.append
    )

    assert result == EXIT_OK, "the offline verifier handles the sentinel"
    assert len(opener.requests) == DOWNLOAD_ATTEMPTS, "honour the fixed retry budget"
    assert len(delays) == DOWNLOAD_ATTEMPTS - 1, "do not wait after the final attempt"
    assert status.read_text(encoding="ascii").strip() == "0", (
        "represent an incomplete response without an HTTP status"
    )
    assert archive.read_bytes() == b"", "do not retain a partial archive"
    diagnostics = json.loads(report.read_text(encoding="ascii"))
    assert diagnostics["attempts"] == DOWNLOAD_ATTEMPTS, "record bounded attempts"
    assert diagnostics["status_class"] == "unknown", "hide absent status details"
    assert diagnostics["elapsed_ms"] >= 0, "record non-negative elapsed time"


def test_upload_treats_an_incomplete_http_response_as_ambiguous(
    tmp_path: pathlib.Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Fail once on a truncated upload response without leaking the token."""
    metadata = tmp_path / "metadata.json"
    archive = tmp_path / "archive.crate"
    _ = metadata.write_bytes(b"{}")
    _ = archive.write_bytes(b"candidate")
    opener = FakeOpener(http.client.IncompleteRead(TEST_CREDENTIAL.encode(), 1))

    result = main(
        ["upload", "--metadata", str(metadata), "--archive", str(archive)],
        opener=opener,
        environment={"CRATES_IO_TOKEN": TEST_CREDENTIAL},
    )

    assert result == EXIT_UPLOAD_FAILED, "a truncated write response is ambiguous"
    assert len(opener.requests) == 1, "never retry an ambiguous upload"
    captured = capsys.readouterr()
    assert "outcome is ambiguous" in captured.err, "diagnose the uncertain write"
    assert TEST_CREDENTIAL not in captured.out + captured.err, (
        "redact the short-lived token"
    )


@settings(max_examples=100, deadline=None, derandomize=True)
@given(
    metadata=st.binary(max_size=4096),
    archive=st.binary(max_size=8192),
)
def test_upload_frame_round_trips_varied_metadata_and_archive_bytes(
    metadata: bytes, archive: bytes
) -> None:
    """Preserve arbitrary payload bytes and encode each exact frame length."""
    frame = _upload_frame(metadata, archive)
    metadata_length = struct.unpack_from("<I", frame)[0]
    archive_length_offset = 4 + metadata_length
    archive_length = struct.unpack_from("<I", frame, archive_length_offset)[0]
    archive_offset = archive_length_offset + 4

    assert frame[4:archive_length_offset] == metadata, "preserve metadata bytes"
    assert archive_length == len(archive), "record the exact archive byte length"
    assert frame[archive_offset:] == archive, "preserve retained archive bytes"


@settings(max_examples=100, deadline=None, derandomize=True)
@given(case=DEPENDENCY_PROPERTIES)
def test_dependency_normalization_preserves_valid_manifest_values(
    case: DependencyProperties,
) -> None:
    """Retain dependency identity, feature flags, target, and kind."""
    specification = {
        "version": case.version,
        "features": case.features,
        "optional": case.optional,
        "default-features": case.default_features,
    }

    dependency = crates_io_upload_metadata._dependency(
        case.name, specification, case.kind, case.target
    )

    assert dependency["name"] == case.name, "preserve the manifest dependency name"
    assert dependency["version_req"] == case.version, "preserve the version requirement"
    assert dependency["features"] == case.features, "preserve requested features"
    assert dependency["optional"] is case.optional, "preserve optionality"
    assert dependency["default_features"] is case.default_features, (
        "preserve the default-feature flag"
    )
    assert dependency["target"] == case.target, "preserve the target condition"
    assert dependency["kind"] == case.kind, "preserve the dependency kind"


@settings(max_examples=100, deadline=None, derandomize=True)
@given(case=DEPENDENCY_PROPERTIES, target=IDENTIFIER)
def test_dependency_records_flatten_all_kinds_and_target_tables(
    case: DependencyProperties, target: str
) -> None:
    """Preserve dependency kinds and target conditions through table flattening."""
    specification = {
        "version": case.version,
        "features": case.features,
        "optional": case.optional,
        "default-features": case.default_features,
    }
    tables = {
        "dependencies": {case.name: specification},
        "dev-dependencies": {case.name: specification},
        "build-dependencies": {case.name: specification},
        "target": {
            target: {
                "dependencies": {case.name: specification},
                "dev-dependencies": {case.name: specification},
                "build-dependencies": {case.name: specification},
            }
        },
    }

    dependencies = crates_io_upload_metadata._dependency_records(
        typ.cast("dict[str, object]", tables)
    )

    actual_sources = [
        (dependency["kind"], dependency["target"]) for dependency in dependencies
    ]
    expected_sources = [
        (kind, None if target_condition is None else target)
        for kind, target_condition in EXPECTED_DEPENDENCY_SOURCES
    ]
    assert actual_sources == expected_sources, (
        "retain each manifest dependency table and target condition"
    )


@pytest.mark.parametrize(
    ("specification", "expected_issue"),
    [
        pytest.param(None, MetadataIssue.DEPENDENCY_SPEC, id="invalid-specification"),
        pytest.param({}, MetadataIssue.DEPENDENCY_VERSION, id="missing-version"),
        pytest.param(
            {"version": "^1", "features": "derive"},
            MetadataIssue.STRING_ARRAY,
            id="non-array-features",
        ),
        pytest.param(
            {"version": "^1", "optional": "yes"},
            MetadataIssue.DEPENDENCY_FLAGS,
            id="non-boolean-flag",
        ),
    ],
)
def test_dependency_normalization_rejects_malformed_manifest_values(
    specification: object, expected_issue: MetadataIssue
) -> None:
    """Classify malformed dependency shapes before registry serialization."""
    with pytest.raises(MetadataError) as raised:
        crates_io_upload_metadata._dependency("serde", specification, "normal", None)

    assert raised.value.issue is expected_issue, "report the malformed field category"


@pytest.mark.parametrize(
    "candidate",
    [
        pytest.param("../outside.md", id="parent-relative"),
        pytest.param("/outside.md", id="absolute"),
        pytest.param("docs\\README.md", id="windows-separator"),
    ],
)
def test_readme_paths_reject_escaping_and_ambiguous_values(candidate: str) -> None:
    """Keep packaged README selection within the archive root on every host."""
    import pathlib

    assert not crates_io_upload_metadata._is_safe_readme_path(
        candidate, pathlib.PurePosixPath(candidate)
    ), "reject README paths that escape or vary by host platform"


@settings(max_examples=100, deadline=None, derandomize=True)
@given(parts=st.lists(README_COMPONENT, min_size=1, max_size=5))
def test_readme_paths_accept_varied_safe_relative_paths(parts: list[str]) -> None:
    """Accept nested README paths that remain relative and platform-neutral."""
    import pathlib

    candidate = "/".join(parts)

    assert crates_io_upload_metadata._is_safe_readme_path(
        candidate, pathlib.PurePosixPath(candidate)
    ), "accept safe relative README paths"


@pytest.mark.parametrize(
    ("manifest", "expected_issue"),
    [
        pytest.param(
            {"target": None}, MetadataIssue.TARGET_TABLE, id="invalid-target-table"
        ),
        pytest.param(
            {"target": {"cfg(unix)": None}},
            MetadataIssue.TARGET_SHAPE,
            id="invalid-target-shape",
        ),
        pytest.param(
            {"target": {"cfg(unix)": {"dependencies": []}}},
            MetadataIssue.TARGET_DEPENDENCIES,
            id="invalid-target-dependencies",
        ),
    ],
)
def test_dependency_records_reject_malformed_target_tables(
    manifest: dict[str, object], expected_issue: MetadataIssue
) -> None:
    """Classify malformed target tables before registry serialization."""
    with pytest.raises(MetadataError) as raised:
        crates_io_upload_metadata._dependency_records(manifest)

    assert raised.value.issue is expected_issue, "report the malformed target category"
