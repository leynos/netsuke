# Architecture decision record (ADR): Publish and verify the Ninja emission determinism contract

## Status

Accepted, with two amendments recorded on 2026-10-10 under _Amendments_.

## Date

2026-10-10

## Context and problem statement

Netsuke's design document states, without qualification, that "the generated
`build.ninja` will be byte-for-byte identical" given the same `Netsukefile` and
environment variables (`docs/netsuke-design.md` §1.2). The README advertises "a
deterministic intermediate build graph" ([`README.md`](../README.md#L172)) and
"reproducible dependency graphs" for the `graph` renderer
([`README.md`](../README.md#L178)). The formal-verification survey asks for
the guarantee to be "strengthened" by deciding "whether byte-for-byte stable
Ninja output and stable action identifiers are public guarantees or current
implementation details" (`docs/formal-verification-methods-in-netsuke.md`
§Determinism contract).

Three problems follow from that state.

**The claim is false as written.** Emission is a function of more than the
manifest and the environment. `generate`, `generate_into`, and
`generate_bundle` are all thin wrappers over `generate_into_with_shell`, which
takes a `RecipeShell`. The wrappers pass `RecipeShell::host_default()`:
`PowerShell` on Windows, `Posix` elsewhere, overridable at runtime by
`NETSUKE_WINDOWS_SHELL`, whose `Bash` route additionally depends on a
filesystem probe. PowerShell takes an `rspfile` branch that POSIX does not
(`src/ninja_gen/mod.rs:379`). Two platforms running the same version on the
same manifest produce different bytes, correctly and by design.

**Implementers cannot cite it.** `#[cfg(kani)]` replaces the IR's map type with
a bounded ordered map (`src/ir/graph_kani_map.rs`), so the Kani harnesses are
structurally incapable of observing the `HashMap` nondeterminism the roadmap
item names. `ADR-004` records the hand-off: the larger-N graph property "is
handed off to the future Proptest roadmap item `4.3.1`". A hand-off needs a
specification to hand off _to_, and there is none.

**Nothing verifies it.** The eight `tests/snapshots/ninja/*.snap` fixtures are
single-run goldens. The repository contains no test that emits twice and
compares, and none that invokes the binary twice over one manifest. The
`GraphView` insertion-order property (`src/graph_view/tests_property.rs`)
compares projected views, not emitted bytes.

Finally, four different statements travel under the word "deterministic" and
are routinely conflated: process-level reproducibility, invariance under
`HashMap` insertion order, invariance under manifest declaration permutation,
and the platform-and-shell parameter. They have different scope, different
strength, and different verification methods. Only one of them is what users
read.

## Decision Drivers

- The guarantee is already user-visible in the README and the design document.
  An unverified published guarantee is worse than an unpublished one, because
  it is relied upon.
- `ADR-004`'s deferred obligation cannot be discharged against an unstated
  property: a property test needs a specification, not a hope.
- Users' real requirement is incremental caching and reproducible artefacts,
  which the _process-level_ statement supports directly. `HashMap` iteration
  order is a mechanism, not an outcome.
- The emitter's structure is load-bearing for its verification. The two
  `sort_by_key` calls are what make emission order-independent, and they are
  also what makes a mutation patch able to demonstrate that the property is
  sensitive to the code it claims to protect.
- Cross-version byte stability is a trap, not a feature. `PARENT_SCHEMA`
  (`"netsuke-serial-v1"`) and `DYNDEP_SCHEMA` (`"netsuke-dyndep-v1"`)
  (`src/ninja_gen/dyndep.rs:68`, `:70`) exist precisely so the staging format
  can change without colliding with an older namespace.

## Requirements

### Functional requirements

- State exactly which of the four statements is a public guarantee, and state
  the parameter tuple it is a function of.
- State the preconditions a caller must meet for the guarantee to apply.
- State which entry point the guarantee covers.
- Record, in the same document, what the guarantee does **not** cover, loudly
  enough that a reader does not infer it.

### Technical requirements

- Give every planned property a numbered precondition it can cite, so that a
  generator's domain and a test's skip conditions have one authority.
- Record why Kani cannot discharge the ordering obligation, so the hand-off in
  `ADR-004` is not re-attempted the same way.
- Record the two places where the domain model is not pure, without repairing
  either: both are in use and neither is on this work's critical path.

## Options considered

### Option A: Publish no guarantee; keep determinism an implementation detail

Rejected. The README already publishes it, the design document states it more
strongly still, and users cache against it. Withdrawing the claim would be a
behavioural regression in the only sense that matters to a user, and it would
leave `ADR-004`'s deferral pointing nowhere.

### Option B: Promise byte stability across Netsuke versions

Rejected. The dyndep schema tags exist so the staging format _can_ change
(`src/ninja_gen/dyndep.rs:68`); a version-stability promise would make them
pointless, and every future change to the emitter would become a breaking
change requiring a migration note.

### Option C: Port the IR to an ordered map (`BTreeMap`) so determinism is structural

Considered and rejected, but not for the reason the ExecPlan's own constraint
would supply. Determinism is already structural: the emitter sorts both
collections. Adopting a `BTreeMap` would make the two `sort_by_key` calls dead
code, which in turn would make the committed mutation patches (`MUT-EDGESORT`,
`MUT-ACTIONSORT`) stop failing anything. Determinism would be preserved and its
evidence would evaporate. Structural determinism and mutation-demonstrated
determinism are alternatives here, not complements.

### Option D: Precompute and cache `path_key` per edge

Rejected as unnecessary at production scale. Measured at 16.2 ms release for a
10,000-edge graph, and 1.866 ms per 200-target case in a dev build. The
allocation cost is a test-budget item, not a production defect, and a cache
would add an invalidation surface to a value that is currently a pure function.

## Decision outcome / proposed direction

**Y-statement:** In the context of a manifest compiled to a Ninja build graph
and emitted as `build.ninja`, and facing the forces of incremental-build
caching, reproducible artefacts, a claim already published in the README, and a
verification hand-off from Kani that has nothing to verify against, we decided
to publish exactly one determinism guarantee — process-level reproducibility on
an identical
`(manifest, environment, platform, shell selection, Netsuke version)` tuple —
and to record the order-invariance, declaration-invariance, and non-invariance
statements as internal invariants with their own preconditions, accepting that
byte stability across Netsuke versions is explicitly not promised, that the two
domain-purity leaks remain unrepaired, and that a source-shape contract must
carry the forward-looking half of the guarantee.

**Public guarantee (G-1).** For a fixed parameter tuple

```plaintext
(manifest, environment, platform, recipe-shell selection, Netsuke version)
```

Netsuke emits byte-identical output. This is a _process-level_ statement,
observable by running the binary twice, and it applies to the complete
artefact: the main build file text, every dyndep sidecar's relative path, and
every sidecar's content.

**Internal invariants (I-1 … I-3).** These are not published as promises, but
they are the mechanism G-1 rests on and each carries its own preconditions.

- **I-1, insertion-order invariance.** Two `BuildGraph` values equal as values,
  whose maps and edge arena were populated in different sequences, emit
  identical bytes.

  This is not testable by generation alone. `HashMap`'s `RandomState` is not
  part of the Proptest seed, so two graphs built from two permutations may
  iterate identically by chance, and a case that does says nothing. `EP-M0`
  measured the consequence: iteration order is a function of the _key set_, not
  of insertion order alone, so a map with fewer than five keys frequently has
  no alternative iteration order to reach at all. The verification plan's
  answer is to assert the invariant at the extracted collection helpers
  (`ordered_edges`, `ordered_actions`) over an explicitly shuffled `Vec`, where
  perturbation is by construction, and separately to compare whole bundles only
  on a well-separated key-set pair.

- **I-2, declaration-order invariance.** Permuting target or action
  declarations within `manifest.targets` or `manifest.actions` does not change
  the emitted bytes, for manifests whose rule names are pairwise distinct.

  The precondition is load-bearing. `process_rules`
  (`src/ir/from_manifest.rs:99`) inserts into `rule_map` in declaration order,
  so a later rule of the same name replaces an earlier one and `process_targets`
  (`src/ir/from_manifest.rs:118`) resolves every target through the survivor.
  No `DuplicateRule` variant exists anywhere in `src/`, so duplicate rule names
  are last-writer-wins rather than an error. Permuting two same-named rules
  with different bodies therefore _does_ change the bytes; the unrestricted
  statement is false. Note also that moving a declaration between
  `manifest.actions` and `manifest.targets` is a permutation of the declaration
  multiset that is not order-neutral, since `process_targets` chains the two
  lists.

- **I-3, two deliberate non-invariances.** Within a single edge,
  `DisplayEdge` (`src/ninja_gen/display_edge.rs:22`) renders each path vector
  verbatim in declaration order. Emission is therefore _not_ invariant under
  permuting an edge's own output, input, implicit-dependency, or
  order-only-dependency lists — even though `path_key` is invariant over the
  same lists. Separately, error-path diagnostic selection and ordering are
  excluded from every statement above: `reject_unsupported_path_characters` and
  `reject_reserved_paths` both iterate `graph.edges()` and return on the first
  offender, so which error a multi-fault graph reports is a property of the
  arena's insertion order. Emitting different diagnostics for the same two
  faults in different orders is permitted and untested.

**Not promised (N-1 … N-4).** These are stated as loudly as G-1, because each
is an inference a reader would otherwise draw.

- **N-1, cross-version.** Byte stability is **not** promised across Netsuke
  versions. A version bump may change emitted bytes for an unchanged manifest,
  environment, platform, and shell selection. This is intended: `PARENT_SCHEMA`
  and `DYNDEP_SCHEMA` (`src/ninja_gen/dyndep.rs:68`, `:70`) incorporate an
  explicit format tag into every content-addressed identity so that a
  staging-format change yields a fresh namespace instead of a silent collision
  with sidecars written by an older Netsuke. `ninja_required_version` is
  likewise emitted conditionally, only when staging is present.
- **N-2, cross-platform.** Byte stability is **not** promised across platforms
  or across different recipe-shell selections on one platform. PowerShell and
  POSIX are different renderers: they disagree on `rspfile` emission, on
  backtick handling, and on script wrapping.
- **N-3, within-edge list order.** Covered by I-3. Two graphs differing only in
  the order of one edge's `explicit_outputs` may emit different bytes while
  both being correct.
- **N-4, error paths.** Covered by I-3.

**Preconditions (P-1 … P-5).** A property may cite these rather than restating
them, and a counter-example violating a precondition is out of contract rather
than a defect.

- **P-1, well-formed graph.** A `BuildGraph` satisfying the invariants
  `from_manifest` establishes: every edge has at least one explicit output;
  explicit output sets are pairwise disjoint across edges and internally
  duplicate-free; no output path is empty; every `action_id` exists in
  `actions`; every edge indexed under key `k` has `k` among its explicit
  outputs; and no path contains a rejected character.
- **P-2, `generate` requires a sidecar-free graph.** `generate`,
  `generate_into`, and `generate_with_shell` reject a graph for which
  `graph_requires_dyndep` is true, because they cannot materialize sidecars.
  G-1 over a staged graph is claimed for `generate_bundle` only. This
  precondition is not a caveat on the guarantee; it is a boundary between two
  entry points, and the rejection is itself tested.
- **P-3, path syntax.** No explicit output, implicit output, input, implicit
  dependency, order-only dependency, or default target contains `$`, `:`, `|`,
  or a control character (`src/ninja_gen/path_syntax.rs`), and none lies inside
  or beneath the reserved `.netsuke/serial` or `.netsuke/dyndep` namespaces
  (`src/ninja_gen/dyndep.rs:338`).
- **P-4, rule-name distinctness.** Required by I-2's declaration-permutation
  form only. It constrains the manifest, not the emitter.
- **P-5, sampling is not proof.** Every statement here is sampled by Proptest
  or executed twice by a directed test. Where an exhaustive small-N result
  exists it is the Kani harnesses of roadmap 4.2.x, and the two layers are
  complementary: Kani is exhaustive within a bound but cannot see the
  nondeterminism, and Proptest sees the nondeterminism but samples.

### Why Kani cannot discharge I-1

Under `#[cfg(kani)]`, `src/ir/graph.rs` aliases `IrHashMap` to a bounded
ordered map defined in `src/ir/graph_kani_map.rs`. The Kani build therefore has
no `HashMap` in the graph at all, and the nondeterminism I-1 names does not
exist in the model. Adding a `-Z`-free harness cannot help: the substitution is
a `cfg`, not a flag, and it is what makes the existing IR harnesses tractable
(`graph_kani_map.rs` exists so that "Kani harnesses can drive the real lowering
and cycle-detection logic without depending on `std::collections::HashMap`").
The unavoidable honest statement is that exhaustiveness and nondeterminism are
in tension here, and the project has chosen to keep the exhaustive harnesses at
one to three nodes and hand the larger-N and order-invariance work to Proptest.

### The alternative that does carry the forward-looking half

No property in this plan can catch a _future_ unordered collection. A generator
varies the fields it knows about; a map keyed on pools, or on variables, or on
anything the generator does not model, will never be perturbed by it. An
ordered-map port would have provided structural immunity, at the cost recorded
under Option C.

A source-shape contract provides the same immunity more cheaply: a test that
reads every file under `src/ninja_gen/` and fails when a `std::collections`
hash map or set is iterated (with two named membership-only exemptions, both
pre-existing `seen` sets). It is the `OBL-NOHASH` obligation, and it is the
only element of this work that constrains code not yet written.

## Goals and non-goals

- Goals:
  - One published, testable guarantee, stated with its full parameter tuple.
  - A numbered precondition list that obligations cite rather than restate.
  - An explicit record of what is not promised, so no reader infers it.
- Non-goals:
  - Repairing the two domain-purity leaks (below). Both are real; neither is
    on this work's critical path, and changing either is a behaviour change
    that `Constraint 1` forbids.
  - Cross-version or cross-platform byte stability.
  - Determinism of error-path diagnostics.
  - Any change to the shipped binary's observable behaviour.

## Known risks and limitations

- **The order-invariance obligation is only as strong as its perturbation.**
  Where the key set offers no second iteration order, a bundle-comparison case
  passes without testing anything. This is why the helper-level property is the
  primary evidence and the bundle comparison is secondary, and why the number
  of skipped cases is counted rather than swallowed.
- **Shrinking does not converge on this domain.** `EP-M0` measured a
  fifty-edge counter-example failing to minimize in thirty seconds, with four
  runs of one seed producing 39, 90, 43, and 73 edges. The mitigation is a
  compact `Debug` and a tolerance on assertion shape, not a larger iteration
  cap, which would only push the run into nextest's kill window where no
  regression seed is persisted.
- **`Action`'s identity is a serialization artefact.** `Action` derives
  `Serialize` and `ActionHasher` defines action identity as the SHA-256 of its
  canonical JSON. A change to the JSON representation of any field silently
  re-identifies every action. This is a genuine purity leak; it is recorded,
  not fixed, and it is why the interning obligation claims only the direction
  sampling can support (`a == b` implies equal hashes) and never
  collision-freedom.
- **A verification cfg selects the domain's collection type.** `IrHashMap`'s
  definition is chosen by `#[cfg(kani)]` (`src/ir/graph.rs:22`–`:28`). The
  domain model does not fully own its own representation. Recorded, not fixed.
- **`default_targets` may repeat.** `process_defaults`
  (`src/ir/from_manifest.rs:186`) extends without deduplication and `Vec::sort`
  does not deduplicate, so a manifest naming one default twice emits that
  operand twice. The behaviour is deliberate — it is the manifest's own
  ordering, sorted — but it surprises readers, and the obligation pins it.

## Amendments

Two facts changed after the obligations were drafted. Both are corrections to
prior claims rather than changes of decision, and both are recorded here
because the ExecPlan's obligations cite this document.

### Amendment 1: the 200-output bound is a cost budget, not a correctness bound

The ExecPlan's `Constraint 10` justifies its 200-explicit-output cap with the
claim that "`insert_edge_for_outputs` stores an edge once per output, so the
emitter's sort sees `targets.len()`, not the edge count". That was true when
the plan was drafted on 2026-09-09. It stopped being true on 2026-09-18 when
`2c030fd1` ("Store multi-output build edges once", #652/#714) made
`insert_edge_for_outputs` call `graph.insert_edge` directly, which calls
`insert_canonical_edge` and pushes _one_ arena entry. `graph.edges()` now
iterates the arena, so the emitter's sort sees the edge count.

The bound survives as a cost budget rather than a correctness bound, and the
measured per-case cost table it was derived from remains valid. What does not
survive is the stated mechanism, which must not be quoted as a fact about the
emitter.

### Amendment 2: an output-less edge is not dropped, and may not be constructible

`Constraint 10`'s sibling preconditions, and the `OBL-NOLOSS` obligation,
describe "output-less edges dropped while their rule block is still emitted".
Re-reading the current code, `insert_canonical_edge` pushes the edge
unconditionally and `render_edges` renders every arena entry, with no `seen`
set. An output-less edge therefore produces a `build : action` line with an
empty left-hand side; if real Ninja rejects that, it rejects it as invalid
syntax, not as a silent drop.

Whether the case is reachable at all is unresolved. `Target::name` is a
`StringOrList` whose `Empty` variant maps to an empty vector
(`src/ast/string_or_list.rs:52`), so a typed manifest could in principle carry
one; `get_target_display_name` (`src/ir/from_manifest_support.rs:363`)
explicitly tolerates the empty case, returning an empty string "when the target
declares no explicit outputs". Both are construction-side rather than
lowering-side, so it is not established that a manifest reaching the loader can
produce one. This ADR does not settle it. The obligation stands, its domain
note must not assert the silent drop as fact, and establishing the reachability
is `EP-M4`'s business.

## Architectural Rationale

The decision keeps the repository's architecture honest about where its
guarantees live. `BuildGraph` is the domain model; `src/ninja_gen/` is an
outbound adapter rendering it into one backend's text, writing to a `W: Write`
so that the adapter does not own the sink. G-1 is a statement about the
composition, which is why the process-level test spans both, while the
order-invariance and default-ordering invariants are stated and tested at the
adapter's own entry points.

Recording N-1 and N-2 also aligns the documents with each other. The design
document's unqualified byte-equality claim is false across platforms as
written, and the README's two determinism-flavoured lines concern the
intermediate graph and the `graph` renderer respectively, not `build.ninja`.
Reconciling those statements is `EP-M8`'s work; this record is the authority it
reconciles to.

## Implementation references

- IR collections and the Kani substitution:
  [`src/ir/graph.rs`](../src/ir/graph.rs) and
  [`src/ir/graph_kani_map.rs`](../src/ir/graph_kani_map.rs)
- Lowering, rule resolution, and default targets:
  [`src/ir/from_manifest.rs`](../src/ir/from_manifest.rs) and
  [`src/ir/from_manifest_support.rs`](../src/ir/from_manifest_support.rs)
- Emission entry points:
  [`src/ninja_gen/mod.rs`](../src/ninja_gen/mod.rs),
  [`src/ninja_gen/explicit_shell.rs`](../src/ninja_gen/explicit_shell.rs), and
  [`src/ninja_gen/dyndep.rs`](../src/ninja_gen/dyndep.rs)
- Within-edge rendering, exempt from I-1:
  [`src/ninja_gen/display_edge.rs`](../src/ninja_gen/display_edge.rs)
- Path and namespace preconditions:
  [`src/ninja_gen/path_syntax.rs`](../src/ninja_gen/path_syntax.rs)
- The deferred obligation this discharges:
  [ADR-004](adr-004-bound-kani-ir-harnesses-to-small-n.md)
- Prior art for order-invariance testing:
  [`src/graph_view/tests_property.rs`](../src/graph_view/tests_property.rs)
- ExecPlan:
  [4.3.1 Proptest coverage for deterministic Ninja emission](execplans/4-3-1-proptests-for-deterministic-ninja-emission.md)

## Related decisions

- [ADR-004: Bound Kani IR harnesses to small N][adr-004]
- [ADR-011: Use Ninja dyndep for serial dependency ordering][adr-011]
- [ADR-012: Bound dyndep sidecar retention][adr-012]
- [ADR-019: Structured command shell selection][adr-019]

[adr-004]: adr-004-bound-kani-ir-harnesses-to-small-n.md
[adr-011]: adr-011-use-ninja-dyndep-for-serial-dependency-ordering.md
[adr-012]: adr-012-bound-dyndep-sidecar-retention.md
[adr-019]: adr-019-structured-command-shell-selection.md
