"""Hold the input-path contract to relationships rather than spellings.

The job forwards a directory of trusted evidence, a directory of downloads, and
the record the verifier writes. If any two of them can name the same directory
-- or nest inside each other -- the verification compares the release with
itself. The contract's own default already satisfies the relationship, so a
check that refused everything would pass the contract and still say nothing;
each rejection case here is paired with that accepted control.

Run via ``make test-workflow-contracts``.
"""

import pytest
from release_verify_inventory_paths import assert_disjoint_inputs
from release_verify_inventory_support import EXPECTED_JOB_INPUTS

#: One input each case changes, keyed by the fault it introduces. Every value
#: either repeats a path under another spelling or nests one input in another.
FAULTS: dict[str, dict[str, str]] = {
    "download-equals-trusted": {"download-directory": "trusted"},
    "download-equals-trusted-trailing-slash": {"download-directory": "trusted/"},
    "download-equals-trusted-dot-component": {"download-directory": "./trusted"},
    "output-equals-trusted": {"output-path": "trusted"},
    "download-inside-trusted": {"download-directory": "trusted/download"},
    "output-inside-trusted": {"output-path": "trusted/verified.json"},
    "trusted-inside-download": {
        "trusted-directory": "release-download/trusted",
        "declaration-file": "release-download/trusted/declaration.txt",
        "trusted-record-file": "release-download/trusted/trusted.json",
    },
    "output-inside-download": {"output-path": "release-download/verified.json"},
    "download-under-output-path": {
        "download-directory": "verified/verified-digests.json/download",
    },
    "declaration-equals-trusted": {"declaration-file": "trusted"},
    "record-equals-trusted": {"trusted-record-file": "trusted"},
    "declaration-outside-trusted": {"declaration-file": "declaration.txt"},
}


def _with(change: dict[str, str]) -> dict[str, object]:
    """Return the job's inputs with one case's change applied."""
    return {**EXPECTED_JOB_INPUTS, **change}


@pytest.mark.parametrize("change", FAULTS.values(), ids=FAULTS.keys())
def test_overlapping_inputs_are_rejected(change: dict[str, str]) -> None:
    """Refuse equality and containment in either direction."""
    with pytest.raises(AssertionError):
        assert_disjoint_inputs(_with(change))


def test_the_job_inputs_are_disjoint() -> None:
    """Accept the inputs the job really forwards.

    The control for the cases above: without it, an assertion that rejected
    every call would still satisfy their expectations.
    """
    assert_disjoint_inputs(_with({}))
