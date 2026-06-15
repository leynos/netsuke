# Regression coverage for conditional action dependency manifests (3.14.5)

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

## Purpose / big picture

Netsuke already implements manifest-time conditional planning: actions and
targets may carry `foreach` (fan-out) and `when` (filter) keys that are
evaluated before the typed Abstract Syntax Tree (AST) is built (roadmap 3.14.1
and 3.14.2); `command_available(name, **kwargs)` is a non-throwing probe that
returns `false` for absent commands instead of raising (roadmap 3.14.4); and
target/action `deps` lower into a separate *implicit dependency* class in the
intermediate representation (IR) and into Ninja's `|` implicit-dependency
syntax (roadmap 3.14.3).

The behaviour works. What is missing is a deliberate, durable *regression net*
that pins these behaviours together as a single coherent contract: "a manifest
can select exactly one of two complementary actions based on tool availability,
the selection happens without running any shell command, and the selected
action's declared dependencies survive lowering into the IR and the generated
Ninja file." Roadmap item 3.14.5 asks for precisely this coverage.

After this change a contributor can run `make test` and see new tests that
fail if any of the following regress:

1. Action-level `when` filtering or action-level `foreach` fan-out.
2. Complementary `command_available(...)` / `not command_available(...)`
   branches (the motivating real-world case is "use `cargo nextest` when it is
   installed, otherwise fall back to `cargo test`") selecting *exactly one*
   action.
3. The absent-command fallback path being chosen *without invoking the
   `shell()` helper* — that is, conditional selection depends only on the
   executable-discovery boundary, never on the command-execution boundary.
4. `deps` lowering into IR `implicit_deps` and into Ninja `build` statements,
   including for conditionally-selected actions.

The deliverable is test code plus small deterministic fixtures and
documentation. No production behaviour changes.

## Constraints

Hard invariants that must hold throughout implementation. Violation requires
escalation, not workarounds.

- This is a **test-and-documentation-only** change. Do not modify production
  semantics in `src/manifest/`, `src/ir/`, `src/ninja_gen.rs`, or
  `src/stdlib/`. The single permitted exception is adding a narrowly-scoped,
  test-only seam (for example a `pub(crate)` constructor or `#[cfg(test)]`
  helper) *if and only if* an existing public or `pub(crate)` surface cannot
  drive a required scenario; any such seam must be recorded in the Decision Log
  and must not change runtime behaviour.
- Do not weaken or delete existing tests. New tests are additive. Existing
  passing tests must continue to pass unchanged.
- Preserve the existing implicit `phony: true` behaviour for actions; tests
  must observe it, not alter it.
- All commit gateways must pass before each CodeRabbit review and before each
  milestone is declared complete: `make check-fmt`, `make typecheck`,
  `make lint`, and `make test`.
- British English with Oxford spelling in all prose and doc comments
  (per the documentation style guide and `en-gb-oxendict`).
- Determinism: no new test may depend on the host's real `PATH` contents.
  "Present" cases must inject a temporary directory containing a fake
  executable via the public `StdlibConfig::with_path_override(...)` seam.
  "Absent" cases must combine **all three** guards together: an empty
  `path_override`, a guaranteed-absent command name (for example a
  UUID-suffixed name), *and* `cwd_mode="never"`. An empty `path_override`
  alone is **not** sufficient: `parse_path_entries` maps an empty PATH
  component to the current directory and the default `CwdMode::Auto` scans the
  cwd (`src/stdlib/which/env.rs:91`, `src/stdlib/which/options.rs:11`), so
  without `cwd_mode="never"` the temporary workspace's own contents could
  shadow the lookup.

## Tolerances (exception triggers)

- Scope: if production (non-test) source files require changes beyond a single
  additive test-only seam, stop and escalate. If the total new/changed line
  count exceeds roughly 900 lines net, stop and reassess granularity.
- Interface: if any existing public API signature must change to make a
  scenario testable, stop and escalate.
- Dependencies: adding `googletest` and `pretty_assertions` as
  `[dev-dependencies]` is pre-authorised by the task brief (see Decision Log).
  Use of `googletest` is **confined to the two in-crate white-box test files**
  and is **gated on a Stage A interop spike** (`#[gtest]`+`#[rstest]` under the
  pinned `rstest` 0.18.0); if the spike fails, fall back to bare
  `verify_that!(...)?` returning `googletest::Result<()>` without `#[gtest]`,
  or to `anyhow::ensure!`. `tests/` integration and snapshot files stay on the
  existing `ensure!`+`insta` idiom. Any *other* new dependency triggers
  escalation.
- Iterations: if a new test cannot be made to pass after 3 focused attempts
  and the production behaviour appears correct, stop and escalate with the
  evidence (the test may be encoding a wrong expectation).
- Sabotage check: if a newly-added test still passes after the targeted
  production line it guards is deliberately broken (see "Validation"), the test
  is vacuous; stop and redesign it before proceeding.
- Ambiguity: if the "exactly one action" or "without invoking `shell()`"
  requirement turns out to be satisfiable in materially different ways, present
  the options with trade-offs.

## Risks

- Risk: Tests of `command_available` accidentally depend on the host PATH and
  become flaky in CI.
  Severity: high. Likelihood: medium.
  Mitigation: drive the real `which` resolver through the **public**
  `StdlibConfig::with_path_override(...)` seam (threaded into the resolver by
  `register_with_config`, `src/stdlib/register.rs:104`) for present cases, and
  the three-guard absent recipe (empty `path_override`, a guaranteed-absent
  name, and `cwd_mode="never"`) for absent cases. Precedent:
  `tests/which_diagnostic_snapshot_tests.rs:30` already uses
  `StdlibConfig::new(..).with_path_override(OsString::new())`. Note: the
  private `mod which` makes `WhichConfig` unreachable from tests; do **not**
  attempt to use it.

- Risk: The `which` resolver cache is mistaken for process-global state and
  someone "fixes" non-existent cross-test contamination with `fresh=true` or
  serialisation.
  Severity: low. Likelihood: medium.
  Mitigation: the `WhichResolver` (with its `Arc<Mutex<LruCache>>`) is
  constructed fresh per `minijinja::Environment` (per `register`/`register_with_config`
  call); there is no static/`OnceLock`/`thread_local` resolver. Each test
  builds its own environment, so the cache is env-scoped and cannot leak across
  tests. The cache key is also partitioned by `path_override`/`cwd`. The new
  tests therefore need no `#[serial]` and may run in parallel.

- Risk: `googletest`/`pretty_assertions` clash with the codebase's existing
  uniform `anyhow::ensure!` style and create inconsistency.
  Severity: low. Likelihood: high.
  Mitigation: confine the new assertion crates to the new 3.14.5 test files;
  keep using `ensure!` where it already reads well; document the convention in
  `docs/developers-guide.md` so future tests are consistent.

- Risk: New Ninja snapshots are environment-sensitive (path separators,
  ordering) and churn.
  Severity: medium. Likelihood: low.
  Mitigation: reuse the established `insta` settings and
  `tests/snapshots/ninja/` location; keep fixtures POSIX-path only; rely on the
  IR's deterministic ordering (outputs sorted) already exercised by existing
  snapshots.

- Risk: Overlap with property/bounded-verification roadmap items (4.2.x Kani,
  4.3.2 Proptest for expansion invariants) leads to scope creep or duplicated
  intent.
  Severity: medium. Likelihood: medium.
  Mitigation: 3.14.5 delivers *example-based* regression coverage only.
  Property and bounded-model coverage of the same invariants is explicitly
  deferred to 4.2.x/4.3.x and noted in the Decision Log.

- Risk: The "without invoking `shell()`" assertion is driven through the
  `StdlibState::is_impure()` flag, which flips for **any** impure helper —
  `shell()`, `grep()`, *and* `fetch()` (network)
  (`src/stdlib/command/mod.rs:81-102`, `src/stdlib/network/mod.rs:94,177`). It
  is therefore a proxy for "no impure stdlib helper ran during selection", which
  is strictly weaker than "the command-execution port specifically was not
  driven".
  Severity: low. Likelihood: low.
  Mitigation: keep the no-shell fixtures minimal (only `command_available` and
  plain `command:` recipes, no `grep`/`fetch`) so the flag cleanly means "no
  impure helper ran during selection", and state this scoping honestly in both
  the test module and the Hexagonal framing. If a finer-grained,
  shell-specific observable is ever required, escalate rather than overloading
  `is_impure()`.

## Progress

- [ ] Stage A: confirm interfaces and finalise the gap analysis (no code).
- [ ] Stage B: add new tests/fixtures in the failing-then-passing discipline
      with per-test sabotage evidence (completed: none; remaining: all).
- [ ] Stage C: documentation updates (users-guide, developers-guide,
      component architecture, ADR if warranted).
- [ ] Stage D: full gate run, CodeRabbit review, roadmap tick.

(Timestamps to be added as work proceeds.)

## Surprises & discoveries

- Observation: the `path_override` seam is **publicly reachable**, but
  `expand_foreach` is `pub(crate)`.
  Evidence: `StdlibConfig::with_path_override(...)` is `pub`
  (`src/stdlib/config.rs:240`) and `register_with_config` is `pub` and returns
  the `StdlibState`; by contrast `mod which` is private
  (`src/stdlib/mod.rs:18`), so `WhichConfig` is unreachable, and
  `src/manifest/expand.rs:38` shows `pub(crate) fn expand_foreach`.
  Impact: path injection needs **no new seam** — use the public
  `with_path_override`/`register_with_config` pairing. The deterministic
  real-resolver scenarios (nextest-vs-legacy and no-shell) must still be
  **in-crate white-box tests** under `src/manifest/expand_test_cases/`, but the
  reason is solely that they call the `pub(crate)` `expand_foreach`, *not* the
  path override. The earlier speculative "test-only `WhichConfig` constructor
  seam" is dropped as unnecessary.

- Observation: a clean observability seam already exists for "no impure helper
  ran": `StdlibState::is_impure()` is `pub`, and `shell()`, `grep()`, and
  `fetch()` set the shared `impure: Arc<AtomicBool>` when they execute. The
  flag is set **eagerly** (the first statement in the `shell` closure, before
  any spawn), so merely *invoking* the helper flips it regardless of success.
  Evidence: `src/stdlib/mod.rs:44` (`pub fn is_impure`),
  `src/stdlib/command/mod.rs:84` (eager store), `src/stdlib/network/mod.rs:94`.
  Impact: the "absent-command fallback without invoking `shell()`" requirement
  is testable by asserting `is_impure() == false` after expansion of a fixture
  that uses no impure helpers; and the control sub-case can flip the flag
  deterministically with no binary dependency. Scope caveat: `expand_foreach`
  evaluates only `when:`/`foreach:` expressions, not `command:` recipes, so the
  assertion proves *selection-time* purity (exactly the 3.14.5 boundary), not
  whole-pipeline purity.

- Observation: meaningful coverage already exists and must not be duplicated.
  Evidence and inventory are in "Context and orientation".
  Impact: 3.14.5 is a *gap-fill*, not a green-field test suite.

## Decision log

- Decision: Treat 3.14.5 as test-and-docs only; the implementation
  (3.14.2–3.14.4) is already complete.
  Rationale: the roadmap marks 3.14.2/3.14.3/3.14.4 done; recon confirms the
  behaviour is present. The roadmap bullet text is literally "Test ...".
  Date/Author: 2026-06-15, planning agent.

- Decision: Adopt `googletest` in a **confined, spike-gated** form, plus
  `pretty_assertions` broadly, rather than using googletest throughout.
  Specifics: `googletest` (`verify_that!`/`assert_that!` with matchers such as
  `eq`, `len`, `contains`, `unordered_elements_are!`, `not`, `none`) is used
  **only** in the two in-crate white-box files
  (`command_available_selection_cases.rs`, `command_available_no_shell_cases.rs`),
  and only after a Stage A spike confirms `#[gtest]`+`#[rstest]` compile and run
  under the pinned `rstest` 0.18.0. `tests/` integration and snapshot files keep
  the established `anyhow::ensure!`+`insta` idiom. `pretty_assertions` 1.4.x
  (drop-in `assert_eq!`/`assert_ne!` shadow; human-facing diffs only, never
  used for snapshot comparison) may be used wherever a value-equality diff
  improves readability.
  Rationale: the brief requests these crates for "clear test semantics", but the
  community-of-experts review unanimously flagged that (a) googletest has zero
  prior use in this repo (which is uniformly `ensure!`-based), (b) its
  `#[gtest]`/`#[rstest]` interop is unverified against the old `rstest` 0.18.0
  pin and is a real compatibility risk, and (c) the 3.14.5 assertions are
  trivial. Confining googletest to the two files that most benefit (real-world
  matcher ergonomics for the selection/no-shell cases) honours the brief while
  containing the inconsistency and interop risk; the Stage A spike with a
  defined fallback removes the "assume it works" hazard.
  Date/Author: 2026-06-15, planning agent.
  **OPEN QUESTION FOR THE APPROVER (see "Open questions"):** the crew's
  recommendation was stronger — drop `googletest` entirely and keep only
  `pretty_assertions`. This plan keeps googletest (confined) to respect the
  brief; the approver may instead elect the crew's recommendation at the
  approval gate.

- Decision: Inject `path_override` via the public
  `StdlibConfig::with_path_override`; do not use `WhichConfig`.
  Rationale: `mod which` is private, so `WhichConfig` is unreachable from tests;
  the public `with_path_override`+`register_with_config` pairing already drives
  the real resolver (precedent: `tests/which_diagnostic_snapshot_tests.rs:30`),
  needing no new seam. In-crate placement is forced only by `expand_foreach`
  being `pub(crate)`.
  Date/Author: 2026-06-15, planning agent.

- Decision: Document, but do not work around, the env-scoped `which` cache.
  Rationale: the resolver cache is per-`Environment`, not process-global; the
  new tests build their own environments and need no `#[serial]` and no
  `fresh=true`. Recording this prevents future cargo-culted serialisation.
  Date/Author: 2026-06-15, planning agent.

- Decision: Defer property-based and bounded-model coverage of the same
  invariants to roadmap 4.2.x (Kani) and 4.3.2 (Proptest for manifest
  expansion invariants).
  Rationale: 3.14.5 is scoped to example/behavioural/snapshot regression
  coverage; 4.3.2 already owns "foreach preserves non-control fields", "when is
  removed after evaluation", and "item/index injected". Duplicating that here
  would create two owners for one invariant.
  Date/Author: 2026-06-15, planning agent.

- Decision: `ortho_config` is out of scope for this item.
  Rationale: `ortho_config` governs layered CLI/configuration precedence
  (roadmap 3.11.x) and localized help; 3.14.5 concerns manifest-time expansion
  and lowering, which do not read layered configuration. The brief's mention is
  acknowledged and explicitly scoped out to prevent creep.
  Date/Author: 2026-06-15, planning agent.

- Decision: Place deterministic real-resolver tests in-crate; place the
  end-to-end deps+Ninja snapshot test in `tests/` using a guaranteed-absent
  command for determinism.
  Rationale: `WhichConfig`/`expand_foreach` are `pub(crate)` (white-box only);
  an absent command resolves to `false` deterministically regardless of host,
  so a `tests/`-level snapshot can select the fallback branch reproducibly.
  Date/Author: 2026-06-15, planning agent.

## Outcomes & retrospective

To be completed at milestone boundaries and at completion.

## Context and orientation

This section assumes no prior knowledge of the repository.

### Vocabulary

- **Manifest**: the user's YAML build description (`Netsukefile` / `*.yml`).
- **Typed AST**: the deserialized Rust representation of a manifest
  (`src/ast.rs`: `NetsukeManifest`, `Target`, `Rule`, `Recipe`,
  `StringOrList`).
- **Manifest-time expansion**: evaluation of `foreach`/`when` on the *raw*
  manifest value before the typed AST exists (`src/manifest/expand.rs`).
- **IR (intermediate representation)**: the build graph
  (`src/ir/graph.rs`: `BuildGraph`, `Action`, `BuildEdge`).
- **Implicit dependency**: a `deps` entry that affects rebuild/ordering but is
  not passed to the recipe as `$in`; emitted in Ninja after a `|` separator.
- **`command_available(name, **kwargs)`**: a non-throwing Jinja function that
  returns `true`/`false` for executable presence
  (`src/stdlib/which/mod.rs`).
- **`shell()`**: an *impure* Jinja filter that runs a command during rendering
  and sets the stdlib `impure` flag (`src/stdlib/command/`).

### Pipeline (data flow)

`manifest::from_str/from_path`
→ parse YAML to a raw `ManifestValue`
→ register the stdlib into a Jinja `Environment` (this is where
  `command_available` and `shell` are bound), capturing a `StdlibState`
→ `expand::expand_foreach(&mut doc, &env)` removes `when:false` entries and
  fans out `foreach`, injecting `item`/`index`
→ deserialize to the typed AST (`deserialize_actions` stamps `phony: true`)
→ `render::render_manifest` evaluates remaining string templates
→ `ir::BuildGraph::from_manifest` lowers `sources`→`inputs`,
  `deps`→`implicit_deps`, `order_only_deps`→`order_only_deps`, and runs cycle
  detection over `inputs` + `implicit_deps`
→ `ninja_gen::generate` formats `build` lines as
  `build <out> [| <impl_out>]: <rule> <in> [| <impl_deps>] [|| <order_only>]`.

Key files and symbols (full paths relative to repo root):

- `src/ast.rs`: `Target` (fields `sources`, `deps`, `order_only_deps`,
  `phony`), `Rule.deps`, `StringOrList`, `deserialize_actions`.
- `src/manifest/expand.rs`: `expand_foreach` (`pub(crate)`),
  `expand_section`, `expand_target`, `when_allows`, `eval_when`,
  `when_context`, `inject_iteration_vars`, `FilteringStats`.
- `src/manifest/mod.rs`: `from_str`, `from_path`, `from_str_named`
  (takes `stdlib_config: Option<StdlibConfig>`).
- `src/stdlib/mod.rs`: `StdlibState::is_impure` / `reset_impure` (`pub`).
- `src/stdlib/register.rs`: `register`, `register_with_config` (`pub`).
- `src/stdlib/which/mod.rs`: `WhichConfig::new` (`pub(crate)`),
  `command_available_with`, `is_command_available`.
- `src/ir/from_manifest.rs`: `BuildGraph::from_manifest`, `process_targets`
  (`to_paths(&target.sources)`→inputs, `to_paths(&target.deps)`→implicit_deps).
- `src/ir/graph.rs`: `BuildEdge { inputs, implicit_deps, order_only_deps, ... }`.
- `src/ninja_gen.rs`: `generate`, `generate_into`, `DisplayEdge` (the `|`/`||`
  formatting).

### Existing coverage (do not duplicate; extend or complement)

- `src/manifest/expand_test_cases/condition_cases.rs`: parameterised
  `targets`/`actions` cases for `when:false` removal before typed AST,
  `foreach` iteration-var injection, static action `when` dropping, sequence
  `foreach`, truthiness table, invalid-`when` errors, and **complementary
  `command_available` branches for actions** — but the latter uses a *stubbed*
  `command_available` function, not the real resolver.
- `src/manifest/expand_test_cases/target_command_available_cases.rs`: the same
  complementary pattern for targets, also stubbed.
- `tests/data/actions_command_available_absent.yml` + BDD scenarios in
  `tests/features/manifest.feature` ("Selecting a fallback action when a
  command is unavailable", "Parsing fails when command availability receives
  invalid options") and steps in `tests/bdd/steps/conditional_manifest.rs`:
  exercise the *real* resolver for the absent case, but do **not** assert that
  `shell()` was not invoked, and do not use the nextest-vs-legacy framing.
- `tests/ir_from_manifest_tests.rs`: `manifest_deps_populate_implicit_deps`,
  `manifest_deps_do_not_contribute_to_recipe_inputs` — static targets only.
- `tests/ninja_snapshot_tests.rs`: `conditional_manifest_ninja_snapshot`
  (foreach+when on a target), `implicit_deps_manifest_ninja_snapshot`
  (`tests/data/implicit_deps.yml`) — but the two concerns (conditional
  selection and `deps` lowering) are never combined in one fixture.

### Gap analysis (what 3.14.5 must add)

1. **Action-level `when` and `foreach`** — mostly covered. Gap: a single
   integrated regression fixture exercising an action that carries `foreach`,
   `when`, *and* `deps` together, end-to-end to Ninja.
2. **Complementary nextest/legacy branches select exactly one action** — not
   covered. The existing complementary tests are stubbed and use generic
   names. New: a real-resolver, nextest-vs-legacy scenario proving exactly one
   action survives in both the present and absent worlds.
3. **Absent-command fallback without invoking `shell()`** — not covered. New:
   assert `StdlibState::is_impure() == false` after selecting the fallback.
4. **`deps` lowering for conditional actions, in IR and Ninja** — partially
   covered for static targets only. New: assert that a *conditionally-selected*
   action's `deps` become `implicit_deps` in the IR and `| ...` in Ninja, and
   that the *unselected* branch's deps never appear.

### Hexagonal framing (boundaries this suite protects)

Treat manifest-time conditional expansion as **domain/policy** logic. It
depends on two **driven ports**:

- *Executable discovery* (the `which` resolver behind `command_available`).
- *Command execution* (the `shell()` helper).

The core regression contract is a boundary assertion: **conditional selection
drives no impure side-effecting port.** `StdlibState::is_impure()` is the
observable proxy, but it is honest about its breadth — it flips for *any*
impure helper (`shell()`, `grep()`, or `fetch()`), so the precise statement the
test pins is "selection executed no impure stdlib helper", which subsumes (and
is slightly broader than) "the command-execution port was not driven". The
no-shell fixtures deliberately use only `command_available` and plain
`command:` recipes so the flag's meaning is unambiguous in context. No new
ports or adapters are introduced; the tests pin the existing boundary so future
refactors cannot quietly make selection depend on running commands. This plan
uses hexagonal thinking to choose *what to assert at the boundary*, not to
restructure code. (If a strictly shell-only observable is ever needed, that is
an escalation, not an overload of `is_impure()`.)

## Plan of work

### Stage A — confirm interfaces, run the interop spike, finalise gap analysis

This stage adds **one throwaway spike** (immediately reverted) and otherwise
changes no code.

1. **googletest interop spike (go/no-go for the googletest decision).** Add a
   single throwaway test under the lib target:

   ```rust
   // scratch, never committed
   use googletest::prelude::*;
   use rstest::rstest;
   #[gtest]
   #[rstest]
   #[case(1)]
   fn spike(#[case] n: i32) -> googletest::Result<()> { verify_that!(n, eq(1)) }
   ```

   Run it. If it compiles and passes under `rstest` 0.18.0, proceed with the
   confined googletest approach. If it fails, adopt the fallback (bare
   `verify_that!(...)?` in a plain `#[rstest] -> googletest::Result<()>` without
   `#[gtest]`, or `ensure!`), and record the outcome in the Decision Log.
   Revert the spike before any milestone commit.

2. **Confirm the injection seam (public, no new seam).** Verify
   `crate::stdlib::register_with_config(&mut Environment, StdlibConfig)
   -> anyhow::Result<StdlibState>` and that `StdlibConfig::with_path_override`
   threads into the resolver (`src/stdlib/register.rs:104`). Mirror the
   precedent in `tests/which_diagnostic_snapshot_tests.rs:30`. Do **not** use
   `WhichConfig` (private module).

3. **Confirm `expand_foreach` drives selection** in a white-box test with the
   *real* `command_available` registered (existing cases use a stubbed
   `Environment::new()` function; the new cases register the full stdlib so the
   real resolver runs).

4. **Confirm deterministic deps ordering** in `DisplayEdge::fmt`
   (`src/ninja_gen.rs`): record the exact `build` line shape and confirm
   `implicit_deps`/`order_only_deps` are emitted in a deterministic order
   before writing any snapshot. If ordering is insertion-order rather than
   sorted, fixtures must use already-sorted deps so the snapshot is stable.

Go/no-go: if the spike fails *and* no acceptable fallback compiles, or any
required seam is genuinely missing, escalate per Tolerances.

### Stage B — red/green tests with sabotage evidence

Because the production behaviour already exists, classic Red-Green-Refactor is
adapted: each new test is written, shown to **pass** against current code, and
then proven **non-vacuous** by a *sabotage check* — temporarily breaking the
single production line it guards and confirming the test fails. The sabotage
evidence must capture the **assertion-level failure message** (the specific
`verify_that!`/`ensure!`/`assert_snapshot!` line that fired and a diff that
corresponds to the guarded behaviour, e.g. "expected len 1, got 2", or
`implicit_deps` content appearing under `inputs`), not merely "test failed" —
this proves the test fails for the *right reason*, which a bare pass/fail
toggle does not. The sabotage diff is reverted with an explicit
`git checkout -- <file>` (recorded so a resumed agent can confirm a clean tree)
and is never committed; the transcript is recorded under "Artifacts and notes".
This is the documented substitute for the red stage (see the execplans guidance
on observable substitutes).

Add the following, smallest first:

1. **In-crate: nextest-vs-legacy exactly-one-action (real resolver).**
   New file `src/manifest/expand_test_cases/command_available_selection_cases.rs`
   (wired into the `expand_test_cases` module). An `#[rstest]` parameterised
   over two worlds, driving the *real* `command_available` (registered via
   `register_with_config` with a `StdlibConfig`):
   - *present*: a `tempfile::TempDir` containing a fake `cargo-nextest`
     executable, injected via `StdlibConfig::with_path_override`. Reuse the
     repo's existing cross-platform helper idiom from
     `tests/which_diagnostic_snapshot_tests.rs:48-75` (`tool_filename` adds
     `.cmd` on Windows; `mark_executable` sets `0o755` on Unix) so the present
     branch runs on **both** platforms rather than a bare `#[cfg(unix)]` — the
     Windows PATHEXT path (`src/stdlib/which/env.rs`) is the part most likely to
     regress. Expect exactly the `run-tests-nextest` action (`command: cargo
     nextest run`) to survive.
   - *absent*: the three-guard recipe — empty `path_override` +
     guaranteed-absent command name + `cwd_mode="never"`. Expect exactly the
     `run-tests-legacy` action (`command: cargo test`) to survive.
   Drive `expand_foreach`, then assert (googletest, per Stage A spike):
   `verify_that!(actions, len(eq(1)))?`, the surviving action's `name`/`command`
   via `matches_pattern!`/`eq`, and that `when` was removed.
   This also explicitly pins **bullet 1** for actions: include one case where
   the surviving action additionally carries a `foreach` so the assertion
   covers action-level fan-out and `when` together (complementing, not
   duplicating, `condition_cases.rs`).

2. **In-crate: absent fallback without `shell()`.**
   New file `src/manifest/expand_test_cases/command_available_no_shell_cases.rs`.
   Register the full stdlib (so `command_available`, `shell`, `grep`, `fetch`
   all exist), capture `StdlibState`, expand a manifest whose two actions use
   `command_available("<guaranteed-absent>")` / `not command_available(...)`
   and whose recipes are plain `command:` strings (no impure helpers), then
   assert: exactly one action (the fallback) survives, `when` removed, and
   `verify_that!(state.is_impure(), eq(false))?`. Add a contrasting *control*
   sub-case proving the proxy is non-vacuous: a fixture that calls
   `shell('true')` **inside a `when:` expression** (e.g.
   `when: "{{ '' | shell('true') }} == ''"`) — because `expand_foreach`
   evaluates `when:`/`foreach:` but not `command:` recipes, the helper must be
   in a `when:` to flip the flag during the same call under test. The impure
   flag is set eagerly on invocation, so assert `is_impure()==true`
   deterministically with **no** `ensure_binaries_available` gate (avoiding a
   skip-driven hole).

3. **Integration + IR: conditional action carries `deps` into `implicit_deps`,
   including the `item`-in-`deps` interaction.**
   New fixture `tests/data/conditional_action_deps.yml`: a target and an action,
   each selected via a complementary `command_available` pair using a
   guaranteed-absent command (`cwd_mode="never"` retained) so the fallback
   branch is deterministic, each carrying `sources`, `deps`, and
   `order_only_deps`. Crucially, the action uses `foreach` and interpolates
   `{{ item }}` (or `{{ index }}`) into **one `deps` entry** (e.g.
   `deps: [build/{{ item }}.o]`) — the per-item implicit-dependency case is the
   single most likely real-world regression and is currently untested anywhere.
   **Add the new tests to the existing `tests/ir_from_manifest_tests.rs`** (beside
   the static-deps cases `manifest_deps_populate_implicit_deps` /
   `manifest_deps_do_not_contribute_to_recipe_inputs`, keeping all deps-lowering
   tests in one place). Assert that each selected edge has `implicit_deps` equal
   (unordered) to the declared/substituted `deps`, `inputs` equal to `sources`
   only, `order_only_deps` carried through, the expected per-`item`
   substitution, and that none of the *unselected* branch's paths appear in any
   edge. (Cycle detection over `implicit_deps` is already owned by
   `src/ir/cycle.rs` unit tests and is not re-tested here.)

4. **Snapshot + real-ninja: conditional action deps reach the Ninja file.**
   New test `conditional_action_deps_ninja_snapshot` **added to
   `tests/ninja_snapshot_tests.rs`** using the fixture from B.3. Assert the
   selected `build` line contains `| <dep>` (implicit) **and** the `|| <order_only>`
   segment (make `order_only_deps` definitely present in the fixture so this is
   not "where applicable"); assert the unselected branch's outputs/deps are
   absent; then `insta::assert_snapshot!` into `tests/snapshots/ninja/` reusing
   the existing `Settings::set_snapshot_path`. Use `insta::assert_snapshot!`
   only for the rendered Ninja text — never `pretty_assertions::assert_eq!`.
   Where `ninja`/`python3` are available, also run `ninja -t query`/`-n` to
   prove the file is valid and reaches a no-op second pass, mirroring
   `touch_manifest_ninja_validation`; surface any skip via `eprintln!` (captured
   by the harness) in addition to `tracing::warn!` so a binary-less CI run does
   not silently validate nothing (see Risks / Doggylump S2).

5. **BDD: the combined conditional-action-with-deps scenario only.**
   The unit (B.1), IR (B.3), and snapshot+real-ninja (B.4) layers already pin
   selection and deps emission; existing BDD scenarios
   (`tests/features/manifest.feature:132`, `tests/features/ninja.feature:31`)
   already cover fallback-selection and deps-emission *separately*. To avoid
   redundant step-matcher maintenance, B.5 adds **only** the genuinely new
   externally-observable case: a single scenario where a *conditionally-selected*
   action's `deps` appear as Ninja implicit dependencies end-to-end. Reuse
   existing steps in `tests/bdd/steps/conditional_manifest.rs` and
   `tests/bdd/steps/ninja.rs`; add at most one new `Then` (the selected action
   exposes a given implicit dependency) if no existing step fits. If even this
   is fully covered by reframing an existing scenario, prefer reframing over a
   new scenario.

Each of B.1–B.5 ends with the focused test passing, its sabotage check
recorded, and the relevant gate (`make test`) green.

### Stage C — documentation

- `docs/users-guide.md`: ensure the conditional-action / `deps` semantics are
  described from a user's perspective (selection by tool availability; `deps`
  as implicit dependencies that affect rebuilds but not recipe arguments). Add
  a short worked nextest-vs-legacy example if not already present. (Behaviour
  is unchanged, so this is clarification, not new UI.)
- `docs/developers-guide.md`: document the test conventions introduced here —
  when to reach for `googletest`/`pretty_assertions` vs `ensure!`, the
  `path_override`/`cwd_mode` pattern for deterministic `command_available`
  tests, and the `is_impure()`-as-boundary-proxy idiom.
- Component architecture doc for the manifest/stdlib boundary: record the
  port framing (executable-discovery vs command-execution) and that 3.14.5
  pins it. If the decision to standardise the boundary assertion is judged
  substantive, capture it as an ADR using the `arch-decision-records`
  Y-Statement format and reference it from this plan and the design doc.
- `docs/roadmap.md`: tick 3.14.5 and its four sub-bullets on completion.

### Stage D — gates, review, finalise

Run the full gate suite, then `coderabbit review --agent`; clear all concerns
before declaring done. Update Progress/Outcomes.

## Concrete steps

Run from the repository root. Use `tee` to a per-action log under `/tmp` so
truncated output can be reviewed:

```bash
# focused red/green loop for a single new test (example)
cargo test --test ir_from_manifest_tests conditional_action_deps \
  2>&1 | tee /tmp/test-netsuke-$(git branch --show-current).out
```

```bash
# in-crate (white-box) expansion tests live in the library target
cargo test --lib expand_test_cases::command_available \
  2>&1 | tee /tmp/test-netsuke-$(git branch --show-current).out
```

```bash
# update/inspect snapshots deliberately (review before accepting)
cargo insta test --review    # local authoring only
# CI / gates must never auto-accept a changed snapshot:
INSTA_UPDATE=no make test
```

```bash
# milestone gates (run sequentially to benefit from build caching)
make check-fmt 2>&1 | tee /tmp/check-fmt-netsuke-$(git branch --show-current).out
make typecheck 2>&1 | tee /tmp/typecheck-netsuke-$(git branch --show-current).out
make lint      2>&1 | tee /tmp/lint-netsuke-$(git branch --show-current).out
make test      2>&1 | tee /tmp/test-netsuke-$(git branch --show-current).out
```

Expected: each gate reports success; `make test` shows the new tests passing
and the existing suite unchanged.

## Validation and acceptance

Acceptance is behavioural:

1. `make test` passes with the new tests present.
2. For each new test, the recorded **sabotage check** demonstrates the test
   fails when its guarded production line is broken and passes when restored.
   Representative sabotage points:
   - selection: force `is_command_available` to always return `true` (or always
     `false`) → the exactly-one-action and nextest/legacy tests must fail.
   - no-shell: make conditional selection call `shell()` (or force
     `impure` true) → the `is_impure()==false` test must fail.
   - deps: make `process_targets` route `to_paths(&target.deps)` into `inputs`
     instead of `implicit_deps` → the IR and Ninja deps tests must fail.
3. The new Ninja snapshot matches and, where `ninja`/`python3` are available,
   the generated file builds and a second pass reports "no work to do".
4. BDD scenarios for selection and deps emission pass via the `bdd_tests`
   harness.

Quality criteria ("done"):

- Tests: all new unit (`rstest` + `googletest`), integration, snapshot
  (`insta`), and BDD (`rstest-bdd`) tests pass; existing tests unchanged.
- Lint/typecheck/format: `make lint`, `make typecheck`, `make check-fmt` clean
  with `-D warnings`.
- Docs: users-guide, developers-guide, component architecture (and ADR if
  created) updated; markdown lint clean.
- Review: `coderabbit review --agent` concerns all resolved.

## Idempotence and recovery

All steps are additive and re-runnable. Snapshots are created deliberately via
`cargo insta` and reviewed before acceptance; a wrong snapshot is corrected by
re-reviewing, not by force-accepting. Sabotage checks are always reverted
(`git checkout -- <file>`) and never committed. Commit after each of B.1–B.5
so any step can be rolled back independently.

## Artifacts and notes

Record here, as work proceeds: the exact surviving-action assertions, the
generated Ninja snapshot for the conditional-deps fixture, and the sabotage
transcripts (broken-line diff + failing test name + restored pass) that prove
each new test is non-vacuous.

## Interfaces and dependencies

New `[dev-dependencies]` in `Cargo.toml` (pre-authorised by the brief):

```toml
# Cargo.toml
googletest = "0.14"
pretty_assertions = "1.4"
```

Existing crates reused: `rstest` 0.18, `rstest-bdd`/`rstest-bdd-macros` 0.5,
`insta` 1 (yaml), `tempfile`, `test_support`, `anyhow` (`ensure!`).
(`serial_test` is **not** needed — the new tests build their own environments
and inject `path_override`, so they never touch process-global PATH; see the
env-scoped-cache Decision.)

Seams relied upon (all already public/`pub(crate)` and present; **no new seam
required**):

- `crate::manifest::expand::expand_foreach(&mut ManifestValue, &Environment)
  -> anyhow::Result<FilteringStats>` (`pub(crate)` — forces in-crate placement
  of the selection/no-shell tests).
- `crate::stdlib::register_with_config(&mut Environment, StdlibConfig)
  -> anyhow::Result<StdlibState>` (`pub`; note the `Result`),
  `StdlibConfig::with_path_override(impl Into<OsString>)` (`pub`,
  `src/stdlib/config.rs:240`), and `StdlibState::is_impure(&self) -> bool`
  (`pub`). Do **not** reference `WhichConfig` (private module).
- `netsuke::ir::BuildGraph::from_manifest(&NetsukeManifest)
  -> Result<BuildGraph, IrGenError>` and `netsuke::ninja_gen::generate(&BuildGraph)
  -> Result<String, NinjaGenError>` (`pub`) for the integration/snapshot tests.

New/changed test files and fixtures (final names to be confirmed in Stage A):

- new `src/manifest/expand_test_cases/command_available_selection_cases.rs`
- new `src/manifest/expand_test_cases/command_available_no_shell_cases.rs`
  (both wired into the `expand_test_cases` module).
- new `tests/data/conditional_action_deps.yml`.
- additions to existing `tests/ir_from_manifest_tests.rs` (deps lowering) and
  `tests/ninja_snapshot_tests.rs` (Ninja snapshot + real-ninja) — kept beside
  their static-deps counterparts, not in a separate file.
- new `tests/snapshots/ninja/ninja_snapshot_tests__conditional_action_deps_ninja.snap`.
- one new combined scenario in `tests/features/ninja.feature` (or
  `manifest.feature`) plus at most one new step.
- new `[dev-dependencies]` entries `googletest = "0.14"` and
  `pretty_assertions = "1.4"` in `Cargo.toml`.

## Open questions for the approver

1. **googletest.** The brief mandates googletest; the community-of-experts
   review unanimously recommended dropping it (keeping only `pretty_assertions`)
   because it has no prior use here, its `#[gtest]`+`#[rstest]` interop is
   unverified under `rstest` 0.18.0, and the assertions are trivial. This plan
   keeps googletest in a confined, spike-gated form to respect the brief. The
   approver may instead choose: (a) confined + spike-gated (as planned),
   (b) `pretty_assertions` only, no googletest, or (c) googletest throughout.
2. **BDD scope.** Reviewers judged a full BDD layer largely redundant; the plan
   reduces B.5 to the single combined scenario. Confirm this is the desired
   depth, or request fuller BDD coverage.
3. **Stray file.** `src/stdlib/command/mod_backup.rs` is an unreferenced ~41 KB
   backup duplicating impure-flag logic. Out of scope for 3.14.5, but flagged
   for separate cleanup so future grep-based reasoning is not misled.

## Signposted documentation and skills

Documentation to consult while implementing:

- `docs/netsuke-design.md` §2.5 (manifest-time `foreach`/`when` semantics),
  §2.4 and §5.3 (dependency classes and Ninja lowering),
  and the "executable discovery" section (`command_available`).
- `docs/rust-testing-with-rstest-fixtures.md` — fixture and `#[case]` patterns.
- `docs/reliable-testing-in-rust-via-dependency-injection.md` — the
  `path_override`/resolver-injection approach used for deterministic
  `command_available` tests.
- `docs/rust-doctest-dry-guide.md` — if any doc examples are added.
- `docs/rstest-bdd-users-guide.md` — feature/step wiring for the BDD additions.
- `docs/ortho-config-users-guide.md` — referenced by the brief; explicitly
  out of scope here (see Decision Log).

Skills to load while implementing:

- `rust-router` then `rust-unit-testing` (assertion helpers, fixtures, table
  tests, `googletest`/`pretty_assertions`/`insta` usage).
- `hexagonal-architecture` — to keep the boundary assertions honest.
- `nextest` — for the nextest-vs-legacy framing and running the suite.
- `leta` — for navigation/refactors.
- `arch-decision-records` — only if the boundary decision warrants an ADR.
- `proptest` / `kani` — *not* used here; their invariants are owned by 4.3.2 /
  4.2.x (recorded in the Decision Log).

## Revision note

- 2026-06-15 — Revised after a community-of-experts review (Logisphere crew:
  structural, contract/correctness, alternatives/DX, reliability/ops lenses).
  What changed and why:
  - Corrected the injection seam: `WhichConfig` is in a private module and
    unreachable; switched to the public `StdlibConfig::with_path_override`
    +`register_with_config` pairing (precedent in
    `tests/which_diagnostic_snapshot_tests.rs`). Removed the speculative
    test-only seam. In-crate placement is now justified solely by
    `expand_foreach` being `pub(crate)`.
  - Restated the `is_impure()` boundary proxy honestly: it flips for `shell()`,
    `grep()`, *and* `fetch()`; downgraded the hexagonal claim accordingly.
  - Added a Stage A `#[gtest]`+`#[rstest]` interop spike (rstest 0.18.0 risk)
    with a defined fallback; confined googletest to the two white-box files and
    surfaced the crew's "drop googletest" recommendation as an approver choice.
  - Hardened determinism: the absent case now requires empty `path_override` +
    guaranteed-absent name + `cwd_mode="never"` together.
  - Fixed the `shell()` control sub-case to live in a `when:` expression
    (expansion evaluates `when`/`foreach`, not `command:` recipes) and removed
    its binary-availability gate (the impure flag is set eagerly).
  - Added the `item`-in-`deps` interaction (highest-value uncaught regression),
    pinned action-level `foreach` explicitly, and folded the IR/Ninja tests
    into existing files. Trimmed the BDD layer to the single combined scenario.
  - Added CI snapshot discipline (`INSTA_UPDATE=no`), skip visibility via
    `eprintln!`, the env-scoped-cache note (no `serial_test`), corrected
    `register_with_config -> anyhow::Result<StdlibState>`, and an
    "Open questions for the approver" section.
  Effect on remaining work: scope and file count are essentially unchanged; the
  plan is now anchored to reachable seams and verifiable assumptions. No code
  has been written; the plan remains in DRAFT pending approval.
