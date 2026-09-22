"""Building one bounded edit to a valid lane, family by family.

``codescene_lane_edit_model`` says what an edit *is* and what the contract must
say about it; this module says how each family is built. The split is the
dependency direction — every constructor here returns a model value, and the
model imports none of them — and it is also what keeps each module within the
size the linters allow.

Each constructor is total over the lane it names: the field, input or step it
edits is one the contract requires, and the clean fixture carries every one of
them, asserted in ``codescene_lane_properties_test``. A mutation against
something the fixture never had would change nothing, and a change that changed
nothing would let the property pass without the contract reading anything.

Which values a generated edit takes is not decided here either; the strategies
in ``codescene_lane_edits`` draw them wider than the contract's accepted set,
so a rebinding that happened to name the expected value cannot stand in for a
real departure.

Run via ``make test-workflow-contracts``.
"""

import copy
import typing as typ

from codescene_credential_invariants import CREDENTIAL_ENVIRONMENT_KEY
from codescene_lane_edit_model import (
    DUPLICATION,
    INSERTION,
    MISBINDING,
    REMOVAL,
    REORDERING,
    REVERSAL,
    UNGATING,
    WEAKENING,
    Mutation,
    visible_names,
)
from codescene_upload_invariants import CODESCENE_UPLOAD_STEP, REPORT_STEP_NAMES
from codescene_upload_lane_data import inputs_of, step_of

if typ.TYPE_CHECKING:
    import collections.abc as cabc


def _reportable_mutation(
    family: str,
    description: str,
    apply: cabc.Callable[[list[dict[str, object]]], None],
    named: tuple[str, ...],
) -> Mutation:
    """Return a reportable mutation carrying ``named`` as its visible names.

    Every mutation here that the contract must report is built this way, so the
    one field they share — that they are reportable — is stated once rather than
    repeated eleven times. The names are taken ready-made rather than as
    variadic strings, because the factories that pass through ``visible_names``
    have already decided them, and a helper that re-derived them would be a
    second opinion about the same rule.

    Returns
    -------
    Mutation
        The mutation and the verdict the contract must return on it.
    """
    return Mutation(family, description, apply, reportable=True, named=named)


def index_of(steps: cabc.Sequence[dict[str, object]], name: str) -> int:
    """Return the position of the uniquely named step ``name``."""
    return next(
        position for position, step in enumerate(steps) if step is step_of(steps, name)
    )


def removed_step(name: str) -> Mutation:
    """Return a mutation deleting the step called ``name``."""

    def apply(steps: list[dict[str, object]]) -> None:
        """Delete the named step from ``steps``."""
        steps.pop(index_of(steps, name))

    return _reportable_mutation(
        REMOVAL,
        f"{name!r} removed from the lane",
        apply,
        (name,),
    )


def removed_field(step_name: str, field: str) -> Mutation:
    """Return a mutation deleting ``field`` from the step called ``step_name``.

    Returns
    -------
    Mutation
        The mutation and the verdict the contract must return on it.
    """

    def apply(steps: list[dict[str, object]]) -> None:
        """Delete ``field`` from the named step."""
        step_of(steps, step_name).pop(field)

    return _reportable_mutation(
        REMOVAL,
        f"{field!r} removed from {step_name!r}",
        apply,
        visible_names(step_name, field),
    )


def removed_input(step_name: str, input_name: str) -> Mutation:
    """Return a mutation deleting the ``input_name`` input of a guarded step."""

    def apply(steps: list[dict[str, object]]) -> None:
        """Delete ``input_name`` from the named step's inputs."""
        inputs_of(step_of(steps, step_name)).pop(input_name)

    return _reportable_mutation(
        REMOVAL,
        f"{input_name!r} removed from {step_name!r}",
        apply,
        visible_names(step_name, input_name),
    )


def duplicated_step(name: str) -> Mutation:
    """Return a mutation declaring the step called ``name`` a second time.

    The copy is taken from the lane rather than rebuilt, because a duplicate
    that differed from its original in some other field would be two edits in
    one: the contract would have a fault to report whether or not it noticed
    the repetition.

    Returns
    -------
    Mutation
        The mutation and the verdict the contract must return on it.
    """

    def apply(steps: list[dict[str, object]]) -> None:
        """Append a deep copy of the named step to the lane."""
        steps.append(copy.deepcopy(step_of(steps, name)))

    return _reportable_mutation(
        DUPLICATION,
        f"{name!r} declared a second time",
        apply,
        (name,),
    )


def moved_step(name: str, position: int) -> Mutation:
    """Return a mutation moving the step called ``name`` to ``position``.

    Where the step belongs is the model's own statement of the order, not
    something read back out of the lane, so a mutation that leaves the step
    where the contract wants it is recorded as the control it is rather than
    silently being a no-op with a fault claimed for it.

    Returns
    -------
    Mutation
        The mutation and the verdict the contract must return on it.
    """
    expected = REPORT_STEP_NAMES.index(name)

    def apply(steps: list[dict[str, object]]) -> None:
        """Lift the named step out and reinsert it at ``position``."""
        moved = steps.pop(index_of(steps, name))
        steps.insert(position, moved)

    return Mutation(
        REORDERING,
        f"{name!r} moved to position {position}",
        apply,
        reportable=position != expected,
        named=(name,) if position != expected else (),
    )


def swapped_steps(first: str, second: str) -> Mutation:
    """Return a mutation exchanging the positions of two guarded steps."""

    def apply(steps: list[dict[str, object]]) -> None:
        """Exchange the two named steps in place."""
        left, right = index_of(steps, first), index_of(steps, second)
        steps[left], steps[right] = steps[right], steps[left]

    return _reportable_mutation(
        REORDERING,
        f"{first!r} and {second!r} exchanged",
        apply,
        (first, second),
    )


def misbound_input(step_name: str, input_name: str, value: str) -> Mutation:
    """Return a mutation rebinding an input to ``value``."""

    def apply(steps: list[dict[str, object]]) -> None:
        """Rebind ``input_name`` on the named step to ``value``."""
        inputs_of(step_of(steps, step_name))[input_name] = value

    return _reportable_mutation(
        MISBINDING,
        f"{step_name!r} given {input_name}={value!r}",
        apply,
        visible_names(step_name, input_name),
    )


def smuggled_input(step_name: str, input_name: str, value: str) -> Mutation:
    """Return a mutation adding an input the contract requires to be absent."""

    def apply(steps: list[dict[str, object]]) -> None:
        """Supply ``input_name`` on the named step, which must not receive it."""
        inputs_of(step_of(steps, step_name))[input_name] = value

    return _reportable_mutation(
        MISBINDING,
        f"{step_name!r} given {input_name}={value!r}",
        apply,
        visible_names(step_name, input_name),
    )


def rebound_environment(value: str) -> Mutation:
    """Return a mutation reading the credential from the wrong context."""

    def apply(steps: list[dict[str, object]]) -> None:
        """Point the upload's credential environment entry at ``value``."""
        environment = step_of(steps, CODESCENE_UPLOAD_STEP)["env"]
        if isinstance(environment, dict):
            environment[CREDENTIAL_ENVIRONMENT_KEY] = value

    return _reportable_mutation(
        MISBINDING,
        f"the upload reads {CREDENTIAL_ENVIRONMENT_KEY} from {value!r}",
        apply,
        visible_names(CODESCENE_UPLOAD_STEP),
    )


def ungated_upload(condition: str | None) -> Mutation:
    """Return a mutation removing the upload's gate.

    Both spellings of a removed gate are generated: the entry deleted, and the
    entry present with the empty condition the clean lane's entry is not. They
    fail differently — one leaves the contract nothing to compare, the other
    leaves it an empty string — so a contract that recognised only one would
    still be letting an ungated upload through.

    Returns
    -------
    Mutation
        The mutation and the verdict the contract must return on it.
    """

    def apply(steps: list[dict[str, object]]) -> None:
        """Strip the upload's gate, or set it to ``condition``."""
        upload = step_of(steps, CODESCENE_UPLOAD_STEP)
        if condition is None:
            upload.pop("if", None)
        else:
            upload["if"] = condition

    described = "<absent>" if condition is None else repr(condition)
    return _reportable_mutation(
        UNGATING,
        f"the upload's gate set to {described}",
        apply,
        visible_names(CODESCENE_UPLOAD_STEP),
    )


def reversed_gate(condition: str) -> Mutation:
    """Return a mutation whose condition opens on the run the gate must skip."""

    def apply(steps: list[dict[str, object]]) -> None:
        """Set the upload's gate to the reversed ``condition``."""
        step_of(steps, CODESCENE_UPLOAD_STEP)["if"] = condition

    return _reportable_mutation(
        REVERSAL,
        f"the upload's gate reversed to {condition!r}",
        apply,
        visible_names(CODESCENE_UPLOAD_STEP),
    )


def weakened_gate(condition: str) -> Mutation:
    """Return a mutation naming the credential without gating on it."""

    def apply(steps: list[dict[str, object]]) -> None:
        """Set the upload's gate to the weakened ``condition``."""
        step_of(steps, CODESCENE_UPLOAD_STEP)["if"] = condition

    return _reportable_mutation(
        WEAKENING,
        f"the upload's gate weakened to {condition!r}",
        apply,
        visible_names(CODESCENE_UPLOAD_STEP),
    )


def inserted_step(payload: dict[str, object], position: int) -> Mutation:
    """Return a mutation placing an unrelated step at ``position``.

    Nothing is removed and nothing guarded is touched, so the lane remains one
    the contract must accept: the relative order of the guarded steps is
    unchanged, and every one of them still carries what the contract reads.

    Returns
    -------
    Mutation
        The mutation and the verdict the contract must return on it.
    """

    def apply(steps: list[dict[str, object]]) -> None:
        """Insert a deep copy of the unrelated ``payload`` at ``position``."""
        steps.insert(position, copy.deepcopy(payload))

    return Mutation(
        INSERTION,
        f"an unrelated step inserted at position {position}",
        apply,
        reportable=False,
    )
