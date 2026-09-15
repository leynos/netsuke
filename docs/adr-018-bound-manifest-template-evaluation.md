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
evaluation and consumed fuel across the manifest; each successful expression,
macro call, or render refunds its unused reservation. A streaming writer bounds
a rendered value and aggregate rendered bytes. Netsuke accounts for template
source, `foreach` cardinality, and aggregate expanded targets and actions.

The defaults are 1,000,000 instructions per evaluation, 100,000,000
instructions per manifest, 1 MiB per rendered value, 16 MiB rendered output, 4
MiB source, 10,000 `foreach` values, and 50,000 expanded entries. Exhaustion
uses a localized, redacted diagnostic and closed-vocabulary telemetry labels.
The budget types contain only accounting data; the Jinja adapter owns
localization and MiniJinja error conversion, and the normal full-load boundary
owns exhaustion telemetry. Manifest-query loading selects
`ManifestLoadMode::ManifestQuery`, which suppresses this budget-exhaustion
metric at the loading boundary; its omitted expansion-report observer controls
the separate expansion metrics. Other instrumentation remains owned by the
boundaries that invoke it. Budget values from the project file and every file
in its `extends` chain may only narrow ceilings established by defaults,
trusted configuration, the environment, or CLI flags. Malformed, non-positive,
and unrepresentable budget values are rejected during configuration merge
rather than treated as absent.

## Rationale

One budget makes build and manifest-query loading subject to the same resource
contract. Streaming stops output before it is materialized, while lazy iterator
consumption stops expansion before it clones every requested entry.

## Consequences

Large manifests now fail deterministically before host memory or CPU
exhaustion. Operators can lower every ceiling through trusted configuration.
Project configuration, including its `extends` chain, is not permitted to widen
an operator-established ceiling.

## Alternatives considered

- **Rely on cgroups or CI timeouts.** Rejected because they are late, host-wide,
  and do not provide a manifest-local diagnostic.
- **Use independent local caps.** Rejected because many small fields could evade
  them without shared aggregate accounting.
- **Couple the budget domain to MiniJinja or metrics.** Rejected because the
  accounting model must remain reusable by expansion and rendering, while
  localization and observability belong at their respective adapters.

## Implementation references

- [`src/manifest/budget/`](../src/manifest/budget/)
- [`src/manifest/expand/`](../src/manifest/expand/)
- [`src/manifest/render.rs`](../src/manifest/render.rs)
- [`src/manifest/jinja_macros/`](../src/manifest/jinja_macros/)
- [`src/manifest/budget_adapter.rs`](../src/manifest/budget_adapter.rs)
- [`src/manifest/loading.rs`](../src/manifest/loading.rs)
- [`src/cli/discovery_layers.rs`](../src/cli/discovery_layers.rs)
- [`src/cli/manifest_budget_policy.rs`](../src/cli/manifest_budget_policy.rs)
