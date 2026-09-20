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
  missing optional secret into a failed trunk run. Naming the variable does not
  make the condition a gate: `env.CS_ACCESS_TOKEN == ''` and
  `!env.CS_ACCESS_TOKEN` both name it and both open on that same run, so the
  condition has to compare it for inequality with the empty string.

All three ask the same question — is this value the credential, read from the
namespace that makes it one — and all three are answered by enumerating the
references an expression names rather than by searching its text. The third adds
an operator to that question: a condition naming the credential is not yet gated
on it, so the comparison *beside* the reference is read, and it is read from
where the reference was found rather than from an operand captured as text. A
pattern deciding what an operand is before it knows where one ends has to fix a
spelling, and the two spellings differ: `env['X'] != ''` closes its bracket
against the quote, `env[ 'X' ] != ''` against a space. A substring check gets
each of these wrong in its own way:

- `CS_ACCESS_TOKEN` is a substring of `NOT_CS_ACCESS_TOKEN`, so a gate on
  `env.NOT_CS_ACCESS_TOKEN != ''` satisfied it. That name is unset, so the
  condition compares `''` with `''`, never opens, and the lane reads as gated
  while submitting nothing at all. The same containment trap would have
  accepted `env.TOKEN_CS_ACCESS_TOKEN`.
- A declared value of `env.secrets.CS_ACCESS_TOKEN` contains `secrets.` while
  reading the field `CS_ACCESS_TOKEN` off the step's own environment. It has to
  be an identifier in the ``secrets`` namespace, not a value mentioning one.
- Quoted text names nothing. GitHub's expression grammar treats a name as a
  reference only when it is written unquoted, so `${{ 'env.CS_ACCESS_TOKEN' !=
  '' }}` is a comparison of a non-empty string literal against the empty string
  — always true, gating nothing — and must not satisfy the gate check.

The namespace is part of each answer. The ``if`` is evaluated against ``env``,
so a gate written against ``secrets`` names the same secret through a namespace
the condition cannot read, and is not evidence that the step is gated on the
variable it exported.

Separated from ``codescene_upload_invariants`` so neither module outgrows the
400-line limit the Python lint gate enforces.

Run via ``make test-workflow-contracts``.
"""

import re
import typing as typ

from workflow_variable_scan import expression_references, reference_occurrences

#: The credential the upload reads. The step must be gated on it rather than
#: running with an empty value, and the value must come from a secret.
CREDENTIAL_ENVIRONMENT_KEY: typ.Final[str] = "CS_ACCESS_TOKEN"
CREDENTIAL_INPUT: typ.Final[str] = "access-token"

#: The expression namespace a credential must be read from. Named separately
#: from [`CREDENTIAL_GATE_NAMESPACE`] because the two are read in different
#: places: the secret store is the only source a value may come from, and the
#: step's own environment is the only thing its ``if`` can compare.
CREDENTIAL_SOURCE_NAMESPACE: typ.Final[str] = "secrets"

#: The namespace the upload's `if` gate must read the credential from. The
#: condition is evaluated against `env`, so this is the namespace that proves
#: the step is gated on the variable it exported rather than on any same-named
#: value from elsewhere.
CREDENTIAL_GATE_NAMESPACE: typ.Final[str] = "env"

#: Matches the comparison *after* a reference: `!= ''`, with nothing but space
#: allowed between the operator and the literal, and the reference itself left
#: out of the pattern entirely.
#:
#: Neither pattern reads a name. The name, the namespace, and the distinction
#: between a reference and a quoted run are all decided by
#: `reference_occurrences`, so what remains here is only the operator and the
#: literal — the text the reference's own span does not cover. For the pattern
#: to read a name as well would be a second, weaker reading of the same
#: grammar, and the two readings would have to agree about quoting: growth in
#: either would let `'env.X' != ''` satisfy a check whose reference reader had
#: already called it a literal.
PRESENT_COMPARISON: typ.Final[re.Pattern[str]] = re.compile(r"\s*!=\s*''")

#: The same comparison with the operand on the right, so that `'' != env.X`
#: counts. Applied to the text *before* the reference, anchored at its right
#: end, which is what keeps it from matching a comparison further up the
#: condition. Both orders are accepted because a gate is not wrong for reading
#: more naturally one way round than the other.
PRESENT_COMPARISON_REVERSED: typ.Final[re.Pattern[str]] = re.compile(r"''\s*!=\s*$")


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


def gates_on_credential(value: object) -> bool:
    """Return whether ``value`` gates on the credential being non-empty.

    Naming the credential is not gating on it. Every condition below names it
    and none of them waits for it to be present:

    - `env.CS_ACCESS_TOKEN == ''` runs the step *precisely* when the secret is
      absent, and skips every authenticated upload — the inverse of a gate.
    - `!env.CS_ACCESS_TOKEN` is true on the same run, since the empty string is
      falsy.
    - `env.CS_ACCESS_TOKEN` alone is the variable itself rather than a
      comparison, and a non-empty token coerces to `true` while an absent one
      coerces to `false` — so it gates on presence by accident, through
      coercion, rather than by the comparison the lane means.

    Read as an identifier check alone, each of these satisfies the contract
    while leaving the trunk run to fail over an optional secret. So the
    comparison is read: the text immediately after the credential has to be the
    `!= ''` the lane means, with the credential on the left. The mirror
    spelling — `'' != env.X` — is read by the same test with the positions
    swapped, so neither order is favoured.

    The comparison is read from the *reference's position* rather than from an
    operand captured as text. An operand pattern has to decide where an operand
    ends before it knows what the operand is, and the two spellings of an index
    end differently: `env['X'] != ''` has no space inside the brackets and
    `env[ 'X' ] != ''` does, so a pattern reading a run of non-space characters
    captured `]` as the operand and reported a real gate as though it gated on
    nothing. Reading forward from where the reference was found asks the
    question the contract actually means — is this comparison against the
    empty string — of either spelling alike.

    Parameters
    ----------
    value
        The candidate condition, which is a ``str`` only when the step declared
        one. A step's ``if`` is an expression without the delimiters, so it is
        scanned whole.

    Returns
    -------
    bool
        Whether some comparison in the condition puts the credential on one
        side of a `!= ''`.
    """
    if not isinstance(value, str):
        return False
    return any(
        namespace == CREDENTIAL_GATE_NAMESPACE
        and name == CREDENTIAL_ENVIRONMENT_KEY
        and _compares_present(value, start, end)
        for namespace, name, start, end in reference_occurrences(value, bare=True)
    )


def _compares_present(value: str, start: int, end: int) -> bool:
    """Whether the comparison around a reference tests it against `''`.

    The reference occupying ``start:end`` is on one side of an inequality with
    the empty string literal, and nothing but space may come between the
    reference and the operator — so the reference has to *be* the operand, not
    a term somewhere earlier in a longer condition. Either order counts, so
    `env.X != ''` and `'' != env.X` are read alike.

    Reading the operator from the reference's own span is what keeps a
    neighbouring comparison from being read as this one. A pattern capturing an
    operand as a run of text has the opposite problem: to admit the spaced
    index spelling `env[ 'X' ] != ''` it has to let an operand contain a space,
    and it then reads `env.X == '' && y != ''` as one operand spanning
    `env.X == ''` — accepting a condition whose credential comparison is the
    *inverted* one.

    Returns
    -------
    bool
        Whether the reference is compared against the empty string.
    """
    if PRESENT_COMPARISON.match(value[end:]) is not None:
        return True
    return PRESENT_COMPARISON_REVERSED.search(value[:start]) is not None


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
    if not names_credential(declared, namespace=CREDENTIAL_SOURCE_NAMESPACE):
        offenders.append(
            f"the upload step must declare "
            f"env.{CREDENTIAL_ENVIRONMENT_KEY} from a github secret, got "
            f"{declared!r}"
        )
    # The value handed to the action has to be the one the gate compared, and
    # the gate reads `env`. A reference from another namespace — `github.`, or
    # `vars.`, which this repository declares no variables in — names a
    # same-named value from elsewhere, which resolves to the empty string and
    # leaves the action unauthenticated while the step reads as configured.
    if not names_credential(token, namespace=CREDENTIAL_GATE_NAMESPACE):
        offenders.append(
            f"the upload step must pass {CREDENTIAL_INPUT} the "
            f"{CREDENTIAL_ENVIRONMENT_KEY} it gated on, got {token!r}"
        )
    if not gates_on_credential(condition):
        offenders.append(
            f"the upload step must be conditional on "
            f"{CREDENTIAL_ENVIRONMENT_KEY} being present, got {condition!r}; an "
            f"ungated step fails the trunk run over a missing optional secret, "
            f"and a condition naming the credential without comparing it "
            f"against non-emptiness opens on exactly the run the gate exists "
            f"to skip"
        )
    return offenders
