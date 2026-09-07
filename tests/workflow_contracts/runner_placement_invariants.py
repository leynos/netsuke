"""Define pure validation rules for Netsuke's runner placement and caching.

These helpers are owned by the workflow-contract suites. They accept normalized
step, runner-assignment, or cache-ownership data, perform no YAML input/output,
and should not be used by production code. Checked-in workflow tests and
generated property tests share them so both forms enforce one contract.

The placement policy they encode is: Ubicloud runs the Linux jobs that block a
developer, GitHub-hosted runners take everything else. Ubicloud offers Linux
runners only, so Windows and macOS have no Ubicloud option; scheduled,
delayed-comment, and administrative jobs are API-bound and gain nothing from a
build shape.
"""

NINJA_ACTION_REPOSITORY = "seanmiddleditch/gha-setup-ninja@"
NINJA_ACTION = (
    "seanmiddleditch/gha-setup-ninja@3b1f8f94a2f8254bd26914c4ab9474d4f0015f67"
)
WINDOWS_PATH_STEP_NAME = "Expose Windows global tool path"
WINDOWS_PACKAGE_ACTION = "leynos/shared-actions/.github/actions/windows-package@"
CALLER_SELECTED_RUNNER = "${{ inputs.runner }}"
CALLEE_SELECTED_RUNNER = "<callee-selected>"

WINDOWS_PATH_FRAGMENTS = (
    "[Environment]::GetFolderPath([Environment+SpecialFolder]::UserProfile)",
    "'.dotnet'",
    "'tools'",
    "$Env:GITHUB_PATH",
)

#: The Ubicloud label for the default Linux lane. `ubicloud-standard-2` alone
#: would also select Ubuntu 24.04 today, but naming the image keeps a change to
#: Ubicloud's default from silently moving compiled tools onto another glibc.
UBICLOUD_DEFAULT_LABEL = "ubicloud-standard-2-ubuntu-2404"
#: The escalated shape, carried by the two jobs that run the instrumented
#: build. The recipe treats
#: `-4` as a ceiling reached on evidence: the two-vCPU shape lost its runner
#: mid-instrumented-build, and both jobs now sample memory so the next review
#: either confirms the escalation or returns them to `-2`.
INSTRUMENTED_BUILD_JOBS = (
    ("ci.yml", "build-test"),
    ("coverage-main.yml", "coverage-upload"),
)
UBICLOUD_LARGE_LABEL = "ubicloud-standard-4-ubuntu-2404"
#: The deliberate Ubuntu 22.04 compatibility lane.
UBICLOUD_COMPAT_LABEL = "ubicloud-standard-2-ubuntu-2204"
UBICLOUD_LABELS = (
    UBICLOUD_DEFAULT_LABEL,
    UBICLOUD_LARGE_LABEL,
    UBICLOUD_COMPAT_LABEL,
)

#: vCPU count of every runner shape the repository selects. Worker bounds are
#: derived from these numbers, never chosen independently.
LANE_VCPUS = {
    UBICLOUD_DEFAULT_LABEL: 2,
    UBICLOUD_LARGE_LABEL: 4,
    UBICLOUD_COMPAT_LABEL: 2,
    "windows-latest": 4,
}

REQUIRED_RUNNER_ASSIGNMENTS = {
    "ci.build-test": UBICLOUD_LARGE_LABEL,
    "ci-windows.build-test-windows": "windows-latest",
    "ci-windows.windows-native-recipe-smoke": "windows-latest",
    "ci.kani-smoke": UBICLOUD_DEFAULT_LABEL,
    "coverage-main.coverage-upload": UBICLOUD_LARGE_LABEL,
    "coverage-pr-submit.submit-coverage": UBICLOUD_DEFAULT_LABEL,
    "delayed-pr-comment.delay_and_comment": "ubuntu-latest",
    "netsukefile-test.netsukefile": UBICLOUD_COMPAT_LABEL,
    "release.metadata": "ubuntu-latest",
    "release.build-linux": UBICLOUD_DEFAULT_LABEL,
    "release.build-windows": "windows-latest",
    "release.macos.x86_64-apple-darwin": "macos-15-intel",
    "release.macos.aarch64-apple-darwin": "macos-15",
    "release.windows-native-recipe-smoke": "windows-latest",
    "release.release": "ubuntu-latest",
    "build-and-package.build": CALLER_SELECTED_RUNNER,
    "mutation-testing.mutation": CALLEE_SELECTED_RUNNER,
    "dependabot-automerge.automerge": CALLEE_SELECTED_RUNNER,
}

UBICLOUD_ASSIGNMENT_KEYS = tuple(
    key
    for key, runner in REQUIRED_RUNNER_ASSIGNMENTS.items()
    if runner in UBICLOUD_LABELS
)

#: Jobs that must never reach a Ubicloud label. Windows and macOS have no
#: Ubicloud image; the rest are API-bound or not developer-blocking.
GITHUB_HOSTED_ONLY_KEYS = (
    "ci-windows.build-test-windows",
    "ci-windows.windows-native-recipe-smoke",
    "delayed-pr-comment.delay_and_comment",
    "release.metadata",
    "release.build-windows",
    "release.macos.x86_64-apple-darwin",
    "release.macos.aarch64-apple-darwin",
    "release.windows-native-recipe-smoke",
    "release.release",
)


def is_ubicloud_label(runner: str) -> bool:
    """Return whether a runner label selects a Ubicloud VM.

    Parameters
    ----------
    runner
        The runner label a job or action assigns, such as
        ``"ubicloud-standard-2-ubuntu-2404"`` or ``"windows-latest"``.

    Returns
    -------
    bool
        ``True`` when the label names a Ubicloud-hosted shape.
    """
    return runner.startswith("ubicloud-")


def is_valid_ninja_sequence(
    steps: list[dict[str, object]], first_consumer: str
) -> bool:
    """Return whether one pinned Ninja setup precedes the first consumer.

    Parameters
    ----------
    steps
        The job's normalized workflow steps, in declaration order.
    first_consumer
        The ``name`` of the first step that requires Ninja on the ``PATH``.

    Returns
    -------
    bool
        ``True`` when exactly one step installs the pinned Ninja action and
        that step precedes every step named ``first_consumer``.

    Notes
    -----
    A missing, duplicated, or late setup step leaves a consumer running
    against an unpinned or absent Ninja binary, so the sequence is checked
    structurally rather than merely for presence.
    """
    setup_indices = [
        index
        for index, step in enumerate(steps)
        if str(step.get("uses", "")).startswith(NINJA_ACTION_REPOSITORY)
    ]
    consumer_indices = [
        index for index, step in enumerate(steps) if step.get("name") == first_consumer
    ]
    return bool(
        len(setup_indices) == 1
        and steps[setup_indices[0]].get("uses") == NINJA_ACTION
        and consumer_indices
        and setup_indices[0] < min(consumer_indices)
    )


def is_valid_windows_tool_path_sequence(
    steps: list[dict[str, object]],
) -> bool:
    """Return whether one guarded PowerShell path setup precedes packaging.

    Parameters
    ----------
    steps
        The job's normalized workflow steps, in declaration order.

    Returns
    -------
    bool
        ``True`` when exactly one step configures the Windows tool path, that
        step matches the required guard, shell, and script fragments, and it
        precedes the Windows packaging step.

    Notes
    -----
    Packaging before the tool path is exposed leaves the installer built
    against a `PATH` that omits the `.NET` global tool directory, so the
    ordering is enforced structurally alongside the step's own shape.
    """
    path_indices = [
        index for index, step in enumerate(steps) if _is_windows_path_candidate(step)
    ]
    package_indices = [
        index
        for index, step in enumerate(steps)
        if str(step.get("uses", "")).startswith(WINDOWS_PACKAGE_ACTION)
    ]
    if len(path_indices) != 1 or len(package_indices) != 1:
        return False

    path_index = path_indices[0]
    return _is_valid_windows_path_step(steps[path_index]) and (
        path_index < package_indices[0]
    )


def has_required_runner_assignments(assignments: dict[str, str]) -> bool:
    """Return whether every owned and delegated runner keeps its assignment.

    Parameters
    ----------
    assignments
        A ``job.step`` (or ``workflow.job``) key mapped to the runner label
        it was placed on, gathered from the checked-in workflows.

    Returns
    -------
    bool
        ``True`` when ``assignments`` is exactly equal to
        :data:`REQUIRED_RUNNER_ASSIGNMENTS`: every required key is present
        with its required runner, and no extra or missing key remains.
    """
    return dict(assignments) == REQUIRED_RUNNER_ASSIGNMENTS


def has_single_cache_owner(owners: list[tuple[str, str]]) -> bool:
    """Return whether no cached path is claimed by more than one step.

    Parameters
    ----------
    owners
        ``(step identifier, cached path)`` pairs drawn from one workflow's
        cache steps.

    Returns
    -------
    bool
        ``True`` when every path appears at most once.

    Notes
    -----
    Two writers make cache warmth, eviction, and storage cost impossible to
    reason about, which is why the rule is structural rather than advisory.
    """
    paths = [path for _, path in owners]
    return len(paths) == len(set(paths))


def is_bounded_worker_count(vcpus: int, flags: dict[str, str]) -> bool:
    """Return whether every worker flag stays within the lane's vCPU count.

    Parameters
    ----------
    vcpus
        vCPU count of the runner shape the job was placed on.
    flags
        Worker-count environment variables mapped to their literal values,
        such as ``{"BUILD_JOBS": "-j 2"}``.

    Returns
    -------
    bool
        ``True`` when every flag names a positive count no greater than
        ``vcpus``.
    """
    counts = [_trailing_count(value) for value in flags.values()]
    return bool(counts) and all(
        count is not None and 0 < count <= vcpus for count in counts
    )


def _contains_unquoted_or(expression: str) -> bool:
    """Report whether `||` appears anywhere outside a quoted literal."""
    quote: str | None = None
    index = 0
    length = len(expression)
    while index < length:
        char = expression[index]
        if quote is not None:
            if char == quote:
                quote = None
        elif char in "'\"":
            quote = char
        elif expression[index : index + 2] == "||":
            return True
        index += 1
    return False


def is_trunk_only_save(condition: str) -> bool:
    """Return whether a cache-save condition is restricted to a trunk push.

    Parameters
    ----------
    condition
        The step's ``if`` expression, as written in the workflow or action.

    Returns
    -------
    bool
        ``True`` when the condition names both a ``push`` event and the
        ``main`` ref, and contains no disjunction that could authorise a save
        without both.

    Notes
    -----
    A pull request that saved would race the designated writer for the
    reservation and publish a generation no reviewer has seen. Naming both
    predicates is not enough: a disjunction of the two, such as
    ``github.event_name == 'push' || github.ref == 'refs/heads/main'``, lets
    either authorise a save alone.

    Any ``||`` outside a quoted literal is rejected, at every depth rather
    than only at the top level. Checking the top level alone was not enough:
    wrapping the whole disjunction in parentheses, as
    ``(github.event_name == 'push' || github.ref == 'refs/heads/main')``,
    leaves no top-level ``||`` to find while meaning exactly the same thing.
    Rejecting outright rather than analysing the expression's structure is
    deliberate. Every cache-save condition in this repository is a conjunction
    of equality tests, so nothing legitimate needs a disjunction, and a
    predicate that tried to decide which disjunctions are safe would be a
    second, subtler thing to get wrong.
    """
    normalized = " ".join(condition.split())
    if _contains_unquoted_or(normalized):
        return False
    return (
        "github.event_name == 'push'" in normalized
        and "github.ref == 'refs/heads/main'" in normalized
    )


def _trailing_count(value: str) -> int | None:
    """Return the worker count a flag ends with, or ``None`` when absent."""
    tail = value.strip().rsplit(" ", 1)[-1] if " " in value.strip() else value.strip()
    return int(tail) if tail.isdigit() else None


def _is_windows_path_candidate(step: dict[str, object]) -> bool:
    """Identify a step that appears to configure the Windows tool path."""
    script = str(step.get("run", ""))
    return bool(
        step.get("name") == WINDOWS_PATH_STEP_NAME
        or (
            "$Env:GITHUB_PATH" in script
            and ("SpecialFolder" in script or ".dotnet" in script)
        )
    )


def _is_valid_windows_path_step(step: dict[str, object]) -> bool:
    """Validate the guarded PowerShell known-folder path setup itself."""
    script = str(step.get("run", ""))
    return bool(
        step.get("name") == WINDOWS_PATH_STEP_NAME
        and step.get("if") == "inputs.platform == 'windows'"
        and step.get("shell") == "pwsh"
        and all(fragment in script for fragment in WINDOWS_PATH_FRAGMENTS)
    )
