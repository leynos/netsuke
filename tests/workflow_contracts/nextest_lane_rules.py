"""Hold every Linux job that runs the nextest suite to the pinned ``mold``.

The suite drives ``make`` recipes gated on ``check-build-tools``, which refuses
to run without the pinned ``mold`` on ``PATH``. That holds even where the
measured build never links with ``mold``: the coverage lanes assign
``RUSTFLAGS`` and so displace the linker flag, yet their tests still reach the
preflight. So every Linux job that runs the suite must run
``make install-build-tools`` in a step of its own, unguarded, before the suite.

Which jobs those are is derived, not listed. A job runs the suite when a step
calls the shared coverage action, runs ``cargo nextest run`` itself, or invokes
a Make goal whose recipe, directly or through its prerequisites, runs it. A job
is Linux when a ``runs-on`` label, in scalar, sequence or ``labels`` form, names
Ubuntu or Linux. Windows jobs are outside the rule: ``mold`` ships for Linux
only, and the configuration's ``cfg`` gate leaves the flag inert there.

Nothing here touches a file; the caller supplies parsed workflows and the
Makefile text. Run via ``make test-workflow-contracts``.
"""

from build_standard_predicates import (
    RULE_HEADER,
    installs_build_standard,
    invoked_make_targets,
    logical_lines,
    rule_prerequisites,
    step_name,
    uses_coverage_action,
)

#: Words in a runner label that place a job on Linux.
LINUX_MARKERS = ("ubuntu", "linux")

#: Words in a runner label that place a job off Linux, where `mold` is inert.
OTHER_PLATFORM_MARKERS = ("windows", "macos")


def runner_labels(job: dict[str, object]) -> list[str]:
    """Return a job's ``runs-on`` labels from any of its three forms.

    Returns
    -------
    list[str]
        The labels, or none when ``runs-on`` is absent or an unplaceable shape.

    Examples
    --------
    >>> runner_labels({"runs-on": "ubuntu-latest"})
    ['ubuntu-latest']
    >>> runner_labels({"runs-on": {"group": "big", "labels": ["linux", "x64"]}})
    ['linux', 'x64']

    """
    match job.get("runs-on"):
        case str() as label:
            return [label]
        case list() as labels:
            return [str(label) for label in labels]
        case {"labels": str() as label}:
            return [label]
        case {"labels": list() as labels}:
            return [str(label) for label in labels]
        case _:
            return []


def is_linux_job(job: dict[str, object]) -> bool:
    """Return whether any of a job's runner labels names Linux.

    Returns
    -------
    bool
        True when a label names Ubuntu or Linux.

    Examples
    --------
    >>> is_linux_job({"runs-on": ["self-hosted", "ubicloud-standard-4-ubuntu-2404"]})
    True
    >>> is_linux_job({"runs-on": "windows-latest"})
    False

    """
    return any(
        marker in label.casefold()
        for label in runner_labels(job)
        for marker in LINUX_MARKERS
    )


def is_unplaced_job(job: dict[str, object]) -> bool:
    """Return whether a job's runner cannot be placed on any platform.

    An expression such as ``${{ matrix.os }}`` may resolve to Linux, so a
    suite job on one is reported rather than exempted.

    Returns
    -------
    bool
        True when no label names Linux, Windows or macOS.

    Examples
    --------
    >>> is_unplaced_job({"runs-on": "${{ matrix.os }}"})
    True
    >>> is_unplaced_job({"runs-on": "windows-latest"})
    False

    """
    labels = [label.casefold() for label in runner_labels(job)]
    markers = LINUX_MARKERS + OTHER_PLATFORM_MARKERS
    return not any(marker in label for label in labels for marker in markers)


def _recipes(makefile: str) -> dict[str, str]:
    """Return each Make goal's recipe text, tab-indented lines joined."""
    recipes: dict[str, str] = {}
    goal = ""
    for line in makefile.splitlines():
        if line.startswith("\t") and goal:
            recipes[goal] = recipes.get(goal, "") + line + "\n"
            continue
        if not line.strip() or line.startswith("#"):
            # Make ignores blank and comment-only lines among recipe lines, so
            # they must not end the recipe being read.
            continue
        match = RULE_HEADER.match(line)
        goal = match.group("goal") if match else ""
    return recipes


def runs_nextest(script: str) -> bool:
    """Return whether a script runs ``cargo nextest run`` itself.

    Returns
    -------
    bool
        True when a command on some logical line is ``nextest run``.

    Examples
    --------
    >>> runs_nextest("$(CARGO) nextest run --workspace")
    True
    >>> runs_nextest("cargo nextest list")
    False

    """
    for line in logical_lines(script):
        words = line.split()
        for index, word in enumerate(words[:-1]):
            if word == "nextest" and words[index + 1] == "run":
                return True
    return False


def nextest_goals(makefile: str) -> frozenset[str]:
    """Return every Make goal that runs the nextest suite, however indirectly."""
    recipes = _recipes(makefile)
    prerequisites = rule_prerequisites(makefile)
    # A recipe running `$(MAKE) test-nextest` reaches the suite as surely as a
    # declared prerequisite does, so sub-make goals count as edges too.
    for goal, recipe in recipes.items():
        prerequisites.setdefault(goal, []).extend(invoked_make_targets(recipe))
    direct = {goal for goal, recipe in recipes.items() if runs_nextest(recipe)}
    return _widen(frozenset(direct), prerequisites)


def _widen(
    goals: frozenset[str], prerequisites: dict[str, list[str]]
) -> frozenset[str]:
    """Add every goal that needs one of ``goals``, until nothing more is added."""
    widened = goals | {
        goal for goal, needs in prerequisites.items() if goals.intersection(needs)
    }
    return widened if widened == goals else _widen(widened, prerequisites)


def runs_suite(step: dict[str, object], goals: frozenset[str]) -> bool:
    """Return whether a step runs the nextest suite."""
    if uses_coverage_action(step):
        return True
    match step.get("run"):
        case str() as script:
            return runs_nextest(script) or bool(invoked_make_targets(script) & goals)
        case _:
            return False


type Steps = list[dict[str, object]]


def _suite_jobs(
    documents: dict[str, dict[str, object]], goals: frozenset[str]
) -> list[tuple[str, Steps]]:
    """Return ("workflow:job", steps) for every Linux job running the suite."""
    lanes = ((where, _job_steps(job)) for where, job in _linux_jobs(documents))
    return [
        (where, steps)
        for where, steps in lanes
        if any(runs_suite(step, goals) for step in steps)
    ]


def _linux_jobs(
    documents: dict[str, dict[str, object]],
) -> list[tuple[str, dict[str, object]]]:
    """Return ("workflow:job", job) for every Linux job in every workflow."""
    return [
        (f"{name}:{job_id}", job)
        for name, document in documents.items()
        for job_id, job in _jobs_of(document).items()
        if is_linux_job(job)
    ]


def _jobs_of(document: dict[str, object]) -> dict[str, dict[str, object]]:
    """Return a workflow's well-formed jobs, dropping anything not a mapping."""
    jobs = document.get("jobs")
    if not isinstance(jobs, dict):
        return {}
    return {str(key): job for key, job in jobs.items() if isinstance(job, dict)}


def _job_steps(job: dict[str, object]) -> Steps:
    """Return a job's steps that are mappings; a reusable-workflow call has none."""
    steps = job.get("steps")
    if not isinstance(steps, list):
        return []
    return [step for step in steps if isinstance(step, dict)]


def suite_lanes(documents: dict[str, dict[str, object]], makefile: str) -> list[str]:
    """Return "workflow:job" for every Linux job with a step running the suite."""
    return [where for where, _ in _suite_jobs(documents, nextest_goals(makefile))]


def _lane_violations(where: str, steps: Steps, goals: frozenset[str]) -> list[str]:
    """Check one suite lane's install step against the rule."""
    first_suite = next(i for i, step in enumerate(steps) if runs_suite(step, goals))
    installs = [i for i, step in enumerate(steps) if installs_build_standard(step)]
    if not installs:
        return [
            f"{where}: runs the nextest suite but never runs `make install-build-tools`"
        ]
    install = installs[0]
    problems = []
    if install > first_suite:
        problems.append(
            f"{where}: installs the build standard only after "
            f"{step_name(steps[first_suite])!r} runs the suite"
        )
    if "if" in steps[install]:
        problems.append(f"{where}: `make install-build-tools` must not carry an `if:`")
    if steps[install].get("continue-on-error") not in {None, False}:
        problems.append(
            f"{where}: `make install-build-tools` must not continue on error"
        )
    return problems


def nextest_lane_violations(
    documents: dict[str, dict[str, object]], makefile: str
) -> list[str]:
    """Report every Linux suite lane that does not install ``mold`` first.

    A reading with no suite lane at all is itself a violation: the rule is a
    refusal, and a refusal over nothing would pass any repository.

    Returns
    -------
    list[str]
        One message per violation; empty when every lane complies.
    """
    goals = nextest_goals(makefile)
    if not goals:
        return ["the Makefile declares no goal that runs `cargo nextest run`"]
    lanes = _suite_jobs(documents, goals)
    if not lanes:
        return ["no Linux workflow job runs the nextest suite"]
    problems = [
        f"{where}: runs the nextest suite on a runner no label places"
        for where in _unplaced_suite_jobs(documents, goals)
    ]
    for where, steps in lanes:
        problems.extend(_lane_violations(where, steps, goals))
    return problems


def _unplaced_suite_jobs(
    documents: dict[str, dict[str, object]], goals: frozenset[str]
) -> list[str]:
    """Return "workflow:job" for every suite job whose runner is unplaced."""
    return [
        f"{name}:{job_id}"
        for name, document in documents.items()
        for job_id, job in _jobs_of(document).items()
        if is_unplaced_job(job)
        and any(runs_suite(step, goals) for step in _job_steps(job))
    ]
