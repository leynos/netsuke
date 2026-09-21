"""How a bounded edit to a valid lane is drawn, family by family.

``codescene_lane_edit_model`` says what one edit *is* and what the contract must
say about it; ``codescene_lane_mutations`` builds one of each family; this
module says which ones the generator may produce. The split is the dependency
direction — the strategies draw the edits and the edits know nothing of them —
and it is also what keeps each module within the size the linters allow.

Each family draws from a pool wider than the contract's accepted values, because
a mutation is only worth stating a property over if it is a *real* departure
from a valid lane: a rebinding that happened to name the expected value would
be a no-op with a fault claimed for it. The pools are the spellings a workflow
can write rather than arbitrary text, so a contract that recognised the one
spelling the repository happens to use fails here rather than passing.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import itertools
import typing as typ

from ci_coverage_wiring_invariants import COVERAGE_REPORT_PATH
from codescene_credential_invariants import (
    CREDENTIAL_ENVIRONMENT_KEY,
    CREDENTIAL_GATE_NAMESPACE,
    CREDENTIAL_INPUT,
    CREDENTIAL_SOURCE_NAMESPACE,
)
from codescene_lane_edit_model import (
    REQUIRED_FIELDS,
    REQUIRED_INPUTS,
    Mutation,
)
from codescene_lane_mutations import (
    duplicated_step,
    inserted_step,
    misbound_input,
    moved_step,
    rebound_environment,
    removed_field,
    removed_input,
    removed_step,
    reversed_gate,
    smuggled_input,
    swapped_steps,
    ungated_upload,
    weakened_gate,
)
from codescene_report_validation_invariants import REPORT_VALIDATOR_SCRIPT
from codescene_upload_invariants import (
    CHECKSUM_INPUTS,
    CODESCENE_UPLOAD_STEP,
    COVERAGE_FORMAT_INPUT,
    COVERAGE_FORMAT_VALUE,
    COVERAGE_STEP,
    OUTPUT_PATH_INPUT,
    PUBLICATION_OPT_OUT_INPUT,
    REPORT_STEP_NAMES,
    UPLOAD_PATH_INPUT,
)
from hypothesis import strategies as st

#: The expression contexts a generated value may address. A mutation that
#: rebinds a value to a context the contract does not accept is the whole point
#: of the misbinding family, so the pool has to be wider than the accepted one.
CONTEXTS: typ.Final[tuple[str, ...]] = (
    CREDENTIAL_GATE_NAMESPACE,
    CREDENTIAL_SOURCE_NAMESPACE,
    "github",
    "vars",
    "steps",
    "runner",
)


def _other_than(namespace: str) -> st.SearchStrategy[str]:
    """Return a strategy for a context other than ``namespace``."""
    return st.sampled_from(
        tuple(context for context in CONTEXTS if context != namespace)
    )


def _unlike(value: str) -> st.SearchStrategy[str]:
    """Return a strategy for a non-empty string that is never ``value``.

    Built by appending rather than by drawing and discarding, so no generated
    value is ever rejected and the property never depends on how much of the
    space a filter happens to admit.

    Returns
    -------
    st.SearchStrategy
        Strategies for such a string.
    """
    return st.text(
        alphabet="abcdefghijklmnopqrstuvwxyz0123456789-_/.",
        max_size=12,
    ).map(lambda drawn: f"{drawn}~")


def removals() -> st.SearchStrategy[Mutation]:
    """Return a strategy for mutations removing part of a required condition.

    Returns
    -------
    st.SearchStrategy
    """
    steps = st.sampled_from([removed_step(name) for name in REPORT_STEP_NAMES])
    fields = st.sampled_from([
        removed_field(step_name, field)
        for step_name, names in REQUIRED_FIELDS.items()
        for field in names
    ])
    inputs = st.sampled_from([
        removed_input(step_name, input_name)
        for step_name, names in REQUIRED_INPUTS.items()
        for input_name in names
    ])
    return st.one_of(steps, fields, inputs)


def duplications() -> st.SearchStrategy[Mutation]:
    """Return a strategy for mutations declaring a guarded step twice.

    Returns
    -------
    st.SearchStrategy
    """
    return st.sampled_from([duplicated_step(name) for name in REPORT_STEP_NAMES])


def reorderings() -> st.SearchStrategy[Mutation]:
    """Return a strategy for mutations moving a guarded step.

    Returns
    -------
    st.SearchStrategy
    """
    moved = st.tuples(
        st.sampled_from(REPORT_STEP_NAMES),
        st.integers(min_value=0, max_value=len(REPORT_STEP_NAMES) - 1),
    ).map(lambda pair: moved_step(pair[0], pair[1]))
    swapped = st.sampled_from(list(itertools.combinations(REPORT_STEP_NAMES, 2))).map(
        lambda pair: swapped_steps(pair[0], pair[1])
    )
    return st.one_of(moved, swapped)


def misbindings() -> st.SearchStrategy[Mutation]:
    """Return a strategy for mutations binding an input to the wrong value.

    Each arm is a different way for a value to be wrong: a path the generator
    never wrote, a format the upload would parse as another shape, a credential
    read from a context the gate cannot compare, and an input the contract
    requires to stay absent. All four are the *same* family because they are one
    kind of fault — the lane binds something the contract did not ask for — and
    a contract that caught one while missing another would be stated over the
    wiring rather than over the agreement it exists to guarantee.

    Returns
    -------
    st.SearchStrategy
    """
    paths = st.sampled_from([
        (COVERAGE_STEP, OUTPUT_PATH_INPUT),
        (CODESCENE_UPLOAD_STEP, UPLOAD_PATH_INPUT),
    ])
    path = st.tuples(paths, _unlike(COVERAGE_REPORT_PATH)).map(
        lambda pair: misbound_input(pair[0][0], pair[0][1], pair[1])
    )
    format_ = st.tuples(
        st.sampled_from((COVERAGE_STEP, CODESCENE_UPLOAD_STEP)),
        _unlike(COVERAGE_FORMAT_VALUE),
    ).map(lambda pair: misbound_input(pair[0], COVERAGE_FORMAT_INPUT, pair[1]))
    token = st.one_of(
        _other_than(CREDENTIAL_GATE_NAMESPACE).map(
            lambda context: f"${{{{ {context}.{CREDENTIAL_ENVIRONMENT_KEY} }}}}"
        ),
        _unlike(CREDENTIAL_ENVIRONMENT_KEY),
    ).map(lambda value: misbound_input(CODESCENE_UPLOAD_STEP, CREDENTIAL_INPUT, value))
    environment = (
        _other_than(CREDENTIAL_SOURCE_NAMESPACE)
        .map(lambda context: f"${{{{ {context}.{CREDENTIAL_ENVIRONMENT_KEY} }}}}")
        .map(rebound_environment)
    )
    smuggled = st.sampled_from(
        [
            (CODESCENE_UPLOAD_STEP, input_name, "abc123")
            for input_name in CHECKSUM_INPUTS
        ]
        + [
            (COVERAGE_STEP, PUBLICATION_OPT_OUT_INPUT, "false"),
            (CODESCENE_UPLOAD_STEP, PUBLICATION_OPT_OUT_INPUT, "false"),
        ]
    ).map(lambda triple: smuggled_input(*triple))
    return st.one_of(path, format_, token, environment, smuggled)


def ungatings() -> st.SearchStrategy[Mutation]:
    """Return a strategy for mutations deleting the upload's gate.

    Returns
    -------
    st.SearchStrategy
    """
    return st.one_of(st.none(), st.just("")).map(ungated_upload)


def reversals() -> st.SearchStrategy[Mutation]:
    """Return a strategy for mutations inverting the upload's gate.

    Each arm names the credential, reads it from the context the gate can see,
    and opens on exactly the run the gate exists to skip. A contract that asked
    only whether the credential were *named* would accept every one of them.

    Returns
    -------
    st.SearchStrategy
    """
    gate = CREDENTIAL_GATE_NAMESPACE
    key = CREDENTIAL_ENVIRONMENT_KEY
    return st.sampled_from([
        f"{gate}.{key} == ''",
        f"!{gate}.{key}",
        f"{gate}['{key}'] == ''",
        f"{gate}[ '{key}' ] == ''",
        f"'' == {gate}.{key}",
        f"${{{{ {gate}.{key} == '' }}}}",
        f"${{{{ {gate}['{key}'] == '' }}}}",
        f"${{{{ !{gate}.{key} }}}}",
    ]).map(reversed_gate)


def weakenings() -> st.SearchStrategy[Mutation]:
    """Return a strategy for mutations naming the credential without gating.

    Three ways to name it and gate on nothing, each of which a contract reading
    only for the name would accept: the bare reference, which is not a
    comparison at all; a quoted literal that spells the reference exactly, which
    GitHub reads as text; and the same name reached through a context the
    condition cannot compare, which resolves to the empty string on every run.

    Returns
    -------
    st.SearchStrategy
    """
    gate = CREDENTIAL_GATE_NAMESPACE
    key = CREDENTIAL_ENVIRONMENT_KEY
    bare = st.sampled_from([
        f"{gate}.{key}",
        f"{gate}['{key}']",
        f"${{{{ {gate}.{key} }}}}",
    ])
    spelled = st.just(f"${{{{ '{gate}.{key}' != '' }}}}")
    elsewhere = _other_than(gate).map(
        lambda context: f"${{{{ {context}.{key} != '' }}}}"
    )
    unimplemented = st.sampled_from([
        "github.event_name == 'push'",
        "runner.os == 'Linux'",
        f"env.NOT_{key} != ''",
        f"${{{{ vars.{key} != '' }}}}",
    ])
    return st.one_of(bare, spelled, elsewhere, unimplemented).map(weakened_gate)


def payloads() -> st.SearchStrategy[dict[str, object]]:
    """Return a strategy for a step the contract is not stated over.

    The name is prefixed, so a drawn payload can never collide with a guarded
    step: a collision would be a rename dressed as an insertion, and the
    property stated over it would be claiming something about a lane the
    contract reads rather than about the lanes it ignores.

    The payloads are drawn to be *plausible*, not merely harmless. Two of them
    spell the validator's path and the report's path in a `run` block, which is
    text the validating step's own clauses would read if they were stated over
    the lane instead of over the step: a scan that had lost that scoping would
    report a lane that is correct.

    Returns
    -------
    st.SearchStrategy
    """
    name = st.from_regex(r"[A-Za-z_][A-Za-z0-9_]{0,10}", fullmatch=True).map(
        lambda drawn: f"unrelated: {drawn}"
    )
    action = st.sampled_from([
        ("actions/checkout", "0" * 40),
        ("astral-sh/setup-uv", "1" * 40),
        ("leynos/shared-actions", "v1"),
    ]).map(lambda pair: f"{pair[0]}@{pair[1]}")
    prose = st.sampled_from([
        f"echo {REPORT_VALIDATOR_SCRIPT}",
        f"test -s {COVERAGE_REPORT_PATH}",
        f"cat {COVERAGE_REPORT_PATH}",
        "echo ready",
    ])
    return st.one_of(
        st.tuples(name, action).map(lambda pair: {"name": pair[0], "uses": pair[1]}),
        st.tuples(name, prose).map(lambda pair: {"name": pair[0], "run": pair[1]}),
        st.tuples(name, prose).map(
            lambda pair: {
                "name": pair[0],
                "if": "${{ vars.UNDECLARED != '' }}",
                "run": pair[1],
            }
        ),
        st.tuples(name, action).map(
            lambda pair: {
                "name": pair[0],
                "uses": pair[1],
                "with": {"publish": "false"},
            }
        ),
    )


def payload_positions() -> st.SearchStrategy[int]:
    """Return a strategy for a place to put a step the contract ignores.

    The range runs one past the last guarded step as well as between them, so an
    inserted step is reached at both ends of the lane: a rule stated over a
    window of the lane rather than over the steps in it would report only the
    positions that fall inside that window.

    Returns
    -------
    st.SearchStrategy
    """
    return st.integers(min_value=0, max_value=len(REPORT_STEP_NAMES))


def inserted_steps() -> st.SearchStrategy[Mutation]:
    """Return a strategy for mutations adding a step the contract ignores.

    This is the control. A rule that reported everything would be as useless as
    one that reported nothing, and only a mutation the contract must accept
    separates them; drawing it from the same distribution as the faults is what
    stops a property from passing by generating faults alone.

    Returns
    -------
    st.SearchStrategy
    """
    return st.tuples(payloads(), payload_positions()).map(
        lambda pair: inserted_step(pair[0], pair[1])
    )


def mutations() -> st.SearchStrategy[Mutation]:
    """Return a strategy for one bounded edit to a valid lane.

    Every family is present, the control included.

    Returns
    -------
    st.SearchStrategy
    """
    return st.one_of(
        removals(),
        duplications(),
        reorderings(),
        misbindings(),
        ungatings(),
        reversals(),
        weakenings(),
        inserted_steps(),
    )
