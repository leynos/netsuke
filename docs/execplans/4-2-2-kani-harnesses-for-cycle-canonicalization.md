# 4.2.2. Add Kani harnesses for cycle canonicalization

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: IN PROGRESS

## Purpose / big picture

Roadmap item `4.2.2` adds the second substantive set of Kani harnesses to
Netsuke. Where roadmap item `4.2.1` proved cycle *detection* properties of
`cycle::contains_cycle`, this item proves cycle *canonicalization* properties
of the pure normalisation function `canonicalize_cycle` in
[`src/ir/cycle.rs`](../../src/ir/cycle.rs). Canonicalization is what turns a
raw depth-first-search (DFS) cycle witness such as `[c, a, b, c]` into a
stable, reproducible report such as `[a, b, c, a]`, so a bug here would make
Netsuke's circular-dependency diagnostics non-deterministic or wrong even when
detection itself is correct.

`canonicalize_cycle` is a `Vec<Utf8PathBuf> -> Vec<Utf8PathBuf>` transform that
rotates a closed cycle so the lexicographically smallest node appears first,
then re-closes it by appending that node again. It has no `HashMap`, no
`serde`, no Fluent message formatting, and no recursion, so it is the strongest
narrow proof candidate in the repository and a plausible later Verus kernel
under roadmap item `4.4.3`.

After implementation and approval, three classes of property will be exercised
by Kani at bounded but exhaustive coverage, one per roadmap subitem:

- the canonical output preserves the input length and remains a closed cycle
  (its first and last node are identical),
- the multiset of interior nodes is preserved (canonicalization is a pure
  rotation, never adding, dropping, or duplicating a node), and
- the selected start node is stable under the current ordering rule: the first
  node of the canonical output is lexicographically smallest among the interior
  nodes.

The user-visible success criterion is operational: `make kani-ir` (and the
unfiltered `make kani-full`) run a named set of `#[kani::proof]` harnesses to
verification success, each harness fails when its targeted production code path
is deliberately broken (the recorded "mutation discipline" patches), and the
existing `make check-fmt`, `make lint`, `make test`, `make markdownlint`, and
`make nixie` gates continue to pass without change.

This plan is approval-gated. It must be reviewed and explicitly approved before
any source file is edited. The open questions in Stage A list the small number
of decisions that need the user's confirmation.

## Constraints

- The user approved implementation on 2026-06-22. Stage B may proceed using the
  per-node-count harness shape, N ceiling of 4, and ADR-004 inheritance already
  recommended in the Decision Log.
- Do not modify the public Application Programming Interface (API) of the
  `netsuke::ir` module. The
  `pub use graph::{Action, BuildEdge, BuildGraph, IrGenError}` line in
  [`src/ir/mod.rs`](../../src/ir/mod.rs) is the public surface; it must not
  change. `canonicalize_cycle` and its helpers (`find_rotation_start`,
  `rotate_cycle`, `rotate_index`, `path_cmp`) are private to `src/ir/cycle.rs`
  and must stay private; the harnesses reach them through `use super::*` from
  the `#[cfg(kani)]` verification submodule.
- Do not modify `canonicalize_cycle` or any other production code path in
  `src/ir/cycle.rs` to make the harnesses tractable. The existing
  `#[cfg(kani)]` single-byte `path_cmp`/`path_eq` helpers (lines ~475-497) are
  already present from `4.2.1` and are reused as-is. If a production change
  appears necessary, stop and escalate.
- Do not add Kani harnesses, `cfg(kani)` modules, or `[package.metadata.kani]`
  changes to any code outside `src/ir/cycle.rs` and its sibling
  `src/ir/cycle_verification.rs`. Manifest lowering, command interpolation, the
  Ninja generator, and the runner are out of scope. Command interpolation
  harnesses are roadmap item `4.2.3` and must not be drafted here.
- Do not register `kani` as a dependency in `[dependencies]` or
  `[dev-dependencies]`. Kani injects the `kani` crate as a sysroot crate when
  `cargo kani` is the driver; adding it to Cargo manifests breaks ordinary
  `cargo build` and `cargo test`. Adding or amending the
  `[package.metadata.kani]` table is permitted but is not expected, because the
  table, the `cfg(kani)` lint declaration, and `default-unwind = "6"` already
  exist from `4.2.1`.
- Do not add `proptest` coverage as part of this item.
  [`src/ir/cycle_property_tests.rs`](../../src/ir/cycle_property_tests.rs)
  already exercises these exact canonicalization properties at larger N
  (`canonicalize_is_idempotent`, `all_rotations_canonicalize_identically`,
  `canonical_first_node_is_smallest`, `canonical_cycle_is_closed`). The Kani
  harnesses are the bounded-exhaustive complement and must cross-reference
  those proptests, not duplicate or replace them.
- Do not add or modify any user-facing CLI flag, OrthoConfig field, or Fluent
  message. This work is internal to the IR domain.
- Keep [`docs/users-guide.md`](../users-guide.md) unchanged: canonicalization is
  an internal representation detail with no new user-visible behaviour. Update
  [`docs/developers-guide.md`](../developers-guide.md) to extend the existing
  "Kani harness inventory" table. Update
  [`docs/formal-verification-methods-in-netsuke.md`](../formal-verification-methods-in-netsuke.md)
  only if a footnote is needed to record the bound; do not rewrite its
  recommendation.
- Decide at Stage A whether to extend the existing
  [`docs/adr-004-bound-kani-ir-harnesses-to-small-n.md`](../adr-004-bound-kani-ir-harnesses-to-small-n.md)
  or add a new ADR. The recommendation (see Decision Log) is to inherit
  ADR-004, because `4.2.2` makes no new architectural choice; it applies the
  same small-bounded-N decision along a different bounding axis (cycle length
  rather than map size).
- Documentation prose must follow
  [`docs/documentation-style-guide.md`](../documentation-style-guide.md) and
  use en-GB-oxendict spelling and grammar.
- Run long validation commands sequentially. Do not run format checks, lints, or
  tests in parallel. Capture each command's output with `tee` under `/tmp`
  using the filename template described in `AGENTS.md`.
- Run every Kani command under an explicit systemd resource cap. The wrapper is:

  ```sh
  timeout --kill-after=20s 5m \
    systemd-run \
      --user \
      --scope \
      --expand-environment=no \
      -p CPUQuota=200% \
      -p MemoryMax=8G \
      -p MemorySwapMax=0 \
      -p TasksMax=96 \
      -p IOWeight=20 \
      /usr/bin/nice -n 15 \
      <kani-command>
  ```

  Include the known Kani `LD_LIBRARY_PATH` inside `<kani-command>` when invoking
  `cargo kani`, `make kani-ir`, or `make kani-full`. On this host, the
  original root-system scope form required interactive authentication and
  `-p Nice=15` was not accepted as a unit property; the user-scope wrapper
  above applies the available CPU, memory, swap, task, and I/O caps and runs
  the verifier process through `nice`. Do not run uncapped Kani, CBMC, or
  solver commands again.
- Use `coderabbit review --agent` after each major implementation milestone, and
  clear all concerns before moving to the next. Run it only after the
  deterministic gates pass.
- Commit only after gates pass. Use the file-based commit-message workflow (the
  `commit-message` skill); do not pass `-m`. Skipping hooks (`--no-verify`) is
  forbidden. Do not amend prior commits to fix issues; create new commits.

## Tolerances (exception triggers)

- Scope: if implementation requires changes to more than 8 files beyond this
  ExecPlan, stop and escalate. The expected implementation files are
  `src/ir/cycle_verification.rs`, `docs/developers-guide.md`,
  `docs/roadmap.md`, the chosen ADR file (and `docs/contents.md` if a new ADR
  is added), the three per-harness mutation patch files under
  `docs/verification/mutations/`, and this ExecPlan. `src/ir/cycle.rs`
  production code is **not** expected to change; if it must, that is an
  interface/constraint exception, not merely a scope one.
- Interface: if any change to the public API of `netsuke::ir`, or any widening
  of `canonicalize_cycle` or its helpers beyond their current private
  visibility, becomes necessary to make the harnesses compile, stop and present
  options.
- Dependencies: if any new `[dependencies]`, `[dev-dependencies]`, or
  `[build-dependencies]` entry is required, stop and escalate.
- Solver runtime: if any single harness takes more than five wall-clock minutes
  on the reference machine (six-core Rocky 10, 64 GB RAM), stop and consider
  narrowing the node bound, reducing the alphabet, or splitting the harness.
  `canonicalize_cycle` is allocation-light and operates on tiny fixed-length
  vectors, so each harness is expected to complete in seconds; a multi-minute
  run signals a modelling mistake (for example, symbolic-length vectors or
  symbolic strings) rather than inherent cost.
- Mutation discipline: if any harness still passes after the matching production
  code path is deliberately broken (see Stage E), stop and redesign the
  harness. A passing mutation is a falsified proof. Each mutation is recorded
  as a literal patch file at
  `docs/verification/mutations/<harness-name>.patch`, so future maintainers can
  replay it. If a recorded mutation ceases to apply because of an intervening
  refactor, the harness must be re-validated against an updated patch in the
  same commit as the refactor.
- Lint friction: if Clippy lints in `Cargo.toml` (notably `unwrap_used`,
  `expect_used`, `indexing_slicing`, `panic_in_result_fn`,
  `missing_docs_in_private_items`) cannot be satisfied or scoped with a
  `reason = "..."` clause, stop and escalate before adding broad
  `#[allow(...)]` umbrellas. `allow_attributes_without_reason = "deny"` is in
  force.
- Validation: if `make check-fmt`, `make lint`, or `make test` fails after two
  focused fix attempts, stop and escalate with the captured `/tmp` log paths.
- Review: if `coderabbit review --agent` raises unresolved correctness, testing,
  or documentation concerns, do not proceed until they are addressed or
  explicitly waived. The `4.2.1` work observed the review service stalling at
  `preparing_sandbox`; if that recurs, record it and proceed on the strength of
  the deterministic gates and Kani verification rather than blocking
  indefinitely.
- Ambiguity: if `make kani-full` cannot complete the full cumulative harness set
  (the nine `4.2.1` harnesses plus the new canonicalization harnesses) in under
  thirty minutes, stop and propose splitting the smoke and full gates or adding
  a filtered `make kani-cycle-canon` target.

## Risks

- Risk: symbolic `Vec<Utf8PathBuf>` or symbolic strings would blow up the
  solver, as confirmed by Kani's own guidance (string problems scale poorly
  past 10-20 characters; symbolic string indexing has been observed consuming
  tens of gigabytes). Severity: high. Likelihood: medium. Mitigation: never use
  symbolic strings or symbolic-length vectors. Each harness fixes the node
  count N concretely and draws one symbolic byte per node from a tiny alphabet,
  reusing the existing `#[cfg(kani)]` single-byte `path_cmp`/`path_eq` helpers
  so node identity reduces to integer comparison.

- Risk: bounded proofs are incomplete. A property could hold for N in {2, 3, 4}
  but fail for a larger cycle. Severity: medium. Likelihood: low. Mitigation:
  the proptest suite in `cycle_property_tests.rs` already covers the same
  properties for cycles up to length 8-10, so the larger-N assurance is
  retained outside Kani. The developers' guide and the ADR record that Kani
  covers small N and Proptest covers the tail, mirroring the `4.2.1`
  reconciliation.

- Risk: choosing the wrong unwind bound. `rotate_cycle` and the multiset-count
  loops iterate over the cycle, so an under-set `#[kani::unwind(N)]` yields a
  failed unwinding assertion and "undetermined" checks. Severity: low.
  Likelihood: medium. Mitigation: start at `closed_length + 1` and increase by
  one or two if an unwinding assertion fires (a `break`/`continue` can need two
  or three extra iterations). An under-set bound fails loudly; it never
  produces a false pass.

- Risk: the input precondition. `canonicalize_cycle` documents that the input
  must contain at least two nodes with identical first and last elements and
  carries a `debug_assert!`. A harness that fed an open or too-short vector
  would prove nothing useful. Severity: medium. Likelihood: low. Mitigation:
  harness helpers build the closed cycle by construction
  (`vec![n0, n1, ..., n0]`) rather than `assume`-ing a symbolic vector into
  shape, so the precondition holds without wasted symbolic search.

- Risk: the function consumes its input by value (`mut cycle`) and calls
  `cycle.pop()`, so the post-call assertions cannot read the original interior
  from the moved vector. Severity: low. Likelihood: medium. Mitigation: capture
  the interior slice (or per-symbol counts) into owned values before the call,
  and pass a clone into `canonicalize_cycle`.

- Risk: future contributors might fold Kani into `make test`, `make lint`, or
  `make check-fmt`, breaking the cache and runtime tolerances for ordinary
  builds. Severity: medium. Likelihood: medium. Mitigation: the developers'
  guide already states Kani is not part of those gates; the new inventory rows
  do not change that.

- Risk: ADR numbering collision. The repository already contains several files
  numbered `adr-004-*` (a known artefact of parallel branches). Severity: low.
  Likelihood: low. Mitigation: this plan inherits the existing
  `adr-004-bound-kani-ir-harnesses-to-small-n.md` rather than minting a new
  number; if Stage A elects a new ADR, the next genuinely free number must be
  confirmed against `docs/contents.md` before writing it.

## Progress

- [x] (2026-06-20T00:00:00Z) Loaded the `leta`, `rust-router`,
      `hexagonal-architecture`, and `execplans` skills for this planning task.
- [x] (2026-06-20T00:00:00Z) Created a `leta` workspace for the repository
      worktree.
- [x] (2026-06-20T00:00:00Z) Reviewed `docs/roadmap.md` §4.2.2,
      `docs/formal-verification-methods-in-netsuke.md` §Optional Verus proof
      kernel, the predecessor execplan `4-2-1-*.md`, `src/ir/cycle.rs`
      (canonicalization helpers and `#[cfg(kani)]` path helpers),
      `src/ir/cycle_verification.rs`, `src/ir/cycle_property_tests.rs`,
      `docs/developers-guide.md` §Kani harness inventory, `Makefile` Kani
      targets, and the existing mutation patches under
      `docs/verification/mutations/`.
- [x] (2026-06-20T00:00:00Z) Ran a Plan-agent to design the harness milestone
      structure and a Firecrawl-backed research agent to refresh Kani prior art
      (bounded symbolic vectors, permutation/multiset encodings, unwind bounds,
      solver choice).
- [x] (2026-06-20T00:00:00Z) Drafted this approval-gated ExecPlan and ran a
      Logisphere community-of-experts design review, revising the plan to
      address each finding.
- [x] (2026-06-22T21:41:49Z) Stage A: implementation was explicitly approved by
      the user. The combined-per-N harness shape, N ceiling of 4, and ADR-004
      inheritance are accepted for implementation.
- [x] (2026-06-22T21:42:47Z) Stage B (red): added deliberately failing
      canonicalization harness scaffolds to `src/ir/cycle_verification.rs`.
      The expected failure is the scaffold assertion
      `output.len() == 0`, which proves Kani reaches the new harness bodies
      before the real properties replace them.
- [x] (2026-06-22T21:44:50Z) Stage B discovery: `cargo kani list` initially
      failed because `kani-compiler` could not load
      `libLLVM.so.21.1-rust-1.93.0-nightly` when invoked from Cargo build
      scripts. `make kani-check` confirmed the pinned `0.67.0` version, and
      rerunning discovery with
      `LD_LIBRARY_PATH=/home/leynos/.kani/kani-0.67.0/toolchain/lib:/home/leynos/.kani/kani-0.67.0/lib`
      succeeded. The output at
      `/tmp/kani-list-netsuke-4-2-2-kani-harnesses-for-cycle-canonicalization-stage-b.out`
      lists 12 harnesses, including the three new canonicalization stubs.
- [x] (2026-06-22T21:47:16Z) Stage B red run: `make kani-ir` with the explicit
      Kani `LD_LIBRARY_PATH` completed the full harness set and reported
      "9 successfully verified harnesses, 3 failures, 12 total". The only
      failures were the deliberately false red-stage assertions in
      `canonicalize_two_node_cycle_is_canonical`,
      `canonicalize_three_node_cycle_is_canonical`, and
      `canonicalize_four_node_cycle_is_canonical`. Evidence:
      `/tmp/kani-ir-netsuke-4-2-2-kani-harnesses-for-cycle-canonicalization-stage-b.out`.
- [x] (2026-06-22T21:49:42Z) Stage B deterministic gates passed after the red
      scaffold: `make check-fmt`, `make lint`, `make test`,
      `make markdownlint`, and `make nixie`. Evidence logs:
      `/tmp/check-fmt-netsuke-4-2-2-kani-harnesses-for-cycle-canonicalization-stage-b.out`,
      `/tmp/lint-netsuke-4-2-2-kani-harnesses-for-cycle-canonicalization-stage-b.out`,
      `/tmp/test-netsuke-4-2-2-kani-harnesses-for-cycle-canonicalization-stage-b.out`,
      `/tmp/markdownlint-netsuke-4-2-2-kani-harnesses-for-cycle-canonicalization-stage-b.out`,
      and
      `/tmp/nixie-netsuke-4-2-2-kani-harnesses-for-cycle-canonicalization-stage-b.out`.
- [x] (2026-06-22T21:52:06Z) Stage B CodeRabbit review completed with
      zero findings. Evidence:
      `/tmp/coderabbit-netsuke-4-2-2-kani-harnesses-for-cycle-canonicalization-stage-b.out`.
- [x] Stage B (red): add the placeholder harnesses to
      `src/ir/cycle_verification.rs`, confirm `cargo kani list` discovers them,
      and confirm `make kani-ir` runs them.
- [ ] (2026-06-22T21:53:25Z) Stage C length/closure: replaced the false
      scaffold assertions with symbolic closed-cycle construction and real
      `canonicalize_cycle` length and closure assertions for N in {2, 3, 4}.
      The new helpers are private to `src/ir/cycle_verification.rs`, are owned
      by the Kani harnesses only, and must not become production abstractions.
- [ ] (2026-06-22T22:27:40Z) Stage C resource recovery: confirmed no Kani,
      CBMC, solver, or `make kani` processes remained after the interrupted
      uncapped run. System resources were healthy (`free -h` reported 122 GiB
      available memory; `df -h . /tmp` reported 35% filesystem use). The
      interrupted run showed symbolic `char`/`String` construction was too
      expensive, so `symbolic_node` now uses a symbolic selector over concrete
      one-byte paths. All future Kani commands must use the resource-capped
      `systemd-run` wrapper recorded in Constraints.
- [ ] (2026-06-22T22:42:51Z) Stage C focused proof: the two-node
      length-and-closure harness passed under the user-scope resource cap with
      the explicit Kani `LD_LIBRARY_PATH`. Evidence:
      `/tmp/kani-two-netsuke-4-2-2-kani-harnesses-for-cycle-canonicalization-stage-c-length-selector.out`.
      The run took 177.3307 seconds, which is under the five-minute timeout but
      slow enough that three- and four-node harnesses must be tried one at a
      time and treated as tractability evidence before expanding the property
      set.
- [ ] (2026-06-22T22:58:13Z) Stage C tractability finding: the first
      three-node length-and-closure run, still using a four-symbol alphabet,
      reached SAT conversion and then hit the 8G user-scope `MemoryMax`.
      `systemd` reported `Result: oom-kill`, 8G peak memory, and 35.288 seconds
      CPU. Evidence:
      `/tmp/kani-three-netsuke-4-2-2-kani-harnesses-for-cycle-canonicalization-stage-c-length-selector.out`
      plus the user-unit journal for `run-p9576-i9876.scope`. The harness model
      now uses an alphabet of size N for each N-node harness, because N ordered
      symbols cover every equality/order pattern among N interior nodes while
      avoiding redundant smaller-harness states.
- [ ] (2026-06-22T23:14:11Z) Stage C second tractability finding: the reduced
      three-node length-and-closure run with alphabet size 3 still reached
      213332 symbolic steps, 2707 verification conditions after simplification,
      and then hit the 8G user-scope `MemoryMax`. `systemd` reported
      `Result: oom-kill`, 8G peak memory, and 41.280 seconds CPU for
- [ ] (2026-06-22T23:19:46Z) Stage C rejected fallback: concrete finite
      enumeration over the planned alphabets was tested as an alternative to
      symbolic selectors. It was worse: even the two-node harness reached
      216692 symbolic steps, 4096 verification conditions after
      simplification, and hit the 8G user-scope `MemoryMax`. `systemd` reported
      `Result: oom-kill`, 8G peak memory, and 54.441 seconds CPU for
      `run-p52626-i52926.scope`. The code has been reverted to the N-sized
      symbolic-selector model, where N=2 verifies but N=3 exceeds the mandated
      resource cap.
- [ ] Stage C (green): implement the length-and-closure, interior-multiset, and
      stable-start harnesses, one commit per property.
- [ ] (2026-06-22T23:24:32Z) Stage C is blocked under the current tolerances:
      the direct end-to-end `canonicalize_cycle(Vec<Utf8PathBuf>)` proof shape
      verifies N=2 but cannot verify N=3 inside the required 8G memory cap. Do
      not attempt N=4 or the interior-multiset/stable-start properties in this
      shape until the tolerance is changed or the proof strategy is narrowed.
      Viable next decisions are: lower the Kani bound to N=2 and rely on
      Proptest for N>=3; allow `--no-memory-safety-checks` for this functional
      canonicalization proof; allow a Kani-only non-allocating model of the
      canonicalization algorithm and cover production with mutation tests; or
      raise the memory cap for Kani. The current implementation agent has not
      taken any of those policy decisions.
- [ ] Stage D (refactor and docs): extract shared harness helpers, extend the
      harness inventory in `docs/developers-guide.md`, add the three mutation
      patches, and update the chosen ADR.
- [ ] Stage E (validate and review): run mutation discipline per harness, run
      the deterministic gates and `make kani-ir`, and run
      `coderabbit review --agent`.
- [ ] Stage F (PR and roadmap): mark roadmap `4.2.2` and its three subitems
      done, push the branch, and update the draft pull request.

## Surprises & Discoveries

- Observation: the three roadmap properties for `4.2.2` are already asserted by
  the existing proptest suite in `cycle_property_tests.rs`
  (`canonical_cycle_is_closed`, the implicit length preservation in
  `all_rotations_canonicalize_identically`, and
  `canonical_first_node_is_smallest`). Impact: the Kani work is a
  bounded-exhaustive complement, not new coverage of an untested function; the
  plan cross-references the proptests rather than re-deriving the properties.

- Observation: `canonicalize_cycle` has no `HashMap`, `serde`, Fluent, or
  recursion. Evidence: `src/ir/cycle.rs:424-443`. Impact: the budget hazards
  that forced the `4.2.1` cycle-detection harnesses down to a production
  `contains_cycle` boolean entry point do not apply here; the function can be
  harnessed end-to-end at its natural contract boundary.

- Observation: Kani 0.55+ provides `kani::vec::exact_vec::<T, N>()` and
  `kani::bounded_any::<T, N>()` for bounded symbolic vectors, and the official
  reference templates a bounded "reverse is its own inverse" involution proof
  over `Vec<bool>`. Evidence: Kani BoundedArbitrary reference. Impact: the plan
  prefers fixed-length-per-harness construction (one symbolic byte per node)
  over these APIs for clarity and minimal unwind cost, but records them as the
  sanctioned alternative if a single variable-length harness is later wanted.

- Observation: because canonicalization is specifically a *rotation*, "interior
  multiset preserved" can be encoded either as per-alphabet-symbol count
  equality or as the existential rotation statement
  `∃k: out[i] == in[(i+k) % N]`. Evidence: research agent findings; Kani
  permutation prior art (the verify-rust-std SmallSort challenge). Impact: the
  plan uses the count-based encoding to match the roadmap wording ("interior
  node multiset is preserved") *and* adds the rotation statement as a stronger
  superset, after the Logisphere review flagged that the three roadmap
  properties alone do not pin cyclic order.

- Observation: the interior-multiset property cannot be falsified by a small
  production mutation in isolation, because a rotation always preserves the
  multiset; any structural break that disturbs it also disturbs length or the
  rotation. Evidence: Logisphere (Telefono) analysis of the three candidate
  mutations. Impact: the multiset assertion is honestly documented as co-proven
  with length and the rotation check, rather than backed by a contrived
  isolating patch; the rotation strengthening is what makes the harness
  non-vacuous here.

- Observation: `symbolic_node` must construct its one-byte path name without
  `unwrap`/`expect`, because `Cargo.toml` denies those Clippy lints and
  `allow_attributes_without_reason = "deny"` is in force. Evidence: the existing
  `Cargo.toml` lint set and the `4.2.1` lint-friction lessons. Impact: the
  helper originally built the name via `(b as char).to_string()` over an
  `assume`-constrained ASCII byte, which satisfied Clippy but made Kani explore
  symbolic character and string encoding. The implementation changed to a
  symbolic selector over concrete `path("a")` through `path("d")` values to
  keep the proof input one-byte and avoid symbolic string construction.

- Observation: the installed Kani 0.67.0 binary is version-correct but Cargo
  build scripts could not load Kani's bundled LLVM library until
  `LD_LIBRARY_PATH` included `/home/leynos/.kani/kani-0.67.0/toolchain/lib` and
  `/home/leynos/.kani/kani-0.67.0/lib`. Evidence:
  `/tmp/kani-list-netsuke-4-2-2-kani-harnesses-for-cycle-canonicalization-stage-b.out`
  after retry, and
  `/tmp/kani-check-netsuke-4-2-2-kani-harnesses-for-cycle-canonicalization-stage-b.out`.
  Impact: Kani validation commands in this implementation run with that
  explicit library path; no repository configuration is changed.

- Observation: uncapped Kani runs can consume excessive workstation resources
  before the five-minute solver-runtime tolerance is reached. Evidence: the
  interrupted Stage C length/closure run became silent during the four-node
  harness after entering the solver, and the user subsequently required
  resource capping. Impact: the solver-runtime tolerance remains, but every
  Kani invocation is now additionally constrained by `timeout`, a transient
  `systemd-run --user --scope` with CPU, memory, task, and I/O weight limits,
  and `/usr/bin/nice -n 15`.

- Observation: replacing symbolic `char`/`String` construction with a symbolic
  selector over concrete one-byte paths makes the two-node length-and-closure
  proof complete under the cap, but it still takes 177.3307 seconds. Impact:
  continue Stage C by running N=3 and N=4 independently under the same cap; if
  either exceeds the five-minute timeout, record that as a bounded-model
  tractability limit before widening or changing the proof model.

- Observation: the first capped three-node length-and-closure run exceeded the
  8G memory cap before producing a verdict when every node used the four-symbol
  alphabet. Impact: smaller harnesses now use an N-sized alphabet (`a|b` for
  N=2, `a|b|c` for N=3, and `a|b|c|d` for N=4). This preserves the bounded
  order/equality cases relevant to `canonicalize_cycle` while reducing
  redundant symbolic states.

- Observation: the N-sized symbolic alphabet did not materially reduce the
  three-node solver problem; it still reached 2707 verification conditions and
  hit the 8G memory cap. Concrete finite enumeration was tried next and hit the
  8G cap even for N=2 because it duplicated the allocation-heavy production
  paths across cases. Impact: keep the symbolic-selector model as the least bad
  direct encoding, but treat N=3 as blocked under the required resource cap.

## Decision Log

- Decision: keep this ExecPlan pre-implementation and approval-gated. Rationale:
  the user stated the plan must be approved before implementation. Date/Author:
  2026-06-20 / planning agent.

- Decision: harness `canonicalize_cycle` end-to-end rather than harnessing
  `find_rotation_start`, `rotate_cycle`, and `rotate_index` in isolation.
  Rationale: the three roadmap properties are all stated about the *output of
  `canonicalize_cycle`*; harnessing the helpers separately would prove weaker
  helper-local statements rather than the contract, and the function is cheap
  enough to verify whole. The helpers receive incidental coverage through the
  end-to-end harnesses and through the mutation patches that break them.
  Date/Author: 2026-06-20 / planning agent.

- Decision: construct symbolic input as a fixed-length closed cycle with one
  symbolic byte per node, drawn from a tiny alphabet, rather than a
  symbolic-length vector or symbolic strings. Rationale: symbolic strings and
  symbolic-length heap structures are the documented Kani budget hazards; a
  fixed N with single-byte node identity reuses the existing `#[cfg(kani)]`
  `path_cmp` comparison and keeps each proof to a handful of symbolic bytes.
  During implementation the alphabet was refined to size N per N-node harness,
  because an N-symbol total order is sufficient to cover all equality/order
  patterns among N interior nodes. After the three-node symbolic-selector proof
  hit the 8G cap twice, concrete finite enumeration was tested and rejected
  because it hit the 8G cap even for N=2. Date/Author: 2026-06-20 / planning
  agent; refined 2026-06-22 / implementation agent.

- Decision: add a fourth, stronger "output is a rotation of the input interior"
  assertion to each harness, on top of the three roadmap properties. Rationale:
  a Logisphere review (Telefono) observed that length, closure, multiset, and
  smallest-first together do not pin the cyclic order, so an order-scrambling
  bug could satisfy all three roadmap subitems vacuously. The rotation assertion
  (`∃k: out[i] == in[(i+k) % N]`) is cheap for N <= 4, closes that gap, and
  gives the otherwise co-proven multiset property a clean falsification path.
  The count-based multiset assertion is retained verbatim as the record of
  roadmap subitem two. Date/Author: 2026-06-20 / planning agent after
  Logisphere review.

- Decision: split by node count N rather than by property, with one harness per
  N asserting the three roadmap properties plus the rotation strengthening, and
  bind N to {2, 3, 4} interior nodes. Rationale: one solver invocation per
  symbolic input covers every assertion cheaply, matching the `4.2.1` precedent
  of one harness per concrete shape (`two_node_cycle_reports_cycle_a_first`/
  `_b_first`). The mutation patches show which break each harness catches.
  Date/Author: 2026-06-20 / planning agent. **Stage A open question:** confirm
  the combined-per-N shape and the N ceiling of 4.

- Decision: allow repeated node bytes (do not require distinct nodes).
  Rationale: `canonicalize_cycle` itself does not require distinct nodes; the
  proptest suite filters for uniqueness, but admitting ties strengthens the
  Kani proof and forces the `find_rotation_start` tie-break (keep the first
  minimal index) to be exercised. The stable-start assertion therefore uses
  `path_cmp(..) != Ordering::Greater` rather than strict `Less`. Date/Author:
  2026-06-20 / planning agent.

- Decision: encode interior-multiset preservation as per-alphabet-symbol count
  equality. Rationale: it matches the roadmap's exact wording and is cheaper
  than general multiset machinery over a fixed alphabet; the
  rotation-existential encoding is recorded as an alternative in Surprises &
  Discoveries. Date/Author: 2026-06-20 / planning agent.

- Decision: use `#[kani::solver(kissat)]` for every new harness. Rationale:
  consistency with all existing `4.2.1` harnesses and the documented strong
  bounded-SAT performance of kissat; for N <= 4 the proofs are tiny, so solver
  choice is unlikely to matter, and the attribute is pinned rather than left to
  the version-dependent default. Date/Author: 2026-06-20 / planning agent.

- Decision: inherit ADR-004 rather than write a new ADR. Rationale: `4.2.2`
  makes no new, hard-to-reverse architectural choice. It applies ADR-004's
  small-bounded-N-with-Proptest-hand-off decision along a different bounding
  axis (cycle length, not map size), introduces no public API, no collection
  port, and no contract change (there is no hasher or serde to stub). The plan
  extends ADR-004's consequences with one sentence recording the
  canonicalization bounding axis and its proptest tail. Date/Author: 2026-06-20
  / planning agent. **Stage A open question:** confirm inheriting ADR-004
  versus minting a new ADR.

- Decision: proceed with implementation from this ExecPlan on 2026-06-22.
  Rationale: the user explicitly instructed implementation of the planned
  functionality, including keeping this ExecPlan current, using CodeRabbit
  after major milestones, and committing frequently. This satisfies the
  approval gate and accepts the open Stage A recommendations: one harness per
  node count for N in {2, 3, 4}, plus inheriting ADR-004 rather than creating a
  new ADR. Date/Author: 2026-06-22 / implementation agent.

- Decision: keep `symbolic_node`, `close_cycle`, and the per-N closed-cycle
  builders as private Kani harness helpers in `src/ir/cycle_verification.rs`.
  Rationale: these helpers model bounded proof inputs only. They are not a
  domain abstraction, are not reusable outside the verification module, and
  must not widen `canonicalize_cycle` or its private helpers. This satisfies
  the repository's helper policy without adding a production port. Date/Author:
  2026-06-22 / implementation agent.

- Decision: execute all future Kani commands through the resource-capped
  `timeout` and `systemd-run --user --scope` wrapper adapted from the user's
  required cap. Rationale: bounded model checking can still stress CPU and
  memory while a single harness remains under the logical runtime tolerance.
  The root-system scope form requires interactive authentication on this host,
  and `Nice` is not accepted as a unit property here, so the user-scope command
  applies the available systemd CPU, memory, swap, task-count, and I/O caps and
  delegates niceness to `/usr/bin/nice -n 15`. Date/Author: 2026-06-22 /
  implementation agent.

## Outcomes & Retrospective

To be completed at major milestones and at completion. Compare the result
against the purpose: bounded-exhaustive Kani proofs of length-and-closure,
interior-multiset preservation, and stable-start selection for
`canonicalize_cycle`, with the deterministic gates and the larger-N proptests
left intact.

## Context and orientation

Netsuke is a Rust build-system compiler. It reads a YAML Ain't Markup Language
(YAML) `Netsukefile`, expands MiniJinja-controlled manifest logic, lowers the
result into a static Intermediate Representation (IR), emits a deterministic
Ninja file, and delegates execution to the Ninja subprocess. The IR is the
semantic commitment point; once constructed, downstream code treats it as
authoritative.

Cycle handling lives in [`src/ir/cycle.rs`](../../src/ir/cycle.rs):

- `CycleDetector` and the `cycle::analyse` entry point traverse `BuildEdge`
  inputs and implicit dependencies to find a circular dependency. When a cycle
  is found, the DFS yields a raw witness path whose first and last nodes are
  the same (the standard closed-cycle representation).
- `canonicalize_cycle` (lines ~424-443) normalises that raw witness so the
  diagnostic is stable: it computes the rotation start with
  `find_rotation_start` (the index of the lexicographically smallest interior
  node), pops the duplicated closing node, and rebuilds the closed cycle with
  `rotate_cycle` (which uses `rotate_index` for wrap-around). The net effect is
  that all rotations of the same cycle, in either traversal direction's
  starting point, collapse to one canonical sequence beginning at the smallest
  node.
- Under `#[cfg(kani)]`, `path_cmp` and `path_eq` (lines ~475-497) compare paths
  by their single leading byte. This is the device that keeps node identity
  cheap for the model checker; the harnesses rely on it and must use one-byte
  node names.

The current canonicalization signature and helpers, for reference:

```rust
// src/ir/cycle.rs
fn find_rotation_start(cycle: &[Utf8PathBuf], len: usize) -> usize;
fn rotate_cycle(cycle: &[Utf8PathBuf], start: usize, len: usize) -> Vec<Utf8PathBuf>;
fn canonicalize_cycle(mut cycle: Vec<Utf8PathBuf>) -> Vec<Utf8PathBuf>;
const fn rotate_index(start: usize, offset: usize, len: usize) -> usize;
```

The existing Kani harnesses for cycle *detection* live in
[`src/ir/cycle_verification.rs`](../../src/ir/cycle_verification.rs), declared
at the foot of `src/ir/cycle.rs` as:

```rust
#[cfg(kani)]
#[path = "cycle_verification.rs"]
mod verification;
```

That module begins with `use super::*;`, so it already reaches the private
`canonicalize_cycle`, `find_rotation_start`, `rotate_cycle`, `path_cmp`, and
`Ordering` symbols. The new harnesses are added to this same file.

The relevant supporting files are:

- [`src/ir/cycle_property_tests.rs`](../../src/ir/cycle_property_tests.rs): the
  proptest suite that already covers idempotence, rotation-invariance,
  smallest-first, and closure for cycles up to length 8-10. The Kani harnesses
  are its bounded-exhaustive complement.
- [`Cargo.toml`](../../Cargo.toml): the strict Clippy lint set, the
  `[lints.rust] unexpected_cfgs` declaration for `cfg(kani)`, and
  `[package.metadata.kani.flags] default-unwind = "6"`. None of these need to
  change.
- [`Makefile`](../../Makefile): `make kani-full` runs the bare cumulative
  `cargo kani` suite and `make kani-ir` aliases it. A filtered
  `make kani-cycle-canon` may be added if Stage A elects a per-feature target.
- [`docs/developers-guide.md`](../developers-guide.md): the "Kani harness
  inventory" table (rows around lines 186-196) that this plan extends.
- [`docs/adr-004-bound-kani-ir-harnesses-to-small-n.md`](../adr-004-bound-kani-ir-harnesses-to-small-n.md):
  the decision this plan inherits.
- [`docs/verification/mutations/`](../verification/mutations/): the directory of
  literal mutation patches, one per harness.

Architectural rules this plan applies (concrete invariants, not pattern
transplant): the IR is the part of the codebase with no input/output of its own;
`canonicalize_cycle` is a pure function within it; Kani harnesses are added as
a private `#[cfg(kani)]` module inside the file they verify; no private symbol
is widened to make a harness compile; and the existing developer gates remain
unaffected by harness code, which compiles only under `--cfg kani`.

## Skills and references

Use these skills while implementing this plan:

- `execplans`: keep this document current as work proceeds.
- `kani`: harness shape, the four-phase pattern (deterministic setup,
  nondeterministic population, precondition `assume`, invariant `assert`),
  unwind discipline, mutation discipline, and the "narrowest function that
  still proves the contract" rule applied here at the function's contract
  boundary.
- `rust-verification`: justify the Kani-versus-Proptest split for the larger-N
  property.
- `hexagonal-architecture`: protect the IR domain boundary by forbidding any
  visibility widening for verification; use the skill to police drift rather
  than as a pattern source.
- `arch-decision-records`: if Stage A elects a new ADR, write it with the
  project's Y-statement template; otherwise extend ADR-004.
- `rust-unit-testing`: model harness helpers after the existing `path` and
  `edge` helpers in `src/ir/cycle_verification.rs`.
- `leta`: use for symbol navigation when extracting helpers.
- `firecrawl`: use again if implementation needs fresh external Kani facts.
- `commit-message`: use the file-based commit workflow for every commit.
- `pr-creation`: use when creating and updating the draft pull request.
- `en-gb-oxendict`: applies to all prose written or revised by this plan.
- `code-review`: align the `coderabbit review --agent` pass at Stage E with the
  skill's expectations.

Primary local references:

- [`docs/roadmap.md`](../roadmap.md) §4.2.2.
- [`docs/formal-verification-methods-in-netsuke.md`](../formal-verification-methods-in-netsuke.md)
  §Optional Verus proof kernel (the cycle canonicalization contract: length
  preserved, cycle closed, interior multiset preserved, chosen start node
  stable).
- [`docs/execplans/4-2-1-kani-harnesses-for-manifest-to-ir-safety-checks.md`](4-2-1-kani-harnesses-for-manifest-to-ir-safety-checks.md):
  the predecessor whose structure, conventions, and lessons this plan mirrors.
- [`docs/developers-guide.md`](../developers-guide.md) §Kani harness inventory.
- [`docs/adr-004-bound-kani-ir-harnesses-to-small-n.md`](../adr-004-bound-kani-ir-harnesses-to-small-n.md).
- [`docs/documentation-style-guide.md`](../documentation-style-guide.md)
  §Architecture Decision Records.

External references consulted during planning:

- Kani usage guide: <https://model-checking.github.io/kani/usage.html>.
- Kani BoundedArbitrary reference (bounded vectors, the reverse-involution
  template, the incompleteness caution):
  <https://model-checking.github.io/kani/reference/bounded_arbitrary.html>.
- Kani `kani::vec` module (`any_vec`, `exact_vec`):
  <https://model-checking.github.io/kani/crates/doc/kani/vec/index.html>.
- Kani attributes reference (`#[kani::unwind]`, `#[kani::solver]`, the small
  array sort example):
  <https://model-checking.github.io/kani/reference/attributes.html>.
- Kani loop-unwinding tutorial (unwind = iterations + 1; string scaling limits):
  <https://model-checking.github.io/kani/tutorial-loop-unwinding.html>.
- Kani nondeterministic-variables tutorial (heap-structure scaling caution):
  <https://model-checking.github.io/kani/tutorial-nondeterministic-variables.html>.
- Kani "Turbocharging Rust Code Verification" (solver comparison; kissat and
  CaDiCaL versus MiniSat):
  <https://model-checking.github.io/kani-verifier-blog/2023/08/03/turbocharging-rust-code-verification.html>.
- verify-rust-std SmallSort challenge (permutation/sortedness prior art):
  <https://model-checking.github.io/verify-rust-std/challenges/0008-smallsort.html>.

## Plan of work

Stages run in order. Each stage ends with validation. Do not advance unless the
validation passes.

### Stage A — Approval gate (no code changes)

Present this draft to the user. Resolve the open questions listed under "Open
questions". Do not begin Stage B until the user explicitly approves the plan.
If the user changes scope, revise this ExecPlan before any source file edit.

### Stage B — Red: scaffold the verification surface

Make the smallest change that compiles and exposes the new harnesses to Kani's
discovery, proving the harness loop reaches the new names.

1. In `src/ir/cycle_verification.rs`, add three harness stubs named
   `canonicalize_two_node_cycle_is_canonical`,
   `canonicalize_three_node_cycle_is_canonical`, and
   `canonicalize_four_node_cycle_is_canonical`, each with the
   `#[kani::proof] #[kani::solver(kissat)] #[kani::unwind(...)]` attributes. To
   honour the `4.2.1` lesson against vacuous `kani::assert(true, ...)` proofs,
   write each stub to assert a deliberately false shape (for example, assert
   the output length is wrong) so the red stage fails for the expected reason,
   then correct it in Stage C. No new `Cargo.toml` or `Makefile` change is
   required because the `cfg(kani)` lint, `default-unwind`, and `make kani-ir`
   alias already exist.
2. Run `cargo kani list` and confirm the three new harness names appear. Capture
   the output under
   `/tmp/kani-list-netsuke-4-2-2-kani-harnesses-for-cycle-canonicalization-stage-b.out`.
3. Run `make kani-ir` and confirm the new harnesses fail for the expected
   (deliberately false) reason while the existing nine harnesses still pass.
   Capture the output under
   `/tmp/kani-ir-netsuke-4-2-2-kani-harnesses-for-cycle-canonicalization-stage-b.out`.
4. Run `make check-fmt`, `make lint`, `make test`, `make markdownlint`, and
   `make nixie` to confirm the harness scaffold (which compiles only under
   `--cfg kani`) does not regress ordinary gates.
5. Run `coderabbit review --agent` for the Stage B diff; resolve all concerns.
6. Commit with subject "Scaffold Kani canonicalization harnesses (4.2.2)" using
   the file-based commit-message workflow.

### Stage C — Green: implement the three property harnesses

Replace each stub with its real assertions. One commit per property so
reviewers can isolate which harness covers which roadmap subitem. Each harness
follows the four-phase shape: build the closed symbolic cycle, capture the
interior baseline, call `canonicalize_cycle`, then assert.

The sketches below are illustrative shapes, not literal source; real helper
signatures adapt to the strict Clippy lint set. Shared helpers (`symbolic_node`,
`closed_cycle_n`, `count_byte`) live in the same module.

1. **Length and closed output** (roadmap subitem one). For each N, assert the
   output length equals the input length and the output is closed.

   ```rust
   #[kani::proof]
   #[kani::solver(kissat)]
   #[kani::unwind(5)]
   fn canonicalize_two_node_cycle_is_canonical() {
       let input = closed_cycle_n(2); // vec![n0, n1, n0], symbolic bytes
       let out = canonicalize_cycle(input.clone());

       kani::assert(out.len() == input.len(), "length preserved");
       kani::assert(out.first() == out.last(), "output cycle is closed");
       // ... multiset and stable-start assertions added below ...
   }
   ```

2. **Interior multiset preserved** (roadmap subitem two). Capture per-symbol
   counts over the interior of the input before the call, then compare against
   the interior of the output, iterating a fixed alphabet.

   ```rust
   let alphabet = [b'a', b'b', b'c']; // four symbols for the N=4 harness
   let in_interior = &input[..input.len() - 1];
   let out_interior = &out[..out.len() - 1];
   for sym in alphabet {
       kani::assert(
           count_byte(in_interior, sym) == count_byte(out_interior, sym),
           "interior multiset preserved",
       );
   }
   ```

3. **Stable start node** (roadmap subitem three). Assert the first node of the
   canonical output is lexicographically smallest among the interior nodes,
   using the production `#[cfg(kani)]` `path_cmp` so the proof matches the
   exact ordering rule. Use `!= Ordering::Greater` because ties are admitted.

   ```rust
   if let Some(first) = out.first() {
       for node in out_interior {
           kani::assert(
               path_cmp(first.as_path(), node.as_path()) != Ordering::Greater,
               "canonical first node is smallest",
           );
       }
   }
   ```

4. **Order preserved (non-vacuity strengthening)**. The three roadmap
   properties together do not pin the cyclic order: a bug that reordered the
   interior while preserving the multiset, closure, and smallest-first start
   would satisfy them all yet produce a wrong canonical form. The proptest
   `all_rotations_canonicalize_identically` guards this at larger N; to keep
   the Kani harnesses non-vacuous against the same class of bug, add one more
   assertion that the output interior is a genuine rotation of the input
   interior. For fixed small N this is cheap:

   ```rust
   // is_rotation_of: ∃k in 0..n such that out_interior[i] == in_interior[(i + k) % n]
   kani::assert(
       is_rotation_of(out_interior, &in_interior),
       "canonical output is a rotation of the input interior",
   );
   ```

   This also gives the multiset property a clean falsification path (see the
   mutation note in Stage E): a structural break that scrambles order without
   changing length is caught here even when the count-based multiset check is
   not. Keep the count-based multiset assertion as the literal record of
   roadmap subitem two; the rotation assertion is the stronger superset.

   The `canonicalize_three_node_cycle_is_canonical` (`#[kani::unwind(6)]`) and
   `canonicalize_four_node_cycle_is_canonical` (`#[kani::unwind(7)]`) harnesses
   repeat all four assertions over their larger fixed N. The harness contains
   several sequential loops (`rotate_cycle`'s rebuild, the per-symbol counts,
   the smallest-first scan, and the rotation search), and a single
   `#[kani::unwind]` applies to all of them, so the bound must cover the
   longest loop: start at `max(N, alphabet_len) + 1` and raise it by one or two
   if an unwinding assertion fires. For N == 4 with a four-symbol alphabet that
   is `unwind(7)`.

After each property commit, run the focused `cargo kani --harness <name>` for
the affected harnesses and confirm verification success with zero failed checks.

### Stage D — Refactor and docs

1. Factor the shared `symbolic_node`, `closed_cycle_n`, and `count_byte` helpers
   so the three harnesses do not duplicate construction logic. Keep
   `src/ir/cycle_verification.rs` within the repository's source-file size
   guideline; if it would exceed it, split the canonicalization harnesses into
   a sibling `src/ir/cycle_canon_verification.rs` declared the same way and
   record the split in the Decision Log.
2. Extend the "Kani harness inventory" table in `docs/developers-guide.md` with
   three rows (module `src/ir/cycle_verification.rs`, property, bound, and a
   note that they drive the pure `canonicalize_cycle` over single-byte node
   names and that larger-N coverage lives in `cycle_property_tests.rs`).
3. Add the three mutation patches under `docs/verification/mutations/` (see
   Stage E for their content) and confirm each passes `git apply --check`.
4. Extend `docs/adr-004-bound-kani-ir-harnesses-to-small-n.md` with one sentence
   recording the canonicalization bounding axis (cycle length, alphabet <= 4)
   and its proptest tail, unless Stage A elected a new ADR, in which case write
   that ADR and index it in `docs/contents.md`.
5. Validate documentation with `make markdownlint` and `make nixie`.

### Stage E — Validate and review

1. Run the mutation discipline, one patch per harness. Apply each patch with
   `git apply`, run the focused `cargo kani --harness <name>`, confirm the
   harness now *fails*, then restore with `git apply -R` in the same session.
   The three mutations are:

   - `ir__cycle__verification__canonicalize_two_node_cycle_is_canonical.patch`:
     break `find_rotation_start` to always return `0` (drop the `start` update).
     This cleanly isolates the stable-start assertion: the output is still a
     valid rotation of the input, so length, closure, multiset, and the rotation
     check all still hold, and only the smallest-first assertion fails.
   - `ir__cycle__verification__canonicalize_three_node_cycle_is_canonical.patch`:
     break `rotate_cycle` to omit the closing `push` of the first node. This
     breaks the length and closed-output assertions.
   - `ir__cycle__verification__canonicalize_four_node_cycle_is_canonical.patch`:
     break `rotate_index` to drop the wrap-around (`index` instead of
     `index - len`). With `start > 0` this indexes out of range, so
     `rotate_cycle` skips nodes; the harness fails on the rotation, multiset, and
     length assertions together.

   Note on isolation: only the first mutation isolates a single property,
   because rotation inherently preserves the interior multiset, so no small
   production mutation can scramble the multiset while keeping the length,
   closure, and order intact. The interior-multiset assertion is therefore
   co-proven with length and the rotation check rather than independently
   falsifiable; this is recorded honestly in the developers' guide note rather
   than papered over with a contrived patch. The requirement that each harness
   *fails* under its recorded mutation is still met for all three.

2. Run the deterministic gates sequentially: `make check-fmt`, `make lint`,
   `make test`, `make markdownlint`, and `make nixie`. Then run `make kani-ir`
   and confirm the full IR suite (the nine `4.2.1` harnesses plus the three new
   canonicalization harnesses) verifies with zero failures. Capture every
   command's output under `/tmp` with the `tee` template.
3. Run `coderabbit review --agent` and resolve all findings. If the service
   stalls at `preparing_sandbox` as it did during `4.2.1`, record the
   observation and proceed on the strength of the deterministic gates.

### Stage F — PR and roadmap

1. Mark roadmap `4.2.2` and its three subitems done in `docs/roadmap.md`, with a
   short note that Kani covers small N and `cycle_property_tests.rs` covers the
   larger-N tail.
2. Rename the branch to `4-2-2-kani-harnesses-for-cycle-canonicalization` (via
   GitHub's rename flow if a PR already exists), push with upstream tracking to
   `origin/4-2-2-kani-harnesses-for-cycle-canonicalization`, and update the
   draft pull request with the implementation summary.

## Concrete steps

Run all commands from the repository worktree root. The Kani commands inherit
the local `LD_LIBRARY_PATH` workaround documented in the `4.2.1` execplan if
the installed driver cannot find its bundled toolchain libraries.

```bash
# Stage B discovery
cargo kani list \
  | tee /tmp/kani-list-netsuke-4-2-2-kani-harnesses-for-cycle-canonicalization-stage-b.out

# Stage C focused runs (one per harness)
cargo kani --harness canonicalize_two_node_cycle_is_canonical
cargo kani --harness canonicalize_three_node_cycle_is_canonical
cargo kani --harness canonicalize_four_node_cycle_is_canonical

# Stage E mutation discipline (repeat per patch)
git apply docs/verification/mutations/ir__cycle__verification__canonicalize_two_node_cycle_is_canonical.patch
cargo kani --harness canonicalize_two_node_cycle_is_canonical   # expect FAILURE
git apply -R docs/verification/mutations/ir__cycle__verification__canonicalize_two_node_cycle_is_canonical.patch

# Stage E gates
make check-fmt | tee /tmp/check-fmt-netsuke-4-2-2-kani-harnesses-for-cycle-canonicalization.out
make lint      | tee /tmp/lint-netsuke-4-2-2-kani-harnesses-for-cycle-canonicalization.out
make test      | tee /tmp/test-netsuke-4-2-2-kani-harnesses-for-cycle-canonicalization.out
make markdownlint | tee /tmp/markdownlint-netsuke-4-2-2-kani-harnesses-for-cycle-canonicalization.out
make nixie     | tee /tmp/nixie-netsuke-4-2-2-kani-harnesses-for-cycle-canonicalization.out
make kani-ir   | tee /tmp/kani-ir-netsuke-4-2-2-kani-harnesses-for-cycle-canonicalization.out
```

Expected Stage E `make kani-ir` summary: twelve harnesses verified, zero
failures (the nine inherited from `4.2.1` plus the three added here).

## Validation and acceptance

Acceptance is behaviour a human can verify:

- Before Stage C, the three canonicalization harnesses fail (the deliberately
  false scaffold assertions). After Stage C, `cargo kani --harness <name>`
  reports verification success with zero failed checks for each.
- For each harness, applying its recorded mutation patch and re-running
  `cargo kani --harness <name>` produces at least one failed check; reversing
  the patch restores success. This proves each assertion is load-bearing.
- `make kani-ir` reports twelve successfully verified harnesses and zero
  failures.
- `make check-fmt`, `make lint`, `make test`, `make markdownlint`, and
  `make nixie` all pass unchanged.

Quality criteria (what "done" means):

- Tests: `make test` passes; the proptest canonicalization suite in
  `cycle_property_tests.rs` continues to pass and remains the larger-N owner.
- Lint/typecheck: `make lint` and `make check-fmt` pass with no new
  `#[allow(...)]` umbrellas lacking a `reason`.
- Verification: `make kani-ir` verifies all twelve harnesses; every new harness
  fails under its recorded mutation.
- Review: `coderabbit review --agent` raises no unresolved correctness, testing,
  or documentation concerns (subject to the `preparing_sandbox` caveat).

## Idempotence and recovery

All steps are re-runnable. The mutation patches are applied and reversed in the
same shell session; if a session ends mid-mutation, run `git apply -R` (or
`git checkout -- src/ir/cycle.rs`) to restore the production code before
re-running gates. The harness code compiles only under `--cfg kani`, so an
incomplete Stage C never affects `cargo build` or `cargo test`. Commits are
small and per-property, so any stage can be rolled back with `git revert`
without disturbing the others.

## Artifacts and notes

The most important artefacts are the three harnesses in
`src/ir/cycle_verification.rs`, the three mutation patches under
`docs/verification/mutations/`, and the extended harness inventory in
`docs/developers-guide.md`. Capture the Stage E `make kani-ir` transcript
showing twelve verified harnesses as the headline evidence.

## Interfaces and dependencies

No public interface changes. The harnesses depend only on private symbols of
`src/ir/cycle.rs` reached through `use super::*`:

```rust
// src/ir/cycle.rs (private; reached from the #[cfg(kani)] verification module)
fn canonicalize_cycle(cycle: Vec<Utf8PathBuf>) -> Vec<Utf8PathBuf>;
fn path_cmp(left: &Utf8Path, right: &Utf8Path) -> std::cmp::Ordering; // #[cfg(kani)] single-byte form
```

New harness-only items added to `src/ir/cycle_verification.rs`:

```rust
// All #[cfg(kani)]; never compiled into ordinary builds.
fn symbolic_node() -> Utf8PathBuf;                  // one symbolic byte from a small alphabet
fn closed_cycle_n(n: usize) -> Vec<Utf8PathBuf>;    // builds vec![n0, .., n0] of fixed n
fn count_byte(nodes: &[Utf8PathBuf], sym: u8) -> usize;
fn is_rotation_of(out_interior: &[Utf8PathBuf], in_interior: &[Utf8PathBuf]) -> bool;

#[kani::proof] fn canonicalize_two_node_cycle_is_canonical();
#[kani::proof] fn canonicalize_three_node_cycle_is_canonical();
#[kani::proof] fn canonicalize_four_node_cycle_is_canonical();
```

`symbolic_node` must avoid `unwrap`/`expect`, because `Cargo.toml` denies
`clippy::unwrap_used` and `clippy::expect_used` and
`allow_attributes_without_reason = "deny"` is in force. Construct the one-byte
name without a fallible conversion, for example:

```rust
fn symbolic_node() -> Utf8PathBuf {
    let b: u8 = kani::any();
    kani::assume(b >= b'a' && b <= b'c'); // tiny alphabet; widen to b'd' for N == 4
    Utf8PathBuf::from((b as char).to_string()) // b is an ASCII letter, so this is total
}
```

If any harness body genuinely needs a suppressed lint (for example, the
panic-on-failure shape), add a module-level
`#![allow(..., reason = "Kani harnesses panic on proof failure by design")]`
preamble to `src/ir/cycle_verification.rs`, matching the `4.2.1` convention,
rather than an unscoped `#[allow(...)]`.

No new external dependency is introduced.

## Open questions (resolve at Stage A)

1. Harness shape: confirm one combined harness per node count N (asserting all
   three properties together), versus one harness per property. Recommendation:
   combined per N.
2. Node-count ceiling: confirm N in {2, 3, 4} interior nodes. Recommendation:
   ceiling of 4, mirroring the `4.2.1` small-N stance; the proptests cover the
   tail.
3. ADR: confirm inheriting
   `adr-004-bound-kani-ir-harnesses-to-small-n.md` (recommended) versus minting
   a new ADR for the canonicalization bounding axis.
4. Make target: confirm whether to add a filtered `make kani-cycle-canon`
   target now, or defer it until `make kani-full` runtime approaches the
   thirty-minute tolerance. Recommendation: defer; `make kani-ir` already
   exists.

## Revision note

- 2026-06-20 (planning agent, after Logisphere community-of-experts review):
  Added a fourth "output is a rotation of the input interior" assertion to each
  harness so the suite is non-vacuous against order-scrambling bugs that the
  three roadmap properties alone would miss; the count-based multiset assertion
  is retained verbatim for roadmap subitem two. Specified that `symbolic_node`
  must avoid `unwrap`/`expect` under the denied Clippy lints and build the name
  via `(b as char).to_string()`. Reconciled the unwind guidance to
  `max(N, alphabet_len) + 1` because one `#[kani::unwind]` covers all of the
  harness's sequential loops. Recorded honestly that the interior-multiset
  property is co-proven (not independently falsifiable by a small mutation) and
  documented the three mutations accordingly. These changes affect Stage C,
  Stage E, the Interfaces section, the Decision Log, and Surprises &
  Discoveries; they do not change the scope, the file list, or the approval
  gate.
