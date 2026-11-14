# Evaluate a minimal Verus proof kernel for cycle canonicalization (roadmap 4.4.3)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: DRAFT

Revision 1, drafted on 2026-11-15 against `origin/main` at
`5c707ef339254a60d58f191c563bfe8597be9deb`. Implementation must not begin until
the user explicitly approves this plan.

Branch: `4-4-3-verus-proof-kernel-for-cycle-canonicalization`

Roadmap item: `4.4.3` in `docs/roadmap.md`.

## Purpose / big picture

Netsuke compiles a YAML manifest (a `Netsukefile`) into a Ninja build file.
Before writing any Ninja text it lowers the manifest into an intermediate
representation (IR), a graph of build edges, and rejects the graph if it
contains a dependency cycle. When it finds one, the error names the cycle as a
list of paths, for example
`Circular dependency detected: ["headers.stamp", "tool", "headers.stamp"]`.

A cycle has no natural first node: `a -> b -> c -> a` and `b -> c -> a -> b`
are the same cycle. Netsuke therefore *canonicalizes* every reported cycle so
the message is deterministic: it rotates the cycle so its smallest node comes
first, then repeats that node at the end to close the cycle again. The code is
`canonicalize_cycle_by` in `src/ir/cycle/support.rs`, a 20-line generic kernel
built from three helpers (`find_rotation_start_by`, `rotate_cycle_by`, and
`rotate_index`), plus a one-line path wrapper, `canonicalize_cycle`.

Today that kernel is checked in three ways. Kani, a bounded model checker,
proves it correct for cycles of two, three, and four *distinct* `u8` node
identifiers (roadmap 4.2.2). Proptest, a property-testing library, samples
cycles of up to ten single-letter path names. Named unit tests pin a handful of
examples. Nothing establishes the contract for every cycle length, nothing
checks behaviour when two nodes compare equal, and nothing states which
properties the canonical form must have in terms independent of the algorithm.

Verus is a deductive verifier for Rust: it proves that code satisfies stated
specifications for *all* inputs, using the Z3 solver, without running the code.
Netsuke already pins Verus under `tools/verus/` and proves one kernel with it,
`verus/canonical_value/main.rs`. The formal-verification design names cycle
canonicalization as the other accepted Verus kernel, but none exists.

This item *evaluates* that kernel. The deliverable is a decision backed by
evidence, not a guaranteed proof. Work proceeds in this order:

1. Write an independent mathematical specification of a canonical cycle.
2. Write a proof-specific mirror of the production kernel over `u64` node
   identifiers, outside Cargo, under `verus/cycle_canonicalization/main.rs`.
3. Prove, for every cycle length, that the mirror meets the specification and
   that the specification has the properties the design promises.
4. Compare the proof's scope and cost against the Kani harnesses it parallels.
5. Accept the kernel only if it is narrower and cheaper than that Kani
   equivalent, as roadmap 4.4.3 requires. Otherwise reject it, with evidence.

If accepted, a maintainer can observe success by running:

```bash
make install-verus
make verus
```

and seeing both kernels verified with zero errors, for example
`verification results:: N verified, 0 errors` once per proof file. `make test`
also gains ordinary-gate checks that tie the production kernel to the same
specification at large cycle lengths, keep the Verus files free of trusted
shortcuts, and flag any production edit that would make the mirror stale.

User-visible behaviour does not change. Cycle diagnostics, their order, their
localization, and their JSON form stay exactly as they are; this work proves
and documents the existing rule. The one documentation correction users might
notice is that the design document's "lexicographically smallest node" is made
precise: the order is camino's `Utf8Path` ordering, which compares path
components rather than raw bytes, and ties go to the first occurrence.

This closes roadmap item 4.4.3 and the open question left by
`docs/formal-verification-methods-in-netsuke.md` §"Optional Verus proof
kernel", which accepts a cycle-canonicalization kernel in principle while the
tree contains none.

## Context and orientation

This section orients a reader who has only this file and the working tree.

### The production kernel

`src/ir/cycle/support.rs` (163 lines) owns canonicalization. The relevant
items, quoted from `origin/main` at `5c707ef33`, are:

```rust
pub(super) fn find_rotation_start_by<T>(
    cycle: &[T],
    len: usize,
    compare: fn(&T, &T) -> Ordering,
) -> usize {
    let mut start = 0;
    let mut index = 1;
    while index < len {
        if let (Some(candidate), Some(current)) = (cycle.get(index), cycle.get(start))
            && compare(candidate, current) == Ordering::Less
        {
            start = index;
        }
        index += 1;
    }
    start
}

pub(super) fn rotate_cycle_by<T: Clone>(cycle: &[T], start: usize, len: usize) -> Vec<T> {
    let mut canonical = Vec::with_capacity(len + 1);
    let mut offset = 0;
    while offset < len {
        if let Some(node) = cycle.get(rotate_index(start, offset, len)) {
            canonical.push(node.clone());
        }
        offset += 1;
    }
    if let Some(first) = canonical.first().cloned() {
        canonical.push(first);
    }
    canonical
}

pub(super) fn canonicalize_cycle_by<T: Clone>(
    mut cycle: Vec<T>,
    compare: fn(&T, &T) -> Ordering,
) -> Vec<T> {
    debug_assert!(cycle.len() >= 2, "cycle detection should yield at least two nodes",);
    if cycle.len() < 2 {
        return cycle;
    }
    let len = cycle.len() - 1;
    let start = find_rotation_start_by(&cycle, len, compare);
    cycle.pop();
    rotate_cycle_by(&cycle, start, len)
}

pub(super) fn canonicalize_cycle(cycle: Vec<Utf8PathBuf>) -> Vec<Utf8PathBuf> {
    canonicalize_cycle_by(cycle, compare_cycle_paths)
}

pub(super) const fn rotate_index(start: usize, offset: usize, len: usize) -> usize {
    let index = start + offset;
    if index >= len { index - len } else { index }
}
```

The module also defines `compare_cycle_paths`, which calls `path_cmp`. Outside
`cfg(kani)`, `path_cmp` is `left.cmp(right)` on `Utf8Path`. Camino 1.2.6 (the
version in `Cargo.lock`) implements `Ord` for `Utf8Path` as
`self.components().cmp(other.components())`. The order is therefore
component-wise, not byte-wise, and is consistent with camino's `Eq`.

The cycle representation is *closed*. The detector (`cycle_from_stack` in
`src/ir/cycle/detector.rs`, lines 280-291) pushes the depth-first-search stack
slice from the revisited node onwards, then the revisited node again. The input
to canonicalization is therefore `[n0, n1, ..., nk, n0]`. The kernel treats
everything except the last element as the *interior*. It never checks that the
first and last elements are equal; it simply drops the last element and
re-closes the rotated interior with its own first element.

Four behaviours matter to this plan and none is documented today:

- **Tie-breaking.** `find_rotation_start_by` replaces its candidate only on a
  strict `Less`, so among equal minima the first occurrence wins.
- **Short input.** A `debug_assert!` panics on fewer than two elements in debug
  and test builds. In release builds, the guard returns the input unchanged.
- **Dead `None` branches.** Every `get` index is in bounds, so the `if let`
  fall-through branches never run. Production uses `get` only to satisfy the
  repository's indexing lints.
- **Redundant `pop`.** `rotate_cycle_by` reads only indices below `len`, so the
  `cycle.pop()` call does not affect the result. A mutant that deletes it is
  semantically equivalent; see `Verification plan`.

The only production caller is `back_edge_result` in
`src/ir/cycle/detector.rs:198`, which runs in `CycleSearch::Path` mode. The
public surface is `analyse` in `src/ir/cycle/mod.rs`, which returns
`CycleDetectionReport.cycle: Option<Vec<Utf8PathBuf>>`. That value flows into
`IrGenError::CircularDependency` and the human and JSON diagnostics.

### Existing verification of the kernel

`src/ir/cycle/verification.rs` holds 11 Kani harnesses. Four concern
canonicalization; the helpers they call are in
`src/ir/cycle/verification/support.rs`:

- `canonicalize_two_node_cycle_is_canonical`,
  `canonicalize_three_node_cycle_is_canonical`, and
  `canonicalize_four_node_cycle_is_canonical` drive the production
  `canonicalize_cycle_by` over symbolic `u8` identifiers. Each draws from an
  alphabet of size N and uses `kani::assume` to keep the identifiers
  **distinct**. They assert, through `assert_kernel_canonical_properties`,
  length preservation, closure, interior multiset preservation, a smallest
  first node, and that the output interior is a rotation of the input interior.
  All three use `#[kani::unwind(6)]` and `#[kani::solver(kissat)]`.
- `canonicalize_path_wrapper_matches_u8_kernel_for_two_nodes` checks that the
  `Utf8PathBuf` wrapper agrees with the `u8` kernel for the two concrete
  orderings `[a, b, a]` and `[b, a, b]`. Under `cfg(kani)`, the path comparator
  is a first-byte comparison, not camino's component order.

These four harnesses are the "Kani equivalent" the roadmap's acceptance rule
compares against.
`docs/execplans/4-2-2-kani-harnesses-for-cycle-canonicalization.md` records
their cost on the implementing host: N=2 took 4.577 s, N=3 took 7.602 s, N=4
took 11.815 s, and the path wrapper took 8.778 s of Kani verification time.
That is 23.994 s for the three kernel harnesses. Direct `Utf8PathBuf` proofs at
three nodes exceeded the 8 GiB memory cap, which is why the 4.2.2 boundary
stops at the `u8` kernel
([ADR-0047](../adrs/adr-0047-bound-kani-intermediate-representation-ir-harnesses-to-small-n.md)).
Four mutation patches in `docs/verification/mutations/` (named
`ir__cycle__verification__canonicalize_*.patch`) show each harness fails on a
seeded defect.

`src/ir/cycle/property_tests/mod.rs` (364 lines) holds the Proptest suite:
`canonicalize_is_idempotent`, `all_rotations_canonicalize_identically`,
`canonical_first_node_is_smallest`, and `canonical_cycle_is_closed`. They use 2
to 10 unique single-letter node names, so they never generate duplicates or
multi-component paths. `src/ir/cycle/tests.rs` and
`src/ir/cycle/property_tests/analyse_tests.rs` pin examples through `analyse`.

### Existing Verus infrastructure

- `tools/verus/VERSION` pins Verus `0.2026.10.04.426d8b0`.
  `tools/verus/SHA256SUMS` pins the x86-64 Linux archive digest.
- The `Makefile` pins `rust-prover-tools` at
  `592558f7183b554fc749696246984c5d7300d029` through `PROVER_TOOLS_SOURCE`.
  `make install-verus` and `make verus` delegate to its `verus install` and
  `verus run` subcommands.
  `VERUS_FLAGS ?= --proof-file verus/canonical_value/main.rs` is the default.
  Repeating `--proof-file` runs an ordered batch that stops at the first
  failure.
- `verus/canonical_value/main.rs` (399 lines) is the precedent kernel. It is a
  spec-level refinement proof over `Seq<u64>` with a `//!` module comment that
  states exactly what it does and does not prove. It ends with an empty
  `fn main() {}`, which Verus requires, and contains no `assume`, `admit`, or
  external body. Its recorded result is
  `verification results:: 16 verified, 0 errors`. No run time is recorded.
- Verus is outside `make test`, `make lint`, `make check-fmt`, `make all`,
  `make kani-check`, `make kani-full`, `make formal-pr`, and CI. No `.github/`
  file mentions Verus.
- Three tests pin the `make verus` default:
  - `tests/prover_tools_make_tests.rs`, around line 215, expects
    `["verus", "run", "--proof-file", "verus/canonical_value/main.rs"]`.
  - `scripts/tests/test_prover_tools_integration.py`, around line 149,
    expects `verus run --proof-file verus/canonical_value/main.rs` in standard
    output.
  - `tests/prover_tools_verus_e2e_tests.rs`, line 35, defines
    `CANONICAL_PROOF` for the real-verifier lane.

  The e2e lane skips when the pinned Verus install is absent.
  `make test-verus-e2e` sets `NETSUKE_VERUS_E2E_STRICT=1` to turn the skip into
  a failure.
- No test enforces the canonical-value kernel's "no `assume`" claim, its
  400-line limit, or its `//!` header. The 6.1.2 plan records that the kernel
  briefly reached 414 lines unnoticed.
- `tests/env_access_suppressions/roots.rs` already scans `verus/` recursively,
  and `.gitignore` excludes `.verus/` and `verus/build/`. A new subdirectory
  needs no registration there.

At the time of drafting, Verus is not installed on the implementing host:
`which verus` finds nothing. Kani 0.67.0 is installed under `~/.kani`.

### Size survey and historical budgets

Comparable plans and their budgets:

- `docs/execplans/6-1-2-canonical-value-key-and-equality-relation.md` set no
  numeric file or line limits. Its EP-M2 landed as 15 files, +2280/-1. Its
  Verus kernel exceeded the 400-line cap after a documentation repair and had
  to be trimmed back to 399.
- `docs/execplans/4-4-2-document-cycle-detection-scope.md` began with a
  20-file tolerance. Its own inventory needed about 30 files, so revision 2
  split the limit into 3 production files and 40 production lines, and 35 files
  and 1,200 net lines overall. The cause was missed scope at drafting, not
  growth during execution.
- `docs/execplans/4-2-2-kani-harnesses-for-cycle-canonicalization.md` allowed
  8 files and 5 minutes of solver time per harness. It never raised either.
  When one stage was blocked, the proof boundary was narrowed instead (the `u8`
  kernel).
- `docs/execplans/4-1-3-record-phase-1-scope-boundary-for-verus-and-stateright.md`
  allowed 4 files and required any real Verus proof to become a later item.
  Item 4.4.3 is that later item.

Lesson: inventory every pinned test and every doc that names the Verus layout
before setting the file budget, and budget the kernel's comment block inside
the 400-line cap. The inventory below gives the estimate the tolerances use.

Expected non-ExecPlan changes, if the kernel is accepted:

- Code:
  - `verus/cycle_canonicalization/main.rs` (new, up to 400 lines, the hard cap).
  - `src/ir/cycle/property_tests/canonical_spec_tests.rs` (new, about 200
    lines).
  - `src/ir/cycle/property_tests/mod.rs` (one `mod` line).
  - `tests/verus_kernel_contract_tests.rs` (new, about 180 lines).
  - Its insta snapshot under `tests/snapshots/` (new, about 50 lines; counted
    as code because it is a test artefact).
  - `tests/prover_tools_make_tests.rs`,
    `scripts/tests/test_prover_tools_integration.py`, and possibly
    `tests/prover_tools_verus_e2e_tests.rs` (default-batch expectations,
    about 20 lines in total).
- Configuration:
  - `Makefile` (`VERUS_FLAGS` default and its comment, under 10 lines).
- Documentation:
  - `docs/adrs/adr-0082-accept-a-verus-proof-kernel-for-cycle-canonicalization.md`
    (new, about 150 lines).
  - `docs/developers-guide.md` (about 70 lines).
  - `docs/formal-verification-methods-in-netsuke.md` (about 30 lines).
  - `docs/netsuke-design.md` (about 15 lines).
  - `docs/repository-layout.md` (about 15 lines).
  - `docs/contents.md` (about 4 lines).
  - `docs/roadmap.md` (about 10 lines).

That totals 18 files: about 920 code lines (net), under 10 configuration lines,
and about 300 documentation lines.

## Signposts: documentation and skills

Read these before starting, in this order:

- `AGENTS.md`, for gates, file-size limits, doc-comment rules, the en-GB
  Oxford spelling rule, and the testing rules (no in-process environment
  mutation; every assertion carries a message).
- `docs/formal-verification-methods-in-netsuke.md`, especially "Current
  repository state", "Kani for the IR core", and "Optional Verus proof kernel".
- `docs/developers-guide.md`, the `make verus` description (around line 6966)
  and "Kani harness inventory" (around line 7190).
- `docs/netsuke-design.md` §5.3, step 4 "Graph Validation" (around line 2622).
- [ADR-0047](../adrs/adr-0047-bound-kani-intermediate-representation-ir-harnesses-to-small-n.md)
  and
  [ADR-0068](../adrs/adr-0068-detect-cycles-across-all-prerequisite-kinds.md).
- `docs/execplans/4-2-2-kani-harnesses-for-cycle-canonicalization.md` and
  `docs/execplans/6-1-2-canonical-value-key-and-equality-relation.md`, as
  historical evidence. Do not edit them.
- `verus/canonical_value/main.rs`, the house style for a Verus kernel.
- `docs/rust-testing-with-rstest-fixtures.md`,
  `docs/reliable-testing-in-rust-via-dependency-injection.md`, and
  `docs/rust-doctest-dry-guide.md`, for test shape.
- `docs/rstest-bdd-users-guide.md`. It is not used here, because no
  user-visible behaviour changes; see `Decision log` D9.
- `docs/ortho-config-users-guide.md`. It is not used here, because no
  configuration surface changes; see `Decision log` D10.
- `docs/documentation-style-guide.md`, for ADR rules (four-digit serials,
  required sections, amendment banners) and Markdown rules.
- `docs/netsuke-design.md`, the design document of record.

Skills to load during implementation:

- `rust-router`, then `verus` for proof engineering (triggers, `assert ... by`,
  `broadcast use vstd::seq::group_seq_axioms`, and the review checklist).
- `rust-verification`, if the proof-versus-test split needs revisiting.
- `proptest`, for the specification-oracle and rank-image properties.
- `rust-unit-testing`, for `rstest` tables, `googletest`, and
  `pretty_assertions`.
- `kani`, only to re-measure the Kani baseline.
- `hexagonal-architecture`, as a boundary check. The kernel is pure domain
  logic with no ports. The proof must not pull adapters (filesystem, process,
  `HashMap` storage) into its boundary.
- `codegraph-mcp`, for callers and impact analysis on `src/ir/cycle/support.rs`.
- `execplans`, to keep this document current.
- `en-gb-oxendict-style`, for prose.

## Conformance basis

Upstream artefacts, all at `origin/main` `5c707ef33`:

- `docs/roadmap.md` §4.4, item 4.4.3. Requirements:
  - `RM-443-OUT`: keep the proof outside Cargo.
  - `RM-443-MODEL`: use proof-specific model types rather than production
    `HashMap` structures.
  - `RM-443-ACC`: accept the proof only if it remains narrower and cheaper than
    the Kani equivalent.
  - Prerequisites 4.2.2 and 4.1.3 are both complete.
- `docs/formal-verification-methods-in-netsuke.md` §"Optional Verus proof
  kernel":
  - `FV-KERNEL`: proof-kernel-only, not a production subsystem, and not a
    routine gate.
  - `FV-CLAUSES`: the four contract clauses (length preserved, cycle stays
    closed, interior multiset preserved, start node stable under the ordering
    rule).
  - `FV-GATES`: Verus stays outside the pull-request gates, `make test`,
    `make lint`, `make check-fmt`, `make all`, `make kani-check`,
    `make kani-full`, and `make formal-pr`.
  - `FV-CI`: a Verus CI job is added only after a stable kernel exists.
- `docs/execplans/4-1-3-record-phase-1-scope-boundary-for-verus-and-stateright.md`:
  - `SB-CARGO`: keep Verus out of production Cargo.
  - `SB-FIRST`: the first proof focuses on cycle canonicalization.
- [ADR-0047](../adrs/adr-0047-bound-kani-intermediate-representation-ir-harnesses-to-small-n.md)
  (`ADR47-U8`): Kani proves the rotation kernel over `u8` for N=2..4. Direct
  `Utf8PathBuf` proofs are out of budget.
- [ADR-0068](../adrs/adr-0068-detect-cycles-across-all-prerequisite-kinds.md)
  (`ADR68-DET`): cycle paths are deterministic.
- `docs/netsuke-design.md` §5.3 step 4 (`DD-ROT`): the cycle list is rotated
  so its smallest node appears first, ensuring deterministic error messages.
- `AGENTS.md`:
  - `AG-400`: 400-line file cap.
  - `AG-DOC`: `//!` module comments and `///` item comments.
  - `AG-VER`: use `verus` for selected lemmas, with no artificial proof
    obligations.
  - `AG-ADR`: substantive decisions go in an ADR.

No Terms of Reference document governs this item. The roadmap, the
formal-verification design, and the ADRs above are the governing artefacts.

Trace chains:

```plaintext
RM-443-OUT, SB-CARGO, FV-KERNEL -> EP-M1 -> verus/cycle_canonicalization/main.rs
  (no Cargo manifest entry; VC-HYGIENE in tests/verus_kernel_contract_tests.rs)
RM-443-MODEL -> EP-M1 -> Seq<u64>/Vec<u64> model; no HashMap, Utf8PathBuf, or
  BuildGraph in the proof (VC-HYGIENE forbidden-path check)
FV-CLAUSES, DD-ROT, ADR68-DET -> EP-M1 -> T-CANON, C1..C7 in the kernel
  -> EP-M2 -> canonical_spec_tests (production meets the same specification)
RM-443-ACC -> EP-M0 baseline + EP-M1 measurement -> decision gate G1
  -> ADR-0082 (accept or reject)
FV-GATES, FV-CI -> EP-M3 -> make verus default batch only; no CI or gate change
AG-ADR -> EP-M4 -> ADR-0082, design doc, developers' guide
```

## Constraints

- Implementation begins only after the user explicitly approves this plan.
- Keep the Verus proof outside Cargo (`RM-443-OUT`, `SB-CARGO`). Do not add
  `vstd`, `verus_builtin`, or any Verus crate to any `Cargo.toml`. Do not add a
  workspace member or `#[path]` import between `verus/` and `src/`.
- Use proof-specific model types (`RM-443-MODEL`). The kernel uses `u64` node
  identifiers in `Vec<u64>` and `Seq<u64>`. It must not mention `HashMap`,
  `IrHashMap`, `BuildGraph`, `BuildEdge`, `Utf8PathBuf`, `camino`, MiniJinja,
  or any filesystem or process API.
- Do not add Verus to `make test`, `make lint`, `make check-fmt`, `make all`,
  `make kani-check`, `make kani-full`, `make formal-pr`, or any CI workflow
  (`FV-GATES`, `FV-CI`). Plain Rust tests that only *read* the Verus source as
  text may run in `make test`; they do not run Verus.
- Do not change production behaviour. `src/ir/cycle/support.rs` must keep its
  current semantics: rotation rule, tie-break, short-input guard, and
  `debug_assert!`. A production edit is allowed only if the Verus proof exposes
  a real defect. That triggers the escalation in `Tolerances`.
- Keep every existing Kani harness, Kani mutation patch, and Proptest
  property. The Verus kernel complements them; it does not replace them
  (`Decision log` D4).
- No `assume`, `admit`, `#[verifier::external_body]`,
  `#[verifier::external]`, `external_fn_specification`, `assume_specification`,
  `#[verifier::trusted]`, or user-declared axiom in any Verus file. The only
  trusted base is Verus, Z3, and `vstd`.
- Every file stays at or below 400 lines (`AG-400`), including the Verus
  kernel and its comments. Every module begins with `//!`, and every function,
  spec function, and lemma carries `///` (`AG-DOC`).
- No new dependencies. `proptest`, `rstest`, `googletest`,
  `pretty_assertions`, `insta`, `regex`, and `syn` are already dev-dependencies.
  `sha2` is a normal dependency.
- No in-process environment mutation in tests. The new tests read files under
  the repository root, located from `env!("CARGO_MANIFEST_DIR")` at compile
  time. That is a compile-time constant, not a runtime environment read.
- Use `cap_std` and `camino` for filesystem access in new tests, as the
  repository's other source-reading contract tests do.
- Do not edit completed ExecPlans (4.2.2, 4.4.2, 6.1.2, 4.1.3); they are
  historical. Accepted ADRs are not rewritten; add amendment banners only if a
  decision changes. No ADR decision changes here.
- The ADR serial must be re-derived across every remote branch immediately
  before the ADR commit and again before the push (`Concrete steps`, step M4.1).
  `ADR-0082` is provisional.
- Comments and docs use en-GB Oxford spelling (`-ize`, `-yse`, `-our`).

## Tolerances (exception triggers)

Counting rules:

- Comparison base: the merge base with `origin/main`
  (`git merge-base origin/main HEAD`).
- Command: `git diff --numstat "$(git merge-base origin/main HEAD)" -- .
  ':(exclude)docs/execplans/4-4-3-verus-proof-kernel-for-cycle-canonicalization.md'`
- This ExecPlan is excluded from every count, including when it is edited in
  the same commit as other files.
- Limits measure **net** lines (additions minus deletions) per category.
  Categories are assigned whole-file:
  - Code: `verus/**`, `src/**`, `tests/**` (including snapshots), and
    `scripts/**`.
  - Configuration: `Makefile`, `Cargo.toml`, `Cargo.lock`, and
    `.github/**`.
  - Documentation: `docs/**` except ExecPlans, and `README*`.
- There are no generated-file exclusions.

The budgets below use the size survey in `Context and orientation`: estimates
plus about 25% contingency. The contingency follows 4.4.2's missed-inventory
overrun and 6.1.2's late comment growth. Spare capacity in one category never
offsets another.

- Files: stop and escalate if more than 22 non-ExecPlan files change. The
  estimate is 18.
- Code: stop and escalate above 1,150 net lines. The estimate is about 920.
- Kernel size: stop and escalate if `verus/cycle_canonicalization/main.rs`
  cannot be kept at or below 400 lines. Do not split it into submodules to
  dodge the cap without approval; a split is itself evidence against "narrower"
  (G1).
- Configuration: stop and escalate above 15 net lines, or on any change to
  `Cargo.toml`, `Cargo.lock`, or `.github/**`. The estimate is under 10, all in
  the `Makefile`.
- Documentation: stop and escalate above 400 net lines. The estimate is
  about 300.
- Interface: stop and escalate on any change to a production signature or
  visibility in `src/`, or on any change to `make verus` other than its default
  `VERUS_FLAGS`.
- Dependencies: stop and escalate on any new crate, Verus version bump, or
  `rust-prover-tools` pin bump.
- Proof soundness: stop and escalate if a proof would need any construct
  listed under `Constraints` as forbidden, or a precondition that excludes
  inputs the production caller can supply.
- Proof effort: stop and escalate if, after 6 focused proof iterations on a
  single obligation, Verus still reports errors or timeouts at the default
  `rlimit`. Raising `rlimit` above Verus's default counts as an escalation.
- Proof cost: stop at decision gate G1 if the accepted kernel's measured
  verification time or peak memory fails the acceptance rule. That is a
  rejection outcome, not an iteration trigger. Do not tune the proof to scrape
  under the threshold by deleting obligations listed in `Verification plan`.
- Production defect: stop and escalate if Verus or the new property tests
  expose a production behaviour that contradicts `DD-ROT` or the 4.2.2 Kani
  claims.
- Iterations: stop and escalate if a gate still fails after 3 fix attempts
  on the same failure.
- Time: stop and escalate if any milestone exceeds 2 working sessions.
- Ambiguity: stop and escalate if a reviewer or the user questions the
  meaning of "narrower" or "cheaper" beyond the definitions in
  `Verification plan` §"Acceptance rule".

## Risks

- R1 (medium): Verus may not accept the production loop shape verbatim.
  Production uses `slice::get` with `if let` chains and `Vec::with_capacity`,
  `first().cloned()`, and `pop`. `vstd` may lack specifications for some of
  them. Mitigation: the mirror may replace a `get`-and-`if let` with direct
  indexing, but only where the proof shows the index is in bounds, so the
  `None` branch is dead. Record every such rewrite in a mirror-divergence list
  in the kernel's `//!` comment and in `Decision log`. Each rewrite must
  preserve control flow at the level of comparisons, indices, and pushes.
- R2 (medium): rotation-composition reasoning with wrap-around indices may hit
  trigger or nonlinear-arithmetic trouble. Mitigation: state rotation
  index-wise with an explicit case split (`i + k < n` or not) instead of `%`.
  Scope helper lemmas with `assert ... by`. Use
  `broadcast use vstd::seq::group_seq_axioms`.
- R3 (medium): the kernel may not fit in 400 lines with the house-style
  comment block. Mitigation: keep the `//!` block to about 40 lines and move
  the long rationale to ADR-0082 and the developers' guide. If it still exceeds
  the cap, that counts against "narrower" at G1.
- R4 (low): Verus may be slower than the recorded Kani baseline because of
  Z3 start-up or `vstd` loading. Mitigation: compare like with like (see
  `Acceptance rule`). Record both verifier time and end-to-end wall time, and
  re-measure the Kani baseline on the same host in the same session.
- R5 (medium): the mirror may drift from production after this item lands.
  Mitigation: VC-TRIPWIRE, an insta snapshot of the whitespace-normalized,
  comment-stripped source of the four mirrored production functions. Any edit
  to them fails `make test` until a maintainer re-reviews the mirror and
  accepts the new snapshot. The snapshot header and the developers' guide say
  so.
- R6 (low): installing Verus writes a large `.verus/` tree (about 1.6 GB, per
  the 6.1.2 plan). The coverage walk once tripped over it. Mitigation:
  `.gitignore` and `tests/env_access_suppressions/roots.rs` already exclude
  `.verus/` and `verus/build/`. Confirm with `git status` after install.
- R7 (low): the ADR serial may collide with an open branch. Mitigation: the
  sweep in `Concrete steps` M4.1, run immediately before the ADR commit and
  again before the push.
- R8 (low): the `make verus` default change ripples into three pinned tests,
  two of them outside `make test` (`make test-prover-tools-integration` and
  `make test-verus-e2e`). Mitigation: EP-M3 runs both explicitly.
- R9 (low): the precondition bounding the cycle length, needed so
  `start + offset` cannot overflow `usize`, might be judged an assumption.
  Mitigation: it is a `requires` clause discharged by the axiom AXM-ALLOC (see
  `Verification plan`), not an `assume`. VC-PREMISE records why every real
  caller satisfies it.
- R10 (low): `make test` builds the new contract test even on hosts without
  Verus. Mitigation: the test only reads text, needs no Verus, and runs
  everywhere.

## Verification plan

This section is written before any implementation, and is the source from which
the implementation is designed. Lemma names are hypotheses about the eventual
proof structure. Implementation may merge, split, or drop them, and must record
any such change in `Decision log`. No theorem statement below is claimed proved
until EP-M1 records Verus output for it.

### Where Verus is warranted and where it is not

Verus is warranted for exactly one thing: the pure rotation kernel's contract
over **all** cycle lengths and **all** value patterns, including ties. Kani
covers only N=2..4 with distinct values, and Proptest only samples. The kernel
is pure and loop-based, its contract is crisp, and it needs no I/O, concurrency,
`HashMap`, or `unsafe`. That is the profile the `verus` skill and `FV-KERNEL`
describe.

Verus is **not** warranted for the following. Each item gets the cheaper
technique named:

- The detector's traversal (`CycleDetector`). It is already covered by Kani
  (7 harnesses), the edge-oracle Proptest, and BDD and end-to-end tests under
  ADR-0068. It is `HashMap`-driven, which `RM-443-MODEL` excludes.
- The `Utf8PathBuf` wrapper and camino's ordering. These are third-party
  behaviour, treated as the axiom AXM-ORD. The repository-owned glue is checked
  by VC-RANK, a Proptest property, against the real camino type.
- The short-input guard and `debug_assert!`. The mirror models the guard so
  the contract is total, but the debug panic is a debug-build diagnostic,
  covered by inspection and the existing detector guarantee that a cycle has at
  least two elements.
- Diagnostics formatting, localization, and JSON. These are unchanged and
  covered by existing snapshot and BDD tests.
- `make verus` wiring. This is integration plumbing, covered by the existing
  fake-verifier and real-verifier test lanes.

### Abstract model

The model works over sequences of `u64` node identifiers with the natural order
`<=`. It is a stand-in for any lawful total order (see AXM-ORD and LEM-RANK).
Definitions, stated independently of the algorithm:

```rust
/// The interior of a closed cycle: every element except the closing repeat.
spec fn interior(c: Seq<u64>) -> Seq<u64> { c.drop_last() }

/// `s` rotated left by `i`: elements `i..n` followed by `0..i`.
spec fn rotation(s: Seq<u64>, i: int) -> Seq<u64>
    recommends 0 <= i <= s.len()
{ s.subrange(i, s.len() as int) + s.subrange(0, i) }

/// `i` is the first index holding a minimal element of `s`.
spec fn is_first_min(s: Seq<u64>, i: int) -> bool {
    0 <= i < s.len()
    && (forall |j: int| 0 <= j < s.len() ==> s[i] <= #[trigger] s[j])
    && (forall |j: int| 0 <= j < i ==> s[i] < #[trigger] s[j])
}

/// The canonical closed form of interior `s` started at `i`.
spec fn canonical_form(s: Seq<u64>, i: int) -> Seq<u64> {
    rotation(s, i).push(s[i])
}

/// `s` has exactly one minimal element.
spec fn has_unique_min(s: Seq<u64>) -> bool {
    exists |i: int| is_first_min(s, i)
        && forall |j: int| 0 <= j < s.len() && j != i ==> s[i] < #[trigger] s[j]
}
```

The trigger annotations above are illustrative. Trigger choice is deferred to
implementation (see R2).

### Proof-specific mirror

The mirror lives in `verus/cycle_canonicalization/main.rs`. It is `exec` code
that transcribes the four production functions over `Vec<u64>` with `<` as the
comparator:

- `rotate_index_model(start, offset, len) -> usize` mirrors `rotate_index`.
- `find_rotation_start_model(cycle: &Vec<u64>, len) -> usize` mirrors
  `find_rotation_start_by`.
- `rotate_cycle_model(cycle: &Vec<u64>, start, len) -> Vec<u64>` mirrors
  `rotate_cycle_by`.
- `canonicalize_cycle_model(cycle: Vec<u64>) -> Vec<u64>` mirrors
  `canonicalize_cycle_by`, including the `len < 2` guard and the `pop`.

Representation relationship (REP): each mirror function has the same loop
structure, the same index arithmetic, the same comparison direction (strict `<`
standing for `Ordering::Less`), the same push order, and the same early-return
guard as its production counterpart. The only divergences allowed are those
listed under R1, each enumerated in the kernel's `//!` comment. Three things
carry REP: code review, VC-TRIPWIRE (production changes force re-review), and
VC-SPEC-ORACLE (production meets the same specification on executable inputs).

### Axioms (trusted, not proved here)

- AXM-VERUS: Verus `0.2026.10.04.426d8b0`, its bundled Z3, and `vstd` are
  sound for the constructs used. This is the documented trusted computing base.
- AXM-ORD: `Utf8PathBuf: Ord` (camino 1.2.6) is a lawful total order
  (reflexive, antisymmetric with respect to its `Eq`, transitive, and total),
  as Rust's `Ord` contract requires. This is the documented interface of a
  third-party crate.
- AXM-CLONE: `Utf8PathBuf::clone` returns a value equal to its source.
- AXM-ALLOC: a Rust allocation never exceeds `isize::MAX` bytes. A `Vec<T>`
  with `size_of::<T>() >= 1` therefore has length at most `isize::MAX`, and
  `Utf8PathBuf` is 24 bytes on 64-bit targets. This discharges the mirror's
  length precondition for every production call.
- AXM-DET: the detector passes canonicalization a closed sequence of length at
  least 2 whose interior elements are pairwise distinct, because a
  depth-first-search stack holds each visiting node once. This is an existing
  detector property. It is checked by VC-DISTINCT (EP-M2), not by Verus.

### Invariants and lemmas

Main theorem, stated as the mirror's `ensures`:

- T-CANON: for every `cycle: Vec<u64>` with
  `cycle.len() <= MAX_MODEL_LEN`, `canonicalize_cycle_model(cycle)` returns
  `out` such that:
  - if `cycle@.len() < 2`, then `out@ == cycle@`;
  - otherwise there exists `i` with `is_first_min(interior(cycle@), i)` and
    `out@ == canonical_form(interior(cycle@), i)`.

  Here `MAX_MODEL_LEN` is `usize::MAX / 2`, or whatever weaker bound the
  overflow obligation actually needs; implementation records the exact value.

Loop and helper contracts (provisional):

- INV-FIND: in `find_rotation_start_model`, at loop head with counter `index`,
  `is_first_min(cycle@.subrange(0, index), start)` holds, given
  `1 <= len <= cycle.len()`. On exit,
  `is_first_min(cycle@.subrange(0, len), start)`.
- INV-ROT: in `rotate_cycle_model`, at loop head with counter `offset`,
  `canonical@ == rotation(cycle@.subrange(0, len), start).subrange(0, offset)`.
  On exit, after the closing push,
  `out@ == canonical_form(cycle@.subrange(0, len), start)`.
- LEM-IDX: `rotate_index_model(start, offset, len)` equals
  `start + offset` when that is below `len`, and `start + offset - len`
  otherwise. The result is below `len`, and the addition does not overflow
  under the length precondition.
- LEM-ROT-AT: for `0 <= k < n`, `rotation(s, i)[k] == s[i + k]` when
  `i + k < n`, and `s[i + k - n]` otherwise. This connects LEM-IDX to INV-ROT.
- LEM-POP: `cycle@.subrange(0, len)` equals `interior(cycle@)`, before and
  after `pop`. This is why the `pop` mutant is equivalent.

Specification corollaries. These prove that `canonical_form` at a first-min
index has the properties `FV-CLAUSES` and `DD-ROT` promise. They are the
semantic content; T-CANON transfers them to the mirror.

- C1 (length): `canonical_form(s, i).len() == s.len() + 1`, so
  `out.len() == cycle.len()`.
- C2 (closure): `out[0] == out[out.len() - 1]`.
- C3 (multiset): `interior(out).to_multiset() == s.to_multiset()`. Via
  `vstd::seq_lib::lemma_multiset_commutative` over the two subranges.
- C4 (minimal start): `forall j. out[0] <= s[j]`.
- C5 (tie-break): `forall j < i. out[0] < s[j]`, so the output starts at the
  first occurrence of the minimum. Kani cannot express this because its
  harnesses assume distinct identifiers.
- C6 (idempotence): `is_first_min(interior(out), 0)`, and
  `canonical_form(interior(out), 0) == out`. Applying the kernel to its own
  output changes nothing. This holds without the unique-minimum premise.
- C7 (rotation invariance, the "stable start" clause): if
  `has_unique_min(s)` and `t == rotation(s, k)` for `0 <= k < s.len()`, then
  the canonical forms of `s` and `t` at their first-min indices are equal. This
  is the property that makes cycle diagnostics deterministic regardless of
  where the depth-first search entered the cycle. It needs
  - LEM-ROT-COMPOSE: `rotation(rotation(s, k), j)` equals `rotation(s, k + j)`
    when `k + j < n`, and `rotation(s, k + j - n)` otherwise; and
  - LEM-FM-UNIQ: at most one index satisfies `is_first_min(s, _)`.
- C7-NEED (premise necessity): a concrete witness,
  `s = [1, 2, 1, 3]` and `t = rotation(s, 2) = [1, 3, 1, 2]`, whose canonical
  forms differ (`[1, 2, 1, 3, 1]` against `[1, 3, 1, 2, 1]`). This shows C7's
  unique-minimum premise is necessary, not decorative. AXM-DET supplies that
  premise in production, because distinct interiors have unique minima.

### Mapping to the Kani equivalent

The three `u8` kernel harnesses assert length (C1), closure (C2), multiset
(C3), smallest start (C4), and "is a rotation" (INV-ROT/T-CANON) for N=2..4
with distinct values. The Verus kernel proves the same five clauses for every N
and every value pattern, and adds C5, C6, and C7. It does **not** cover the
path-wrapper harness, which stays Kani's and gains VC-RANK as a Proptest
companion.

### Verification artefacts, methods, and evidence

Each obligation is listed with its method, artefact, command, and expected
evidence. Each also has a non-vacuity argument explaining how it can fail.

**VP-1: T-CANON, INV-*, LEM-*, C1–C7, and C7-NEED.**

- Method: Verus deductive proof.
- Artefact: `verus/cycle_canonicalization/main.rs`.
- Command:
  `make verus VERUS_FLAGS='--proof-file verus/cycle_canonicalization/main.rs'`.
- Expected: `verification results:: N verified, 0 errors`, with N recorded.
- Non-vacuity:
  1. Witness `exec` functions call `canonicalize_cycle_model` on
     `vec![3, 1, 2, 3]` (expecting `[1, 2, 3, 1]`), on `vec![2, 1, 1, 2]`
     (duplicates; expecting `[1, 1, 2, 1]`), on `vec![5, 5, 5, 5]`, and on
     `vec![7]` (guard). Each asserts the exact output. These show the
     `requires` clauses are satisfiable and the `ensures` clauses pin a
     specific result rather than holding vacuously.
  2. Spec-level witnesses show C7's premise is inhabited
     (`has_unique_min([3, 1, 2])`) and C7-NEED shows it is necessary.
  3. Seeded mutants of the mirror must each make Verus report an error:
     - M1: `<` becomes `>` in `find_rotation_start_model`.
     - M2: `<` becomes `<=`, so the last minimum wins. Kani's distinct-value
       harnesses cannot catch this mutant.
     - M3: delete the closing push in `rotate_cycle_model`.
     - M4: `index >= len` becomes `index > len` in `rotate_index_model`.
     - M5: `len` becomes `cycle.len()` in `canonicalize_cycle_model`.

     M6 (delete `pop`) is expected to verify, because it is semantically
     equivalent (LEM-POP). The run records it as such.

     Each mutant is applied in a scratch copy under `/tmp`, never in the
     worktree. The Verus error line is recorded in `Outcomes & retrospective`.
  4. A trusted-base audit (VC-HYGIENE, below) confirms the file has no
     forbidden construct.
- Discharge condition: zero errors on the unmutated file, an error on each of
  M1–M5, and VC-HYGIENE green.

**VC-SPEC-ORACLE: production meets the specification on executable inputs, at
large N, including ties.**

- Method: Proptest.
- Artefact: `src/ir/cycle/property_tests/canonical_spec_tests.rs`.
- Implementation: a straightforward Rust transcription of `is_first_min` and
  `canonical_form`. It is quadratic and declarative, and shares no code with
  production. Production `canonicalize_cycle_by(closed, u64::cmp)` must equal
  it for interiors of length 1..=64 drawn from a small alphabet (0..=7, so ties
  are frequent) and from a wide alphabet (all `u64`).
- Command:
  `cargo nextest run --all-features -E 'test(/canonical_spec/)'`, then
  `make test`.
- Non-vacuity:
  - Classify generated cases with `prop_assert!` counters or a
    `proptest::test_runner` result log. Record the share with a duplicated
    minimum, which must be non-zero.
  - Seeded production mutants, applied in scratch only, must fail the
    property: flipping `Less` to `Greater` (already a 4.2.2 patch), and a
    tie-break flip that uses `!= Greater`.
  - This is the red substitute for this milestone, because production
    already behaves correctly; see `Decision log` D7.

**VC-RANK: the path wrapper agrees with the `u64` kernel on the rank image of
real camino paths (AXM-ORD glue, LEM-RANK).**

- Method: Proptest.
- Artefact: the same file as VC-SPEC-ORACLE.
- Implementation: generate cycles of multi-component `Utf8PathBuf` values,
  including pairs that differ byte-wise but compare equal component-wise (for
  example `a/b` and `a//b`). Map each path to its rank among the cycle's
  sorted, deduplicated values. Assert that `canonicalize_cycle(paths)` mapped
  to ranks equals `canonicalize_cycle_by(ranks, u64::cmp)`.
- Non-vacuity:
  - Record the share of cases containing a component-equal but byte-unequal
    pair, which must be non-zero.
  - A scratch mutant that switches `path_cmp` to `as_str().cmp` must fail at
    least one case where byte order and component order disagree. This is
    the evidence that the property depends on the real comparator.

**VC-TIES: named examples.**

- Method: `rstest` table with `googletest` and `pretty_assertions`.
- Artefact: `canonical_spec_tests.rs`.
- Cases: `[2, 1, 1, 2] -> [1, 1, 2, 1]`, `[5, 5, 5, 5]` unchanged,
  `[3, 1, 2, 1, 3] -> [1, 2, 1, 3, 1]`, and the two-node cases.
- Non-vacuity: the M2-style tie mutant in scratch fails the first and third
  cases.

**VC-DISTINCT: AXM-DET holds.**

- Method: Proptest over `analyse`.
- Artefact: `canonical_spec_tests.rs`.
- Implementation: for generated cyclic graphs (reusing the existing
  `EdgeBuilder` helpers), every reported cycle has length at least 2 and a
  pairwise-distinct interior.
- Before writing it, check whether an existing property already asserts this;
  if so, cite it in `Decision log` instead.
- Non-vacuity: generator classification records how many cases were cyclic,
  which must be non-zero.

**VC-HYGIENE: trusted-base and layout contract for every Verus kernel.**

- Method: `rstest` contract test, run in `make test`, that reads text and
  needs no Verus.
- Artefact: `tests/verus_kernel_contract_tests.rs`, parameterized over
  `verus/canonical_value/main.rs` and `verus/cycle_canonicalization/main.rs`.
- Assertions, made after stripping `//` comments:
  - The file exists and is at most 400 lines.
  - It starts with `//!`.
  - It contains no forbidden construct: `assume(`, `admit(`, `external_body`,
    `external_fn_specification`, `assume_specification`, `verifier::external`,
    `verifier::trusted`, or `axiom`.
  - It mentions none of `HashMap`, `Utf8PathBuf`, `camino`, `BuildGraph`,
    `std::fs`, or `std::process`.
  - The Verus files are not referenced from any `Cargo.toml`.
- Non-vacuity: `rstest` cases feed in-memory fixtures. One fixture with
  `assume(false)` in code must fail. One fixture with the word `assume` only
  inside a `//` comment must pass, since the canonical-value kernel's prose
  mentions it. One 401-line fixture must fail. The checker is a pure function
  over text, so it can be tested without files.

**VC-TRIPWIRE: mirror drift detection.**

- Method: insta snapshot.
- Artefact: the same test file, plus `tests/snapshots/`.
- Implementation: extract the four mirrored production functions from
  `src/ir/cycle/support.rs` by signature line and brace matching, strip `//`
  comments, collapse whitespace, and snapshot the result.
- The snapshot's description says: "If this snapshot changes, re-review
  `verus/cycle_canonicalization/main.rs` against the new production code and
  re-run `make verus` before accepting." A sibling assertion checks that each
  of the four mirror function names still occurs in the Verus file.
- Non-vacuity: a unit test of the extractor on an inline fixture shows it
  captures a whole function body, including nested braces, and fails on a
  missing function.

**VC-WIRING: `make verus` runs both kernels.**

- Method: the existing pinned tests, updated in EP-M3.
- Command: `make test`, `make test-prover-tools-integration`, and, with
  Verus installed, `make test-verus-e2e` (strict mode).
- Non-vacuity: the tests are updated first and fail against the old
  `Makefile` default (red), then pass after the change (green).

### Acceptance rule (decision gate G1)

This is how `RM-443-ACC` is made measurable. The Kani equivalent is the three
`u8` kernel harnesses, which are the harnesses whose obligations the Verus
kernel duplicates. The path-wrapper harness has no Verus counterpart and stays
outside the comparison.

**Narrower.** The Verus kernel must satisfy all of these:

- N1: Its proof boundary is no wider than the Kani kernel harnesses' boundary.
  That means the pure rotation kernel only: no path type, comparator adapter,
  detector, `HashMap`, or other production structure, as enforced by VC-HYGIENE.
- N2: Its trusted base adds nothing beyond AXM-VERUS. There is no `assume`,
  `admit`, external body, or user axiom (VC-HYGIENE). AXM-ORD and AXM-ALLOC are
  used only in prose, to relate the model to production. They are never encoded
  as Verus facts.
- N3: It is one file of at most 400 lines.
- N4: It adds no Cargo dependency, CI job, or routine gate.

**Cheaper.** These are measured on the same host, in the same session, each
with a warm cache:

- K1: Verus verification time for the kernel file must not exceed the summed
  Kani verification time of the three kernel harnesses, re-measured in EP-M0.
  Measure Verus with its `--time` output and Kani with its per-harness
  "Verification Time" lines.
- K2: Peak resident memory of the Verus process tree must not exceed the
  peak of the Kani harness runs. Measure both with `/usr/bin/time -v` ("Maximum
  resident set size").
- K3: Record end-to-end wall time for both, including Kani's compile through
  `kani-compiler` and Verus's own start-up. This is informative, not a gate,
  because Kani's compile cost is shared by all 17 harnesses.

If N1–N4, K1, and K2 all hold, the kernel is **accepted**: continue with EP-M2
to EP-M4, and ADR-0082 records acceptance with the measurements.

If any of them fails, the kernel is **rejected**:

1. Stop. Set `Status: BLOCKED`.
2. Record the evidence in `Decision log`.
3. Escalate to the user with two options:
   - Option A: record the rejection in an ADR, amend the formal-verification
     document's "two kernels are accepted" statement, and delete the kernel
     file from the branch.
   - Option B: if the miss is marginal, take a user-approved revision of the
     rule.

Do not silently relax the rule.

## Milestones and plateaus

Each milestone ends in a coherent state that is safe to stop at. Gates listed
as "standard gates" are, sequentially and through the `scrutineer` agent:

1. `make check-fmt`
2. `make lint`
3. `make doc-coverage`
4. `make test`
5. `make markdownlint`
6. `make nixie`

Each gate's output goes to `/tmp/<gate>-netsuke-<branch>.out`. Because
`tests/execplan_status_contract_tests.rs` reads this plan, `make test` must
also pass on plan-only commits. Also run `make test-workflow-contracts` before
the final push, per the repository's plan-edit practice.

### EP-M0: Preflight and Kani baseline

- Requirements: `RM-443-ACC` (baseline).
- Work: no code changes.
  - Install Verus with `make install-verus`.
  - Confirm `make verus` verifies the existing canonical-value kernel, and
    record its time as a Verus start-up reference.
  - Re-measure the three Kani kernel harnesses and record K1/K2 baselines.
  - Confirm the exact `prover-tools verus run` flag for passing `--time`.
- End state: unchanged source tree. This plan gains the baseline evidence.
- Acceptance: the baseline figures are recorded in `Artefacts and notes`, and
  `git status` is clean apart from this plan.
- Recovery: none needed. Re-run the measurements.
- Commit: plan update only, after the standard gates.

### EP-M1: Verus kernel prototype and decision gate G1 (prototyping)

- Requirements: `RM-443-OUT`, `RM-443-MODEL`, `FV-CLAUSES`, `DD-ROT`, and
  `RM-443-ACC`.
- Work:
  1. Write `verus/cycle_canonicalization/main.rs`: the `//!` comment, the
     spec functions, the mirror with its `requires` and `ensures`, and the
     witnesses. Run Verus first **without** loop invariants or lemmas. Red:
     Verus must report errors on the loop postconditions.
  2. Add invariants and lemmas until it verifies. Green.
  3. Refactor for size and trigger hygiene.
  4. Run mutants M1–M6 in scratch.
  5. Measure K1/K2/K3.
  6. Apply G1.
- End state, if accepted: the kernel file exists and verifies. Nothing else
  changes. `make verus` still defaults to the canonical-value kernel, so the
  tree is coherent.
- End state, if rejected: stop, as described under `Acceptance rule`.
- Acceptance:
  - `make verus VERUS_FLAGS='--proof-file verus/cycle_canonicalization/main.rs'`
    prints `0 errors`.
  - Each of M1–M5 prints at least one error, and M6 verifies.
  - The standard gates pass. `make test` is unaffected except by the plan.
  - `wc -l verus/cycle_canonicalization/main.rs` prints at most 400.
- Conformance check: does the mirror's divergence list match R1's allowance?
  Is any production type mentioned? Are the trusted constructs absent?
- Recovery: the kernel is a single additive file. `git revert` the commit.
- Commit: `Add Verus proof kernel for cycle canonicalization (4.4.3)`.

### EP-M2: Runtime correspondence and kernel hygiene tests

- Requirements: the REP relationship, AXM-ORD glue, AXM-DET, and `N2`
  enforcement.
- Work:
  - Add `canonical_spec_tests.rs`, registered from
    `src/ir/cycle/property_tests/mod.rs`, containing VC-SPEC-ORACLE,
    VC-RANK, VC-TIES, and VC-DISTINCT.
  - Add `tests/verus_kernel_contract_tests.rs` with VC-HYGIENE and
    VC-TRIPWIRE, plus its snapshot.
  - Red-green as stated per artefact. For the contract test, write the
    checker's fixture tests first: they fail to compile, then fail, before
    the checker exists.
- End state: `make test` enforces the kernel's hygiene and tripwire, and
  checks production against the specification. Verus is still not in any gate.
- Acceptance:
  - The standard gates pass.
  - The new tests appear in the nextest output. Check this by running
    `cargo nextest list --all-features | grep -E 'canonical_spec|verus_kernel_contract'`.
  - The scratch mutant evidence for VC-SPEC-ORACLE, VC-RANK, and VC-TIES is
    recorded.
- Note on Kani scope: `src/ir/cycle/property_tests/` lies under the `src/ir/`
  entry in `tools/kani/proof-scope.toml`. The pull request therefore runs
  `kani-smoke`; that is expected.
- Recovery: revert the commit; the tests are additive.
- Commit: `Test cycle canonicalization against its Verus specification`.

### EP-M3: `make verus` runs every accepted kernel

- Requirements: `FV-GATES` and `FV-CI`. The default changes, but no gate does.
- Work:
  1. Update `tests/prover_tools_make_tests.rs`,
     `scripts/tests/test_prover_tools_integration.py`, and, if its default
     case asserts the single file, `tests/prover_tools_verus_e2e_tests.rs`.
     Each must expect the ordered default batch:
     `--proof-file verus/canonical_value/main.rs --proof-file
     verus/cycle_canonicalization/main.rs`.
  2. Run them. Red.
  3. Change `VERUS_FLAGS` in the `Makefile`. Green.
- End state: a bare `make verus` proves both kernels. No routine gate runs
  Verus.
- Acceptance:
  - The standard gates pass.
  - `make test-prover-tools-integration` passes.
  - `make test-verus-e2e` passes in strict mode with Verus installed.
  - `make verus` prints two `0 errors` result lines.
- Recovery: revert the commit, which restores the single-file default.
- Commit: `Run both Verus kernels from make verus`.

### EP-M4: Decision record and documentation

- Requirements: `AG-ADR`, `DD-ROT` precision, and roadmap completion.
- Work:
  - Write ADR-0082 and index it in `docs/contents.md`.
  - Update `docs/netsuke-design.md` §5.3 step 4: state the precise ordering
    and tie rule, the contract clauses C1–C7, and links to the ADR and the
    kernel.
  - Update `docs/formal-verification-methods-in-netsuke.md`:
    - "Current repository state": two kernels exist, and `make verus` runs
      both.
    - "Kani for the IR core" and "Optional Verus proof kernel": the kernel
      now exists. Correct the stale `src/ir/cycle/mod.rs` reference to
      `src/ir/cycle/support.rs`, and correct footnote 7.
    - "Layout": the real path.
  - Update `docs/developers-guide.md`:
    - The `make verus` default.
    - A "Verus kernels" subsection: what each proves and does not prove, the
      mirror-maintenance rule tied to VC-TRIPWIRE, how to run the mutants,
      and the acceptance measurements.
    - Correct the stale `src/ir/cycle/mod.rs` reference.
  - Add `verus/` and `tools/verus/` entries to `docs/repository-layout.md`.
  - Mark roadmap 4.4.3 and its three sub-bullets done, with a completion note
    linking the ADR and this plan.
  - Set this plan's status to `COMPLETE`.
  - `docs/users-guide.md` does not change; see D9.
- Acceptance:
  - The standard gates, then `make test-workflow-contracts`.
  - Every ADR index entry's label matches its filename (`Concrete steps`
    M4.2).
  - `make spelling` passes as part of `make markdownlint`.
- Recovery: documentation-only. Revert the commit.
- Commit: `Record the cycle canonicalization Verus kernel decision`.

## Plan of work

The work is additive: a new proof file, a new test file, a new contract test, a
one-line `Makefile` default change, and documentation. No production code
changes.

The Verus kernel follows the house style of `verus/canonical_value/main.rs`:

1. A `//!` block of at most about 40 lines. It states what is proved, what is
   not, the representation relationship, the mirror-divergence list, and the
   trusted base.
2. `use vstd::prelude::*;` and then `verus! { ... }`.
3. Spec definitions.
4. Specification lemmas: LEM-ROT-AT, LEM-FM-UNIQ, LEM-ROT-COMPOSE, and C1–C7.
5. Mirror `exec` functions, with contracts and loop invariants.
6. Witnesses.
7. `fn main() {}`.

Lemma bodies are written only when Verus asks for them. Start from the
`ensures` clauses and let the reported failures drive which helper lemma is
needed. The provisional list in `Verification plan` is a map, not a quota.

The new property tests sit beside the existing cycle property tests, so they
use the same `EdgeBuilder` helpers and the existing module-private access to
`support::canonicalize_cycle_by`. The specification transcription in the test
must not call any production helper. It re-implements `is_first_min` with two
nested loops, which keeps the oracle independent.

The contract test is an integration test under `tests/`. It locates the
repository root with `env!("CARGO_MANIFEST_DIR")` and opens files through
`cap_std::fs_utf8::Dir`. Its checker is a pure function over `&str`, so the
fixture tests need no files.

## Concrete steps

Run every command from the worktree root,
`/home/leynos/.lody/repos/github---leynos---netsuke/worktrees/b60c44a4-1a20-4f80-8bc6-a44a565e923f`.
Let `BR=$(git branch --show-current)`.

M0.1, install and smoke-test Verus:

```bash
make install-verus 2>&1 | tee /tmp/install-verus-netsuke-$BR.out
make verus 2>&1 | tee /tmp/verus-netsuke-$BR.out
git status --short   # expect no .verus/ or verus/build/ entries
```

Expected tail: `verification results:: 16 verified, 0 errors`.

M0.2, confirm the time flag:

```bash
uv tool run --from 'git+https://github.com/leynos/rust-prover-tools@592558f7183b554fc749696246984c5d7300d029' \
  prover-tools verus run --help | tee /tmp/prover-help-netsuke-$BR.out
```

Use the documented extra-argument flag, expected to be `--extra-arg`, to pass
`--time` through to Verus.

M0.3, Kani baseline. Run each harness separately, using the same `cargo-kani`
the Makefile uses:

```bash
for h in canonicalize_two_node_cycle_is_canonical \
         canonicalize_three_node_cycle_is_canonical \
         canonicalize_four_node_cycle_is_canonical; do
  /usr/bin/time -v "$CARGO_HOME/bin/cargo-kani" --harness "$h" \
    2>&1 | tee "/tmp/kani-$h-netsuke-$BR.out"
done
grep -h -E 'Verification Time|Maximum resident' /tmp/kani-canonicalize_*-netsuke-$BR.out
```

`CARGO_HOME` defaults to `~/.cargo`. Run the loop twice and record the second,
warm run. Record the figures in `Artefacts and notes`. If the Makefile's
`kani-full` recipe adds flags (for example `KANI_RUSTFLAGS`), use the same
flags.

M1, prove and measure:

```bash
make verus VERUS_FLAGS='--proof-file verus/cycle_canonicalization/main.rs --extra-arg --time' \
  2>&1 | tee /tmp/verus-cycle-netsuke-$BR.out
/usr/bin/time -v make verus VERUS_FLAGS='--proof-file verus/cycle_canonicalization/main.rs' \
  2>&1 | tee /tmp/verus-cycle-mem-netsuke-$BR.out
wc -l verus/cycle_canonicalization/main.rs
```

Mutants, always in a scratch copy:

```bash
S=$(mktemp -d /tmp/verus-mutant-XXXX)
cp verus/cycle_canonicalization/main.rs "$S/main.rs"
# edit $S/main.rs for one mutant, then:
make verus VERUS_FLAGS="--proof-file $S/main.rs" 2>&1 | tee "/tmp/verus-M1-netsuke-$BR.out"
```

M2, focused runs before the full gates:

```bash
cargo nextest run --all-features -E 'test(/canonical_spec/)' 2>&1 | tee /tmp/canon-spec-netsuke-$BR.out
cargo nextest run --all-features --test verus_kernel_contract_tests 2>&1 | tee /tmp/verus-contract-netsuke-$BR.out
cargo insta review   # accept the tripwire snapshot only after reading it
```

M3:

```bash
make test-prover-tools-integration 2>&1 | tee /tmp/prover-int-netsuke-$BR.out
make test-verus-e2e 2>&1 | tee /tmp/verus-e2e-netsuke-$BR.out
make verus 2>&1 | tee /tmp/verus-all-netsuke-$BR.out
```

M4.1, re-derive the ADR serial across every remote branch. Do this immediately
before the ADR commit, and again before the push:

```bash
git fetch --all --prune -q
for ref in $(git for-each-ref --format='%(refname)' refs/remotes/origin); do
  git ls-tree -r --name-only "$ref" -- docs/ 2>/dev/null \
    | grep -oE '(^|/)adr-[0-9]{3,4}-' | grep -oE '[0-9]{3,4}'
done | sort -n | uniq | tail -3
```

On 2026-11-15 the highest allocation anywhere was 0081, held by the PR #808
branch, so 0082 is provisional. If the ceiling has moved, take the next number
above it and update every reference.

M4.2, check that every ADR index label matches its filename:

```bash
tr '\n' ' ' < docs/contents.md \
  | grep -oE '\[ADR-[0-9]{4}\]\(adrs/adr-[0-9]{4}-' \
  | awk -F'[][()-]' '{ if ($3 != $8) print "MISMATCH", $0 }'
```

The command should print nothing.

## Validation and acceptance

The item is done when all of the following hold:

- Accepted at G1, with N1–N4, K1, and K2 recorded.
- `make verus` (default, both kernels) prints two result lines ending
  `0 errors`. The cycle kernel's line reports at least as many verified items
  as there are entries in `Verification plan`, after any merges recorded in
  `Decision log`.
- Each seeded mutant M1–M5 makes Verus report an error, and M6 verifies.
  Transcripts are summarized in `Outcomes & retrospective`.
- `make test` passes, and its output includes the new tests:
  - `canonical_spec_tests` (VC-SPEC-ORACLE, VC-RANK, VC-TIES, VC-DISTINCT);
  - `verus_kernel_contract_tests` (VC-HYGIENE over both kernels, and
    VC-TRIPWIRE).
- Before the change, the contract test's fixture cases fail. The scratch
  production mutants fail VC-SPEC-ORACLE and VC-RANK.
- `make check-fmt`, `make lint`, `make doc-coverage`, `make markdownlint`,
  `make nixie`, `make test-workflow-contracts`,
  `make test-prover-tools-integration`, and `make test-verus-e2e` all pass.
- `docs/roadmap.md` marks 4.4.3 and its sub-bullets done. ADR-0082 is indexed.
  The design document states C1–C7 and the precise order.

Proposed acceptance scenarios, for maintainers. These are not a feature file,
because no user-visible behaviour changes (D9). They are exercised by the
commands above.

```plaintext
Feature: Cycle canonicalization proof kernel
  Scenario: A maintainer proves every accepted kernel
    Given the pinned Verus release is installed with "make install-verus"
    When the maintainer runs "make verus"
    Then the canonical value kernel reports 0 errors
    And the cycle canonicalization kernel reports 0 errors

  Scenario: The production kernel is edited
    Given a change to rotate_cycle_by in src/ir/cycle/support.rs
    When the maintainer runs "make test"
    Then the mirror tripwire snapshot fails
    And its message asks for the Verus mirror to be re-reviewed

  Scenario: A trusted shortcut is introduced into a kernel
    Given a Verus kernel containing "assume(false)"
    When the maintainer runs "make test"
    Then the Verus kernel contract test fails and names the construct
```

## Idempotence and recovery

Every step is re-runnable. `make install-verus` is idempotent against the
pinned version. Mutant runs use fresh `/tmp` directories and never touch the
worktree. Each milestone is one commit, or a small series, that `git revert`
undoes cleanly. If a gate fails half-way, fix the cause and re-run that gate
and every later gate in sequence. Do not re-run a gate that passed unless the
tree has changed since.

## Artefacts and notes

These are filled in during implementation.

- EP-M0 Verus start-up reference (canonical-value kernel): *pending*.
- EP-M0 Kani K1/K2 baseline (three kernel harnesses, warm): *pending*.
- EP-M1 Verus K1/K2/K3: *pending*.
- EP-M1 mutant results: *pending*.

## Interfaces and dependencies

- No production interface changes.
- New test-only items:
  - module `crate::ir::cycle::property_tests::canonical_spec_tests` (private);
  - integration test binary `verus_kernel_contract_tests`.
- New proof file: `verus/cycle_canonicalization/main.rs`. It is not part of
  any Cargo target.
- `Makefile`: the `VERUS_FLAGS` default becomes a two-file ordered batch.
  `make verus` keeps its interface otherwise.
- Dependencies: none added. Uses existing dev-dependencies (`proptest`,
  `rstest`, `googletest`, `pretty_assertions`, `insta`, `cap-std`, and
  `camino`) and the pinned Verus and `rust-prover-tools`.
- Reuse policy for the new contract test (`AGENTS.md` abstraction rule):
  - Its checker is the single owner of the "Verus kernel hygiene" rule. Any
    future `verus/<kernel>/main.rs` is registered by adding one `#[case]`.
  - It must not grow into a general Rust-source linter.
  - The developers' guide records this rule.

## Progress

- [x] 2026-11-15: branch renamed to
  `4-4-3-verus-proof-kernel-for-cycle-canonicalization`.
- [x] 2026-11-15: Lody session title set.
- [x] 2026-11-15: reconnaissance of the production kernel, Kani coverage,
  Verus infrastructure, documentation, and budget history.
- [x] 2026-11-15: ADR ceiling sweep. The highest allocation is 0081, so 0082
  is provisional.
- [x] 2026-11-15: draft revision 1 written.
- [ ] Expert-panel design review applied.
- [ ] User approval.
- [ ] EP-M0 preflight and Kani baseline.
- [ ] EP-M1 Verus kernel and decision gate G1.
- [ ] EP-M2 runtime correspondence and hygiene tests.
- [ ] EP-M3 `make verus` default batch.
- [ ] EP-M4 ADR, documentation, and roadmap.

## Surprises & discoveries

- 2026-11-15: several documents cite `src/ir/cycle/mod.rs` (and older plans
  `src/ir/cycle.rs`) as the home of `canonicalize_cycle`. It lives in
  `src/ir/cycle/support.rs`. Live docs are corrected in EP-M4; completed plans
  are left alone.
- 2026-11-15: the design document says "lexicographically smallest". The
  production order is camino's component-wise `Utf8Path` order, which differs
  from byte order for inputs such as `a/b` and `a-b`. No test generated
  multi-component paths until VC-RANK.
- 2026-11-15: the Kani canonicalization harnesses assume distinct
  identifiers, so the tie-break rule has never been verified. Proptest uses
  unique names too.
- 2026-11-15: `cycle.pop()` in `canonicalize_cycle_by` cannot affect the
  result, because `rotate_cycle_by` reads only indices below `len`. Deleting it
  is an equivalent mutant (M6).
- 2026-11-15: nothing enforces the canonical-value kernel's "no `assume`"
  claim or its 400-line cap. VC-HYGIENE closes that gap for both kernels.

## Decision log

- D1. Decision: the mirror is `exec` code over `Vec<u64>` that transcribes
  the production loops, proved against a declarative specification. The
  alternative was a purely spec-level recursive model like the canonical-value
  kernel. Rationale: the defects the 4.2.2 mutation patches seed are a flipped
  comparison, a lost closing push, and a wrong wrap. They live in loop control
  and index arithmetic, which a spec-level model would restate rather than
  check. An `exec` mirror also exposes the real overflow obligation on
  `start + offset` (AXM-ALLOC). Rejected: a `#[path]` import of `support.rs`.
  It drags in camino, `BuildGraph`, and `IrHashMap`, which violates
  `RM-443-MODEL`, and Verus cannot verify calls through an unspecified generic
  `fn` comparator.
- D2. Decision: `u64` with natural order, not a generic `T` with a spec
  comparator. Rationale: the Kani equivalent uses `u8` natural order, so the
  comparison is like for like. Generic scaffolding is speculative, per the
  `verus` skill. The lift to `Utf8PathBuf` is the rank-embedding argument
  (LEM-RANK: the kernel touches values only through comparison and clone), and
  VC-RANK exercises it against real camino.
- D3. Decision: prove C5 (tie-break), C6 (idempotence), and C7 (rotation
  invariance, with C7-NEED), beyond the Kani harness clauses. Rationale: C7 is
  the formal-verification design's "start node is stable" clause and the reason
  canonicalization exists (`DD-ROT`). C5 is the only rule that decides output
  on ties. Both live inside the same kernel boundary, so N1 still holds.
  "Narrower" constrains the boundary and trusted base, not the depth of the
  result.
- D4. Decision: keep all Kani canonicalization harnesses. Rationale: Kani
  checks the *compiled production* function, and Verus checks a mirror. Kani at
  small N is the strongest available evidence for the representation
  relationship. Retiring them would trade a production-code check for a mirror
  check. The user may override this at approval.
- D5. Decision: change the `make verus` default to run both kernels.
  Rationale: an accepted kernel that the default command skips will rot unrun.
  The ordered batch is the documented mechanism. No routine gate changes, which
  preserves `FV-GATES` and `FV-CI`.
- D6. Decision: add VC-HYGIENE and VC-TRIPWIRE to `make test`. Rationale: they
  are plain text checks that need no Verus. They mechanically enforce N2 and N3
  and the REP relationship, and they close the unenforced-claims gap the 6.1.2
  plan recorded. They do not run Verus, so `FV-GATES` holds.
- D7. Decision: for VC-SPEC-ORACLE and VC-RANK the red step is a seeded
  production mutant in scratch, not a failing test against the tree. Rationale:
  production is already correct. A test that fails on the unmodified tree would
  be a false red. The mutant run is the nearest observable substitute and is
  recorded.
- D8. Decision: provisional ADR serial 0082, re-derived at commit time.
  Rationale: on 2026-11-15 the highest allocation across remote branches was
  0081.
- D9. Decision: no `rstest-bdd` feature file and no users'-guide change.
  Rationale: no user-visible behaviour changes. Diagnostics, ordering, and exit
  codes are identical. The maintainer-facing scenarios above are exercised by
  Make targets and contract tests instead.
- D10. Decision: `ortho_config` is not involved. Rationale: no CLI flag,
  configuration key, or localized help text changes.
- D11. Decision: the short-input guard is modelled in the mirror, so T-CANON
  is total, but the `debug_assert!` panic is not modelled. Rationale: it is a
  debug-only diagnostic for an input the detector cannot produce (AXM-DET). The
  release-path behaviour is what a proof should pin.

## Outcomes & retrospective

Not started.

## Revision note

- Revision 1 (2026-11-15): initial draft, written from reconnaissance of the
  tree at `5c707ef33`.
