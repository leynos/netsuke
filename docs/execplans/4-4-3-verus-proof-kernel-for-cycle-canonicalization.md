# Evaluate a minimal Verus proof kernel for cycle canonicalization (roadmap 4.4.3)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: DRAFT

Revision 3, rebased on 2026-11-15 onto `origin/main` at
`fa06bdb21e49dfcd70e432e85123127ec9961684`. It follows an expert-panel design
review (revision 2) and the user's direction on decision D4 and on the new
Verus CI workflow (see `Revision note`). Implementation must not begin until
the user explicitly approves this plan.

Branch: `4-4-3-verus-proof-kernel-for-cycle-canonicalization`

Roadmap item: `4.4.3` in `docs/roadmap.md`.

## Purpose / big picture

Netsuke compiles a YAML manifest (a `Netsukefile`) into a Ninja build file.
Before it writes any Ninja text it lowers the manifest into an intermediate
representation (IR), a graph of build edges, and rejects the graph if it
contains a dependency cycle. The error names the cycle as a list of paths, for
example
`Circular dependency detected: ["headers.stamp", "tool", "headers.stamp"]`.

A cycle has no natural first node: `a -> b -> c -> a` and `b -> c -> a -> b`
are the same cycle. Netsuke therefore *canonicalizes* every reported cycle so
the message is deterministic. It rotates the cycle so that its smallest node
comes first, then repeats that node at the end to close the cycle again. The
code is `canonicalize_cycle_by` in `src/ir/cycle/support.rs`, a 20-line generic
kernel built from three helpers (`find_rotation_start_by`, `rotate_cycle_by`,
and `rotate_index`) and wrapped for paths by the one-line `canonicalize_cycle`.

Today Kani, a bounded model checker, proves that kernel correct for cycles of
two, three, and four *distinct* `u8` node identifiers (roadmap 4.2.2).
Proptest, a property-testing library, samples cycles of up to ten single-letter
names. Nothing establishes the contract for every cycle length, nothing ever
exercises real multi-component paths, and nothing states in
algorithm-independent terms what a canonical cycle is.

Verus is a deductive verifier for Rust: it proves that code meets a stated
specification for *all* inputs, using the Z3 solver, without running the code.
Netsuke already pins Verus under `tools/verus/` and proves one kernel with it,
`verus/canonical_value/main.rs`. The formal-verification design accepts a
cycle-canonicalization kernel in principle, but none exists, and no
architectural decision record (ADR) has ratified either kernel.

This item *evaluates* that kernel. The deliverable is a decision backed by
evidence, and either verdict completes the item:

1. Add executable specification tests that check production canonicalization
   against an independent definition of "canonical", at large cycle lengths,
   with ties and with real camino paths. These land whatever the verdict.
2. Write a proof-specific mirror of the production kernel over `u64` node
   identifiers, outside Cargo, at `verus/cycle_canonicalization/main.rs`, and
   prove it meets the same specification for every cycle length.
3. Measure the proof against the Kani harnesses it parallels and apply the
   roadmap rule: accept only if the proof is narrower and cheaper than the Kani
   equivalent.
4. Record the verdict in an ADR. On acceptance, wire the kernel into
   `make verus`, guard it with ordinary-gate contract tests, and document it.
   On rejection, record the evidence and correct the documents that pre-judged
   acceptance.

A maintainer observes an accepted outcome by running `make install-verus` and
then `make verus`, and seeing two verifier result lines, each ending
`0 errors`. On either outcome, `make test` gains tests that fail if production
canonicalization stops matching the specification.

User-visible behaviour does not change. Cycle diagnostics, their order, their
localization, and their JSON (JavaScript Object Notation) form stay exactly as
they are. The design document's "lexicographically smallest node" is made
precise: the order is camino's `Utf8Path` ordering, which compares path
components rather than raw bytes, and ties go to the first occurrence.

What the proof will and will not establish must be stated precisely, here and
in every document this item touches. Verus proves properties of a `u64`
*mirror*. Production is related to that mirror by a mechanically checked
structural correspondence and by sampling tests against the same specification.
It is not proved directly.

## Context and orientation

### Glossary

- **ADR**: architectural decision record, a file under `docs/adrs/`.
- **CI**: continuous integration, the GitHub Actions workflows under
  `.github/`.
- **Kani**: a bounded model checker for Rust. It explores every execution of
  real compiled code up to a stated bound. Its proofs are *harnesses*, which
  are functions marked `#[kani::proof]`.
- **CBMC**: the C Bounded Model Checker, the solver back end Kani drives. Kani
  reports its per-harness solver time as "Verification Time".
- **Verus**: a deductive verifier for Rust. Code inside a `verus! { ... }`
  block has three modes:
  - `spec` functions are mathematical definitions;
  - `proof` functions are lemmas;
  - `exec` functions are ordinary compiled code annotated with `requires`
    (preconditions), `ensures` (postconditions), and loop `invariant` and
    `decreases` (termination) clauses.
- **vstd**: Verus's standard library of specifications and lemmas. `Seq<T>`
  is its mathematical sequence type; `v@` is the `Seq` view of a `Vec` `v`.
- **Trigger**: a pattern that tells Z3 when to instantiate a quantified fact.
  Wrong triggers make true lemmas fail to verify.
- **rlimit**: Verus's per-function solver resource limit. Raising it hides
  proofs that are too expensive.
- **Mirror**: proof-specific `exec` code in the Verus file that transcribes the
  production functions over `u64` values.
- **Mutant**: a deliberately broken copy of code or specification. A
  verification method is shown to be non-vacuous when it rejects the mutant.
- **Mutation patch**: a committed `git apply` patch under
  `docs/verification/mutations/` that seeds one such defect.
- **Proptest**: Rust property-based testing. It generates many inputs from a
  *strategy* and checks a property on each.
- **nextest**: `cargo-nextest`, the test runner `make test` uses.
- **Warm run**: a second run of a command after its caches and build artefacts
  exist, so that only the work under test is timed.

### The production kernel

`src/ir/cycle/support.rs` (163 lines) owns canonicalization. The relevant items
at `5c707ef33` are:

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

`compare_cycle_paths` calls `path_cmp`. Outside `cfg(kani)`, `path_cmp` is
`left.cmp(right)` on `Utf8Path`. Camino 1.2.6, the version in `Cargo.lock`,
implements both `Ord` and `Eq` for `Utf8Path` over `components()`. As a result,
`a/b` and `a//b` compare `Equal`, and `a/b` sorts before `a-b` even though `-`
(0x2D) is below `/` (0x2F) in byte order.

The cycle representation is *closed*. The detector (`cycle_from_stack` in
`src/ir/cycle/detector.rs`, lines 280-291) pushes the slice of the
depth-first-search stack from the revisited node onwards, then the revisited
node again. That closing element is spelled as the dependency named it, and the
stack lookup matches it by camino `Eq`. The input to canonicalization is
therefore `[n0, n1, ..., nk, n0']`, where `n0'` equals `n0` under camino `Eq`
but may differ byte-wise. The kernel treats everything except the last element
as the *interior*. It never checks closure: it drops the last element and
re-closes the rotated interior with a clone of its own first element.

Four behaviours are undocumented today:

- **Tie-breaking.** `find_rotation_start_by` replaces its candidate only on a
  strict `Less`, so the first occurrence of the minimum wins.
- **Short input.** A `debug_assert!` panics on fewer than two elements in debug
  builds. In release builds the guard returns the input unchanged.
- **Dead `None` branches.** Every `get` index is in bounds, so the `if let`
  fall-through branches never run. Production uses `get` to satisfy the
  `indexing_slicing` lint.
- **Redundant `pop`.** `rotate_cycle_by` reads only indices below `len`, so the
  `pop` cannot affect the result.

The only production caller is `back_edge_result` (`src/ir/cycle/detector.rs`,
line 198), in `CycleSearch::Path` mode. The public surface is `analyse`
(`src/ir/cycle/mod.rs`), whose `CycleDetectionReport.cycle` flows into
`IrGenError::CircularDependency` and the human and JSON diagnostics. The doc
comment on `canonicalize_cycle` (`support.rs`, line 84) and a test comment
(`src/ir/cycle/property_tests/mod.rs`, line 150) also say "lexicographically".

### Existing verification of the kernel

`src/ir/cycle/verification.rs` holds 11 of the repository's 17 Kani harnesses.
Four concern canonicalization; their helpers are in
`src/ir/cycle/verification/support.rs` (289 lines):

- `canonicalize_two_node_cycle_is_canonical`,
  `canonicalize_three_node_cycle_is_canonical`, and
  `canonicalize_four_node_cycle_is_canonical` drive the production
  `canonicalize_cycle_by` over symbolic `u8` identifiers. Each uses
  `kani::assume` to keep the identifiers **distinct** (verification support,
  lines 52, 63, and 75-82). They assert length preservation, closure, interior
  multiset preservation, a smallest first node, and that the output interior is
  a rotation of the input interior. These three are the **Kani equivalent** of
  this item.
- `canonicalize_path_wrapper_matches_u8_kernel_for_two_nodes` checks that the
  `Utf8PathBuf` wrapper agrees with the `u8` kernel on `[a, b, a]` and
  `[b, a, b]`. No Verus obligation mirrors it, so it is excluded from the
  comparison.

`docs/execplans/4-2-2-kani-harnesses-for-cycle-canonicalization.md` recorded
these Kani verification times on its implementing host:

| Harness          | Verification time |
| ---------------- | ----------------- |
| N=2              | 4.577 s           |
| N=3              | 7.602 s           |
| N=4              | 11.815 s          |
| Three-kernel sum | 23.994 s          |

Direct `Utf8PathBuf` proofs at three nodes exceeded an 8 GiB memory cap, which
is why that boundary stops at the `u8` kernel
([ADR-0047](../adrs/adr-0047-bound-kani-intermediate-representation-ir-harnesses-to-small-n.md)).
Four mutation patches,
`docs/verification/mutations/ir__cycle__verification__canonicalize_*.patch`,
seed a `Less`-to-`Greater` flip, a lost closing push, a wrong wrap in
`rotate_index`, and a reversed path comparator.

`src/ir/cycle/property_tests/mod.rs` (364 lines) holds four canonicalization
properties over 2-10 unique single-letter names. No existing test asserts that
`analyse` reports a cycle with a distinct interior, and none generates
multi-component paths or duplicates.

### Existing Verus infrastructure

- `tools/verus/VERSION` pins Verus `0.2026.10.04.426d8b0`, and
  `tools/verus/SHA256SUMS` pins its x86-64 Linux archive. The `Makefile` pins
  `rust-prover-tools` at `592558f7183b554fc749696246984c5d7300d029`
  (`PROVER_TOOLS_SOURCE`). `make install-verus` and `make verus` delegate to its
  `verus install` and `verus run` subcommands. The default is:

  ```make
  VERUS_FLAGS ?= --proof-file verus/canonical_value/main.rs
  ```

  Repeating `--proof-file` runs an ordered batch that stops at the first
  failure. `verus run` also takes a repeatable `--extra-arg=<value>` that is
  appended to the Verus command line. It passes no `--edition` flag.
- `verus/canonical_value/main.rs` (399 lines) is the precedent kernel. It is a
  spec-level refinement proof over `Seq<u64>`. Its `//!` comment states what it
  proves and what it does not, and it ends with the `fn main() {}` Verus
  requires. Its recorded result is
  `verification results:: 16 verified, 0 errors`; no run time was recorded.
- Verus is outside `make test`, `make lint`, `make check-fmt`, `make all`,
  `make kani-check`, `make kani-full`, and `make formal-pr`. Since #1138 it
  runs in CI through `.github/workflows/proofs.yml`:
  - the job installs Verus and Z3 only through the shared
    `leynos/shared-actions` `install-prover-tools` action, which reads
    `tools/verus/VERSION`, then runs `make verus`;
  - it triggers on pull requests and pushes to `main` that touch `verus/**`,
    `tools/verus/**`, the `Makefile`, or the workflow itself, weekly on
    Wednesday at 05:17 UTC, and on manual dispatch;
  - it has a 30-minute timeout.

  Because `make verus` runs only the files in `VERUS_FLAGS`, a kernel missing
  from that default is never checked in CI. Because of the path filters, a
  change to production code that a kernel mirrors does not re-run the proof
  until the weekly run unless that source path is added to both the
  `pull_request` and `push` filters.
- One exception matters. `tests/prover_tools_verus_e2e_tests.rs` is compiled
  and run by `make test`. Its cases skip when the pinned install under
  `.verus/` is absent, but they run real Verus through `make verus` when it is
  present (`tests/prover_tools_verus_e2e_tests/support.rs`, lines 33-61).
  Installing Verus in EP-M0 therefore puts real Verus runs inside `make test`
  *on that host*. `make test-verus-e2e` sets `NETSUKE_VERUS_E2E_STRICT=1` to
  turn the skip into a failure.
- Three tests pin the `make verus` default:
  - `tests/prover_tools_make_tests.rs`, lines 197-217
    (`verus_uses_the_canonical_value_proof_by_default`);
  - `scripts/tests/test_prover_tools_integration.py`, line 149, a substring
    check;
  - `tests/prover_tools_verus_e2e_tests.rs`, lines 50-69, which asserts
    `verification_runs(&output) == 1` and that the output contains `0 errors`.
- Docs that describe the default: `docs/developers-guide.md` lines 6966-6968,
  6991-6992, and 7446-7452, and
  `docs/formal-verification-methods-in-netsuke.md` lines 66-67, 106-110,
  172-214, 245-270, and footnote 7.
- Nothing enforces the canonical-value kernel's "no `assume`" claim, its
  400-line cap, or its doc comments. The 6.1.2 plan records that the kernel
  reached 414 lines unnoticed.
- `.gitignore` excludes `.verus/` (line 21). Verus build output lives inside
  `.verus/<version>/`, and `tests/env_access_suppressions/roots.rs` skips it.

On the drafting host, Verus is not installed and `/usr/bin/time` does not
exist. Kani 0.67.0 is installed under `~/.kani`.

### Reusable helpers (abstraction sweep)

- `tests/support/makefile.rs` provides `repo_root()` (capability-scoped
  `cap_std::fs_utf8::Dir` from `env!("CARGO_MANIFEST_DIR")`) and
  `read_repo_file`. Integration tests reuse them with `#[path]`. The new
  contract test reuses them and adds no file-access helper.
- `src/stdlib/canonical_value/tests/properties.rs` (lines 72-96) defines the
  Proptest pattern for a Verus companion suite. A `law_runner` uses a free seed
  for the law itself. A `coverage_runner` uses a fixed seed (`RngSeed::Fixed`),
  tallies generated classes into a `BTreeMap`, and requires each class to be
  non-zero. The new property suite copies that pattern, with its own small
  runners. Sharing across `src/stdlib` and `src/ir` would add a cross-module
  test dependency for about 20 lines.
- `src/ir/cycle/property_tests/participation_tests.rs` (359 lines) defines
  `participation_graph_strategy`, which generates graphs with every dependency
  kind, multi-output aliases, and back arcs. VC-DISTINCT reuses it.
- `syn` (features `full` and `visit`) is a dev-dependency, but without `quote`
  or `extra-traits` it cannot print an item back to text. Adding either would
  change `Cargo.toml`. Function extraction therefore uses a small
  brace-matching scanner over `str::char_indices`, owned by the new contract
  test.
- No existing test snapshots or hashes production source text.

### Size survey and historical budgets

Comparable plans:

- 6.1.2 (`docs/execplans/6-1-2-canonical-value-key-and-equality-relation.md`)
  set no numeric limits. Its EP-M2 landed as 15 files, +2280/-1, and its kernel
  had to be trimmed back under 400 lines after a documentation repair.
- 4.4.2 (`docs/execplans/4-4-2-document-cycle-detection-scope.md`) started at
  20 files. Its own inventory needed about 30, so revision 2 raised the limit
  to 35 files and 1,200 net lines before approval. That was missed scope at
  drafting, not growth during execution.
- 4.2.2 allowed 8 files and never raised the limit. A blocked stage was
  resolved by narrowing the proof boundary.
- 4.1.3 allowed 4 files and deferred any real proof to this item.

Lessons applied here:

- Inventory every pinned test and every doc that names the Verus layout before
  budgeting.
- Budget the kernel's comment block inside the 400-line cap.
- Expect the specification tests to run larger than a first guess. The panel
  estimated 250-300 lines for the property suite and 220-260 for the contract
  test.

Expected non-ExecPlan changes, on the acceptance path:

| Category      | Path                                                               | Estimate (net lines) |
| ------------- | ------------------------------------------------------------------ | -------------------- |
| Code          | `verus/cycle_canonicalization/main.rs` (new)                       | 330-380              |
| Code          | `src/ir/cycle/property_tests/canonical_spec_tests.rs` (new)        | 280                  |
| Code          | `src/ir/cycle/property_tests/mod.rs` (`mod` line; comment fix)     | 2                    |
| Code          | `src/ir/cycle/property_tests/participation_tests.rs` (VC-DISTINCT) | 35                   |
| Code          | `src/ir/cycle/support.rs` (doc-comment wording only)               | 1                    |
| Code          | `tests/verus_kernel_contract_tests.rs` (new)                       | 260                  |
| Code          | `tests/verus_kernel_contract_tests/correspondence.rs` (new)        | 140                  |
| Code          | `tests/prover_tools_make_tests.rs`                                 | 12                   |
| Code          | `tests/prover_tools_verus_e2e_tests.rs`                            | 15                   |
| Code          | `scripts/tests/test_prover_tools_integration.py`                   | 6                    |
| Configuration | `Makefile` (`VERUS_FLAGS` default and comment)                     | 6                    |
| Documentation | ADR-0082 (new)                                                     | 170                  |
| Documentation | `docs/developers-guide.md`                                         | 80                   |
| Documentation | `docs/formal-verification-methods-in-netsuke.md`                   | 40                   |
| Documentation | `docs/netsuke-design.md`                                           | 15                   |
| Documentation | `docs/repository-layout.md`                                        | 20                   |
| Documentation | `docs/contents.md`                                                 | 4                    |
| Documentation | `docs/roadmap.md`                                                  | 10                   |

Revision 3 adds these, on the acceptance path only:

| Category      | Path                                                            | Estimate (net lines) |
| ------------- | --------------------------------------------------------------- | -------------------- |
| Configuration | `.github/workflows/proofs.yml` (two path entries; comment)      | 3                    |
| Code          | `src/ir/cycle/verification.rs` (two harnesses removed)          | -16                  |
| Code          | `src/ir/cycle/verification/support.rs` (two builders removed)   | -40                  |
| Code          | `docs/verification/mutations/..._three_node_...patch` (deleted) | -13                  |
| Code          | `docs/verification/mutations/..._four_node_...patch` (deleted)  | -14                  |
| Documentation | `docs/adrs/adr-0047-...md` (amendment banner and entry)         | 12                   |

The mutation patches are executable test inputs, so they count as code even
though they live under `docs/`. Retirement also removes two rows from the
developers' guide harness table and changes the harness counts in the
formal-verification document. Those edits are included in the documentation
estimates above.

That is 24 files: about 1,070 net code lines, about 9 configuration lines, and
about 350 documentation lines. The rejection path deletes the kernel and the
correspondence file and skips the `Makefile`, workflow, and retirement changes,
so it is smaller in every category.

If `make spelling` regenerates `typos.toml`, the regenerated file is not
committed here. `AGENTS.md` requires a separate base pull request for it.

## Signposts: documentation and skills

Read these first:

- `AGENTS.md`, for gates, the 400-line cap, doc-comment rules, en-GB Oxford
  spelling, and the testing rules (no in-process environment mutation; every
  assertion carries a message; `rstest` fixtures; no `.expect()` outside test
  functions).
- `docs/formal-verification-methods-in-netsuke.md`: "Current repository
  state", "Kani for the IR core", and "Optional Verus proof kernel".
- `docs/developers-guide.md`: the `make verus` description (around line 6966),
  "Kani harness inventory" (around line 7190), and the Verus policy paragraph
  (around line 7446).
- `docs/netsuke-design.md` §5.3, step 4 "Graph Validation" (around line 2622).
- [ADR-0047](../adrs/adr-0047-bound-kani-intermediate-representation-ir-harnesses-to-small-n.md)
  and
  [ADR-0068](../adrs/adr-0068-detect-cycles-across-all-prerequisite-kinds.md).
- The 4.2.2 and 6.1.2 ExecPlans, as historical evidence only. Do not edit
  them.
- `verus/canonical_value/main.rs`, for house style.
- `docs/rust-testing-with-rstest-fixtures.md`,
  `docs/reliable-testing-in-rust-via-dependency-injection.md`, and
  `docs/rust-doctest-dry-guide.md`.
- `docs/rstest-bdd-users-guide.md`. It is not used here, because no
  user-visible behaviour changes (D9).
- `docs/ortho-config-users-guide.md`. It is not used here, because no
  configuration surface changes (D10).
- `docs/documentation-style-guide.md`, for ADR sections, amendment banners,
  and Markdown rules.

Skills to load during implementation:

- `rust-router`, then `verus` for proof engineering: triggers,
  `assert ... by`, loop isolation, and the review checklist.
- `proptest`, for the specification and rank properties.
- `rust-unit-testing`, for `rstest`, `googletest`, and `pretty_assertions`.
- `kani`, only to re-measure the baseline.
- `hexagonal-architecture`, as a boundary check. The kernel is pure domain
  logic with no ports. Neither the proof nor its tests may pull adapters
  (filesystem, process, `HashMap` storage) into the kernel's boundary.
- `codegraph-mcp`, for callers and impact on `src/ir/cycle/support.rs`.
- `execplans`, to keep this document current.
- `en-gb-oxendict-style`, for prose.

## Conformance basis

Upstream artefacts, all at `origin/main` `5c707ef33`:

- `docs/roadmap.md` §4.4, item 4.4.3. Its prerequisites, 4.2.2 and 4.1.3, are
  complete.
  - `RM-443-OUT`: keep the proof outside Cargo.
  - `RM-443-MODEL`: use proof-specific model types rather than production
    `HashMap` structures.
  - `RM-443-ACC`: accept the proof only if it remains narrower and cheaper
    than the Kani equivalent.
- `docs/formal-verification-methods-in-netsuke.md` §"Optional Verus proof
  kernel":
  - `FV-KERNEL`: proof-kernel-only, not a production subsystem or routine
    gate.
  - `FV-CLAUSES`: length preserved, the cycle stays closed, the interior
    multiset is preserved, and the start node is stable under the ordering
    rule.
  - `FV-GATES`: Verus stays outside `make test`, `make lint`,
    `make check-fmt`, `make all`, `make kani-check`, `make kani-full`, and
    `make formal-pr`.
  - `FV-CI` (as amended by #1138): CI runs every proof the repository holds,
    through `make verus` in `.github/workflows/proofs.yml`, installing Verus
    only through the shared `install-prover-tools` action.
- User direction, 2026-11-15:
  - `UD-D4`: retire the three- and four-node Kani canonicalization harnesses,
    because Verus now runs in CI.
  - `UD-FLAGS`: a new kernel must be added to `make verus` (to `VERUS_FLAGS`,
    or by running every proof under `verus/`), or it passes only when run by
    hand.
  - `UD-PATHS`: production source paths that a kernel covers must be added to
    both the `pull_request` and `push` path filters of `proofs.yml`.
  - `UD-INSTALL`: Verus is installed only from the `tools/verus/VERSION` pin,
    through `install-prover-tools` in CI and through `rust-prover-tools`
    (`make install-verus`, `make verus`) locally.
- `docs/execplans/4-1-3-record-phase-1-scope-boundary-for-verus-and-stateright.md`:
  - `SB-CARGO`: keep Verus out of production Cargo.
  - `SB-FIRST`: the first proof focuses on cycle canonicalization.
- [ADR-0047](../adrs/adr-0047-bound-kani-intermediate-representation-ir-harnesses-to-small-n.md)
  (`ADR47-U8`): Kani proves the rotation kernel over `u8` for N=2..4.
- [ADR-0068](../adrs/adr-0068-detect-cycles-across-all-prerequisite-kinds.md)
  (`ADR68-DET`): cycle paths are deterministic.
- `docs/netsuke-design.md` §5.3 step 4 (`DD-ROT`): the cycle is rotated so its
  smallest node comes first, ensuring deterministic messages.
- `AGENTS.md`:
  - `AG-400`: no file over 400 lines;
  - `AG-DOC`: `//!` on every module; `///` on every non-test function, and in
    Verus files on every `fn`, `spec fn`, and `proof fn`;
  - `AG-VER`: Verus for selected lemmas only, with no artificial obligations;
  - `AG-ADR`: substantive decisions go in an ADR.

No Terms of Reference document governs this item.

Baseline-to-target gaps found by reconnaissance:

- `GAP-ORDER`: the design document says "lexicographically smallest". The
  real order is component-wise, and no test generates multi-component paths.
- `GAP-TIE`: the tie-break rule is unstated and unchecked.
- `GAP-ALLN`: no evidence covers cycle lengths above 10, or the
  rotation-invariance property beyond sampling.
- `GAP-HYGIENE`: the Verus kernel's trust and size claims are unenforced.
- `GAP-STALE-REF`: live docs cite `src/ir/cycle/mod.rs` for code that lives in
  `src/ir/cycle/support.rs`.
- `GAP-NO-ADR`: no ADR ratifies Verus kernel acceptance.

Trace chains:

```plaintext
GAP-ORDER, AXM-ORD -> EP-M1 -> VC-RANK; EP-M4 -> design doc and support.rs comment
GAP-TIE -> EP-M1 -> VC-TIES, VC-SPEC-ORACLE; EP-M2 -> C5 in the kernel
GAP-ALLN, FV-CLAUSES, DD-ROT, ADR68-DET -> EP-M1 -> VC-SPEC-ORACLE (sampled, N<=64)
  -> EP-M2 -> T-CANON, C1-C8 (proved of the mirror, every N)
RM-443-OUT, SB-CARGO, FV-KERNEL, AG-400, AG-DOC -> EP-M1/EP-M3 -> VC-HYGIENE
RM-443-MODEL, ADR47-U8 -> EP-M2 -> u64 mirror; VC-HYGIENE forbidden-type check
REP (mirror ~ production) -> EP-M3 -> VC-CORRESPOND
RM-443-ACC -> EP-M0 baseline + EP-M2 measurement -> gate G1 -> EP-M4 or EP-M4R
FV-GATES, FV-CI, UD-FLAGS, UD-PATHS -> EP-M3 -> VC-WIRING (Makefile default and
  proofs.yml filters checked by VC-HYGIENE); CI Proofs job green on the PR
UD-D4, ADR47-U8 -> EP-M3 -> N=3/N=4 harnesses and patches removed; EP-M4 ->
  ADR-0047 amendment banner
UD-INSTALL -> Constraints; EP-M0 installs through make install-verus only
AG-VER, SB-FIRST -> Verification plan "Where Verus is warranted"
GAP-HYGIENE -> EP-M1 -> VC-HYGIENE over verus/canonical_value/main.rs
GAP-STALE-REF, GAP-NO-ADR, AG-ADR -> EP-M4 or EP-M4R -> ADR-0082, doc edits
```

## Constraints

- Implementation begins only after the user explicitly approves this plan.
- Keep the Verus proof outside Cargo (`RM-443-OUT`, `SB-CARGO`). Do not add
  `vstd`, `verus_builtin`, or any Verus crate to any `Cargo.toml`. There is no
  workspace member and no `#[path]` import between `verus/` and `src/`.
- Use proof-specific model types (`RM-443-MODEL`). The kernel uses `u64` values
  in `Vec<u64>`, `&[u64]`, and `Seq<u64>`. It must not mention `HashMap`,
  `IrHashMap`, `BuildGraph`, `BuildEdge`, `Utf8PathBuf`, `camino`, `minijinja`,
  or any filesystem or process API.
- Do not add Verus to `make test`, `make lint`, `make check-fmt`, `make all`,
  `make kani-check`, `make kani-full`, or `make formal-pr` (`FV-GATES`). In CI,
  Verus runs only in `.github/workflows/proofs.yml` (`FV-CI`). The one
  permitted workflow edit adds `src/ir/cycle/support.rs` to that file's
  `pull_request` and `push` path filters (`UD-PATHS`). Plain Rust tests that
  read the Verus source as text may run in `make test`. The pre-existing e2e
  lane's behaviour on hosts with Verus installed is unchanged in kind; it now
  proves two files instead of one.
- Install Verus only from the `tools/verus/VERSION` pin: locally through
  `make install-verus` and `make verus` (`rust-prover-tools`), and in CI through
  `install-prover-tools` (`UD-INSTALL`). No other build, cache, or download of
  Verus may supply evidence. The panel's prototype ran on an unpinned cached
  build and is feasibility evidence only.
- Do not change production behaviour. The only production edit permitted is
  the doc-comment wording on `canonicalize_cycle` (`support.rs`, line 84),
  which is outside the four mirrored functions. Any other production edit is an
  escalation (see `Tolerances`).
- On the acceptance path only, retire exactly the two Kani harnesses
  `canonicalize_three_node_cycle_is_canonical` and
  `canonicalize_four_node_cycle_is_canonical` (`UD-D4`, D4). Also remove their
  now-unused builders, `closed_three_node_id_cycle` and
  `closed_four_node_id_cycle`, with any helper only they use, and their two
  mutation patches, because `tests/kani_mutation_evidence_tests.rs` requires
  every patch to name a live harness. Keep the two-node kernel harness, the
  path-wrapper harness, every other Kani harness and patch, and every Proptest
  property. On the rejection path, retire nothing.
- Verus files contain no `assume`, `admit`, `#[verifier::external_body]`,
  `#[verifier::external]`, `external_fn_specification`, `assume_specification`,
  `external_type_specification`, `#[verifier::trusted]`, `uninterp`, `axiom`
  function, `#[verifier::exec_allows_no_decreases_clause]`, `rlimit` override,
  or `unsafe`. The trusted base is Verus, Z3, and `vstd`. Every `exec` loop has
  a `decreases` clause.
- In the mirror, ghost code (`assert`, `let ghost`, lemma calls) appears only
  inside `proof { ... }` blocks or in `requires`, `ensures`, `invariant`, and
  `decreases` clauses. This keeps VC-CORRESPOND's normalization simple.
- Every file stays at or below 400 lines (`AG-400`), including the Verus kernel
  and its comments, without splitting the kernel. `AG-DOC` applies; test
  functions carry no `///`.
- No new dependencies. `proptest`, `rstest`, `googletest`,
  `pretty_assertions`, `cap-std`, `camino`, and `anyhow` already serve the
  tests.
- No in-process environment mutation. Tests find the repository through
  `env!("CARGO_MANIFEST_DIR")`, a compile-time constant, via
  `tests/support/makefile.rs`.
- Never compile a mutated crate under `/tmp`; `/tmp` is not a build target.
  Production mutants are applied with `git apply` in the worktree on a clean,
  committed tree, run, reverted with `git apply -R`, and confirmed with
  `git diff --exit-code`. Never do this while a gate batch is running. Verus
  mutants are single files and may live under `/tmp`.
- Completed ExecPlans (4.1.3, 4.2.2, 4.4.2, 6.1.2) stay untouched. Accepted ADRs
  are not rewritten. On the acceptance path, ADR-0047 gains an
  `Amended by ADR-0082 (date)` banner and an `## Amendments` entry, because
  retiring N=3 and N=4 changes its N=2..4 boundary (D12).
- Transient measurements (timings, memory peaks, host details) go in the ADR's
  evidence section and this plan, never in the developers' guide.
- The ADR serial is re-derived across every remote branch immediately before
  the ADR commit and again before the push. `ADR-0082` is provisional.
- Prose uses en-GB Oxford spelling (`-ize`, `-yse`, `-our`).

## Tolerances (exception triggers)

Counting rules:

- Comparison base: `B=$(git merge-base origin/main HEAD)`, counted after each
  commit, so untracked files are never missed.
- This ExecPlan is excluded from every count, including when it is edited in
  the same commit as other files.
- Limits are **net** lines (additions minus deletions) per category, assigned
  whole-file:
  - Code: `verus/**`, `src/**`, `tests/**`, `scripts/**`.
  - Configuration: `Makefile`, `Cargo.toml`, `Cargo.lock`, `.github/**`,
    `typos*.toml`.
  - Documentation: `docs/**` except `docs/execplans/**`, and `README*`.
- There are no generated-file exclusions.

Counting command:

```bash
B=$(git merge-base origin/main HEAD)
git diff --numstat "$B" HEAD -- . ':(exclude)docs/execplans/**' \
  | awk '
      { net = $1 - $2 }
      $3 ~ /^(verus|src|tests|scripts)\// { code += net; files++; next }
      $3 ~ /^(Makefile|Cargo\.(toml|lock)|typos.*\.toml)$/ || $3 ~ /^\.github\// { conf += net; files++; next }
      $3 ~ /^docs\// || $3 ~ /^README/ { docs += net; files++; next }
      { other += net; files++ }
      END { printf "files=%d code=%d config=%d docs=%d other=%d\n", files, code, conf, docs, other }'
```

Budgets: the estimates from `Context and orientation` plus about 15-20%
contingency, informed by 4.4.2's inventory overrun and the panel's line
estimates. Spare capacity in one category never offsets another.

- Files: escalate above 28 non-ExecPlan files (estimate 24). Any `other`
  category file is itself an escalation. Deleted files count.
- Code: escalate above 1,350 net lines (estimate about 1,150).
- Kernel: escalate if `verus/cycle_canonicalization/main.rs` exceeds 400 lines.
  Splitting it into modules is not an allowed workaround; that would be
  evidence against "narrower".
- Configuration: escalate above 14 net lines, or on any change to
  `Cargo.toml`, `Cargo.lock`, or `.github/**` other than the two path-filter
  lines in `.github/workflows/proofs.yml` (estimate 8: 6 in the `Makefile`, 2 in
  `proofs.yml`).
- Documentation: escalate above 420 net lines (estimate about 340).
- Interface: escalate on any production signature, visibility, or behaviour
  change, or any `make verus` change beyond its default `VERUS_FLAGS`.
- Dependencies: escalate on any new crate, Verus version bump, or
  `rust-prover-tools` pin bump.
- Proof soundness: escalate if a proof needs any construct forbidden under
  `Constraints`, or a precondition that excludes inputs a production caller can
  supply.
- Proof effort: escalate if Verus still reports errors or timeouts after 6
  focused iterations on one obligation at the default `rlimit`.
- Mirror fidelity: escalate if the mirror needs a divergence beyond the
  allowance in R1, because VC-CORRESPOND would then need a wider rewrite table.
- Proof cost: G1 decides the outcome. Do not tune the proof to pass G1 by
  deleting obligations from `Verification plan`.
- Production defect: escalate if Verus or the new tests show production
  contradicting `DD-ROT` or the 4.2.2 Kani claims.
- Real-verifier test time: escalate if the e2e default case, now running both
  kernels, takes over 150 s. The nextest slow-test limit is 300 s
  (`.config/nextest.toml`, line 25).
- Iterations: escalate if a gate still fails after 3 fix attempts on the same
  failure.
- Time: escalate if a milestone exceeds 10 hours of active work.
- Ambiguity: escalate if a reviewer or the user disputes the meaning of
  "narrower" or "cheaper" beyond the `Acceptance rule` below.

## Risks

Each risk gives severity (impact if it occurs), likelihood, and mitigation.

- R1. Severity medium, likelihood certain: the production let-chain is
  unsupported. Verus rejects `if let ... && cond` under the default edition,
  and with `--edition=2024` it reports "does not yet support … let
  expressions". The panel confirmed both on an older Verus build.
  - Mitigation: one divergence is allowed. The let-chain becomes a nested
    `if let (Some(candidate), Some(current)) = (cycle.get(index),
    cycle.get(start)) { if *candidate < *current { start = index; } }`, which
    has the same control flow.
  - Keep `get`, `first().cloned()`, `Vec::with_capacity`, `push`, and `pop`
    verbatim. The panel confirmed `vstd` specifies them. Replacing `get` with
    indexing is not permitted.
  - Ghost-only additions do not count as divergences.
  - EP-M0 confirms all of this on the pinned version with a short probe file.
- R2. Severity medium, likelihood medium: trigger and loop-isolation trouble.
  Verus isolates each loop by default, so the invariants must restate the
  bounds and the facts the loop uses.
  - Mitigation: state rotation index-wise with an explicit case split rather
    than `%`. Scope helper facts with `assert ... by`. Restate needed facts in
    `invariant` clauses rather than using `#[verifier::loop_isolation(false)]`.
    That attribute is sound and permitted, but avoided.
  - No `broadcast use` is expected; the panel's prototype needed none.
- R3. Severity medium, likelihood low: the kernel may exceed 400 lines.
  - Mitigation: the panel's prototype was 253 lines without a `//!` block, and
    the estimate with house-style comments is 335-380. Keep the `//!` block to
    about 40 lines, let C1-C5 share one lemma, and move rationale to the ADR.
- R4. Severity medium, likelihood medium: measurement noise, because other
  agents compile on this host.
  - Mitigation: take the median of 5 warm runs, use CPU time (user plus
    system), require a margin (K1), and measure Kani and Verus in the same
    session.
- R5. Severity high, likelihood medium: the mirror may drift from production.
  - Mitigation: VC-CORRESPOND fails `make test` whenever either side changes
    without a matching change on the other. It cannot prove that someone
    re-ran Verus after editing the mirror. The developers' guide states that
    obligation, and ADR-0082 names it as the weakest link.
- R6. Severity low, likelihood low: installing Verus writes a large `.verus/`
  tree.
  - Mitigation: it is ignored. Confirm `git status` is clean after install.
- R7. Severity medium, likelihood medium: the ADR serial collides with an open
  branch.
  - Mitigation: Concrete step M4.1, run twice.
- R8. Severity low, likelihood certain: the `make verus` default change touches
  three pinned tests, two outside `make test`.
  - Mitigation: EP-M3 runs `make test-prover-tools-integration` and
    `make test-verus-e2e` explicitly.
- R9. Severity low, likelihood low: the length precondition is read as an
  assumption.
  - Mitigation: it is a `requires` clause discharged by AXM-ALLOC. VC-PREMISE
    records why every production call satisfies it.
- R10. Severity medium, likelihood certain on the implementing host: real
  Verus enters `make test` through the e2e lane once installed.
  - Mitigation: record the case's duration in EP-M3. The 150 s tolerance
    keeps it well under the 300 s slow-test limit.
- R11. Severity medium, likelihood medium: rejection. The evaluation can
  legitimately fail G1.
  - Mitigation: EP-M4R makes rejection a complete outcome.
- R12. Severity low, likelihood low: the pinned Verus differs from the panel's
  older build.
  - Mitigation: EP-M0's probe file and canonical-value run catch version
    differences before any proof work. Only pinned-Verus output counts as
    evidence (`UD-INSTALL`).
- R13. Severity medium, likelihood low: retiring N=3 and N=4 removes bounded
  checks of the *compiled* production kernel at those sizes.
  - Mitigation: the retirement happens only after G1 acceptance. The two-node
    harness stays, and it must kill both retired patches' faults before they
    are deleted (EP-M3 step 4). VC-CORRESPOND ties the proved mirror to
    production. VC-SPEC-ORACLE samples production at every length up to 64.
- R14. Severity medium, likelihood medium: the CI Proofs job does not run
  when only `src/ir/cycle/support.rs` changes.
  - Mitigation: EP-M3 adds the path to both filters (`UD-PATHS`), and
    VC-HYGIENE fails `make test` if the filter is ever dropped (D17).

## Verification plan

This section was written before any implementation and drives its design. Lemma
names are hypotheses about the eventual proof structure. Implementation may
merge, split, or drop lemmas, but never obligations, and records every such
change in `Decision log`. No theorem below counts as proved until EP-M2 records
Verus output for it.

During review, a panel member wrote a prototype of the whole kernel in scratch
against an older Verus (`0.2026.03.17.a96bad0`, toolchain 1.94.0). It verified
with `17 verified, 0 errors` in about 1.1 s, with a peak of about 258 MB,
without forbidden constructs. That is feasibility evidence only. It is not
committed, not on the pinned version, and not a substitute for EP-M2.

### Where Verus is warranted and where it is not

What Verus can add on inputs production actually reaches is modest, and
rejection is a live outcome.

The warranted obligation is T-CANON together with C7 (rotation invariance) and
C8 (completeness) for **every** cycle length. Kani checks per-input clauses for
N=2..4 and never states C7 or C8. Proptest samples C7 up to N=10. No other
technique gives "for every N" for the property that makes diagnostics
deterministic.

C5 (first-occurrence tie-break) makes the generic kernel's contract total. It
is **not** a production gap: AXM-DET says production interiors never contain
two nodes that compare equal. C5 and the M2 mutant matter for the generic
helper's documented rule, not for observable output.

Cheaper alternatives, considered:

- Drop `kani::assume(distinct)` from the Kani harnesses. This covers ties at
  N=2..4 for about 10 changed lines, but still says nothing for N above 4.
  Rejected as a substitute: it leaves `GAP-ALLN` open. It remains a valid
  follow-up if the kernel is rejected.
- VC-SPEC-ORACLE alone. This samples every N up to 64 against the
  specification at no proof cost, and it is adopted regardless of the verdict.
  It does not establish "for every N".

Verus is **not** used for the following:

- The detector's traversal. It is `HashMap`-driven, which `RM-443-MODEL`
  excludes, and is already covered by 7 Kani harnesses and the ADR-0068
  property, behavioural, and end-to-end tests.
- The path wrapper and camino's ordering. These are third-party behaviour
  (AXM-ORD), and VC-RANK tests the glue.
- The `debug_assert!` panic, which is a debug-only diagnostic (D11).
- Diagnostics formatting, localization, JSON, and `make verus` wiring. These
  are integration plumbing covered by existing tests and VC-WIRING.

### Abstract model

The model works over `u64` with natural order `<=`, standing for any lawful
total order (LEM-RANK). The definitions are independent of the algorithm:

```rust
/// The interior of a closed cycle: every element except the closing repeat.
spec fn interior(c: Seq<u64>) -> Seq<u64> { c.drop_last() }

/// `s` rotated left by `i`: elements `i..n` followed by `0..i`.
spec fn rotation(s: Seq<u64>, i: int) -> Seq<u64> {
    s.subrange(i, s.len() as int) + s.subrange(0, i)
}

/// `i` is the first index holding a minimal element of `s`.
spec fn is_first_min(s: Seq<u64>, i: int) -> bool {
    0 <= i < s.len()
    && (forall |j: int| 0 <= j < s.len() ==> s[i] <= #[trigger] s[j])
    && (forall |j: int| 0 <= j < i ==> s[i] < #[trigger] s[j])
}

/// The canonical closed form of interior `s` started at `i`.
spec fn canonical_form(s: Seq<u64>, i: int) -> Seq<u64> { rotation(s, i).push(s[i]) }

/// `s` has exactly one minimal element.
spec fn has_unique_min(s: Seq<u64>) -> bool {
    exists |i: int| is_first_min(s, i)
        && forall |j: int| 0 <= j < s.len() && j != i ==> s[i] < #[trigger] s[j]
}
```

### Proof-specific mirror and its representation relationship

The mirror in `verus/cycle_canonicalization/main.rs` is `exec` code that
transcribes the four production functions over `u64`, with `<` standing for
`compare(..) == Ordering::Less`. The signatures and contracts are provisional:

- `const fn rotate_index_model(start: usize, offset: usize, len: usize) -> (r: usize)`
  requires `start < len`, `offset < len`, and `len <= usize::MAX / 2 + 1`. It
  ensures `r == start + offset` when that is below `len`, and
  `r == start + offset - len` otherwise. LEM-IDX folds into this contract.
- `fn find_rotation_start_model(cycle: &[u64], len: usize) -> (start: usize)`
  requires `1 <= len <= cycle.len()`. It ensures
  `is_first_min(cycle@.subrange(0, len as int), start as int)`.
- `fn rotate_cycle_model(cycle: &[u64], start: usize, len: usize) -> (out: Vec<u64>)`
  requires `start < len <= cycle.len()` and `len <= usize::MAX / 2`. It ensures
  `out@ == canonical_form(cycle@.subrange(0, len as int), start as int)`. The
  `<=` (not `==`) between `len` and `cycle.len()` is deliberate: it makes the
  `pop` mutant M6 verify, which is correct because that mutant is equivalent.
  With `==`, M6 would fail and look like a false kill.
- `fn canonicalize_cycle_model(cycle: Vec<u64>) -> (out: Vec<u64>)` requires
  `cycle.len() <= usize::MAX / 2` (`MAX_MODEL_LEN`). This bound is exact:
  `start + offset <= 2 * len - 2` must fit in `usize`, and
  `usize::MAX / 2 == isize::MAX`.

REP, the representation relationship: the four mirror bodies equal the four
production bodies token-for-token, after removing ghost code and applying a
closed rewrite table:

| Production                                                 | Mirror                                      |
| ---------------------------------------------------------- | ------------------------------------------- |
| generic `T`, `compare` parameter                           | `u64`, no parameter                         |
| `&& compare(candidate, current) == Ordering::Less {` … `}` | `{ if *candidate < *current {` … `} }`      |
| `find_rotation_start_by(&cycle, len, compare)`             | `find_rotation_start_model(&cycle, len)`    |
| `rotate_cycle_by` / `rotate_index` names                   | `rotate_cycle_model` / `rotate_index_model` |

VC-CORRESPOND checks REP mechanically.

### Axioms (trusted, not proved here)

- AXM-VERUS: Verus `0.2026.10.04.426d8b0`, its bundled Z3, and `vstd` are sound
  for the constructs used.
- AXM-ORD: `Utf8Path: Ord` in camino 1.2.6 is a lawful total order consistent
  with its `Eq`, as Rust's `Ord` contract requires. Both compare `components()`.
- AXM-CLONE: `Utf8PathBuf::clone` returns a value equal to, and byte-identical
  with, its source.
- AXM-ALLOC: a Rust allocation never exceeds `isize::MAX` bytes. A `Vec<T>` of
  a non-zero-sized `T` therefore has length at most `isize::MAX`, which is
  `usize::MAX / 2`. `Utf8PathBuf` is 24 bytes on 64-bit targets.
- AXM-DET: the detector passes canonicalization a sequence of length at least
  2 in which no two interior nodes compare `Equal` under `path_cmp`. This is
  because a depth-first-search stack holds each visiting node once, keyed by
  camino `Eq`. The closing element equals the first under `Eq` but may differ
  byte-wise.

The **Verus trusted base** for any claim about production is therefore
AXM-VERUS + REP + LEM-RANK + AXM-ORD + AXM-CLONE + AXM-DET (for C7). Kani's
trusted base for its kernel harnesses is Kani and CBMC alone, because Kani
checks the compiled production function. On trust, Verus is **wider**. The
weakest link is REP. VC-CORRESPOND checks its syntax, but nothing enforces that
Verus is re-run after an edit.

### Invariants, lemmas, and theorems

The main theorem is the mirror's `ensures` clause:

- T-CANON: for every `cycle` with `cycle.len() <= usize::MAX / 2`,
  `canonicalize_cycle_model(cycle)` returns `out` such that:
  - if `cycle@.len() < 2`, then `out@ == cycle@`;
  - otherwise,
    `exists |i| is_first_min(interior(cycle@), i)`, and
    `forall |i| #[trigger] is_first_min(interior(cycle@), i) ==>
    out@ == canonical_form(interior(cycle@), i)`.

  The `forall` half makes determinism part of the contract. It is discharged
  with LEM-FM-UNIQ.

Loop and helper obligations:

- INV-FIND: in `find_rotation_start_model`, at the loop head,
  `1 <= index <= len` and `is_first_min(cycle@.subrange(0, index), start)`. The
  loop has `decreases len - index`.
- INV-ROT: in `rotate_cycle_model`, at the loop head,
  `canonical@ == rotation(cycle@.subrange(0, len), start).subrange(0, offset)`,
  with the bounds and LEM-ROT-AT restated (R2). The loop has
  `decreases len - offset`.
- LEM-ROT-AT: for `0 <= k < n`, `rotation(s, i)[k]` is `s[i + k]` when
  `i + k < n`, and `s[i + k - n]` otherwise.
- LEM-POP: after `cycle.pop()`, `cycle@ == interior(old_cycle@)` and
  `cycle@.subrange(0, len) == cycle@`. This is why M6 is equivalent.
- LEM-FM-UNIQ: `is_first_min(s, i) && is_first_min(s, j) ==> i == j`. It
  holds without any premise.
- LEM-ROT-COMPOSE: rotating `rotation(s, k)` by `j` gives
  `rotation(s, k + j)` when `k + j < n`, and `rotation(s, k + j - n)`
  otherwise. It is needed for C8, and is likely unnecessary for C7, which the
  panel proved pointwise.

Specification corollaries, at `i` with `is_first_min(s, i)`, where
`out == canonical_form(s, i)`. These are the semantic content; T-CANON
transfers them to the mirror.

- C1, length: `out.len() == s.len() + 1`, which equals `cycle.len()`.
- C2, closure: `out[0] == out[out.len() - 1]`.
- C3, multiset: `interior(out).to_multiset() == s.to_multiset()`. This uses
  `vstd::seq_lib::lemma_multiset_commutative` twice, once on
  `subrange(i, n) + subrange(0, i)` and once on
  `subrange(0, i) + subrange(i, n)`, together with
  `s.subrange(0, i) + s.subrange(i, n) =~= s`.
- C4, minimal start: for every `j`, `out[0] <= s[j]`.
- C5, tie-break: for every `j < i`, `out[0] < s[j]`. The 4.2.2 harnesses do not
  check this, because they assume distinct identifiers.
- C6, idempotence: `is_first_min(interior(out), 0)` and
  `canonical_form(interior(out), 0) == out`. This needs no unique-minimum
  premise.
- C7, rotation invariance (`FV-CLAUSES` "stable start"): if `has_unique_min(s)`,
  `0 <= k < s.len()`, `t == rotation(s, k)`, and `is_first_min(t, m)`, then
  `canonical_form(t, m) == canonical_form(s, i)`.
- C7-NEED, premise necessity: with `s = [1, 2, 1, 3]` and
  `t = rotation(s, 2) = [1, 3, 1, 2]`, the canonical forms differ:
  `[1, 2, 1, 3, 1]` against `[1, 3, 1, 2, 1]`.
- C8, completeness: if `is_first_min(s, i)`, `is_first_min(t, m)`, and
  `canonical_form(s, i) == canonical_form(t, m)`, then `t` is a rotation of `s`
  (`exists |k| 0 <= k < s.len() && t == rotation(s, k)`). C7 and C8 together
  say that two cycles with unique minima share a canonical form exactly when
  they are rotations of each other. That is the full meaning of "canonical".
- LEM-RANK, a paper lemma (parametricity), not a Verus lemma. Premises:
  - the kernel observes values only through `compare(..) == Ordering::Less`
    and `clone`, which VC-CORRESPOND's rewrite table makes checkable;
  - AXM-ORD;
  - AXM-CLONE.

  Conclusion: for any input, mapping each element to its rank class yields a
  `u64` input on which the kernel selects the same start index and makes the
  same pushes. Its executable evidence is VC-RANK.
- VC-PREMISE, a paper argument: every production call satisfies
  `cycle.len() <= usize::MAX / 2`, by AXM-ALLOC.

Required proof functions. These replace a bare "N verified" count. A name may
change if the obligation and its `ensures` are unchanged:

- `lemma_rotation_at`
- `lemma_first_min_unique`
- `lemma_canonical_clauses` (C1-C5)
- `lemma_idempotent` (C6)
- `lemma_rotation_invariant` (C7)
- `lemma_canonical_complete` (C8)
- `witness_c7_need`
- the four `*_model` exec functions
- the `exec` witnesses listed below

### Mapping to the Kani equivalent

The three `u8` kernel harnesses assert C1-C4 and "is a rotation" for N=2..4
with distinct values. The Verus kernel proves those five clauses of the mirror
for every N and every value pattern, and adds C5-C8. Neither establishes
anything about the path wrapper, which stays Kani's, with VC-RANK as its
Proptest companion.

On acceptance, the N=3 and N=4 harnesses retire (`UD-D4`). The two-node harness
remains the bounded check of the compiled kernel.

### Verification artefacts, methods, and evidence

**VP-1: Verus proof of T-CANON, INV-*, LEM-*, C1-C8, and C7-NEED.**

- Method: deductive proof.
- Artefact: `verus/cycle_canonicalization/main.rs`.
- Command:
  `make verus VERUS_FLAGS='--proof-file verus/cycle_canonicalization/main.rs'`.
- Expected result: `verification results:: N verified, 0 errors`, with every
  required proof function present.
- Non-vacuity:
  1. `exec` witnesses call `canonicalize_cycle_model` and assert the exact
     output. An unhinted exact-output assertion is expected to fail; each
     witness needs three hints first (for `[3, 1, 2, 3]`):

     ```rust
     let ghost input = v@;
     assert(input =~= seq![3u64, 1, 2, 3]);
     assert(interior(input) =~= seq![3u64, 1, 2]);
     assert(is_first_min(interior(input), 1));
     ```

     The witnesses, with the expected output of each:
     - `[3, 1, 2, 3]` gives `[1, 2, 3, 1]`.
     - `[2, 1, 1, 2]` (ties) gives `[1, 1, 2, 1]`.
     - `[5, 5, 5, 5]` is unchanged.
     - `[7]` (the guard) needs only `assert(v@ =~= seq![7u64])`.
  2. `has_unique_min([3, 1, 2])` is proved by exhibiting index 1, so C7's
     premise is inhabited. C7-NEED shows the premise is necessary.
  3. **Spec mutants.** Each must make at least one witness fail to verify,
     which shows the witnesses test the specification against intent rather
     than mere satisfiability:
     - S1: drop the strict tie clause from `is_first_min`.
     - S2: define `rotation` as `subrange(0, i) + subrange(i, n)`.
  4. **Mirror mutants**, run on `/tmp` copies, must each produce a Verus
     error. The expected killer is noted for each:

     | Mutant | Change | Expected killer |
     | ------ | ------ | --------------- |
     | M1 | `<` to `>` in `find_rotation_start_model` | INV-FIND |
     | M2 | `<` to `<=` (last minimum wins) | INV-FIND / C5 |
     | M3 | delete the closing push | `rotate_cycle_model` ensures |
     | M4 | `index >= len` to `index > len` | `rotate_index_model` ensures |
     | M5 | `let len = cycle.len() - 1` to `cycle.len()` | preconditions |
     | M7 | `index = 1` to `index = 2` initially | INV-FIND entry |
     | M8 | `canonical.first()` to `canonical.last()` | ensures |
     | M9 | `offset < len` to `offset + 1 < len` | ensures |
     | M10 | `start = index` to `start = index - 1` | INV-FIND |
     | M11 | iterate the interior backwards from `start` | T-CANON rotation equality (C1-C4 still hold) |

     Two mutants are equivalent and must verify, each recorded as such:
     - M6, delete `cycle.pop()`, by LEM-POP;
     - M5c, pass `cycle.len()` as `len` to `rotate_cycle_model` after the
       `pop`, because after `pop` the two are equal.
  5. The VC-HYGIENE audit passes, and the Verus log shows no warnings about
     triggers or isolation that suggest a vacuous quantifier.
- Discharge: zero errors on the unmutated file; an error on S1, S2, M1-M5,
  and M7-M11; M6 and M5c verify; VC-HYGIENE green.

**VC-SPEC-ORACLE: production meets the specification on executable inputs.**

- Method: Proptest (law runner with a free seed; coverage runner with a fixed
  seed).
- Artefact: `src/ir/cycle/property_tests/canonical_spec_tests.rs`.
- Implementation:
  - A Rust transcription of `is_first_min`, `rotation`, and `canonical_form`.
    It is declarative, written with iterators and `get` (no `%`, no indexing,
    nesting at most 4 deep, functions under 70 lines, per the lint
    configuration), and calls no production helper.
  - Production `canonicalize_cycle_by(closed, u64::cmp)` must equal
    `canonical_form(interior, first_min)`.
  - Interiors have lengths 1 to 64, drawn from a narrow alphabet (0..=7, so
    ties are frequent) and a wide one (any `u64`).
  - Also checked: C6 idempotence on production output, and C7 on generated
    unique-minimum interiors and their rotations.
- Command: `make proptest` (its filter matches `property_tests`), then
  `make test`.
- Non-vacuity:
  - The coverage runner tallies the classes "length ≥ 11", "duplicated
    minimum", "minimum not at index 0", and "length 1 interior", and requires
    each to be non-zero.
  - Production mutants, applied with `git apply` as described under
    `Constraints`, must each fail the property:
    - the four existing `ir__cycle__verification__canonicalize_*.patch`
      files;
    - an ad hoc tie flip that uses `!= Ordering::Greater`, which only the tie
      cases detect.

**VC-RANK: the path wrapper agrees with the `u64` kernel on real camino paths
(LEM-RANK evidence).**

- Method: Proptest.
- Artefact: `canonical_spec_tests.rs`.
- Implementation:
  - Generate interiors of multi-component `Utf8PathBuf` values with no two
    `Equal` under camino, which is the AXM-DET shape. Close each with an
    element that equals the first under `Eq` but is spelled differently in
    some cases (for example `a//b` closing `a/b`), which is the reachable
    case.
  - Include order-disagreeing pairs such as `a/b` and `a-b`.
  - Rank each path as `sorted.partition_point(|q| q < p)` over the interior,
    sorted by `Ord` and deduplicated by `PartialEq`.
  - Assert the premise directly: for every pair,
    `p.cmp(q) == rank(p).cmp(rank(q))`.
  - Assert ranks: `canonicalize_cycle(paths)` mapped to ranks equals
    `canonicalize_cycle_by(ranks, u64::cmp)`.
  - Assert spelling: the path output, compared byte-wise with `as_str`,
    equals the input interior rotated at the start index the `u64` kernel
    chose, closed with a clone of its first element. The original closing
    spelling must not appear unless it is also the interior spelling.
- Non-vacuity:
  - Coverage tallies "closing spelling differs" and "byte order disagrees
    with component order", and requires both to be non-zero.
  - An ad hoc production mutant switching `path_cmp` to
    `left.as_str().cmp(right.as_str())` must fail.
  - The existing reversed-comparator patch must fail.

**VC-TIES: named examples.**

- Method: `rstest` table with `googletest` matchers and
  `pretty_assertions::assert_eq!`, every assertion carrying a message.
- Artefact: `canonical_spec_tests.rs`.
- Cases:
  - `[2, 1, 1, 2]` gives `[1, 1, 2, 1]`.
  - `[5, 5, 5, 5]` is unchanged.
  - `[3, 1, 2, 1, 3]` gives `[1, 2, 1, 3, 1]`.
  - `[1, 2, 1]` and `[2, 1, 2]` both give `[1, 2, 1]`.
- Non-vacuity: the tie-flip mutant fails the first and third cases.

**VC-DISTINCT: AXM-DET holds.**

- Method: Proptest over `analyse`, plus one named example.
- Artefact: `src/ir/cycle/property_tests/participation_tests.rs`, reusing
  `participation_graph_strategy`. The file is 359 lines and stays under 400.
- Property: every reported cycle has length at least 2, its last element
  equals its first under `Eq`, and no two interior nodes compare `Equal`.
- Named example: a manifest-shaped graph whose dependency spells a path
  `a//b` while the producing target spells it `a/b`. The reported interior is
  still distinct.
- Non-vacuity: coverage requires a non-zero count of cyclic cases and of
  cycles longer than 2.

**VC-HYGIENE: the trusted-base and layout contract for every Verus kernel.**

- Method: `rstest` integration test, run by `make test`. It reads text only
  and needs no Verus.
- Artefact: `tests/verus_kernel_contract_tests.rs`.
- Discovery: walk `verus/*/main.rs` through `repo_root()`, without manual
  registration.
- Raw-text checks:
  - the file is at most 400 lines and starts with `//!`;
  - every `fn`, `spec fn`, and `proof fn` item is preceded by `///`, ignoring
    attributes.
- Comment-stripped checks, with `//` line and `/* */` block comments
  removed:
  - The file contains none of the forbidden constructs, matched as tokens:
    `\bassume\s*\(`, `\badmit\s*\(`, `\baxiom\s+fn\b`, `external_body`,
    `verifier::external`, `verifier(external`, `external_fn_specification`,
    `external_type_specification`, `assume_specification`,
    `verifier::trusted`, `\buninterp\b`, `exec_allows_no_decreases_clause`,
    `\brlimit\b`, and `\bunsafe\b`.
  - The file names none of the forbidden types: `HashMap`, `IrHashMap`,
    `Utf8PathBuf`, `camino`, `BuildGraph`, `BuildEdge`, `minijinja`,
    `std::fs`, or `std::process`.
- Cross-file checks:
  - no `Cargo.toml` mentions `verus/`;
  - the `Makefile` `VERUS_FLAGS` default lists exactly the discovered kernels,
    in sorted order. This mechanically closes D5's "unrun kernel" risk
    (`UD-FLAGS`).
  - `.github/workflows/proofs.yml`, parsed with `serde_yaml`, includes in both
    `on.pull_request.paths` and `on.push.paths`:
    - `verus/**`, `tools/verus/**`, and `Makefile`;
    - every production path a kernel mirrors, as declared by VC-CORRESPOND's
      table (`src/ir/cycle/support.rs`).

    This is `UD-PATHS` and D17.
- Non-vacuity: the checker is a pure function over `&str`, tested with
  in-memory fixtures:
  - `assume(false)` in code fails;
  - `assume` inside a comment passes;
  - `broadcast use vstd::seq::group_seq_axioms;` passes;
  - `#[verifier::external_body]` fails;
  - a 401-line text generated as `"x\n".repeat(401)` fails;
  - an undocumented `proof fn` fails;
  - a workflow fixture whose `push` filter omits `src/ir/cycle/support.rs`
    fails.

**VC-CORRESPOND: REP, the mirror matching production.**

- Method: integration test in the same binary, with the module
  `tests/verus_kernel_contract_tests/correspondence.rs`.
- Implementation:
  1. Extract the bodies of the four production functions from
     `src/ir/cycle/support.rs` and of the four mirror functions from the
     Verus file. Use a brace-matching scanner over `char_indices` that skips
     string and character literals.
  2. Remove from the mirror `proof { ... }` blocks and the clauses between a
     `while` condition and its body.
  3. Strip comments, collapse whitespace, and apply the rewrite table under
     REP to the production text.
  4. Assert equality, and print a unified diff on mismatch.
- Non-vacuity, with inline fixtures:
  - the extractor captures nested braces and a `'{'` character literal;
  - a missing function fails;
  - a mirror with a flipped `<` fails;
  - a production body with an extra statement fails.
- The failure message names the obligation: "re-run `make verus` after
  updating the mirror".

**VC-WIRING: `make verus` runs both kernels.**

- Method: the existing pinned tests, updated in EP-M3.
- Changes:
  - The Python check asserts the whole command line, so it can go red.
  - The Rust make test expects all six argument-vector tokens, and is
    renamed from `verus_uses_the_canonical_value_proof_by_default` to
    `verus_runs_every_kernel_by_default`.
  - The e2e test asserts `verification_runs(&output) == 2` and two result
    lines that each end in `0 errors`.
- Commands: `make test`, `make test-prover-tools-integration`, and, with
  Verus installed, `make test-verus-e2e`. In CI, the pull request's
  `Proofs / Verus` check runs `make verus`, which now covers both kernels. The
  pull request touches `verus/**`, the `Makefile`, and `proofs.yml`, so the
  check is triggered.
- Non-vacuity: the updated tests fail against the old default and old filters
  (red), then pass (green).

### Acceptance rule (decision gate G1)

This rule makes `RM-443-ACC` measurable. The Kani equivalent is the three `u8`
kernel harnesses, because their obligations are the ones the kernel duplicates.

**Scope fences.** These are true by construction and recorded, not scored. A
breach means a constraint was violated, which is an escalation, not a verdict:

- F1: the proof boundary is the pure rotation kernel only.
- F2: no forbidden trusted construct.
- F3: one file of at most 400 lines.
- F4: no Cargo dependency, no new CI job or workflow, and no routine gate.
  The existing `proofs.yml` job runs the kernel.

**Narrower.** All three must hold:

- N5, mechanical correspondence: the mirror relates to production only
  through the four-row rewrite table, and VC-CORRESPOND passes. Needing any
  further rewrite rejects the kernel.
- N6, assurance delta: the proof establishes, for some reachable input class,
  a property neither the Kani harnesses nor the existing Proptest suite
  establishes. The planned witnesses are C7 and T-CANON for N above 10. If
  either fails to verify, N6 fails.
- N7, mutation adequacy: every mirror and spec mutant listed in VP-1 is killed
  for its expected reason, and the two equivalent mutants verify.

**Cheaper.** These are measured in one session on one host, as the median of 5
warm runs:

- K1, time: the Verus `total-time` figure (from `--extra-arg=--time`), which
  includes `vstd` loading and conservatively penalizes Verus, must be at most
  0.8 times the summed Kani "Verification Time" of the three harnesses. Record
  CPU time (user plus system) for both, for information.
- K2, memory: the peak memory of the Verus run must be at most the largest
  Kani harness peak. Both are measured as a cgroup `Memory peak` covering the
  whole process tree (Concrete steps M0.3). Each figure states whether it
  includes `kani-compiler`. A warm Kani run reuses the harness build, so its
  peak is mostly CBMC.
- K3, end-to-end wall time for both, including Kani's compile through
  `kani-compiler`. Informative only, because that compile is shared by all 17
  repository harnesses.
- K4, maintenance surface: lines that exist only because of Verus (the kernel
  and the contract-test files) against lines that exist only for the three Kani
  kernel harnesses (the harnesses and their `verification/support.rs` helpers).
  Informative only; ADR-0082 must state and justify the ratio.

Acceptance is **partly substitutive**: under `UD-D4`, accepting the kernel
retires the N=3 and N=4 harnesses. That removes about 19.4 s of recorded
`kani-full` verification time (7.602 s + 11.815 s at 4.2.2, re-measured in
EP-M0) from every Kani run, which happens on each pull request that changes a
proof input. The Verus job adds its own cost in `proofs.yml`, which runs only
when its path filters match and weekly. ADR-0082 records both sides.

**Verdict.**

- Accept if N5, N6, N7, K1, and K2 all hold. Continue with EP-M3 and EP-M4.
- Reject if any of them fails. Continue with EP-M4R, which completes the
  item. There is no "marginal" acceptance and no rule tuning at the gate. A
  change to the rule needs the user's approval and a plan revision.

## Milestones and plateaus

The standard gates run sequentially, each teed to
`/tmp/<gate>-netsuke-<branch>.out`, with `set -o pipefail`:

1. `make check-fmt`
2. `make lint`
3. `make typecheck`
4. `make doc-coverage`
5. `make markdownlint`
6. `make nixie`
7. `make test`

`make test-workflow-contracts` runs after them whenever a milestone adds a
`src/` module or changes the `Makefile`, and before every push. In this
environment a dedicated gate-runner agent runs these, but any maintainer can
run them in the order above.

Every milestone has the same compatibility decision. No compatibility layer is
needed: every changed surface is internal, test-only, or a maintainer Make
default, and the project is pre-1.0.

### EP-M0: Preflight and baselines

- Requirements: `RM-443-ACC` (baseline), R1, R12.
- Work, with no source changes:
  - Install Verus.
  - Verify the canonical-value kernel and record its time as a start-up
    reference.
  - Run a 30-line probe file on the pinned version confirming R1's allowance:
    the nested `if let`, `slice::get`, `first().cloned()`,
    `Vec::with_capacity`, `pop`, a `const fn` with a contract, and the
    let-chain rejection.
  - Confirm the `--extra-arg=--time` spelling.
  - Re-measure the three Kani harnesses (K1, K2, K3 inputs).
- End state: the tree is unchanged apart from this plan's `Artefacts and notes`.
- Acceptance: the baseline figures are recorded; `git status --short` shows
  only this plan.
- Conformance check:
  1. Requirements met: the baseline exists for G1.
  2. Design conformance: unaffected.
  3. Assumptions: R1 and R12 are confirmed or falsified. If falsified, revise
     the plan before EP-M2.
  4. Interfaces or dependencies: none.
  5. Trust boundary: none.
  6. Trace links: unchanged.
- Recovery: re-run the measurements.
- Remaining gaps: every gap is still open.
- Commit: `Record 4.4.3 Verus and Kani baselines` (plan only).

### EP-M1: Executable specification and hygiene tests

- Requirements: `GAP-ORDER`, `GAP-TIE`, `GAP-HYGIENE`, the AXM-DET check, and
  part of `GAP-ALLN` (sampled).
- Work:
  - Add `canonical_spec_tests.rs` (VC-SPEC-ORACLE, VC-RANK, VC-TIES) and
    register it in `property_tests/mod.rs`.
  - Add VC-DISTINCT to `participation_tests.rs`.
  - Add `tests/verus_kernel_contract_tests.rs` with VC-HYGIENE over the
    discovered kernels. Only `canonical_value` exists at this point, and the
    `VERUS_FLAGS` cross-check expects exactly it.
  - Fix the "lexicographically" comments in `support.rs` (line 84, doc
    comment only) and `property_tests/mod.rs` (line 150).
- Red-green-refactor:
  - For VC-HYGIENE, write the fixture tests before the checker. They fail to
    compile, then fail, then pass once the checker exists.
  - For the specification properties, production is already correct, so the
    red step is the seeded production mutants (D7). Each must fail its
    property. Record the failing property and its minimal case.
- End state: `make test` checks production against the specification and
  enforces hygiene on the existing kernel. No Verus change. The tree is
  coherent whatever G1 later decides.
- Acceptance:
  - The standard gates and `make test-workflow-contracts` pass.
  - `cargo nextest list --all-features` output, under the gate `RUSTFLAGS`,
    includes `canonical_spec_tests` and `verus_kernel_contract_tests`.
  - Coverage tallies are non-zero.
  - Each mutant's failure is recorded in `Outcomes & retrospective`.
- Conformance check:
  1. Do the oracle and production share no code?
  2. Do the tests touch only test-only modules, so no Kani proof-scope change
     is needed? `src/ir/` changes trigger `kani-smoke` on the pull request,
     which is expected.
  3. Did any upstream assumption fail? AXM-DET or AXM-ORD failing is an
     escalation.
  4. Was any public interface or dependency introduced?
  5. Is the trust boundary unchanged?
  6. Do the trace links resolve?
- Recovery: `git revert` the commit; the tests are additive.
- Remaining gaps: `GAP-ALLN` (proof), REP, `GAP-STALE-REF`, `GAP-NO-ADR`.
- Commit: `Test cycle canonicalization against its spec`.

### EP-M2: Verus kernel prototype and decision gate G1 (prototyping)

- Requirements: `RM-443-OUT`, `RM-443-MODEL`, `FV-CLAUSES`, `DD-ROT`,
  `GAP-ALLN`, and `RM-443-ACC`.
- Work:
  1. Write `verus/cycle_canonicalization/main.rs`: the `//!` comment, the
     specification, and the mirror with its `requires` and `ensures`, but no
     invariants or lemmas.
  2. Red: run Verus. It must report errors on the loop postconditions.
  3. Green: add invariants and lemmas until it verifies, letting the reported
     failures choose which lemma comes next.
  4. Refactor for size and trigger hygiene.
  5. Run the S, M, and equivalent mutants.
  6. Measure K1-K4 and apply G1.
  7. Extend VC-HYGIENE's expectations to include the new kernel only on
     acceptance; that change belongs to EP-M3.
- End state if accepted: the kernel file exists and verifies. `make verus`
  still defaults to the canonical-value kernel, and VC-HYGIENE's `VERUS_FLAGS`
  check is temporarily red. To keep the plateau coherent, commit the kernel
  together with EP-M3's `Makefile` change; see EP-M3.
- End state if rejected: the kernel is not committed. The measurements,
  mutant table, and kernel SHA-256 go into `Artefacts and notes`, and the
  kernel text is kept for EP-M4R.
- Acceptance:
  - `make verus VERUS_FLAGS='--proof-file verus/cycle_canonicalization/main.rs'`
    reports `0 errors`, with every required proof function present.
  - The mutant results match the VP-1 table.
  - `wc -l` reports at most 400 lines.
  - The G1 verdict is recorded with its numbers.
- Conformance check:
  1. Is the mirror limited to the R1 divergence?
  2. Is no production type mentioned?
  3. Are no trusted constructs present?
  4. Does any proof need a precondition a production caller could violate?
  5. Are the K figures stage-matched?
  6. Is the verdict recorded before any further edits?
- Recovery: the kernel is a single file; delete it or `git revert`.
- Remaining gaps: REP check and wiring (EP-M3); documentation and ADR
  (EP-M4 or EP-M4R).
- Commit: none on its own. Accepted work is committed in EP-M3; a rejection
  goes to EP-M4R.

### EP-M3: Accept path: kernel, correspondence, CI wiring, and Kani retirement

- Requirements: REP (N5, enforced), `FV-GATES`, `FV-CI`, `UD-FLAGS`,
  `UD-PATHS`, and `UD-D4`.
- Work:
  1. Add `correspondence.rs` (VC-CORRESPOND), with its fixtures first. Red:
     they fail before the scanner exists.
  2. Extend VC-HYGIENE with the `proofs.yml` check (see VC-WIRING) and update
     the three pinned tests to the two-file default. Red: they fail against
     the old `Makefile` and workflow.
  3. Commit the kernel. Change `VERUS_FLAGS` to
     `--proof-file verus/canonical_value/main.rs --proof-file
     verus/cycle_canonicalization/main.rs` and update its comment. Add
     `src/ir/cycle/support.rs` to both the `pull_request` and `push` `paths`
     lists of `.github/workflows/proofs.yml`, and update the workflow's header
     comment to name both proofs. Green: VC-HYGIENE, VC-CORRESPOND, and
     VC-WIRING all pass.
  4. Retire the N=3 and N=4 Kani harnesses (`UD-D4`), as a separate commit:
     - delete `canonicalize_three_node_cycle_is_canonical` and
       `canonicalize_four_node_cycle_is_canonical` from
       `src/ir/cycle/verification.rs`;
     - delete `closed_three_node_id_cycle`, `closed_four_node_id_cycle`, and
       any helper only they used from `src/ir/cycle/verification/support.rs`;
     - delete `docs/verification/mutations/ir__cycle__verification__canonicalize_three_node_cycle_is_canonical.patch`
       and `..._four_node_cycle_is_canonical.patch`.

     Their faults (a lost closing push and a missing wrap) stay covered by the
     two-node harness, by Verus mutants M3 and M4, and by VC-SPEC-ORACLE's
     recorded production-mutant evidence from EP-M1. Before deleting, confirm
     the two-node harness kills both faults. Apply each patch with the
     `git apply` and `git apply -R` procedure, run the command below, and
     record the failing assertion:

     ```bash
     H=canonicalize_two_node_cycle_is_canonical
     make kani-full KANI_FLAGS="--harness $H --exact"
     ```

- End state:
  - `make verus` proves both kernels, and the CI Proofs job runs them whenever
    a kernel, its pin, the `Makefile`, the workflow, or the mirrored
    `src/ir/cycle/support.rs` changes.
  - `make test` enforces hygiene, correspondence, the default, and the path
    filters. No routine gate runs Verus, except the pre-existing e2e lane on
    hosts with Verus installed.
  - Kani holds 15 harnesses, nine of them in `src/ir/cycle/verification.rs`.
- Acceptance:
  - The standard gates, `make test-workflow-contracts`,
    `make test-prover-tools-integration`, `make test-verus-e2e` (strict), and
    `make test-kani-mutations` pass.
  - `make verus` prints two `0 errors` lines.
  - The e2e default case's duration is recorded and is under 150 s.
  - `make kani-ir` reports 15 harnesses verified, 0 failures.
  - On the pushed pull request, the `Proofs / Verus` check passes and the
    `kani-smoke` job passes.
- Conformance check:
  1. Do `FV-GATES` and `FV-CI` hold, with `proofs.yml` the only workflow
     touched?
  2. Is the `proofs.yml` edit limited to the two path entries and its comment?
  3. Does the rewrite table have exactly four rows?
  4. Was no dependency added?
  5. Were exactly the two named harnesses, their builders, and their patches
     removed?
  6. Is VC-WIRING linked in the trace?
- Recovery: `git revert` of the retirement commit restores the two harnesses
  and patches. `git revert` of the wiring commit restores the single-file
  default and the old path filters, and removes the kernel and the
  correspondence test.
- Remaining gaps: `GAP-STALE-REF`, `GAP-NO-ADR`, `GAP-ORDER` (design-document
  text).
- Commit: `Add Verus kernel for cycle canonicalization`.

### EP-M4: Accept path: decision record and documentation

- Requirements: `AG-ADR`, `GAP-NO-ADR`, `GAP-STALE-REF`, `GAP-ORDER`, and
  roadmap completion.
- Work:
  - ADR. Write
    `docs/adrs/adr-0082-accept-a-verus-proof-kernel-for-cycle-canonicalization.md`
    with these sections:
    - Status (Accepted, with date and a summary);
    - Date;
    - Context and problem statement;
    - Decision drivers;
    - Options considered: spec-level model, `exec` mirror, `#[path]` import,
      dropping Kani's distinctness assumption, the specification oracle
      alone;
    - Decision outcome;
    - Consequences, including the trusted-base comparison, REP as the weakest
      link, and the additive cost (K4);
    - Evidence, with the G1 measurements and mutant results;
    - Known risks;
    - Cross-references to ADR-0047 and ADR-0068.

    It also ratifies the formal-verification document's in-principle
    acceptance. Index it in `docs/contents.md`.
  - `docs/netsuke-design.md` §5.3 step 4. State in plain words:
    - the order is camino `Utf8Path` component order;
    - ties go to the first occurrence;
    - the canonical form preserves length, closure, and the node multiset;
    - it starts at the minimum;
    - two cycles share a canonical form exactly when they are rotations of
      each other.

    Then say what is proved of the mirror, what is tested of production, and
    link the ADR and the kernel. Do not use plan identifiers such as C1-C8.
  - `docs/adrs/adr-0047-bound-kani-intermediate-representation-ir-harnesses-to-small-n.md`:
    an `Amended by ADR-0082 (date)` banner and an `## Amendments` entry. The
    entry records that the N=3 and N=4 canonicalization harnesses retired in
    favour of the Verus kernel, and that N=2 and the path wrapper remain (D12).
  - `docs/formal-verification-methods-in-netsuke.md`:
    - lines 47-48 and 344: "17 harnesses" becomes 15, and the cycle module's
      11 becomes 9;
    - the `proofs.yml` description: both proofs, and the mirrored source path
      in the filters;
    - lines 66-67: two kernels, and the `make verus` default batch;
    - lines 104-110: the kernel now exists;
    - lines 172-214: the kernel exists, with an ADR link;
    - "Layout" lines 245-270: the real `verus/<kernel>/main.rs` paths;
    - footnote 7: `src/ir/cycle/support.rs`.
  - `docs/developers-guide.md`:
    - lines 6966-6968 and 6991-6992: the default batch;
    - lines 7203-7208: N=2 only, with N=3 and N=4 retired to the Verus kernel,
      and mention VC-RANK and VC-SPEC-ORACLE;
    - the Kani harness table (around lines 7273-7274): delete the N=3 and N=4
      rows;
    - the CI Proofs description: both kernels, and why
      `src/ir/cycle/support.rs` is in its path filters;
    - lines 7446-7452: correct the stale path, two kernels, and run "by
      default" for `make verus`;
    - a new "Verus kernels" subsection: what each kernel proves and does not
      prove, the trusted-base statement, the mirror-maintenance rule (edit
      both sides, re-run `make verus`; VC-CORRESPOND enforces the syntax), how
      to run mutants, the hygiene rule, and its reuse policy (one owner; a new
      kernel is discovered automatically);
    - the acceptance rule by reference to the ADR, with no timings;
    - lines 12989-12994 (the IR cycle architecture): one sentence pointing at
      the kernel and the specification tests.
  - `docs/repository-layout.md`:
    - `verus/`, `verus/canonical_value/`, `verus/cycle_canonicalization/`,
      and `tools/verus/` in the tree and in path responsibilities;
    - in the `src/ir/cycle/support.rs` ownership section (around line 204),
      the coupling: editing the four functions requires updating the mirror
      and re-running `make verus`.
  - `docs/roadmap.md`: mark 4.4.3 and its three sub-bullets `[x]`, with a
    completion note linking the ADR and this plan.
  - This plan's status becomes `COMPLETE`.
  - `docs/users-guide.md` does not change (D9).
- Stale-claim sweep, which must print only intended hits:

  ```bash
  STALE='src/ir/cycle/mod.rs|lexicograph|one Verus kernel'
  STALE="$STALE|Two (kernels )?are accepted|defaults to .verus/canonical_value"
  rg -n "$STALE" docs src --glob '!docs/execplans/**'
  ```

- Acceptance:
  - The standard gates and `make test-workflow-contracts` pass.
  - Concrete step M4.2 reports 64 ADR index entries, exit 0, and no
    unindexed ADR.
  - The serial re-check (M4.1) passes immediately before commit and push.
- Conformance check:
  1. Every `GAP-*` is closed.
  2. The design document makes no over-claims.
  3. No timings appear in live guides.
  4. ADR sections are complete.
  5. The ADR index is consistent.
  6. Trace links resolve.
- Recovery: a documentation-only commit; `git revert`.
- Remaining gaps: none in scope.
- Commit: `Record Verus cycle kernel decision`.

### EP-M4R: Reject path: rejection record

- Requirements: `RM-443-ACC` (negative verdict), `AG-ADR`, `GAP-NO-ADR`,
  `GAP-STALE-REF`, and `GAP-ORDER`.
- Work:
  - Write
    `docs/adrs/adr-0082-reject-a-verus-proof-kernel-for-cycle-canonicalization.md`.
    It has the same sections as the acceptance ADR, plus an appendix holding
    the specification, theorem statements, measurements, mutant table, and
    the kernel's SHA-256.
  - Preserve the kernel text by a branch-independent route: ask the user
    whether to push a tag `evidence/4-4-3-rejected-kernel` (an outward-facing
    action needing approval) or to keep the full text in the ADR appendix.
    Never cite branch commit SHAs, because squash merges discard them.
  - Amend the formal-verification document (lines 17-18, 104-110, and
    182-192) and the developers' guide (line 7449). The cycle kernel was
    evaluated and rejected, with an ADR link, and one kernel is accepted.
  - Make the same design-document precision edit as EP-M4, without the proof
    claims.
  - Mark roadmap 4.4.3 `[x]` with the note "evaluated: rejected", linking the
    ADR.
  - EP-M1's tests remain; there are no `Makefile` or wiring changes.
- Acceptance: the same as EP-M4, minus `make verus` and wiring.
- Conformance check: the same six items, plus confirmation that no document
  still claims the cycle kernel is accepted.
- Recovery: documentation-only; `git revert`.
- Remaining gaps: `GAP-ALLN` stays open; the ADR names the cheaper
  follow-ups.
- Commit: `Record rejection of Verus cycle kernel`.

## Plan of work

The work is additive. The only production edit is one doc-comment word.

1. EP-M0 gathers evidence.
2. EP-M1 lands the specification tests and the hygiene contract, which are
   useful whatever the verdict.
3. EP-M2 writes and judges the proof.
4. EP-M3 and EP-M4 integrate an accepted kernel; EP-M4R records a rejected
   one.

The Verus kernel follows `verus/canonical_value/main.rs`:

1. a `//!` block of about 40 lines covering what is proved, what is not, REP
   and its rewrite table, the R1 divergence, and the trusted base;
2. `use vstd::prelude::*;` and `verus! { ... }`;
3. the specification;
4. the specification lemmas;
5. the mirror with its contracts;
6. the witnesses;
7. `fn main() {}`.

Lemma bodies are written only when Verus asks for them.

The new property tests sit beside the existing cycle property tests and use
their module-private access to `support::canonicalize_cycle_by`. The contract
test lives under `tests/`. It reuses `tests/support/makefile.rs` through
`#[path]`, and its helpers return `anyhow::Result`, using `ensure!` rather than
`.expect()`.

## Concrete steps

Run every command from the repository root. Start each shell with:

```bash
set -o pipefail
BR=$(git branch --show-current)
```

M0.1: install Verus and run the existing kernel:

```bash
make install-verus 2>&1 | tee "/tmp/install-verus-netsuke-$BR.out"
make verus 2>&1 | tee "/tmp/verus-netsuke-$BR.out"
git status --short
```

Expected: the output ends with `verification results:: 16 verified, 0 errors`,
and `git status` shows only this plan.

M0.2: check the pinned CLI's flags, and run the probe file from a scratch
directory:

```bash
make verus VERUS_FLAGS=--help 2>&1 | tee "/tmp/verus-help-netsuke-$BR.out"
P=$(mktemp -d /tmp/verus-probe-XXXX)
# write $P/probe.rs (about 30 lines), then:
make verus VERUS_FLAGS="--proof-file $P/probe.rs --extra-arg=--time" 2>&1 \
  | tee "/tmp/verus-probe-netsuke-$BR.out"
```

M0.3: the memory and time harness. `/usr/bin/time` is absent, so wrap each
command in a transient systemd user unit, whose cgroup covers the whole process
tree:

```bash
measure() {  # usage: measure LABEL COMMAND...
  systemd-run --user --wait --collect --same-dir -p MemoryAccounting=yes \
    -E PATH -E HOME -E CARGO_HOME -E RUSTUP_HOME -E RUSTC_WRAPPER "$@" \
    2>&1 | tee "/tmp/measure-$1-netsuke-$BR.out"
}
for h in canonicalize_two_node_cycle_is_canonical \
         canonicalize_three_node_cycle_is_canonical \
         canonicalize_four_node_cycle_is_canonical; do
  for run in 1 2 3 4 5 6; do   # run 1 warms the build; record runs 2-6
    measure "kani-$h-$run" make kani-full KANI_FLAGS="--harness $h --exact"
  done
done
grep -h -E 'Verification Time|Memory peak|CPU time' /tmp/measure-kani-*-netsuke-$BR.out
```

`systemd-run --wait` prints `Memory peak:` and `CPU time consumed:` when the
unit ends. If the user manager is unavailable, fall back to a `python3 -I`
wrapper that prints `resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss`.
That reports the largest single process, and K2 must then say so.

M2: prove, then measure:

```bash
make verus VERUS_FLAGS='--proof-file verus/cycle_canonicalization/main.rs --extra-arg=--time' \
  2>&1 | tee "/tmp/verus-cycle-netsuke-$BR.out"
for run in 1 2 3 4 5 6; do
  measure "verus-cycle-$run" make verus \
    VERUS_FLAGS='--proof-file verus/cycle_canonicalization/main.rs --extra-arg=--time'
done
wc -l verus/cycle_canonicalization/main.rs
```

Verus mutants, each in a fresh scratch copy:

```bash
S=$(mktemp -d /tmp/verus-mutant-XXXX)
cp verus/cycle_canonicalization/main.rs "$S/main.rs"
# apply one mutant to $S/main.rs, then:
make verus VERUS_FLAGS="--proof-file $S/main.rs" 2>&1 | tee "/tmp/verus-mutant-M1-netsuke-$BR.out"
```

Production mutants (EP-M1), only on a clean, committed tree and never during a
gate batch:

```bash
git diff --exit-code && git diff --cached --exit-code
git apply docs/verification/mutations/ir__cycle__verification__canonicalize_two_node_cycle_is_canonical.patch
make proptest 2>&1 | tee "/tmp/mutant-two-node-netsuke-$BR.out"   # expect a failure
git apply -R docs/verification/mutations/ir__cycle__verification__canonicalize_two_node_cycle_is_canonical.patch
git diff --exit-code
```

M3:

```bash
make test-prover-tools-integration 2>&1 | tee "/tmp/prover-int-netsuke-$BR.out"
make test-verus-e2e 2>&1 | tee "/tmp/verus-e2e-netsuke-$BR.out"
make verus 2>&1 | tee "/tmp/verus-all-netsuke-$BR.out"
grep -n -A6 -E '^  (pull_request|push):' .github/workflows/proofs.yml   # both list src/ir/cycle/support.rs
make kani-ir 2>&1 | tee "/tmp/kani-ir-netsuke-$BR.out"                  # after retirement: 15 verified
make test-kani-mutations 2>&1 | tee "/tmp/kani-mutations-netsuke-$BR.out"
```

M4.1: re-derive the ADR serial. Use the clean SSH environment the user's global
instructions prescribe for remote Git (`GIT_CONFIG_*` variables and token
variables removed, `GIT_SSH_COMMAND=/usr/bin/ssh`):

```bash
git fetch git@github.com:leynos/netsuke.git '+refs/heads/*:refs/remotes/origin/*' -q
for ref in $(git for-each-ref --format='%(refname)' refs/remotes/origin); do
  git ls-tree -r --name-only "$ref" -- docs/ 2>/dev/null \
    | grep -oE '(^|/)adr-[0-9]{3,4}-' | grep -oE '[0-9]{3,4}'
  git grep -h -o -E 'ADR-0?0?[89][0-9]' "$ref" -- docs 2>/dev/null | grep -oE '[0-9]+'
done | sed 's/^0*//' | sort -n | uniq | tail -3
```

On 2026-11-15 the highest allocation anywhere was 81, on the PR #808 branch. If
the ceiling has moved, take the next number above it and update every
reference. Then assert, for every ADR index entry, that the label matches the
filename.

M4.2: check the ADR index:

```bash
tr '\n' ' ' < docs/contents.md \
  | grep -oE '\[(ADR|adr)-[0-9]+[^]]*\]\(adrs/adr-[0-9]{4}-[^)]*\)' \
  | sed -E 's/^\[(ADR|adr)-([0-9]+)[^]]*\]\(adrs\/adr-([0-9]{4})-.*$/\2 \3/' \
  | awk '{ n++ } $1 + 0 != $2 + 0 { print "MISMATCH", $0; bad = 1 }
         END { print n " ADR index entries"; exit bad }'
comm -23 <(ls docs/adrs | grep -E '^adr-[0-9]{4}-.*\.md$' | sort) \
  <(grep -oE '\(adrs/adr-[^)]+\.md\)' docs/contents.md | sed -E 's/^\(adrs\/(.*)\)$/\1/' | sort -u)
```

Expected: `64 ADR index entries`, exit 0, and an empty `comm` listing. On
`main` today it prints `63 ADR index entries`.

## Validation and acceptance

The item is done when one of the following holds.

**Acceptance path:**

- G1 accepted, with N5-N7 and K1-K4 recorded.
- `make verus` prints two result lines ending `0 errors`, with every required
  proof function present.
- The mutant table matches VP-1.
- `make test` includes `canonical_spec_tests`, the VC-DISTINCT property,
  `verus_kernel_contract_tests` (hygiene over both kernels, correspondence, and
  the default cross-check), and the updated wiring tests.
- The production mutants fail VC-SPEC-ORACLE and VC-RANK. The tie flip fails
  VC-TIES.
- `make check-fmt`, `make lint`, `make typecheck`, `make doc-coverage`,
  `make markdownlint`, `make nixie`, `make test`,
  `make test-workflow-contracts`, `make test-prover-tools-integration`, and
  `make test-verus-e2e` pass.
- Roadmap 4.4.3 is marked done, ADR-0082 is indexed, and the design document
  states the precise rule.

**Rejection path:**

- G1 rejected, with the failing criterion and its numbers.
- EP-M1's tests pass in `make test`.
- The rejection ADR is indexed. No document claims the cycle kernel is
  accepted. Roadmap 4.4.3 is marked done as "evaluated: rejected".
- The standard gates and `make test-workflow-contracts` pass.

Proposed acceptance scenarios for maintainers. No feature file is created,
because no user-visible behaviour changes (D9). Each scenario is traced to its
requirement and evidence:

```gherkin
Feature: Cycle canonicalization proof kernel

  # FV-GATES, RM-443-ACC -> EP-M3 -> VC-WIRING, make verus transcript
  Scenario: A maintainer proves every accepted kernel
    Given the pinned Verus release is installed with "make install-verus"
    When the maintainer runs "make verus"
    Then the canonical value kernel reports 0 errors
    And the cycle canonicalization kernel reports 0 errors

  # REP, R5 -> EP-M3 -> VC-CORRESPOND
  Scenario: The production kernel changes without the mirror
    Given an extra statement in rotate_cycle_by in src/ir/cycle/support.rs
    When the maintainer runs "make test"
    Then the Verus kernel correspondence test fails
    And its message asks for the mirror to be updated and "make verus" re-run

  # GAP-HYGIENE -> EP-M1 -> VC-HYGIENE
  Scenario: A trusted shortcut enters a kernel
    Given a Verus kernel containing "assume(false)" outside a comment
    When the maintainer runs "make test"
    Then the Verus kernel contract test fails and names the construct

  # GAP-ORDER, AXM-ORD -> EP-M1 -> VC-RANK
  Scenario: Path order is component-wise
    Given a cycle through "a-b" and "a/b"
    When Netsuke canonicalizes it
    Then "a/b" comes first
```

## Idempotence and recovery

Every step is re-runnable. `make install-verus` is idempotent against the pin.
Verus mutants use fresh `/tmp` directories. Production mutants are applied and
reverted in place, with `git diff --exit-code` before and after. Each milestone
ends in a commit that `git revert` undoes cleanly. If a gate fails, fix the
cause and re-run that gate and every later gate in order. Never edit the tree
while a gate batch is running; an edit voids the batch.

## Artefacts and notes

These are filled in during implementation:

- EP-M0: Verus start-up reference (canonical-value kernel): *pending*.
- EP-M0: probe-file results on the pinned Verus: *pending*.
- EP-M0: Kani K1/K2/K3 baselines (median of runs 2-6): *pending*.
- EP-M1: production-mutant failures and coverage tallies: *pending*.
- EP-M2: Verus K1-K4, the mutant table, and the G1 verdict: *pending*.
- EP-M3: e2e default-case duration: *pending*.

## Interfaces and dependencies

- No production interface changes.
- New test-only items:
  - the private module
    `crate::ir::cycle::property_tests::canonical_spec_tests`;
  - the integration test binary `verus_kernel_contract_tests`, with module
    `correspondence`.
- New proof file `verus/cycle_canonicalization/main.rs`, outside every Cargo
  target.
- The `Makefile` `VERUS_FLAGS` default becomes a two-file ordered batch, on
  the acceptance path only.
- `.github/workflows/proofs.yml` gains `src/ir/cycle/support.rs` in both path
  filters, on the acceptance path only.
- Kani loses two harnesses and two mutation patches, on the acceptance path
  only.
- Dependencies: none added.
- Reuse policy:
  - The VC-HYGIENE checker is the single owner of the Verus-kernel hygiene
    rule. It discovers `verus/*/main.rs` automatically and must not grow into
    a general Rust linter.
  - The correspondence scanner is owned by this one test. It is not a shared
    parser; a second user would need its own design decision.
  - The developers' guide records both rules.

## Progress

- [x] 2026-11-15T00:05+01:00: renamed the branch to
  `4-4-3-verus-proof-kernel-for-cycle-canonicalization`.
- [x] 2026-11-15T00:20+01:00: completed reconnaissance of the production
  kernel, Kani coverage, Verus infrastructure, documentation, and budget
  history.
- [x] 2026-11-15T00:25+01:00: ran the ADR ceiling sweep. The highest
  allocation is 81, so 0082 is provisional.
- [x] 2026-11-15T00:30+01:00: wrote draft revision 1.
- [x] 2026-11-15T00:45+01:00: received the four-member expert-panel review
  (Verus proof engineering, formal-methods evaluation, test and tooling,
  documentation and governance).
- [x] 2026-11-15T00:55+01:00: wrote revision 2, applying the review.
- [x] 2026-11-15: opened draft plan pull request #1136.
- [x] 2026-11-15: rebased onto `origin/main` `fa06bdb21`, cleanly; the
  range-diff showed both commits unchanged. Main had added #1138 (the Verus CI
  workflow) and #1139.
- [x] 2026-11-15: wrote revision 3, applying the user's D4 decision and the CI
  notes.
- [x] 2026-11-16: rebased onto `origin/main` `f49104fc4` (#1128, project
  blocklist policy). The range-diff showed all three commits unchanged. #1128
  touches no Verus, cycle, Kani, or proof file. It inserted 14 lines into
  `docs/developers-guide.md` at line 10726, which moved EP-M4's IR cycle
  architecture reference from lines 12975-12980 to 12989-12994. No other
  developers' guide reference cited here moved.
- [ ] User approval.
- [ ] EP-M0 preflight and baselines.
- [ ] EP-M1 specification and hygiene tests.
- [ ] EP-M2 Verus kernel and G1.
- [ ] EP-M3 wiring, or EP-M4R rejection record.
- [ ] EP-M4 documentation and roadmap (acceptance path).
- [ ] Implementation pull request, titled with `(4.4.3)` and linking this
  plan.

## Surprises & discoveries

- 2026-11-15: stale references. Evidence: live docs cite
  `src/ir/cycle/mod.rs` (and older plans cite `src/ir/cycle.rs`), but the code
  is in `src/ir/cycle/support.rs`. Impact: `GAP-STALE-REF`, fixed in EP-M4 or
  EP-M4R; completed plans are left alone.
- 2026-11-15: the documented order is wrong. Evidence: the design document
  and two comments say "lexicographically", but camino orders by components, and
  `a/b` sorts before `a-b`. Impact: `GAP-ORDER` and VC-RANK.
- 2026-11-15: ties have never been verified. Evidence: the Kani harnesses
  assume distinct identifiers, and Proptest uses unique names. Impact:
  `GAP-TIE`. Under AXM-DET, ties do not reach production, so this is a contract
  gap, not a production defect.
- 2026-11-15: the `pop` is redundant. Evidence: `rotate_cycle_by` reads only
  indices below `len`. Impact: M6 and M5c are equivalent mutants.
- 2026-11-15: two tooling gaps. Evidence: `/usr/bin/time` is absent on the
  host, and real Verus runs inside `make test` through the e2e lane once
  installed. Impact: the M0.3 measurement harness and R10.
- 2026-11-15: let-chains are unsupported. Evidence: the panel's prototype on
  Verus `0.2026.03.17` rejected the production let-chain in both editions.
  Impact: R1's single allowed divergence.
- 2026-11-15: a hygiene contradiction. Evidence: revision 1's VC-HYGIENE
  banned the substring `axiom`, which would have rejected `group_seq_axioms`.
  Impact: token-level matching, and a fixture that proves the case passes.

## Decision log

- D1 (2026-11-15). Decision: the mirror is `exec` code over `u64` that
  transcribes the production loops, proved against a declarative specification.
  - Rationale: the 4.2.2 mutation patches seed a flipped comparison, a lost
    push, and a wrong wrap. Those defects live in loop control and index
    arithmetic, which a spec-level model would restate rather than check.
    The `exec` mirror also exposes the real overflow obligation.
  - Rejected: a `#[path]` import of `support.rs`, which drags in camino,
    `BuildGraph`, and `IrHashMap`, violating `RM-443-MODEL`; Verus also cannot
    verify calls through an unspecified generic `fn` comparator.
  - Rejected: a spec-only model, which is cheaper but checks less.
- D2 (2026-11-15). Decision: `u64` with natural order, not a generic `T`.
  - Rationale: like-for-like with Kani's `u8`, and no speculative generics.
    The lift to paths is LEM-RANK, evidenced by VC-RANK.
- D3 (2026-11-15). Decision: prove C5-C8 beyond the Kani clauses.
  - Rationale: C7 and C8 are what "canonical" means and why the code exists
    (`DD-ROT`). C5 makes the generic contract total. All stay inside the
    kernel boundary.
  - The headline justification is C7 and T-CANON for every N, not ties
    (see "Where Verus is warranted").
- D4 (2026-11-15, revised by the user the same day). Decision: on
  acceptance, retire the N=3 and N=4 Kani canonicalization harnesses, their
  builders, and their mutation patches (`UD-D4`). Keep the two-node kernel
  harness and the path-wrapper harness.
  - Revision 2 recommended keeping all of them, because Verus then had no CI
    job. #1138 added `proofs.yml`, which removes that objection, and the user
    chose option (b).
  - The two-node harness stays as the only check of the *compiled* production
    kernel. The Verus kernel covers every length of the mirror. VC-CORRESPOND
    ties the two together.
  - On rejection nothing is retired, because the proof would not exist to
    replace them.
  - Consequence: acceptance is partly substitutive (G1, K4).
- D5 (2026-11-15). Decision: on acceptance, `make verus` defaults to every
  kernel. VC-HYGIENE cross-checks the default against the discovered kernels.
  - Rationale: an accepted kernel the default skips will rot unrun.
- D6 (2026-11-15). Decision: VC-HYGIENE and VC-CORRESPOND run in `make test`.
  - Rationale: they are text checks that need no Verus, they enforce F2, F3,
    N5, and REP mechanically, and they close the unenforced-claims gap the
    6.1.2 plan recorded. `FV-GATES` holds.
- D7 (2026-11-15). Decision: for specification tests against already-correct
  production, the red step is a seeded production mutant applied with
  `git apply`, never a test that fails on the unmodified tree.
- D8 (2026-11-15). Decision: the ADR serial is provisionally 0082, and the
  slug is chosen at G1 ("accept" or "reject"), re-derived at commit and push.
- D9 (2026-11-15). Decision: no `rstest-bdd` feature file and no users'-guide
  change.
  - Rationale: no user-visible behaviour changes. The maintainer scenarios
    are exercised by Make targets and contract tests.
- D10 (2026-11-15). Decision: `ortho_config` is not involved, because no CLI
  flag, configuration key, or localized help changes.
- D11 (2026-11-15). Decision: model the short-input guard so T-CANON is
  total, but not the `debug_assert!` panic, which is a debug-only diagnostic
  for an input AXM-DET excludes.
- D12 (2026-11-15, revised). Decision: on acceptance, ADR-0047 gets an
  `Amended by ADR-0082` banner and an `## Amendments` entry.
  - Rationale: D4 now retires part of the N=2..4 Kani boundary that ADR-0047
    recorded. Revision 2 had said no amendment was needed, which held only
    while every harness was kept.
- D16 (2026-11-15). Decision: keep the explicit two-file `VERUS_FLAGS` batch
  rather than making `make verus` glob `verus/*/main.rs`.
  - Rationale: the explicit list keeps the documented `--proof-file`
    interface and its pinned tests. VC-HYGIENE's cross-check fails `make test`
    whenever a discovered kernel is missing from the default, which closes the
    "passes only when run by hand" risk (`UD-FLAGS`) as firmly as a glob would.
- D17 (2026-11-15). Decision: VC-HYGIENE also checks that each kernel's
  mirrored production path, which VC-CORRESPOND declares
  (`src/ir/cycle/support.rs`), appears in both the `pull_request` and `push`
  path filters of `proofs.yml` (`UD-PATHS`).
  - Rationale: without the filter, a production change would re-check its
    proof only at the weekly run. VC-CORRESPOND catches syntactic drift in
    `make test`, but only CI re-runs the proof itself.
- D13 (2026-11-15). Decision: drift is detected by a structural
  correspondence test, not an insta snapshot or digest.
  - Rationale: a snapshot of production source has no precedent here, can be
    accepted without touching the mirror, and conflicts with `AGENTS.md`
    snapshot guidance. A digest forces a mirror edit but not a correct one.
    The correspondence test checks both sides against a closed rewrite table.
- D14 (2026-11-15). Decision: memory is measured through a cgroup peak via
  `systemd-run --user`, because `/usr/bin/time` is absent and a cgroup covers
  the whole process tree.
- D15 (2026-11-15). Decision: the specification tests (EP-M1) land whatever
  the verdict.
  - Rationale: they close `GAP-ORDER` and `GAP-TIE` and sample `GAP-ALLN`
    without Verus, and they make rejection a complete, useful outcome.

## Outcomes & retrospective

Not started.

## Revision note

- Revision 1 (2026-11-15): initial draft from reconnaissance of `5c707ef33`.
- Revision 2 (2026-11-15): applied the expert-panel review.
  - Acceptance rule and outcomes:
    - Split the acceptance rule into scope fences (by construction) and
      falsifiable criteria (N5-N7, K1-K4), with stage-matched,
      median-of-five measurements and a 0.8 margin.
    - Stated that acceptance is additive, and that the Verus trusted base is
      wider than Kani's.
    - Made rejection a complete outcome (EP-M4R) and dropped "marginal"
      acceptance.
  - Milestones: reordered them so the specification tests land first,
    whatever the verdict.
  - Mirror and drift detection:
    - Replaced the snapshot tripwire with a structural correspondence test.
    - Narrowed the mirror divergence to the let-chain split, which the panel
      found Verus rejects.
  - Proof content:
    - Fixed T-CANON's `ensures` shape, `MAX_MODEL_LEN`, LEM-POP, M5, and the
      witness recipe.
    - Added C8, spec mutants, and mutants M7-M11, and listed the equivalent
      mutants.
    - Reworded AXM-DET in terms of `Equal` under the path order.
    - Replaced the "N verified" count with required proof-function names.
  - Hygiene: switched to token-level matching (the old substring ban would
    have rejected `group_seq_axioms`) and added constructs that weaken
    proofs.
  - Commands and gates:
    - Replaced `/usr/bin/time` with a cgroup measurement and fixed the Kani,
      `--extra-arg`, and ADR-index commands.
    - Added `make typecheck`, plus `make test-workflow-contracts` at EP-M1
      and EP-M3.
  - Plan housekeeping:
    - Added a glossary, gap identifiers, plateau fields, the doc edit sites,
      and the plan PR step.
    - Recalibrated the budgets (code 1,350; documentation 420; files 22) from
      the panel's line estimates and 4.4.2's history.
  - Effect on remaining work: more test code in EP-M1, a stricter gate, and an
    honest completion path for either verdict.
- Revision 3 (2026-11-15): rebased onto `fa06bdb21`, and applied the user's
  direction.
  - The new CI workflow: #1138 added `.github/workflows/proofs.yml`. The plan
    now:
    - records `FV-CI` as amended, and adds `UD-D4`, `UD-FLAGS`, `UD-PATHS`,
      and `UD-INSTALL`;
    - adds `src/ir/cycle/support.rs` to both path filters in EP-M3;
    - extends VC-HYGIENE to check the filters (D17);
    - explains why the explicit `VERUS_FLAGS` batch stays (D16);
    - requires pinned-only Verus installation.
  - The D4 decision: on acceptance, the N=3 and N=4 Kani harnesses, their
    builders, and their patches retire, after the two-node harness is shown
    to kill both patches' faults. ADR-0047 gains an amendment (D12 revised).
    G1 notes that acceptance is now partly substitutive. New risks: R13 and
    R14.
  - Budgets: files rise from 22 to 28 and configuration from 12 to 14 for the
    six added paths. The code and documentation limits are unchanged.
  - Effect on remaining work: EP-M3 gains a workflow edit and a retirement
    commit. EP-M4 gains the ADR-0047 amendment and harness-count corrections.
