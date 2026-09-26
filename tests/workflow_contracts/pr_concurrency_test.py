"""Contracts cancelling superseded pull-request runs.

Every push to a pull request starts a fresh run of each gate, and the run
already in flight is answering a question about a commit nobody will merge.
Left alone it holds a paid runner until it finishes. A concurrency group keyed
on the pull request makes the newer run cancel the older one.

Cancellation stays conditioned on the event. A literal ``cancel-in-progress:
true`` would also cancel a push to ``main``, a schedule and a dispatch, none of
which has a successor repeating its work, so the exact expression is asserted.
The group keeps two pushes to one pull request together and every other run
apart; when there is no pull request its fallback is ``github.run_id``, so no
trunk push or dispatch replaces another that is still pending (estate rule
"PR-lane concurrency fallback"). Rather than search the group's text, the
contract renders it for a set of run contexts and compares the results.

Only ``pull_request`` is in scope. The ``pull_request_target`` workflow merges
Dependabot pull requests, and cancelling a merge mid-flight is a hazard with
no minutes to win. ``ci-windows.yml`` is called from ``ci.yml`` and runs
inside the caller's group.

Workflows are read at test setup through ``workflow_loading``, which refuses a
repeated mapping key: PyYAML would otherwise keep the last of two
``concurrency:`` blocks and say nothing.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import pytest
from ci_coverage_wiring_invariants import declares_trigger
from pr_concurrency_groups import (
    FIRST_PUSH,
    MUST_PART,
    MUST_SHARE,
    fallback_problems,
    render_group,
    shared_groups,
)
from workflow_loading import REPO_ROOT, WorkflowReadError, all_workflow_documents

type Workflows = dict[str, dict[str, object]]

#: The exact ``cancel-in-progress`` expression every pull-request workflow
#: carries. A literal ``true`` is a YAML boolean and never equals it.
CANCEL_EXPRESSION: typ.Final = "${{ github.event_name == 'pull_request' }}"

#: The trigger that puts a workflow in scope; see the module docstring.
PULL_REQUEST: typ.Final = "pull_request"

#: Workflows known to start on ``pull_request``. Discovery is dynamic, so a
#: new workflow is covered the day it lands, but a discovery that silently
#: empties would pass every contract here; this is the floor it must reach.
KNOWN_PULL_REQUEST_WORKFLOWS: typ.Final = frozenset({
    "ci.yml",
    "netsukefile-test.yml",
    "release-dry-run.yml",
})

#: Two workflow names that must never share a group on one pull request.
SYNTHETIC_WORKFLOW_NAMES: typ.Final = ("CI", "Release Dry Run")


def _concurrency(document: dict[str, object]) -> dict[object, object]:
    """Return a workflow's top-level concurrency mapping, or an empty one.

    The shorthand string form cannot carry ``cancel-in-progress`` at all, so
    it reads as empty here, as does a workflow declaring no concurrency.

    Returns
    -------
    dict[object, object]
        The ``concurrency:`` mapping, or an empty mapping.
    """
    declared = document.get("concurrency")
    return declared if isinstance(declared, dict) else {}


def _group(document: dict[str, object]) -> str:
    """Return a workflow's concurrency group as written, or an empty string."""
    return str(_concurrency(document).get("group", ""))


def _workflow_name(file_name: str, document: dict[str, object]) -> str:
    """Return the name Actions gives a workflow: its ``name:``, else its path."""
    declared = document.get("name")
    return declared if isinstance(declared, str) else f".github/workflows/{file_name}"


@pytest.fixture
def workflows() -> Workflows:
    """Read every workflow at setup, failing with the reason if one cannot be.

    Returns
    -------
    Workflows
        File name to parsed document.
    """
    try:
        documents = all_workflow_documents(REPO_ROOT / ".github" / "workflows")
    except WorkflowReadError as error:
        pytest.fail(f"cannot read the workflows: {error}")
    return documents


@pytest.fixture
def pr_workflows(workflows: Workflows) -> Workflows:
    """Return the workflows a pull request starts, keyed by file name.

    Returns
    -------
    Workflows
        The workflows declaring ``pull_request`` in any ``on:`` form.
    """
    return {
        name: document
        for name, document in workflows.items()
        if declares_trigger(document, PULL_REQUEST)
    }


def test_discovery_still_finds_the_known_pull_request_workflows(
    pr_workflows: Workflows,
) -> None:
    """Discovery reaches its floor, so the contracts below are not vacuous."""
    missing = sorted(KNOWN_PULL_REQUEST_WORKFLOWS - set(pr_workflows))
    assert not missing, (
        f"these workflows start on pull_request but discovery missed them: "
        f"{missing}; every contract below would pass without them"
    )


def test_every_pull_request_workflow_declares_a_group(
    pr_workflows: Workflows,
) -> None:
    """Without a group, a superseded run holds its runner until it finishes."""
    missing = sorted(name for name, doc in pr_workflows.items() if not _group(doc))
    assert not missing, f"these pull-request workflows declare no group: {missing}"


def test_two_pushes_to_one_pull_request_share_a_group(
    pr_workflows: Workflows,
) -> None:
    """The newer push lands in its predecessor's group, so it can cancel it.

    A group built from the run identifier alone, the SHA, or the run
    identifier ahead of the pull-request number renders differently for each
    push and cancels nothing.
    """
    split = {
        name: [render_group(_group(doc), run) for pair in MUST_SHARE for run in pair]
        for name, doc in pr_workflows.items()
        if any(
            render_group(_group(doc), first).casefold()
            != render_group(_group(doc), second).casefold()
            for first, second in MUST_SHARE
        )
    }
    assert not split, f"these groups split one pull request's pushes: {split}"


def test_no_other_two_runs_share_a_group(pr_workflows: Workflows) -> None:
    """No run cancels another pull request's, and none replaces a pending one.

    The runs are a pull request, a fork's pull request from a branch of the
    same name, two pushes to ``main``, two dispatches of another branch and
    two scheduled runs. A ``github.head_ref`` group collides on the forks; a
    ``github.ref`` fallback collides on the trunk pushes.
    """
    colliding = {
        name: shared
        for name, doc in pr_workflows.items()
        if (
            shared := shared_groups({
                str(index): render_group(_group(doc), run)
                for index, run in enumerate(MUST_PART)
            })
        )
    }
    assert not colliding, f"these groups join runs that must stay apart: {colliding}"


def test_the_run_identifier_is_only_the_fallback(pr_workflows: Workflows) -> None:
    """``github.run_id`` appears once, behind the pull-request number."""
    problems = {
        name: found
        for name, doc in pr_workflows.items()
        if (found := fallback_problems(_group(doc)))
    }
    assert not problems, f"these groups break the fallback rule: {problems}"


def test_no_two_workflows_share_a_group_for_one_pull_request(
    pr_workflows: Workflows,
) -> None:
    """Two workflows on one pull request never cancel each other.

    Each workflow is rendered under the name Actions gives it, and the groups
    are compared casefolded because GitHub treats group names that way.
    """
    rendered = {
        name: render_group(
            _group(doc), {**FIRST_PUSH, "github.workflow": _workflow_name(name, doc)}
        )
        for name, doc in pr_workflows.items()
    }
    shared = shared_groups(rendered)
    assert not shared, f"these workflows share a group for one pull request: {shared}"


def test_each_group_is_keyed_on_the_workflow(pr_workflows: Workflows) -> None:
    """A group renders differently under two workflow names.

    The cross-workflow test above passes today only because the three
    workflows happen to have distinct names. This renders each group under two
    synthetic names, so a group without ``github.workflow`` fails even while
    it is the only one of its shape.
    """
    unkeyed = {
        name: _group(doc)
        for name, doc in pr_workflows.items()
        if shared_groups({
            workflow: render_group(
                _group(doc), {**FIRST_PUSH, "github.workflow": workflow}
            )
            for workflow in SYNTHETIC_WORKFLOW_NAMES
        })
    }
    assert not unkeyed, (
        f"these groups ignore the workflow name, so two pull-request workflows "
        f"would cancel each other: {unkeyed}"
    )


def test_cancellation_is_conditioned_on_the_event(pr_workflows: Workflows) -> None:
    """Cancellation applies to pull requests only.

    A literal ``true`` reads as stricter and is a regression: it would cancel
    a push to ``main``, a schedule or a dispatch, none of which has a
    successor that repeats its work.
    """
    wrong = {
        name: declared
        for name, doc in pr_workflows.items()
        if (declared := _concurrency(doc).get("cancel-in-progress"))
        != CANCEL_EXPRESSION
    }
    assert not wrong, (
        f"these workflows must set cancel-in-progress to {CANCEL_EXPRESSION!r}: {wrong}"
    )


def test_no_other_workflow_cancels_a_run(workflows: Workflows) -> None:
    """A workflow no pull request starts never cancels a run in progress.

    Publishers, releases and scheduled work have no successor repeating their
    work: cancelling a coverage upload or a release half-way loses it.
    """
    cancelling = {
        name: declared
        for name, doc in workflows.items()
        if not declares_trigger(doc, PULL_REQUEST)
        and (declared := _concurrency(doc).get("cancel-in-progress", False))
        is not False
    }
    assert not cancelling, (
        f"these workflows run on no pull request but may cancel a run: {cancelling}"
    )


#: The release lane's concurrency, exactly. One group per tag ref keeps two
#: releases of the same tag from running at once; ``False`` queues the newer
#: behind the one in progress instead of abandoning a half-published release.
RELEASE_CONCURRENCY: typ.Final = {
    "group": "release-${{ github.ref }}",
    "cancel-in-progress": False,
}


def test_the_release_lane_queues_rather_than_cancels(workflows: Workflows) -> None:
    """``release.yml`` serializes per ref and never cancels a running release.

    The clause above treats a missing ``cancel-in-progress`` as not
    cancelling, so it passes with the release block deleted, and then two
    pushes of one tag would publish concurrently. This pins the block itself.
    """
    declared = _concurrency(workflows.get("release.yml", {}))
    assert declared == RELEASE_CONCURRENCY, (
        f"release.yml must declare concurrency {RELEASE_CONCURRENCY!r}, "
        f"found {declared!r}"
    )
