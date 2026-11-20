"""Load the release-admission record validator the runtime tests assert with.

The runtime tests run the gate as a child process and decode what it wrote, so
they need a schema that says what a well-formed metric or trace record is. That
schema is the workflow-contract module
``tests/workflow_contracts/release_admission_metrics.py``, which already
validates the records this gate produces in the contract suite; restating it
here would let the two drift into disagreeing oracles.

It is loaded by file path rather than imported, because the test process runs
with ``PYTHONPATH=scripts`` and neither directory is a package. The protocol
below is the narrow face of that module this suite relies on, so a reader can
see the whole contract in one place while the implementation stays shared.

This lives apart from :mod:`release_admission_test_harness` because it is not
part of the subprocess boundary: nothing here starts a process or touches the
gate's files, and the harness module is held to the repository's 400-line
ceiling.
"""

import importlib.util
import typing as typ
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
#: The workflow-contract module whose schema the runtime records must satisfy.
METRICS_VALIDATOR_PATH = (
    REPO_ROOT / "tests" / "workflow_contracts" / "release_admission_metrics.py"
)


class MetricsValidator(typ.Protocol):
    """Define finite-JSON parsing and fixed-label validation operations.

    Enforce ordered schemas, bounded vocabularies, and finite JSON values.
    """

    def parse_metrics(self, lines: list[str]) -> list[dict[str, object]]:
        """Parse finite JSON Lines metric records into mappings.

        Parameters
        ----------
        lines
            JSON Lines containing release-admission metric records.

        Returns
        -------
        list[dict[str, object]]
            Decoded metric objects in input order.
        """

    def validate_metrics(self, records: list[dict[str, object]]) -> None:
        """Validate records against the fixed metric contract.

        Parameters
        ----------
        records
            Decoded metric records to validate.
        """

    def parse_traces(self, lines: list[str]) -> list[dict[str, object]]:
        """Parse finite JSON Lines release-admission trace records.

        Parameters
        ----------
        lines
            JSON Lines containing release-admission trace records.

        Returns
        -------
        list[dict[str, object]]
            Decoded trace objects in input order.
        """

    def validate_traces(self, records: list[dict[str, object]]) -> None:
        """Validate traces against the fixed trace contract.

        Parameters
        ----------
        records
            Decoded trace records to validate.
        """


def load_metrics_validator() -> MetricsValidator:
    """Load the workflow-contract validator without changing the import path.

    Returns
    -------
    MetricsValidator
        Validator implementation loaded from the workflow-contract module.

    Raises
    ------
    AssertionError
        If the workflow-contract module has no importable loader.

    Notes
    -----
    File loading preserves one validator contract without package installation,
    and keeps the record schema shared with the workflow-contract tests rather
    than restated here.
    """
    specification = importlib.util.spec_from_file_location(
        "release_admission_metrics_contract", METRICS_VALIDATOR_PATH
    )
    if specification is None or specification.loader is None:
        message = "the release-admission metrics validator must be loadable"
        raise AssertionError(message)
    module = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(module)
    return typ.cast("MetricsValidator", module)


METRICS_VALIDATOR = load_metrics_validator()


__all__ = (
    "METRICS_VALIDATOR",
    "METRICS_VALIDATOR_PATH",
    "REPO_ROOT",
    "MetricsValidator",
    "load_metrics_validator",
)
