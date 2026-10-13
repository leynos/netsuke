# Add canonical value equality for standard-library helpers

This ExecPlan is a living document. Keep `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` current as work proceeds.

Status: IN PROGRESS

## Purpose / big picture

Standard-library helpers need one deterministic definition of value equality
before ordered set operations and duplicate detection can share a contract.
After this change, Rust callers can derive an opaque canonical key for a
supported MiniJinja value, compare two values by those keys, and deduplicate a
materialized slice while retaining references to first representatives in input
order. Unsupported values fail with a typed error naming their kind. No
template function or filter is registered in this step, so existing manifests
keep their current rendering behaviour; the observable result is a reusable,
tested Rust contract for later helpers.

This plan covers roadmap item 6.1.2 only. It targets a release after v0.1.0
final and does not widen the issue #594 hardening release. Implementation is
under way; the invocation naming this plan supplies execution approval.

## Constraints

- Follow RFC 0006 §6.7 and use the existing `serde_json_canonicalizer`
  dependency. Do not substitute MiniJinja's native equality or hash semantics.
- A key is opaque immutable RFC 8785 canonical JSON bytes. Object member order
  is insignificant; sequence order is significant. `none` is JSON `null`.
  Undefined, callables, `now()` objects, non-string mapping keys at any depth,
  and other unsupported representations are errors.
- Snapshot each admitted value once into a controlled JSON-compatible tree,
  retaining numeric representations, then serialize that tree with the existing
  canonicalizer. Do not use `serde_json::Value` if it rejects or coerces
  supported `i128`/`u128` values. The serializer must not re-read a custom
  object after admission.
- Enumerate each map once. Reject non-string keys with a deterministic typed
  category before visiting values; choose the rejected key kind
  deterministically if several are present, then sort accepted string keys
  before recursively snapshotting their values. This makes canonical bytes and
  the first reported invalid value independent of host-map iteration order.
- Admit scalar JSON values and finite, stable, acyclic Map/Seq representations,
  including well-behaved custom MiniJinja objects exposing those
  representations. Reject Plain, Iterable-only, Bytes, Undefined, Invalid,
  `now()`, and other unsupported kinds. Map/Seq iteration failure is a typed
  error. `ObjectRepr::Map`/`Seq` does not prove host iteration is finite,
  stable, pure, or acyclic; document this host precondition. MiniJinja exposes
  no public identity hook that would let this adapter promise cycle detection.
- Keep the pure canonical byte and index-deduplication kernels separate from
  MiniJinja traversal. The adapter invokes host-object iteration under the
  stated precondition and cannot police arbitrary host-code effects. Netsuke
  must add no I/O, process state, locale, clock, randomness, filesystem, or
  subprocess access.
- Deduplication receives a materialized `&[Value]`, validates every value
  (including later duplicates), returns references to first representatives,
  and produces no partial result if any value is invalid. Do not expose any
  membership-map iteration order. Do not consume arbitrary iterables or invent
  a size/depth limit; RFC 0006 §6.8 and roadmap 6.1.3 own those bounds.
- Expose only `CanonicalKey`, `CanonicalValueError`, `canonical_key`,
  `canonical_eq`, and `stable_deduplicate` from the `stdlib` module. Keep the
  implementation module, key bytes, and controlled snapshot tree private.
  `canonical_eq` must validate both operands before returning, including
  self-comparison; no MiniJinja-equality fast path is allowed.
- Keep the error typed and identify the rejected value kind. Do not add Fluent
  keys, diagnostic codes, localization conversion, or a generic error
  framework; roadmap 6.1.4 owns those concerns.
- Use the already declared `proptest`, `rstest-bdd`, `googletest`,
  `pretty_assertions`, and `insta` dependencies. Add no dependency.
- A Verus proof of the unbounded first-index kernel is required for feature
  acceptance. Invoke it explicitly through `make verus`; keep it outside
  ordinary Cargo, pull-request, and routine developer gates. Do not use
  `assume`, `admit`, trusted/external proof bodies, or equivalent shortcuts.
- Keep Rust files under 400 lines, document new modules/functions as required,
  and preserve the project's borrow-centric API style. No source-API
  compatibility layer is required for this new pre-1.0 surface.

## Tolerances (exception triggers)

- Stop if ordinary finite MiniJinja maps/sequences cannot be snapshotted
  without coercion, or if the supported domain must differ from RFC 0006 §6.7.
  Record concrete values and options before changing the contract.
- Stop if implementation requires arbitrary input bounds, arbitrary iterable
  consumption, cycle detection through private MiniJinja internals, a new
  crate, an unreviewed public surface, or a new trust boundary.
- Stop if a released external API requires compatibility machinery. The
  permitted public change is only the narrow additive foundation stated above.
- Stop if Verus cannot prove the inhabited abstract first-index kernel
  substantively, or proof completion depends on assuming the runtime bridge.
  Revise the proof plan rather than weakening the requirement to sampled tests.
- Preserve deterministic gate failures and logs; fix them without weakening
  gates or creating an isolated Cargo cache. For shared compiler-admission
  waits follow the repository's `build-limits` instructions.
- Escalate any material ambiguity about MiniJinja's supported value
  representations or current `uniq` semantics. Do not invent a template
  contract to resolve it.

## Risks

- Risk: MiniJinja containers and custom values use `Object`; Map/Seq
  representation does not establish finite, stable, pure, acyclic traversal.
  Severity: high. Likelihood: medium. Mitigation: traverse once, reject
  iteration failures and unsupported representations, document the host-value
  precondition, and test custom Map/Seq plus failing iterators. The adapter
  cannot detect cycles or constrain arbitrary host code without a separate
  identity/materialization contract.
- Risk: RFC 8785 number serialization uses IEEE-754 binary64 semantics and can
  deliberately collapse source values such as `2^53 + 1` and `2^53`. Severity:
  medium. Likelihood: high. Mitigation: assert boundary behaviour and
  signed-zero behaviour; do not claim integer-injective equality. Exercise
  `i64`, `u64`, `i128`, `u128`, and `f64` representations through the actual
  canonicalizer.
- Risk: recursive snapshots and retained unique keys can use stack and memory
  proportional to caller input. Severity: high. Likelihood: low in current
  manifest use. Mitigation: do not consume lazy iterables; state complexity as
  traversal/output bytes plus per-map key sorting and retained key bytes. Leave
  shared bounds to 6.1.3. No bound is claimed for arbitrary embedding inputs.
- Risk: Verus can prove the abstract key/index kernel without proving Rust or
  third-party serialization. Severity: high. Likelihood: medium. Mitigation:
  model source indices and abstract byte keys, review the trusted base, compare
  the implementation algorithm to the model, and use independent runtime tests
  for the MiniJinja/canonicalizer bridge.
- Risk: `docs/developers-guide.md` currently limits first Verus work to cycle
  canonicalization. Severity: medium. Likelihood: high. Mitigation: the plan's
  approved proof milestone explicitly amends that policy to add this narrow
  optional kernel while preserving its exclusion from default workflows.
- Risk: installing Verus creates `.verus/<version>/` in the workspace root,
  which holds 13 `.rs` files, is not covered by `.gitignore`, and is absent from
  `MACHINE_LOCAL_DIRECTORIES`. Severity: high. Likelihood: high, because Stage
  A has already performed the install on this machine. The
  `every_rust_source_in_the_workspace_is_scanned` contract reports a Rust
  source outside `COMPILED_SOURCE_ROOTS` rather than excusing it, so the gate's
  verdict would depend on whether the operator had installed Verus — the exact
  machine-dependent verdict that invariant exists to prevent. Mitigation:
  falsify the failure first, then make the name honestly ignored by adding
  `.verus/` to `.gitignore` beside `.uv-cache` and `.netsuke`, which is the
  appeal `walk_tests.rs` requires every skipped name to satisfy. Do not add the
  name to `MACHINE_LOCAL_DIRECTORIES` while `git check-ignore` declines it:
  that is the defect that list's self-test was written to catch. Note the
  ordering constraint — the skip-list self-test asks a scratch repository
  seeded from the repository's `.gitignore`, so the `.gitignore` entry must
  land in the same commit as any list change. Resolved in Stage A: the defect
  was reproduced first (58 of 59 tests passed;
  `every_rust_source_in_the_workspace_is_scanned` failed naming twelve paths
  beginning `.verus/0.2026.10.04.426d8b0/verus/build/*/out/`, one of them
  `serde-c5751747ce96f67e/out/private.rs`; the thirteenth was not reported
  because nextest cancels still-running tests on the first failure), then both
  edits landed together, and the same suite then passed 59 of 59. See the
  corresponding Progress and Decision log entries.
- Risk: the plan's "Map/Seq iteration failure is a typed error" overstates what
  MiniJinja exposes. Severity: low. Likelihood: high. A `NonEnumerable`
  enumerator yields `None` rather than an error, and a mid-iteration failure is
  not observable at all. Mitigation: map `None` to a typed rejection, document
  that the residual case is covered by the host precondition, and test the
  `NonEnumerable` shape explicitly so the wording is pinned to observed
  behaviour rather than to an assumed API.
- Risk: changing current `uniq` behaviour would alter existing manifests.
  Severity: high. Likelihood: medium. Mitigation: preserve `uniq` and
  `group_by` registrations and behaviour; a later consumer migration needs its
  own accepted contract.
- Risk: adding a module can break a **derived whole-crate contract** while every
  file-scoped gate stays green. Severity: medium. Likelihood: high. Evidence:
  `src/stdlib/canonical_value/` carries no Kani marker of its own, yet adding
  `mod canonical_value;` to `src/stdlib/mod.rs` silently widened the Kani proof
  scope, because the scope is a *transitive module closure* recomputed from
  source by `tests/workflow_contracts/kani_proof_scope_test.py` rather than a
  list anyone edited. The same shape is latent in
  `tests/env_access_suppressions.rs`, which walks every workspace directory and
  fails on reachable Rust source outside its scanned roots. Both classes are
  invisible to `cargo test` on the touched file, to Clippy, and to formatting.
  Mitigation: run `make test-workflow-contracts` unconditionally rather than by
  change-detection, because the trigger is a *new file* whose effect is
  computed rather than declared; and when such a contract fires, derive its
  scope file from source instead of pattern-matching the failure message.

## Progress

- [x] (2026-10-13) Inspect roadmap 6.1.2, RFC 0006 §6.7, dependent RFC
  constraints, `uniq`, the Verus Make targets, and required design/testing
  references.
- [x] (2026-10-13) Wyvern reconnaissance and three paired six-lens reviews
  examined API boundaries, numeric semantics, proof non-vacuity, and delivery
  gates. Findings are incorporated in this DRAFT; all six expert lenses
  completed final readback with no remaining blockers.
- [x] (2026-10-13) Draft proof sketch, runtime bridge, test strategy,
  milestones, tolerances, and conformance trace. No implementation has begun.
- [x] (2026-10-13) Planning-document gates passed: `make fmt`,
  `make check-fmt`, `make markdownlint` (including spelling), and `make nixie`.
  No runtime code changed; Rust lint/test/doc-coverage and the future Verus
  proof have not been run for this documentation-only planning change.
- [x] (2026-10-13) Obtain explicit approval before implementation. The
  invocation
  `use /sdlc-implementation 6-1-2-canonical-value-key-and-equality-relation`
  names this plan and instructs implementation, which the skill's contract
  records as execution approval; no separate approval was requested.
- [x] (2026-10-13) Characterize MiniJinja values and confirm the approved API
  against actual constructors, iterator failures, numeric serializers, and
  pinned Verus tools. Characterization crate at `/tmp/canon-probe` (outside the
  tracked tree) pinned `minijinja =2.24.0`, `serde_json =1.0.151`
  (`preserve_order`), and `serde_json_canonicalizer =0.3.2`. Its full output is
  reproduced in the Surprises & discoveries entries above; the admitted and
  rejected matrices in the Verification plan record the conclusions.
- [x] (2026-10-13) Pin the Verus toolchain and repair the gate its install
  broke. Added `tools/verus/VERSION` (`0.2026.10.04.426d8b0`) and
  `tools/verus/SHA256SUMS` (digest `6240186628…1a0` for
  `verus-0.2026.10.04.426d8b0-x86-linux.zip`), then ran `make install-verus`
  run from the worktree, which printed `Installed Verus 0.2026.10.04.426d8b0`.
  The install wrote `.verus/<version>/` into the workspace root, which broke
  `every_rust_source_in_the_workspace_is_scanned`; the defect was reproduced
  with `cargo nextest run --test env_access_suppressions` (59 run, 58 passed, 1
  failed, naming twelve `.verus/**/out/*.rs` paths) and then fixed by adding
  `.verus/` to `.gitignore` in the same change as `".verus"` in
  `MACHINE_LOCAL_DIRECTORIES`, after which the same suite passed 59 of 59.
- [x] (2026-10-13) EP-M1 gate batch 2 ran at the frozen revision
  `65aba802f91b14a205ad8c399b6d7be59ccd13f6`, tree clean before and after every
  gate. Six of seven green: `check-fmt` (RC=0, 4 s), `lint` (RC=0, 263 s),
  `doc-coverage` (RC=0, 426 s, 98.87% against an 80% threshold), `markdownlint`
  (RC=0, 33 s), `nixie` (RC=0, 3 s), `spelling` (RC=0, 21 s). `test` is red
  (RC=2) for the environmental reason recorded under Surprises, and `doctest` —
  the second prerequisite of `make test`, which the abort had never reached at
  any revision — was run on its own and passed (RC=0, 100 s). Deliberately
  excluded, with reasons: `test-workflow-contracts` and
  `test-documentation-contracts` (no workflow, contract suite, or marked
  example changed), `test-coverage-artifact` (no `scripts/` validator changed),
  and `make verus` (no proof file exists until EP-M3).
- (2026-10-13) The milestone CodeRabbit CLI review requested through the
  scrutineer ran at the frozen revision
  `0ce95e3189f91cc80da85139c09c7567a3558a17` and reported **zero findings**
  (`RC=0`, 101 s,
  `{"type":"complete","status":"review_completed","findings":0}`). The tree was
  clean before and after, and `reviewedFiles` is exactly the branch's five-file
  diff against `b14bde20`, so the review's comparison base is the PR base. This
  closes the EP-M1 review obligation. It is a **local** assessment only: the
  hosted CodeRabbit assessment that appears on PR #912 belongs to
  `pr-babysitting` and remains outstanding.
- [x] (2026-10-13) EP-M2 landed as `8523eb02` (15 files, +2280/-1), and its gate
  batch ran at that frozen revision with the tree clean before and after every
  gate. Four of seven were green: `lint` (RC=0, 271 s, the full cascade with no
  stage masked), `doc-coverage` (RC=0, 98.88% against an 80% threshold), `test`
  (RC=0, 287 s, nextest `3934 tests run: 3934 passed, 6 skipped`, doctests
  green), and `nixie` (RC=0, 1 s). Three were red, and all three were
  self-inflicted by `8523eb02` itself: `check-fmt` (RC=2) because
  `mdtablefix --check` wanted to re-wrap this file's own new prose; `spelling`
  (RC=2) on five sites, being `hand-written` in three files plus `mis-stated`
  here; and `markdownlint` (RC=2) as a *masked* failure, where the red
  `spelling` prerequisite aborted before `markdownlint-cli2` ran at all — a
  direct probe of the masked stage returned RC=0, 0 issues in 181 files.
- [x] (2026-10-13) Those three gates were repaired in one mechanical change:
  `make fmt` applied the `mdtablefix --in-place` re-wrap, and the five spelling
  sites became `handwritten` and an incorrectly stated `now()` kind. A
  token-level comparison of the reformatted diff against the raw diff reports
  those two word substitutions as the *only* non-re-wrap differences, so no
  pre-existing prose was canonicalized as collateral. The repair is then
  validated by re-running the whole applicable gate set rather than only the
  three that had been red: a subset re-run would leave the four earlier green
  results standing against a revision they no longer describe.
- [x] (2026-10-13) The EP-M2 milestone CodeRabbit CLI review ran through the
  scrutineer against the branch-versus-base diff (`coderabbit` 0.9.0, RC=0, 417
  s, no rate limit) and reported six findings covering five distinct concerns.
  All five were verified against current source before any repair;
  `reviewedFiles` matched the branch's 19-path diff exactly.
- [x] (2026-10-13) Those five concerns were disposed of as follows.
  **Repaired:** `admit_mapping` cloned the member key only because
  `&path.with_member(&key)` borrows it after the move, so the path is now built
  first and the owned key inserted; the machine-local-directory comment said
  the verifier's install tree was *unpinned* when `make install-verus` requires
  the version file `aa32c294` added. **Refused, with rationale:** deleting the
  non-string guard in `sort_members` on the grounds that
  `refute_non_string_key` already rejects every key that guard would. The guard
  is load-bearing for a different reason than the finding assumes — the outer
  check reports the *lowest* non-string kind and is therefore
  order-independent, while the inner guard reports whichever non-string key
  sorts first; the two disagree whenever a non-string key is not the first
  sorted member. **Tightened:** `distinct_count` no longer `.ok()`-filters
  refusals away (which would have silently shrunk the expected count and
  weakened the very property it guards), and the BDD order step now asserts the
  whole retained tail rather than one element of it.
- [x] (2026-10-13) The full applicable gate set was re-run at `58bc8357`.
  Six gates were green — `check-fmt`, `lint`, `doc-coverage` (98.88%),
  `markdownlint` (181 files), `spelling`, and `nixie` — and `make test` was
  **red**. The verdict is the red one, so the milestone did not pass.
  `make test` also never reached its `doctest` prerequisite, because the target
  is `test: test-nextest doctest` and the red `test-nextest` aborted it first:
  the log holds 3 `make` invocations and 0 `Doc-tests` lines. The doctest half
  is therefore **unverified**, not green.
- [x] (2026-10-13) The red was diagnosed as a latent defect in EP-M2's own
  test, not a regression from the finding repair and not a flake to retry.
  Evidence: the same test binary at the same revision passed on attempts 1 and
  2 and failed on attempt 3 with "class `bool` was never generated in 64 cases".
  `git diff` from the pre-repair revision to `58bc8357` over `properties.rs`
  shows the repair touched only `distinct_count` and its call site, so the
  failing test and its generator are untouched by it. Root cause, read from the
  pinned source: `Config::default()` sets `rng_seed: RngSeed::Random`, so
  `TestRunner::new` seeds from the OS and the class-coverage assertion was
  sampling-probabilistic. The module's own doc comment already claimed those
  counts "are reproducible", which the code never delivered.
- [x] (2026-10-13) That was repaired by splitting one fused test into two, on
  the fault line the defect exposed: a **law** is worth checking against fresh
  inputs, while **coverage** is a claim about the generator rather than about
  one draw from it. `generated_values_obey_the_equivalence_relation` now runs
  on a free-seeded runner and asserts only the laws;
  `generated_values_reach_every_material_class` runs on a runner pinned to
  `RngSeed::Fixed(0x612)` over 512 cases and reports the classification counts
  EP-V1 requires. Neither obligation was weakened: the coverage gate now covers
  eight times as many cases and its verdict no longer depends on the seed.
- [x] (2026-10-13) The repair was validated by measurement, not by observing a
  pass. The frozen sample reaches all seven classes with margin —
  `{"bool":
  38, "float": 74, "int": 38, "map": 97, "null": 33, "seq": 124, "string":
  108}` —
  and three consecutive runs reported a byte-identical tally. The negative
  control is decisive: with the `bool` arm deleted from the generator, **both
  law tests still passed** while the tripwire failed, naming the missing class.
  That is the non-vacuity guarantee EP-V1 asks for, and the previously fused
  test could not provide it.
- [x] (2026-10-13) The full applicable gate set was re-run at `adbb8d61`. Six
  were green — `check-fmt`, `lint`, `doc-coverage` (98.88%), `markdownlint`
  (181 files), `spelling`, and `nixie` — and the doctest half of `make test`,
  which the previous batch had left unverified, was checked directly and is
  green: two `Doc-tests` targets, 136 passed, 0 failed, 32 ignored. One gate
  timed out:
  `packaging_smoke_tests::packaged_manifest_retains_build_script_sources` hit
  its 300 s budget at 300.022 s with no assertion failure, under a host load of
  24.40 on 24 cores. That test appears in no branch diff and has no
  `slow-timeout` widening of its own, so it is an environmental timeout rather
  than a branch defect, and this branch was deliberately not patched for it.
- [x] (2026-10-13) A **seventh** red then surfaced that the directed gate set
  had never covered, and it was deterministic and branch-introduced. CI run
  `38396731593` at this same revision concluded `failure`, in the `build-test`
  job's **Workflow contract tests** step — not the packaging test the batch had
  suspected — which skipped the four steps after it, including "Test and
  Measure Coverage". The assertion names four files the Kani harnesses reach but
  `tools/kani/proof-scope.toml` did not list. It reproduced locally
  byte-identically (`1 failed, 1263 passed, 3 skipped`).
- [x] (2026-10-13) That red was repaired by extending the proof scope with
  exactly the four files the closure reaches. The mechanism is that
  `src/stdlib/canonical_value/` carries no Kani marker of its own, so it never
  entered the scope when the crate was written, but `src/stdlib/mod.rs`
  re-exports its five public names, so every harness that reaches the standard
  library now reaches it transitively. The four entries were **computed**, not
  copied from the failure text: a probe over the real module graph returned
  twelve seeds and a reached set for `src/stdlib/canonical_value/` that is
  exactly those four files. The `tests/` subtree is deliberately absent, being
  a test-only module that `cargo kani` compiles out. The contract file then
  passed 7 of 7 in a focused re-run.
- [x] (2026-10-13) The EP-M2 milestone CodeRabbit CLI review was **re-run**
  against the current tip, because the first round's head-SHA binding had gone
  stale: it reviewed `b27da3ed`, and the branch had since moved twice, for a
  finding repair and a determinism repair. The re-review covered the committed
  range `b14bde20...d0ac4283` with `reviewedFiles` matching the branch's
  20-path diff exactly (`coderabbit` 0.9.0, RC=0, no rate limiting, tree clean
  before and after). It reported three findings comprising **two distinct
  concerns**, and both were verified against source before any repair.
- [x] (2026-10-13) Both concerns were repaired. **First**, this plan's own
  Progress list carried a continuation line indented six spaces where its
  sibling uses two, holding a single word; the line count was surveyed, and it
  was the only six-space continuation in the file. Joining the word into the
  following line fixes it in a 77-column line, inside the 80-column rule. The
  review's own suggested one-line join measured 81 columns and would have been
  wrapped straight back by the formatter. **Second**,
  `src/stdlib/canonical_value/tests/mod.rs` named a `get_value` parameter `key`
  and then read it with a bare `let _ = key;` purely to silence the
  unused-variable lint. The parameter is now `_key` and the artificial read is
  gone. The rename is scoped to the one impl of nine that does not use the
  parameter; the two other `get_value` impls in the same file do use it. The
  `let _ =` precedents elsewhere in the tree were checked and are
  `#[cfg]`-justified, which cannot apply here, as this file carries no `cfg`
  attribute at all. Neither repair justifies re-opening the milestone: both are
  presentational, neither changes an observable behaviour, and the second makes
  a fixture's signature honest about what it ignores.
- [x] (2026-10-13) The gate batch over the repaired candidate ran over the
  frozen tree and reported four of six completed gates green — `check-fmt` (3
  s), `lint` (305 s, the full cascade with no stage masked), `doc-coverage`
  (424 s, aggregate **5055/5112 = 98.88%** against the 80% threshold), and
  `nixie` (1 s). `markdownlint` and its `spelling` prerequisite were both red
  (RC=2, 9 s each), and both named the *same* defect: this plan's own new prose
  contained `mis-indented`, which the tokenizer split into a bare `mis` and
  reported as a misspelling at `:381`. A supplementary unmasked probe of the
  stage behind the red prerequisite returned `0 issues in 0 files` over 181
  files, so the two reds were one defect observed twice rather than two
  independent failures. The repair reworded to `over-indented` and recorded the
  hazard under Surprises. The `test` gate was stopped rather than allowed to
  finish, because it was running on bytes the repair was about to change and
  the shared compile queue is contended.
- [x] (2026-10-13) The repaired candidate was re-gated. The eight applicable
  targets are all green on the frozen tree, bound to content hash `2d7aadbd…`:
  `check-fmt`, `lint` (233 s), `doc-coverage` (**5055/5112 = 98.88%** against
  the 80% threshold), `markdownlint`, `spelling`, `nixie`, `test`, and
  `test-workflow-contracts` (1264 passed, 3 skipped) — the last because the
  Kani scope repair touched a workflow contract. `test` is green in both halves:
  `test-nextest` at **3935/3935 passed** (6 skipped) and its `doctest`
  prerequisite, which the first attempt never reached, run separately at RC=0
  and 105 s. The first `make test` of this batch was red on a single
  **timeout**, not a failure:
  `packaging_smoke_tests::packaged_manifest_retains_build_script_sources` was
  killed at `300.007 s` against the 300 s cap while the host was at load 17.6
  on 24 cores, with zero assertion failures and the `nested-cargo-builds`
  group's `max-threads = 1` making every member queue behind one another. The
  identical bytes then **passed** at `290.107 s` on re-run. Both runs'
  `test-nextest` phases carry the same **573**
  `managed sccache server is unreachable` lines, and `packaging_smoke_tests.rs`
  appears in no branch diff. So the verdict is load-determined, sitting ~10 s
  inside a 300 s cap against a ~288 s isolated measurement, and the red does
  not describe this candidate. The risk entry's earlier claim that the sccache
  count discriminates a pass from a fail was measured false and corrected.
  Widening the allowance remains out of scope here, for the membership-rot and
  budget reasons the entry already gives. A check of `origin/main` then found
  that `1a5fc57f` (PR #906) **has already widened this exact test's allowance
  to 600 s**, so the timeout is a property of this branch's base rather than of
  the candidate: it should not recur after a rebase onto current `main`. That
  commit also edits `.config/nextest.toml`, which renumbers the two line
  citations this plan makes against that file; both are flagged in the risk
  entry for re-derivation after the rebase.
- [x] (2026-10-13) EP-M2 close-out verified by a second, independent batch on
  the frozen tree, bound to content hash `89beebdb…` (1488 files). All eight
  targets exited 0 sequentially, and the harness's post-run `hashlines.after`
  came out byte-identical to its pre-run `hashlines`, so no gate wrote to any
  tracked or untracked non-ignored file. `test` ran **both** halves this time:
  `test-nextest` at **3935/3935 passed** (6 skipped) in 421 s, and `doctest`
  (95 + 2 + 39 passed) because its prerequisite was green rather than aborting
  it. The load-sensitive packaging test passed at **245.656 s**, inside its 300
  s cap, so neither environmental trap fired. The batch's `test-nextest` phase
  carries **1034** `managed sccache server is unreachable` lines and still
  passed — a third independent observation, and further evidence against the
  retired claim that this count discriminates a verdict: it is not even stable
  across runs (573, 573, 1034) and carries no information about the outcome.
  The only delta after that batch is this entry's own prose, in this one file,
  so the applicable re-gate is the Markdown-sensitive subset (`check-fmt`,
  `markdownlint`, `spelling`, `nixie`) rather than the Rust suite.
- [ ] Implement canonical keys/equality and stable deduplication; discharge
  property, BDD, runtime-boundary, and Verus obligations.
- [ ] Update design and formal-verification guidance, run sequential gates, and
  mark roadmap 6.1.2 done only after all feature acceptance evidence passes.

## Surprises & discoveries

- Surprise (measured 2026-10-13, EP-M2 review): an automated review's *stated
  remedy* is evidence about the defect, not a patch. The review correctly
  identified an over-indented continuation line in this plan and then proposed
  joining it to its predecessor, which measures 81 columns and sits outside the
  80-column prose rule this repository enforces. Applying the finding was
  right; applying its remedy would have created a formatting failure. The
  lesson generalizes: read a finding's *diagnosis* and derive the repair
  locally against the local conventions, because the reviewer does not run the
  local formatter. The same round showed the converse hazard on the review side
  — a rebuttal of the finding asserted two premises about the offending line
  (its indent, and its word count) that measurement showed to be false, so a
  disposition cannot rest on a reviewer's or a rebutter's description of the
  bytes either. Both directions are settled the same way: measure the line.
- Surprise (measured 2026-10-13, EP-M2 repair): the hyphen-split spelling
  hazard is not confined to the fixtures the earlier entry describes — it
  recurs in *this plan's own prose*, in the very sentence that records the
  earlier occurrence. Writing `mis-indented` here red the spelling gate, which
  read the bare fragment `mis` and proposed `miss` or `mist` at `:381`. The
  tokenizer strips the hyphen and reads the fragment as a standalone word,
  because the prefix is one the dictionary does not carry. Measured: the
  pre-edit plan held `mis-stated` at `:245` and passed, so the prefix is safe
  or fatal depending only on whether the token is inside a code span. The
  repair reworded to `over-indented` rather than extending `typos.local.toml`,
  whose own comment reserves it for identifiers and quotations; the pre-existing
  `mis-grouping` entry is domain vocabulary and is the exception that proves
  the rule. The lesson for a plan author is narrow but bite-sized: a compound
  whose first element is a short dictionary word can red a gate, and the
  failure is invisible to `mdtablefix`, to the formatter, and to every local
  check except the one that runs last.
- Surprise (measured 2026-10-13, EP-M2): a new module can be outside the Kani
  proof scope without containing a single line of Kani, because the scope is a
  *transitive closure* and `src/stdlib/mod.rs` re-exports the new module's
  public names. Impact: the directed gate set for this milestone had excluded
  `make test-workflow-contracts` on the reasonable-sounding ground that no
  workflow or contract suite was changed — but "changed no contract test" and
  "cannot affect a contract test" are different claims, and the second one is
  false here. A gate whose subject is a *derived* property of the whole crate
  cannot be scoped by asking which files were edited. The check that caught
  this was CI, one full revision after a locally green gate set; the local
  gates could only have caught it if the workflow-contract target had been in
  the set. See `docs/adr-039-change-scoped-kani-gate.md`.
- Surprise (measured 2026-10-13, EP-M2): a property test can be **latently
  non-deterministic** and pass a whole green gate set before anyone sees it
  fail. `TestRunner::new` seeds from the OS, so an assertion about the
  generator's reach was really an assertion about one draw from it. Impact: the
  lesson is not "seed the runner" but "classify the claim first" — a claim
  about a generator must not be carried by a sample that varies, and a claim
  about a law is best carried by one that does. Two claims with opposite
  sampling needs do not belong in one test. The check that caught this was
  re-running the full set; a targeted re-run of the three previously-red gates
  would have missed it entirely.
- Surprise (measured 2026-10-13, EP-M2 batch 4): **a diagnostic count that
  separates two *historic* observations can still fail to separate two
  *contemporaneous* ones.** The risk entry read the 573
  `managed sccache server is unreachable` lines as decisive against an earlier
  1-line pass, and that was true of the pair it compared. This batch then
  produced a **TIMEOUT** at `300.007 s` and a **PASS** at `290.107 s` on
  byte-identical bytes, **both** carrying 573 such lines. Impact: the count
  establishes that the cache was down and that compiles ran cold; it carries no
  information about which side of the cap a given run lands on, because the
  verdict is set by host load during the test's window. The lesson is about the
  shape of the inference, not the number: a discriminator validated against
  *retrospective* pairs must be re-validated against a *prospective* pair
  before it is used to exonerate or indict a candidate. Here the second sample
  falsified it, and the plan's own earlier claim was corrected in place rather
  than left standing next to contradicting evidence.
- Observation: current `uniq` uses `IndexSet<Value>` and follows MiniJinja's
  equality; RFC 0006 defines canonical JSON equality for later set helpers.
  Evidence: `src/stdlib/collections.rs`, `uniq_filter`. Impact: 6.1.2 adds a
  reusable foundation without migrating `uniq`.
- Observation: JCS uses IEEE-754 number semantics rather than source-integer
  identity. Evidence: RFC 8785 §§3.1, 3.2.2.3, and Appendix B, checked via
  Firecrawl and the [RFC text](https://www.rfc-editor.org/rfc/rfc8785/). Impact:
  `2^53 + 1` can share a key with `2^53`; negative and positive zero serialize
  alike. No arbitrary-precision integer-injectivity claim is valid.
- Observation: the repo already delegates optional Verus execution through
  `make verus`, and the pin files this observation once listed as absent now
  exist. Evidence: `Makefile`, `tools/verus/VERSION`, `tools/verus/SHA256SUMS`,
  and a `git log` showing them added by `aa32c294` ("Pin Verus and stop its
  install tree breaking the workspace scan"). Superseded in part: the
  planning-time statement that implementation "must choose and record the
  upstream Verus release and add `tools/verus/VERSION` plus
  `tools/verus/SHA256SUMS` before installation" was true when written and has
  since been discharged; see the Progress and Decision log entries for
  `aa32c294`. **Do not** re-add or overwrite the pins on the strength of this
  line — `VERSION` holds `0.2026.10.04.426d8b0` and the `SHA256SUMS` digest was
  computed from three independent downloads and is not published upstream, so
  overwriting it destroys work that cannot be recovered from the vendor.
- Withdrawn (2026-10-13, Stage A): an intervening revision of this section
  claimed the pinned `prover-tools` invocation does **not** accept
  `--repo-root`, and withdrew the original planning-time claim as false. That
  withdrawal was itself **false**, and it is withdrawn here in turn. The flag
  **is** declared (`verus run --help` and `verus install --help` both list
  `--repo-root PATH [env var: INPUT_REPO_ROOT]`), so forwarding it is supported
  and the original planning-time note was right. The same revision asserted
  that cyclopts "ignores unknown options for options it does not declare" and
  that the earlier install "succeeded because the flag was inert, not because
  it was consumed"; both are false —
  `prover-tools verus run --this-flag-was-never-declared` prints
  `Unknown option: --this-flag-was-never-declared`. The error came from reading
  docstring fenced examples in `rust_prover_tools/verus/models.py` and
  `run.py`, which show a Python caller constructing
  `VerusPaths(repo_root=Path.cwd())` directly and so cannot witness what the
  CLI declares. Two consequences hold. First, the proof invocation should be
  run from the worktree root as planned; `--repo-root` is available but
  unnecessary. Second, the method that produced the false correction is the
  lesson: a claim about what a *command* accepts must be settled by probing the
  command, and reading source that merely *uses* an underlying model type is
  not evidence about its CLI surface.
- Observation: MiniJinja's public object representation does not establish
  finiteness, purity, or cycle freedom. Evidence: MiniJinja `ObjectRepr`,
  `try_iter`, and `try_iter_pairs` APIs. Impact: explicitly document the host
  precondition; do not promise cycle detection or adapter-enforced bounds.
- Observation (measured 2026-10-13, Stage A): `to_iter_pairs` is not reachable
  from a `Value`. `Value::try_iter` exists and yields a `Result<ValueIter>` for
  `none`/`undefined`, strings, and objects, but returns `Err(InvalidOperation)`
  for every scalar; it yields a map's **keys**, never pairs. Pair iteration is
  `ObjectExt::try_iter_pairs(&DynObject)` — reached as `value.as_object()`,
  which is `None` for every non-object scalar. Evidence: MiniJinja 2.24.0
  `src/value/mod.rs:1475-1495` and `src/value/object.rs:363-394`; probe output
  `try_iter on Map -> ["a", "b"]` and `try_iter_pairs on Map -> ["k=7"]`.
  Impact: the adapter must dispatch on `ValueKind` first and only then call
  `as_object()`, using `repr()` to choose map vs sequence traversal. `try_iter`
  alone cannot distinguish a map from a sequence, and for a map it silently
  yields keys — so a naive `try_iter`-based snapshot would emit a map's keys as
  a JSON array. This is the trap the developers' guide already documents at
  line 221.
- Observation (measured 2026-10-13, Stage A): a `Map`-represented object whose
  enumerator is `NonEnumerable` serializes through the canonicalizer as `{}`,
  and one whose `get_value` returns the key serializes as `{"1":"x"}` for an
  integer-keyed map. Evidence: probe lines `failing map ... jcs={}` and
  `int-keyed map ... jcs={"1":"x"}`. Impact: **the canonicalizer does not
  reject non-string map keys**. `serde_json`'s `MapKeySerializer` coerces
  integer keys to quoted strings and `JcsFormatter` writes them without the
  `as f64` cast that its value-position `write_i64` applies. The plan's "reject
  non-string mapping keys at any depth" is therefore load-bearing rather than
  defensive, and cannot be delegated to the serializer.
- Observation (measured 2026-10-13, Stage A): an object whose `enumerate`
  reports `NonEnumerable` makes `try_iter`/`try_iter_pairs` return `None` — not
  an error. Evidence: probe lines `PlainThing ... try_iter -> None` and
  `FailingMap try_iter_pairs -> None (NonEnumerable)`. Impact: `None` means
  "not enumerable", which the adapter maps to a typed rejection; it is not an
  iteration failure. A genuine mid-iteration failure is not observable through
  this API at all, so the plan's "Map/Seq iteration failure is a typed error"
  must be read as covering the `NonEnumerable`/`None` case, and the host
  precondition covers the rest.
- Observation (measured 2026-10-13, Stage A): neither Rust's own `==` nor the
  canonicalizer normalizes Unicode. Evidence: probe lines
  `nfc == nfd (Rust) = false`, `nfc jcs = "café"`, `nfd jcs = "café"`
  (byte-distinct). RFC 8785 §3.2.3 sorts property names as raw UTF-16 code
  units with no normalization step and no NFC requirement. Impact: equality is
  byte-identity over unnormalized JCS, so canonically equivalent-but-distinct
  key spellings are distinct values. Record as an explicit non-goal; do not add
  a normalization dependency.
- Observation (measured 2026-10-13, Stage A): every MiniJinja numeric width
  reaches JCS as a binary64. Evidence: probe lines
  `i64 min … jcs=-9223372036854776000`, `u64 max … 18446744073709552000`,
  `i128 pow53+1 … 9007199254740992`, `u128 max … 3.402823669209385e+38`,
  `f64 -0.0 … 0`, and `i128::MAX -> 1.7014118346046923e+38`. Confirms the
  pre-recorded risk rather than refuting it: `JcsFormatter::write_i128`/
  `write_u128` cast through `as f64`, so the collapse is in the canonicalizer,
  not in `serde_json::Value`. Widening the snapshot tree to `I128`/`U128`
  preserves fidelity and keeps the door open for a future canonicalizer, but it
  does **not** and cannot make keys injective over the integer domain.
  `i64::MIN` already loses precision, so the collapse is not an `i128`-only
  concern.
- Observation (measured 2026-10-13, Stage A): the pinned Verus release is
  `0.2026.10.04.426d8b0`, and `make install-verus` installs it reproducibly.
  Evidence: `make install-verus` run from the worktree printed
  `Installed Verus 0.2026.10.04.426d8b0 in .../.verus/0.2026.10.04.426d8b0/verus`.
  The upstream release publishes **no** checksum file (confirmed via DeepWiki
  against `verus-lang/verus`), so the digest in `tools/verus/SHA256SUMS` is
  locally computed from three independent downloads that agreed. Reachable
  commands are `prover-tools verus install` and `prover-tools verus run`; there
  is no `install-verus` subcommand, so `make install-verus` is the supported
  entry point and the Makefile forwards `VERUS_INSTALL_FLAGS`/`VERUS_FLAGS`.
- Observation (measured 2026-10-13, Stage A): installing Verus writes a
  `.verus/<version>/` tree into the workspace root. It is 1.6 GB and untracked,
  and `git check-ignore -v .verus` exits 1, so git does not ignore it. It is
  also absent from `MACHINE_LOCAL_DIRECTORIES`
  (`tests/env_access_suppressions/roots.rs`), and it contains 13 `.rs` files
  under `verus/build/*/out/`. Impact: the workspace-coverage contract test
  fails on any machine where Verus has been installed, so the gate's verdict
  turned on the operator rather than on the tree. Measured both ways: with the
  directory present and unnamed,
  `cargo nextest run --test env_access_suppressions` reported 59 run, 58
  passed, 1 failed — `every_rust_source_in_the_workspace_is_scanned`, whose
  message listed twelve `.verus/**/out/*.rs` paths ahead of the fix — and after
  adding `.verus/` to `.gitignore` and `".verus"` to
  `MACHINE_LOCAL_DIRECTORIES` the same suite passed 59 of 59. Attribution is a
  controlled pair, not two unrelated runs: `.verus/` was present in both, the
  machine and the test target were the same, and the only difference was the
  two-file edit. The failing half was produced by a delegated falsification
  experiment whose log (`/tmp/verus-hypothesis-1791870278.out`, finished
  07:56:07 local) was written before the fix landed (07:57:56–07:58:13), so its
  binary was compiled from the unpatched tree. See the risk entry, the Progress
  entry, and the Decision log.
- Observation (measured 2026-10-13, EP-M1 batch 2): **the `make test` failure at
  `65aba802` is the down-`sccache` condition, not the per-test allowance.** Two
  attempts against the same revision, same clean tree, one test, two different
  signatures: attempt 1 **returned** a `launch-failed` spawn refusal at 141 s,
  attempt 2 was **terminated** by nextest at 306.6 s
  (`TIMEOUT [306.613s] … production_build_module_slice_has_expected_boundary`,
  summary
  `3896/3912 tests run: 3895 passed (16 slow), 1 timed out, 6 skipped` — zero
  failures). Both logs carry `the managed sccache server is unreachable` —
  **574** lines in attempt 1, **573** in attempt 2. `sccache --show-stats`
  reports `Compile requests 0`, i.e. the server was *reset*, not idle, so every
  nested compile ran from scratch and the production dependency graph blew the
  300 s cap on its own. **Correction (measured 2026-10-13, EP-M2 batch 4):**
  this entry originally called that line count "the decisive measurement" and
  contrasted it with a threshold of **1** such line for a pass. The count does
  **not** discriminate. The same 573 lines appear in the `test-nextest` phase
  of both a **TIMEOUT** (`300.007 s`) and a **PASS** (`290.107 s`), on
  byte-identical bytes bound to content hash `2d7aadbd…`, differing only in
  host load at the moment the test ran. The count identifies that the cache was
  down; it does not predict the verdict, and a future timeout must not be read
  as branch-introduced on the strength of it. Evidence:
  `/tmp/test-netsuke-6-1-2-…out` (attempt 2, RC=2, 700 s),
  `/tmp/batch2-65aba802-test-attempt1-netsuke-6-1-2-…out` (attempt 1, preserved
  byte-identical, 4860 lines), and `build-limits-status` reporting
  `sccache unreachable, compiling directly`. Impact: the tempting reading —
  invited by the configuration's own comment at `.config/nextest.toml:84-92`
  that "a margin of 38 s on a 300 s cap is not a budget" — is that the
  allowance is too small. It is at best secondary: **a widened timeout would
  not have saved either attempt while the cache was down.** Cache first, budget
  second. The failing test is untouched by this branch
  (`git diff --name-only origin/main...HEAD` does not include
  `tests/build_module_slice_ui_tests.rs`), so no commit here can be the cause.

- Observation (measured 2026-10-13):
  `production_build_module_slice_has_expected_boundary`
  (`tests/build_module_slice_ui_tests.rs:81`) is a named member of the
  `nested-cargo-builds` test-group (`.config/nextest.toml:66`, whose
  `max-threads = 1` serializes it), but that override sets only `test-group` and
  `success-output` — **no `slow-timeout`** — so group members keep the
  profile's 300 s while queueing behind one another. Impact: this is a real
  finding and it is **deliberately not actioned in EP-M1**. EP-M2 and EP-M3 add
  further nested-Cargo members to the same serialized group, so an allowance
  calibrated now would be calibrated against today's membership and rot as the
  group grows; the decision is deferred to EP-M4, where the full membership
  exists and both halves can be measured together. A second reason it cannot be
  done here: widening an allowance to turn a gate green is the move the budget
  question exists to prevent, and EP-M1 is a gate-verification milestone, not a
  test-infrastructure one.

  **Superseded in part (measured 2026-10-13, EP-M2).** The widening deferred
  above was performed on `main` by a commit this branch does not contain:
  `1a5fc57f` ("Preserve project provenance across extends for
  environment-access policy (#844) (#906)") splits the combined override and
  gives `packaged_manifest_retains_build_script_sources` its own
  `slow-timeout = { period = "60s", terminate-after = 10 }`, i.e. **600 s**.
  That is the very test that timed out on this branch at `300.007 s`, so the
  timeout recorded above is a *base* property and not a candidate defect — and
  once this branch rebases, the timeout should stop recurring for that test.
  Two consequences follow for the text above. The reasoning that an allowance
  calibrated now "would rot as the group grows" is not falsified, but its
  remedy has already been applied upstream for this member, and the
  "deliberately not actioned" note describes a choice that `main` no longer
  offers. The line citation `.config/nextest.toml:84-92`, and the `:66`
  citation below it, are **base-relative and will shift** under that rebase: at
  this branch's base, `:84-92` holds the "a margin of 38 s on a 300 s cap is
  not a budget" comment the text quotes, while at `origin/main` the same range
  holds the override's rationale. Re-derive both anchors after rebasing rather
  than trusting the numbers.

- Observation (measured 2026-10-13): **`doctest` is the second prerequisite of
  the documented `make test` (Makefile:226) and had never run at any revision
  across three attempts.** The pattern-safe check is
  `grep -c 'Doc-tests ' <log>` → `0`, confirmed on both `make test` attempts;
  the single `test result:` line in those logs belongs to a nextest child
  process, not to rustdoc. Impact: `make test` could not be recorded as
  exercised while half of it was unrun, so `make doctest` was run on its own at
  the frozen revision and **passed**: `GATE=doctest RC=0 SECONDS=100`, tree
  clean before and after, **two** targets (`Doc-tests netsuke`,
  `Doc-tests test_support`) and **three** `test result:` blocks —
  `88 + 2 + 39 = 129` passed, 0 failed, `26 + 0 + 6 = 32` ignored. The
  `netsuke` target emits two blocks because it has a normal and a compile-fail
  phase; the third block is the specific miscount this repository has made
  before, so targets are counted with `grep -E 'Doc-tests '`, never by counting
  result blocks. No warning was emitted, and `RC=0` under `-D warnings` is what
  makes that a gate rather than an absence of evidence.

- Observation (measured 2026-10-13): **the doctest pass's own work is trivial;
  its wall-clock is compilation.** The accepted run reports
  `all doctests ran in 1.38s` against 100 s of gate wall-clock, and a prior
  netsuke doctest log (`/tmp/GATE6-issue-875-doctest.out`) showed the same
  shape. Impact: the cost of `make doctest` tracks target warmth and `sccache`
  availability, not the size of the doctest suite — which is why the run above
  took 100 s on a warm tree, and why a cold, cache-down tree can turn it into a
  multi-hour stall. A stall is distinguishable from slowness cheaply: read a
  candidate process's tick counters in `/proc/<pid>/stat` (fields 14 + 15)
  twice, about 20 s apart — roughly 1 tick across 20 s means idle-blocked on a
  lock or a build-limits slot, not slow. Cross-check `build-limits-status` for
  `held 4` with a queue behind it. The risk did not materialize here, and the
  run was bounded to about 15 minutes before it would have been stopped and
  reported as a finding, because a previous run of this same target hung for
  three hours holding a slot after emitting only its first two crates, and a
  dead parent never reports.

- **`coderabbit review --agent --committed` reviews the branch against its
  base, not the tip commit — and the supervisor's wrong-base test was wrong.**
  The scrutineer was briefed that a finding naming a `.rs` path would indicate
  a review-base error, on the belief that the commit under review was a single
  Markdown file. Measured: the tip commit is one file (`88 0` insertions and
  deletions), while the branch diff against `b14bde20` is five files including
  `tests/env_access_suppressions/roots.rs`, which carries a real
  `22 insertions, 10 deletions` change from Stage A. A finding against that
  file would therefore have been a **correct** finding about real code, not a
  base error, and the briefing's rule would have suppressed it. The scrutineer
  declined to apply the rule and said so. Impact: two statements must not be
  used as tests — "no `.rs` path may be named" is not a wrong-base test (the
  checkable identity is `comparisonBaseCommitId` equalling the PR base), and
  "purely additive, 0 deletions" is a property of the **tip commit**, not of
  the branch, whose diff carries 10 deletions. Both are recorded so a future
  milestone does not re-import them as rules.
- **The scrutineer verified the reviewed revision from two independent sources
  rather than the tool's self-report**, and that precedent is worth keeping:
  the review's own `git.json` (`head`, `baseCommitId`,
  `comparisonBaseCommitId`), `internalState.json` (`reviewedCommitIds`,
  `crReviewed`), and `.session-complete-v2` showing a finished rather than
  truncated review. It also re-derived the `.verus` thirteen-file count,
  confirmed `git check-ignore` resolves `.verus` to `.gitignore:21`, and ran the
  `-ize` spelling scan **with a liveness control** (4 `-ize` forms present in
  the same text) so that a zero-violation result was not vacuously empty.
  Impact: the zero-findings result is corroborated by checks the supervisor did
  not perform, not merely asserted.

## Decision log

- Decision: expose the narrow API `CanonicalKey`, `CanonicalValueError`,
  `canonical_key(&Value) -> Result<CanonicalKey, CanonicalValueError>`,
  `canonical_eq(&Value, &Value) -> Result<bool, CanonicalValueError>`, and
  `stable_deduplicate(&[Value]) -> Result<Vec<&Value>, CanonicalValueError>`
  from `src/stdlib`; keep the implementation module private. `CanonicalKey`
  implements `Eq`, `PartialEq`, and `Hash` over its private bytes and has no
  unchecked constructor. `CanonicalValueError` is a semantic `thiserror` enum
  covering unsupported kind, non-string map key, invalid traversal, non-finite
  number, and canonicalizer failure; structured access identifies the rejected
  kind. Validate the left operand first, then the right; an error terminates
  the operation, but no successful false/true result bypasses either operand.
  Deduplication reports the earliest invalid input index without partial
  output. Rationale: later helpers and embedding callers need a common pure
  contract; first representatives remain original values without cloning or
  reconstruction. `canonical_eq` must derive both keys before comparing.
  Date/Author: 2026-10-13, planning draft.
- Decision: snapshot accepted MiniJinja values exactly once into a private
  controlled tree with explicit Null, Bool, String, I128, U128, F64, Seq, and
  Map variants (or a semantically equivalent representation). Pass this tree
  directly to the canonicalizer. Enumerate maps once, reject non-string keys
  deterministically, and sort string keys before visiting values. Rationale:
  prevents coercive serializer conversions and avoids re-reading
  mutable/dynamic objects. Reject non-finite floats and unsupported kinds
  before serialization. Date/Author: 2026-10-13, planning draft.
- Decision: accept finite, stable, acyclic Map/Seq custom objects under a
  documented host-value precondition; reject Plain, Iterable-only, Bytes,
  Undefined, Invalid, `now()`, nested non-string map keys, and failed
  iterators. Do not claim the adapter detects cycles or arbitrary host effects.
  Rationale: MiniJinja uses objects for containers and exposes no public object
  identity needed to distinguish or cycle-check every implementation.
  Date/Author: 2026-10-13, planning draft.
- Decision: validate all input items and return no partial deduplication result
  if one is unsupported; preserve `uniq` and all template registrations.
  Rationale: deterministic error behaviour and no silent fast-path around
  invalid data; the roadmap requests a foundation, not an implicit filter
  migration. Date/Author: 2026-10-13, planning draft.
- Decision: require Verus for the unbounded first-index kernel, but keep it out
  of ordinary Cargo, CI, and routine gates. Extend both formal-verification
  guidance and the developers' guide's cycle-only Verus statement. Rationale:
  proof is part of this task's feature acceptance, while the existing project
  policy keeps Verus opt-in for normal workflows. Date/Author: 2026-10-13,
  planning draft.
- Decision: pin Verus `0.2026.10.04.426d8b0` in `tools/verus/VERSION`, with the
  `x86-linux` asset digest recorded in `tools/verus/SHA256SUMS`. Rationale: the
  release is published and stable, `prover-tools` resolves both files by fixed
  path, and the install was verified end to end. Upstream ships no checksum
  file, so the recorded digest is locally computed from three agreeing
  downloads and the file says so, rather than implying an upstream signature
  that does not exist. Date/Author: 2026-10-13, implementation.
- Decision: treat the canonicalizer's binary64 collapse as the accepted
  contract, not a tolerance breach. The snapshot tree still carries `I128` and
  `U128` variants so no precision is discarded on the way in, but the plan
  makes **no** integer-injectivity claim, and the tests assert the collapse
  explicitly (`2^53 + 1` shares a key with `2^53`; `i64::MIN` does not round
  trip). Rationale: RFC 8785 §3.2.2.3 defines the number domain as IEEE-754
  binary64 and RFC 0006 §6.7 adopts JCS verbatim, so the domain is correct as
  specified. The tolerance trigger — "the supported domain must differ from RFC
  0006 §6.7" — is not met: the domain is unchanged and only the previously
  recorded risk is confirmed. Date/Author: 2026-10-13, implementation.
- Decision: treat Unicode normalization as an explicit non-goal. Equality is
  byte-identity over unnormalized JCS output, so NFC and NFD spellings of the
  same grapheme are distinct values with distinct keys. Rationale: RFC 8785
  §3.2.3 sorts raw property names as UTF-16 code units and specifies no
  normalization step, so adding one would deviate from the adopted contract and
  from RFC 0006 §6.7. MiniJinja and Rust likewise do not normalize, so no layer
  silently introduces it. Documented rather than implemented. Date/Author:
  2026-10-13, implementation.
- Decision: dispatch on `ValueKind` and then `as_object()`/`ObjectRepr` in the
  snapshot adapter; do not use `Value::try_iter` to discover container shape.
  Rationale: `try_iter` is reachable for scalars only as an error, cannot
  distinguish a map from a sequence, and for a map yields keys rather than
  pairs, so a `try_iter`-driven snapshot would silently emit a map as an array
  of its keys. Pair iteration requires `ObjectExt::try_iter_pairs` on the
  `DynObject`, and `as_object()` is `None` for every scalar. Date/Author:
  2026-10-13, implementation.
- Decision: the adapter rejects non-string map keys itself, before snapshotting
  values, and does not rely on the canonicalizer to reject them. Rationale:
  measured behaviour shows an integer-keyed map reaching JCS as `{"1":"x"}` —
  `serde_json`'s key serializer quotes numeric keys and `JcsFormatter` writes
  them without the binary64 cast, so no layer raises an error. Date/Author:
  2026-10-13, implementation.
- Decision: ship the pinned Verus toolchain and the `.verus/` ignore/skip
  repair as one commit with its ExecPlan record. Rationale: they are one
  logical unit of work, not three — the pin files are what make
  `make install-verus` reproducible, and the install they enable is what writes
  the directory that broke a deterministic gate. Splitting them would leave a
  published pin whose documented use breaks `make test` on the next machine,
  which is a defect the pair exists to remove. Date/Author: 2026-10-13,
  implementation.
- Decision: `make test-workflow-contracts` joins the directed gate set for
  every remaining batch on this branch, and the scope repair is committed on
  its own rather than folded into the milestone. Rationale: the Kani
  proof-scope contract is a *derived* whole-crate property, so it can be broken
  by a change that edits no contract test at all — which is exactly what
  happened, and the cost was a red CI run discovered one revision late. Scoping
  the gate set by "which files did I edit" is the reasoning that missed it, so
  the set now includes the target unconditionally instead of on a
  change-detection rule that has already been shown to under-approximate.
  Date/Author: 2026-10-13, implementation.

## Outcomes & retrospective

Draft outcome: this plan defines the canonical domain, concrete Rust API,
first-wins semantics, host-object precondition, numerical limits, and
proof/test responsibilities. It deliberately leaves `uniq` unchanged. No
feature result is claimed yet. Wyvern reconnaissance and the
Pandalump/Wafflecat, Telefono/Doggylump, and Buzzy Bee/Dinolump review pairs
are complete. Review changed the initial key-only proof into source-index
refinement, made host-object preconditions explicit, fixed one-snapshot
admission and numeric semantics, and required both formal-policy documents to
be updated. At implementation closeout, record actual supported kinds, proof
boundary, test evidence, documentation changes, and residual resource risks;
only then mark roadmap item 6.1.2 complete.

## Context and orientation

Netsuke expands YAML manifests with MiniJinja and compiles them to a
deterministic Ninja graph. Standard-library implementations live under
`src/stdlib`. `src/stdlib/collections.rs` registers `uniq`, `flatten`,
`compact`, and `group_by`; `uniq` currently uses MiniJinja equality. The new
private module belongs under `src/stdlib`, with only the stated foundation
re-exported.

RFC 0006 §6.7 defines equality as byte equality of RFC 8785 canonical JSON
produced by `serde_json_canonicalizer`. RFC 0006 §6.8 defines shared input and
materialization bounds, which roadmap 6.1.3 implements. RFC 0006 §6.9 defines
typed diagnostics, with Fluent mapping in 6.1.4. RFCs 0013 and 0014 constrain
structured data and mapping conversions; in particular a lossy conversion
cannot establish source equality.

The repository's Makefile delegates Verus installation and execution through
the pinned `rust-prover-tools` source. There is no existing Verus model for
canonical values, so this plan adds a standalone model under `verus/`, outside
Cargo. The model uses abstract canonical byte keys and source indices; it does
not prove MiniJinja or the third-party canonicalizer.

## Conformance basis

- Baseline: repository state `b14bde201810a945b17ad484af3bd46749de2dad`.
- `docs/terms-of-reference.md`, Draft v0.2, last revised 2026-09-25:
  deterministic build graph, purity, and trust boundaries.
- `docs/netsuke-design.md`, “Generic collection filters”: current filter
  placement and `uniq` behaviour, which this step preserves.
- `docs/roadmap.md` 6.1.2 (`EP-RQ-612`): existing dependency, unsupported-kind
  errors, order-preserving helper, and property criteria.
- RFC 0006 §6.7 (`EP-RQ-JCS`): canonical equality, key sorting, sequence order,
  typed rejection, and first-wins deduplication. RFC 0006 §6.8 (`EP-RQ-BOUNDS`)
  leaves shared bounds to 6.1.3; §6.9 (`EP-RQ-DIAGNOSTICS`) leaves localized
  codes to 6.1.4.
- RFC 0006 §11.3 and `src/stdlib/collections.rs`: preserve `uniq` and
  `group_by` behaviour and registrations.
- RFCs 0013 §§5.5, 5.7, 8.1 and 0014 §5.7: serializer coercion, key domains,
  and numeric round-trip constraints.
- RFC 8785 §§3.1, 3.2.2.3, and Appendix B, plus the existing
  [`serde_json_canonicalizer` 0.3.2 API](https://docs.rs/serde_json_canonicalizer/0.3.2/serde_json_canonicalizer/):
  external canonicalization contract. Third-party internals are axioms;
  Netsuke-owned adapter behaviour is tested against the actual interface.
- `docs/formal-verification-methods-in-netsuke.md` and
  `docs/developers-guide.md` Verus policy: this approved work explicitly adds a
  narrow optional kernel without making it an ordinary gate.
- Required references: `docs/rust-testing-with-rstest-fixtures.md`,
  `docs/rust-doctest-dry-guide.md`,
  `docs/reliable-testing-in-rust-via-dependency-injection.md`,
  `docs/ortho-config-users-guide.md`, and `docs/rstest-bdd-users-guide.md`.
  These govern fixtures, doctests, injected dependencies, diagnostic
  boundaries, and BDD scenarios.
- Skills: `execplans`, `codegraph-mcp`, `hexagonal-architecture`, `rust-router`,
  `rust-verification`, `proptest`, `verus`, `firecrawl-mcp`, and
  `logisphere-experts`.

Trace:

```plaintext
RFC0006-6.7 + ROADMAP-6.1.2
  -> EP-M1 domain/API characterization
  -> EP-M2 canonical key and equality
  -> EP-M3 stable dedup + tests + Verus proof
  -> EP-M4 design/proof documentation + sequential gates
```

No change to the Terms of Reference or a new ADR is currently required. If the
implementation changes the published standard-library contract or establishes a
durable proof-driven architecture decision, update the design document and add
an ADR only when the decision warrants a separate record.

## Verification plan

The abstract model accepts a finite sequence of byte keys `K = [k0, ..., kn]`.
Define output as the increasing source-index sequence `R` where index `i` is
retained exactly when no earlier key `K[j]`, `j < i`, equals `K[i]`. The Rust
helper then returns `&input[R[t]]`. This specification does not claim JCS keys
distinguish source numbers that JCS serializes to the same IEEE-754 value.

### EP-V1 — equality is an equivalence relation

Statement: for admitted values, key equality is reflexive, symmetric, and
transitive. Method: Proptest over recursively generated values plus named
`rstest` cases. Domain: null, booleans, strings/Unicode, finite numbers, nested
arrays/maps, insertion-order permutations, signed zero, and JCS numeric
boundaries. Artefact: tests inside `src/stdlib/canonical_value/mod.rs`.

Evidence: `cargo nextest run --lib canonical_value` fails before implementation
for absent behaviour, passes after it, then `make test` passes. The generator
constructs valid values directly rather than filtering arbitrary values; report
classification counts for every material class. Include equivalent triples
constructed independently and unlike values. A negative control that retains
source map insertion order in the key must fail on reordered-map fixtures;
another that inverts equality must fail reflexivity. A reversed operand
comparison alone is not a sufficient fault because equality remains symmetric.

### EP-V2 — admission and canonical bytes match the contract

Statement: reordered object members produce equal keys, sequence order matters,
unsupported nested keys/kinds fail before coercion, and emitted bytes match the
canonicalizer on selected JCS fixtures. Method: finite `rstest` cases and real
MiniJinja/canonicalizer integration tests. Artefact:
`src/stdlib/canonical_value/mod.rs` tests.

Domain: nested integer/boolean map keys, Undefined, callable, `now()` object,
Plain/Iterable/Bytes/Invalid values, finite stable acyclic custom Map/Seq
objects, failing Map/Seq iterators, non-finite floats, `-0.0`/`0.0`,
`2^53 + 1`, minimum/maximum `i64`, `u64`, `i128`, `u128`, exponent boundaries,
and RFC 8785 Appendix B cases. The snapshot tree retains explicit Null, Bool,
String, I128, U128, F64, Seq, and Map representations, then serializes that
snapshot once.

Evidence: direct assertions check exact keys/error kinds. Require `2^53 + 1` and
`2^53` to share a JCS key, and negative/positive zero to share a key. Exercise
extreme integers through the direct canonicalizer interface to confirm accepted
range. A nested-key validation omission must fail on a non-string nested key; a
coercive integer-key conversion must fail on a collision fixture; re-reading a
mutating custom object must fail a one-snapshot test. No snapshot file is
warranted unless genuinely multivariant diagnostic rendering is added. No
CLI/end-to-end test is warranted because this step adds no user-facing template
operation.

Non-vacuity requires every rejected case to reach the adapter and every
admitted class to reach the actual canonicalizer. Hostile or cyclic custom
objects are outside the enforceable contract; document the finite, stable,
acyclic host-value precondition rather than claiming those inputs are detected.

### EP-V3 — stable deduplication returns first representatives

Statement: output references point to increasing least-occurrence indices;
every distinct key appears once; all input keys are represented; and later
equal values never replace their first representative. Method: Proptest against
an independent quadratic first-index oracle, plus a standalone Verus proof over
the abstract kernel and a runtime correspondence test. Artefacts:
`src/stdlib/canonical_value/mod.rs` and `verus/canonical_value/main.rs`.

The proof model processes `(index, key)` pairs left to right, maintains
abstract seen keys and retained source indices, and proves that the final index
sequence equals the specification's least-index sequence. Prove input-range,
strict index ordering, key uniqueness, coverage, and minimality. Do not claim
the proof establishes the MiniJinja-to-key conversion; the implementation must
mirror the model's append-or-skip algorithm, while runtime tests compare
returned reference identity with the independent source-index oracle.

At processed index `i`, establish `seen = {K[j] | 0 <= j < i}` and that
retained indices are exactly the least occurrences in that prefix, in
increasing order. Prove that each membership-test append/skip branch preserves
this representation relation and refines the independent least-index
specification. The runtime loop must use those same state transitions; review
that correspondence and test the actual Rust implementation. This is an
abstract algorithm refinement proof, not a claim that Verus verifies Rust's
`HashSet` internals or the entire production adapter.

Domain and witnesses: empty, singleton, all-distinct, all-equal, repeated
values at head/middle/tail, interleaved duplicates, reordered map duplicates,
order-distinct sequences, and generated mixed values. Generator classification
must show each shape is reached without excessive filtering. A negative control
that retains the last duplicate or emits membership-set iteration order must
fail. The runtime oracle computes each key then scans prior keys quadratically;
pointer identity verifies retained values are the original first input items.

Before running the proof, add the chosen upstream version to
`tools/verus/VERSION` and checksum data to `tools/verus/SHA256SUMS` according to
`docs/formal-verification-methods-in-netsuke.md`. Verify the pinned verifier
and its checksum, then run `make install-verus` if absent and:

```sh
make verus VERUS_FLAGS='--proof-file verus/canonical_value/main.rs'
```

Run it from the worktree root. `--proof-file` is a real option of
`prover-tools verus run`, and so is `--repo-root` (`verus run --help` lists
`--repo-root PATH [env var: INPUT_REPO_ROOT]`); both were accepted as the
planning-time note said, and the intervening claim that `--repo-root` did not
exist is withdrawn above. Running from the worktree root remains the ordinary
form because `repo_root` defaults to the working directory, which is where the
pin files resolve from. Proof acceptance requires satisfiable witnesses,
reviewed trusted assumptions, and no `assume`, `admit`, external body,
contradiction, or theorem equivalent to an assumption. Verus is required for
feature completion, but remains outside the ordinary gates.

### EP-V4 — public purity and ordering boundary

Statement: no public output order depends on membership storage; no Netsuke
environmental inputs enter the canonical kernels. Method: API review and repeat
tests over different map insertion orders. Artefacts: public exports in
`src/stdlib/mod.rs` and tests in `src/stdlib/canonical_value/mod.rs`.

Evidence: repeated calls return identical keys and first-wins references. Use a
witness with at least two duplicate classes whose first-occurrence order
differs from lexical/hash order. Complexity is proportional to snapshot
traversal and serialized bytes, key sorting per map, and retained unique key
bytes; no unbounded embedding-input resource guarantee is claimed.

### External assumptions and proof limits

The proof assumes the mathematical semantics of byte equality and its own
finite-sequence model. The runtime adapter relies on documented MiniJinja value
and object interfaces, the caller's finite/stable/acyclic host-object
precondition, and the documented RFC 8785 behaviour of
`serde_json_canonicalizer`. Do not attempt to prove third-party internals.
Proptest and direct fixtures verify the repository-owned snapshot/serialization
boundary. Kani is not warranted: the core contract applies to unbounded
sequence lengths rather than an interesting finite state bound.

## Plan of work

### Stage A — characterize the runtime domain

Inspect MiniJinja scalar/object representations and iteration APIs, and the
locked canonicalizer's serializer interface. Create an evidence-backed table of
admitted/rejected cases. Confirm that one snapshot can preserve numeric scalar
representations and avoid re-reading objects. Record that arbitrary custom
objects are admitted only under the finite, stable, acyclic host precondition.
**Already done — do not repeat.** `tools/verus/VERSION` and
`tools/verus/SHA256SUMS` exist, added by `aa32c294` through the existing pinned
`prover-tools` path; `VERSION` holds `0.2026.10.04.426d8b0`. This step
originally read "add `tools/verus/VERSION` and `tools/verus/SHA256SUMS` for a
selected upstream release … then verify its checksum", which was correct when
planned and has since been discharged. Verify the pins match the tree and
proceed; **do not** overwrite the digest.

Go/no-go: proceed only if ordinary finite maps/sequences and JSON scalars have
a deterministic non-coercive route to JCS, unsupported values fail before
coercion, and the abstract proof boundary remains independent of MiniJinja.

### Stage B — write failing tests and BDD contract

Add unit/property cases in `src/stdlib/canonical_value/mod.rs`. Add
`tests/features/canonical_value.feature`, bindings in
`tests/bdd/steps/stdlib/canonical_value.rs`, and wire the steps/scenarios
through `tests/bdd/steps/stdlib/mod.rs`. Add isolated scenario state under
`tests/bdd/fixtures/` and connect it to `TestWorld` in
`tests/bdd/fixtures/mod.rs`; use no process-global mutable state.
`tests/bdd_tests.rs` already discovers `tests/features`, so retain that
entrypoint without redundant scenario registration. Describe the Rust boundary;
do not imply a Jinja filter exists. Scenarios:

```gherkin
Feature: Canonical value identity

  Scenario: Reordered maps have the same canonical key
    Given two string-keyed maps with identical members in different orders
    When their canonical keys are derived
    Then the keys are equal

  Scenario: Deduplication keeps the first equal value
    Given a materialized sequence with repeated canonical values
    When stable deduplication is applied
    Then the original first values appear once in first-appearance order

  Scenario: Unsupported values fail before coercion
    Given a value outside the canonical JSON domain
    When its canonical key is requested
    Then a typed error identifies its value kind
```

Run the red stage with `cargo nextest run --lib canonical_value` and
`cargo nextest run --test bdd_tests canonical_value`. Failure must be due to
missing/incorrect behaviour, not malformed feature wiring. Verify that each
focused filter selects nonzero tests; use the generated scenario names reported
by Nextest if its naming convention differs. Use `rstest` fixtures,
`googletest` matchers for predicates and typed errors, and `pretty_assertions`
for expected sequences/bytes. Keep canonical admission tests separate from the
index-selection oracle so a common conversion fault cannot bless both.

### Stage C — implement key, equality, stable dedup, and proof together

Add private `src/stdlib/canonical_value/mod.rs` with child modules
`admission.rs`, `error.rs`, and `tests/mod.rs` as needed to keep every Rust
file under 400 lines. The admission module owns MiniJinja traversal; the
private snapshot tree, canonical key, and index selection have no MiniJinja
dependency. Document permitted callers and composition in
`docs/netsuke-design.md` and `docs/developers-guide.md` before extracting
shared helpers. Recheck the repository for equivalent abstractions;
`src/hasher.rs::ActionHasher::hash` uses the canonicalizer but is an action
digest, not a reusable value key. Implement the public API from
`src/stdlib/mod.rs`. The snapshot tree must validate recursively, retain
numeric kinds, reject unsupported map keys/values, and be the only value passed
to JCS. Equality derives both keys before comparing. Stable dedup traverses the
full input in order, validates every value, records membership privately, and
returns original references at first occurrence. On failure return an error
without partial output.

Implement `verus/canonical_value/main.rs` alongside the algorithm. Prove the
abstract least-index model with source indices and constructive append/skip
steps. Review the trusted base and model correspondence. Keep Verus files
outside Cargo and ordinary CI.

Run focused tests for green. Run the Verus command above and require proof
success before declaring this plateau complete.

### Stage D — document and validate

Update `docs/netsuke-design.md` with the canonical foundation and unchanged
`uniq` semantics. Update `docs/formal-verification-methods-in-netsuke.md` and
`docs/developers-guide.md` to admit canonical deduplication as a second narrow
Verus kernel; the developers' guide currently says first Verus work must focus
on cycle canonicalization. Preserve its rule that Verus remains outside normal
Cargo and routine CI. Update `docs/developers-guide.md` with the host-object
precondition and reuse boundary if that practice will guide future helper
authors. No users' guide change is needed because no template helper is
registered. Add an ADR only if implementation reveals a durable decision beyond
RFC 0006.

After each coherent implementation plateau, run required gates sequentially:
`make check-fmt`, `make lint`, `make doc-coverage`, `make test`. When docs
change, first run `make fmt`, then run `make markdownlint` and `make nixie`. At
final acceptance run every listed gate and the proof. Capture each gate's
output under `/tmp` as repository instructions require; never overlap
formatting, lint, and test gates.

At each boundary trace `EP-RQ-612` and `EP-RQ-JCS` to evidence; confirm no
change to existing `uniq`/`group_by` registrations, no pull-forward of 6.1.3 or
6.1.4 scaffolding, no new dependency/trust boundary, and no exposed hash order.

## Milestones and plateaus

### EP-M1 — characterized domain and toolchain

Outcome: an evidence-backed supported-value matrix, concrete API, and pinned
Verus release/checksum exist. Requirements: clarify EP-RQ-612 and EP-RQ-JCS.
Acceptance: tests can construct each admitted/rejected kind; extreme numbers
and JCS boundaries are exercised; `prover-tools`/Make flags and verifier
checksum are confirmed. Conformance: RFC domain preserved; ordinary
maps/sequences usable; no coercion, new template name, dependency, wire format,
or unapproved trust boundary. Recovery: retain characterization only and revise
this plan if the contract cannot be met. Remaining: implementation/proof/docs.
Compatibility: none.

### EP-M2 — canonical key and equality

Outcome: typed keys and equality operate on supported values; errors occur
before coercion; existing template behaviour is unchanged. Requirements:
EP-RQ-JCS key semantics; advance EP-RQ-612. Acceptance: EP-V1/EP-V2 tests and
Rust BDD scenario pass, including invalid self-comparison and validation of
both operands. Conformance: recursive one-time snapshot; no error localization
or bounds pulled forward; no new dependency/registration; trace links current.
Recovery: retain red tests and revert incoherent implementation changes.
Remaining: stable dedup and proof. Compatibility: none.

- (2026-10-13) Implementation landed under `src/stdlib/canonical_value/` behind
  exactly five public names (`CanonicalKey`, `CanonicalValueError`,
  `canonical_key`, `canonical_eq`, `stable_deduplicate`), wired into
  `src/stdlib/mod.rs` by one `mod` line and one `pub use`. Focused reds were
  observed before green: the first library run failed three cases on behaviour
  (a test-index bug, generator starvation, and an incorrectly stated `now()`
  kind), and no red in this packet was a compile error because the library
  compiled before the test file did. A supplementary filtered doctest run found
  a real defect the gates would not have: `context!` has no array arm, so the
  module example's heterogeneous array failed to compile (E0308) and was
  corrected.
- (2026-10-13) The filters were shown to discriminate behaviour rather than
  merely pass: one surgical mutation to `stable_deduplicate` (retain every
  value) turned the library filter red in 2 cases and the BDD filter red in 1,
  and both were re-run green after the revert. The measured `now()` kind is
  `Plain`, not `Iterable` as the RFC text's shape suggests; the test asserts
  the measured kind and says so, because a kind-less reading of that fixture
  could silently assert the wrong thing.
- (2026-10-13) The test battery did **not** fit one file: a single `tests.rs`
  reached 854 lines, 2.1x the 400-line `module_max_lines` cap, and the sibling
  battery was 432. A census settled the shape — of the nine `.rs` files over
  400 lines in this repo, all nine are `tests/*.rs` crate roots and no nested
  file exceeds the cap — so the battery was split by *kind of judgement* into
  `mod.rs` (fixtures and helpers), `boundaries.rs` (refusals), `parity.rs`
  (canonicalizer parity), `properties.rs` (generated inputs), and `examples.rs`
  (handwritten inputs). The 19 cases are unchanged, verified by identical
  counts before and after. This is a house-shape decision, not a line-count
  dodge: `src/stdlib/network/tests/` already nests a `mod.rs` beside siblings.

### EP-M3 — first-index dedup and Verus proof

Outcome: helper returns original first references in order and the abstract
unbounded kernel is proved. Requirements: complete EP-RQ-612 and first-wins
EP-RQ-JCS. Acceptance: EP-V3 Proptest/oracle/BDD tests pass; seeded last-wins
or hash-order fault is rejected; Verus proves index
range/order/coverage/uniqueness/ minimality without shortcuts. Conformance:
runtime tests cover the bridge; proof is not described as proving serializer
correctness; Verus stays outside ordinary gates; trace links current. Recovery:
stop and revise the proof plan if assumptions or actual tool flags differ.
Remaining: docs/final gates. Compatibility: none.

### EP-M4 — documented delivery

Outcome: design and proof guidance explain the contract/limits; sequential
gates pass; roadmap item 6.1.2 is marked done. Acceptance: updated docs, all
gates and proof pass, and no user guide entry claims a nonexistent template
helper. Conformance: upstream assumptions reconciled; no hidden CI gate/new
dependency/ format; trace links current. Recovery: preserve last passing
commit, repair the failure, then rerun affected gates and all dependent gates.
Remaining: later helpers decide their own bounds, diagnostics, and integration.
Compatibility: none.

## Concrete steps

Run from the repository root:

1. Inspect current dependency and filter behaviour:

   ```sh
   rg -n 'serde_json_canonicalizer|uniq_filter|group_by_filter' Cargo.toml src/stdlib
   ```

2. Inspect MiniJinja/canonicalizer locked source APIs and construct direct
   fixtures for scalar kinds, custom Map/Seq, iterator failures, nested invalid
   keys, and numeric extremes. Record the admitted tree representation.

3. Red: add unit/Proptest tests to `src/stdlib/canonical_value/mod.rs`, BDD
   feature and bindings at the named paths; run:

   ```sh
   cargo nextest run --lib canonical_value
   cargo nextest run --test bdd_tests canonical_value
   ```

4. Green: implement the smallest canonical tree, key, equality and dedup
   behaviour. Rerun both focused commands. Refactor without changing behaviour
   and rerun them.

5. Verify pinned Verus version/checksum from `tools/verus/VERSION` and
   `tools/verus/SHA256SUMS`; install through `make install-verus` if required;
   then run:

   ```sh
   make verus VERUS_FLAGS='--proof-file verus/canonical_value/main.rs'
   ```

6. Run each full quality gate sequentially after each major plateau, with logs
   under `/tmp`:

   ```sh
   make check-fmt
   make lint
   make doc-coverage
   make test
   make markdownlint
   make nixie
   ```

   Markdown/Mermaid gates apply when docs change. Run all six at final
   acceptance. Add the Verus command to EP-M3 and final acceptance.

7. Review the final diff for the public export list, unchanged filters, proof
   assumptions, source file lengths, docs, and roadmap state before committing
   each coherent change. Use the commit-message skill, a message file, and
   gated atomic commits; do not commit an intentionally red test state.
   Preserve proof logs and red/green evidence in this plan as work progresses.

## Validation and acceptance

Red-Green-Refactor is available and required. Red tests fail because the
canonical key/dedup behaviour is absent or wrong. Green passes after the
minimal implementation. Refactor changes structure only and focused tests plus
plateau gates pass again. Do not leave expected-failure markers in the
completed suite.

The contract example uses a materialized input so references have clear
provenance:

```rust
let input = [map_a, map_b, reordered_map_a];
let retained = stable_deduplicate(&input)?;
assert_eq!(retained.len(), 2);
assert!(std::ptr::eq(retained[0], &input[0]));
assert!(std::ptr::eq(retained[1], &input[1]));
```

The final names should match the API in `Constraints`. Completion requires
equivalence properties over generated admitted values; first-index properties
against an independent quadratic oracle; typed unsupported-value errors before
coercion; successful unbounded Verus proof; runtime tests against MiniJinja and
the canonicalizer; unchanged template registrations; no public hash iteration
order; and all required sequential gates passing. Only then mark roadmap 6.1.2
done.

The applicable set is the eight targets `check-fmt`, `lint`, `doc-coverage`,
`markdownlint`, `spelling`, `nixie`, `test`, and `test-workflow-contracts`, plus
`test-documentation-contracts` when a marked example changes,
`test-coverage-artifact` when a validator under `scripts/` changes, and
`make verus` once EP-M3's proof file exists. `test-workflow-contracts` is
unconditional rather than change-scoped: its Kani proof-scope contract derives
a whole-crate closure, so an edit to an unrelated module can break it without
touching anything under `tests/workflow_contracts/`. That is what happened at
`adbb8d61`, and a change-scoped rule would miss it again.

## Revision note

- 2026-10-13: Initial DRAFT from roadmap 6.1.2, RFC 0006 §6.7, repository
  reconnaissance, Firecrawl check of RFC 8785, Wyvern findings, and six-lens
  review. Defined the host-object boundary, numeric semantics, exact APIs and
  test paths, an index-based proof sketch, and the required documentation
  policy update. No implementation started; approval remains required.

- 2026-10-13: Final expert readback accepted the revised design. Recorded
  documentation-gate evidence, explicit prefix-state refinement, original-value
  identity assertions, module ownership, and existing BDD fixture/discovery
  wiring. That readback was design review only; it did not establish feature
  implementation or discharge the proof obligations, and the status it recorded
  is superseded by the entry below.

- 2026-10-13: Moved to `IN PROGRESS` with the receipt of execution approval,
  which the invocation naming this plan supplies. Stage A landed the
  `CanonicalKey`, `CanonicalValueError`, `canonical_key`, `canonical_eq`, and
  `stable_deduplicate` APIs with their admission layer and property tests; the
  milestone CLI review then raised five distinct concerns, all verified against
  source and disposed of at EP-M2. Two were repaired (`admit_mapping` no longer
  clones the member key; the machine-local-directory comment says the verifier
  is *pinned*), one was refused with rationale (the `sort_members` non-string
  guard is load-bearing for order-independence), and two tightened weak
  assertions. The remaining obligations — the index-based Verus proof, the
  independent quadratic oracle, and the documentation delta — are planned as
  EP-M3 and EP-M4 and are **not** discharged by Stage A.

- 2026-10-13: EP-M2 closed. A second milestone CLI review round raised three
  findings across two presentational concerns, both repaired and both gated.
  The milestone's own evidence then falsified a claim this plan had recorded:
  the `sccache` line count does not discriminate a passing `make test` from a
  timing-out one, demonstrated by a `TIMEOUT` and a `PASS` carrying the same
  573 lines on byte-identical bytes. The risk entry was corrected in place, the
  Progress and Surprises sections extended, and the candidate re-gated green
  across all eight applicable targets. EP-M3 (Verus proof and oracle) and EP-M4
  (documentation delta) remain outstanding.
