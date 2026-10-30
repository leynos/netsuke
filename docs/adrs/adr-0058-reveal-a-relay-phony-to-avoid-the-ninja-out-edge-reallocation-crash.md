# Architectural decision record (ADR) 0058: Reveal a relay phony to avoid the Ninja out-edge reallocation crash

## Status

Accepted.

## Date

2026-10-23

## Context and problem statement

[ADR-0011](adr-0011-use-ninja-dyndep-for-serial-dependency-ordering.md) lowers a
`dependency_order: serial` list with two or more dependencies into staged
phony gates, one content-addressed Ninja dyndep sidecar per dependency. As
originally implemented, each sidecar revealed the **real** dependency:

```ninja
ninja_dyndep_version = 1
build <gate>: dyndep | <real-dep>
```

That shape crashes Ninja. In `LoadDyndeps`, loading a sidecar calls
`UpdateEdge`, which calls `node->AddOutEdge(edge)` on the revealed node.
`AddOutEdge` appends to `Node::out_edges_`, a `std::vector`. Meanwhile
`Plan::NodeFinished` iterates that same vector. When a later sidecar reveals a
node whose out-edge vector is being iterated and has no spare capacity, the
append reallocates the vector and the iteration reads freed memory.

Reachability was measured rather than reasoned about, on Ninja 1.11.1 and on
the debug build of 1.11.1. The measurement used the *emitted* bundle for the
crashing shape, rewritten into the pre-relay form, not a hand-built model of
it; the same bundle as the emitter now produces is accepted. Which targets
Ninja is asked about is itself a variable, and the deciding question is whether
the request reaches the staged gate at all: the pre-relay bundle dies when the
invocation names the graph's full explicit-output set, and dies when it names
the staged gate on its own, while the requests that never reach the gate — an
output lying outside it, such as one of the ordinary outputs — exit 0. Three
conditions are needed together:

1. a node revealed by a later sidecar, not the first one;
2. an **order-only** edge naming that node, loaded earlier in the file — an
   ordinary edge to the same node does not trigger it;
3. exactly the consumer count that leaves the out-edge vector at full capacity,
   so the next append reallocates instead of fitting. That count is not a
   threshold: with the order-only edge present, adding one extra ordinary
   consumer of the revealed node segfaults, two do not, three do again, and
   five and eight do not.

The upstream defect is
[ninja-build/ninja#2662](https://github.com/ninja-build/ninja/issues/2662). A
fix is proposed but unmerged at the time of this record, so Netsuke cannot wait
for it.

Netsuke's exposure is not hypothetical: the serial-lowering shape that produces
condition 1 is exactly the shape this repository emits, and a user graph that
happens to supply conditions 2 and 3 turns a correct manifest into a segfault.

## Decision

Every stage's sidecar reveals a **relay phony** instead of the real dependency.
The relay is a phony build statement that the sidecar's revealed edge names and
that carries the real dependency:

```ninja
build <relay>: phony | <real-dep>
build <sidecar>: phony [<previous gate>]
build <gate>: phony || <sidecar>
  dyndep = <sidecar>
```

and the sidecar body becomes:

```ninja
ninja_dyndep_version = 1
build <gate>: dyndep | <relay>
```

The relay is created for the purpose and is named by no user edge. Loading a
sidecar therefore appends to a node's out-edge vector that no
`Plan::NodeFinished` iteration can be traversing, and the reallocation hazard
disappears regardless of the consumer count.

Staging cost rises from `1 + 2N` to `1 + 3N` statements per serial edge with
`N` implicit dependencies: relay, sidecar-producing phony, and gate. Ordered
semantics, the Ninja 1.10 floor, the reserved `.netsuke/serial` and
`.netsuke/dyndep` namespaces, content addressing, and bundle ownership are all
unchanged.

## Rationale

- **It removes the precondition, not the symptom.** The crash needs the
  revealed node to be one a live iterator is walking. Revealing a node that has
  no other consumer removes that property structurally, rather than tuning
  consumer counts or relying on vector capacity.
- **One statement buys the immunity.** The relay is a single extra phony per
  dependency. A workaround that reordered user edges or suppressed order-only
  dependencies would change user-visible semantics; a pool or a second
  scheduler would break ADR-0011's single-invocation guarantee.
- **It is invisible to users.** Relay paths live beneath the reserved
  `.netsuke/serial` namespace alongside gates, so no user graph can collide
  with them and no manifest author has to know they exist.
- **The alternative was fragile.** Accepting the defect and documenting an
  affected-Ninja release list would leave every user's build exposed to an
  environment-dependent segfault, and the affected set is not knowable in
  advance — the crash depends on diagram shape, not on a version boundary.

## Consequences

- Emitted output changes for every staged serial bundle. Determinism is
  unaffected: relays are named from the same stage index and parent identity as
  gates, so they are as content-stable as what they replace.
- The sidecar body no longer names the real dependency, so a reader cannot
  recover the ordered dependency list from the sidecars alone. It can be
  recovered from the relay build lines, and the property tests resolve it that
  way.
- Tests that assert "the sidecar reveals the declared dependency" must resolve
  through the relay. `src/ninja_gen/property_tests/mod.rs` and
  `src/ninja_gen/property_tests/dependency_only.rs` do so through shared
  helpers.
- `src/ninja_gen/dyndep/tests` asserts structurally that every sidecar names a
  relay and that no sidecar names a real dependency.
  `src/ninja_gen/determinism/property_tests/ninja_oracle/relay_regression.rs`
  pins the measured crashing shape and requires a real Ninja binary to accept
  the emitted bundle.
- When upstream merges a fix, this decision is revisited rather than assumed
  permanent. Removing the relay is a deliberate optional simplification, not a
  cleanup, and must delete the linked tests and amend this record.

## Alternatives considered

### Accept the crash and document affected Ninja versions

Rejected. The trigger depends on graph shape and vector capacity, not on a
version boundary, so no version list would be sound. A user with an affected
manifest and a correct manifest differ only in graph shape.

### Reveal the real dependency but avoid order-only edges to revealed nodes

Rejected. Order-only dependencies are a declared user relationship
(`order_only_deps`) that the emitter has no authority to drop or reorder.
Suppressing them would silently change scheduling semantics.

### Reveal a gate rather than a dedicated relay

Rejected. Gates are already targets of the preceding stage's sidecar-producing
phony and are named by the aggregate edge, so they are exactly the
already-consumed nodes the crash requires. A node with no other consumer must
be created for the purpose.

### Pin Ninja to a fixed build until upstream lands a fix

Rejected. Ninja is a user-supplied external tool; Netsuke does not vendor or
pin it, and pinning would make the generated bundle unusable with the Ninja on
a user's `PATH`.

## Implementation references

- Staged lowering:
  [`src/ninja_gen/dyndep/mod.rs`](../../src/ninja_gen/dyndep/mod.rs)
- Relay naming and sidecar rendering:
  [`src/ninja_gen/dyndep/staging.rs`](../../src/ninja_gen/dyndep/staging.rs)
- Structural assertions:
  [`src/ninja_gen/dyndep/tests/mod.rs`](../../src/ninja_gen/dyndep/tests/mod.rs)
- End-to-end regression:
  [`src/ninja_gen/determinism/property_tests/ninja_oracle/relay_regression.rs`](../../src/ninja_gen/determinism/property_tests/ninja_oracle/relay_regression.rs)
- Developer guidance:
  [developers' guide](../developers-guide.md#serial-dependency-bundles)
- User contract:
  [user's guide](../users-guide.md#run-direct-dependencies-serially)
- ExecPlan:
  [4.3.1. Add Proptest coverage for deterministic Ninja emission](../execplans/4-3-1-proptests-for-deterministic-ninja-emission.md)

## References

- [ADR-0011: Use Ninja dyndep for serial dependency ordering](adr-0011-use-ninja-dyndep-for-serial-dependency-ordering.md)
- [ADR-0030: Publish and verify the Ninja emission determinism contract](adr-0030-ninja-emission-determinism-contract.md)
- [ninja-build/ninja#2662](https://github.com/ninja-build/ninja/issues/2662)
