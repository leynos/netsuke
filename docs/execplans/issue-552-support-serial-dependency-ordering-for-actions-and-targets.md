# Issue 552: Support serial dependency ordering with Ninja dyndep

This ExecPlan is a living document. Keep `Progress`, `Surprises & discoveries`,
`Decision log`, and `Outcomes & retrospective` current as implementation
proceeds.

Status: **Complete — implementation and documentation changes pass the full
deterministic suite and independent review.**

Issue: [#552](https://github.com/leynos/netsuke/issues/552)

## Purpose and big picture

Netsuke currently treats every dependency list as an unordered graph. Users who
want an aggregate action such as `all` to run formatting, linting, tests, and
spelling in declaration order must encode orchestration outside the manifest.
The desired syntax is a closed `dependency_order` field on actions and targets:

```yaml
actions:
  all:
    dependency_order: serial
    deps:
      - check-fmt
      - lint
      - test
      - spelling
```

After this change, omitting the field continues to mean `parallel`. A serial
list starts each dependency only after the preceding dependency has completed
successfully, stops before later entries after a failure, and continues to use
one Ninja scheduler so shared work is built at most once per invocation.
Unrelated graph branches remain eligible to run concurrently.

The observable implementation uses Ninja's dynamic dependency feature,
`dyndep`, introduced in Ninja 1.10. Netsuke emits a staged chain of phony gate
edges. Each stage's dyndep file reveals exactly one real dependency, while the
next dyndep file is not available to Ninja until the preceding gate succeeds.
This differs materially from an order-only chain around already-visible
dependencies: Ninja cannot schedule a dependency before its dyndep statement
has revealed it.

The implementation is complete when schema, IR, Ninja generation, dyndep-file
materialization, documentation, and regression tests satisfy every acceptance
criterion in issue #552 and all repository gates pass.

## Constraints

- Use the declarative field `dependency_order`, with a closed enum containing
  `parallel` and `serial`. Do not add a bare Boolean `serial` flag.
- Default to `parallel` so existing manifests and generated Ninja remain
  unchanged.
- Apply the field only to a target or action's `deps` list. It must not reorder
  `sources`, `order_only_deps`, the dependencies of another node, or the global
  worker pool.
- Preserve the user's `deps` declaration order through parsing, IR lowering,
  and Ninja generation.
- Keep one top-level Ninja invocation. Do not implement this feature through
  nested `netsuke build` or nested `ninja` commands.
- Use Ninja dyndep syntax version 1 and require Ninja 1.10 or newer only when a
  generated build uses serial dependency ordering.
- Preserve shared-dependency memoization within the encompassing Ninja build.
- Avoid Ninja pools: depth-one pools provide mutual exclusion, not a
  declaration-order guarantee, and would broaden the serialization scope.
- Treat generated dyndep files as part of the generated Ninja artefact. Never
  print a main Ninja file that silently relies on sidecars Netsuke has not
  materialized.
- Keep generated paths deterministic, content-addressed where appropriate,
  relative to the effective Ninja working directory, and isolated beneath the
  existing `.netsuke` state namespace.
- Use `cap_std`, `cap_std::fs_utf8`, and `camino` for production file access;
  do not introduce `std::fs` or `std::path` production code.
- Materialize sidecars atomically and idempotently so concurrent Netsuke
  processes cannot observe partial dyndep content.
- Do not add a new external crate unless implementation proves that the
  workspace cannot provide the required digest or atomic-write primitive.
- No production Rust source file may exceed 400 lines. In particular,
  `src/ninja_gen/mod.rs` and `src/runner/process/mod.rs` are already near the
  limit, so new responsibilities belong in focused submodules.
- Every new module must begin with a `//!` module-level comment, and every new
  public API must have Rustdoc with a useful example.
- Follow Red–Green–Refactor. Capture the expected failing focused test before
  making it pass, but never commit a state whose required gates fail.
- Update user, design, developer, repository-layout, roadmap, and architectural
  decision documentation as described below.
- Run repository gates sequentially, using the shared Cargo cache. Do not run
  formatting, linting, or test gates in parallel.
- Do not implement this draft until the user explicitly approves it.

## Tolerances

- **Scope:** if correct implementation requires a Netsuke-owned scheduler,
  global graph serialization, or a manifest redesign beyond `dependency_order`,
  stop and obtain approval. Those are architectural scope changes, not
  implementation details.
- **Dependencies:** the target is zero new crates. If an additional crate is
  unavoidable, stop and present the crate, version, maintenance status, and why
  existing dependencies or standard library facilities are insufficient.
- **Generated-output compatibility:** parallel manifests should retain
  byte-for-byte generated Ninja output. If unavoidable output churn appears,
  isolate it, explain it, and obtain approval before refreshing broad snapshots.
- **Public API compatibility:** existing callers of `ninja_gen::generate` and
  `generate_into` must continue to work for ordinary graphs. A graph containing
  serial ordering may require the new bundle API; the string-only APIs must
  return a specific error instead of returning an incomplete build file.
- **Performance:** serial lowering may add one gate and one small dyndep file
  per dependency. It must remain linear in the number of serial dependencies
  and must not traverse unrelated subgraphs repeatedly.
- **State:** `.netsuke/dyndep` is a reusable generated cache. `netsuke clean`
  need not delete it, but stale content must be harmless because filenames are
  content-addressed.
- **Platform support:** sidecar generation must be shell-independent and use
  Rust filesystem APIs so Windows, macOS, and Linux release builds share the
  same behaviour.
- **Compatibility boundary:** a later serial dependency that is also directly
  requested or independently reachable from another requested top-level branch
  may become visible through that other branch and execute early. If acceptance
  requires suppressing that independent reachability, stop: static Ninja cannot
  both keep ordering local and globally delay the shared node.
- **Time:** there is no deadline tolerance. Prefer a correct, reviewable design
  and complete evidence over a rushed implementation.

## Risks

- **Later dependencies can leak through another requested branch.** Dyndep
  delays only the graph path it controls. Ninja unifies nodes globally, so an
  independent path to a later dependency can expose it before the serial gate.
  Mitigation: document the boundary, test that a genuinely unrelated branch
  remains concurrent, and stop for a scheduler-level redesign if stronger
  semantics are required.
- **Incomplete generated artefacts.** A main Ninja file that references absent
  dyndep sidecars fails during Ninja graph loading. Mitigation: introduce a
  bundle type, route every CLI execution and generation path through it, and
  make string-only generation reject serial graphs.
- **Races while writing sidecars.** Two builds may generate the same content at
  once. Mitigation: write a unique temporary file in `.netsuke/dyndep`, flush
  it, atomically rename it, and treat an already-present matching digest as
  success.
- **Synthetic output collisions.** User targets could name a path chosen for a
  gate or dyndep file. Mitigation: reserve `.netsuke/serial` and
  `.netsuke/dyndep` for this feature, reject exact or prefix collisions with a
  localized error, and document the reservation.
- **Freshness propagation can be lost.** Depending only on the final gate may
  not make every real dependency contribute to the aggregate target's dirty
  state. Mitigation: list every gate as an implicit dependency of the annotated
  edge and add repeat-build tests that mutate each dependency in turn.
- **Escaping errors can corrupt dyndep syntax.** Ninja paths have escaping
  rules distinct from YAML and shell quoting. Mitigation: reuse the generator's
  existing path-rendering machinery, test spaces and Ninja metacharacters, and
  avoid manually concatenating unescaped paths.
- **Action outputs are synthetic.** Actions and ordinary targets share the AST
  `Target` shape but lower differently. Mitigation: exercise both forms at AST,
  IR, snapshot, behavioural, and real-Ninja levels.
- **Ninja version failures may be obscure.** Older Ninja releases do not
  support dyndep. Mitigation: emit `ninja_required_version = 1.10` when the
  feature is present and document the resulting minimum version.
- **Source-file size pressure.** Adding logic directly to near-limit modules
  would violate repository policy. Mitigation: establish focused dyndep
  generation and materialization modules before adding the implementation.

## Progress

- [x] (2026-08-10 11:59Z) Inspected issue #552, current AST-to-IR-to-Ninja
  lowering, runner generation paths, behavioural fixtures, snapshots, and
  real-Ninja integration tests.
- [x] (2026-08-10 11:59Z) Falsified the proposed order-only phony gate chain:
  real dependencies remain transitively visible and start in parallel.
- [x] (2026-08-10 11:59Z) Falsified recursive per-dependency Ninja execution:
  separate child schedulers rebuild a shared dependency more than once.
- [x] (2026-08-10 11:59Z) Validated a minimal dyndep chain with real Ninja:
  declaration order, failure short-circuiting, shared work reuse, and unrelated
  branch concurrency behaved as required.
- [x] (2026-08-10 11:59Z) Drafted this self-contained implementation plan.
- [x] (2026-08-10) User approved the plan and its compatibility boundary via the
  implementation request; the developer also received the repository gate list.
- [x] (2026-08-10) Added schema and IR regressions: omission defaults to
  `parallel`, explicit `parallel`/`serial` parse on targets and actions, an
  unknown value such as `sequential` is rejected, and declaration order and the
  `DependencyOrder` survive lowering for both targets and actions.
- [x] (2026-08-10) Implemented the AST and IR representation: the manifest AST
  owns the Serde-enabled `DependencyOrder::{Parallel, Serial}` syntax, while
  the IR owns a serialization-free domain enum with the same variants.
  `from_manifest` converts explicitly between them and preserves dependency
  declaration order. Updated every direct `BuildEdge` literal, doctest, and
  fixture to use the domain type. `cargo check --all-targets` and 739 library
  tests plus the touched integration tests passed.
- [x] (2026-08-10) Implemented deterministic Ninja bundle and dyndep lowering:
  the new `src/ninja_gen/dyndep.rs` submodule adds `GeneratedNinja` (main text
  plus content-addressed `GeneratedDyndep` sidecars) and `generate_bundle`.
  Serial multi-dependency edges lower into one phony gate and sidecar per
  dependency; the version floor is emitted only when gates exist; sidecars are
  content-addressed beneath `.netsuke/dyndep`; and string-only generation
  returns `NinjaGenError::DyndepFilesRequired` without writing partial output.
  Added reserved-namespace collision errors and localization keys across all 35
  catalogues. Unit, integration, and doctests pass.
- [x] (2026-08-10) Implemented atomic dyndep sidecar materialization in
  every CLI path. `src/runner/process/dyndep_files.rs` materializes the bundle
  sidecars beneath `.netsuke/dyndep` relative to the effective Ninja working
  directory, using capability-scoped writes, a same-directory `create_new`
  temporary file, and an atomic rename. Existing content is verified and
  reused; corruption and concurrent-writer outcomes are covered.
  `generate_ninja` now routes every build, clean, and generate invocation
  through `generate_bundle` plus materialization before invoking Ninja. Added
  runtime tests driving real Ninja: strict declaration order, failure
  short-circuiting, and materializer idempotence/corruption paths. Verified
  end-to-end with a real serial manifest and real Ninja 1.11.1 (order observed:
  fmt, lint, test, all). Full suite: 1930 tests pass.
- [x] (2026-08-11 19:43Z) Re-read this plan, the issue, current implementation
  commits, the decision-record convention, and every documentation target named
  in Stage 6 before beginning the documentation milestone. Confirmed that the
  serial implementation uses the planned AST-to-IR-to-bundle-to-materializer
  flow without a scheduler or new dependency.
- [x] (2026-08-11) Documented the manifest syntax and user-visible execution
  contract in `docs/users-guide.md`, including the default, serial scope,
  failure handling, shared-work behaviour, independent-reachability boundary,
  Ninja 1.10 floor, generated sidecars, and reserved paths. Updated the
  design, developer, repository-layout, contents, and roadmap documents and
  added ADR-010 for the durable backend decision.
- [x] (2026-08-12) Ran the full deterministic suite. Formatting, type checking,
  linting, Markdown linting, and Mermaid validation passed; the documentation
  example loader rejected the new YAML fence because it lacked a
  `tested-example` marker. Made the sample a complete manifest, registered its
  stable identifier in the executable-documentation tests, and will rerun the
  complete suite before committing.
- [x] (2026-08-12) Re-ran the complete deterministic suite after the
  executable-example correction. `make check-fmt`, `make typecheck`,
  `make lint`, `make test`, `make markdownlint`, and `make nixie` passed.
  `make test` reported 1,939 passed tests, one skipped test, and passing
  doctests. The canonical command-specific logs use the current branch suffix
  beneath `/tmp`.
- [x] (2026-08-12) Committed the documentation and executable-example coverage
  as `7153538` (`Document serial dependency ordering (#552)`).
- [x] (2026-08-12) Ran `coderabbit review --agent` on the committed milestone.
  It completed successfully with zero actionable findings. The review log uses
  the current branch suffix beneath `/tmp` and ends in `-2.out`.
- [x] (2026-08-12) Refactored the duplicated no-staging assertions in
  `src/ninja_gen/dyndep_tests.rs` into the private
  `assert_edge_produces_no_staging` helper without merging the distinct
  parallel and one-element-serial tests. Committed as `df68f82`. The focused
  dyndep module, `make check-fmt`, `make typecheck`, `make lint`, and
  `make test` passed.
- [x] (2026-08-12) Linearized `write_atomic` in
  `src/runner/process/dyndep_files.rs` by extracting temporary-file creation,
  collision verification, write-and-sync, and rename-race helpers. Added the
  matching-final-sidecar temporary-name-collision regression and committed the
  change as `04e2369`. `make check-fmt`, `make typecheck`, `make lint`, and
  `make test` all passed; the full test suite reported 1,940 passed tests, one
  skipped test, and passing doctests.
- [x] (2026-08-12) Split `generate_bundle` edge processing into private
  `render_edges`, `render_edge`, serial-edge, and display-edge helpers while
  preserving sorted de-duplication, MissingAction construction, staged-gate
  lowering, and emitted text. Committed as `ce47d61`. The focused dyndep tests
  and `make check-fmt`, `make typecheck`, `make lint`, and `make test` passed
  with no generated-output or snapshot changes.
- [x] (2026-08-12) Added the empty-sidecar fast path to
  `materialize_dyndep_files` before capability opening and state-directory
  creation, with a focused test proving it does not create `.netsuke/dyndep`.
  The focused materializer suite passed. The first full lint run exposed that
  the root Whitaker recipe did not pass the existing `dylint.toml` policy to
  Dylint; its documented build-script and ambient-path exclusions therefore
  appeared as false positives. The recipe now supplies `DYLINT_TOML` explicitly
  for the root pass, matching the existing `test_support` pass. `make
  check-fmt`, `make typecheck`, `make lint`, `make test`, `make markdownlint`,
  and `make nixie` all passed afterwards.
- [x] (2026-08-12) Replaced the deterministic temporary-sidecar suffix with a
  per-process, monotonic attempt name and retry `create_new` collisions. Keep
  every name below the final sidecar parent so the capability-scoped rename
  stays atomic, and pass the created path through rename-race cleanup rather
  than regenerating it. Cover stale files, generated-name distinction, and an
  existing matching final sidecar with another temporary file. The focused
  suite passed 7 tests. The first full lint run identified a five-argument
  helper and then a by-value context; grouping and borrowing the context
  resolved both without changing the atomic protocol. `make check-fmt`, `make
  typecheck`, `make lint`, `make test` (1,943 passed, 1 skipped, doctests
  passed), `make markdownlint`, and `make nixie` all passed. CodeRabbit found
  no concerns.
- [x] Committed each green logical change and recorded final evidence here.
- [x] (2026-08-12) Review follow-up: added a real-Ninja regression with two
  serial consumers of one shared dependency and an unrelated branch. The test
  proves the shared output executes once and uses a marker handshake to prove
  that unrelated work progresses concurrently. Added the missing v0.1.0
  migration guidance and removed developer-specific workspace metadata from
  this plan. All deterministic gates passed and CodeRabbit returned no
  findings for this milestone.
- [x] (2026-08-12) Separated effect-free `GeneratedNinja` production from the
  runner's explicit, capability-injected publication command. The command
  handlers now consume the bundle after publication, removing the generated
  main-string copy. Serial rendering uses a dependency view instead of cloning
  `BuildEdge`, and materialization borrows sidecar paths. Added bounded spans,
  outcome counters, and duration histograms at bundle generation, serial
  lowering, and sidecar publication boundaries. The focused dyndep suite and
  runtime Ninja suite passed; all deterministic gates passed (1,944 tests, one
  skipped, and passing doctests), then CodeRabbit reported zero findings.
- [x] (2026-08-12) Verified the latest review findings against the current
  branch. Stale findings were skipped because the absolute path is already
  removed, the shared-work and unrelated-concurrency runtime test already
  exists, and the generation/publication and clone issues are resolved. The
  confirmed minimal fixes cover docs and locales, the render policy assertion,
  reserved-path matrix and prefix boundary, bundle equivalence and shared gate
  predicate/`write!`, pipe rejection, fixture-link and header cleanup, IR
  dependency-order assertions, integration helpers, Ninja-not-found handling,
  and materializer cleanup, bounded retries, directory scans, and localized
  errors. Focused evidence: 21 dyndep tests, one render test, and 29 touched
  integration/runtime tests passed.
- [x] (2026-08-12) Completed the review-fix validation. `make check-fmt`,
  `make typecheck`, `make lint`, `make test`, `make markdownlint`, and `make
  nixie` passed; the full suite reported 1,952 passed tests, one skipped test,
  and passing doctests. CodeRabbit completed with zero findings.
- [x] (2026-08-12) Validated the second review remediation: control-character
  path rejection, dependency-order module split, concurrent runtime-order
  coverage, schema and migration links, and targeted locale corrections are
  implemented. All 52 focused tests passed. `make check-fmt`, `make typecheck`,
  `make lint`, `make test`, `make markdownlint`, and `make nixie` passed; the
  full suite reported 1,970 passed tests, one skipped test, and passing
  doctests. The subsequent independent CodeRabbit review reported zero
  findings.
- [x] (2026-08-12) Closed the remaining serial-dyndep review gaps without
  changing scheduling semantics. Public-CLI tests now prove runner-owned
  sidecar publication, declaration order, and failure short-circuiting at
  `-j 3`; generation telemetry lives at the runner boundary; four bounded
  serial-lowering properties cover staging, order, repetition, content-address
  invariants, and determinism; AST and IR dependency-order types are distinct;
  and existing-sidecar verification is bounded at 16 MiB. All focused suites
  passed. The six full deterministic gates passed with 1,979 tests, one skip,
  and passing doctests, followed by a zero-finding CodeRabbit review.
- [x] (2026-08-14) Re-verified the documentation and locale review findings
  against the current branch. Applied only the still-valid caption, bundle
  signature, migration wording, Finnish state-expression, Polish path-label,
  and developer-guide boundary corrections; the table contents and runtime
  behaviour remain unchanged.
- [x] (2026-08-14) Refactored bounded sidecar verification into named open,
  size-check, bounded-read, and outcome helpers. Added public-CLI coverage for
  serial `generate` publication plus Ninja loading and serial `clean`
  publication before Ninja dispatch. The 9 focused materializer tests and 4
  serial CLI tests passed. `make check-fmt`, `make typecheck`, `make lint`,
  `make test`, `make markdownlint`, and `make nixie` passed; the full suite
  reported 1,981 passed tests, one skipped test, and passing doctests. The
  subsequent CodeRabbit review completed with zero actionable findings.

## Surprises and discoveries

- (2026-08-12) The first serial-lowering property run correctly shrank to two
  repeated dependencies, but exposed a test-oracle error rather than a
  generator defect: the preceding gate constrains the next sidecar-producing
  phony edge, not the gate edge that consumes that sidecar. The property now
  asserts the actual staging relationship. The existing named repeated-
  dependency unit test already pins the shrunk case, so no generated regression
  seed was retained.
- (2026-08-12) The first full second-review test run found that the shared
  shell-quoting BDD fixture still contained a newline-bearing output path. The
  new generator validation correctly rejected the whole graph, preventing two
  otherwise unrelated quoting scenarios from observing generated content. The
  fixture now uses its existing apostrophe path as the edge case; IR-level
  command interpolation coverage remains separate from the generator's stricter
  Ninja path contract.
- (2026-08-11) The prior materializer commit accidentally left surplus blank
  lines at EOF in each changed Fluent catalogue. `git show --check` reports
  them even though the current worktree is clean. Remove only those trailing
  blank lines in a preparatory cleanup before the next full validation run.
- (2026-08-11) The first fresh full-gate run stopped before review: typecheck
  found an unused runtime-test helper, and Clippy found `expect` calls in
  fallible bundle formatting plus three small idiom violations in the
  materializer. `check-fmt`, Markdown linting, and Mermaid validation passed.
  The correction remains within the approved implementation and needs no new
  dependency or architecture.

- (2026-08-10) Re-validated the staged dyndep chain with real Ninja 1.11.1:
  declaration order holds when all sidecars are pre-materialized; a later
  sidecar is revealed only after the preceding gate; failure of an early real
  dependency stops later stages from being scheduled; and unrelated branches
  remain available. The chain requires no generator recipe and no nested Ninja
  process.
- (2026-08-10) Ninja path escaping in build/dyndep documents uses `$` as the
  escape character. Spaces, `$`, `:`, `|`, and similar metacharacters in target
  or dependency paths must use Ninja's dollar escape: a dollar sign followed by
  a space for a literal space, `$$` for a literal dollar, `$:` for colon, and
  `$|` for pipe. Unescaped spaces split a token into multiple paths. The
  generator therefore needs a dedicated Ninja path-escape helper distinct from
  the existing shell-script escaping.

- (2026-08-10) Ninja resolves every path named in a build file — including a
  `dyndep =` value and every path inside the referenced dyndep document —
  relative to Ninja's process working directory, which is the `-C` directory
  when one is supplied. The directory containing the main build file does not
  affect path resolution. Confirmed with Ninja 1.11.1: with the main build file
  in an OS temp directory and `-C` set to the user's project directory, a
  sidecar written beneath `project/.netsuke/dyndep/` is located, loaded, and
  its revealed dependency built correctly. The runner therefore needs no
  architectural change; the plan's existing `.netsuke` navigation already
  matches Ninja's model.
- A dyndep document updates the edge by naming the edge's *outputs*, not the
  dyndep file itself. The first failed probes named the sidecar path in the
  sidecar's `build` statement, which Ninja rejects with
  `not mentioned in its dyndep file`. The accepted form is
  `build <edge-output>: dyndep | <real-dep>`.

- An order-only dependency on a phony gate orders only the gate itself. Ninja
  eagerly schedules all already-visible transitive inputs, so the real recipes
  behind later gates still start concurrently. This invalidates the original
  phony-chain proposal even though the gate commands appear ordered.
- Giving every stage a real command that recursively invokes Ninja provides
  ordering and failure propagation, but it breaks the shared-dependency
  requirement because each child Ninja process owns a separate build memo.
- Dyndep changes the decisive property: the next real dependency is absent
  from Ninja's graph until the preceding gate completes and makes the next
  dyndep file available.
- Static, pre-materialized dyndep files can still be revealed in stages. A
  phony edge may name each existing sidecar as its output and depend on the
  previous gate. No generator recipe or nested process is required.
- Ninja's `rspfile_content` binding cannot conveniently encode the required
  multiline dyndep document. A literal `\n` remains literal and produces an
  invalid file. Netsuke therefore needs to materialize sidecars itself.
- `src/ninja_gen/mod.rs` was 400 lines before the module split, and
  `src/runner/process/mod.rs` is close to that limit. The implementation must
  be modular rather than appended to those files.
- Netsuke already owns the `.netsuke` workspace-state namespace through its
  fetch cache, so `.netsuke/dyndep` does not introduce a second state root.
- (2026-08-12) The initial real-Ninja tests established order and failure but
  did not encode the plan's independently observed shared-work and unrelated
  concurrency behaviour. The review correctly treats both as separate runtime
  contracts, so the follow-up test uses a bounded marker handshake rather than
  a timing-only assertion.
- (2026-08-12) `GeneratedNinja` already had a consuming `into_parts` API, so
  command handlers can move the generated main string only after sidecar
  publication succeeds. An edge-display dependency view similarly removes the
  need to clone a complete `BuildEdge` during serial lowering.

## Decision log

- **Decision:** retain the existing AST, IR, Ninja generation, runner, and
  localization implementation commits as the reviewed functional baseline, then
  repair only their trailing-catalogue-whitespace defect before full gates.
  **Rationale:** the defect does not alter Fluent messages or behaviour, but a
  clean diff is required before the first post-implementation review. **Date:**
  2026-08-11.
- **Decision:** propagate `fmt::Error` through the existing
  `NinjaGenError::Format` conversion instead of asserting that `String`
  formatting cannot fail. **Rationale:** this keeps the generator's public
  error contract intact and satisfies the repository's no-`expect` policy
  without adding an abstraction. **Date:** 2026-08-11.
- **Decision:** keep `generate_bundle` as a read-only query and move all
  sidecar publication to explicit runner command boundaries. **Rationale:**
  generation must be usable without a filesystem effect; accepting a
  capability-scoped directory at the materializer makes the publication
  authority explicit and testable. **Date:** 2026-08-12.
- **Decision:** keep generation telemetry in the runner boundary's
  `src/runner/dyndep_generation_telemetry.rs` and publication telemetry in
  `src/runner/process/dyndep_telemetry.rs`; keep `src/ninja_gen` generation and
  rendering telemetry-free. **Rationale:** the separate wrappers make query
  and command policy explicit while restricting fields to bounded counts and
  outcome categories. **Date:** 2026-08-12.
- **Decision:** reject an existing dyndep sidecar larger than 16 MiB before
  allocating verification storage. **Rationale:** generated sidecars are small
  Ninja fragments, so this conservative ceiling supports large manifests while
  preventing an untrusted existing file from causing unbounded memory use. A
  metadata-sized buffer and one-byte growth probe also bound reads if the file
  changes during verification. **Date:** 2026-08-12.

- **Decision:** use staged Ninja dyndep files rather than an order-only gate
  chain, a pool, or recursive builds. **Rationale:** it is the only evaluated
  design that keeps a single scheduler, prevents later dependencies from
  becoming schedulable through the annotated path, propagates failure, and
  leaves unrelated work unconstrained. **Date:** 2026-08-10.
- **Decision:** give the manifest AST and domain IR distinct
  `DependencyOrder::{Parallel, Serial}` enums, converting explicitly while
  lowering each `BuildEdge`. **Rationale:** the closed AST enum owns YAML and
  Serde policy, while the IR enum remains backend-agnostic and free of syntax
  responsibilities; both avoid inferring scheduling policy from graph shape.
  **Date:** 2026-08-12.
- **Decision:** apply `dependency_order` only to `Target::deps`.
  **Rationale:** actions already use the target shape, while sources and
  order-only dependencies have distinct freshness semantics not covered by the
  issue. **Date:** 2026-08-10.
- **Decision:** keep documentation terminology aligned with the implementation
  boundary: `ast::DependencyOrder` owns manifest serialization, while
  `ir::DependencyOrder` is the serialization-free domain type explicitly
  produced during lowering. **Rationale:** this makes the developer guide and
  design diagram describe the fallible `GeneratedNinja` bundle contract without
  changing the serial dependency contract. **Date:** 2026-08-14.
- **Decision:** preserve ordinary dependency nodes in IR and perform
  Ninja-specific staged lowering in the Ninja generator. **Rationale:** dyndep
  gates are a backend mechanism, not a user graph concept; keeping them out of
  IR preserves cycle diagnostics and other backends' view of the manifest.
  **Date:** 2026-08-10.
- **Decision:** introduce a generated bundle containing the main Ninja text and
  zero or more dyndep sidecars. **Rationale:** string-only generation cannot
  represent the complete executable artefact. A bundle makes omission of
  required sidecars difficult. **Date:** 2026-08-10.
- **Decision:** store immutable, content-addressed sidecars beneath
  `.netsuke/dyndep`, and gates beneath `.netsuke/serial`. **Rationale:**
  deterministic names make generation reproducible, reuse safe, and stale cache
  entries harmless. **Date:** 2026-08-10.
- **Decision:** generate no dyndep chain for zero- or one-element serial lists.
  **Rationale:** no relative ordering exists to enforce, so ordinary lowering
  is equivalent and avoids unnecessary generated state. **Date:** 2026-08-10.
- **Decision:** list all generated gates, in order, as implicit dependencies of
  the annotated edge. **Rationale:** every real dependency must continue to
  participate in dirty checking; relying only on the final gate obscures that
  invariant. **Date:** 2026-08-10.
- **Decision:** document independent reachability as a semantic boundary rather
  than globally constraining shared nodes. **Rationale:** global delay would
  serialize unrelated branches and violate the scoped-behaviour acceptance
  criterion. Stronger semantics require a Netsuke scheduler and explicit
  approval. **Date:** 2026-08-10.
- **Decision:** record the architecture in a new ADR before calling the feature
  complete. **Rationale:** the Ninja version floor, generated sidecars, state
  namespace, and public generator contract are durable choices that are costly
  to reverse. **Date:** 2026-08-10.

## Outcomes and retrospective

The implementation has delivered the planned closed schema enum, backend-only
staged dyndep lowering, complete generated bundle, capability-scoped atomic
sidecar materialization, and the user-facing and maintainer documentation. The
documentation makes the intentionally limited path-scoped execution guarantee
explicit rather than implying global serialization. The complete deterministic
suite passed after the executable documentation sample was registered, and
CodeRabbit found no actionable concerns on `7153538`. The feature is ready for
the draft pull request and normal reviewer evaluation.

## Context and orientation

The relevant pipeline is deliberately small:

```plaintext
Netsukefile YAML
    -> src/ast.rs Target
    -> src/ir/from_manifest.rs process_targets
    -> src/ir/graph.rs BuildEdge
    -> src/ninja_gen/mod.rs generated Ninja artefact
    -> src/runner/* materialization and Ninja invocation
```

In this plan, a *real dependency* is the action or target named by a manifest
`deps` entry. A *gate* is a synthetic phony Ninja output representing one
position in a serial list. A *dyndep sidecar* is a small Ninja-syntax document
that adds one real dependency to one gate after Ninja has loaded the main build
file. A *bundle* is the main build-file text plus every sidecar required to
execute it.

The syntax and graph-loading constraints used below follow the official
[Ninja dyndep reference](https://ninja-build.org/manual.html#ref_dyndep). In
particular, the main edge names its dyndep file as an input and each sidecar
contains the version header plus a one-to-one update for that edge.

`src/ast.rs` defines `Target`, which is shared by ordinary targets and actions.
Its `deps` vector is already ordered by YAML declaration. Add a serde-backed
enum here rather than representing ordering as a string or Boolean.

`src/ir/from_manifest.rs::process_targets` currently transfers `Target::deps` to
`BuildEdge::implicit_deps`. Keep the vector unchanged and copy the new enum to
the edge. Existing cycle detection continues to inspect the real dependency
graph rather than generated gates.

`src/ir/graph.rs` defines `BuildEdge`. The field belongs here because
generation must know whether the edge's implicit dependencies are ordered. Many
tests and Rustdoc examples construct `BuildEdge` directly; update every literal
mechanically and default it to parallel.

`src/ninja_gen/mod.rs` currently renders a graph to one string. Extract dyndep
identifier, sidecar, and staged-edge construction into
`src/ninja_gen/dyndep.rs`. Keep the top-level module responsible for ordinary
rendering and selecting the staged representation.

`src/runner/mod.rs` and `src/runner/process/mod.rs` connect generation to
`build`, `clean`, and `generate`. Add sidecar materialization in a new focused
module such as `src/runner/process/dyndep_files.rs`; do not let a caller invoke
Ninja with a serial main file until its bundle is materialized.

The principal existing tests are:

- `tests/ast_tests/parsing.rs` and `tests/ast_tests/actions.rs` for manifest
  syntax;
- `tests/ir_from_manifest_tests.rs` for dependency lowering;
- `tests/ninja_snapshot_tests.rs` for stable generated Ninja;
- `tests/ninja_gen_integration_tests.rs` for real Ninja execution;
- `tests/features/ninja.feature` and `tests/bdd/steps/ninja.rs` for externally
  described generation behaviour; and
- `test_support/src/ninja_gen.rs` for shared generator fixtures.

The current real-Ninja no-op test intentionally runs Ninja once to populate
`.ninja_log` before asserting that a second invocation is a no-op. Preserve
that pattern in serial freshness tests so the result reflects Ninja's normal
incremental state rather than a cold build.

## Proposed generated form

For a target `all` whose serial dependencies are `check-fmt`, `lint`, and
`test`, generate deterministic paths represented schematically below. The
actual identifiers use stable digests and escaped Ninja paths.

```ninja
ninja_required_version = 1.10

build .netsuke/dyndep/<first-digest>.dd: phony
build .netsuke/serial/<parent-digest>/000: phony || .netsuke/dyndep/<first-digest>.dd
  dyndep = .netsuke/dyndep/<first-digest>.dd

build .netsuke/dyndep/<second-digest>.dd: phony .netsuke/serial/<parent-digest>/000
build .netsuke/serial/<parent-digest>/001: phony || .netsuke/dyndep/<second-digest>.dd
  dyndep = .netsuke/dyndep/<second-digest>.dd

build .netsuke/dyndep/<third-digest>.dd: phony .netsuke/serial/<parent-digest>/001
build .netsuke/serial/<parent-digest>/002: phony || .netsuke/dyndep/<third-digest>.dd
  dyndep = .netsuke/dyndep/<third-digest>.dd

build all: <action-rule> | .netsuke/serial/<parent-digest>/000 $
    .netsuke/serial/<parent-digest>/001 .netsuke/serial/<parent-digest>/002
```

The first sidecar contains:

```ninja
ninja_dyndep_version = 1
build .netsuke/serial/<parent-digest>/000: dyndep | check-fmt
```

The later sidecars have the same shape for `lint` and `test`. Ninja can load
the first sidecar immediately and therefore schedule `check-fmt`. The second
sidecar's phony-producing edge depends on the first gate, so `lint` remains
unknown through this path until `check-fmt` succeeds. If `check-fmt` fails, the
first gate never completes, the second sidecar never becomes available, and
later dependencies are not scheduled through the serial list.

Each gate is a phony alias of exactly one real dependency. Repeated or diamond
dependencies still name the same real Ninja node, so the single Ninja scheduler
executes that node at most once.

## Plan of work

### Stage 1: Establish red behavioural contracts

Add parser tests for targets and actions covering omitted ordering, explicit
`parallel`, explicit `serial`, and rejection of an unknown value such as
`sequential`. Add IR tests showing that dependency order and the original
`implicit_deps` sequence survive lowering.

Add generator tests that describe a complete bundle rather than only the main
string. The first red assertion should require:

- `ninja_required_version = 1.10` only for a multi-dependency serial edge;
- one deterministic gate and sidecar per dependency;
- each later sidecar-producing edge to depend explicitly on the previous gate;
- one dyndep statement per gate with the matching real dependency;
- every gate to appear on the annotated edge in declaration order; and
- ordinary parallel snapshots to remain unchanged.

Add a Gherkin scenario and fixture at `tests/data/dependency_order_serial.yml`.
The scenario should compile a target and an action to IR, generate a bundle,
and inspect the ordered dependency names revealed by its sidecars. The feature
text should describe user behaviour, not implementation internals beyond the
fact that valid staged dyndep output is generated.

Capture the failing focused commands and their failure messages in the
`Progress` section. Then implement enough of stages 2–4 to make the tests green
before committing.

### Stage 2: Add the AST and IR contract

In `src/ast.rs`, add the closed enum and target field. The intended public
shape is:

```rust
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DependencyOrder {
    #[default]
    Parallel,
    Serial,
}

pub struct Target {
    // Existing fields remain in their current order.
    #[serde(default)]
    pub dependency_order: DependencyOrder,
}
```

Adjust derives to match the surrounding AST types and add Rustdoc examples
showing the default and serial forms. Do not add this field to `Rule`.

In `src/ir/graph.rs`, define a domain-only `DependencyOrder` with no Serde
derives or attributes and add it to `BuildEdge`. In `src/ir/from_manifest.rs`,
convert the parsed AST value explicitly while leaving `implicit_deps` in source
order. Update every direct `BuildEdge` construction, fixture, and doctest to use
the IR type explicitly or use a shared test constructor where one already
exists. Do not introduce a new general-purpose builder solely to conceal
updates.

Run the parser and IR tests. Also run existing cycle tests to prove the new
field does not change graph validation.

### Stage 3: Generate a complete Ninja bundle

Before extracting any helper, repeat the repository sweep for equivalent
bundle, digest, path-escape, and sidecar abstractions. Record the ownership and
reuse boundary in `docs/developers-guide.md`: the new types belong to Ninja
generation and may be consumed by runner/output adapters, but must not become
generic manifest or filesystem abstractions.

Add types along these lines, refining names to fit existing conventions:

```rust
pub struct GeneratedNinja {
    build_file: String,
    dyndep_files: Vec<GeneratedDyndep>,
}

pub struct GeneratedDyndep {
    relative_path: Utf8PathBuf,
    content: String,
}

pub fn generate_bundle(graph: &BuildGraph) -> Result<GeneratedNinja, NinjaGenError>;
```

Expose read-only accessors or consuming methods needed by the CLI. Keep fields
private if callers do not need to construct invalid bundles. Document that all
paths are relative to the effective Ninja working directory.

Keep `generate` and `generate_into` source-compatible for graphs without serial
ordering. If called with a multi-dependency serial edge, return a localized
`NinjaGenError::DyndepFilesRequired` directing the caller to `generate_bundle`;
never return main text that cannot run alone. Perform this check before writing
any bytes so `generate_into` cannot leave partial output in its caller's writer.

Implement the staged lowering in `src/ninja_gen/dyndep.rs`:

1. Iterate serial `implicit_deps` in their stored order.
2. Derive a stable parent identity from the annotated edge's canonical output
   identity and an explicit schema/version tag.
3. Derive a gate path from that identity and a zero-padded position.
4. Render the sidecar content using the generator's existing Ninja path
   escaping.
5. Derive the `.dd` filename from a cryptographic digest of the complete
   sidecar bytes and a format-version tag.
6. Emit a phony sidecar edge. Starting with the second stage, make it explicitly
   depend on the prior gate.
7. Emit a phony gate edge with the sidecar as an order-only input and its
   `dyndep` binding.
8. Replace the annotated edge's direct implicit dependencies with all gate
   outputs in the same order.

Deduplicate identical sidecar content in the bundle by relative path, but do
not collapse gate positions. This preserves the declaration sequence while
allowing the real Ninja node to remain shared.

Return a localized collision error when a user output occupies the reserved
`.netsuke/serial` or `.netsuke/dyndep` namespace. Add unit coverage for the
error and documentation for the reservation.

Do not emit the Ninja version binding or any generated files for parallel,
empty serial, or one-element serial lists. Add tests for all three cases.

### Stage 4: Materialize dyndep files atomically

Extend the internal generated-content wrapper used by
`src/runner/process/mod.rs` so it carries `GeneratedNinja`, while preserving
the current main-text access required by stdout and JSON output.

Create `src/runner/process/dyndep_files.rs`. Its single responsibility is to
materialize the bundle's sidecars under the effective Ninja working directory.
Its algorithm should be:

1. Open the effective working directory through the existing capability-based
   filesystem seam. Honour CLI `-C`; otherwise use the current directory.
2. Create `.netsuke/dyndep` if it is absent.
3. If a final content-addressed file exists, read it and verify its content.
   Matching content is success; mismatched content is a corruption error.
4. Otherwise create a unique same-directory temporary file with `create_new`,
   write all bytes, flush them, and rename it atomically to the final path.
5. If another process wins the rename race, verify the winning file and treat
   matching content as success.
6. Clean up only the temporary file owned by this attempt. Never truncate or
   replace an existing final sidecar in place.

Use a narrow injected filesystem seam only if required for deterministic unit
tests; first reuse the runner's existing capability abstractions. Test initial
creation, idempotent reuse, corrupt-content rejection, nested-directory
creation, and the competing-writer outcome without mutating process-wide
environment variables.

Route all executable and export paths through bundle materialization:

- `netsuke build` materializes before writing or invoking the main Ninja file;
- `netsuke clean` does the same because loading a serial build file requires
  its dyndep inputs even when cleaning;
- `netsuke generate --output <path>` materializes relative to the effective
  Ninja working directory and writes the main file;
- `netsuke generate --stdout` and JSON output also materialize sidecars, then
  return only the main file text in their existing output field.

Document the side effect of stdout/JSON generation. If analysis shows that an
output path outside the working directory makes relative sidecars ambiguous,
retain the working-directory rule rather than inferring a new base from the
output filename.

### Stage 5: Prove runtime semantics with real Ninja

Create a focused integration-test module, splitting existing files if needed to
remain below 400 lines. Use actual Ninja processes and filesystem markers, not
assertions over textual edge order alone.

Add these cases:

- **Declaration order:** dependency one writes marker `one`; dependency two
  first requires `one`, then writes `two`; dependency three requires `two`. The
  aggregate succeeds only if recipes start in order.
- **Shared dependency:** the first and second serial entries both depend on a
  real `common` node that appends one log entry. Assert exactly one entry for
  `common` in one Ninja invocation.
- **Literal repeated entry:** use the same dependency twice in one serial list
  and prove its recipe runs once while both gate stages complete.
- **Failure short-circuit:** make the first dependency fail deliberately and
  assert no marker or log line exists for later dependencies.
- **Default parallel behaviour:** with `dependency_order` omitted, have two
  dependencies create start markers and wait with a bounded timeout for the
  peer marker. They succeed only if Ninja may run them concurrently.
- **Scoped serialization:** make the first serial dependency and an unrelated
  requested branch meet at a bounded barrier. Assert both start concurrently,
  while the second serial dependency begins only after the first finishes.
- **Incremental freshness:** perform a cold build, then a no-op build. Mutate
  each real serial dependency in turn and assert the aggregate rebuilds. Finish
  with another true no-op invocation after `.ninja_log` has been populated.
- **Path escaping:** use dependency and target paths containing spaces and a
  Ninja metacharacter, and prove both main and dyndep files load correctly.

Avoid timing-only assertions. Markers and bounded handshakes should establish
happens-before and concurrency. A timeout is only a deadlock guard and should
be generous enough for loaded CI workers.

The scoped test intentionally uses an unrelated branch. Do not add a test that
claims a later dependency independently exposed by another branch will remain
hidden; that is outside the stated compatibility boundary.

### Stage 6: Document the feature and its architecture

Create `docs/adr-010-use-ninja-dyndep-for-serial-dependency-ordering.md` using
the repository ADR format. Verify the next ADR number immediately before
creation. The ADR must include a Y-statement and record:

- the rejected order-only gate, pool, and recursive-build alternatives;
- the Ninja 1.10 version floor for serial builds;
- the generated bundle and `.netsuke` namespace;
- the single-scheduler shared-dependency property;
- the independent-reachability boundary; and
- consequences for generated-output consumers and cache cleanup.

Mark the ADR `Proposed` while implementation is in progress and `Accepted` only
after behaviour and gates pass. Add it to `docs/contents.md`.

Update:

- `docs/users-guide.md` with action and target syntax, defaulting, ordered
  execution, failure, shared work, scope, Ninja version, generated state, and
  the independent-reachability boundary;
- `docs/netsuke-design.md` with the AST/IR policy, generated bundle, staged
  dyndep graph, and why gates remain backend-only;
- `docs/developers-guide.md` with bundle ownership, materialization invariants,
  reserved paths, and focused-test guidance;
- `docs/repository-layout.md` with the new generator/runner submodules and
  `.netsuke/dyndep` state;
- `docs/roadmap.md` with issue #552 status, marking it complete only after all
  evidence is present; and
- relevant Rustdoc on `DependencyOrder`, bundle APIs, errors, and the
  materializer.

Use en-GB-oxendict prose, 80-column paragraphs, attributed code fences, and the
documentation style guide. Run `make fmt` after documentation edits and inspect
the diff so unrelated mechanical reflow is not included.

### Stage 7: Refactor, validate, and commit

Once the feature is green, review the changed code and its neighbours for
duplication, long functions, excessive parameters, complex conditionals, and
feature envy. Any non-essential refactor belongs in a separate subsequent
commit and must pass the same gates. Do not broaden the feature commit merely
to tidy unrelated code.

Use small green commits. A suitable sequence is:

1. add the AST/IR contract and its tests;
2. add Ninja bundle generation, materialization, and semantic regressions;
3. document the syntax and architectural decision; and
4. apply a separate focused refactor only if post-commit review justifies one.

Before every commit, run the relevant focused tests and gates. Before declaring
the work complete, run all project gates sequentially through the repository's
gate-running workflow:

```bash
make check-fmt
make typecheck
make lint
make test
make markdownlint
make nixie
```

Capture each command with `tee` to a branch-specific file under `/tmp`, inspect
the complete log on failure, and record the final result and log path in
`Progress`. Do not substitute a narrower command for a named project gate.

## Concrete implementation steps

All commands run from the repository root:

```plaintext
<repository root>
```

First confirm branch and cleanliness:

```bash
git branch --show-current
git status --short
```

Locate all construction and generation seams before editing:

```bash
rg -n 'Target \{|BuildEdge \{|generate_into|generate\(' src tests test_support
rg -n 'NinjaContent|handle_build|handle_clean|handle_generate' src tests
rg -n '\.netsuke|cap_std|fs_utf8|rename' src tests
```

Run the existing focused baseline tests before adding red cases:

```bash
cargo nextest run --test ast_tests --test ir_from_manifest_tests
cargo nextest run --test ninja_snapshot_tests --test ninja_gen_integration_tests
cargo nextest run --test bdd -- ninja
```

Use the exact test-binary and filter names discovered by `cargo nextest list`
if the last BDD filter is not accepted. Record any pre-existing failure before
editing and do not attribute it to this work.

After adding each red test group, run only that group, record the expected
failure, implement the smallest corresponding production slice, and rerun until
green. Example commands, to be adjusted to the final test names, are:

```bash
cargo nextest run --test ast_tests dependency_order
cargo nextest run --test ir_from_manifest_tests dependency_order
cargo nextest run --test ninja_snapshot_tests serial_dependency
cargo nextest run --test ninja_gen_integration_tests serial_dependency
cargo nextest run --test bdd serial_dependencies
```

After changes to Rust or Markdown, format once and inspect the resulting diff:

```bash
make fmt
git status --short
git diff --check
git diff --stat
```

Then run the complete sequential gates listed in stage 7. Use the
commit-message skill to prepare each commit message in imperative mood with a
wrapped body. Do not push or open a pull request unless separately requested.

## Validation and acceptance

Acceptance is evidence-based. The following must all be true:

- A target and an action both accept `dependency_order: serial`.
- `parallel` is accepted explicitly, omission defaults to it, and any unknown
  enum value produces a localized manifest error.
- Serial `deps` retain declaration order from YAML through IR and every staged
  dyndep sidecar.
- A real-Ninja test proves ordered start, not merely ordered gate completion.
- A real-Ninja test proves later dependencies do not start after an earlier
  failure.
- Shared and repeated real dependencies execute once in one top-level Ninja
  invocation.
- A real-Ninja barrier test proves omitted ordering remains parallel.
- A real-Ninja barrier test proves an unrelated graph branch remains parallel
  with the active serial stage.
- Sources and order-only dependencies remain governed by their existing
  semantics.
- Rebuild and no-op tests prove every serial dependency still participates in
  aggregate freshness.
- Serial generation produces a complete bundle, uses valid dyndep syntax, and
  declares Ninja 1.10 as the required version.
- Parallel generation produces no dyndep files and preserves existing snapshot
  output.
- Sidecar writes are deterministic, idempotent, atomic, capability-oriented,
  and covered for corruption and race outcomes.
- CLI build, clean, file output, stdout output, and JSON output never expose or
  execute an unmaterialized serial bundle.
- User and internal documentation state the exact guarantees and the
  independent-reachability boundary.
- No source file exceeds 400 lines, no lint is suppressed for convenience, and
  every repository gate passes.

The behavioural feature should contain a scenario equivalent to:

```gherkin
Scenario: Serial dependencies preserve their declaration order
  Given a manifest with a serial target depending on check-fmt, lint, and test
  When the manifest is compiled and its Ninja bundle is generated
  Then the target dependency order is serial
  And the dyndep stages reveal check-fmt, lint, and test in that order
```

The runtime tests, rather than this textual scenario alone, are authoritative
for concurrency, failure, and shared-execution semantics.

## Idempotence and recovery

Generation is deterministic: identical graph input produces identical main
text, sidecar paths, and sidecar bytes. Repeating materialization reuses an
existing sidecar only after verifying its content. Content-addressed filenames
mean abandoned older files do not affect the new graph.

If Netsuke is interrupted before rename, only its uniquely named temporary file
may remain. A later run may ignore or remove that temporary file and safely
retry. Never use a broad recursive deletion to recover. If a final digest path
contains mismatched bytes, report corruption with the exact relative path and
require the user to remove that single cache file before retrying.

`netsuke clean` cleans build outputs through Ninja but may leave immutable
dyndep cache entries. Document manual recovery as removal of the narrow
`.netsuke/dyndep` directory only; never suggest deleting the entire workspace or
`.netsuke` root.

If an implementation experiment shows that the dyndep sequence does not meet
one of the validated invariants, preserve the failing fixture, update
`Surprises & discoveries`, revert only the uncommitted experiment, and return
to the last green commit. Do not compensate with a pool or nested invocation.

## Artefacts and notes

During implementation, retain concise evidence in this document:

- the first failing assertion for each red test group;
- one representative generated main-file fragment and sidecar;
- the observed marker/log order from the real-Ninja ordering test;
- proof that the shared dependency log contains one entry;
- proof that failure leaves later markers absent;
- proof that the unrelated branch crosses the concurrency barrier; and
- final gate commands, log paths, and commit identifiers.

Do not paste complete build logs or broad snapshots into the plan. Store logs
under `/tmp` and summarize the decisive lines here.

## Interfaces and dependencies

The intended interfaces at completion are:

```rust
pub enum DependencyOrder {
    Parallel,
    Serial,
}

pub struct BuildEdge {
    // Existing fields.
    pub dependency_order: DependencyOrder,
}

pub struct GeneratedNinja { /* private fields */ }

pub struct GeneratedDyndep { /* private fields */ }

pub fn generate_bundle(graph: &BuildGraph) -> Result<GeneratedNinja, NinjaGenError>;
```

`GeneratedNinja` must provide the main build text and a read-only or consuming
view of its sidecars. `GeneratedDyndep` must expose only a relative UTF-8 path
and immutable content. The runner materializer consumes those values but does
not decide graph structure or naming.

`NinjaGenError` gains localized variants for requesting string-only output from
a serial graph and for reserved-output collisions. The materializer gains a
typed or contextual error for directory creation, temporary writes, rename
races, and digest-path corruption. Preserve domain errors within libraries and
convert to `eyre` only at the application boundary, following existing runner
conventions.

No external dependency is planned. Reuse the workspace's current hashing, UTF-8
path, localization, error, and capability-filesystem facilities after
confirming their exact APIs in `Cargo.toml` and existing call sites.

## Revision note

2026-08-10: Initial draft. It replaces the disproven order-only phony-gate
proposal with a staged dyndep bundle, adds atomic sidecar materialization, and
records the independent-reachability limit that must be approved with the
implementation approach.

2026-08-11: Updated the live status after reconciling the committed
implementation with the plan. Added the documentation-preparation evidence and
the narrow trailing-catalogue-whitespace cleanup required before the first full
validation and review. This does not change the remaining implementation scope.

2026-08-11: Completed the user and maintainer documentation milestone after
review feedback identified that the implementation-only plan was insufficient
for issue #552 acceptance. ADR-010 records the staged-dyndep decision and the
user guide now states the syntax, guarantees, limitations, version floor, and
generated-state behaviour. Final gates and independent review remain before
completion.

2026-08-12: The first full documentation gate run exposed the repository's
executable-fence contract. The serial-syntax sample is now a valid standalone
manifest with an explicit marker and an entry in the documentation-example
registry, so its syntax cannot drift without the normal test suite detecting
it.

2026-08-12: The full suite passed after the executable-example correction, and
the independent CodeRabbit review of `7153538` returned no actionable concerns.

2026-08-14: Re-verified the remaining documentation and locale findings against
the current implementation. Updated only the six requested documentation
surfaces, including the `src/ninja_gen/mod.rs` path and AST-to-IR conversion
description; no Rust or test files were changed.
This completes the implementation plan; the remaining work is only to refresh
the existing draft pull request with the completed documentation milestone.

2026-08-14: Reduced bounded sidecar verification complexity without changing
its limit or concurrent-growth detection. Public-CLI tests now cover serial
sidecar publication for `generate` and `clean`, including loading generated
output with Ninja. Focused and full deterministic validation passed; the
post-milestone independent review reported zero actionable findings.

2026-08-12: Applied the requested test-only assertion-helper extraction after
plan completion. It changes neither production code nor feature semantics and
retains each separately named zero-staging behaviour test.

2026-08-12: Refactored the atomic dyndep sidecar writer without changing its
protocol. The temporary-name collision path still accepts only matching final
content, while corruption, a missing final sidecar, write failures, and rename
failures retain their existing localization and error behaviour.

2026-08-12: Reduced `generate_bundle` complexity by separating stable edge
selection from individual-edge rendering. The new helpers retain the exact
serial staging and ordinary display paths, so generated Ninja text, sidecar
content, and MissingAction errors remain unchanged.

2026-08-12: Added an empty-sidecar materialization fast path. The first full
gate run exposed pre-existing ambient filesystem findings because the root
Whitaker recipe omitted its `DYLINT_TOML` input. Passing the existing policy to
that invocation restores its intentionally narrow exclusions without weakening
the capability lint; the full deterministic suite passed afterwards.

2026-08-12: Temporary sidecar names are private to the runner materializer and
must never be reused by callers. A process identifier and monotonic sequence
keep concurrent write attempts distinct; `create_new` remains the final
authority and retries a stale collision. The helper may be used only to create
or inspect sidecars relative to the final dyndep path, preserving same-directory
atomic rename semantics.

2026-08-12: `RenameFailureContext` is a private, single-use grouping for the
exact temporary path, final path, and expected content after one attempted
rename. Only `rename_temp_file` constructs it and only `handle_rename_failure`
consumes it; it must not become a runner-wide filesystem abstraction.
The first lint pass rejected a by-value context parameter, so the failure
handler borrows the private context instead; this does not alter the cleanup
path or error ownership.

2026-08-12: PR review follow-up reopens the plan for two missing observable
contracts and a query-command boundary repair. The new real-Ninja regression
proves single execution for shared serial work and concurrent progress for an
unrelated branch. Bundle generation remains effect-free; runner command
boundaries will open and inject the filesystem capability for sidecar
publication. The remaining revision adds bounded outcome and duration
telemetry and removes unnecessary ownership copies.

2026-08-12: Completed the review remediation in two green commits. Real Ninja
now proves execute-once shared work and unrelated-branch concurrency; the
generator returns an effect-free bundle that command-boundary publication
materializes through an injected directory capability; and bounded telemetry
covers bundle generation, serial lowering, and sidecar materialization. The
final deterministic gate run passed all six checks (1,944 tests, one skipped,
and passing doctests), and CodeRabbit reported zero findings before publication.

2026-08-12: Verified the documentation review findings against the current
implementation. The plan contains no machine-specific absolute path, so no
path replacement was needed; the test-results sentence was corrected, and the
ADR, design sketch, and user's guide now align with the `BuildEdge` policy and
staged serial-ordering contract.

2026-08-12: A second review pass verified every reported schema, link,
punctuation, locale, control-character, and test-structure issue against the
current branch. The minimal remediation adds the missing manifest-schema field,
corrects the requested catalogue wording, rejects Ninja path controls across
all edge fields and default targets, moves dependency-order lowering tests into
a dedicated module, and exercises serial runtime order with concurrent jobs and
observable marker preconditions. No finding in this pass was stale.

2026-08-12: Separated the manifest syntax enum from the domain IR policy. The
Serde-enabled AST type still owns the lowercase YAML spelling and parallel
default; `from_manifest` now converts both variants explicitly into a distinct,
serialization-free IR enum. Focused AST and lowering tests retain omission and
variant coverage, and a compile-fail Rustdoc check guards the IR boundary.

2026-08-12: Bounded existing-sidecar verification at 16 MiB. Publication now
checks metadata before allocation, reads through a metadata-sized limit, and
uses a one-byte probe so concurrent growth cannot make the read unbounded. An
oversized sidecar fails through a dedicated localized category present in all
35 catalogues; the focused materialization suite passes all nine tests.

2026-08-12: Restored runner-boundary proof with real `netsuke -j 3 build`
processes, while retaining the direct Ninja tests as focused lowering evidence.
Moved all generation timing, spans, registration, counters, and histograms out
of `src/ninja_gen` and into the runner-owned `generate_ninja` boundary. The
generator is again a pure, fallible graph-to-bundle transformation and the
serial contract remains one scheduler with shared-work reuse, ordered failure
short-circuiting, and concurrency for unrelated branches.

2026-08-12: Clarified the telemetry ownership decision: generation telemetry is
runner-owned by `src/runner/dyndep_generation_telemetry.rs`, publication
telemetry is owned by `src/runner/process/dyndep_telemetry.rs`, and Ninja
generation/rendering remains telemetry-free.
