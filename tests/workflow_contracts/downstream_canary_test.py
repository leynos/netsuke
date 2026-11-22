"""Hold the shape of the shared downstream-canary composite action.

The release workflow runs every downstream migration canary through
``.github/actions/downstream-canary``. These checks keep that bootstrap honest:
it checks out the pinned downstream revision, builds the exact candidate, runs
the phases in order under the platform's shell, always records provenance, and
pins every external action to a full commit SHA. They assert the pin's shape,
never its value, so a Dependabot bump does not fail them.

Run via ``make test-workflow-contracts``.
"""

import re
import typing as typ

import pytest
from action_references import require_external_action_sha
from workflow_loading import (
    REPO_ROOT,
    read_workflow_document,
    require_list,
    require_mapping,
    unique_step_index,
)

if typ.TYPE_CHECKING:  # pragma: no cover - imported for annotations only
    import pathlib as pl

ACTION_DIRECTORY: pl.Path = REPO_ROOT / ".github" / "actions" / "downstream-canary"

#: Each external action the canary uses, keyed by the step that uses it.
EXTERNAL_ACTIONS = {
    "Check out the pinned downstream revision": "actions/checkout",
    "Install Ninja": "seanmiddleditch/gha-setup-ninja",
    "Install uv and the runner's Python": "astral-sh/setup-uv",
    "Install cargo-nextest": "taiki-e/install-action",
    "Install Whitaker": "leynos/shared-actions/.github/actions/install-whitaker",
    "Upload downstream canary provenance": "actions/upload-artifact",
}

#: The phases in the order they must run.
PHASE_ORDER = (
    "Check out the pinned downstream revision",
    "Build the Netsuke candidate",
    "Generate the downstream Ninja manifest (Bash)",
    "Generate the downstream Ninja manifest (PowerShell)",
    "Assert the Ninja manifest exists",
    "Run the downstream targets (Bash)",
    "Run the downstream targets (PowerShell)",
    "Record downstream canary provenance",
    "Upload downstream canary provenance",
)

#: The downstream checkout: the matrix's repository at its pinned commit, beside
#: rather than inside Netsuke's checkout, with no credential persisted.
PINNED_CHECKOUT = {
    "repository": "${{ inputs.repository }}",
    "ref": "${{ inputs.ref }}",
    "path": "downstream",
    "persist-credentials": False,
}

#: The closed tool vocabulary the ``tools`` input documents.
TOOLS = ("nextest", "whitaker", "markdownlint", "libpq", "libsqlite3")


def canary_action() -> dict[str, object]:
    """Load the composite action as a mapping.

    Returns
    -------
    dict[str, object]
        The parsed ``action.yml``.
    """
    return require_mapping(
        read_workflow_document(ACTION_DIRECTORY / "action.yml"), "downstream canary"
    )


def canary_steps() -> list[dict[str, object]]:
    """Return the composite action's steps.

    Returns
    -------
    list[dict[str, object]]
        The steps in declaration order.
    """
    runs = require_mapping(canary_action().get("runs"), "runs")
    return [
        require_mapping(step, "step")
        for step in require_list(runs.get("steps"), "runs.steps")
    ]


def step(name: str) -> dict[str, object]:
    """Return the uniquely named step.

    Returns
    -------
    dict[str, object]
        The step mapping.
    """
    steps = canary_steps()
    return steps[unique_step_index(steps, name)]


@pytest.mark.parametrize(("name", "action"), EXTERNAL_ACTIONS.items())
def test_external_actions_are_pinned_to_full_commits(name: str, action: str) -> None:
    """Every external action is named exactly and pinned to a full SHA.

    Scenario: the canary runs third-party code inside the release pipeline.
    Invariant: a tag or branch could move under the release, so each
    reference must be a 40-character commit; the value itself is not asserted.
    """
    require_external_action_sha(step(name).get("uses"), action, name)


def test_no_step_uses_an_unlisted_external_action() -> None:
    """A new external action must be added to the pinned inventory above.

    Scenario: someone adds an action to the bootstrap. Invariant: every
    ``uses`` belongs to a step this suite checks for a full-SHA pin.
    """
    unlisted = [
        entry.get("name")
        for entry in canary_steps()
        if "uses" in entry and entry.get("name") not in EXTERNAL_ACTIONS
    ]
    assert not unlisted, f"external actions without a pin contract: {unlisted!r}"


def test_downstream_checkout_is_pinned_and_credential_free() -> None:
    """The downstream repository is checked out at the pin, with no token kept.

    Scenario: the one cross-repository checkout in this repository.
    Invariant: it reads the matrix's repository and full commit into
    ``downstream/``, and persists no credential into that untrusted tree.
    """
    inputs = require_mapping(
        step("Check out the pinned downstream revision").get("with"), "with"
    )
    assert inputs == PINNED_CHECKOUT, (
        f"the downstream checkout must be pinned and credential-free: {inputs!r}"
    )


def test_candidate_is_built_from_the_resolved_commit_and_version() -> None:
    """The candidate installer receives exactly the resolved identity.

    Scenario: the canary must exercise the release candidate, not a nearby
    build. Invariant: the installer is driven by the commit and version
    inputs, and refuses a binary reporting any other version.
    """
    build = step("Build the Netsuke candidate")
    assert require_mapping(build.get("env"), "env") == {
        "NETSUKE_CANDIDATE_REVISION": "${{ inputs.netsuke-commit }}",
        "NETSUKE_CANDIDATE_VERSION": "${{ inputs.netsuke-version }}",
    }, "the installer must build the resolved commit and version"
    assert build.get("id") == "netsuke", "later phases read steps.netsuke outputs"


def test_phases_run_in_order() -> None:
    """Checkout, build, generate, assert, run, record, upload: in that order.

    Scenario: the phases depend on one another's files. Invariant: each named
    phase exists exactly once and follows the one before it.
    """
    steps = canary_steps()
    indices = [unique_step_index(steps, name) for name in PHASE_ORDER]
    assert indices == sorted(indices), f"phases out of order: {indices!r}"


@pytest.mark.parametrize(
    ("variant", "shell", "platform"),
    [("(Bash)", "bash", "linux"), ("(PowerShell)", "pwsh", "windows")],
)
def test_each_platform_drives_the_runner_from_its_own_shell(
    variant: str, shell: str, platform: str
) -> None:
    """Linux runs the Bash variants and Windows the PowerShell ones.

    Scenario: the OrthoConfig Windows leg. Invariant: every shell-specific
    phase is gated on its platform, so exactly one variant runs per job.
    """
    variants = [
        entry for entry in canary_steps() if str(entry.get("name")).endswith(variant)
    ]
    assert len(variants) == 2, f"expected generate and run {variant} steps"
    for entry in variants:
        assert (entry.get("shell"), entry.get("if")) == (
            shell,
            f"inputs.platform == '{platform}'",
        ), f"{entry.get('name')} must run under {shell} on {platform}"


def test_provenance_is_always_recorded_and_uploaded_only_on_request() -> None:
    """A failed canary still reports; dry runs keep the record in the summary.

    Scenario: a canary fails during setup. Invariant: the record step runs
    regardless, and the artefact upload needs the caller's explicit opt-in.
    """
    assert step("Record downstream canary provenance").get("if") == "always()", (
        "provenance must be recorded even when an earlier phase failed"
    )
    assert step("Upload downstream canary provenance").get("if") == (
        "always() && inputs.upload-provenance == 'true'"
    ), "the provenance upload must be opt-in and survive a failed phase"


def test_extra_environment_never_reaches_the_record() -> None:
    """Service credentials are passed to the tools but never recorded.

    Scenario: MXD's PostgreSQL lane passes a connection string. Invariant: the
    record step neither receives the ``environment`` input nor passes it on.
    """
    record = step("Record downstream canary provenance")
    assert "inputs.environment" not in str(record.get("env")), (
        "the record step must not receive the extra environment"
    )
    assert "--environment" not in str(record.get("run")), (
        "the record step must not pass the extra environment to the runner"
    )


def test_every_referenced_script_exists() -> None:
    """Each script a step runs through ``GITHUB_ACTION_PATH`` is present.

    Scenario: a script is renamed or moved. Invariant: the action's relative
    references resolve to files in this repository.
    """
    references = {
        match
        for entry in canary_steps()
        for match in re.findall(
            r"GITHUB_ACTION_PATH/([^\"\s]+)", str(entry.get("run", ""))
        )
    }
    assert references, "the action should reference its scripts by action path"
    missing = [
        path for path in references if not (ACTION_DIRECTORY / path).resolve().is_file()
    ]
    assert not missing, f"missing scripts: {missing!r}"


def test_tool_installs_match_the_documented_vocabulary() -> None:
    """Every documented tool has one install step, and nothing else does.

    Scenario: a canary names a tool. Invariant: the closed vocabulary in the
    ``tools`` description is exactly the set of tokens the steps test for.
    """
    conditions = " ".join(str(entry.get("if", "")) for entry in canary_steps())
    installed = set(re.findall(r"' (\w+) '\)", conditions))
    description = str(
        require_mapping(
            require_mapping(canary_action().get("inputs"), "inputs").get("tools"),
            "tools",
        ).get("description")
    )
    documented = {tool for tool in TOOLS if f"`{tool}`" in description}
    assert installed == documented == set(TOOLS), (
        f"installed {installed!r}, documented {documented!r}, expected {TOOLS!r}"
    )
