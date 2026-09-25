"""What the trunk lane must prove about the credential it submits with.

The upload step hands the CodeScene action a token. Two facts about that
arrangement have to hold, and the reason for both is the same: the upload is a
composite action, so its nested steps inherit the calling step's environment.

- The token is taken straight from the secret store. A value read out of an
  environment is a value that reached an environment, and every nested step of
  the action would have been able to read it there.
- The step is gated on the token being available. A fork, or any repository
  that has not set the secret, runs this lane with the token absent; an ungated
  upload then fails the trunk run over a secret that was never required, and a
  gate naming the credential without testing what it published opens on exactly
  the run the gate exists to skip.

Neither the environment nor the comparison is read from this module's own
pattern. The guard is the upload's ``if``, and the rule it has to satisfy —
both conjuncts present as top-level conjuncts, with no unquoted ``||`` — is
``is_trunk_only_upload``'s, imported from the module that states it for the
repository file. Delegating it is what keeps one rule from being written twice:
this module drives the same predicate over the synthetic lane the property
tests generate, so a lane that stopped satisfying it fails here for the same
reason the repository file would fail there.

Separated from ``codescene_upload_invariants`` so neither module outgrows the
400-line limit the Python lint gate enforces.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

from ci_coverage_wiring_invariants import is_trunk_only_upload
from yaml_strings import iter_strings

#: The credential the upload reads. Its name is the secret's as well as the
#: name every mention of it in this repository uses.
CREDENTIAL_ENVIRONMENT_KEY: typ.Final[str] = "CS_ACCESS_TOKEN"

#: The action input the credential is handed through.
CREDENTIAL_INPUT: typ.Final[str] = "access-token"

#: The expression namespace the credential's value must be read from. The
#: secret store is the only source that does not put the token into an
#: environment on the way to the action.
CREDENTIAL_SOURCE_NAMESPACE: typ.Final[str] = "secrets"

#: The exact value the upload must hand the action.
#:
#: Stated whole rather than as "names the credential", because the fault this
#: replaces was a value that named the credential while reading it from
#: somewhere else: `${{ env.CS_ACCESS_TOKEN }}` names it and hands the action
#: whatever the runner's environment held, which is the shape the composite
#: action made dangerous in the first place.
CREDENTIAL_INPUT_VALUE: typ.Final[str] = (
    f"${{{{ {CREDENTIAL_SOURCE_NAMESPACE}.{CREDENTIAL_ENVIRONMENT_KEY} }}}}"
)


def _names_credential(value: object) -> bool:
    """Return whether any string inside ``value`` names the credential.

    Used only to refuse an environment that holds the token, so containment is
    the right question here: a step declaring `UNUSED_CS_ACCESS_TOKEN` has still
    written the credential into an environment every nested step of the
    composite action inherits.

    Returns
    -------
    bool
        `True` when some string in the value contains the credential's name.
    """
    return any(
        CREDENTIAL_ENVIRONMENT_KEY in text for text in iter_strings(value)
    )


def credential_offenders(
    upload: dict[str, object], inputs: dict[str, object]
) -> list[str]:
    """Return every fault in how the upload step receives its credential.

    Parameters
    ----------
    upload
        The parsed upload step, which owns the ``if`` gate and any ``env``.
    inputs
        That step's ``with`` block, already resolved by the caller, so this
        module reads no workflow structure of its own.

    Returns
    -------
    list[str]
        One entry per fault, empty when the step takes the credential from the
        secret, gates on it, and puts it in no environment.
    """
    token = inputs.get(CREDENTIAL_INPUT)
    condition = upload.get("if")
    environment = upload.get("env")

    offenders: list[str] = []
    if token != CREDENTIAL_INPUT_VALUE:
        offenders.append(
            f"the upload step must pass {CREDENTIAL_INPUT} "
            f"{CREDENTIAL_INPUT_VALUE!r} directly, got {token!r}; a value read "
            f"from anywhere else has reached an environment the action's "
            f"nested steps inherit"
        )
    if not is_trunk_only_upload(condition):
        offenders.append(
            f"the upload step must be gated on {CREDENTIAL_ENVIRONMENT_KEY} "
            f"being available and on the trunk ref, got {condition!r}; an "
            f"ungated step fails the trunk run over a missing optional secret, "
            f"and a gate that does not test what the check step published opens "
            f"on exactly the run the gate exists to skip"
        )
    if _names_credential(environment):
        offenders.append(
            f"the upload step must declare no environment holding "
            f"{CREDENTIAL_ENVIRONMENT_KEY}, got {environment!r}; a composite "
            f"action's nested steps inherit the calling step's env"
        )
    return offenders
