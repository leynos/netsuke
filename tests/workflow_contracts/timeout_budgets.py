"""Reading the timers that can end a test run.

The contract in :mod:`timeout_ordering_test` compares budgets written
down in three different files. Turning those files into comparable
seconds is the part that can be wrong without any file being wrong, so it
lives here where it can be read on its own.

The reading that matters most is the per-test one. nextest warns once per
``period`` and terminates after ``terminate-after`` of them, so the budget
is their product. Every period in this repository is 60 s, so a reader
taking the period alone would report a 60 s allowance where the real
figure is 300 s, or 600 s for the two Windows overrides.

See "Test timeouts: the tiers this repository sets" in
``docs/developers-guide.md``.
"""

import re
import typing as typ

import yaml
from workflow_loading import REPO_ROOT

#: The environment variable the shared coverage action reads for its
#: wall-clock cap on one `cargo` invocation.
WATCHDOG_VARIABLE: typ.Final[str] = "RUN_RUST_CARGO_WAIT_TIMEOUT"

#: The action whose steps run under that watchdog.
COVERAGE_ACTION: typ.Final[str] = (
    "leynos/shared-actions/.github/actions/generate-coverage"
)

#: Everything in a coverage job that is not the `cargo` invocation the
#: watchdog bounds: checkout, toolchain setup, cache restore, linting, and
#: whatever follows the coverage step. The job timer covers it; the
#: watchdog does not.
#:
#: Measured from the worst of several runs rather than one. Across twelve
#: successful `ci.yml` runs the widest gap between the coverage step and
#: its job was 384 s on run 34047430187; across twelve of
#: `coverage-main.yml` it was 53 s on run 33809357448. Fifteen minutes
#: covers the worse of those, and none of those runs was genuinely cold.
OUTSIDE_WATCHDOG_ALLOWANCE_SECONDS: typ.Final[float] = 15 * 60.0

#: Build time inside the `cargo` invocation, before nextest starts its own
#: clock. Only used if a `global-timeout` appears: the watchdog must cover
#: it as well as the whole-run budget.
COLD_BUILD_ALLOWANCE_SECONDS: typ.Final[float] = 10 * 60.0

#: Floor for the termination allowance, used when the configuration sets
#: no grace period, as this one does not. Generous against nextest's
#: ten-second default and far too small to hide a real overrun.
MINIMUM_TERMINATION_ALLOWANCE_SECONDS: typ.Final[float] = 60.0

NEXTEST_CONFIG = REPO_ROOT / ".config" / "nextest.toml"
WORKFLOWS_DIRECTORY = REPO_ROOT / ".github" / "workflows"

_DURATION: typ.Final[re.Pattern[str]] = re.compile(
    r"^\s*(?P<value>\d+(?:\.\d+)?)\s*(?P<unit>ms|s|m|h)\s*$"
)

_UNIT_SECONDS: typ.Final[dict[str, float]] = {
    "ms": 0.001,
    "s": 1.0,
    "m": 60.0,
    "h": 3600.0,
}

#: One `slow-timeout` inline table, captured whole so the period and the
#: multiplier that scales it are read together. Reading `period` alone
#: would understate the budget by whatever `terminate-after` says.
_SLOW_TIMEOUT: typ.Final[re.Pattern[str]] = re.compile(
    r"slow-timeout\s*=\s*\{(?P<body>[^}]*)\}"
)

_GRACE_PERIOD: typ.Final[re.Pattern[str]] = re.compile(r'grace-period\s*=\s*"([^"]+)"')


def seconds(duration: str) -> float:
    """Convert a nextest duration to seconds.

    Parameters
    ----------
    duration : str
        A duration as nextest spells it, such as ``"60s"``.

    Returns
    -------
    float
        The duration in seconds.
    """
    match = _DURATION.match(duration)
    assert match is not None, f"unrecognized nextest duration {duration!r}"
    return float(match["value"]) * _UNIT_SECONDS[match["unit"]]


def largest_test_allowance(config_text: str) -> float:
    """Return the longest a single test may run, in seconds.

    nextest warns once per ``period`` and terminates after
    ``terminate-after`` of them, so the budget is their product. This
    repository sets five on Linux and ten on Windows against a 60 s
    period, so reading the period alone would understate the largest
    allowance by a factor of ten.

    Parameters
    ----------
    config_text : str
        The nextest configuration file's text.

    Returns
    -------
    float
        The longest per-test budget.
    """
    budgets: list[float] = []
    for match in _SLOW_TIMEOUT.finditer(config_text):
        body = match["body"]
        period = re.search(r'period\s*=\s*"([^"]+)"', body)
        assert period is not None, f"slow-timeout without a period: {body!r}"
        terminate = re.search(r"terminate-after\s*=\s*(\d+)", body)
        multiplier = 1 if terminate is None else int(terminate[1])
        budgets.append(seconds(period[1]) * multiplier)
    assert budgets, "nextest.toml must set at least one slow-timeout"
    return max(budgets)


def termination_allowance(config_text: str) -> float:
    """Return the time nextest may take to stop the run, in seconds.

    Hitting the global timeout starts nextest's ordinary termination
    procedure rather than stopping the run: on Unix it signals the process
    group and waits ``slow-timeout.grace-period`` before killing it; on
    Windows termination is immediate and the grace period is ignored for
    timeouts. Read from the configuration rather than fixed, so a profile
    that raised its grace period raises the requirement too. This file
    sets none, so the floor applies.

    Parameters
    ----------
    config_text : str
        The nextest configuration file's text.

    Returns
    -------
    float
        The largest configured grace period, or the floor when that is
        smaller or absent.
    """
    periods = _GRACE_PERIOD.findall(config_text)
    largest = max((seconds(period) for period in periods), default=0.0)
    return max(largest, MINIMUM_TERMINATION_ALLOWANCE_SECONDS)


def global_timeout(config_text: str) -> float | None:
    """Return the whole-run budget, or None when none is set.

    Parameters
    ----------
    config_text : str
        The nextest configuration file's text.

    Returns
    -------
    float or None
        The whole-run budget in seconds, or None.
    """
    match = re.search(r'^global-timeout\s*=\s*"([^"]+)"', config_text, re.MULTILINE)
    return None if match is None else seconds(match[1])


class CoverageLane(typ.NamedTuple):
    """One coverage step, with the budgets around it.

    Attributes
    ----------
    workflow : str
        The workflow file's name.
    job : str
        The job the step belongs to.
    step : str
        The step's declared name.
    watchdog : float or None
        The watchdog budget in seconds, or None when the job sets none
        and so inherits the action's 1,800 s default.
    job_timeout : float or None
        The job's ``timeout-minutes`` in seconds, or None when it
        declares none and so inherits GitHub's six-hour default.
    """

    workflow: str
    job: str
    step: str
    watchdog: float | None
    job_timeout: float | None

    def __str__(self) -> str:
        """Return a location suitable for a failure message.

        Returns
        -------
        str
            ``workflow:job:step`` for this lane.
        """
        return f"{self.workflow}:{self.job}:{self.step!r}"


def _watchdog_of(job: dict[str, typ.Any], step: dict[str, typ.Any]) -> float | None:
    """Return the watchdog budget in force for one step.

    A step's own environment wins over the job's, as GitHub resolves it,
    so a lane that overrode the job value is read as it will run rather
    than as the job declares.

    Parameters
    ----------
    job : dict[str, typ.Any]
        The enclosing job.
    step : dict[str, typ.Any]
        The coverage step.

    Returns
    -------
    float or None
        The budget in seconds, or None when neither sets one.
    """
    for source in ((step.get("env") or {}), (job.get("env") or {})):
        raw = source.get(WATCHDOG_VARIABLE)
        if raw is not None:
            return float(str(raw))
    return None


def _workflow_documents() -> dict[str, dict[str, typ.Any]]:
    """Return every workflow document, keyed by file name.

    Both extensions are read. A coverage lane in the other one would
    otherwise escape every assertion below without failing anything.

    Returns
    -------
    dict[str, dict[str, typ.Any]]
        File name to parsed document.
    """
    documents: dict[str, dict[str, typ.Any]] = {}
    for pattern in ("*.yml", "*.yaml"):
        for path in sorted(WORKFLOWS_DIRECTORY.glob(pattern)):
            parsed = yaml.safe_load(path.read_text(encoding="utf-8"))
            if isinstance(parsed, dict):
                documents[path.name] = parsed
    return documents


def coverage_lanes_of() -> tuple[CoverageLane, ...]:
    """Return every step invoking the coverage action, with its budgets.

    Every such step is included, not only those whose job sets a
    watchdog, so a lane that lost its override is visible as ``None``
    rather than absent. An absent entry would make the assertions skip it
    silently and restore the action's default.

    Returns
    -------
    tuple[CoverageLane, ...]
        One entry per coverage step.
    """
    lanes: list[CoverageLane] = []
    for name, document in _workflow_documents().items():
        for job_name, job in (document.get("jobs") or {}).items():
            if not isinstance(job, dict):
                continue
            raw_timeout = job.get("timeout-minutes")
            timeout = None if raw_timeout is None else float(raw_timeout) * 60.0
            for step in job.get("steps") or []:
                if not isinstance(step, dict):
                    continue
                if COVERAGE_ACTION not in str(step.get("uses", "")):
                    continue
                lanes.append(
                    CoverageLane(
                        workflow=name,
                        job=str(job_name),
                        step=str(step.get("name", "")) or str(job_name),
                        watchdog=_watchdog_of(job, step),
                        job_timeout=timeout,
                    )
                )
    return tuple(lanes)
