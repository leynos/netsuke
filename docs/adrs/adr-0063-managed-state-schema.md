# Architectural decision record (ADR) 0063: Managed state schema

## Status

Proposed. This decision remains pending review and explicit acceptance.

## Date

2026-10-24

## Context and problem statement

RFC 0021 proposes named state declarations, readiness probes, optional
preparation, and three operations in the structured command union. The schema
must distinguish a condition that can be checked from evidence that preparation
occurred. A failed or incomplete probe must not become permission to run an
installer, and the initial surface must not claim that arbitrary project code
is sandboxed.

## Decision Drivers

- Keep readiness, preparation, and durable preparation evidence distinct.
- Provide useful built-in checks without requiring a package manager or plugin.
- Fail closed when probe evidence is unknown or cannot be inspected.
- Reuse the structured-command boundary and shared operator policy.

## Requirements

### Functional requirements

- Support state kinds `directory`, `file`, `python-venv`, and `custom`.
- Require the `nagios` protocol when an external probe is declared.
- Keep `require_state`, `ensure_state`, and `prepare_state` as separate
  operations with distinct mutation authority.
- Prevent an unknown or unaccepted degraded result from authorising repair.

### Technical requirements

- Use the structured command block from ADR-0062; do not add a state-private
  command parser.
- Bound probe execution by positive time and output limits.
- Permit state operations only in always-run actions or targets with
  `always: true`.
- Merge operator limits under ADR-0062's trusted-ceiling rule.

## Options considered

### Option A: Built-in readiness plus an optional external condition

Use a small fixed set of built-in kinds and allow one bounded external probe to
add a functional condition. Preparation remains explicit and runs at most once
from `ensure_state` after a definite not-ready result.

### Option B: External probes only

This avoids built-in assumptions but forces common presence checks into project
scripts and leaves their semantics and budgets inconsistent.

### Option C: Automatic repair on every failed check

Retrying preparation whenever readiness is not established would turn
permission failures, unsupported inspection, and unknown probe results into
implicit mutation triggers.

## Decision outcome / proposed direction

### State definitions and outcomes

A state declaration has a closed `kind`, kind-specific configuration, optional
`path`, optional `identity`, optional `prepare`, optional `probe`, and optional
`accept_degraded`. Directory and file probes check object type without
following symlinks and make no content-freshness claim. A custom state may
observe a service without a path, but this grants no network access or remote
mutation.

The result algebra is `ready`, `not_ready`, `degraded`, and `unknown`.
`unknown` never satisfies a required condition or authorises preparation.
`degraded` fails by default and does not trigger repair. A state may accept a
degraded result only within trusted operator policy; the warning remains
observable. Inspection errors take precedence over a possible repair decision.

### Identity and operations

`identity.files` contains exact capability-scoped paths, not shell patterns.
When identity is declared, the normalized definition, probe implementation and
version, preparation plan, relevant declared inputs, declared file contents,
and resolved execution settings contribute to identity. External probe files
participate in that identity. Without identity, readiness depends on fresh
probes and requires no preparation record.

`require_state` verifies readiness and never prepares. `ensure_state` verifies,
prepares once only after a definite not-ready result when preparation is
declared and authorised, then verifies again. `prepare_state` explicitly
requests preparation and subsequent verification. All operations are fail-fast;
there is no retry loop or inferred installer.

### External probe protocol and limits

When `probe.external` is present, `probe.protocol` is required and its initial
closed vocabulary is `nagios`. Exit codes 0, 1, 2, and 3 map to `ready`,
`degraded`, `not_ready`, and `unknown`. Other exits, signals, spawn failures,
timeouts, malformed output, and output-budget exhaustion yield `unknown` with
distinct execution reasons.

The default timeout is 10 seconds and the combined stdout/stderr limit is
65,536 bytes. Optional `probe.timeout_seconds` and `probe.max_output_bytes` are
positive integers capped by trusted ceilings `probe_timeout_ceiling_seconds` and
`probe_output_ceiling_bytes`. `probe_external_deny` can prohibit external
probes. The shared configuration layer owns these fields and their provenance
under ADR-0062.

External probes reuse the structured invocation fields allowed by RFC 0021:
`invoke`, `env`, `cwd`, and an allowed `shell`. The runner makes no read-only
or network-sandbox claim. It drains both streams with bounds and terminates and
reaps the owned process tree on timeout, cancellation, or excess output. Probe
output is bounded, escaped, and redacted by the shared diagnostic policy.

State operations are allowed only in actions, which are always-run, or targets
with `always: true`. Incremental targets cannot silently skip readiness checks
because another output is current.

## Goals and non-goals

- Goals:
  - Make readiness explicit and freshly checked at each state operation.
  - Keep repair as a bounded mutation with post-verification.
  - Reuse common command and configuration boundaries.
- Non-goals:
  - Define lease implementation or durable-record retention limits.
  - Claim sandboxing, package-set integrity, or an automatic repair loop.
  - Make bare command recipes depend on state machinery.

## Migration plan

1. Implement pure definitions and readiness outcomes in roadmap item 23.1.1.
2. Specify the `python-venv` inspection contract and any interpreter constraint
   in 23.1.2 before enabling those checks.
3. Add state operations to the shared AST and codec, then implement leases,
   records, probe execution, and operator restrictions in their owning tasks.
4. Keep ordinary builds and the quickstart free of state declarations and
   probe work.

## Known risks and limitations

- A custom external probe is executable project code; the schema cannot prove
  that it is observational or idempotent.
- A lockfile digest records preparation inputs, not current installation
  integrity. The initial `python-venv` probe does not claim package-set
  verification.
- Before leases and records are implemented, this schema alone does not
  coordinate separate Netsuke invocations mutating the same state.

## Outstanding decisions

- Roadmap item 23.1.2 owns the exact `python-venv` inspection contract and any
  interpreter-constraint field.
- Roadmap items 20.2.1 and 20.2.3 own portable lease identity and acquisition;
  merge an ADR amendment before their code merges if that contract changes.
- Roadmap item 23.2.2 owns state-record retention and interrupted recovery;
  merge an ADR amendment before its code merges if those semantics change.
- Roadmap item 23.3.2 owns executable-identity restrictions for external
  probes; merge an ADR amendment before that code merges if policy changes.
- A uv-specific package-integrity probe is outside the initial feature and has
  no implementation owner until separate evidence establishes demand.
