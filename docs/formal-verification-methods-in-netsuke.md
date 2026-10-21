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

This section describes the tree as it stands on 2026-10-13. The rest of this
document is a forward-looking plan, so its tenses read as design intent rather
than as a description of what exists; this section is the one kept current.

The plan below has largely been implemented. `Cargo.toml` carries `proptest`
for generated testing, a `[package.metadata.kani.flags]` table, and a declared
`cfg(kani)` expectation in `unexpected_cfgs`.[^1] Verus is pinned under
`tools/verus/` and invocable through `make verus`, but remains outside the
ordinary dependency set.

The `Makefile` now exposes the formal-verification surface the "Repository
integration plan" below describes: `install-kani`, `kani-check`, `kani-full`,
`kani-ir`, `test-kani-mutations`, `test-kani-scope-wrapper`, `install-verus`,
`verus`, and `formal-pr`, alongside the build, test, lint, formatting, Markdown
lint, and Mermaid validation targets.[^2]

The `CI` workflow does define a separate proof job. `kani-smoke` is a dedicated
required job covering every harness on pushes to `main`, nightly schedules, and
manual dispatches, and on pull requests it runs only when a proof input changes
(see
[ADR-0039](adrs/adr-0039-run-the-kani-proofs-on-a-pull-request-only-when-their-inputs-change.md)).[^3]
The 15 harnesses live in three verification modules:
`src/ir/cycle/verification.rs` (9), `src/ir/from_manifest/verification.rs` (4),
and `src/ir/cmd_interpolate/verification.rs` (2). `src/ir/graph/kani_map.rs` is
`cfg(kani)` support code those harnesses use, not a harness location.

That earlier absence still shapes the design. Netsuke did not need a new
verification workspace as a first step: the strongest verification targets
already lived in production modules with clear semantic boundaries, so the
lightweight, module-adjacent approach described below is what the repository
adopted — harnesses colocated beside the helpers they exercise, and the Verus
kernel kept outside ordinary Cargo.

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
- Self-edges and bounded multi-node cycles (up to 10 nodes with depth limit
  of 20 edges) always fail with `CircularDependency`.
- Acyclic bounded graphs (same size limits) are never rejected as cyclic.
- Missing dependencies are recorded without creating false cycles.

`src/ir/cycle/mod.rs` is the strongest narrow proof candidate in the current
repository. `canonicalize_cycle` has a crisp normalization contract that is
small enough for Kani immediately and remains a plausible Verus kernel if
stronger proof obligations become worthwhile.[^7] It is not yet one: the two
kernels the "Optional Verus proof kernel" section below accepts are the
canonical value key and equality relation, which is committed, and this cycle
model, which is not.

### Kani for command interpolation

`src/ir/cmd_interpolate/mod.rs` is another high-value target because it is
compact, load-bearing, and security-sensitive. It recognizes only the internal
`INS_TOKEN` and `OUTS_TOKEN` tokens emitted by manifest rendering, identically
in both recipe kinds; `$in`, `$out`, `$ins`, and `$outs` are all literal shell
variables. On POSIX-compatible routes both recipe kinds reject markers inside
backticks; `command:` text additionally rejects unmatched backticks and a
substituted result that fails the current `shlex` guard, while scripts are
exempt from those two checks. PowerShell treats backticks as native escapes
rather than protected regions.[^8]

Kani proves two allocation-free kernels. An eight-character symbolic window
with a symbolic offset proves that literal `$in` and `$out` prefixes never
select a Netsuke marker. A 32-character symbolic array with a symbolic position
drives both real marker constants through `find_substitution`, proving exact,
boundary-independent matching, including truncated and near-miss candidates.

The production scanner exceeded the five-minute, 8 GiB Kani cap at six and
eight characters, and the guard necessarily drives that scanner. Adversarial
Proptest properties therefore cover templates of up to 256 characters with at
most eight placeholders. They independently specify POSIX scanner behaviour,
verify protected-placeholder and odd-backtick rejection, and verify that the
real `shlex` guard evaluates the substituted command.

### Proptest for determinism and manifest semantics

Netsuke already documents deterministic graph generation as part of its public
shape, and the implementation sorts actions, edges, and default targets when
rendering the Ninja file.[^4][^6] That makes generated testing a natural fit
for determinism checks.

The first Proptest coverage should focus on (generated graphs bounded to 50
actions and 100 edges):

- stable Ninja output across `HashMap` insertion orders,
- stable `default` target ordering,
- `path_key` invariance for equivalent output sets, and
- action-hash stability for field-preserving permutations.[^6]

All four now exist. The determinism suites live in
`src/ninja_gen/determinism/property_tests/`, one module per obligation in
[ADR-030](adr-030-ninja-emission-determinism-contract.md), over a shared
generator bounded at 50 actions and 100 edges; a real `ninja` binary provides
the oracle for the emitted bundle.

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

Verus is optional in phase 1 and remains proof-kernel-only. A proof kernel is a
small proof-specific model for one mathematical contract; it is not a
production subsystem, a replacement implementation, or a normal developer
workflow gate. Netsuke should not add Verus installer scripts, Verus Make
targets, Verus Continuous Integration (CI), or Verus files under ordinary Cargo
as part of the phase-1 boundary.

Verus is not the correct first tool for the manifest or command-interpolation
layers. Two kernels are accepted, and both are narrow normalization or
algorithm models rather than production subsystems.

The first is a cycle canonicalization model related to `src/ir/cycle/mod.rs`.
That model is appropriate because `canonicalize_cycle` exposes a clear
mathematical contract:

- output length is preserved,
- the cycle remains closed,
- the interior node multiset is preserved, and
- the chosen start node is stable under the repository's ordering rule.[^7]

The second is the algorithm behind the canonical value key and equality
relation, proved in `verus/canonical_value/main.rs`. It establishes that
first-index deduplication retains exactly the least occurrences of each key,
against an independent quadratic specification stated over the input rather
than derived from any membership structure. The relation's own reflexivity,
symmetry, and transitivity are *not* among its results: they are properties of
byte-identity over a canonical form, and they are checked by the property
battery in `src/stdlib/canonical_value/tests/properties.rs`, not proved here.

Production `HashMap` structures, MiniJinja values, filesystem helpers, and
subprocess orchestration remain outside both proof boundaries. The second
kernel's keys are an abstract sort standing in for the runtime's byte strings,
so the template-to-key conversion, the runtime's own membership structure, and
the production adapter are each outside what it proves, and the kernel carries
no `assume`, `admit`, external body, or axiom beyond `vstd`'s own. The runtime
correspondence is carried by the independent quadratic oracle in
`src/stdlib/canonical_value/tests/oracle.rs`, not by Verus.

Verus remains outside pull-request gates, `make test`, `make lint`,
`make check-fmt`, `make all`, `make kani-check`, `make kani-full`, and
`make formal-pr`.

### Stateright remains deferred

Stateright is not justified by the current architecture. Netsuke is a compiler
and orchestrator that emits a static Ninja file and delegates execution to the
Ninja subprocess.[^4][^10] There is no actor protocol, distributed state
machine, or internal concurrent scheduler to model check today.

Stateright is therefore deferred for phase 1. The project should not add a
Stateright dependency, model directory, Make target, CI job, or documentation
that implies Stateright is part of the active verification workflow.
Reconsidering Stateright would make sense only after an accepted design
introduces a stateful concurrent subsystem: for example a daemon mode, watch
service, remote-execution coordinator, actor protocol, or internal scheduler
with long-lived mutable control-plane state. Batch-oriented manifest
compilation and static Ninja emission are not enough to reopen that decision.

## Repository integration plan

### Layout

The preferred first layout is intentionally lightweight:

```text
.
├── Cargo.toml
├── Makefile
├── tools/
│   ├── kani/
│   │   └── VERSION        # Update when new stable releases ship
│   └── verus/
│       ├── VERSION        # Update when new stable releases ship
│       └── SHA256SUMS     # Update alongside VERSION
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

The diagram is the shape this plan proposed, not an inventory of the tree as it
stands; parts of it were never adopted. The `tools/` entries are real, and
`tools/verus/VERSION` and `tools/verus/SHA256SUMS` are the pins that
`make install-verus` and `make verus` read. The `verus/` filenames and the three
`src/` harness modules are illustrative: the repository instead colocated its
Kani harnesses beside the helpers they exercise, and its one Verus kernel is
`verus/canonical_value/main.rs`. The `## Current repository state` section
above records the actual layout.

Kani and Verus installation and execution should be delegated to the
`rust-prover-tools` CLI rather than repository-local shell scripts. Netsuke
pins the prover CLI in the Makefile and invokes it through `uv tool run`, so
the verifier workflows have one maintained implementation while the Rust
package remains free of Python runtime dependencies.

The delegated Make targets should emit concise maintainer diagnostics to
standard error before invoking `rust-prover-tools`: the pinned prover-tool
source, target name, redacted command shape, relevant version pin, and failure
exit status.

#### Tool version pinning and updates

The `tools/*/VERSION` files define the exact tool versions used in local
development and continuous integration. These pins serve several purposes:

- They ensure reproducible verification results across developer machines.
- They prevent silent drift between local proofs and the CI verification
  environment.
- They allow deliberate testing of tool updates in isolation before promotion.

**Update policy:**

- Update `VERSION` files when new stable releases of Kani or Verus ship, after
  verifying that all existing harnesses and proofs remain valid under the new
  version.
- Update `SHA256SUMS` alongside `VERSION` to match the published release
  artefacts.
- Document any version-specific workarounds or compatibility notes in commit
  messages when updating tool pins.
- If CI and local development ever use different tool versions, treat that as a
  bug requiring immediate correction.

### Makefile and local workflow

The first formal-verification commands should extend the existing `Makefile`
without disturbing the current developer workflow.[^2]

- `make kani-check` should run the fast pinned-version drift guard suitable
  for pull requests.
- The bounded Kani harness suite now also runs on pull requests through
  `make kani-ir`.
- `make kani-full` should run the full Kani suite.
- `make install-kani` should install the pinned Kani version through
  `rust-prover-tools`.
- `make install-verus` and `make verus` should delegate optional Verus
  installation and proof execution to `rust-prover-tools`.
- `make formal-pr` should alias the fast pull-request checks.
- `make formal-nightly` should combine deeper Kani runs with any later Verus
  proofs.

That split keeps the default quality gates fast while still allowing a deeper
scheduled gate.

### Continuous integration (CI)

Formal verification should not be folded into the existing `build-test` job.
The current `CI` workflow already performs formatting, linting, tests, and
coverage, and those checks should remain intact.[^3]

The `kani-smoke` job is a dedicated, required job that runs on every trigger.
It verifies every harness on each push to `main`, nightly, and on a manual
dispatch; on a pull request it runs them only when the pull request changes a
proof input (see
[ADR-0039](adrs/adr-0039-run-the-kani-proofs-on-a-pull-request-only-when-their-inputs-change.md)).
When the proofs run, it:

- installs the pinned Kani front-end and release bundle from checksummed
  archives, then checks the reported version against `tools/kani/VERSION`,
- runs the bounded harness suite through `make kani-ir` (15 harnesses across
  the manifest, cycle, and command-interpolation verification modules), then
  `make install-build-tools` and the mutation patch compile gate through
  `make test-kani-mutations`,
- caches tool downloads separately from the ordinary Rust build artefacts, and
- is bounded by a 30-minute job timeout (`timeout-minutes: 30`).

The dispatch trigger exists for the cache: a dispatch cannot publish a
generation, because the save gates on a push to `main`, so what it measures is
a warm restore. The compile gate is the reason the ceiling moved from 20
minutes, since it builds the whole dependency graph through the Kani frontend
into a `CARGO_TARGET_DIR` no cache restores.

Any later Verus job should be added only after a stable proof kernel exists.

## Design decisions to settle before proof work

Three contracts should be documented before proofs become gating checks.

### Command placeholder contract

The interpolation layer recognizes only the internal `INS_TOKEN` and
`OUTS_TOKEN` tokens emitted by manifest rendering, identically in both recipe
kinds. `$in`, `$out`, `$ins`, and `$outs` are all literal shell variables,
following
[ADR-0034](adrs/adr-0034-preserve-script-in-and-out-as-shell-variables.md). On
POSIX-compatible routes, markers inside backticks are rejected in both recipe
kinds. The two further checks — unmatched backticks and a substituted result
that fails the current `shlex` guard — run only on `command:` text; scripts may
legitimately contain heredocs and other syntax `shlex` cannot model.[^8]
PowerShell treats a backtick as an escape.

**Settled.** Roadmap item 4.4.1 documents this contract for users in the
[README](../README.md#security-and-command-interpolation) and all six
translated editions under *Security and command interpolation*. The decisions
are recorded in [ADR-0027](adrs/adr-0027-command-placeholder-contract.md). The
three questions this section raised are now answered:

- The supported placeholder set is `{{ ins }}` and `{{ outs }}`, and nothing
  else, identically in both recipe kinds. Every dollar-prefixed form is a shell
  variable.
  [ADR-0034](adrs/adr-0034-preserve-script-in-and-out-as-shell-variables.md)
  settled this by removing the former `script:`-only lowering of `$in` and
  `$out`; ADR-0027 states the resulting contract.
- Backtick handling is two mechanisms at two strengths. Rejecting a marker
  inside a backtick or `$( … )` region is a promised invariant. Rejecting a
  `command:` whose substituted text contains an odd backtick count is a
  conservative whole-string parity check that is *not* promised and may widen
  without a breaking change. Netsuke leaves author-written backticks untouched.
  On POSIX routes, active backticks can trigger shell command substitution;
  backticks inside single quotes remain literal.
- `shlex::split` is part of the acceptance contract — its rejection is stable
  and localized — but **not** a stability commitment about the precise accepted
  set, because `Cargo.toml` carries a caret requirement rather than a version
  pin.

### Cycle-participation contract

`BuildEdge` records explicit inputs, implicit dependencies, explicit outputs,
implicit outputs, and order-only dependencies.[^5][^7] Cycle detection walks
explicit inputs and implicit dependencies: in manifest terms, `sources` and
`deps` participate. Order-only dependencies do not participate, because they
only enforce build ordering and do not affect rebuild freshness. The user guide
documents the same contract in its target dependency-field section.

### Determinism contract

The README's [What works today](../README.md#what-works-today) section
publishes the determinism guarantee, and the generator already sorts its output
to support deterministic emission.[^4][^6] The formal-verification work has
since strengthened that contract by deciding which statements are public
guarantees and which are implementation details, and by stating the parameter
tuple each is a function of. The result is
[ADR-030](adr-030-ninja-emission-determinism-contract.md), which the README now
cites in place of a bare reproducibility claim. The earlier form of this
paragraph cited "README line 17" and a phrase, "a reproducible, fully static
dependency graph", that no longer appears in the README; both were stale and
are corrected here rather than reproduced.

## Recommended delivery order

The delivery sequence should stay narrow:

1. Add Kani tooling, delegated prover-tool installation, and pull-request
   smoke targets.
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
[^5]: [`src/ir/from_manifest/mod.rs`](../src/ir/from_manifest/mod.rs) defines
  `BuildGraph::from_manifest`, duplicate-output rejection, rule resolution, and
  cycle reporting.
[^6]: [`src/ninja_gen/mod.rs`](../src/ninja_gen/mod.rs) sorts actions, edges,
      and
  default targets to keep emitted Ninja text deterministic.
[^7]: [`src/ir/cycle/mod.rs`](../src/ir/cycle/mod.rs) defines cycle analysis and
  `canonicalize_cycle`.
[^8]: [`src/ir/cmd_interpolate/mod.rs`](../src/ir/cmd_interpolate/mod.rs)
  defines placeholder substitution and command validation.
[^9]: [`src/manifest/mod.rs`](../src/manifest/mod.rs) describes the YAML-first
  manifest pipeline and re-exports expansion and rendering helpers.
[^10]: [`src/runner/mod.rs`](../src/runner/mod.rs) generates the Ninja manifest
  and delegates execution to the Ninja subprocess.
