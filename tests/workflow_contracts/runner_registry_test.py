"""Hold the self-hosted runner registry to the labels the workflows select.

actionlint rejects a `runs-on` label it does not know, so every self-hosted
label a workflow can select must be registered in `.github/actionlint.yaml`,
and nothing else. The comparison runs in both directions: an unregistered
label would queue forever, and a registration left behind hides a lane that
has already been retired.

The labels are derived from the workflows rather than read from the reviewed
constant, so the two readings are compared. A runner may be named directly,
chosen from a job's own matrix, or passed to a local reusable workflow, and
each of those paths has to reach the inventory for the comparison to mean
anything; a path that silently contributes nothing leaves the registry green
over a runner nobody registered.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import pytest
import yaml
from fork_fallback import read_placement
from runner_matrix import caller_runner_labels, selected_labels
from runner_placement_invariants import (
    GITHUB_HOSTED_LABELS,
    UBICLOUD_DEFAULT_LABEL,
    UBICLOUD_LABELS,
)
from workflow_loading import (
    REPO_ROOT,
    load_workflow,
    require_list,
    require_mapping,
)

WORKFLOW_DIR = REPO_ROOT / ".github" / "workflows"

if typ.TYPE_CHECKING:  # pragma: no cover - typing only
    import collections.abc as cabc


def is_self_hosted_label(label: str) -> bool:
    """Return whether a label names a runner GitHub does not host.

    By name, not by prefix: a prefix test absorbs any new label that looks
    hosted, so a lane moved onto an unknown image would drop out of "in use".
    A matrix selector is resolved against its job's matrix before this
    question; one that could not be resolved arrives here as itself and so
    counts, because an unread label must fail the comparison rather than
    excuse the runner it hides.

    Returns
    -------
    bool
        True when the label names a runner GitHub does not host.
    """
    return label not in GITHUB_HOSTED_LABELS


def _selected_labels(declaration: object, job: dict[str, object]) -> list[str]:
    """Return the labels one `runs-on` value can select.

    Returns
    -------
    list[str]
        Every label the declaration can select, empty when it selects none.
    """
    # Both arms of a conditional count: a job that falls back for forks may
    # run on either, and reading only the declaration would take the whole
    # expression for one unrecognized label.
    placement = read_placement(declaration)
    if placement is not None:
        return [placement.fork, placement.owned]
    match declaration:
        case str() as label:
            return selected_labels(label, job)
        case list() as entries:
            return [str(entry) for entry in entries]
        case _:
            return []


def self_hosted_labels_in_use(jobs: cabc.Iterable[dict[str, object]]) -> set[str]:
    """Return every label the given jobs select that GitHub does not host.

    The jobs are a parameter rather than read inside the function so the two
    properties the registry comparison rests on can be tested against
    synthetic workflows: a matrix-selected label reaches this inventory, and a
    selector no matrix resolves stays visible in it. Both are invisible to a
    test of `selected_labels` alone, because either failure removes the label
    from the inventory rather than changing what that resolver returns.

    Returns
    -------
    set[str]
        The labels the jobs select that GitHub does not host.
    """
    # A matrix selector resolves against the job's own matrix here, so a label
    # chosen that way reaches this inventory exactly as a bare one does. A
    # caller job that passes its runner to a local reusable workflow is read
    # from that input, since its own `runs-on` is absent.
    found: set[str] = set()
    for job in jobs:
        found.update(_selected_labels(job.get("runs-on"), job))
        found.update(caller_runner_labels(job))
    return {label for label in found if is_self_hosted_label(label)}


def _all_jobs() -> list[dict[str, object]]:
    """Return every job declared by every workflow."""
    found: list[dict[str, object]] = []
    for path in sorted([*WORKFLOW_DIR.glob("*.yml"), *WORKFLOW_DIR.glob("*.yaml")]):
        jobs = load_workflow(path).get("jobs")
        if isinstance(jobs, dict):
            found.extend(job for job in jobs.values() if isinstance(job, dict))
    return found


def _self_hosted_labels_in_use() -> set[str]:
    """Return every unhosted label this repository's workflows select.

    Returns
    -------
    set[str]
        The labels the workflows select that GitHub does not host.
    """
    return self_hosted_labels_in_use(_all_jobs())


def test_actionlint_registers_exactly_the_ubicloud_labels_in_use() -> None:
    """Register every self-hosted label in use, and nothing else.

    Equality in both directions. actionlint rejects an unregistered
    self-hosted label, so a typo or an unreviewed shape fails the lint gate
    instead of queueing forever; and a registration left behind after a lane
    moved back to GitHub's pool hides a runner assignment already retired.

    "In use" is derived from the workflows rather than read from the reviewed
    constant, and the two are compared separately, so a lane that quietly
    stops using a shape fails rather than agreeing with a constant nobody
    revisited. The previous form asked only whether each label appeared
    anywhere in the workflow text, which a comment satisfies.
    """
    config = yaml.safe_load(
        (REPO_ROOT / ".github" / "actionlint.yaml").read_text(encoding="utf-8")
    )
    registered = require_mapping(config, "actionlint config")["self-hosted-runner"]
    labels = tuple(
        str(label)
        for label in require_list(
            require_mapping(registered, "self-hosted-runner").get("labels"),
            "self-hosted-runner labels",
        )
    )
    in_use = _self_hosted_labels_in_use()
    assert sorted(labels) == sorted(in_use), (
        f"actionlint must register exactly the self-hosted labels the "
        f"workflows select. It registers {sorted(labels)!r} and they select "
        f"{sorted(in_use)!r}"
    )
    assert sorted(in_use) == sorted(UBICLOUD_LABELS), (
        f"the reviewed label set and the labels in use have diverged: "
        f"{sorted(UBICLOUD_LABELS)!r} against {sorted(in_use)!r}. A shape "
        f"change belongs in both, with the measurement that justifies it"
    )


def test_a_matrix_selected_label_reaches_the_registry_inventory() -> None:
    """A matrix-selected self-hosted runner counts as a bare one does.

    The registry comparison reads this inventory rather than the workflow
    text, so a lane that chooses its runner with `${{ matrix.os }}` drops out
    of the comparison unless its legs are resolved here. A test of the
    resolver alone would keep passing while the inventory lost the label.
    """
    job: dict[str, object] = {
        "runs-on": "${{ matrix.os }}",
        "strategy": {"matrix": {"os": ["ubuntu-latest", UBICLOUD_DEFAULT_LABEL]}},
    }
    labels = self_hosted_labels_in_use([job])
    assert labels == {UBICLOUD_DEFAULT_LABEL}, (
        "the matrix-selected self-hosted label must reach the inventory; "
        f"got {sorted(labels)!r}"
    )


def test_an_unresolved_selector_stays_visible_in_the_registry_inventory() -> None:
    """A selector no matrix resolves must fail the comparison, not vanish.

    A job whose matrix does not declare the axis its `runs-on` reads cannot be
    read at all. Dropping it would let the comparison pass over a runner
    nobody registered, so the selector itself stays in the inventory and the
    set equality finds no registration for it.
    """
    unresolved = "${{ matrix.os }}"
    job: dict[str, object] = {"runs-on": unresolved, "strategy": {"matrix": {}}}
    labels = self_hosted_labels_in_use([job])
    assert labels == {unresolved}, (
        f"an unreadable selector must stay in the inventory, got {sorted(labels)!r}"
    )
    # Set equality is what the registry comparison performs, so an inventory
    # that still holds the selector cannot match the reviewed label set.
    assert labels != set(UBICLOUD_LABELS), (
        "an unreadable selector must leave the registry comparison failing; "
        f"the inventory {sorted(labels)!r} matched {sorted(UBICLOUD_LABELS)!r}"
    )


@pytest.mark.parametrize(
    ("label", "self_hosted"),
    [
        pytest.param("ubicloud-standard-2-ubuntu-2404", True, id="a-label-in-use"),
        pytest.param("ubuntu-latest", False, id="a-named-hosted-label"),
        pytest.param("macos-15-intel", False, id="another-named-one"),
        # The case that separates a named set from a prefix test: a hosted
        # family member this repository does not use, which must be reported
        # rather than silently excused.
        pytest.param("ubuntu-20.04", True, id="a-hosted-family-member-not-named"),
        pytest.param(
            "${{ inputs.runner }}",
            False,
            id="a-caller-supplied-runner-names-no-label-here",
        ),
        # A selector no job could resolve reaches this question as itself, so
        # it counts: otherwise a label nobody registers escapes the comparison.
        pytest.param("${{ matrix.runner }}", True, id="an-unresolved-selector"),
        pytest.param("${{ vars.RUNNER }}", True, id="an-unresolved-expression"),
    ],
)
def test_the_registry_reads_hosted_labels_by_name(
    label: str, *, self_hosted: bool
) -> None:
    """The registry question asks by name, and the two readings differ.

    Over this repository's own workflows a prefix test and the named set agree
    exactly, so the derivation above cannot tell them apart. These cases can.
    """
    assert is_self_hosted_label(label) is self_hosted, (
        f"`{label}` must be classified by name for the registry question"
    )
