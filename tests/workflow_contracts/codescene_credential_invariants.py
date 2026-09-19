"""What the CodeScene submission lane must prove about its credential.

The trunk lane hands the upload action a token, and the step must be arranged
so that an absent token is not a failure. Three separate facts have to hold,
and each fails differently:

- The step exports the credential into its own environment. Without that the
  ``if`` gate has nothing to compare, because a condition is evaluated against
  ``env`` and not against the secret store.
- The credential is read from a secret. A literal is a credential this
  repository does not own, and one no gate could have compared against.
- The step is conditional on the credential being present. An ungated upload
  runs with an empty token whenever the secret is absent — which is every run
  from a fork, and every run from a repository that has not set it — turning a
  missing optional secret into a failed trunk run.

The middle fact is the one a substring check gets wrong. `CS_ACCESS_TOKEN` is a
substring of `NOT_CS_ACCESS_TOKEN`, and for all three checks it was the text
that was searched: a step gated on `env.NOT_CS_ACCESS_TOKEN != ''` was accepted.
That name is unset, so the condition compares `''` with `''`, never opens, and
the lane reads as gated while submitting nothing at all — a green-looking
contract over a lane that does no work. Asking whether some identifier *is* the
credential answers the question the contract is actually about, so the checks
below enumerate the ``(namespace, name)`` pairs an expression names rather than
searching its text.

The namespace is part of that. The ``if`` is evaluated against ``env``, so a
gate written against ``secrets`` names the same secret through a namespace the
condition cannot read, and is not evidence that the step is gated on the
variable it exported.

Separated from ``codescene_upload_invariants`` so neither module outgrows the
400-line limit the Python lint gate enforces.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

from workflow_variable_scan import expression_references

#: The credential the upload reads. The step must be gated on it rather than
#: running with an empty value, and the value must come from a secret.
CREDENTIAL_ENVIRONMENT_KEY: typ.Final[str] = "CS_ACCESS_TOKEN"
CREDENTIAL_INPUT: typ.Final[str] = "access-token"

#: The expression namespace a credential must be read from. Spelling the
#: prefix rather than the whole expression keeps the check independent of the
#: credential's name and of the whitespace inside the braces.
CREDENTIAL_SOURCE_PREFIX: typ.Final[str] = "secrets."

#: The namespace the upload's `if` gate must read the credential from. The
#: condition is evaluated against `env`, so this is the namespace that proves
#: the step is gated on the variable it exported rather than on any same-named
#: value from elsewhere.
CREDENTIAL_GATE_NAMESPACE: typ.Final[str] = "env"


def names_credential(
    value: object, namespace: str | None = None, *, bare: bool = False
) -> bool:
    """Return whether ``value`` names the credential in an expression.

    The credential is named by an identifier, not by a substring of the text
    around it. A check for containment accepts ``${{ env.NOT_CS_ACCESS_TOKEN }}``,
    whose value is empty in exactly the way the missing secret is: the gate
    would then compare ``'' != ''``, the step would not run, and the contract
    would have passed over a lane that never submits anything.

    Parameters
    ----------
    value
        The candidate, which is a ``str`` only when the step declared one.
    namespace
        The namespace the reference must use, or `None` to accept any. A gate
        is evaluated against ``env``, so requiring that namespace keeps the
        step conditional on the environment variable it actually exported
        rather than on some same-named value from another namespace.
    bare
        Whether the value is itself an expression. Passed through to
        `expression_references`; a step's ``if`` is evaluated as an expression
        without the delimiters, so its value is scanned whole.

    Returns
    -------
    bool
        `True` when some identifier in the value is the credential under an
        accepted namespace.
    """
    if not isinstance(value, str):
        return False
    return any(
        name == CREDENTIAL_ENVIRONMENT_KEY
        and (namespace is None or reference_namespace == namespace)
        for reference_namespace, name in expression_references(value, bare=bare)
    )


def credential_offenders(
    upload: dict[str, object], inputs: dict[str, object]
) -> list[str]:
    """Return every fault in how the upload step receives its credential.

    The credential reaches the action through an expression rather than a
    literal, which is what makes the ``if`` gate mean anything: the step
    evaluates the same value the action is handed. The expression may name the
    secret directly or read the environment variable the step exported it to,
    so the test is that the credential is named, not how it is spelled.

    Parameters
    ----------
    upload
        The parsed upload step, which owns the ``env`` and ``if`` entries.
    inputs
        That step's ``with`` block, already resolved by the caller, so this
        module reads no workflow structure of its own.

    Returns
    -------
    list[str]
        One entry per fault, empty when the step both holds the credential and
        gates on it.
    """
    environment = upload.get("env")
    declared = (
        environment.get(CREDENTIAL_ENVIRONMENT_KEY)
        if isinstance(environment, dict)
        else None
    )
    token = inputs.get(CREDENTIAL_INPUT)
    condition = upload.get("if")

    offenders: list[str] = []
    if not (isinstance(declared, str) and CREDENTIAL_SOURCE_PREFIX in declared):
        offenders.append(
            f"the upload step must declare "
            f"env.{CREDENTIAL_ENVIRONMENT_KEY} from a github secret, got "
            f"{declared!r}"
        )
    if not names_credential(token):
        offenders.append(
            f"the upload step must pass {CREDENTIAL_INPUT} the "
            f"{CREDENTIAL_ENVIRONMENT_KEY} it gated on, got {token!r}"
        )
    if not names_credential(condition, namespace=CREDENTIAL_GATE_NAMESPACE, bare=True):
        offenders.append(
            f"the upload step must be conditional on "
            f"{CREDENTIAL_ENVIRONMENT_KEY} being present, got {condition!r}; an "
            f"ungated step fails the trunk run over a missing optional secret"
        )
    return offenders
