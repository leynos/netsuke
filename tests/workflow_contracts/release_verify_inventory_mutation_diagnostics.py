"""Say which diagnostic each deliberate mutation must trigger.

A mutation proves a check only when the contract rejects it for the reason the
mutation was written to test. ``gh-api-substitution`` showed the cost of
settling for less: it was rejected, but by the shell reader's refusal of an
unbalanced substitution rather than by the read-only flag check it meant to
exercise, so the flag check it was there to prove could have stopped working
without the suite noticing. Asserting only that *some* assertion failed cannot
tell those two outcomes apart.

Each name below pairs with an entry in
``release_verify_inventory_mutations.ACTION_MUTATIONS`` or
``WORKFLOW_MUTATIONS``, and the contract test requires the rejection message to
contain its fragment. A companion test refuses a mutation that records no
fragment, so a new mutation has to say what its rejection proves before the
suite accepts it.

Run via ``make test-workflow-contracts``.
"""

#: The message fragment each action mutation must be rejected with, keyed by
#: the mutation's name. The fragments name the check, not the whole message, so
#: a rewording of a diagnostic does not silently weaken the association.
ACTION_DIAGNOSTICS = {
    # A release subcommand can write the release the job is only meant to read.
    "gh-release-download": "must not use the release subcommand",
    "gh-release-spacing": "must not use the release subcommand",
    "gh-release-continued": "must not use the release subcommand",
    "gh-release-midword-continued": "must not use the release subcommand",
    "gh-release-escaped-space": "must not use the release subcommand",
    "gh-release-quoted-subcommand": "must not use the release subcommand",
    # A continued or quoted call is counted, which is what shows the scan read
    # it as the call bash makes rather than missing it.
    "gh-api-continued-method": "must make exactly 1 gh api call(s)",
    "gh-api-midword-continued": "must make exactly 1 gh api call(s)",
    "gh-api-escaped-space": "must make exactly 1 gh api call(s)",
    "gh-api-quoted-subcommand": "must make exactly 1 gh api call(s)",
    # These two deliberately write a quoting shape the reader cannot resolve,
    # so the reader's refusal is the intended outcome.
    "gh-api-substitution-quote": "cannot split step line into words",
    "gh-release-substitution-quote": "cannot split step line into words",
    "gh-api-indirect-command": "must name its commands literally",
    "gh-api-indirect-substitution": "must name its commands literally",
    "gh-api-wrapped-command": "must name its commands literally",
    "gh-api-prefixed-command": "must name its commands literally",
    "gh-api-wrapper-option": "must name its commands literally",
    "gh-api-nested-wrapper": "must name its commands literally",
    "gh-api-eval": "must not hand a run-time string to an interpreter",
    "gh-api-shell": "must not hand a run-time string to an interpreter",
    "gh-api-indirect-subcommand": "must name a gh subcommand the contract can read",
    "gh-api-braced-subcommand": "must name a gh subcommand the contract can read",
    # The write-implying flag is the whole point of these cases.
    "gh-api-field": "must stay read-only",
    "gh-api-inline-field": "must stay read-only",
    "gh-api-short-field": "must stay read-only",
    "gh-api-input": "must stay read-only",
    "gh-api-method-override": "must stay read-only",
    "gh-api-short-method": "must stay read-only",
    "gh-api-substitution": "must stay read-only",
    "secret-token": "must not reference a secret",
    "verifier-repeated": "must run exactly one uv command",
}

#: The message fragment each workflow mutation must be rejected with, keyed by
#: the mutation's name.
WORKFLOW_DIAGNOSTICS = {
    "id-token": "verify-release-inventory.permissions",
    "release-dependency": "verify-release-inventory.needs",
    "workflow-token": "must not inherit GH_TOKEN from the workflow",
    "job-token": "must not inherit GH_TOKEN from the job",
    "trusted-in-download": "must forward the contracted verification inputs",
    "artifact-pattern": "must download the artefact the publisher recorded",
    "artifact-literal": "must download the artefact the publisher recorded",
    "publisher-output": "must record the retained artefact's name as an output",
}
