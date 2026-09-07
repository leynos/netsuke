"""Reading the timers that can end a test run.

The contract in :mod:`timeout_ordering_test` compares budgets written
down in three different files.

The values live here and the reading of `.config/nextest.toml` lives in
``nextest_budgets``, so neither module carries the whole contract and
neither outgrows the 400-line limit the lint gate enforces.

See "Test timeouts: the tiers this repository sets" in
``docs/developers-guide.md``.
"""

import typing as typ

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

#: How far a ceiling must sit above the sum it contains, rather than
#: merely reaching it. A ceiling equal to that sum cancels the job at
#: the moment the watchdog would have reported the overrun, and the
#: report is the only thing that makes an overrun actionable.
CEILING_MARGIN_SECONDS: typ.Final[float] = 15 * 60.0

#: What nextest allows a test between `SIGTERM` and `SIGKILL` when the
#: configuration names no `grace-period`, as this one does not.
NEXTEST_DEFAULT_GRACE_PERIOD_SECONDS: typ.Final[float] = 10.0

#: Added to that grace period to cover the teardown and report writing
#: that follow it. A separate term rather than a floor over the two, so
#: raising a grace period raises the requirement instead of vanishing
#: into it.
TERMINATION_SAFETY_MARGIN_SECONDS: typ.Final[float] = 60.0

NEXTEST_CONFIG = REPO_ROOT / ".config" / "nextest.toml"
WORKFLOWS_DIRECTORY = REPO_ROOT / ".github" / "workflows"
