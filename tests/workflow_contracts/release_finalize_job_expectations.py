"""State the job shape the finalization contract permits.

The guard and the step sequence are expectations rather than loaders: they say
what the job must be, and the contract test compares the checked-in workflow
against them. They live apart from ``release_finalize_contract_support`` so the
loading code and the mutation set stay about mechanics, while this module is
read as the job's specification.

Run via ``make test-workflow-contracts``.
"""

from release_finalize_contract_support import (
    ARTIFACT_ACTION,
    CHECKOUT_ACTION,
    EXPECTED_NEEDS,
    LOCAL_ACTION,
    SETUP_UV_ACTION,
    TRUSTED_ARTIFACT_OUTPUT,
)
from workflow_loading import require_mapping

#: The one condition the job may carry, normalised to single spaces. Every need
#: is named because `!cancelled()` suppresses GitHub's implicit success check,
#: and the rehearsal flag is what keeps a dry run from publishing. The whole
#: expression is pinned rather than its fragments: a trailing `|| true` leaves
#: every fragment intact while letting a failed verifier start the job.
EXPECTED_JOB_GUARD = " && ".join((
    "!cancelled()",
    *(f"needs.{name}.result == 'success'" for name in EXPECTED_NEEDS),
    "needs.metadata.outputs.should_publish == 'true'",
))

#: The exact steps the job may run, in the only order it may run them. An
#: action that runs before its records arrive fails; an extra step could change
#: the release's visibility before the action reads it.
EXPECTED_JOB_STEPS = (
    (CHECKOUT_ACTION,),
    (SETUP_UV_ACTION,),
    ("download", ARTIFACT_ACTION, "release-crate-package", "release-crate-package"),
    ("download", ARTIFACT_ACTION, TRUSTED_ARTIFACT_OUTPUT, "trusted"),
    (LOCAL_ACTION,),
)


def step_signature(step: dict[str, object]) -> tuple[str, ...]:
    """Reduce a job step to the identity the sequence contract pins.

    Parameters
    ----------
    step
        One step of the finalization job.

    Returns
    -------
    tuple[str, ...]
        The action reference alone for an action step, or the reference, the
        artefact name, and the destination for a record download. A `run` step
        reduces to an empty string, which no expected entry matches: the job is
        composed of reviewed actions rather than inline commands.
    """
    uses = str(step.get("uses", ""))
    if uses.startswith(ARTIFACT_ACTION):
        values = require_mapping(step.get("with"), "artifact inputs")
        return (
            "download",
            uses,
            str(values.get("name")),
            str(values.get("path")),
        )
    return (uses,)
