# RFC 0016: Named contention classes

## Preamble

- **RFC number:** 0016
- **Status:** Proposed
- **Created:** 2026-09-19
- **Scope:** Optional concurrency limits lowered to Ninja pools
- **Implementation:** [Progressive-enhancement roadmap, phase 22][roadmap]

## 1. Summary

Allow an action to name a contention class when it should share a bounded number
of concurrent execution slots with other actions. Lower that declaration to
Ninja pools. Keep Ninja as the scheduler, preserve dependencies as ordering and
data requirements, and leave unannotated actions unchanged.

A simple build needs no resource model. One class and one scalar annotation must
suffice to prevent two cooperating native-build actions from overlapping. No
toolchain, state, context, ownership declaration, or typed input is required.

## 2. Problem and existing capabilities

Repositories use serial aggregates and independent worker flags to manage shared
caches and expensive native builds. Ordering requirements, contention, and
subprocess parallelism are different concepts. Serializing an aggregate can
unnecessarily block independent work while failing to constrain unrelated
actions that use the same resource.

The current intermediate representation (IR) already has `Action.pool`. This RFC
supplies the declaration, resolution, bounds, provenance, and backend contract
around that existing concept, not a second resource scheduler. Ninja pools limit
concurrent edges and remain subject to Ninja's global job limit.[^1] They do not
limit the number of threads spawned by one edge.

## 3. Progressive authoring

This proposed fragment serializes two native-build commands while leaving other
actions eligible for normal scheduling:

```yaml
contention_classes:
  native-build:
    capacity: 1

actions:
  - name: rust-test
    contention: native-build
    command: cargo test
  - name: extension
    contention: native-build
    command: maturin develop
```

The absence of `contention` retains the ordinary backend scheduling policy.
Plain legacy commands can opt in: structured-command adoption is not required
for a pool annotation. Capacity is a positive integer; there is no required
worker-count calculation or platform-detection preamble.

## 4. Definition and resolution

`contention_classes` maps names to definitions containing `capacity`. An
executable action or target may set one scalar `contention` reference. Literal
capacities work independently; typed input expressions may be added through [RFC
0014][inputs] without making that feature a prerequisite.

Reject zero, negative, fractional, Boolean, unbounded, and out-of-range
capacities. Unknown fields, duplicate declarations, missing references, and
reserved internal names fail validation. Apply an operator maximum as a hard
ceiling and report the effective capacity and its provenance; malformed values
never become a default or silently coerce to integers.

An action can belong to at most one class in the initial release. A list is an
error, not an implicit request for several locks. An explicit rule annotation
can supply a default when an action has none; conflicting explicit references
must fail rather than depend on include order. The implementation must settle
this interaction with the actual recipe inheritance contract before enabling
rule-level annotations. Initial delivery may support action/target annotations
only, with an explicit unsupported-field diagnostic on rules.

Dependency-only aggregates cannot hold a slot because they execute no command.
Reject aggregate annotations with a remedy naming the executable children; do
not infer recursive inheritance across their dependency closure. An annotation
on a consumer neither constrains its prerequisites nor changes their order.

## 5. Scheduling and lowering

Resolve public names to stable internal class identities before backend
emission. Emit one Ninja pool per used class and attach its identity to the
corresponding executable edges. Unused classes emit no pool or runtime work. The
complete multi-command action holds one edge slot until it finishes or fails;
releasing slots between commands would weaken the declared contract.

A class changes eligibility for concurrent dispatch, not the dependency graph's
meaning. It does not add dependencies, deduplicate actions, guarantee fairness,
or determine completion order among eligible actions. Preserve the existing
serial-dependency dyndep contract rather than replace it with pool-based order.

Do not implement weighted CPU or memory tokens, multiple independent resource
acquisition, distributed locks, or a second ready queue. An action requiring
several related resources can use one explicitly named conservative class.
Unrelated classes cannot express overlapping exclusion sets in this version;
combining their names into a new class would not enforce the original limits.

Ninja's special `console` pool cannot simultaneously be combined with a custom
pool on the same edge. Reject a request that requires both guarantees. Do not
silently discard either console behaviour or contention limits.

Serialize resolved class definitions and references in generated plans and
include scheduling metadata in the existing graph/plan identity. Reject unknown
persisted variants or missing pool definitions before replay. Backends without
pool support must report the unsupported guarantee rather than ignore it. No API
may pass an unchecked public class name straight into Ninja source.

## 6. Scope, worker budgets, and state integration

The concurrency guarantee applies to one Ninja invocation. Two independent
Netsuke invocations do not share a pool. A depth-one class also does not stop an
external Cargo process from using the same directory. Document the guarantee in
human and structured inspection, not only in this RFC.

Internal workers remain separate. A class with capacity one may still launch
many compiler or test threads. Tool adapters can consume typed worker inputs,
but the pool is neither a CPU quota nor a memory bound. Operator job ceilings
and the backend's jobserver behaviour remain authoritative.

[Managed states][states] use integrity leases when cooperating invocations
mutate or consume the same managed path. A contention class can avoid
dispatching unnecessary competitors but does not replace those leases. State
operations inside an action retain its slot; they must not recursively start
Netsuke and wait for the same class. Bounded lock acquisition must not hide a
scheduling deadlock or provide an unbounded retry loop.

Resource exclusivity is not semantic ordering. Cleaning and consuming a path in
the same selected closure remains an ownership conflict even with capacity one.

## 7. Namespaces, inspection, and defaults

Use RFC 0002's provenance and duplicate rules for includes. Bundle-private
classes remain private under RFC 0003. Sharing a class across bundles requires
an explicit importer binding to a declared root class; identical local names
must not accidentally serialize unrelated bundles. Local manifest delivery must
not wait for this optional bundle-binding integration.

Inspection identifies each action's resolved class, requested and effective
capacity, source declaration, and invocation-only scope. JSON uses the existing
versioned metadata envelope. Diagnostics use bounded identifiers and source
spans; metric labels must not contain arbitrary class names or paths.

The default manifest gains no pool, mandatory annotation, or new warning.
[Maturity policies][maturity] may require a class for explicitly selected
actions, but may not infer such a requirement from a command containing the word
`cargo` or silently alter capacity.

## 8. Acceptance and compatibility

Compare unannotated graph and Ninja output against existing fixtures. Add a
small legacy-command example before any larger structured-runner canary. Test
unknown classes, capacity bounds, conflicting definitions, console conflicts,
aggregates, namespace privacy, and persisted-plan compatibility.

Use controlled child processes and a shared test journal to measure live
concurrency. Under global `-j` greater than one, assert that a capacity-one
class never overlaps, a capacity-two class never has three active edges, and an
unrelated action can proceed while that class is occupied. Check the entire
command-sequence interval rather than just process launch timestamps.

Exercise failure, cancellation, and early-stop behaviour without timing-only
sleep assertions. Property-test stable naming, declaration-order independence,
and reference resolution. A separate two-invocation test must demonstrate the
boundary: pools alone do not claim process-wide mutual exclusion.

## 9. Alternatives and outstanding decisions

Serial dependencies encode order, not reusable contention policy. A lock command
inside every recipe repeats infrastructure and hides it from scheduling. A
generalized multi-resource scheduler would duplicate Ninja and create a much
larger correctness burden. Mandatory resource budgets would undermine the
shallow end without necessarily controlling subprocesses.

Ratify the operator ceiling field, class-name lowering, console interaction, and
any rule-default semantics before implementing the public grammar. Reserve
multi-resource allocation and cross-host scheduling for separate evidence-led
proposals. Ordinary native compilation limits must not depend on those features.

## 10. Recommendation

Expose a deliberately small public contract over Ninja pools, with one optional
class per executable edge and explicit scope. Keep dependencies, internal worker
counts, and cross-invocation integrity locks separate.

[roadmap]: ../roadmap-progressive-enhancement.md#22-named-contention-without-a-second-scheduler
[inputs]: 0014-typed-task-inputs.md
[states]: 0013-managed-states-and-probes.md
[maturity]: 0017-progressive-enhancement-and-maturity-policies.md
[^1]: [Ninja manual: pools](https://ninja-build.org/manual.html#ref_pool).
