# Formal verification methods in Netsuke

## Executive summary

Netsuke is a strong candidate for selective formal verification rather than
whole-program proof. The repository is a single Rust package that compiles a
YAML manifest with MiniJinja expansion into a deterministic Ninja build graph,
then delegates execution to the Ninja subprocess.[^1][^2][^3][^4]

The highest-return verification work is concentrated in the semantic core:

1. Kani should verify bounded safety and error-handling properties in the
   Intermediate Representation (IR) pipeline, especially duplicate-output
   rejection, rule resolution, cycle detection, and command interpolation.[^5]
2. Proptest should exercise determinism and manifest-expansion invariants that
   are pure enough for generated testing but awkward to prove directly.[^2][^6]
3. Verus should remain optional and limited to small proof kernels such as
   cycle canonicalization after the Kani and Proptest suites have stabilized.
4. Stateright should remain out of scope until Netsuke gains long-lived mutable
   state, actor-style coordination, or a scheduler that is more complex than
   the current Ninja hand-off.[^1][^4]

## Current repository state

The repository currently has no formal-verification tooling. `Cargo.toml`
contains unit-testing, behavioural-testing, snapshot-testing, and CLI-testing
dependencies, but no `proptest`, `kani`, `verus`, or Stateright support.[^1]
The top-level `Makefile` exposes build, test, lint, formatting, Markdown lint,
and Mermaid validation targets only.[^2] The current `CI` workflow runs the
same conventional checks across supported toolchains and does not define a
separate proof or model-checking job.[^3]

That absence is useful context. Netsuke does not need a new verification
workspace as a first step. The strongest verification targets already live in
production modules with clear semantic boundaries, so a lightweight,
module-adjacent approach is the most proportionate fit.

## Recommended scope

### Kani for the IR core

`BuildGraph::from_manifest` is the main semantic commitment point. It resolves
rules, registers actions, rejects duplicate outputs, records default targets,
and reports cycles before the build graph is returned to the caller.[^5] A bug
in this path risks producing a wrong graph rather than a clean failure.

The first Kani harnesses should cover these properties:

- Duplicate outputs always fail with `DuplicateOutput`.
- Empty rules, multiple rules, and missing named rules return the correct
  `IrGenError` variant.
- Self-edges and bounded multi-node cycles always fail with
  `CircularDependency`.
- Acyclic bounded graphs are never rejected as cyclic.
- Missing dependencies are recorded without creating false cycles.

`src/ir/cycle.rs` is the strongest narrow proof candidate in the current
repository. `canonicalize_cycle` has a crisp normalization contract that is
small enough for Kani immediately and remains a plausible later Verus kernel if
stronger proof obligations become worthwhile.[^7]

### Kani for command interpolation

`src/ir/cmd_interpolate.rs` is another high-value target because it is compact,
load-bearing, and security-sensitive. `interpolate_command` replaces `$in` and
`$out`, avoids rewriting inside backticks, and rejects commands when backticks
are unmatched or the interpolated result fails the current `shlex` guard.[^8]

The initial Kani properties should assert that:

- Only whole-word `$in` and `$out` placeholders are rewritten.
- Identifier-containing strings such as `$input` or `$output` are not
  rewritten accidentally.
- Backtick-delimited regions are preserved.
- Odd numbers of backticks are rejected.
- Successful interpolation always returns a string that passes the current
  syntactic guard.

### Proptest for determinism and manifest semantics

Netsuke already documents deterministic graph generation as part of its public
shape, and the implementation sorts actions, edges, and default targets when
rendering the Ninja file.[^4][^6] That makes generated testing a natural fit
for determinism checks.

The first Proptest coverage should focus on:

- stable Ninja output across `HashMap` insertion orders,
- stable `default` target ordering,
- `path_key` invariance for equivalent output sets, and
- action-hash stability for field-preserving permutations.[^6]

The manifest-expansion pipeline is also a strong Proptest target. The loader
parses YAML first, expands `foreach` and `when`, deserializes into typed data,
and only then renders string fields with MiniJinja.[^9] Generated testing
should lock down these invariants:

- `foreach` preserves non-control fields,
- `when` is removed after evaluation,
- `item` and `index` bindings are injected correctly,
- static targets still honour `when`,
- key order is preserved where the implementation intends to preserve it, and
- rendering is idempotent once no Jinja syntax remains.[^9]

### Optional Verus proof kernel

Verus is not the correct first tool for the manifest or command-interpolation
layers. The code that is most suitable for a later proof is
`canonicalize_cycle`, because the function exposes a clear mathematical
contract:

- output length is preserved,
- the cycle remains closed,
- the interior node multiset is preserved, and
- the chosen start node is stable under the repository's ordering rule.[^7]

If Verus is introduced, it should prove only that narrow normalization model at
first. Production `HashMap` structures, MiniJinja values, filesystem helpers,
and subprocess orchestration should remain outside the first proof boundary.

### Stateright remains deferred

Stateright is not justified by the current architecture. Netsuke is a compiler
and orchestrator that emits a static Ninja file and delegates execution to the
Ninja subprocess.[^4][^10] There is no actor protocol, distributed state
machine, or internal concurrent scheduler to model check today.

Reconsidering Stateright would make sense only after a future daemon mode,
watch service, remote-execution coordinator, or another long-lived concurrent
subsystem exists.

## Repository integration plan

### Layout

The preferred first layout is intentionally lightweight:

```text
.
├── Cargo.toml
├── Makefile
├── scripts/
│   ├── install-kani.sh
│   ├── install-verus.sh
│   └── run-verus.sh
├── tools/
│   ├── kani/
│   │   └── VERSION
│   └── verus/
│       ├── VERSION
│       └── SHA256SUMS
├── verus/
│   ├── netsuke_proofs.rs
│   └── cycle_canonicalization.rs
└── src/
    ├── ir/
    │   └── kani.rs
    ├── manifest/
    │   └── kani.rs
    └── ninja_gen_kani.rs
```

This layout keeps Kani harnesses close to the internal helpers they exercise,
which avoids widening the public API solely for proofs. Verus can remain
outside Cargo until it proves its value.

### Makefile and local workflow

The first formal-verification commands should extend the existing `Makefile`
without disturbing the current developer workflow.[^2]

- `make kani` should run a small smoke-harness set suitable for pull requests.
- `make kani-full` should run the full Kani suite.
- `make formal-pr` should alias the fast pull-request checks.
- `make formal-nightly` should combine deeper Kani runs with any later Verus
  proofs.

That split keeps the default quality gates fast while still allowing a deeper
scheduled gate.

### Continuous integration (CI)

Formal verification should not be folded into the existing `build-test` job.
The current `CI` workflow already performs formatting, linting, tests, and
coverage, and those checks should remain intact.[^3]

The first additional job should be a dedicated `kani-smoke` job that:

- installs the pinned Kani toolchain,
- runs `make kani`, and
- caches tool downloads separately from the ordinary Rust build artefacts.

Any later Verus job should be added only after a stable proof kernel exists.

## Design decisions to settle before proof work

Three contracts should be documented before proofs become gating checks.

### Command placeholder contract

The interpolation layer currently supports `$in` and `$out`, enforces
identifier-style token boundaries, and suppresses substitution inside
backticks.[^8] The project documentation should state whether:

- those are the only supported placeholders,
- backtick suppression is the full contract or a temporary subset of shell
  command-substitution handling, and
- `shlex::split` is part of the semantic acceptance contract or only a guard
  against obviously malformed commands.

### Cycle-participation contract

`BuildEdge` records explicit inputs, explicit outputs, implicit outputs, and
order-only dependencies, but the current cycle detector walks `edge.inputs`
only.[^5][^7] Before proofs are written, the intended scope of cycle detection
should be recorded explicitly:

- explicit inputs only,
- explicit inputs plus order-only dependencies, or
- explicit, implicit, and order-only dependencies together.

### Determinism contract

The README promises a reproducible, fully static dependency graph, and the
generator already sorts its output to support deterministic emission.[^4][^6]
The formal-verification work should therefore define whether byte-for-byte
stable Ninja output and stable action identifiers are public guarantees or
current implementation details.

## Recommended delivery order

The delivery sequence should stay narrow:

1. Add Kani tooling, local scripts, and pull-request smoke targets.
2. Add Kani harnesses for IR generation, cycle handling, and command
   interpolation.
3. Add Proptest coverage for deterministic emission and manifest semantics.
4. Document the deferred Stateright decision and the narrow Verus entry point.
5. Add a small Verus proof kernel only after the earlier suites have stabilized
   and the relevant contracts are written down.

That order produces useful guarantees quickly without forcing the repository
into a proof-first shape that its current architecture does not need.

## References

[^1]: [`Cargo.toml`](../Cargo.toml) lists the current package layout and
  dependency set.
[^2]: [`Makefile`](../Makefile) defines the current local quality-gate targets.
[^3]: [`.github/workflows/ci.yml`](../.github/workflows/ci.yml) defines the
  current continuous integration workflow.
[^4]: [`README.md`](../README.md) describes Netsuke as a compiler from YAML and
  Jinja to Ninja, with deterministic graph generation and Ninja execution.
[^5]: [`src/ir/from_manifest.rs`](../src/ir/from_manifest.rs) defines
  `BuildGraph::from_manifest`, duplicate-output rejection, rule resolution, and
  cycle reporting.
[^6]: [`src/ninja_gen.rs`](../src/ninja_gen.rs) sorts actions, edges, and
  default targets to keep emitted Ninja text deterministic.
[^7]: [`src/ir/cycle.rs`](../src/ir/cycle.rs) defines cycle analysis and
  `canonicalize_cycle`.
[^8]: [`src/ir/cmd_interpolate.rs`](../src/ir/cmd_interpolate.rs) defines
  placeholder substitution and command validation.
[^9]: [`src/manifest/mod.rs`](../src/manifest/mod.rs) describes the YAML-first
  manifest pipeline and re-exports expansion and rendering helpers.
[^10]: [`src/runner/mod.rs`](../src/runner/mod.rs) generates the Ninja manifest
  and delegates execution to the Ninja subprocess.
