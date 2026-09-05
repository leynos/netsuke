# Architecture decision record (ADR): Bound manifest template evaluation

## Status

Accepted.

## Date

2026-09-03

## Context and problem statement

Netsukefiles evaluate MiniJinja expressions while they are parsed, expanded,
rendered, and queried. A compact untrusted manifest could otherwise request
unbounded output, iteration, cloned entries, or instruction work before Ninja
starts.

## Decision

Create one `ManifestBudget` for each manifest load. MiniJinja fuel bounds one
evaluation and the aggregate manifest allowance; a streaming writer bounds a
rendered value and aggregate rendered bytes. Netsuke accounts for template
source, `foreach` cardinality, and aggregate expanded targets and actions.

The defaults are 1,000,000 instructions per evaluation, 100,000,000
instructions per manifest, 1 MiB per rendered value, 16 MiB rendered output, 4
MiB source, 10,000 `foreach` values, and 50,000 expanded entries. Exhaustion
uses a localized, redacted diagnostic and closed-vocabulary telemetry labels.

## Rationale

One budget makes build and manifest-query loading subject to the same resource
contract. Streaming stops output before it is materialized, while lazy iterator
consumption stops expansion before it clones every requested entry.

## Consequences

Large manifests now fail deterministically before host memory or CPU
exhaustion. Operators can lower every ceiling through trusted configuration.
Project configuration is not permitted to widen an operator-established ceiling.

## Alternatives considered

- **Rely on cgroups or CI timeouts.** Rejected because they are late, host-wide,
  and do not provide a manifest-local diagnostic.
- **Use independent local caps.** Rejected because many small fields could evade
  them without shared aggregate accounting.

## Implementation references

- [`src/manifest/budget.rs`](../src/manifest/budget.rs)
- [`src/manifest/expand.rs`](../src/manifest/expand.rs)
- [`src/manifest/render.rs`](../src/manifest/render.rs)
- [`src/manifest/jinja_macros/`](../src/manifest/jinja_macros/)
- [`src/cli/discovery_layers.rs`](../src/cli/discovery_layers.rs)
