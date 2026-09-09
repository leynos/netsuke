# Architecture decision record (ADR): Keep project fetch policy below the operator ceiling

## Status

Accepted.

## Date

2026-09-02

## Context and problem statement

Netsuke combines configuration from built-in defaults, system and user files,
the primary project `.netsuke.toml`, environment variables, and CLI options.
That ordinary precedence is suitable for presentation and build preferences,
but it is not a safe merge rule for network-policy grants. A project checkout
is less trusted than the operator who launches Netsuke. If generic scalar
precedence or vector append semantics are applied to fetch policy, a project
can turn off an operator's default-deny setting or add hosts and schemes to an
explicit allowlist.

The configuration-selection seam is owned by the CLI adapter, as recorded in
[ADR-004](adr-004-explicit-config-selection-outside-orthoconfig.md). The
network boundary also follows the least-authority principle recorded for
filesystem capabilities in
[ADR-010](adr-010-scope-glob-capability-to-literal-prefix.md): the source of a
value determines the authority it may exercise.

## Decision

Treat only the primary project `.netsuke.toml` as a project policy request
before generic configuration merging. Discovery preserves primary-file
provenance and removes that layer's `fetch_default_deny`, `fetch_allow_scheme`,
`fetch_allow_host`, and `trust_project_fetch_policy` fields from the generic
layer, while retaining `fetch_block_host` there so blocks remain cumulative.

Without an operator opt-in, reconcile the captured request as follows:

- Project `fetch_default_deny = true` tightens the resolved operator policy.
- Project `fetch_default_deny = false` cannot weaken an operator default-deny
  setting.
- Project allow-scheme and allow-host entries are discarded.
- Block entries continue to accumulate across all layers, and a block wins over
  an allow.

An operator may explicitly set `trust_project_fetch_policy` from system or user
configuration, the `NETSUKE_TRUST_PROJECT_FETCH_POLICY` environment variable, or
`--trust-project-fetch-policy`. The operator value is selected using the
ordinary trusted-layer precedence. With that opt-in, project allow-scheme and
allow-host entries append to the operator values. A present project
`fetch_default_deny` value applies directly. No project layer can
self-authorize because its field is removed before merging.

Reconciliation is owned by the network-policy domain module. It accepts
domain-shaped operator and project requests, returns the reconciled policy and
bounded outcome counts, and has no tracing or metrics side effects. The CLI
adapter only converts configuration values at the seam and writes the result
back to its configuration object. The merge composition boundary emits one
bounded observer event after successful merge and reconciliation; the event
contains trust state, project-request presence, a fixed default-deny decision,
and requested, accepted, and ignored grant counts, never policy values or paths.

## Rationale

Capturing the primary project's request at discovery preserves provenance
without making generic scalar precedence or vector append semantics define the
security boundary. Keeping blocks in the ordinary merge makes their union
monotonic: a lower-trust project can add a restriction but cannot remove one.
Keeping grants out of the generic primary-project layer prevents accidental
authority transfer, while the explicit opt-in provides a clear choice for
operators who trust a checkout. Returning bounded outcome data lets the merge
observer explain the decision without exposing configuration content. Files
loaded through `extends` remain ordinary file layers by design.

## Consequences

- The effective fetch policy is no longer defined by ordinary file precedence
  alone; its grant-bearing fields have an explicit trust-aware reconciliation
  step.
- Project configuration can make fetch access stricter by enabling
  default-deny or adding blocked hosts, but cannot grant new schemes or hosts
  by default.
- Operators who enable the opt-in accept project grants being appended and a
  project default-deny value being applied directly.
- Files loaded through `extends` retain ordinary file-layer semantics; only the
  exact primary project path is quarantined.

## Alternatives considered

- **Exclude all project fetch fields.** Rejected because project default-deny
  and block entries are useful restrictions that can be safely monotonic.
- **Thread provenance through generic merging and intersect allowlists.**
  Rejected for this change because it would expand the merge model and require
  new policy-set operations without improving the explicit trust contract.
- **Merge once for operator values and once for project values.** Rejected
  because the cached discovery and merge pipeline is deliberately one-pass and
  a second pass would duplicate merge-side effects and telemetry.
- **Let the project set the opt-in.** Rejected because an untrusted source must
  not be able to grant itself authority.
- **Instrument reconciliation directly.** Rejected because the domain operation
  must remain a pure policy decision. The explicit merge-observer seam records
  bounded outcome data after successful reconciliation.

## Implementation references

- Discovery, provenance, and project-request capture:
  [`src/cli/discovery_layers.rs`](../src/cli/discovery_layers.rs)
- Domain reconciliation:
  [`src/stdlib/network/policy/reconciliation.rs`][reconciliation-module]
- Configuration composition and observer event:
  [`src/cli/merge.rs`](../src/cli/merge.rs) and
  [`src/cli/merge_observability.rs`](../src/cli/merge_observability.rs)
- Runtime policy evaluation:
  [`src/cli_policy.rs`](../src/cli_policy.rs)
- User-facing policy guidance:
  [`users-guide.md`](users-guide.md#configure-network-access)

## Related decisions

- [ADR-004: Explicit config selection outside OrthoConfig][adr-004]
- [ADR-010: Scope the glob metadata capability to the literal prefix][adr-010]

[adr-004]: adr-004-explicit-config-selection-outside-orthoconfig.md
[adr-010]: adr-010-scope-glob-capability-to-literal-prefix.md
[reconciliation-module]: ../src/stdlib/network/policy/reconciliation.rs
