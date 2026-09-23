"""Contract tests for the pull-request lane as a closure over workflow calls.

The coverage boundary in ``ci_coverage_wiring_test`` is only as wide as the
set of workflows it reads. These tests hold that set to the closure through
reusable-workflow calls, and hold the two clauses that exist because a
pull-request workflow can reach CodeScene without naming its credential: the
service's host, and ``secrets: inherit`` handed to another repository.

The probe is the one measured on episodic, where a ``workflow_call``-only
workflow, called from a pull-request job with ``secrets: inherit`` and curling
the CodeScene project API, passed every clause of a trigger-enumerated
contract.

Run via ``make test-workflow-contracts``.
"""

import pytest
from ci_coverage_wiring_invariants import (
    CODESCENE_HOST,
    CREDENTIAL_ENVIRONMENT_KEY,
    coverage_surface_offenders,
    pull_request_lane,
)
from workflow_call_closure import (
    UnresolvedWorkflowCallError,
    local_workflow_name,
    reachable_workflows,
)
from workflow_loading import parse_workflow_text

#: A reusable workflow that reaches CodeScene with an inherited credential.
PROBE = f"""
on:
  workflow_call:
jobs:
  probe:
    runs-on: ubuntu-latest
    steps:
      - name: Probe the CodeScene project
        env:
          {CREDENTIAL_ENVIRONMENT_KEY}: ${{{{ secrets.{CREDENTIAL_ENVIRONMENT_KEY} }}}}
        run: curl -fsS https://api.{CODESCENE_HOST}/v2/projects/1 > /dev/null
"""

#: The same probe with the credential taken out, so only the host can find it.
TOKENLESS_PROBE = """
on:
  workflow_call:
jobs:
  probe:
    runs-on: ubuntu-latest
    steps:
      - run: curl -fsS https://API.CodeScene.IO/v2/projects/1 > /dev/null
"""


def _caller(reference: str, secrets: str = "inherit") -> str:
    """Return a pull-request workflow calling ``reference`` with ``secrets``."""
    return f"""
on: pull_request
jobs:
  call:
    uses: {reference}
    secrets: {secrets}
"""


def _documents(**texts: str) -> dict[str, dict[str, object]]:
    """Parse synthetic workflows, keyed by file name with ``_`` for ``.``."""
    documents: dict[str, dict[str, object]] = {}
    for key, text in texts.items():
        parsed = parse_workflow_text(text, key)
        assert isinstance(parsed, dict), f"{key} must parse to a mapping"
        documents[key.replace("_", ".")] = parsed
    return documents


def _lane_offenders(texts: dict[str, str]) -> list[str]:
    """Return every coverage-surface offender across a synthetic lane."""
    documents = _documents(**texts)
    return [
        offender
        for name in sorted(pull_request_lane(documents))
        for offender in coverage_surface_offenders(
            name, documents[name], texts[name.replace(".", "_")]
        )
    ]


@pytest.mark.parametrize(
    "reference",
    [
        "./.github/workflows/probe.yml",
        "$/.github/workflows/probe.yml",
        ".github/workflows/probe.yml",
    ],
)
def test_the_lane_reaches_a_called_workflow(reference: str) -> None:
    """Enumerate a `workflow_call` workflow a pull-request job calls.

    The trigger reading alone reaches the caller only. Each spelling is proved
    on its own, so a reader that knew one of them fails the other case.
    """
    documents = _documents(ci_yml=_caller(reference), probe_yml=PROBE)
    reached = reachable_workflows(documents, ["ci.yml"])
    assert reached == {"ci.yml", "probe.yml"}, f"{reference} must be followed"
    lane = pull_request_lane(documents)
    assert lane == {"ci.yml", "probe.yml"}, f"the lane must hold the callee: {lane}"


def test_the_probe_fails_the_boundary_through_the_closure() -> None:
    """Report the episodic probe by credential and by host once it is reached."""
    offenders = _lane_offenders({
        "ci_yml": _caller("./.github/workflows/probe.yml"),
        "probe_yml": PROBE,
    })
    assert any(
        offender.startswith("probe.yml:") and CREDENTIAL_ENVIRONMENT_KEY in offender
        for offender in offenders
    ), f"the probe's credential must be reported, got {offenders!r}"
    assert any(
        offender.startswith("probe.yml:") and CODESCENE_HOST in offender
        for offender in offenders
    ), f"the probe's host must be reported, got {offenders!r}"


def test_the_host_alone_fails_the_boundary() -> None:
    """Report a curl that names neither the action nor the credential.

    The host is written in mixed case, which a DNS name permits, so a
    case-sensitive match fails this case.
    """
    offenders = _lane_offenders({
        "ci_yml": _caller("./.github/workflows/probe.yml"),
        "probe_yml": TOKENLESS_PROBE,
    })
    assert offenders == [f"probe.yml: raw text contacts {CODESCENE_HOST}"], (
        f"the host must be reported whatever its case, got {offenders!r}"
    )


def test_the_closure_is_transitive() -> None:
    """Follow a call from a called workflow, not only from the entry."""
    middle = _caller("./.github/workflows/probe.yml").replace(
        "on: pull_request", "on: workflow_call"
    )
    documents = _documents(
        ci_yml=_caller("./.github/workflows/middle.yml"),
        middle_yml=middle,
        probe_yml=PROBE,
    )
    lane = pull_request_lane(documents)
    assert lane == {"ci.yml", "middle.yml", "probe.yml"}, (
        f"a callee's own call must be followed, got {sorted(lane)}"
    )


def test_the_closure_stays_narrow() -> None:
    """Leave out what no pull request runs.

    A reusable workflow nothing calls, and a push-only workflow, stay out of
    the lane, so a repository that complies is not failed for holding them.
    """
    documents = _documents(
        ci_yml="on: pull_request\njobs: {}\n",
        probe_yml=PROBE,
        main_yml="on:\n  push:\n    branches: [main]\njobs: {}\n",
    )
    lane = pull_request_lane(documents)
    assert lane == {"ci.yml"}, f"only the pull-request workflow runs, got {lane}"


def test_an_unresolved_local_call_fails_the_reading() -> None:
    """Refuse to vouch for a called workflow the reading never saw."""
    documents = _documents(ci_yml=_caller("./.github/workflows/missing.yml"))
    with pytest.raises(UnresolvedWorkflowCallError, match=r"missing\.yml"):
        pull_request_lane(documents)


@pytest.mark.parametrize(
    ("reference", "expected"),
    [
        ("./.github/workflows/release.yml", "release.yml"),
        (".github/workflows/release.yml", "release.yml"),
        ("$/.github/workflows/release.yml", "release.yml"),
        ("$/.github/workflows/nested/release.yml", None),
        ("./.github/workflows/nested/release.yml", None),
        ("./.github/actions/memory-sampler", None),
        ("leynos/netsuke/.github/workflows/release.yml@main", None),
        ("./.github/workflows/", None),
    ],
)
def test_a_local_call_is_read_by_shape(reference: str, expected: str | None) -> None:
    """Read a call as local exactly when it names a file under the directory."""
    assert local_workflow_name(reference) == expected, (
        f"{reference!r} must read as {expected!r}"
    )


def test_inheriting_into_another_repository_fails_the_boundary() -> None:
    """Refuse `secrets: inherit` on a call whose callee this tree cannot read."""
    offenders = _lane_offenders({
        "ci_yml": _caller("leynos/shared-actions/.github/workflows/x.yml@abc123")
    })
    assert offenders == [
        (
            "ci.yml: job call forwards every secret to another repository's "
            "workflow (inherit)"
        )
    ], f"exactly the cross-repository inherit must be reported, got {offenders!r}"


def test_inheriting_into_a_local_workflow_is_read_through() -> None:
    """Allow `secrets: inherit` locally, where the callee is itself checked.

    `release-dry-run.yml` does exactly this, so refusing it would fail the
    repository for a call whose callee the closure already reads.
    """
    clean = "on:\n  workflow_call:\njobs:\n  noop:\n    runs-on: ubuntu-latest\n"
    offenders = _lane_offenders({
        "ci_yml": _caller("./.github/workflows/clean.yml"),
        "clean_yml": clean,
    })
    assert offenders == [], f"a local inherit is read through, got {offenders!r}"


def test_named_forwarding_into_another_repository_fails_the_boundary() -> None:
    """Report the credential forwarded by name, which needs no inherit."""
    forwarding = _caller(
        "leynos/shared-actions/.github/workflows/x.yml@abc123",
        secrets=(
            f"\n      {CREDENTIAL_ENVIRONMENT_KEY}: "
            f"${{{{ secrets.{CREDENTIAL_ENVIRONMENT_KEY} }}}}"
        ),
    )
    offenders = _lane_offenders({"ci_yml": forwarding})
    assert offenders, "a credential forwarded by name must be reported"
