"""Build bounded, pure CodeScene Check Run and workflow-summary values.

``coverage_pr_submission.py`` is the only production caller. This module never
reads environments or performs I/O: its narrow scope is composing trusted,
already-bounded values before the Check Run publisher boundary.
"""

import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc

type SummaryField = tuple[str, str]

CHECK_RUN_NAME = "CodeScene coverage"
REPORT_FIELDS: tuple[SummaryField, ...] = (
    ("Originating workflow run ID", "workflow_run_id"),
    ("Originating commit SHA", "commit_sha"),
    ("Artifact name", "artifact_name"),
    ("Download outcome", "download_outcome"),
    ("Download duration (ms)", "download_duration_ms"),
    ("Validation outcome", "validation_outcome"),
    ("Validation duration (ms)", "validation_duration_ms"),
    ("Submission outcome", "submission_outcome"),
    ("Submission duration (ms)", "submission_duration_ms"),
)
SUMMARY_FIELDS: tuple[SummaryField, ...] = (
    ("- Originating workflow run ID", "ORIGINATING_WORKFLOW_RUN_ID"),
    ("- Originating commit SHA", "ORIGINATING_COMMIT_SHA"),
    ("- Artifact name", "ARTIFACT_NAME"),
    ("- Download outcome", "ARTIFACT_DOWNLOAD_OUTCOME"),
    ("- Download duration (ms)", "ARTIFACT_DOWNLOAD_DURATION_MS"),
    ("- Validation outcome", "ARTIFACT_VALIDATION_OUTCOME"),
    ("- Validation duration (ms)", "ARTIFACT_VALIDATION_DURATION_MS"),
    ("- Submission outcome", "SUBMISSION_OUTCOME"),
    ("- Submission duration (ms)", "SUBMISSION_DURATION_MS"),
    ("- Check Run publication outcome", "CHECK_RUN_PUBLICATION_OUTCOME"),
    ("- Check Run publication duration (ms)", "CHECK_RUN_PUBLICATION_DURATION_MS"),
)


def report_values(value: cabc.Callable[[str], str]) -> dict[str, str]:
    """Return bounded correlation values for a same-repository Check Run."""
    environment_names = {
        "workflow_run_id": "ORIGINATING_WORKFLOW_RUN_ID",
        "commit_sha": "ORIGINATING_COMMIT_SHA",
        "artifact_name": "ARTIFACT_NAME",
        "download_outcome": "ARTIFACT_DOWNLOAD_OUTCOME",
        "validation_outcome": "ARTIFACT_VALIDATION_OUTCOME",
        "submission_outcome": "SUBMISSION_OUTCOME",
        "download_duration_ms": "ARTIFACT_DOWNLOAD_DURATION_MS",
        "validation_duration_ms": "ARTIFACT_VALIDATION_DURATION_MS",
        "submission_duration_ms": "SUBMISSION_DURATION_MS",
    }
    return {value_name: value(name) for value_name, name in environment_names.items()}


def summary(
    values: cabc.Mapping[str, str],
    fields: cabc.Iterable[SummaryField],
    conclusion: str,
) -> str:
    """Return fixed Check Run summary labels and bounded values."""
    lines = [f"{label}: {values[value_name]}" for label, value_name in fields]
    lines.append(f"Conclusion: {conclusion}")
    return "\n".join(lines)


def fork_summary(value: cabc.Callable[[str], str]) -> str:
    """Return the neutral, bounded summary used for an excluded fork."""
    values = {
        "workflow_run_id": value("ORIGINATING_WORKFLOW_RUN_ID"),
        "commit_sha": value("ORIGINATING_COMMIT_SHA"),
        "artifact_name": value("ARTIFACT_NAME"),
        "download_outcome": "skipped",
        "validation_outcome": "skipped",
        "submission_outcome": "skipped",
    }
    fields = (
        *REPORT_FIELDS[:3],
        ("Download outcome", "download_outcome"),
        ("Validation outcome", "validation_outcome"),
        ("Submission outcome", "submission_outcome"),
    )
    return summary(values, fields, "neutral")


def check_run_payload(
    value: cabc.Callable[[str], str], conclusion: str, report_summary: str
) -> dict[str, object]:
    """Return a completed Check Run payload for the originating commit."""
    return {
        "name": CHECK_RUN_NAME,
        "head_sha": value("ORIGINATING_COMMIT_SHA"),
        "external_id": value("ORIGINATING_WORKFLOW_RUN_ID"),
        "status": "completed",
        "conclusion": conclusion,
        "output": {"title": CHECK_RUN_NAME, "summary": report_summary},
    }


def workflow_summary(value: cabc.Callable[[str], str]) -> str:
    """Return the bounded GitHub Actions summary text without side effects."""
    lines = ["### CodeScene coverage", ""]
    lines.extend(f"{label}: {value(name)}" for label, name in SUMMARY_FIELDS)
    lines.append(f"- Conclusion: {value('CONCLUSION')}")
    return "\n".join(lines)
