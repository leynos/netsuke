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
macro call, or render refunds its unused reservation. `CappedWriter` checks
bytes before accepting them into the buffer it controls, enforcing the
applicable per-value and aggregate rendered-byte allowances at that boundary.
Successful rendering therefore materializes a bounded result, but this does not
promise zero allocation, an exact allocated capacity, or a bound on total
process memory. Netsuke accounts for template source, `foreach` cardinality,
and aggregate expanded targets and actions.

MiniJinja internally materializes macro results and captures before an outer
writer can reject them. The compiled-expression fallback's post-hoc
`charge_macro_output` check detects an oversized completed result but does not
prevent its initial allocation. The same limitation applies to native macro
calls and nested internal captures. This remaining gap is tracked in
[#720](https://github.com/leynos/netsuke/issues/720).

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

After a successful full manifest load, the runner continues to call
`record_manifest_structure` for the bounded fixed-vocabulary structural counts
defined by [ADR-009](adr-009-bounded-redacted-manifest-telemetry.md). A budget
failure aborts before that structural telemetry is emitted. Manifest-query
loading suppresses the budget-exhaustion metric and does not invoke the
full-load structural emission path.

## Rationale

One budget makes build and manifest-query loading subject to the same resource
contract. Incremental checking at the controlled destination rejects bytes
before that destination accepts them, while allowing successful output to be
materialized within the configured allowance. This does not constrain
allocations made while evaluating expressions, producing filter or function
results, invoking nested macros, or capturing output inside MiniJinja. Lazy
iterator consumption still stops expansion before it clones every requested
entry, and shared manifest accounting prevents many small operations from
bypassing aggregate limits.

## Consequences

Configured checks bound engine-observable fuel, accepted output, accounted
source, and expansion work at their implemented enforcement points, with budget
diagnostics when those checks fire. Fuel is engine instruction accounting, not
a complete wall-clock or CPU-time limit; it does not meter every allocation or
all work performed inside a native filter or host callback. These budgets
reduce resource-exhaustion exposure but are not a general in-process sandbox or
a whole-interpreter memory limit. Internal macro and capture buffering remains
tracked by [#720](https://github.com/leynos/netsuke/issues/720).

Operators can lower every ceiling through trusted configuration. Project
configuration, including its `extends` chain, is not permitted to widen an
operator-established ceiling.

## Alternatives considered

- **Rely solely on cgroups or CI timeouts.** A dedicated Linux worker cgroup
  can provide workload-scoped resource containment, so cgroups are not
  inherently host-wide or merely late. This is not the sole decision because it
  adds platform and deployment requirements, provides coarser failure
  attribution, and does not provide the same manifest-local diagnostic when the
  operating system terminates a worker. Process isolation remains possible
  complementary defence in depth, but is not implemented here. Memory
  containment also does not by itself contain CPU use.
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

## Addendum A: Corrected guarantee and residual risk (2026-09-16)

This addendum corrects the Decision, Rationale, Consequences, and cgroup
alternative above. The original wording overstated how far streaming output
checks reach, suggested protection against all host memory and CPU exhaustion,
and inaccurately described cgroups as inherently host-wide. Those claims did
not match the supported MiniJinja API or the enforcement points implemented by
[#670](https://github.com/leynos/netsuke/pull/670).

The corrected guarantee is narrower: `CappedWriter` checks each write before
the controlled destination accepts bytes beyond its per-value or aggregate
allowance. Accounted source, lazy `foreach` consumption, expansion counts, and
MiniJinja fuel are likewise enforced at their respective boundaries. These
checks produce a bounded diagnostic when they fire, but do not promise
allocation-free evaluation, an exact capacity bound, or whole-process memory
containment. The internal macro and capture gap is deferred, not resolved, in
[#720](https://github.com/leynos/netsuke/issues/720).

Terra's finding is a limitation of the supported MiniJinja API and the original
two-file scope, not an inherently insoluble problem. The outer
`render_captured_to` API controls the destination for outer template output;
native macro calls and internal captures can instead use separate, engine-owned
buffers. This includes the compiled-expression fallback and ordinary imported
macros, along with nested macro calls and caller or block captures. The
fallback's post-hoc check can detect an oversized completed result, but it
cannot prevent that result from being allocated first.

The budgets therefore reduce, rather than eliminate, resource-exhaustion
exposure. [#720](https://github.com/leynos/netsuke/issues/720) records the
deferred remedy and the required early-termination coverage; it does not
establish that the current implementation provides that coverage.

## Addendum B: Dependency baseline and deferred engine work (2026-09-16)

[#719](https://github.com/leynos/netsuke/issues/719) is the separate immediate
upgrade to MiniJinja 2.24.0. Its changelog describes an upstream
string-repetition guard with a 100 MB threshold. That guard limits one
operation; it is not Netsuke's configurable per-value quota and does not repair
aggregate output or internal macro and capture buffering. The compatible
version requirement in `Cargo.toml` is not evidence of the currently resolved
version. Netsuke now declares
`minijinja = { version = "2.24.0", features = ["fuel", "loader"] }`, and its
validated lockfile resolves 2.24.0. The upgrade preserves manifest rendering
and budget behaviour, but does not close the internal macro/capture allocation
gap.

The engine patch and 3.x migration in
[#720](https://github.com/leynos/netsuke/issues/720) remain deferred until the
Rust crate's MiniJinja 3.0.0 final release. No alpha, beta, release candidate,
or moving branch satisfies that gate. The final release triggers reinspection
of the upstream API and buffering implementation. Prefer a released upstream
solution if it meets the requirements; do not assume that a fork will still be
necessary.

The proposed upstreamable remedy is a generic budget-aware internal
output/capture buffer, or an equivalent streaming mechanism that propagates
enforcement through nested macros and captures. The required property is that
an append is rejected before a protected buffer accepts bytes beyond its
allowance. Bounded materialization is allowed. Mutable `State` and render-local
extensions may carry accounting state, but they do not enforce a quota unless
every relevant buffer-growth path consults that state.

The design must distinguish intermediate generated bytes from final rendered
bytes. It must avoid both accidental double charging and fresh aggregate
allowances for nested calls. It must preserve native positional and keyword
arguments, defaults, caller blocks, escaping and safe strings, fuel reservation
and refund, and `OutOfFuel` mapping. Typed output-budget exhaustion must reach
Netsuke's adapter as `ErrorKind::WriteFailure`.

Any temporary patch needs an immutable baseline, upstream references,
validation across supported distribution paths, and an explicit retirement
condition. The future regression suite must prove early termination with small
fixtures, not merely observe a final budget error: cover literal output, nested
macros, and captures whose eventual final output is small. The detailed plan is
tracked in [#720](https://github.com/leynos/netsuke/issues/720).

Broader allocator accounting and process isolation are separate concerns from
this output-buffer work. Neither is delivered by this ADR correction.

## Addendum C: Verified MiniJinja 2.24.0 API constraints (2026-09-17)

The finding behind Addendum A is correct about the mechanism: `invoke_macro`
calls `Value::call`, and MiniJinja materializes the complete macro result before
`charge_macro_output` measures it. Capping that result before materialization
is not achievable against the pinned MiniJinja 2.24.0 using only its public API.

- `Value::call` accepts only `&State` and `&[Value]`; it takes no writer, and
  no `call_to` or `call_with_output` variant exists in the crate.
- Every `Output` constructor is crate-private: `new`, `null`,
  `begin_capture`, `end_capture`, and `retarget`. The only public constructor,
  `machinery::make_string_output`, is behind `unstable_machinery`, which
  netsuke does not enable. `Cargo.toml` declares
  `features = ["fuel", "loader"]`, and `--all-features` cannot enable a
  dependency's feature.
- Enabling that feature would not close the route either: `Vm::eval_macro`
  needs a `macro_id`, a `closure`, and a matching `&State`, none of which are
  publicly obtainable.
- `Template::render_captured_to` installs its writer on the top-level
  `Output` of the outer template. A macro invoked as an expression compiles to
  `Instruction::CallObject`, which dispatches to `args[0].call(...)` and so
  never receives that `out`.

The macro body writes into an engine-local `String` and returns it as a
`Value`; netsuke's `let rendered: String = rendered_value.into()` then formats
that result again through `impl From<Value> for String`, so the output is
materialized twice and the reviewer's phrasing understates that cost. Fuel
cannot bound the growth either, because capture and macro instructions are
charged zero fuel.

Addendum B's deferral to [#720](https://github.com/leynos/netsuke/issues/720)
therefore stands unchanged. This addendum reinforces Addenda A and B rather
than contradicting them, and does not claim the guarantee is satisfied. It also
cannot confirm from local evidence that MiniJinja 3.0.0-alpha.1 repairs the
gap: the local registry cache holds only 2.19.0, 2.21.0, and 2.24.0 sources.

## References

- [#651](https://github.com/leynos/netsuke/issues/651) and
  [#670](https://github.com/leynos/netsuke/pull/670): original budget work and
  investigation.
- [#719](https://github.com/leynos/netsuke/issues/719): independent immediate
  upgrade to MiniJinja 2.24.0, whose repetition guard does not solve this gap.
- [#720](https://github.com/leynos/netsuke/issues/720): deferred internal
  macro and capture output-budget remediation.
- [MiniJinja 2.24.0 changelog](https://github.com/mitsuhiko/minijinja/blob/2.24.0/CHANGELOG.md).
- [MiniJinja 2.24.0 `Value::call`](https://github.com/mitsuhiko/minijinja/blob/2.24.0/minijinja/src/value/mod.rs).
- [MiniJinja 2.24.0 output/capture buffers](https://github.com/mitsuhiko/minijinja/blob/2.24.0/minijinja/src/output.rs).
- [MiniJinja 2.24.0 macro buffering](https://github.com/mitsuhiko/minijinja/blob/2.24.0/minijinja/src/vm/macro_object.rs).
- [MiniJinja 2.24.0 macro evaluation](https://github.com/mitsuhiko/minijinja/blob/2.24.0/minijinja/src/vm/mod.rs).
- [MiniJinja 3.0.0-alpha.1 macro buffering](https://github.com/mitsuhiko/minijinja/blob/3.0.0-alpha.1/minijinja/src/vm/macro_object.rs).
- [MiniJinja 3.0.0-alpha.1 output/capture buffers](https://github.com/mitsuhiko/minijinja/blob/3.0.0-alpha.1/minijinja/src/output.rs).
- [MiniJinja 3.0.0-alpha.1 changelog](https://github.com/mitsuhiko/minijinja/blob/3.0.0-alpha.1/CHANGELOG.md).
- [Linux cgroup v2 documentation](https://docs.kernel.org/admin-guide/cgroup-v2.html).
