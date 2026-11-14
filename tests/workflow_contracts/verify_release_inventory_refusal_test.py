"""Execute the inventory verifier against inputs it must refuse.

The verifier's validation step is shell, not a declared interface, so no
structural check can reach it: the action's ``inputs`` block says a value is a
string, not that a non-empty tag is required or that two directories must not
overlap. These cases run the step over a scenario that is otherwise the
matching default, vary one input, and hold three things to account: the run
stops at the validation step, the diagnostic names what is wrong, and nothing
reaches the network.

The cases here vary one declared input at a time. The properties that vary the
*path* — nesting, equality, and symbolic links, including the accepted controls
that keep a refusal from simply refusing everything — live in
``verify_release_inventory_overlap_property_test.py``.

The harness lives in ``verify_release_inventory_harness.py`` and the defaults
these cases vary in ``verify_release_inventory_cases.py``.

Run via ``make test-workflow-contracts``.
"""

import dataclasses
import typing as typ

import pytest
from verify_release_inventory_cases import (
    JOB_INPUTS,
    default_scenario,
    final_diagnostic,
)
from verify_release_inventory_harness import execute
from verify_release_inventory_scenario import Inputs, prepare_nothing

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    import pathlib

    from verify_release_inventory_harness import ActionRun


def _occupy(directory: str) -> cabc.Callable[[pathlib.Path], None]:
    """Return a hook that leaves a file in ``directory`` before the steps run.

    Returns
    -------
    cabc.Callable[[pathlib.Path], None]
        A hook taking the staged workspace.
    """

    def occupy(workspace: pathlib.Path) -> None:
        """Write one leftover file into the download directory."""
        target = workspace / directory
        target.mkdir(parents=True)
        _ = (target / "stale.tar.gz").write_bytes(b"an earlier attempt")

    return occupy


def _block(directory: str) -> cabc.Callable[[pathlib.Path], None]:
    """Return a hook that creates ``directory`` as a file, not a directory.

    Returns
    -------
    cabc.Callable[[pathlib.Path], None]
        A hook taking the staged workspace.
    """

    def block(workspace: pathlib.Path) -> None:
        """Write a regular file where the download directory belongs."""
        _ = (workspace / directory).write_bytes(b"not a directory")

    return block


@dataclasses.dataclass(frozen=True, slots=True)
class Fault:
    """Describe one input the action must refuse.

    Attributes
    ----------
    name
        The case id, naming the fault rather than the value that carries it.
    inputs
        The values the caller hands the action.
    message
        The diagnostic the refusal must carry.
    prepare
        A hook that stages the filesystem state the inputs alone cannot express.
    """

    name: str
    inputs: Inputs
    message: str
    prepare: cabc.Callable[[pathlib.Path], None] = prepare_nothing


#: The input faults the validation step owns. Each is refused before any
#: listing is requested, so a run that reaches the network has already lost.
FAULTS = (
    Fault("empty-tag", Inputs(tag=""), "tag must not be empty"),
    Fault(
        "overlapping-directories",
        Inputs(trusted_directory="release-download"),
        "must be disjoint",
    ),
    Fault(
        "trusted-inside-download",
        Inputs(
            trusted_directory="release-download/trusted",
            declaration_file="release-download/trusted/declaration.txt",
            trusted_record_file="release-download/trusted/trusted.json",
        ),
        "must be disjoint",
    ),
    Fault(
        "download-inside-trusted",
        Inputs(download_directory="trusted/download"),
        "must be disjoint",
    ),
    Fault(
        "output-inside-trusted",
        Inputs(output_path="trusted/verified.json"),
        "must be disjoint",
    ),
    Fault(
        "output-inside-download",
        Inputs(output_path="release-download/verified.json"),
        "must be disjoint",
    ),
    Fault(
        "declaration-outside-trusted",
        Inputs(declaration_file="declaration.txt"),
        "must be inside",
    ),
    Fault(
        "record-outside-trusted",
        Inputs(trusted_record_file="trusted-record.json"),
        "must be inside",
    ),
    Fault(
        "missing-trusted-record",
        Inputs(trusted_record_file="trusted/absent.json"),
        "is not a regular file",
    ),
    Fault(
        "occupied-download-directory",
        JOB_INPUTS,
        "must be empty",
        _occupy("release-download"),
    ),
    Fault(
        "download-path-is-a-file",
        JOB_INPUTS,
        "must be a directory",
        _block("release-download"),
    ),
)


def _assert_trusted_inputs_survive(run: ActionRun) -> None:
    """Require the refusal to leave the publisher's trusted inputs intact.

    The refusal runs before any step writes, so one that removed, truncated, or
    replaced the declaration or the digest record would have destroyed the only
    independent evidence the verification exists to compare against. Each file
    is compared byte for byte with what the harness staged; a check on the
    declaration's names or on the record being non-empty would accept a
    rewritten file that still happened to parse.
    """
    trusted = run.workspace / "trusted"
    assert run.trusted_inputs, "the harness must stage the trusted inputs"
    for name, expected in run.trusted_inputs.items():
        staged = trusted / name
        # The file has to exist before it can be read: a missing one would
        # otherwise surface as an unrelated traceback rather than as this fault.
        assert staged.is_file(), f"a refused run must leave {name} in place"
        assert staged.read_bytes() == expected, (
            f"a refused run must leave {name} byte for byte intact"
        )


@pytest.mark.parametrize("fault", FAULTS, ids=lambda fault: fault.name)
def test_an_input_the_action_cannot_use_is_refused(
    tmp_path: pathlib.Path, fault: Fault
) -> None:
    """Refuse the run at the validation step, naming the input at fault."""
    scenario = default_scenario(inputs=fault.inputs, prepare=fault.prepare)
    run = execute(tmp_path, scenario)
    assert len(run.completed) == 1, "no later step may run on a refused input"
    assert run.completed[0].returncode != 0, final_diagnostic(run)
    assert fault.message in final_diagnostic(run), final_diagnostic(run)
    assert not run.gh_log, "a refused input must not reach the network"
    assert not run.output_path.exists(), "a refused input must leave no record"
    _assert_trusted_inputs_survive(run)
