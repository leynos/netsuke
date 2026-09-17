"""Build bounded mutations of valid runner-placement and cache inputs.

These helpers exist solely to support
``runner_placement_properties_test.py``: each one takes a mutation name and
returns a deliberately broken copy of a valid runner-assignment mapping,
worker-flag set, or cache-save condition, so the property tests can assert
that the validators in `runner_placement_invariants` reject every mutation
but the identity one. They perform no YAML input/output and are not used by
production code.

This module holds the mutation generators only, kept separate from the test
module so `runner_placement_properties_test.py` stays within the
repository's 400-line file cap. It contains no tests of its own.
"""

from fork_fallback import (
    FORK_FALLBACK_KEYS,
    FORK_FALLBACK_RUNNER,
    FORK_FALLBACK_RUNNERS,
    FORK_GUARD,
)
from runner_placement_invariants import (
    REQUIRED_RUNNER_ASSIGNMENTS,
    UBICLOUD_COMPAT_LABEL,
    UBICLOUD_DEFAULT_LABEL,
)


def _apply_github_hosted_mutation(
    assignments: dict[str, str], selected_key: str, expected: str
) -> None:
    """Replace the selected assignment with a GitHub-hosted runner."""
    assignments[selected_key] = _github_hosted_runner_for(selected_key)


def _apply_wrong_ubicloud_image_mutation(
    assignments: dict[str, str], selected_key: str, expected: str
) -> None:
    """Swap the selected assignment to the other Ubicloud image."""
    assignments[selected_key] = (
        UBICLOUD_COMPAT_LABEL
        if expected == UBICLOUD_DEFAULT_LABEL
        else UBICLOUD_DEFAULT_LABEL
    )


def _apply_swapped_platforms_mutation(
    assignments: dict[str, str], selected_key: str, expected: str
) -> None:
    """Swap the Linux and Windows `build-*` runner assignments."""
    linux_key = "release.build-linux"
    windows_key = "release.build-windows"
    assignments[linux_key], assignments[windows_key] = (
        assignments[windows_key],
        assignments[linux_key],
    )


def _apply_intel_macos_replaced_mutation(
    assignments: dict[str, str], selected_key: str, expected: str
) -> None:
    """Replace the Intel macOS assignment with the Apple Silicon runner."""
    assignments["release.macos.x86_64-apple-darwin"] = "macos-15"


#: Maps each non-identity mutation to the helper that applies it in place.
_RUNNER_ASSIGNMENT_MUTATIONS = {
    "github-hosted": _apply_github_hosted_mutation,
    "wrong-ubicloud-image": _apply_wrong_ubicloud_image_mutation,
    "swapped-platforms": _apply_swapped_platforms_mutation,
    "intel-macos-replaced": _apply_intel_macos_replaced_mutation,
}


def mutate_runner_assignments(mutation: str, selected_key: str) -> dict[str, str]:
    """Apply one bounded runner-assignment mutation to the valid mapping.

    Parameters
    ----------
    mutation
        The mutation name: ``"valid"`` for no change, or one of the keys in
        the runner-assignment mutation table (``"github-hosted"``,
        ``"wrong-ubicloud-image"``, ``"swapped-platforms"``, or
        ``"intel-macos-replaced"``).
    selected_key
        The ``job.step`` key the mutation targets. Ignored by mutations,
        such as ``"swapped-platforms"``, that always target fixed keys.

    Returns
    -------
    dict[str, str]
        A copy of
        :data:`~runner_placement_invariants.REQUIRED_RUNNER_ASSIGNMENTS`
        with the named mutation applied, or an unmodified copy when
        ``mutation`` is ``"valid"`` or unrecognized.
    """
    assignments = dict(REQUIRED_RUNNER_ASSIGNMENTS)
    apply_mutation = _RUNNER_ASSIGNMENT_MUTATIONS.get(mutation)
    if apply_mutation is not None:
        apply_mutation(assignments, selected_key, assignments[selected_key])
    return assignments


def _github_hosted_runner_for(selected_key: str) -> str:
    """Choose a bounded hosted-runner mutation for the selected platform."""
    if "windows" in selected_key:
        return "windows-latest"
    if "macos" in selected_key:
        return "macos-15"
    return "ubuntu-latest"


def _worker_flags_for_count(count: int) -> dict[str, str]:
    """Build the three worker-count flags at a literal count value."""
    return {
        "BUILD_JOBS": f"-j {count}",
        "NEXTEST_BUILD_JOBS": f"--build-jobs {count}",
        "NEXTEST_TEST_JOBS": f"-j {count}",
    }


def _oversubscribed_worker_flags(vcpus: int) -> dict[str, str]:
    """Build a worker-flag set one worker over the lane's vCPU count."""
    return _worker_flags_for_count(vcpus + 1)


def _zero_worker_flags(vcpus: int) -> dict[str, str]:
    """Build a worker-flag set requesting zero workers."""
    return _worker_flags_for_count(0)


def _unbounded_worker_flags(vcpus: int) -> dict[str, str]:
    """Build a worker-flag set with no numeric bound at all."""
    return {"BUILD_JOBS": "-j auto", "NEXTEST_TEST_JOBS": "-j auto"}


#: Maps each non-identity mutation to the helper that builds its flag set.
_WORKER_FLAG_MUTATIONS = {
    "oversubscribed": _oversubscribed_worker_flags,
    "zero": _zero_worker_flags,
    "unbounded": _unbounded_worker_flags,
}


def mutate_worker_flags(mutation: str, vcpus: int) -> dict[str, str]:
    """Apply one bounded worker-count mutation to the valid flag set.

    Parameters
    ----------
    mutation
        The mutation name: ``"valid"`` for no change, or one of
        ``"oversubscribed"``, ``"zero"``, or ``"unbounded"``.
    vcpus
        vCPU count of the runner shape the worker flags are built for.

    Returns
    -------
    dict[str, str]
        The worker-count environment variables produced for the named
        mutation, saturating ``vcpus`` when ``mutation`` is ``"valid"``.
    """
    # The unnamed default is the unmutated set, which saturates the lane
    # exactly: `_worker_flags_for_count` at the lane's own vCPU count.
    build_flags = _WORKER_FLAG_MUTATIONS.get(mutation, _worker_flags_for_count)
    return build_flags(vcpus)


def mutate_save_condition(mutation: str) -> str:
    """Apply one bounded mutation to the trunk-only cache-save condition.

    Parameters
    ----------
    mutation
        The mutation name: ``"valid"`` for the correctly guarded condition,
        or one of ``"any-push"``, ``"any-branch"``, ``"unconditional"``,
        ``"disjunctive"``, or ``"parenthesised-disjunctive"``.

    Returns
    -------
    str
        The workflow ``if`` expression produced for the named mutation.
    """
    match mutation:
        case "any-push":
            return "github.event_name == 'push'"
        case "any-branch":
            return "github.ref == 'refs/heads/main'"
        case "unconditional":
            return "always()"
        case "disjunctive":
            return "github.event_name == 'push' || github.ref == 'refs/heads/main'"
        case "parenthesised-disjunctive":
            # Means exactly what the bare disjunction means, but leaves no
            # top-level `||` for a depth-zero scan to find.
            return "(github.event_name == 'push' || github.ref == 'refs/heads/main')"
        case _:
            return (
                "github.event_name == 'push' && github.ref == 'refs/heads/main' "
                "&& steps.caches.outputs.registry-hit != 'true'"
            )


def _declaration(guard: str, fork: str, owned: str) -> str:
    """Render a placement expression from its guard and two arms."""
    return f"${{{{ {guard} && '{fork}' || '{owned}' }}}}"


def valid_fork_fallback_declarations() -> dict[str, str]:
    """Return the `runs-on` every lane should declare, as written.

    A pull-request lane gets the expression over the runner the assignment
    table records for it; every other lane gets that runner outright.

    Returns
    -------
    dict[str, str]
        Assignment key to the declaration that lane should carry.
    """
    return {
        key: (
            _declaration(FORK_GUARD, FORK_FALLBACK_RUNNERS[key], runner)
            if key in FORK_FALLBACK_KEYS
            else runner
        )
        for key, runner in REQUIRED_RUNNER_ASSIGNMENTS.items()
    }


def _apply_arm_dropped_mutation(declarations: dict[str, str], key: str) -> None:
    """Replace a pull-request lane's expression with its owned arm alone."""
    declarations[key] = REQUIRED_RUNNER_ASSIGNMENTS[key]


def _apply_guard_swapped_mutation(declarations: dict[str, str], key: str) -> None:
    """Branch on a sibling field that parses and evaluates just as well."""
    sibling = FORK_GUARD.rsplit(".", 1)[0] + ".private"
    declarations[key] = _declaration(
        sibling, FORK_FALLBACK_RUNNERS[key], REQUIRED_RUNNER_ASSIGNMENTS[key]
    )


def _apply_arms_swapped_mutation(declarations: dict[str, str], key: str) -> None:
    """Send the fork to Ubicloud, which is the runner it cannot obtain."""
    declarations[key] = _declaration(
        FORK_GUARD, REQUIRED_RUNNER_ASSIGNMENTS[key], FORK_FALLBACK_RUNNERS[key]
    )


def _apply_wrong_fork_runner_mutation(declarations: dict[str, str], key: str) -> None:
    """Send the fork to a hosted runner of the wrong platform.

    The owned arm stays correct, so only the fork arm is wrong. Without this
    the fork-arm check is dead: swapping the arms or hosting both of them is
    caught by the owned arm instead, and dropping the check changes nothing any
    case can see.
    """
    declarations[key] = _declaration(
        FORK_GUARD, "windows-latest", REQUIRED_RUNNER_ASSIGNMENTS[key]
    )


def _apply_hosted_on_both_arms_mutation(declarations: dict[str, str], key: str) -> None:
    """Fall back on both arms, so the lane quietly stops using Ubicloud."""
    declarations[key] = _declaration(
        FORK_GUARD, FORK_FALLBACK_RUNNERS[key], FORK_FALLBACK_RUNNERS[key]
    )


def _apply_arm_where_no_fork_reaches_mutation(
    declarations: dict[str, str], key: str
) -> None:
    """Give the coverage upload lane an arm no event can ever take."""
    target = "coverage-main.coverage-upload"
    declarations[target] = _declaration(
        FORK_GUARD, FORK_FALLBACK_RUNNER, REQUIRED_RUNNER_ASSIGNMENTS[target]
    )


def _apply_wrong_fork_image_mutation(declarations: dict[str, str], key: str) -> None:
    """Send the compatibility lane's fork to the current hosted image.

    `ubuntu-latest` is hosted, is Linux, and is the right answer for every
    other lane, so the platform check and the owned-arm check both pass. Only a
    per-lane expectation separates it from `ubuntu-22.04`. Without this case
    the mapping is dead: one shared constant reads identically over every lane
    the repository actually declares.
    """
    target = "netsukefile-test.netsukefile"
    declarations[target] = _declaration(
        FORK_GUARD, FORK_FALLBACK_RUNNER, REQUIRED_RUNNER_ASSIGNMENTS[target]
    )


def _apply_line_break_mutation(declarations: dict[str, str], key: str) -> None:
    """Indent the continuation deeper, which keeps the break in the value."""
    declarations[key] = _declaration(
        FORK_GUARD, FORK_FALLBACK_RUNNERS[key], REQUIRED_RUNNER_ASSIGNMENTS[key]
    ).replace(" && ", "\n&& ", 1)


#: Maps each non-identity fork-fallback mutation to the helper that applies it.
_FORK_FALLBACK_MUTATIONS = {
    "arm-dropped": _apply_arm_dropped_mutation,
    "guard-swapped": _apply_guard_swapped_mutation,
    "arms-swapped": _apply_arms_swapped_mutation,
    "wrong-fork-runner": _apply_wrong_fork_runner_mutation,
    "hosted-on-both-arms": _apply_hosted_on_both_arms_mutation,
    "arm-where-no-fork-reaches": _apply_arm_where_no_fork_reaches_mutation,
    "wrong-fork-image": _apply_wrong_fork_image_mutation,
    "line-break": _apply_line_break_mutation,
}


def mutate_fork_fallback_declarations(mutation: str, key: str) -> dict[str, str]:
    """Apply one bounded fork-fallback mutation to the valid declarations.

    Parameters
    ----------
    mutation
        The mutation name, or ``"valid"`` for no change.
    key
        The pull-request lane the mutation targets. Ignored by mutations that
        always target a fixed lane.

    Returns
    -------
    dict[str, str]
        Assignment key to the job's ``runs-on`` as written, with the named
        mutation applied.
    """
    declarations = valid_fork_fallback_declarations()
    apply_mutation = _FORK_FALLBACK_MUTATIONS.get(mutation)
    if apply_mutation is not None:
        apply_mutation(declarations, key)
    return declarations
