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

Treat the primary project `.netsuke.toml` fetch values as a project policy
request before generic configuration merging. Remove its `fetch_default_deny`,
`fetch_allow_scheme`, `fetch_allow_host`, and `trust_project_fetch_policy`
fields from the generic project layer, while retaining `fetch_block_host` in
that layer so blocks remain cumulative.

Without an operator opt-in, reconcile the captured request as follows:

- Project `fetch_default_deny = true` tightens the resolved operator policy.
- Project `fetch_default_deny = false` cannot weaken an operator default-deny
  setting.
- Project allow-scheme and allow-host entries are discarded.
- Block entries continue to accumulate across all layers, and a block wins over
  an allow.

An operator may explicitly set `trust_project_fetch_policy` from system or user
configuration, the `NETSUKE_TRUST_PROJECT_FETCH_POLICY` environment variable, or
`--trust-project-fetch-policy`. With that opt-in, project allow-scheme and
allow-host entries append to the operator values, and a project
`fetch_default_deny` value applies directly. The project layer cannot
self-authorize the opt-in because its field is removed before merging.

This first boundary covers only the primary project `.netsuke.toml` identified
by project discovery. Files reached through that file's `extends` chain are out
of scope and retain their existing merge semantics until a separate trust
decision is made.

## Rationale

Capturing the project request at discovery preserves provenance at the point
where the project path is known, without threading provenance through the
generic merge library or running a second merge pass. Keeping blocks in the
ordinary merge makes their union monotonic: a lower-trust project can add a
restriction but cannot remove one. Keeping grants out of the project layer
prevents accidental authority transfer, while the explicit opt-in provides a
clear choice for operators who trust a checkout.

## Consequences

- The effective fetch policy is no longer defined by ordinary file precedence
  alone; its grant-bearing fields have an explicit trust-aware reconciliation
  step.
- Project configuration can make fetch access stricter by enabling
  default-deny or adding blocked hosts, but cannot grant new schemes or hosts
  by default.
- Operators who enable the opt-in accept project grants being appended and a
  project default-deny value being applied directly.
- The `extends` limitation is intentional. It leaves existing semantics for
  extended files and identifies a follow-up boundary rather than implying that
  every file in an extends chain has the primary project's trust level.

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

## Implementation references

- Discovery and project-request capture:
  [`src/cli/discovery_layers.rs`](../src/cli/discovery_layers.rs)
- Configuration composition and reconciliation:
  [`src/cli/merge.rs`](../src/cli/merge.rs)
- Runtime policy evaluation:
  [`src/cli_policy.rs`](../src/cli_policy.rs)
- User-facing policy guidance:
  [`users-guide.md`](users-guide.md#configure-network-access)

## Related decisions

- [ADR-004: Explicit config selection outside OrthoConfig][adr-004]
- [ADR-010: Scope the glob metadata capability to the literal prefix][adr-010]

[adr-004]: adr-004-explicit-config-selection-outside-orthoconfig.md
[adr-010]: adr-010-scope-glob-capability-to-literal-prefix.md
