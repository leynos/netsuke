"""Hold the trunk lane's upload step to the inputs and gate it is configured with.

`coverage-main.yml` is the only lane that hands a coverage report to CodeScene.
Its sibling module, ``codescene_upload_contract_test``, holds the lane's
*structure* — which steps exist, where they sit, and that the action is called
once at an immutable pin. What is left is the upload's configuration: the two
names its steps must agree on, the inputs it must not carry, the credential it
is handed, and the gate it is submitted under.

The split is the same question asked of the same lane from two sides, and it is
also what keeps both modules within the size the linters allow. The predicates
under test live in ``codescene_upload_invariants.py`` and in
``codescene_credential_invariants.py``, which owns the rules about the secret
the lane is handed, and the shared parsing helpers in ``workflow_loading.py``.
Following the sibling suites, the detectors are also driven against synthetic
workflow text: a detector that stopped matching would otherwise let the
repository assertion pass by finding nothing to object to.

Run via ``make test-workflow-contracts``.
"""

import pytest
from ci_coverage_wiring_invariants import (
    GENERATE_COVERAGE_ACTION,
    UPLOAD_COVERAGE_ACTION,
    UPLOAD_GUARD_CONJUNCTS,
)
from codescene_credential_invariants import (
    CREDENTIAL_ENVIRONMENT_KEY,
    CREDENTIAL_INPUT,
)
from codescene_upload_invariants import (
    CHECKSUM_INPUTS,
    CODESCENE_UPLOAD_STEP,
    COVERAGE_FORMAT_INPUT,
    COVERAGE_STEP,
    OUTPUT_PATH_INPUT,
    PUBLICATION_OPT_OUT_INPUT,
    UPLOAD_PATH_INPUT,
    upload_contract_offenders,
)
from codescene_upload_lane_data import clean_steps, inputs_of, step_of


@pytest.mark.parametrize(
    ("step_name", "input_name", "replacement"),
    [
        # The write and the read are two independent inputs for one file.
        (COVERAGE_STEP, OUTPUT_PATH_INPUT, "coverage.xml"),
        (CODESCENE_UPLOAD_STEP, UPLOAD_PATH_INPUT, "coverage.xml"),
        # The upload infers nothing from the extension, so a format the
        # generator no longer writes would be parsed as the wrong shape.
        (COVERAGE_STEP, COVERAGE_FORMAT_INPUT, "cobertura"),
        (CODESCENE_UPLOAD_STEP, COVERAGE_FORMAT_INPUT, "cobertura"),
    ],
)
def test_the_detectors_report_a_path_or_format_disagreement(
    step_name: str, input_name: str, replacement: str
) -> None:
    """Fail a lane whose two steps disagree about what was produced."""
    steps = clean_steps()
    inputs_of(step_of(steps, step_name))[input_name] = replacement
    assert upload_contract_offenders(steps), (
        f"{step_name!r} passing {input_name}={replacement!r} must be reported: "
        f"the report is written and read through different inputs"
    )


@pytest.mark.parametrize("checksum_input", CHECKSUM_INPUTS)
def test_the_detectors_report_a_reintroduced_checksum_input(
    checksum_input: str,
) -> None:
    """Fail the lane if any checksum input returns under any spelling.

    This repository declares no repository variables, so a checksum bound to
    one resolves to the empty string and verifies nothing. The pinned action
    does not merely ignore a checksum it cannot match: carrying either input
    fails at the action's own validation step, so the input has to stay gone
    here rather than merely be spelled anew.
    """
    steps = clean_steps()
    inputs_of(step_of(steps, CODESCENE_UPLOAD_STEP))[checksum_input] = "abc123"
    assert upload_contract_offenders(steps), (
        f"{checksum_input!r} must be reported on the upload step"
    )


def test_the_detectors_report_an_upload_that_cannot_be_gated_on() -> None:
    """Fail an upload that is ungated, or gated on something else.

    A fork, or any repository that has not set the secret, runs this lane with
    the token absent. The gate the lane uses for that is the availability the
    check step publishes, so a gate naming anything else — the retired
    environment variable, an inverted comparison, a quoted clause — lets the
    upload run with an empty credential and fail the trunk run over a secret
    that was never required.
    """

    def mutated(field: str, replacement: object) -> list[dict[str, object]]:
        """Return the clean lane with one credential field varied."""
        steps = clean_steps()
        upload = step_of(steps, CODESCENE_UPLOAD_STEP)
        match field:
            case "if":
                upload["if"] = replacement
            case "env":
                upload["env"] = replacement
            case _:
                inputs_of(upload)[CREDENTIAL_INPUT] = replacement
        return steps

    cases: list[tuple[str, object]] = [
        # A literal token is a credential this repository does not own, and one
        # no gate can have compared against: the gate reads a boolean.
        ("token", "cs-secret-in-the-clear"),
        # A gate naming something else is not a gate on the availability.
        ("if", "github.event_name == 'push'"),
        # A gate with no condition is not a gate.
        ("if", ""),
        # A name that merely contains the check's output is a different value,
        # and an unset one: the comparison is then false against true, so the
        # lane would read as gated while submitting nothing.
        ("if", "${{ steps.codescene_token.outputs.availableable == 'true' }}"),
        # The comparison the check step publishes inverted, as a gate. It opens
        # on precisely the run the gate exists to skip: the one where the
        # optional secret is absent and the availability is false.
        (
            "if",
            (
                "steps.codescene_token.outputs.available == 'false'"
                " && github.ref == 'refs/heads/main'"
            ),
        ),
        # The same inversion through truthiness. The false string is truthy, so
        # `!` of the output opens whenever the availability is set at all,
        # which is the mirror image of what the gate has to mean.
        ("if", "!steps.codescene_token.outputs.available"),
        # The availability as a bare condition: no comparison at all. It
        # happens to behave as a gate through truthiness coercion of the
        # string, but it states an intent the contract cannot read, and the
        # lane's own spelling is the explicit comparison.
        ("if", "steps.codescene_token.outputs.available"),
        # The ref clause alone. The upload then runs on any dispatch from any
        # branch, which is the fault the ref clause was added beside.
        ("if", "github.ref == 'refs/heads/main'"),
        # The retired gate: the credential read from the environment the
        # composite action made dangerous. It is the spelling this lane used
        # before the availability step existed, and it accepts a step whose
        # nested action would be handed whatever the runner exported.
        ("if", f"${{{{ env.{CREDENTIAL_ENVIRONMENT_KEY} != '' }}}}"),
        # The retired gate's polarity inverted, which is the form a check
        # reading only for the credential's name accepts.
        ("if", f"env.{CREDENTIAL_ENVIRONMENT_KEY} == ''"),
        # The same inversion through the other addressing syntax. GitHub's
        # contexts reference gives an expression two ways to reach a value, so
        # a gate rejected in the dotted spelling has to be rejected in the
        # index one too.
        ("if", f"env['{CREDENTIAL_ENVIRONMENT_KEY}'] == ''"),
        # The same, with the spaces an index may carry inside its brackets.
        ("if", f"env[ '{CREDENTIAL_ENVIRONMENT_KEY}' ] == ''"),
        # A credential that is only spelled inside a string literal. The
        # comparison is a non-empty literal against the empty string, so it is
        # always true and gates nothing.
        ("if", f"${{{{ 'env.{CREDENTIAL_ENVIRONMENT_KEY}' != '' }}}}"),
        # A credential handed to the action from anywhere but the secret store.
        # The names match, so a check that reads only the name accepts this;
        # the value does not, and the action would be handed whatever that
        # namespace holds rather than the secret the gate opened on.
        ("token", f"${{{{ github.{CREDENTIAL_ENVIRONMENT_KEY} }}}}"),
        # The same containment trap on the value handed to the action.
        ("token", f"${{{{ env.NOT_{CREDENTIAL_ENVIRONMENT_KEY} }}}}"),
        # A `secrets.` value that reads the credential off the step's own
        # environment. It contains `secrets.` while naming no secret: the name
        # comes from `env`, and the upload would be handed whatever the runner
        # left there rather than the repository's token.
        ("env", f"${{{{ env.secrets.{CREDENTIAL_ENVIRONMENT_KEY} }}}}"),
    ]
    for field, replacement in cases:
        assert upload_contract_offenders(mutated(field, replacement)), (
            f"{field}={replacement!r} must be reported on the upload step"
        )


def test_the_detectors_accept_the_guard_however_its_clauses_are_written() -> None:
    """Accept a real gate however it spells the two clauses it conjoins.

    The contract is about the conjunction the step is gated on, not about the
    text it is written in. Its rule is ``is_trunk_only_upload``'s, which splits
    on ``&&`` outside string literals and parentheses and compares each clause
    whole, so the two may be swapped, respaced, or added to. A check reading
    the guard as text would refuse a reordered gate and accept a quoted one,
    which is the failure mode that gets a contract deleted rather than fixed.
    """
    availability, trunk = sorted(UPLOAD_GUARD_CONJUNCTS)
    for condition in [
        f"{availability} && {trunk}",
        f"{trunk} && {availability}",
        f"  {availability}   &&\n  {trunk} ",
        f"{availability} && {trunk} && github.repository == 'leynos/netsuke'",
    ]:
        steps = clean_steps()
        step_of(steps, CODESCENE_UPLOAD_STEP)["if"] = condition
        assert not upload_contract_offenders(steps), (
            f"a gate written {condition!r} conjoins both clauses and must be accepted"
        )


def test_the_clean_lane_declares_no_environment_on_the_upload() -> None:
    """Assert the upload holds no environment at all.

    The contract refuses an environment that names the credential, and this
    repository's lane declares none — a composite action's nested steps inherit
    whatever the calling step exports, so there is nothing to put there. The
    absence is asserted directly because a removal of something absent would
    change nothing, and the model removes only what the fixture carries.
    """
    upload = step_of(clean_steps(), CODESCENE_UPLOAD_STEP)
    assert "env" not in upload, (
        f"the clean lane must declare no environment on {CODESCENE_UPLOAD_STEP!r}; "
        f"got {upload.get('env')!r}"
    )


@pytest.mark.parametrize("step_name", [COVERAGE_STEP, CODESCENE_UPLOAD_STEP])
def test_the_detectors_report_a_lane_that_suppresses_its_own_upload(
    step_name: str,
) -> None:
    """Fail a lane that sets the publication opt-out on either step.

    The opt-out belongs to the pull-request lane, which uses it to keep the
    report local. Set here it would suppress the archive that survives a
    failed run, or — on the upload step, which has no such input — state an
    intent the action does not implement.
    """
    steps = clean_steps()
    inputs_of(step_of(steps, step_name))[PUBLICATION_OPT_OUT_INPUT] = "false"
    assert upload_contract_offenders(steps), (
        f"{PUBLICATION_OPT_OUT_INPUT} on {step_name!r} must be reported"
    )


def test_the_detectors_report_a_variable_the_repository_does_not_declare() -> None:
    """Fail the lane for a ``vars.`` reference to anything undefined.

    This is the narrower form of the defect that motivated the contract: the
    checksum input's value came from a repository variable this repository has
    never declared, so it resolved to the empty string while reading as though
    it verified the installer.
    """
    steps = clean_steps()
    step_of(steps, CODESCENE_UPLOAD_STEP)["if"] = (
        "${{ vars.CODESCENE_CLI_SHA256 != '' }}"
    )
    assert upload_contract_offenders(steps), (
        "a gate on an undeclared repository variable must be reported; it "
        "would never open"
    )


@pytest.mark.parametrize("step_name", [COVERAGE_STEP, CODESCENE_UPLOAD_STEP])
@pytest.mark.parametrize(
    ("pin", "fault"),
    [
        ("v1.2.3", "a tag"),
        ("main", "a branch"),
        ("a576501", "an abbreviated SHA"),
        ("A5765019912A8AB6882B12DB049C7CDE635F3A85", "an uppercase SHA"),
        ("", "no pin at all"),
    ],
)
def test_the_detectors_report_an_unpinned_action_reference(
    step_name: str, pin: str, fault: str
) -> None:
    """Fail a reference that names the action at anything but a full commit SHA.

    A tag or branch resolves to whatever its owner publishes next, so a
    reviewed pin can be replaced without a commit here. The revision itself is
    not asserted — that belongs to the dependency updater — so each case
    varies only the pin's shape.
    """
    steps = clean_steps()
    action = (
        GENERATE_COVERAGE_ACTION
        if step_name == COVERAGE_STEP
        else UPLOAD_COVERAGE_ACTION
    )
    step_of(steps, step_name)["uses"] = f"{action}@{pin}" if pin else action
    assert upload_contract_offenders(steps), (
        f"{step_name!r} pinned to {fault} must be reported"
    )
