"""Read optional per-job files at the offline evidence collector boundary.

Only absent optional files are omitted. Other filesystem failures propagate so
the CLI cannot misreport inaccessible evidence as unavailable evidence.
"""

import typing as typ

from ci_test_evidence_telemetry import load_inventory

if typ.TYPE_CHECKING:
    from pathlib import Path


def read_logs(log_dir: Path | None, jobs: list[dict]) -> dict[str, str]:
    """Load retained logs, omitting only files that are absent.

    Parameters
    ----------
    log_dir
        Directory containing one optional ``<job-id>.log`` file per job.
    jobs
        Jobs from the current saved attempt.

    Returns
    -------
    dict[str, str]
        Text logs keyed by job ID.

    """
    if log_dir is None:
        return {}

    logs = {}
    for job in jobs:
        path = log_dir / f"{job['id']}.log"
        try:
            logs[str(job["id"])] = path.read_text(encoding="utf-8")
        except FileNotFoundError:
            continue
    return logs


def read_inventories(
    telemetry_dir: Path | None, run: dict, jobs: list[dict]
) -> dict[str, object]:
    """Load optional inventories, omitting only files that are absent.

    Parameters
    ----------
    telemetry_dir
        Directory containing one optional ``<job-id>.json`` file per job.
    run
        Saved run response used to validate attempt identity.
    jobs
        Jobs from the current saved attempt.

    Returns
    -------
    dict[str, object]
        Validated inventory objects keyed by job ID.

    """
    if telemetry_dir is None:
        return {}

    inventories = {}
    for job in jobs:
        path = telemetry_dir / f"{job['id']}.json"
        try:
            inventories[str(job["id"])] = load_inventory(
                path, (run["id"], run["run_attempt"], job["id"])
            )
        except FileNotFoundError:
            continue
    return inventories
