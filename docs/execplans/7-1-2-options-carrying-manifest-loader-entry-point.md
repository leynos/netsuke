# Introduce the options-carrying manifest loader entry point (7.1.2)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: COMPLETE

Approved 2026-10-14 by the user, by explicit instruction to proceed with this
plan under the `sdlc-implementation` workflow. That approval accepts the
technical-design deviations listed under `Decision log` (entries marked
"Deviation"). The user had already settled the one open question, about
file-reading filters under test (see `Decision log`).

The branch was rebased onto `origin/main` (`486b876c`) on 2026-10-14 before any
work began, because the base had fallen 64 commits behind while the plan sat in
draft. The rebase applied with zero conflicts and is recorded under
`Surprises & discoveries`; the plan's paths, base commit, and ADR number were
corrected to match. No production code had changed before that rebase.

## Purpose / big picture

Netsuke turns a YAML manifest (a `Netsukefile`) into a Ninja build file. On the
way it renders Jinja templates through MiniJinja, and those templates may call
helpers that look at the host: `env()`, `glob()`, `now()`, `which`, `fetch`,
`shell`, and file readers such as `contents`. Roadmap phase 7 adds
`netsuke test`, which checks a manifest's logic by driving *the same* loader
with those helpers replaced by deterministic stand-ins.

Today the loader cannot be driven that way. It has eleven public entry points
(`from_str`, `from_str_with_env`, `from_path_with_policy_and_env_and_limits`,
and so on), each hard-wiring one combination of inputs, and it knows exactly
two stdlib surfaces: the full build surface and the side-effect-free surface
used by `netsuke help targets`. There is no way to hand it a third surface, and
no way to install a stand-in function before the manifest's `foreach` and
`when` expressions are evaluated.

After this change:

1. One options-carrying entry point, `netsuke::manifest::from_str_with_options`,
   accepts a `ManifestLoadOptions` value naming the stdlib surface, the
   environment, the resource budget, and a stage observer. Every existing entry
   point becomes a thin wrapper that builds options and forwards to it.
2. The loader's stdlib-surface enum, `StdlibRegistration`, gains a `Test` mode
   beside `Full` and `ManifestQuery`. A test load renders pure helpers exactly
   as a build does, reads `now()` only from an explicitly injected clock, reads
   `env()` only from an injected reader (answering "not set" by default), and
   refuses every other host-observing helper with a localized diagnostic
   carrying the stable code `[netsuke::jinja::test_mode::unavailable]` and the
   helper's name.
3. A test surface can carry `TemplateOverlays`: named callables that replace
   same-named template functions. They are registered after the stdlib and
   manifest macros and before `foreach` expansion, and each overlay can reach
   the function it replaced, so later roadmap items can build spies that pass
   through.

A novice can observe success as follows. A new test loads a manifest whose
`foreach` iterates `glob('src/*.c')`, first in test mode with no overlay (the
load fails with `[netsuke::jinja::test_mode::unavailable]` naming `glob`), then
with an overlay for `glob` returning `["a.c", "b.c"]` (the load succeeds and
yields targets `a.o` and `b.o`). A differential property test generates 128
pure manifests and shows that build mode and test mode produce identical
manifests, graphs, and Ninja text for every one. Every pre-existing snapshot
and behavioural test passes unchanged, which demonstrates that the build path
did not move.

There is no user-visible change. No command-line flag, configuration key, or
output changes. `netsuke test` itself arrives in later roadmap items.

## Definitions

Terms used throughout, defined here so that no prior reading is required.

- **Manifest loader.** The code under `src/manifest/` that parses YAML, builds a
  MiniJinja environment, expands `foreach`/`when`, deserializes into
  `NetsukeManifest` (`src/ast/mod.rs`), and renders string fields.
- **Stdlib.** Netsuke's library of Jinja functions, filters, and tests,
  registered into a MiniJinja `Environment` by `src/stdlib/register.rs`.
- **Stdlib surface / registration mode.** Which template helpers are real,
  which are refusing stubs, and what configuration backs them. Selected by the
  crate-private enum `StdlibRegistration` in `src/manifest/mod.rs`.
- **Refusing stub.** A helper registered under its real name whose only
  behaviour is to raise a MiniJinja `InvalidOperation` error naming itself.
  Refusing stubs beat leaving a helper unregistered: under strict-undefined
  MiniJinja an unregistered helper produces a generic "unknown function" error,
  whereas a stub produces a located, actionable diagnostic.
- **Test surface (`TestSurface`).** The payload of the `Test` mode: the
  `StdlibConfig` (which owns the clock), an optional environment reader, and
  the template overlays.
- **Template overlay.** A callable registered as a MiniJinja global function
  *after* the stdlib and manifest macros, so it replaces any same-named
  function. Test doubles (roadmap 7.3.1) will be delivered as overlays.
- **Shadowed function.** The global an overlay replaced, captured at
  registration and handed to the overlay on each call.
- **Environment reader (`EnvReader`).** The closure type backing the `env()`
  template helper (`src/manifest/env/reader.rs`). `ManifestEnvironment` bundles
  a reader with the `EnvAccessPolicy` evaluated before each read.
- **Clock provider.** The closure type backing `now()`, owned by `StdlibConfig`
  since roadmap 7.1.1 (`src/stdlib/time/clock/mod.rs`).
- **Query classification.** In `netsuke help targets`, a `when` expression that
  calls a disabled helper does not fail; `classify_query_evaluation`
  (`src/manifest/jinja_macros/query_classification.rs`) recognizes the query
  refusal and the entry is kept with `conditional: true`.
- **Invariants I4, I5, I7.** Named obligations from the technical design §12.
  I4, semantic fidelity: a manifest with no doubles whose file observations are
  confined to the sandbox gives identical results under `netsuke test` and
  under the build. I5: under test, no build command, network socket, or host
  environment read occurs without explicit opt-in. I7, build-path neutrality:
  the build behaves identically whether or not test machinery exists.
- **Differential test.** A test that runs two code paths over the same input
  and asserts their outputs are identical, so neither path needs a handwritten
  expected value.
- **Characterization snapshot.** An `insta` snapshot recorded from the
  *unchanged* code before a refactor, used afterwards as the oracle proving the
  refactor preserved behaviour.
- **Canonical form.** JSON serialized with sorted keys through the existing
  `serde_json_canonicalizer` dependency, so `HashMap`-backed fields such as
  `NetsukeManifest::vars` snapshot identically in every process.
- **`scrutineer`.** The agent that runs the repository gates sequentially and
  reports results with log paths. Only it runs full gates.
- **Whitaker test-module split.** The repository convention for large test
  modules: a `#[cfg(test)] #[path = "…_tests.rs"] mod tests;` child file, so
  production files stay under the 400-line cap and Whitaker's module lints are
  satisfied.
- **Driving port / driven port.** Hexagonal-architecture vocabulary. A driving
  port is how callers invoke the domain (here, `from_str_with_options`); a
  driven port is what the domain needs from outside (here, the environment
  reader, clock, and template helpers).

## Context and orientation

Assume no prior knowledge of this repository. This section describes the code
as it stands on `main` at commit `486b876c`, which this branch was rebased onto
on 2026-10-14. Line numbers are indicative; anchor on the named items, because
line numbers drift.

Module paths were reconciled against that rebase: `main` landed a module-layout
migration between this plan's draft and its implementation, so several files
named below are now directory modules. See `Surprises & discoveries` for the
full reconciliation.

### The loader core

`src/manifest/mod.rs` owns the private function `evaluate_manifest`, the only
place a manifest is evaluated. It receives a private bundle, `ManifestParse`,
holding the diagnostic name, an `Option<StdlibRegistration>`, the environment
reader and access policy, an optional manifest root (anchoring relative
`glob()` patterns), an optional expansion-report observer, and resource-budget
limits. Its order of work is:

1. notify `ManifestLoadStage::InitialYamlParsing`; charge the source budget;
   parse YAML with `serde_saphyr` into a `serde_json::Value` tree;
2. create a MiniJinja `Environment` with strict undefined behaviour;
3. register `env()` over the injected reader and policy, and `glob()` over the
   manifest root;
4. register the stdlib according to `StdlibRegistration`: `Full(config)` calls
   `stdlib::register_with_config`, `ManifestQuery` calls
   `stdlib::register_manifest_query` (which *overwrites* `env` and `glob` with
   refusing stubs), and `None` calls `stdlib::register` (which builds an
   ambient configuration from the current directory);
5. register manifest `vars` as globals (`src/manifest/registration.rs`, which
   also rejects the reserved names `env` and `glob`);
6. notify `TemplateExpansion`; register manifest macros
   (`src/manifest/jinja_macros/mod.rs`), which add one global per macro and
   append `{% from … import … %}` lines to the global
   `__netsuke_manifest_macro_imports`, prepended to every rendered template;
7. run `expand_foreach_with_budget`, which evaluates `foreach` and `when`
   against the raw value tree;
8. notify `FinalRendering`; deserialize into `NetsukeManifest`; render string
   fields (`src/manifest/render/mod.rs`, whose `RenderMode::ManifestQuery`
   skips command rendering); validate recipes.

The function body is about 68 lines; `clippy.toml` caps functions at 70 lines
and cognitive complexity at 9, so it must be split before it grows.
`src/manifest/budget/adapter.rs::from_str_named` wraps `evaluate_manifest` and
records budget-exhaustion telemetry except for manifest queries.

### The entry points

`src/manifest/mod.rs` exposes `from_str` and `from_str_with_env`.
`src/manifest/parse_with_config.rs` exposes `from_str_with_env_and_config` and
`from_str_with_env_and_policy`. `src/manifest/path_loaders.rs` exposes seven
path-based loaders: `from_path`, `from_path_with_policy`,
`from_path_with_policy_and_limits`, `from_path_with_policy_and_env`,
`from_path_with_policy_and_env_and_limits`,
`from_path_with_policy_and_environment`, and
`from_path_with_policy_and_environment_and_limits`. The last is the production
build's choice (`src/runner/generation.rs`,
`load_manifest_for_build_with_limits`); two of the seven are reached only
through other wrappers. Each path loader ends in
`src/manifest/query.rs::from_path_with_registration`, which opens a
capability-scoped workspace, reads the file, and builds a `StdlibRegistration`
from a second private enum, `ManifestLoadMode` (`Full(NetworkPolicy)` or
`ManifestQuery`). `query.rs` also owns the crate entry
`from_path_for_manifest_query_with_limits`, used by `netsuke help targets`
(`src/runner/generation.rs::load_manifest_with_limits`).

### The restricted-load precedent

`netsuke help targets` established a side-effect-free load. Its pieces are:

- `src/stdlib/register.rs::register_manifest_query`, which registers lexical
  path filters, collection filters, and clock-independent time helpers, then
  refusing stubs for `env`, `glob`, `fetch`, `shell`, `grep`, `contents`,
  `which` (function and filter), `command_available`, `now`, `realpath`,
  `expanduser`, `size`, `linecount`, `hash`, and `digest`;
- `manifest_query_operation_error(operation)`, which builds the stub error from
  a hard-coded English marker (`MANIFEST_QUERY_DISABLED_HELPER_MARKER`). The
  query stubs for `hash`, `digest`, and `contents` accept fewer arguments than
  the real filters, so a call with keyword arguments fails on arity instead of
  refusing. **Path reconciled at rebase:** `main` already extracted this
  function, the marker, `is_manifest_query_disabled_error`, and the stub
  registrations into `src/stdlib/register/query_helpers.rs`, a `#[path]` child
  of `register.rs`. The remaining `register.rs` re-exports
  `is_manifest_query_disabled_error` and declares the marker;
- `is_manifest_query_disabled_error`, which recognizes that marker by
  substring. `classify_query_evaluation` applies it in *every* mode today, and
  a match turns the `when` result into `WhenResolution::Conditional`: the entry
  is kept and marked `conditional: true` (`src/manifest/expand/mod.rs`);
- `src/manifest/env/reader.rs::disabled_env_reader` (`pub(super)`), a reader
  that answers every lookup with `EnvReadError::NotPresent`.

File sizes at `486b876c`: `src/stdlib/register.rs` is 315 lines and its
`query_helpers.rs` child 151, so EP-M1's extraction there shrinks to relocating
the stub cluster into the new `restricted_helpers/`;
`src/manifest/jinja_macros/mod.rs` is still 394 lines and remains at the cap,
so the classifier extraction it needs is unchanged. The repository caps code
files at 400 lines.

### What the full stdlib touches on the host

`register_with_config` installs a legacy Boolean formatter (so `true` renders as
`true`, not `True`), the file tests in `FILE_TESTS` (`dir`, `file`, `symlink`,
and others) that open parent directories with ambient authority
(`src/stdlib/path/fs_utils.rs::parent_dir`), path filters of which `realpath`,
`size`, `contents`, `linecount`, `hash`, and `digest` read the *ambient*
filesystem relative to the process working directory
(`src/stdlib/path/filters.rs`), `expanduser` (reads the configured home),
`which` and `command_available` (search `PATH`, falling back to the host `PATH`
when `StdlibConfig::path_override` is unset), `now()` (reads the configured
clock), `fetch` (network), and `shell`/`grep` (spawn processes). Manifest-query
registration does *not* install the Boolean formatter.

Localized diagnostics that callers must recognize carry a bracketed code copied
verbatim into every catalogue, for example
`stdlib.which.not_found = [netsuke::jinja::which::not_found] …` in
`locales/en-US/messages.ftl`. There are 35 catalogues under `locales/`.

### The governing design

`docs/netsuke-test-framework-technical-design.md` §4.2 and §4.3 specify this
task. The design predates several loader changes (the `ManifestEnvironment`
bundle, `ManifestBudgetLimits`, the manifest root, and ADR-0050's query-mode
telemetry suppression), and its public-field sketch leaves overlays outside the
mode that grants them. `Decision log` records each reconciliation; EP-M5 writes
them back into the design.

### Files a novice will touch

| Path                                                                    | Change                                                                   |
| ----------------------------------------------------------------------- | ------------------------------------------------------------------------ |
| `src/manifest/loader_options.rs` (new)                                  | `ManifestLoadOptions`, `from_str_with_options`, host-helper registration |
| `src/manifest/test_surface.rs` (new)                                    | `TestSurface`, `TemplateOverlays`, `OverlayCallable`, `OverlayCall`      |
| `src/manifest/mod.rs`                                                   | `StdlibRegistration::{AmbientFull, Test}`; split `evaluate_manifest`     |
| `src/manifest/budget/adapter.rs`                                        | ask the registration whether to record telemetry                         |
| `src/manifest/parse_with_config.rs`, `path_loaders.rs`                  | thin wrappers over the options entry                                     |
| `src/manifest/query.rs`                                                 | `from_path_with_options`; `ManifestLoadMode` replaced by a factory       |
| `src/manifest/jinja_macros/query_classification.rs` (new)               | classifier extracted and gated on query mode                             |
| `src/manifest/jinja_macros/mod.rs`, `src/manifest/expand/*`             | pass the query-classification flag                                       |
| `src/stdlib/restricted_helpers/mod.rs` (new)                            | surface inventory, query and test refusing stubs                         |
| `src/stdlib/register.rs`, `src/stdlib/mod.rs`                           | delegate restricted registration; `register_for_test`                    |
| `src/stdlib/time/clock/mod.rs`                                          | re-document `WallClock::is_system` as a policy input                     |
| `src/localization/keys.rs`, `locales/*/messages.ftl`                    | two new keys                                                             |
| `src/manifest/tests/*.rs`, `src/stdlib/restricted_helpers/tests.rs`     | unit, inventory, and internal differential tests                         |
| `tests/manifest_load_options_tests.rs`, `tests/manifest_load_options/*` | public differential and property tests                                   |
| `tests/features/manifest_load_options.feature`, `tests/bdd/…`           | behavioural scenarios, steps, and `TestWorld` slots                      |
| `tests/ui/manifest_load_options_embedder_pass.rs`                       | compile-pass fixture pinning the public surface                          |
| `docs/…`                                                                | design, ADR-0060, ADR-0050 addendum, guides, contents, roadmap           |

*Table 1: Files in scope.*

### The integration-test wiring contract

`tests/integration_test_wiring_tests.rs` fails if a `tests/*/mod.rs` tree is
not declared from some top-level `tests/*.rs` file, or if Cargo's discovered
test targets differ from the `tests/*.rs` files on disk. A new top-level
`tests/manifest_load_options_tests.rs` is automatically a target; its
submodules under `tests/manifest_load_options/` must be declared with `mod`.
The behavioural suite is `tests/bdd_tests.rs`, whose
`scenarios!("tests/features", …)` collects *every* `.feature` file, and
`rstest-bdd-macros` runs with `strict-compile-time-validation`: a feature file
whose steps do not exist fails to compile the whole behavioural binary, so a
feature file and its steps must land in the same commit.

### Relevant documentation and skills

Read before starting: `AGENTS.md`;
`docs/netsuke-test-framework-technical-design.md` §§4, 5, 7, 12–14;
`docs/netsuke-test-framework-ux-design.md` §§1, 7–8;
`docs/rfcs/rfc-0007-netsukefile-testing-framework.md`; `docs/netsuke-design.md`;
`docs/adrs/adr-0008-environment-seam-taxonomy.md`;
`docs/adrs/adr-0010-scope-the-glob-metadata-capability-to-the-literal-prefix.md`;
`docs/adrs/adr-0050-bound-manifest-template-evaluation.md`; `docs/polonius.md`;
`docs/rust-testing-with-rstest-fixtures.md`; `docs/rstest-bdd-users-guide.md`;
`docs/rust-doctest-dry-guide.md`;
`docs/reliable-testing-in-rust-via-dependency-injection.md`;
`docs/snapshot-testing-in-netsuke-using-insta.md`;
`docs/localization-styleguide.md`; `docs/translators-guide.md` (key audit and
right-to-left rules); `docs/developers-guide.md`;
`docs/documentation-style-guide.md`; `docs/ortho-config-users-guide.md` (for
the reason recorded in `Decision log` that this task adds no configuration
surface); and the completed plan `docs/execplans/7-1-1-clock-provider-seam.md`.

Skills to load: `rust-router` first, then `rust-types-and-apis` (options,
opaque constructors), `nll-to-polonius` (the borrowed `ManifestEnvironment` and
stage observer), `rust-errors` (typed refusal sources), `rust-unit-testing`
(rstest tables, googletest matchers, insta settings), `proptest` (manual
`TestRunner` with counters), `hexagonal-architecture` (capability policy in one
place), `execplans`, and `en-gb-oxendict`.

## Constraints

These are hard invariants. Violating one requires escalation, not a workaround.

- C1. The build path's observable behaviour must not change. Every pre-existing
  test, snapshot (`tests/snapshots/**`, `src/snapshots/**`), and behavioural
  scenario passes without modification. A changed pre-existing snapshot is a
  stop condition.
- C2. `netsuke help targets` behaviour must not change, including the exact
  text of its refusal message.
- C3. Existing public entry points keep their exact signatures and semantics
  and become thin wrappers; none is removed or renamed.
- C4. Test mode never consults ambient state: no host environment variable, no
  implicit clock, no ambient filesystem path, no network, and no process spawn
  is reachable from a test-mode load unless a caller explicitly injects it (an
  environment reader, a clock provider via `StdlibConfig::with_clock`, or an
  overlay).
- C5. Capabilities for a load mode are decided in one place,
  `StdlibRegistration`, including whether `env()` and `glob()` are real. No
  second mode enum survives.
- C6. User-facing refusal and collision text goes through Fluent, and every
  diagnostic a caller must recognize carries a bracketed code identical in all
  catalogues. Recognition in code uses typed error sources, never translated
  text.
- C7. No new crate dependency.
- C8. No in-process environment mutation in tests; use injected readers.
- C9. The overlay mechanism is compiled unconditionally, not behind
  `cfg(test)`, because the test runner is production code of the shipped binary.
- C10. No code file exceeds 400 lines; every module starts with `//!`; every
  function carries `///` documentation; `evaluate_manifest` and every new
  function stay within `clippy.toml`'s 70-line and complexity-9 limits.
- C11. Respect `POLONIUS(...)` and `POLONIUS-REFUSED(...)` tags; do not add
  defensive clones without first compiling the borrowing form.
- C12. After EP-M0, characterization snapshots are never re-accepted. From
  EP-M1 onwards `git diff --exit-code` over their directories must pass. The
  baseline such a diff is taken against is `847c19cc`, not EP-M0's acceptance
  commit: that later commit repaired three vacuous rows and recorded the
  exhaustion path separately, and the snapshots have been stable since it. See
  the Progress entry for `847c19cc`.

## Tolerances (exception triggers)

The counts below exclude this plan, the 35 locale catalogues, `.snap` files,
and YAML fixtures, and were set from the inventory in Table 1.

- Scope: stop and escalate beyond 42 changed files or 2,400 net added lines.
- Interface: stop if any existing public signature must change, or if a public
  item beyond those in `Interfaces and dependencies` seems necessary.
- Dependencies: stop if any new crate, including a dev-dependency, seems
  necessary.
- Behaviour: stop if any pre-existing snapshot, doctest, or behavioural
  scenario needs editing to pass (C1, C2), or if a characterization snapshot
  changes after EP-M0 (C12).
- Iterations: stop if a gate still fails after three fix attempts for the same
  root cause.
- Design: stop and set `Status: BLOCKED` if MiniJinja does not replace a
  same-named global on re-registration, or if a typed error source attached to
  a MiniJinja error does not survive to the loader's `anyhow` chain.
- Ambiguity: stop and present options if a helper's disposition under test is
  not settled by the surface inventory in `Decision log`.

## Risks

- Risk: converting entry points to wrappers changes error ordering or text, for
  example by building an ambient `StdlibConfig` before YAML parsing where today
  it is built lazily afterwards. Severity: high. Likelihood: medium.
  Mitigation: a named `AmbientFull` registration resolved at the same step as
  today; canonical characterization snapshots of results *and* error text from
  EP-M0; C12.

- Risk: a test-mode refusal is classified as a query refusal inside `when`,
  keeping the entry as `conditional: true` so the load succeeds with a
  different manifest. Severity: high. Likelihood: medium without a structural
  fix. Mitigation: the classifier runs only for `ManifestQuery` loads (a flag
  passed into expansion), and test refusals carry a distinct typed source;
  OBL-4 seeds both faults.

- Risk: a new stdlib filter or test is added later without a test-mode
  decision and leaks host state. MiniJinja 2.24.0 cannot list registered
  filters or tests. Severity: high. Likelihood: medium over the project's life.
  Mitigation: a single surface inventory drives test registration, and a
  source-scanning contract test fails when any `add_function`, `add_filter`, or
  `add_test` literal under `src/stdlib/` is missing from it (OBL-3).

- Risk: characterization snapshots flake because `NetsukeManifest::vars` is a
  randomly seeded `HashMap` and `serde_json` preserves insertion order, and a
  developer re-accepts a genuine regression. Severity: high. Likelihood: high
  without canonicalization. Mitigation: canonical form for every snapshot; C12.

- Risk: adding two Fluent keys to 35 catalogues breaks the build-time key audit
  or the right-to-left test. Severity: medium. Likelihood: medium. Mitigation:
  change `keys.rs` and every catalogue in one commit; begin `ar`, `fa`, and
  `he` messages whose first token is Latin or a placeable with U+200F as
  `docs/translators-guide.md` requires.

- Risk: `src/stdlib/register.rs` (398 lines) and
  `src/manifest/jinja_macros/mod.rs` (394 lines) overflow the cap. Severity:
  medium. Likelihood: certain. Mitigation: EP-M1 extractions before feature
  work.

- Risk: the public overlay signature cannot express spy passthrough, forcing a
  breaking change in 7.1.3 or 7.3.1. Severity: medium. Likelihood: high with a
  bare `(&State, &[Value])` closure. Mitigation: overlays receive an
  `OverlayCall` context with private fields and accessors, including the
  shadowed function.

- Risk: technical-design divergence confuses later roadmap items.
  Severity: low. Likelihood: certain. Mitigation: EP-M5 updates the design in
  the same change set, and ADR-0060 records the substantive decisions.

- Risk: a peer session pushes to this branch or `main` moves.
  Severity: low. Likelihood: low. Mitigation: fetch before every push; after
  any rebase, re-check ADR numbering and rerun every gate.

- Risk: a milestone gate reports red for host load rather than for the
  candidate, and the red is either committed past or mistaken for a defect.
  Severity: medium. Likelihood: high — the Stage A baseline already hit it once
  (`cli_configuration_fixture_compiles` at the 300 s cap with the four-slot
  compiler pool fully held by peer worktrees). Mitigation: before treating a red
  `make test` as a defect, read the queue census from its log (compile-slot
  waits, sccache-unreachable notices, `build-limits:` admission failures) and
  confirm the diff against `origin/main` does not touch the failing test, its
  nextest policy, or the crate it builds. Re-run rather than repair; the
  repository deliberately keeps this budget at five periods.

## Progress

- [x] (2026-09-27) Renamed the branch to
  `7-1-2-options-carrying-manifest-loader-entry-point` and set its upstream.
- [x] (2026-09-27) Reconnaissance of the loader, query mode, stdlib
  registration, and test infrastructure.
- [x] (2026-09-27) Drafted this ExecPlan.
- [x] (2026-09-27) Expert-panel review (structure and contracts, failure modes
  and verification, alternatives and viability); all three returned "revise".
- [x] (2026-09-27) Revised the plan to address every panel finding (see the
  revision note).
- [x] (2026-09-27) User decision on the file-reading filters: extend roadmap
  7.4.1 to root them at the sandbox. Roadmap 7.4.1 and design §5.5 updated in
  the plan's pull request.
- [x] (2026-10-14) User approval of the plan and the listed deviations, given
  by explicit instruction to proceed under the `sdlc-implementation` workflow.
- [x] (2026-10-25) The user explicitly reconfirmed approval before this
  rebase; the header was set to `APPROVED` before history rewriting.
- [x] (2026-10-14) Rebase onto `origin/main` `486b876c` (64 commits ahead of the
  plan's base `ebcedaef`); rehearsed in a scratch clone first, applied with
  zero conflicts, pushed, and local/remote/tracking refs verified equal.
- [x] (2026-10-14) Plan corrections for the rebase: base commit, module paths
  after `main`'s layout migration, and ADR-041 renumbered to ADR-055 (since
  renumbered again to ADR-0060; see `Surprises & discoveries`).
- [x] (2026-10-14) Stage A baseline: the full gate set run sequentially by
  `scrutineer` against the untouched branch at `0b2a366b`. Six gates pass;
  `make test` is red from compiler-admission contention, not from this
  candidate. See `Artefacts and notes`.
- [x] (2026-10-15) EP-M0 — characterization baseline. Seven new fixtures under
  `tests/data/manifest_load_options/` (the four planned plus three recorded as
  a deviation), plus the reused `tests/data/glob.yml`; the public suite
  (`tests/manifest_load_options_tests.rs`, `tests/manifest_load_options/`) with
  12 rows over the eleven public entry points; the crate-internal rows in
  `src/manifest/tests/wrapper_equivalence.rs` and
  `src/stdlib/register/refusal_snapshots.rs`. Every snapshot reviewed by
  reading before acceptance. No production file changed: the one source edit
  outside tests is `stable_snapshot_settings` in
  `src/snapshot_test_support.rs`, which `src/lib.rs:30` gates behind
  `#[cfg(test)]`, and `src/stdlib/register.rs` gains only a `#[cfg(test)]`
  child declaration.
- [x] (2026-10-15) EP-M0 gate repair. `make check-fmt` and
  `make doc-coverage` pass at `71f26e1f`; `make lint` was **red** on a
  `clippy::shadow_reuse` in `src/manifest/tests/wrapper_equivalence.rs`, and
  the abort left every integration-test target behind it unlinted. Linting them
  revealed eight findings in the EP-M0 suites, all repaired in `fbf221f0`; the
  characterization snapshots are unchanged, which is the evidence the repair is
  a refactor and not a behaviour change. See `Surprises & discoveries`.
- [x] (2026-10-15) Diagrammed the four `StdlibRegistration` modes in technical
  design §4.3 (`04aa8366`), with the screen-reader description and caption the
  style guide requires.
- [x] (2026-10-15) EP-M0 second gate repair. `make lint` was **red** at
  `1d75efce` on two Whitaker `conditional_max_n_branches` findings that Clippy
  cannot reach, and the abort left `yamllint`, `actionlint`, and every Python
  lint stage unrun. Repaired in `9191699b` by extracting the two disjunctions
  into named predicates on `Capabilities`. `make test` also timed out on an
  admission-pool-bound row (see `Surprises & discoveries`).
- [x] (2026-10-15) EP-M0 gate re-run at `ca7ed7d4`: **eight of nine gates
  green**, including the full `lint` cascade whose Python and YAML stages the
  earlier abort had concealed. `make test` timed out on a third distinct
  nested-Cargo row, which — with the rows all belonging to one serialized group
  — is the evidence that the red is queue latency rather than the candidate.
  `doctest` is unverified, not failed. See `Artefacts and notes`.
- [x] (2026-10-15) Documentation-delta gate re-run at `f1b4b03d` and again at
  `87ff771f`, the head that carries the EP-M0 records and the corrections
  above: **all five gates green** (`check-fmt` 2 s, `markdownlint` 29 s,
  `spelling` 10 s, `nixie` 4 s, `test-workflow-contracts` 126-134 s).
  `make spelling` did not rewrite `typos.toml`. The previous batch's greens do
  not transfer to a changed head, which is why this was re-run rather than
  assumed. At `87ff771f` the logs are
  `/tmp/{check-fmt,markdownlint,spelling,nixie,test-workflow-contracts}-87ff771f.out`;
  at `f1b4b03d` the same names carry that revision's suffix. **These five do
  not cover the plan's Rust-read contracts** — see the gate-set correction
  below.
- [x] (2026-10-15) EP-M1 read-only recon completed while the candidate was
  frozen: the correction to the layout rationale (below), the Kani scope
  entries, the placement rule, and the forced-edit inventory. EP-M1 itself
  remains unstarted.
- [x] **Gate-set correction for plan edits (discharged 2026-10-31).** Running
  the cheaper documentation gates is **not sufficient** for a diff that touches
  this plan. *(2026-10-16 update: the batch at `301f2456` did run `make test`
  against this plan and was green, so the deferral below is discharged for that
  revision — but the F-1 edit re-opened the obligation, and the review-repair
  edit above re-opens it again, so the item stays unchecked until the batch
  covering the latest edit is green.)*
  `tests/execplan_status_contract_tests.rs` reads every `docs/execplans/*.md`
  at test time and asserts that the header carries exactly one line-initial
  `Status:` field whose value is in the closed set
  `DRAFT | APPROVED | IN PROGRESS | BLOCKED | COMPLETE` (and that
  `docs/documentation-style-guide.md` documents the same set). It is the sole
  Rust test that reads `docs/execplans/` from disk:
  `tests/kani_scope_wrapper_e2e_tests.rs` names this plan only in its `//!`
  header — its `open_ambient_dir` calls are `/tmp` and `/sys/fs/cgroup`. No
  Markdown gate sees any of that — form, not a Rust contract — so `make test`
  belongs in the plan-touching set. The current header satisfies both rules:
  `grep -n '^Status:'` over the whole file returns exactly one hit, the header
  field whose value is `IN PROGRESS`. The mid-document `Status: BLOCKED` inside
  the `Design: stop and set …` bullet under
  `## Tolerances (exception triggers)` is removed by two independent mechanisms
  — `header_lines()` stops at the first `##` heading, and the filter is
  `line.starts_with("Status:")`, which that bullet fails anyway because it
  opens with a list marker and the word `Design`, not with `Status:`. The
  obligation is discharged by the `61e6cf102` batch, whose `make test` was
  green at 4833 passed / 7 skipped plus 166 doctests; see `Progress`.
- [x] (2026-10-15) The four queued EP-M1 plan corrections applied. The
  placement bullet now prefers `restricted_helpers/tests.rs` and rests on the
  destination the moved file already names (`refusal_snapshots.rs:25-26`), the
  18 `mod.rs`+`tests.rs`-only directories, and the retirement of the `#[path]`
  at `register.rs:39` — **not** on file count, because an earlier draft's
  premise that no `tests/mod.rs` holds one child is false: 8 of the 17 do. Two
  further false claims were found and removed while applying them: that
  `src/manifest/tests/` holds no `mod.rs` (it does, declared by a plain
  `mod tests;` at `src/manifest/mod.rs:249`), and the escaped-backtick code
  span whose quoting made mdtablefix emit a 94-column line it then accepted as
  its own fixed point. Also recorded: EP-M0's split of
  `tests/manifest_load_options/wrapper_equivalence.rs` into
  `wrapper_equivalence/mod.rs` plus `wrapper_equivalence/path_rows.rs`, which
  the plan did not carry. The two artefact paths that named the pre-split file
  — in the OBL-1 statement and in `Stage B` — were corrected in the same edit,
  so no citation in this plan now points at a line number a later reflow can
  invalidate.
- [x] (2026-10-15) Plan-touching gate batch run against the frozen pair: **red,
  3 of 7**. Green: `check-fmt` (ruff 250 files; mdtablefix
  `183 files left unchanged`, independently confirming the reflow repair),
  `typecheck`, `doc-coverage` (65 validator tests; 98.87% against the 80% bar).
  Three independent causes, none of them the plan text itself. (1)
  `markdownlint` MD038, introduced by the plan edit — a code span written
  `` `## ` `` whose trailing space markdownlint rejects; fixed to `` `##` ``.
  (2) `lint` aborted at `lint-clippy` on `clippy::shadow_reuse` in
  `tests/manifest_load_options/fixtures/canonical.rs:86`, where the
  `StageSeam::Accepted` arm rebound `stages` over the parameter of the same
  name; renamed to `notified`, so `lint-whitaker`, `lint-python`, and
  `github-actions-lint` never ran and remain unverified. That file was last
  touched by `847c19cc` and the shadow is present verbatim in it, so the red
  **predated this candidate** and was never caused by the plan edits. (3)
  `nixie` diagram 2, the `Figure 2` flowchart `04aa8366` added, whose
  `Inject[Use the caller's env reader and clock]` label carries an apostrophe
  that mermaid's lexer reads as a string delimiter — the bare label is
  unterminated from that point. Quoted to `Inject["…"]`, matching the shape
  already used at `netsuke-test-framework-technical-design.md:86`. All three
  repaired; `make nixie`, `make markdownlint`, and a targeted
  `cargo clippy --test manifest_load_options_tests` now exit 0 (the last after
  8m20s of compile-slot waiting), and repo-wide mdtablefix still reports 183
  unchanged. `make test` was red on
  `build_module_slice_ui_tests::production_build_module_slice_has_expected_boundary`
  timing out at 300 s under a saturated host, with 16 rows never run and the
  doctest pass never reached — a skipped pass is not a pass, so both remain
  pending rather than passed.
- [x] (2026-10-16) Repairs confirmed at full scope: **six of seven gates
  green**. `check-fmt`, `typecheck`, `lint`, and `doc-coverage` exited 0, and
  `markdownlint` and `nixie` cleared the two Markdown defects. The `lint` gate
  is the one worth recording, because it went **past** `lint-clippy` for the
  first time on this branch: all four stages ran — clippy, `lint-whitaker` over
  both `netsuke-build` and `test_support`, `lint-python` (ruff, pylint at
  10.00/10, the df12 house lints, `ambrleaks`, and `interrogate` at 100%), and
  `github-actions-lint` (yamllint plus actionlint). The `shadow_reuse` string
  appears zero times in that log. Provenance held: all three frozen hashes were
  identical at the start and the end of the batch, and `git status --porcelain`
  was byte-identical at both ends. Repaired in `12e8d47c`.
- [x] (2026-10-16) The one red is the plan's own Risk `:433` case, now measured
  rather than predicted: `make test` aborted on
  `build_module_slice_ui_tests::production_build_module_slice_has_expected_boundary`
  terminating at 300.009 s, with 3927 of 3928 run passing, 6 skipped, and 16
  never run. The queue census is 227 compile-slot waits, a peak of 341 ahead,
  574 sccache-unreachable notices, and **zero** `build-limits:` admission
  failures — so admission never refused and this is queue latency. The changed
  surface touches the failing test's package but not its test file or its
  nextest policy, and the `manifest_load_options_tests` target passed 13 rows
  against 0 failures. Disposition unchanged from `:1839-1842`: **re-run, do not
  repair**, and do not widen the budget. The doctest pass is still unrun — a
  fourth abort short-circuited it — so it remains unknown, not passed.
- [x] (2026-10-16) Milestone CLI review (`coderabbit review --agent` through
  `scrutineer`) returned **three findings over 39 reviewed files**, of which
  **two are unique** — the two minor findings are one defect worded twice on
  the same lines. Both unique findings were verified against source before
  repair, and both are repaired in `8cf507c2`.
  - **major, the temporary-path filter in both of its copies**
    (`src/snapshot_test_support.rs` and
    `tests/manifest_load_options/fixtures/settings.rs`): the pattern required
    **exactly one** separator between prefix components. A successful manifest
    snapshot holds a Windows path **twice** — a diagnostic quotes it with the
    single backslashes `temp_dir()` renders, while the canonical renderer
    serializes the same path through `serde_json`, which **doubles** every
    backslash. `temp_dir().display()` returns the raw (single) form, so a
    pattern built from it could never match the serialized form, and it is the
    serialized form that carries the run-specific workspace name. The filter now
    emits a separator class matching **one or more** of either separator, and
    trims a trailing separator first so two adjacent classes do not demand two
    separators where the text has one. This is the **second** round on this
    filter: `51fb1b49` widened a literal `/` to the class `[\\/]`, which is
    "either separator" but still exactly one character — the round-one repair
    was incomplete, and its own commit message recorded a green that was not
    evidence.
  - **minor, `docs/netsuke-test-framework-technical-design.md` (duplicate):**
    the paragraph claimed `ManifestQuery` reads the process environment. It
    cannot. `register_env_function` runs before the mode match
    (`src/manifest/mod.rs:135`) and installs the real `env`;
    `register_manifest_query` then registers `env` as a **refusal** over it, and
    MiniJinja's `add_function` reaches `Arc::make_mut(..).insert(..)` — an
    overwrite — so the refusal wins. `AmbientFull` is the only mode that reads
    the process environment. That variant is planned for EP-M2 and is absent
    from `src/`, so the correction phrases it as the design's future mode rather
    than inventing implementation.
- [x] (2026-10-16) The filter repair is guarded by an oracle that can see the
  defect. `temp_dir_filter_pattern` is now a pure function of its **argument**,
  because the defect is invisible on Linux: a test that called `temp_dir()`
  could only ever guard the platform that recorded the snapshot. That
  extraction is what lets the Windows spelling be asserted on any host. The
  `rstest` carries five cases — a diagnostic Windows path with single
  backslashes, a serialized Windows path with doubled ones, a Unix path, a
  trailing separator, and a sibling directory that merely shares the prefix
  (`/tmpfoo/ws`, which must be left alone). Non-vacuity was proved by reverting
  the pattern **one clause at a time** and confirming that each revert fails
  **exactly one** row and leaves the other four passing: dropping the
  one-or-more quantifier fails only the JSON-escaped row, and removing the
  trailing-separator trim fails only the trailing-separator row. Two clauses,
  two disjoint failures — so no row is carrying the other's assertion and none
  is tautological. Round one had no such check, which is why it recorded a
  green while the defect was live. The rows use `ensure!` and return
  `anyhow::Result` rather than asserting, because `clippy::panic_in_result_fn`
  is denied workspace-wide (`Cargo.toml:223`) and an assertion inside a
  `Result`-returning test trips it.
- [x] (2026-10-16) A **third** false claim sat next to the code under repair, in
  the integration fixture's module header: it explained the filter by saying
  the loader "puts that workspace's root into the manifest it returns".
  `NetsukeManifest` has no root or path field — it carries `netsuke_version`,
  `vars`, `macros`, `rules`, `actions`, `targets`, and `defaults` — and
  `glob()` returns **relative** template paths. The filter's live justification
  is error text: a path entry point is handed an absolute path and quotes it
  back in an error. Corrected in the same commit, since it sat adjacent to the
  changed lines.
- [x] (2026-10-16) Repair batch at `8cf507c2`: **all seven gates green**, and
  the doctest pass **ran for the first time on this branch**. `check-fmt` (183
  files left unchanged), `typecheck` (`ty` clean), `lint` (all five stages
  complete, `panic_in_result_fn` appearing zero times, so the abort that opened
  this sequence is confirmed fixed), `doc-coverage` (98.87%, aggregate
  4988/5045, against the 80% bar), `markdownlint` (0 issues), and `nixie` (all
  diagrams validated). `make test` aborted on its first attempt at a single 300
  s per-test timeout — `cli_configuration_fixture_compiles`, the same
  nested-compile class as Risk `:433` — with 15 rows never run and the doctest
  pass short-circuited, and **passed on the mandated re-run**: 3954 of 3954,
  plus a doctest pass at 0 failures. That pass is 129 passed and 32 ignored
  across the two targets: `netsuke` contributed 88 passed and 26 ignored, then
  2 compile-fail rows, then `test_support` contributed 39 passed and 6 ignored.
  The announced `running 114` and `running 45` counts include the ignored rows,
  so neither is a pass count. Provenance held: HEAD `8cf507c2` and all three
  blob hashes identical at both ends of the batch, and `git status --porcelain`
  empty at both.
- [x] (2026-10-16) Second milestone CLI review (`coderabbit review --agent`
  through `scrutineer`) ran against the frozen clean tree at `cc7f0084` and
  returned **one `trivial` finding over 39 reviewed files**, with
  `outcome: completed` and no rate-limit text. The log is 3 JSONL lines — one
  `review_context`, one `finding`, one `complete`, and **zero heartbeats** — at
  `/tmp/coderabbit-7-1-2-options-carrying-manifest-loader-entry-point-cc7f0084.out`
  (sha256 `c1a18871…`, 4276 bytes). Unlike the first review, its
  `review_context` carries **no `baseCommit` field at all**, only
  `baseBranch: "main"`; the review binds through the worktree path and the
  branch, which is the reason a content edit voids it and the CLI must be
  re-run rather than the log re-read.
  - **trivial, `tests/manifest_load_options/fixtures/settings.rs:67-89`
    (`[type:docstyle]`), "Remove the duplicated temporary-path filter."** —
    asks that the pure `temp_dir_filter_pattern` be moved to a home both
    snapshot crates can reach, for example `test_support`, with each crate
    keeping its own `Settings` wiring. **Repaired**, on the remedy's own merits.
    The finding's stated reason is false and is recorded as such: it claims "the
    first repair round already missed one copy's defect", but `51fb1b49`
    changed both files and left both holding the same incomplete pattern, so
    nothing was missed. The duplication was nonetheless real and independently
    triggers the `AGENTS.md` refactoring heuristic.
  - **Why the remedy was accepted despite the false premise.** Two copies, each
    guarded only by its own copy of the same five `rstest` cases, can drift
    apart while both suites stay green — the guard travels with the copy, so
    neither copy's tests can see the other. Consolidating removes the class of
    defect instead of adding a check for it. Both copies' cases now run once,
    against one builder, in the crate both snapshot trees already depend on.
  - **Two entries in the plan's own `Constraints` were checked before the
    move.** C7 ("no new crate dependency") is not violated: `regex` is already
    in `Cargo.lock` at 1.13.1 as a dev-dependency of the root crate, so this
    adds a second **consumer** of an already-resolved crate, not a new crate.
    C10 (400-line cap) is satisfied: the new module is 128 lines, and both
    edited files **shrank** — `src/snapshot_test_support.rs` 263 to 198,
    `tests/manifest_load_options/fixtures/settings.rs` 144 to 55.
  - **Non-vacuity was established by mutation, not by inspection.** Reverting
    the shared builder's separator from the one-or-more class `[\\/]+` to the
    single-character class `[\\/]` — that is, re-introducing the round-one
    mistake exactly — fails **case 2 alone**, the JSON-escaped Windows row, with
    `expected "{\"path\": \"[TMP]\"}"`, `got "{\"path\": \"C:\\\\Users\\\\…\"}"`;
    the other four rows still pass. That signature is the same one the plan
    already records for the original defect, which is the evidence that the
    extracted oracle guards the real property rather than tautologically
    agreeing with whatever the builder emits.
- [x] (2026-10-15) `847c19cc` ("Make the characterization rows record what they
  claim") **changed 11 characterization snapshots** and added
  `budget_exhaustion.snap` — all of `tests/snapshots/manifest_load_options/`.
  This is the one commit after EP-M0's acceptance that re-accepted snapshot
  text, and it is **not a C12 breach**: C12's operative clause is "From EP-M1
  onwards", and EP-M1 is unstarted; the commit touched **zero `src/` files**
  (`git show --name-only` lists only `tests/`), so C1 is untouched and no
  production behaviour is implicated; and the rows it fixed could not have been
  meaningfully re-accepted, because three `_and_limits` rows passed
  `exhausted_budget()`, so every fixture failed before parsing and all rendered
  the identical "budget exhausted at Source (limit 8)" text — one row's own doc
  comment claimed it "still shows a successful load" while its body passed the
  exhausted budget. A row that cannot move when ordinary loading changes is not
  a baseline, and repairing it necessarily changes its output.
  - **`847c19cc` — not EP-M0's acceptance commit `c4ca78b8` — is the C12
    baseline.** C12's text now says so. The distinction is load-bearing for a
    future EP-M1 worker: diffing against `c4ca78b8` shows 13 files changed and
    445 insertions against 35 deletions, which would force a choice between
    wrongly escalating a stop condition and wrongly concluding C12 is already
    broken. Diffing against `847c19cc` is clean, and has been across the 11
    subsequent commits up to and including this one.
- [x] (2026-10-16) The milestone CodeRabbit review at `925f803e` returned one
  `minor` finding on `docs/netsuke-test-framework-technical-design.md:247`, and
  it is **valid on both of its claims** — verified against the document's own
  prose rather than accepted on the reviewer's word, then repaired.
  - First claim: `Mode -->|AmbientFull or Full| Build`, with
    `Build --> Inject["Use the caller's env reader and clock"]`, sent
    `AmbientFull` through caller injection. The screen-reader paragraph three
    lines above the fence says the opposite — "`AmbientFull` reads the process
    environment" at `:237-239` — and the Decision log at `:642` states
    "`AmbientFull` is the only mode that reads the process environment", so the
    diagram contradicted its own surrounding text.
  - Second claim: `RestrictedTest` fanned out to `Inject` **and** `Overlay` as
    two parallel edges both reaching `Expand`, which reads as either step being
    sufficient. The registration list at `:214-225` orders them — overlays are
    step 5 and expansion is step 6 — so the `Test` path is now drawn as
    `RestrictedTest → TestEnv → Overlay → Expand`.
  - Repair: the mode branch now has four arms, `AmbientFull` and `Full` are
    separate nodes, `AmbientFull` reads the process environment, and `Full` and
    `Test` each take the caller's reader and clock (still two nodes, because
    `Test` alone continues to overlay registration). The prose and the Figure 2
    caption were already correct and are unchanged; only the figure was wrong.
  - **The quoted-label trap applies to any edit here.** Two labels carry an
    apostrophe, so they are quoted — the unquoted form is what `04aa8366`
    shipped and `make nixie` caught (see the Progress entry for that commit).
    New edges were also checked for undefined and orphan nodes, and the diagram
    was parsed directly with `merman-cli` before the gate ran.
  - The edit keeps the file's own 80-column tightness: the first draft reached
    98 columns in the fence, which MD013 permits (`code_block_line_length` is
    120) but which no line in this document had used at either the base or the
    pre-edit revision. Splitting the branch arms removed the need.
  - `make nixie` passed on the repaired revision ("All diagrams validated
    successfully!") and the read-only mdtablefix probe reports the file
    unchanged. **The repair voids the `925f803e` review**, so the milestone
    review must be re-run against the new head; this entry records the finding
    as repaired, not as closed.
- [x] (2026-10-16) Gate batch and milestone review at `52992f4c`, the
  review-repair head: **nine of nine gates green**, then the re-run milestone
  review, which returned **one `minor` finding** on
  `docs/netsuke-test-framework-technical-design.md:406` — valid, and repaired
  in the next commit.
  - The nine gates ran sequentially under the exclusive gate-runner against the
    frozen revision, with the tree verified clean before and after: `check-fmt`
    2 s, `typecheck` 884 s, `lint` 2679 s (`RESULT: PASSED (minimum: 100.0%,
    actual: 100.0%)`), `doc-coverage` 771 s (98.87% against the 80% bar),
    `test` 445 s, `markdownlint` 29 s, `nixie` 1 s, `spelling` 9 s,
    `test-workflow-contracts` 144 s. Evidence: `/tmp/pr807-batchD-52992f4c/`.
  - `make test` reached green on its **third** invocation, and the first two are
    instrumentation, not candidate, failures — the runner's own chunks were
    bounded and it killed in-flight tests. Attempt 1 (`…/attempt-2/test.out`) is
    a genuine `TIMEOUT [300.015s]` on
    `production_build_module_slice_has_expected_boundary` with 18 `SIGTERM`
    rows from the chunk bound; attempt 2 is a `SIGTERM [111.879s]` on
    `packaged_manifest_retains_build_script_sources` while it was inside its own
    600 s budget, so that attempt was neither a nextest timeout nor a failure of
    the test. The brief's "re-run exactly once if it times out" did not fit this
    host: the passing run needed to be uninterrupted. **Neither red attempt is
    evidence about the candidate.**
  - **Timeout census, corrected by counting rather than recollection.** Across
    the full-suite `make test` invocations preserved for this branch, split on
    the runner's `test invocation` separators and deduplicated by content
    SHA-256: **16 distinct invocations, 8 of which ended in a nextest timeout,
    5 green, and 3 killed by the runner's own chunk bound** (instrumentation,
    not evidence about the candidate). The three timed-out tests are
    `production_build_module_slice_has_expected_boundary` (5 times, at 300 s),
    `cli_configuration_fixture_compiles` (2 times, at 300 s), and
    `packaged_manifest_retains_build_script_sources` (once, at 600 s) — all in
    the `nested-cargo-builds` group (`max-threads = 1`), so every one is a
    nested-Cargo test whose wall clock is decided by the shared compile pool.
    The earlier figure carried in the PR body draft and the ledger — "five of
    six runs, four test names" — was written from recollection and is wrong on
    both numbers. The count needs ANSI escapes stripped and an anchor on
    `TIMEOUT [` rather than the bare word, because a bare grep also matches test
    *names* containing "timeout", and `SLOW [>Ns]` lines are heartbeats that a
    **passing** run emits.
  - The passing run's `Summary` is `3949 tests run: 3949 passed (1 slow), 6
    skipped`, and **both halves of `make test` genuinely ran** — `Doc-tests
    netsuke` (114 announced, 88 passed, 26 ignored, plus 2 compile-fail rows)
    then `Doc-tests test_support` (46 announced, 40 passed, 6 ignored). The
    announced counts include the ignored rows, so neither is a pass count. This
    discharges the "doctest unverified" caveat the earlier batches carried.
  - `markdownlint` reports `Linting: 183 files` and then `Summary: 0 issues in
    0 files`. The second line is the tool's own summary format against those 183
    files, not a zero-file run — read the two lines together, because the
    summary alone looks like a vacuous pass.
  - The ExecPlan contract is confirmed *inside* the gate rather than assumed:
    `netsuke-build::execplan_status_contract_tests` ran with 10 PASS rows
    including `every_execplan_header_status_is_within_the_closed_set`. No test
    output names this plan, so "the contract passed" is the strongest available
    evidence that it read this file; the contract enumerates
    `docs/execplans/*.md` from disk.
  - **The provenance brief dispatched to the gate runner was wrong about the
    diff surface, and the runner caught it.** The brief described the candidate
    as "docs-only — exactly two files"; that describes `HEAD~1..HEAD`. Against
    the stated base
    `6a6fed3a`, `git merge-base` is `486b876c` — the branch is *not* based on
    `origin/main` — and the surface is **44 paths, 40 of them non-Markdown**
    (`src/manifest/tests/*`, `src/stdlib/register.rs`, `test_support/*`,
    `tests/manifest_load_options/*`, 15 insta snapshots). The nine-gate set was
    therefore right on scope grounds alone, independent of the ExecPlan
    argument. A base-versus-branch-tip confusion produced a false description of
    the change surface, exactly as it did for the `nixie` verdict above.
  - The re-run review covered the **whole 44-path surface**: the tool's own
    `reviewedFiles` list is set-identical to
    `git diff --name-only 6a6fed3a...52992f4c`, so nothing was skipped, and it
    warned about no sampling or truncation. Evidence:
    `/tmp/pr807-review-52992f4c/coderabbit.out` (sha256 `fa06c749…`), exit 0.
  - The finding, verified against source rather than accepted on the reviewer's
    word: `:406` read "The test registration therefore supplies sandbox-rooted
    adapters for all three groups", in the present tense, while `:417-422`
    (added by this branch) says the adapters "land in stages", that the test
    registration "refuses all three groups" until they do, and that roadmap
    7.4.1 "replaces the refusals with the sandbox-rooted adapters". The adapters
    genuinely do not exist: `StdlibRegistration` (`src/manifest/mod.rs:96`)
    declares only `Full` and `ManifestQuery`, and no sandbox-rooted adapter
    appears in `src/stdlib/`. The sentence is base text (`:367` in `6a6fed3a`,
    then "for both helpers"), but this branch edited that same line to "all
    three groups" while adding the staging paragraph, so the contradiction is
    now ours to fix. **It was also the section's only present-tense claim that
    the adapters exist** — every other adapter statement is staged (`:417`,
    `:421`, `:425`), while `:378` describes command helpers that *do* exist
    today. Repaired by "supplies" → "will therefore supply", one word.
  - Repair scope note: the finding needed no re-derivation of the design's
    substance, so no Decision-log entry is owed; the staged framing this branch
    added is unchanged and now reads consistently at both ends.
- [x] (2026-10-16) Repair batch at `301f2456` ("Share the temporary-path
  snapshot filter"): **all nine gates green**, including the two the previous
  batch could not reach. `make test` was red on **attempt 1** at the same
  host-contention class as Risk `:433` —
  `production_build_module_slice_has_expected_boundary` terminating at 300.007
  s, 3933/3949 run, 16 rows never run, doctest pass short-circuited — and
  **passed on the mandated re-run**: 3949 of 3949, 6 skipped. The doctest pass
  ran on the re-run and on neither attempt of any earlier batch:
  `Doc-tests netsuke` 88 passed and 26 ignored, plus 2 compile-fail rows, then
  `Doc-tests test_support` 40 passed and 6 ignored. The announced `running 114`
  and `running 46` include the ignored rows, so neither is a pass count.
  - `make test-workflow-contracts` was added to the dispatch **mid-batch** after
    the first assignment omitted it. The omission's reasoning — "the diff
    changes no file under `tests/workflow_contracts/`" — is invalid for this
    candidate: the diff adds a `#[cfg(test)] #[path = ...] mod` child to
    `src/stdlib/register.rs`, which `tools/kani/proof-scope.toml:161` names,
    and `kani_proof_scope_test.py` re-derives that scope from the module
    closure on every run. The gate ran last and was green (1271 passed, 3
    skipped; then 27 passed). **A gate whose subject is a derived whole-crate
    property cannot be scoped by asking which files were edited** — the same
    lesson the EP-M1 Kani note already carries.
  - The red attempt was preserved byte-for-byte under `.attempt1` before the
    re-run overwrote the canonical path, and the census confirms the class: 52
    compile-slot waits, peak 98 ahead, 574 sccache-unreachable notices, and
    **0** `build-limits:` admission failures. The zero is what makes this queue
    latency rather than an admission refusal. All five cases of the shared
    filter passed in both attempts.
- [x] (2026-10-24) Rebased onto `origin/main` `8a961000` after `main` moved 15
  commits on, replaying all 29 branch commits (`486b876c..67894369`) with two
  conflict stops, both in files where `main`'s documentation-layout migration
  (`c8e58a0a8`) met this branch's substance. `range-diff` shows 29 commits with
  exactly 2 changed, and each delta is solely the `ADR-010` → `ADR-0010`
  renumber. The semantic audit found 550 of 550 target-only paths
  byte-identical and no unexplained deletion. `Cargo.lock` needed no rebuild:
  `cargo metadata --locked --offline --no-deps` is clean against `main`'s lock
  plus this branch's single added `"regex",` edge in `test_support`.
  Post-rebase repair of old-style citations and stale figures: see the entry
  below.
- [x] (2026-10-25) Rebased the 30-commit branch range from `8a961000` onto
  fetched `origin/main` `de81d5c3` with Git’s built-in `zdiff3` backend; no
  rebase conflicts. The rebase range-diff matched all 30 replayed commits. All
  180 target-only paths are byte-identical and no branch deletion remains
  against the target. Reapplied and reconciled the pre-existing plan
  corrections.
- [x] (2026-10-30) Rebased the 32-commit range `de81d5c34..a24f4ab9` onto
  fetched `origin/main` `7929a3fc` with Git's built-in `zdiff3` backend. The
  host still selects Weave from the legacy global block in
  `~/.config/git/attributes`, so the unattended rebase bypassed it with
  `-c core.attributesFile=/dev/null`; no merge driver participated. There were
  no conflicts: `range-diff` matched all 32 replayed commits, all 201
  target-only paths are byte-identical, and the only deletion against the
  target is the intended move of `src/stdlib/register/query_helpers.rs`. All
  eight gates pass at `dc6abe26`: `check-fmt` 2.7 s, `typecheck` 9 m 33 s,
  `lint` 14 m 23 s, `doc-coverage` 98.22%, `test` 4826 passed / 7 skipped plus
  3 doctests, `markdownlint` 0 issues, `nixie` 208 diagrams, and
  `test-workflow-contracts` 1520 passed / 3 skipped. Logs:
  `/tmp/check-fmt-dc6abe26.out`, `/tmp/typecheck-dc6abe26.out`,
  `/tmp/lint-dc6abe26.out`, `/tmp/doc-coverage-dc6abe26.out`,
  `/tmp/test-dc6abe26.out`, `/tmp/markdownlint-dc6abe26.out`,
  `/tmp/nixie-dc6abe26.out`, and `/tmp/test-workflow-contracts-dc6abe26.out`.
  Force-pushed with lease from `925f803e`, and local, remote, and tracking refs
  then all read `dc6abe26`.
- [x] (2026-10-30) EP-M1's first extraction, committed as `dc6abe26`: the
  query refusals and the shared surface inventory moved to
  `src/stdlib/restricted_helpers/`, the build registration derives its
  file-test names from `SURFACE`, and the Kani scope names the two new source
  files. The eight-gate batch above is its acceptance evidence; the refusal
  text, registration shapes, and file-test names are unchanged.
- [x] (2026-10-30) EP-M1's second extraction, committed as `f6efa8513`:
  `QueryEvaluation` and `classify_query_evaluation` moved to
  `src/manifest/jinja_macros/query_classification.rs`, with the relocated
  adapter's rationale rewritten for its new home. The parent falls from 394 to
  358 lines. `tools/kani/proof-scope.toml` gains the new path, because the
  harness closure still reaches the classifier through
  `evaluate_when_expression`; `kani_proof_scope_test.py` failed on the move
  before the entry was added and passes with it. Its eight-gate batch is green
  at `f6efa8513`: `check-fmt` 4 s, `typecheck` 4 m 40 s, `lint` 19 m 7 s,
  `doc-coverage` 98.22%, `test` 4826 passed / 7 skipped plus 164 doctests,
  `markdownlint` 0 issues, `nixie` every diagram validated, and
  `test-workflow-contracts` 1520 passed / 3 skipped. Logs:
  `/tmp/check-fmt-f6efa8513.out`, `/tmp/typecheck-f6efa8513.out`,
  `/tmp/lint-f6efa8513.out`, `/tmp/doc-coverage-f6efa8513.out`,
  `/tmp/test-f6efa8513.out`, `/tmp/markdownlint-f6efa8513.out`,
  `/tmp/nixie-f6efa8513.out`, and `/tmp/test-workflow-contracts-f6efa8513.out`.
- [x] (2026-10-30) EP-M1 complete. The batch above is bound to `f6efa8513`;
  the only tracked change after that revision is this Progress record and the
  two checkbox lines, both prose and Markdown only, so the batch is not re-run.
  `make check-fmt` and `make markdownlint` are re-run on the prose commit
  instead.
- [x] (2026-10-30) EP-M1 milestone review via `coderabbit review --agent`
  against `origin/main` (CLI 0.9.1): exit 0, `review_completed`, 51 files
  reviewed, seven unique concerns, none major or critical. Six were verified
  against source and repaired in `427c175d5`; one premise was rejected because
  it claimed `uses_shell.yml` carried the same false coverage statement as
  `uses_fetch.yml`, while that file phrases the claim conditionally. The
  repairs: the `uses_fetch.yml` comment (no public row loads it, and why
  `http://` is load-bearing), two stale test comments, a `register.rs` test
  pinning inventory-to-dispatch coverage, the developers' guide's
  replaced-mechanism name, a design-document status note recording that
  §§4.2-4.3 are superseded by this plan's revision note, and two prose repairs
  in this document.
- [x] (2026-10-31) Eight-gate batch for `427c175d5`: `check-fmt` 4 s,
  `typecheck` 3 s, `lint` 97 s, `doc-coverage` 5349 s at 98.22%, `test` 3317 s
  including the new `every_inventoried_file_test_has_a_build_predicate` row,
  `nixie` 3 s, and `test-workflow-contracts` 167 s at 1520 passed / 3 skipped —
  all exit 0. `markdownlint` was the one red gate, 34 s, reporting exactly two
  MD049 issues in the new status-note sentence. Logs:
  `/tmp/check-fmt-427c175d.out`, `/tmp/typecheck-427c175d.out`,
  `/tmp/lint-427c175d.out`, `/tmp/doc-coverage-427c175d.out`,
  `/tmp/test-427c175d.out`, `/tmp/markdownlint-427c175d.out`,
  `/tmp/nixie-427c175d.out`, `/tmp/test-workflow-contracts-427c175d.out`.
- [x] (2026-10-31) The MD049 repair, `416c73560`, changes one emphasis
  character in Markdown, and `4f57ddde0` regenerates `typos.toml` from the
  shared dictionary (unrelated to this branch's work, committed separately).
  Both are Markdown/config-only deltas, so only the gates sensitive to that
  delta were re-run at `4f57ddde0`: `check-fmt` 7 s, `markdownlint` 43 s with 0
  issues, and `nixie` 3 s, all exit 0. `lint`, `test`, and `doc-coverage` are
  unaffected by prose, which is the recorded gate sensitivity rather than an
  assumption. Logs: `/tmp/check-fmt-4f57ddde0.out`,
  `/tmp/markdownlint-4f57ddde0.out`, `/tmp/nixie-4f57ddde0.out`.
- [x] (2026-10-31) EP-M2 implemented and committed as `ecfeb6c67`
  ("Add options-carrying manifest loader entry point"), 22 files changed. New:
  `src/manifest/loader_options.rs` (`ManifestLoadOptions`, `StdlibRegistration`,
  `HostHelperDisposition`, `from_str_with_options`, loader-owned
  `env()`/`glob()` registration, `query_environment()`);
  `src/manifest/tests/loader_options.rs` (the OBL-2 table); and
  `tests/manifest_load_options_embedder.rs` (the OBL-12 substitute for the
  `trybuild` fixture C7 forbids). Every `from_str*` and `from_path*` entry
  point is now a wrapper, `from_path_with_options` is the crate-private core
  deriving its registration from the opened workspace, and `evaluate_manifest`
  splits into `build_template_environment` and `expand_and_render`. The stale
  `ManifestLoadMode` citation in `docs/developers-guide.md` is corrected; EP-M5
  still owns the full narrative.
- [x] (2026-10-31) EP-M2 local verification before the gate batch:
  `make lint-clippy` exit 0, the focused suites (both wrapper-equivalence
  binaries, the OBL-2 rows, the stdlib refusal snapshots, the loader-options
  doctests) 387 passed / 1607 skipped at exit 0, and `make check-fmt` clean at
  305 Rust files and 207 Markdown files. Logs:
  `/tmp/epm2-lint-clippy-netsuke-7-1-2.out` and
  `/tmp/epm2-focused-tests-netsuke-7-1-2.out`.
- [x] (2026-10-31) OBL-1 non-vacuity discharged as the obligation requires.
  Baseline: 15 of 15 rows pass (`/tmp/epm2-obl1-baseline-netsuke-7-1-2.out`).
  Seeded fault A — the legacy Boolean formatter symbol kept live but never
  installed by `register_with_config` — failed 11 of the 15 rows and passed
  only the four that never render a Boolean: both budget-exhaustion rows,
  `path_entry_point_cannot_inject_a_clock`, and the crate-internal
  `from_path_with_limits_row`. Seeded fault B — `render_manifest_with_budget`
  routed through `RenderMode::ManifestQuery` — failed the same 11 rows and
  passed the same four. Both were reverted with `git checkout --`, every
  `*.snap.new` was deleted, and the baseline then re-ran green. Logs:
  `/tmp/epm2-obl1-fault-a-netsuke-7-1-2.out` and
  `/tmp/epm2-obl1-fault-b-netsuke-7-1-2.out`. Each fault's naive form is a
  compile error under `-D warnings`; see `Surprises & discoveries`.
- [x] (2026-10-31) Eight-gate batch for `61e6cf102`: **seven green, one red**.
  `check-fmt` 3 s, `typecheck` 1669 s, `lint` 4751 s, `doc-coverage` 2477 s at
  98.23% (6382/6497), `test` 1081 s at 4833 passed / 7 skipped plus 166
  doctests, `markdownlint` 25 s with 0 issues, `nixie` 2 s;
  `test-workflow-contracts` red at 129 s with 1 failed / 1519 passed / 3
  skipped. Logs:
  `/tmp/<gate>-netsuke-7-1-2-options-carrying-manifest-loader-entry-point-61e6cf102.out`.
  The batch ran detached, one gate at a time, and took 2 h 51 m of wall clock,
  79 m of it in `lint` behind a compile queue that peaked near 70 requests; the
  supervisor stayed healthy and no gate hit admission refusal.
- [x] (2026-10-31) The one red gate was a real contract failure, not host load:
  `test_scope_covers_the_harness_closure` reported that the Kani harness
  closure now reaches `src/manifest/loader_options.rs`, which
  `tools/kani/proof-scope.toml` did not list. Repaired in `8a7a83718` by adding
  the path in sorted position; the entry mirrors the closure rather than
  widening the proof inputs, so the pull-request Kani decision keeps firing
  exactly when a harness input changes. Re-ran `make test-workflow-contracts` at
  `8a7a83718`: exit 0, 132 s, 1520 passed / 3 skipped. Log:
  `/tmp/test-workflow-contracts-netsuke-7-1-2-options-carrying-manifest-loader-entry-point-8a7a83718.out`.
  `make test` needed no re-run: no Rust test reads `proof-scope.toml` (the
  three consumers are `scripts/kani_proof_scope.py` and the two workflow
  contract modules).
- [x] (2026-10-31) The gate-set correction recorded below is discharged: the
  batch above ran `make test` against this plan's current text at `61e6cf102`,
  and the only later plan edit is this prose record.
- [ ] (2026-10-31) EP-M2 milestone CLI review is **blocked on CodeRabbit's
  organization-wide quota**, not on the candidate. Two invocations of
  `coderabbit review --agent --committed --base origin/main` against
  `68cc2683f` (the second after the stated 51 s wait) both exited 1 with
  `errorType: rate_limit`: "You've used all 10 included reviews currently
  available", `onDemandReviewAvailable: false`. Zero files were reviewed, so
  there are no concerns to verify. Logs:
  `/tmp/epm2-coderabbit-review-68cc2683f.out` and
  `/tmp/epm2-coderabbit-review-68cc2683f-2.out`. The review will be
  re-requested once a slot frees; if it runs after EP-M3 lands it covers both
  milestones and that scope will be recorded with its result. The repository's
  own gates are green and are not affected by the quota.
- [x] (2026-10-31) Published `68cc2683f` over SSH as `leynos`; local head, the
  refreshed `origin/7-1-2-options-carrying-manifest-loader-entry-point`
  tracking ref, and the remote branch all read the same commit, and the branch
  upstream is `origin/<branch>`.
- [x] (2026-10-31) EP-M3 implemented and committed as `b5691e198` ("Add the
  test-mode stdlib registration"), 59 files changed. New:
  `src/manifest/test_surface.rs` (`TestSurface`), the
  `StdlibRegistration::Test(Box<TestSurface>)` variant and
  `ManifestLoadOptions::test`, `src/stdlib/restricted_helpers/test_mode.rs`
  (`TestModeRefusal`, `register_for_test`, the test `glob()` refusal), the
  `stdlib.test_mode.helper_unavailable` key in all 35 catalogues, and the first
  two behavioural scenarios with their steps. `register_for_test` installs only
  the inventory's available rows, refuses the rest with a typed source, and
  refuses `now()` unless `config.clock()` is injected. The loader keeps
  `env()`/`glob()` ownership: a test load answers `env()` from the surface's
  reader and refuses `glob()` through `register_glob_refusal`.
- [x] (2026-10-31) EP-M3 local verification. The focused baseline was 35
  passed / 2627 skipped at exit 0 (`/tmp/epm3-baseline-netsuke-7-1-2.out`):
  eleven `test_mode` rows, eight `restricted_helpers` rows, fourteen
  wrapper-equivalence rows, the corpus test, the fidelity property, and both
  scenarios. `cargo clippy --all-targets --all-features -- -D warnings`
  finished with zero warnings (`/tmp/epm3-clippy-all-netsuke-7-1-2.out`), and
  `make check-fmt` was clean at 305 Rust files and 207 Markdown files.
- [x] (2026-10-31) EP-M3 non-vacuity. Each seeded fault failed the row it
  targets and was reverted before the baseline re-ran green. OBL-3a: registering
  `scratch` in `src/stdlib/collections.rs` without an inventory row failed
  `every_scanned_registration_is_inventoried`, naming `scratch` in
  `src/stdlib/collections.rs` (`/tmp/epm3-obl3-fault-a-netsuke-7-1-2.out`).
  OBL-3b: deleting the `digest` row failed
  `refused_helpers_report_their_own_name` with
  `unknown filter: filter digest is unknown`
  (`/tmp/epm3-obl3-fault-b-netsuke-7-1-2.out`). OBL-4a: classifying test
  refusals as query refusals failed both `when` rows, each load succeeding with
  `conditional: true` (`/tmp/epm3-obl4-fault-a-netsuke-7-1-2.out`). OBL-4b:
  dropping the typed source failed `refusals_carry_a_typed_source` with
  `20 refusing rows lacked a typed source`
  (`/tmp/epm3-obl4-fault-b-netsuke-7-1-2.out`). OBL-5: removing the legacy
  Boolean formatter from the test registration failed the property with `true`
  against `True` (`/tmp/epm3-obl5-fault-netsuke-7-1-2.out`). OBL-6: defaulting
  the reader to the process environment failed
  `test_load_without_a_reader_reads_no_process_environment`, which printed the
  ambient `PATH` (`/tmp/epm3-obl6-fault-netsuke-7-1-2.out`). OBL-7: removing the
  `is_system()` branch failed the default-clock row, whose manifest had
  rendered an ambient timestamp (`/tmp/epm3-obl7-fault-netsuke-7-1-2.out`).
  OBL-11: registering the real `shell` filter under test failed the corpus
  aggregate, which reported no refusal from the `process` family (the plan's
  `hash` seed is inapplicable; see `Surprises & discoveries`)
  (`/tmp/epm3-obl11-fault-netsuke-7-1-2.out`).
- [x] (2026-11-01) EP-M3 lint repairs. The first batch was red on Whitaker's
  `module_max_lines`, and each repair unmasked the next offender because the
  suite stops at the first failing target: `keys.rs` (401 raw lines) split to
  `manifest_keys.rs` in `a347e9828`; `fidelity_property` (476 lines) plus a
  `bumpy_road_function` finding in `corpus_fidelity::compare` repaired in
  `57b925375`; and `test_mode_tests` (448 lines) split into
  `{mod,probe_rows,registration_scan,refusal_text}.rs` in that same commit.
  Focused evidence: 7 surface-contract unit tests and 15
  `manifest_load_options` cases passed before the batch.
- [x] (2026-11-01) EP-M3 workflow-contract repairs, `e5b1641d4`. The batch at
  `57b925375`/`8d7f0bdf0` left only `make test-workflow-contracts` red:
  `test_scope_covers_the_harness_closure` required
  `src/manifest/test_surface.rs` and
  `src/stdlib/restricted_helpers/test_mode.rs` in the Kani scope, and
  `test_repository_modules_follow_directory_layout` rejected the
  `('stdlib/restricted_helpers', 'test')` group. Both `sources` entries were
  added in sorted position and `test_mode_tests/` was renamed to
  `surface_contracts/` (see the Decision log). Re-run: 1520 passed / 3 skipped.
- [x] (2026-11-01) EP-M3 nine-gate batch at `e5b1641d4`: **all nine green** in
  16 m 45 s against one unchanged HEAD and a clean tree. check-fmt 2 s,
  typecheck 162 s, lint 201 s (aggregate, every phase verified), doc-coverage
  16 s at 98.27 %, test 133 s (4855 passed / 7 skipped, doctests included),
  markdownlint 22 s, nixie 2 s, test-workflow-contracts 138 s (1520 passed / 3
  skipped), test-package-verify 329 s. Logs `/tmp/gate<N>-…-e5b1641.out`;
  per-gate status records `/tmp/netsuke-gates-e5b1641/gate<N>.json`. The
  batch's own suite report proves the two previously-red node IDs are unmarked,
  so their absence from the `-q` dot output cannot hide a skip.
- [x] (2026-11-01) EP-M3 milestone CodeRabbit CLI review at `0070b8857`.
  `coderabbit review --agent --committed --base origin/main` (CLI 0.9.2,
  `origin/main` `da06ffb11`) returned `review_completed`, exit 0, in 572 s over
  128 files with six concerns: three minor, three trivial, none major or
  critical. The organization-wide quota had recovered (2 of 10 remaining
  afterwards). All six were verified against the current tree and repaired: two
  plan-file citations (the nextest filter named `load_options`; RFC 0007 was
  cited without its `rfc-` prefix), the stale "consume it in EP-M3" note on
  `SURFACE`, a restated `is_loader_owned` predicate in the registration scan,
  the substring-derived call-shape table in `refusal_text.rs`, and a `concat!`
  failure message whose `{build}`/`{test}` captures never interpolated. Log
  `/tmp/epm3-coderabbit-review-0070b8857.out`.
- [x] (2026-11-01) EP-M4 red stage. Added
  `src/manifest/tests/overlays/{mod,namespaces,registration}.rs` (OBL-8,
  OBL-9), scenario 3 with its steps, and the extended OBL-12 embedder fixture,
  then ran `cargo check -p netsuke-build --lib --tests --all-features`. It
  failed with `error[E0432]: unresolved import` for
  `crate::manifest::{OverlayNameKind, TemplateOverlayError}` and
  `crate::manifest::{OverlayCall, OverlayCallable, TemplateOverlays}`, plus one
  `error[E0599]: no method named with_overlays found for struct TestSurface`,
  exit 101. The shape differs from the plan's predicted
  `unresolved import netsuke::manifest::TemplateOverlays`: the tests live
  inside the crate and name `crate::` paths, and the missing builder method
  surfaced as a separate `E0599` rather than an import error. Log:
  `/tmp/epm4-check.out`; see `Surprises & discoveries`.
- [x] (2026-11-01) EP-M4 implementation. `src/manifest/test_surface.rs` became
  the directory module `src/manifest/test_surface/{mod,overlays}.rs`: `mod.rs`
  keeps `TestSurface` (now carrying `overlays` and the `with_overlays`
  builder), and `overlays.rs` owns `TemplateOverlays`, `OverlayCallable`,
  `OverlayCall`, `TemplateOverlayError`, `OverlayNameKind`, `OverlayNamespace`,
  `overlay_namespace`, validation, and `register_overlays`.
  `build_template_environment` computes the namespace before step 3, reuses it
  for the existing reserved-name check, and registers the overlays last, after
  the macros, so an overlay can replace any helper the earlier steps installed.
  `StdlibRegistration::overlays()` replaced the `#[cfg(test)]`
  `accepts_overlays` probe, and the decision table asserts it against all four
  rows. The collision key `manifest.overlay.name_collision` landed in
  `manifest_keys.rs` and all 35 catalogues in the same change.
- [x] (2026-11-01) EP-M4 green-stage compile. The first green attempt failed
  on two `error[E0277]` sites of my own making: both test closures are typed
  `Result<Value, minijinja::Error>` but used `?` on an `anyhow::Error` from
  `.context(...)`. Replaced with
  `ok_or_else(|| MiniJinjaError::new(ErrorKind::InvalidOperation, …))`, the
  form a third closure in the same file already used. The re-run finished `dev`
  in 16 m 11 s at exit 0 after a compile queue that peaked near 250 waiting
  requests. Logs: `/tmp/epm4-check.out` (red), `/tmp/epm4-check2.out` (green).
- [x] (2026-11-01) EP-M4 focused verification, all green:
  `--lib -E 'test(/manifest::tests::(loader_options|test_mode|overlays)/)'` ran
  27 passed / 2451 skipped (the twelve overlay rows are six namespace and six
  registration cases); `--test manifest_load_options_embedder` ran 2 passed,
  including the extended OBL-12 case; `-E 'test(/manifest_load_options/)'` ran
  18 passed / 4854 skipped, including
  `manifest_load_options_a_template_overlay_stands_in_for_glob_under_test`
  (scenario 3). Log: `/tmp/epm4-focused.out`.
- [x] (2026-11-01) EP-M4 non-vacuity, two seeded faults, both reverted before
  the green re-run. Fault A disabled overlay validation
  (`let _ = validate_overlay_names(…)`): 4 of 6 namespace rows failed, and the
  two "may take this name" rows still passed, which is the intended
  discrimination (`/tmp/epm4-fault-a-netsuke-7-1-2.out`). Fault B registered
  the overlays before the stdlib instead of last: initially only
  `overlay_which_decides_a_when` failed, because
  `passthrough_overlay_returns_the_shadowed_result` and
  `overlay_now_sees_the_refusing_stub` compared against, or reported a stack
  produced by, the stdlib's own answer. Both were strengthened with an
  `Arc<AtomicUsize>` dispatch counter asserted to be exactly 1; re-seeding
  fault B then failed three rows, and reverting it returned the twelve-row run
  to 12 passed (`/tmp/epm4-fault-b2-netsuke-7-1-2.out`).
  `overlay_glob_drives_a_foreach` still passes under the fault, correctly: the
  loader registers `glob()` before the overlays in both orders, so the overlay
  wins either way.
- [x] (2026-11-01) EP-M4 gate batch 1, run by the scrutineer over the
  uncommitted EP-M4 tree at `3fd0db1ce` (content hash `d53b9854b908`, 87 m 13 s
  wall clock). Seven of nine gates were green; three findings were real and
  were repaired before the batch was re-run:
  - `lint` (red, 3 findings, all in `src/manifest/test_surface/overlays.rs`):
    `type_complexity` at the stored dispatch type, `map(_).unwrap_or_default()`
    in `declared_variables`, and a `shadow_reuse` binding in
    `register_overlays`. Fixed with a `Dispatch` type alias, `map_or_default`,
    and renaming the let-else binding to `supplied`.
  - `markdownlint` (red, spelling half): `src/manifest/test_surface.rs` was
    staged as tracked while deleted on disk after the directory split. Fixed by
    staging the deletion (`git add -A`).
  - `test-workflow-contracts` (red, Kani proof-scope contract, both
    directions): the split module left `tools/kani/proof-scope.toml` naming the
    retired `src/manifest/test_surface.rs`. Retargeted to
    `src/manifest/test_surface/mod.rs` and `.../overlays.rs` in alphabetical
    position.
  Each repair was re-verified on its own rather than by waiting for a second
  full batch:
  `cargo clippy -p netsuke-build --lib --all-features -- -D warnings` exited 0
  in 3 m 29 s with no warnings (`/tmp/lint-clippy-lib-…out`);
  `make markdownlint` was clean over 207 files; and the proof-scope contract
  passed 7/7 in 14.02 s under the gate's own `uv` environment. Two initially
  alarming results were invocation artefacts, not defects:
  `cargo clippy -p netsuke --lib` fails because the package is `netsuke-build`,
  and a bare `uv run … pytest tests/workflow_contracts/…` fails at collection
  with `NameError: name 'Path' is not defined` because the module imports
  `Path` only under `typ.TYPE_CHECKING` and the Makefile's pinned `--with` set
  is what supplies it. The batch logs are
  `/tmp/gate{1..9}-netsuke-7-1-2-options-carrying-manifest-loader-entry-point-d53b9854b908.out`.
  EP-M4 was committed as `ec80c6bc1` after the repairs.
- [x] EP-M1 — extractions (restricted helpers, query classifier).
- [x] EP-M2 — options entry point and thin wrappers.
- [x] EP-M3 — `StdlibRegistration::Test`.
- [x] EP-M4 — template overlays.
- [x] (2026-11-01) EP-M5 documentation.
      `docs/netsuke-test-framework-technical-design.md`
  now describes the implemented loader: §4.2 carries the four-variant enum and
  the `SURFACE` inventory, §4.3 the options, `TestSurface`, overlay types, name
  rules, and registration order, §5.5 the full refusing inventory with the
  `which`/`expanduser` deviations, and §7 the globals-enumeration capture the
  earlier text wrongly denied. Every cited anchor was re-derived from the
  source. `docs/adrs/adr-0070-deny-by-default-test-mode-stdlib.md` was minted
  and indexed in `docs/contents.md`; ADR-0050 gained Addendum E for the
  telemetry scope of derived loads; the developers' guide gained a "Manifest
  loader entry points" subsection; `docs/polonius.md` records
  `ManifestLoadOptions<'a>`; `docs/netsuke-design.md` names the four load
  modes; and roadmap 7.1.2 is ticked with all five of its bullets.
- [x] EP-M5 — documentation, ADR, and roadmap closure.
- [x] (2026-11-01) EP-M4 lint repair and second full gate batch. The first
  batch's `clippy::map_unwrap_or` repair used `Option::map_or_default`, which
  the repository's pinned nightly accepts but Whitaker rejects: Whitaker
  compiles the crate on its own `nightly-2026-05-28`, where the method is still
  unstable (E0658). `make lint` was therefore **red** at `0e23c1c04` on a tree
  Clippy had passed. `declared_variables` now uses the let-else form its sibling
  `declared_macros` already uses, which satisfies the lint without the
  unstable method. Two missing spaces in EP-M5's own documentation
  (`StdlibRegistration::records_expansion_and_budget_telemetry`,
  `ManifestLoadOptions::deferred`) were repaired in the same commit,
  `85d71d9b1`, after `make check-fmt`, `make lint-clippy` and
  `make lint-whitaker` each exited 0 on that exact content. The nine-gate set
  was then re-run by the scrutineer against the clean tree at `85d71d9b1` (tree
  `72e02a6d`) and **all nine exited 0**: `check-fmt` 4 s, `typecheck` 1 264 s,
  `lint` 184 s, `doc-coverage` 1 195 s, `test` 1 475 s, `markdownlint` 28 s,
  `nixie` 4 s, `test-workflow-contracts` 148 s, `test-package-verify` 403 s;
  batch wall clock 78 m 27 s (08:35:23–09:53:50). `make test` ran 4 869 nextest
  cases (4 869 passed, 7 skipped) and then every doctest target;
  `make test-workflow-contracts` ran 1 520 passed / 3 skipped and a further 27
  passed. Every gate log carries the same `HEAD` and tree breadcrumbs, so no
  gate read a tree another gate had moved. Logs:
  `/tmp/gate{1..9}-netsuke-7-1-2-options-carrying-manifest-loader-entry-point-85d71d9b1.out`.

## Surprises & discoveries

- Observation: Whitaker's `module_max_lines` stops the run at the first failing
  target, so each repair unmasked the next offender and EP-M3 needed three
  consecutive rounds: `keys.rs` at 401 raw lines, then `fidelity_property` at
  476 beside a `bumpy_road_function` finding in `corpus_fidelity::compare`, then
  `test_mode_tests` at 448. Impact: one red report understates the work;
  budget a serial loop until `make lint-whitaker` exits 0 rather than a single
  repair. The house layout is `foo/mod.rs` plus a plain `mod child;`; `#[path]`
  children are the retired pattern and `clippy::self_named_module_files` denies
  the alternative. Date/Author: 2026-11-01, implementation agent.

- Observation: `rust_module_layout_test.py` accepts a shared-prefix sibling
  group only when the collision is coincidental, which is exactly what an
  exception's rationale must argue. `test_mode.rs` beside `test_mode_tests/`
  collides because the directory tests that file, so no truthful rationale
  existed. Impact: the repair is a rename, not an allowlist entry; the module
  is now `surface_contracts/`, declared `#[cfg(test)] mod surface_contracts;`.
  Date/Author: 2026-11-01, implementation agent.

- Observation: `kani_proof_scope_test.py` recomputes the harness closure from
  the sources each proof reaches, so adding loader sources silently widens the
  required scope. EP-M3's `test_surface.rs` and `test_mode.rs` were reachable
  and unlisted. Impact: a milestone that adds a module reachable from a Kani
  harness must add its `sources` entry, in sorted position, in the same commit.
  Date/Author: 2026-11-01, implementation agent.

- Observation: a gate batch can outlive the tool call that started it, and on
  this host it did. The first dispatch of the `427c175d5` batch stalled at the
  runner's 600 s no-progress watchdog while `make lint` waited behind a compile
  queue several hundred deep; the watchdog terminated the runner and every gate
  child with it, leaving `check-fmt` green, `typecheck` and `lint` truncated
  mid-build. Impact: a batch this long cannot be run as one supervised tool
  call under host saturation. The rerun was launched detached (`setsid nohup`,
  one gate at a time, per-gate `tee` logs) and polled, which survived the same
  queue and took 2 h 28 m of wall clock, 1 h 29 m of it in `doc-coverage`
  alone. Evidence: the stalled `typecheck` log ends mid `Checking` line and the
  stalled `lint` log mid compile-slot wait; the detached batch's logs each end
  with `EXIT_CODE`. Date/Author: 2026-10-31, implementation agent.

- Observation: the planned Kani-scope helper command was misidentified. In
  this checkout, `make test-kani-scope-wrapper` exercises the systemd-run
  wrapper; it neither derives proof-source paths nor prints additions and
  removals. `tests/workflow_contracts/kani_proof_scope_test.py` recomputes the
  harness closure and checks `tools/kani/proof-scope.toml`; validate scope
  edits with `make test-workflow-contracts`. Correct the earlier instruction to
  derive moved source paths from the contract's actual failure evidence rather
  than relying on the unrelated wrapper test.

- Observation: a gate report's "pre-existing base-red" verdict is only as good
  as the revision it compared against, and this batch's `nixie` verdict named
  the wrong one. The scrutineer called diagram 2 "byte-identical to base
  `c16a2ab1`" and advised "leave the `nixie` diagram-2 failure out of that fix,
  as it is pre-existing base-red and needs its own decision". `c16a2ab1` is the
  **tip of this branch**, not the rebase base, so "identical to base" was
  measured against a revision that already contained the defect. Against the
  real base (`486b876c`) the design doc holds exactly one mermaid fence
  (`:83`); diagram 2 is absent, and `git log -S` attributes it to `04aa8366`
  "Diagram the loader's registration modes" — an EP-M0 commit on this branch.
  Impact: had the advice been followed, a green `nixie` would have been
  reported while `ci.yml:261` runs `make nixie`, leaving the branch unmergeable
  on a defect the plan had by then recorded as not its own. The repair is one
  pair of quotes, proven by minimal experiment: `A[it's fine]` fails while
  `A["… it's fine"]` parses. Evidence: `git show 486b876c:<doc>` fence count,
  `git log --reverse -S 'Register query helpers and refusals'`, `merman-cli` on
  handwritten probes. Date/Author: 2026-10-16, implementation agent.

- Observation: the EP-M1 moves touch `tools/kani/proof-scope.toml`, a file the
  plan never mentions. Its `sources` list names
  `src/stdlib/register/query_helpers.rs:162` and `src/stdlib/register.rs:161`;
  the contract at `tests/workflow_contracts/kani_proof_scope_test.py` rejects
  both a closure file missing from the scope (`:126-135`) and a scope entry
  covering nothing (`:180-189`), and both rules are live in the file. Impact:
  relocating `query_helpers.rs` *provably* forces an edit in the same commit —
  the file ceases to exist, so its entry becomes dead and the new path, if the
  harnesses reach it, is uncovered. The `jinja_macros` extraction is weaker:
  `mod.rs` survives it and stays reached, so only the new child's coverage is
  in question, and the contract decides that. Do not pre-emptively edit that
  entry. Evidence: `wyvern` recon at `c4ca78b8`, both contract rules read and
  confirmed live. Date/Author: 2026-10-15, implementation agent.

- Observation: `glob` is a query refusal but absent from the EP-M0 refusal
  inventory, so the OBL-10 baseline is incomplete. The stub file registers six
  always-disabled helpers (`env`, `glob`, `fetch`, `shell`, `grep`,
  `contents`), seven file tests, and ten host-dependent helpers — 23 rows
  against the table's 22. `glob` is the omission, and it is also the one
  refusal the `RestrictedSurface` inventory must eventually describe as a
  *host* helper rather than a stub, since EP-M2 moves `env`/`glob` registration
  into the loader. Impact: an incomplete oracle for a requirement that says
  "every query refusal message"; the row must be added before EP-M1 relocates
  the table. Evidence: `src/stdlib/register/query_helpers.rs:30-33` against
  `src/stdlib/register/refusal_snapshots.rs` at `c4ca78b8`. Date/Author:
  2026-10-15, implementation agent.

- Observation: `FILE_TESTS` is shared across the EP-M1 move boundary.
  `src/stdlib/register/query_helpers.rs:16` imports it to build the file-test
  stubs, and `src/stdlib/register.rs:252-264` consumes it to register the real
  tests. It is currently a private `const` in the parent, so the move forces a
  visibility change or subsumption by the new `SURFACE` inventory — which is
  the point of `SURFACE`: one list, so a test added to one surface cannot stay
  silently unregistered on the other. Evidence: `wyvern` recon at `c4ca78b8`.
  Date/Author: 2026-10-15, implementation agent.

- Observation: `make lint`'s reported error count is a **lower bound**, and the
  EP-M0 gate run proved it again in a new way. `make lint` at `71f26e1f`
  aborted on one `clippy::shadow_reuse` in
  `src/manifest/tests/wrapper_equivalence.rs`. A focused
  `cargo clippy -p netsuke-build --lib --tests` then found **eight** findings,
  seven of them in the `tests/` integration crate. `cargo clippy` was walking
  targets in order and stopped at the failing lib-test target, so the
  integration targets behind it were never linted at all — not clean,
  unvisited. Impact: do not treat a green `make lint` *stage* as coverage of
  every target unless the whole gate is green, and expect repeat lint cycles
  after any change that adds a test target. Evidence:
  `/tmp/clippy-focused-<branch>.out` (the eight findings, exit 2) versus
  `/tmp/clippy-focused2-<branch>.out` (clean afterwards, exit 0); repaired in
  `fbf221f0`. The pre-fix `make lint` log is **not** available as evidence: the
  next gate run reused the same `tee` filename and overwrote it. A log named
  only by gate and branch is a reused label, not a permanent record — copy a
  log worth citing to a name that carries its revision. Date/Author:
  2026-10-15, implementation agent.

- Observation: Whitaker's `conditional_max_n_branches` counts **leaf Boolean
  operands**, not `&&`/`||` operators, and Clippy has no equivalent rule, so
  `clippy` stays green on a construct only `lint-whitaker` rejects. The form
  `hazards.reads_environment && !(seams.environment || refusing_helpers)` is
  three leaves against a ceiling of two, even though it reads as a single
  conjunction. Impact: a Clippy-clean target is not a Whitaker-clean target,
  and the 1977-second `lint-whitaker` stage is the only place this surfaces —
  after the whole Clippy stage has run. Its remedy is stated at
  `docs/whitaker-users-guide.md:407`: extract the sub-predicate. Doing so is
  also a readability win here, since it names *why* a refusing stub answers a
  hazard. Evidence: `/tmp/lint-whitaker-failure-1d75efce.out` (two errors at
  `fixture.rs:156` and `:161`) versus `/tmp/whitaker-focused-<branch>.out`
  (clean, exit 0); repaired in `9191699b`. Date/Author: 2026-10-15,
  implementation agent.

- Observation: `make test` at `1d75efce` failed on nextest's own 300-second cap,
  not on an assertion:
  `command_env_ui_tests::cli_configuration_fixture_compiles` ran
  `TERMINATING [>300.000s]` / `TIMEOUT [300.008s]`, leaving 15 tests unrun and
  `doctest` **never executed**. The same log holds 107
  `waiting for a compile slot` lines, and `.config/nextest.toml:57-70`
  documents this exact row as bound to the compiler-admission pool. The red is
  therefore evidence about host load, not about this candidate — and this is
  the third distinct identity to trip the cap across EP-M0's runs. Impact: the
  timeout is not actionable in the candidate; re-run on a quieter host for the
  doctest evidence rather than raising the limit. Note also that `test-nextest`
  running before `doctest` means a timeout leaves the doctest suite
  *unverified*, not merely failing. Date/Author: 2026-10-15, implementation
  agent.

- Observation: the strongest evidence that a wall-clock gate red belongs to the
  host rather than the candidate is **role inversion between sibling tests**
  (three different rows timed out across three runs), but **the weaker version
  of that claim is the true one**, and the difference matters. All three
  victims sit in `.config/nextest.toml`'s `nested-cargo-builds` group, which
  spawns child Cargo builds and is serialized, so its members queue behind one
  another: `cli_configuration_fixture_compiles` timed out twice, then
  `production_build_module_slice_has_expected_boundary` — which had *passed* at
  168.114 s in the Stage A baseline — timed out instead. **What the third run
  does *not* show is that the earlier victim passed.** It does not appear in
  the log because `test-nextest` fails fast and left 16 rows unscheduled; a
  passing row always prints (`PASS` rows, 3926, equal the reported passed
  count, 3926). So the correct statement is "the set of victims moved", not
  "the two siblings swapped outcomes". Even the weaker form is
  load-attributable — a candidate defect cannot change *which* member of a
  serialized group reaches a fixed wall-clock bound — but it is a claim about
  scheduling order, and it must not be inflated into a claim about the other
  row's verdict. Impact: do not read "absent from the log" as "passed"; check
  whether the runner failed fast first. Two further corollaries: a `TIMEOUT`
  row is not a `FAIL` row (no assertion failed, but unscheduled rows exist),
  and the doctest half never runs, leaving it *unverified* rather than failed.
  Evidence: `/tmp/test-FAIL-ca7ed7d4.out`; the census read 256 compile-slot
  waits with a queue depth of 240 ahead and 1108 sccache-unreachable notices,
  and zero admission failures. Date/Author: 2026-10-15, implementation agent.

- Observation: **a plan can cite the wrong mechanism and still reach the right
  decision**, which is the harder version of a stale citation to catch because
  nothing downstream contradicts it. EP-M1's plan asserted that the flat
  `restricted_helpers.rs` form "fails `prefixed_files_beside_directory()`" and
  that this forced the directory form. Running both functions of
  `tests/workflow_contracts/rust_module_layout_test.py` against synthetic trees
  shows it does not: `prefixed_files_beside_directory` globs
  `f"{directory.name}_*.rs"`, so it catches an underscore-prefixed *sibling*
  (`restricted_helpers_tests.rs`) and never a bare `restricted_helpers.rs`
  beside `restricted_helpers/`. A flat file plus a `#[path]`-declared child
  passes both functions — as the live `register.rs` + `register/` pair does
  today. What actually forbids the flat form is prose:
  `docs/repository-layout.md:409-419`. Impact: cite the rule rather than the
  function, and when a plan names a specific contract function, run that
  function — it is a twenty-line pure-Python port with no dependencies, and
  `pytest` is unavailable outside the uv environment, so importing the module
  directly is not an option. Evidence: a verbatim port of both functions run
  against three synthetic trees (directory-module, flat-plus-`#[path]`-child,
  flat-plus-sibling) and validated against the live tree, where both checks
  report zero violations — matching the contract's own green run. Reproduce the
  port from `rust_module_layout_test.py:23-84`; it needs only `pathlib`,
  `itertools`, and `collections`. Date/Author: 2026-10-15, implementation agent.
- Observation: the technical design's line citations are stale, and the loader
  has grown since the design was written: `ManifestEnvironment`,
  `ManifestBudgetLimits`, the manifest root, a private `ManifestParse` bundle,
  and a second private mode enum, `ManifestLoadMode`, all exist. Evidence:
  `src/manifest/mod.rs`, `src/manifest/query.rs` at `486b876c`. Impact:
  `ManifestLoadOptions` carries the environment bundle and budget limits, and
  `ManifestLoadMode` is replaced (C5).

- Observation: `realpath`, `size`, `contents`, `linecount`, `hash`, and
  `digest` read the ambient filesystem relative to the process working
  directory, not through `StdlibConfig`'s workspace root. Design §5.5 names only
  `glob()` and the file tests as ambient bypasses, and roadmap 7.4.1 provides
  sandbox adapters only for those two. Evidence:
  `src/stdlib/path/filters.rs::register_filters`,
  `src/stdlib/path/path_utils.rs::current_dir_utf8`. Impact: test mode refuses
  all six (C4). Because UX design §8.6 makes filters unmockable in the first
  version, a manifest using them could never be tested. Resolved by the user:
  roadmap 7.4.1 now roots these filters at the sandbox (see `Decision log`).

- Observation: manifest-query registration does not install the legacy Boolean
  formatter that `register_with_config` installs. Evidence:
  `src/stdlib/register.rs`. Impact: test registration installs it, or Boolean
  interpolation would differ from a build and break I4.

- Observation: MiniJinja 2.24.0 `add_function` is `add_global` with a function
  value, `add_global` is a map insert, and functions share one namespace with
  globals. `Environment::globals()` and `Value::call(&State, &[Value])` are
  public, so an overlay *can* capture and call the function it replaces. Design
  §7's statement that MiniJinja "offers no such retrieval" is wrong for
  functions (filters and tests remain unlistable). Evidence:
  `minijinja-2.24.0/src/environment.rs` (`add_function`, `add_global`,
  `globals`, `pub(crate) fn get_filter`), `minijinja-2.24.0/src/value/mod.rs`
  (`Value::call`). Impact: overlays receive the shadowed function now (spy
  passthrough), and the surface inventory must be source-scanned rather than
  read from the environment. EP-M5 corrects design §7.

- Observation: a query refusal inside `when` does not drop an entry; it keeps
  it with `conditional: true`, and the classifier runs in every mode. Evidence:
  `src/manifest/expand/mod.rs` (`WhenResolution::Conditional`),
  `src/manifest/jinja_macros/mod.rs::classify_query_evaluation`. Impact: a
  misclassified test refusal would silently change the manifest; hence the
  query-only gate.

- Observation: loader telemetry other than expansion reports and budget
  exhaustion (template-render and macro metrics, the `env()` lookup counter)
  fires in every mode, including query mode. Evidence:
  `src/manifest/jinja_macros/telemetry.rs`, `src/manifest/env/reader.rs`.
  Impact: "telemetry-free" was too broad; the predicate is named for exactly
  what it gates.

- Observation: `WallClock::is_system` is documented as "recorded for
  diagnostics", and `with_clock(system_clock())` yields `is_system == false`.
  Evidence: `src/stdlib/time/clock/mod.rs`. Impact: test mode treats any clock
  supplied through `with_clock` as explicit injection (C4); the field's
  documentation is updated to say it is a policy input.

- Observation: the base had gone stale between planning and implementation. The
  plan was written against `ebcedaef`; `origin/main` had advanced to
  `486b876c`, 64 commits ahead, and the merge base was `ebcedaef` itself, so
  every one of those 64 commits was invisible to the plan's own review rounds.
  Evidence: `git rev-list --count HEAD..origin/main` = 64;
  `git merge-base HEAD origin/main` = `ebcedaef`. Impact: the rebase was
  rehearsed in a scratch clone before the worktree was touched (free, no
  working-tree risk), and it applied with zero conflicts because this branch's
  own diff is three Markdown files only.

- Observation: the module-layout migration landed on `main` and moved the very
  seams EP-M1 names. Evidence:
  `git diff --stat ebcedaef..486b876c -- src/stdlib/ src/manifest/` shows 160
  files changed. Reconciled as follows. `src/stdlib/register.rs` fell from 398
  to 315 lines because `src/stdlib/register/query_helpers.rs` (new, 151 lines)
  already holds `manifest_query_operation_error`,
  `MANIFEST_QUERY_DISABLED_HELPER_MARKER`, `is_manifest_query_disabled_error`,
  and the stub registrations — EP-M1's first extraction is **partly done
  upstream**, under a different path than the plan's
  `src/stdlib/restricted_helpers/mod.rs`. Under `src/manifest/`,
  `budget_adapter.rs` became `budget/adapter.rs`, `env_reader.rs` became
  `env/reader.rs`, `env_policy/` became `access_policy/`, `expand_tests.rs`
  became `expand/tests/`, `render.rs` became `render/mod.rs`, and `loading.rs`
  and `workspace.rs` were added. Impact: the plan's paths in Table 1,
  `Plan of work`, and `Concrete steps` were corrected to the post-migration
  names; the EP-M1 outcome now says "relocate the existing stub cluster", not
  "create it".

- Observation: the same migration landed a layout contract, and **three of the
  plan's new module paths violate it**. Evidence:
  `tests/workflow_contracts/rust_module_layout_test.py` groups siblings by the
  first `_`-separated token of the module name
  (`module_name.partition("_")[0]`) and treats a directory as a module only
  when it holds `mod.rs`; a group of two or more siblings is an
  `unrecorded prefix group`, and `DISPARATE_PREFIX_EXCEPTIONS` is `{}`. Probed
  against this tree with the contract's own functions:
  `src/stdlib/register_query_snapshots.rs` forms `('stdlib', 'register')` with
  `register.rs`; `src/stdlib/restricted_helpers.rs` plus
  `restricted_helpers_tests.rs` forms `('stdlib', 'restricted')`; and
  `src/manifest/load_options.rs` forms `('manifest', 'load')` with the
  pre-existing `load_stage.rs`. Impact: the three paths were renamed to
  `src/stdlib/register/refusal_snapshots.rs`,
  `src/stdlib/restricted_helpers/{mod.rs,tests.rs}`, and
  `src/manifest/loader_options.rs`; each was re-probed clean. The rename is a
  placement decision, not a scope one: every artefact, row, and milestone keeps
  its identity, and EP-M1's move of the snapshot rows into the new module is
  unchanged. Three further citations named a module by its bare stem where the
  module is a directory with a `mod.rs`, so they were corrected to the
  `/mod.rs` form: `src/manifest/render/`, `src/manifest/tests/budget/`, and
  `src/stdlib/time/clock/`. Two more were wrong for unrelated reasons:
  `src/ninja_gen_property_tests/ninja_oracle.rs` is really
  `src/ninja_gen/property_tests/ninja_oracle.rs` (the same migration), and
  `src/value/mod.rs` is a path inside the `minijinja` crate, not this one, and
  now says so.

- Observation: **ADR-041 is already spent**, and the reservation has since
  moved twice. Evidence: `origin/main` publishes
  `docs/adrs/adr-0041-canonical-recipe-shell-quoting-surface.md`. At the
  2026-10-14 rebase a sweep of every remote branch put the ceiling at **054**
  (held by `docs/crate-decomposition-programme`), and a free number inside the
  bracket was rejected because 044–053 were unclaimed on `main` yet bracketed
  by open allocations; the plan reserved **ADR-055** on that basis. The
  2026-10-24 rebase re-swept and found the number **bracketed after all**:
  `main` had merged `adr-0056-manifest-linting-under-netsuke-check.md`, leaving
  0055 between a merged 0056 and the still-open 0052–0054 allocations, and the
  same rule that rejected 044–053 now rejects 0055. Impact: renumbered to
  **ADR-0060**, above the highest allocation anywhere (`adr-0059-…` on
  `module-slice-timeout-audit`), per `docs/documentation-style-guide.md`'s
  no-renumbering-after-publication rule. Both sweeps must span the *four-digit*
  `docs/adrs/adr-nnnn-` layout as well as the legacy `docs/adr-nnn-` one: a
  `docs/adr-[0-9]{3}-` pattern silently misses every migrated record, and did
  so on the first pass here.

- Observation: the Stage A baseline's `make test` is red, and the cause is
  compiler-admission contention on this host rather than the candidate.
  `cli_configuration_fixture_compiles` hit the 300 s `terminate-after` cap
  while dozens of `build-limits-rustc` processes from peer worktrees held a
  four-slot pool, and its sibling nested-Cargo fixture passed at 168 s in the
  same run. Evidence: the baseline table in `Artefacts and notes`; the
  repository already documents this test's timeout as queue-bound and refuses
  to widen it. Impact: the doctest half of `make test` is unverified at the
  baseline and is re-run later. This is a standing hazard for every milestone
  gate in this plan, not a one-off: the same test can be expected to time out
  again while the host is loaded, so a red `make test` needs the queue census
  (slot waits, sccache notices, admission failures) read before it is treated
  as a defect.

- Observation: the plan's governing documents moved substantially on `main`.
  Evidence: `git diff --stat ebcedaef..486b876c` gives `developers-guide.md`
  2862 changed lines, `roadmap.md` 293, `netsuke-design.md` 284, `contents.md`
  59, `polonius.md` 32, `adr-008` 32, the technical design 27, `adr-018` 8, RFC
  0007 9. Impact: every EP-M5 anchor is re-read from the rebased tree before
  editing, and every patch is applied by quoted context rather than by line
  number. The rebase preserved both intents — roadmap 7.4.1 keeps this plan's
  file-reading-filter extension *and* `main`'s new 7.6 items; technical design
  §5.5 keeps the file-reading-filter paragraph.

- Observation: a filter that rewrites a path inside snapshot text must be
  written against the **serialized** text, not against the path as the
  operating system spells it, and "match either separator" is not the same
  property as "match the escaped form". Evidence: reverting `separator` to
  `[\\/]` fails exactly the JSON-escaped row while every plain row still
  passes, so the two properties are independently observable rather than one
  standing in for the other. Impact: the same trap applies to any future filter
  that redacts a path reaching a snapshot through `serde_json` — the doubled
  form, not the raw form, is what decides whether a run-specific name survives
  into a checked-in file. A sweep for the class confirms these were the only
  two path-rewriting snapshot filters in the tree; every other `add_filter`
  call is a MiniJinja template filter, a different API on a different object.
  Those two copies were separate functions — `src/snapshot_test_support.rs`
  serves `src/snapshots/` and
  `tests/manifest_load_options/fixtures/settings.rs` serves
  `tests/snapshots/` — and they are now one shared builder, per the decision
  recorded below. Likewise the `ManifestQuery` claim was swept: it is named in
  10 places across five documents outside this plan — `developers-guide.md`,
  `roadmap.md`, ADR-0050 (then numbered 018), RFC 0007, and the technical
  design — and the corrected paragraph was the only site asserting the process
  environment.

- Observation: the review's two minor findings were one defect reported twice,
  and a raw finding count therefore overstated the work by a third. Evidence:
  both cite the same file, the same line range, and the same remedy, differing
  only in wording and in the suggested diff. Impact: a finding count must be
  de-duplicated by (file, lines, defect) before it is quoted as a measure of
  how much was wrong, or the record implies work that no one did.

- Observation: a review finding can name a real defect while stating a false
  reason for it, and the reason is checkable on its own. Evidence: the second
  milestone's review filed one `trivial` finding asking that the duplicated
  temporary-path filter be consolidated into `test_support`, justified by "the
  first repair round already missed one copy's defect". That premise is false:
  `git show --stat 51fb1b49` records **both** files changed by that commit, and
  both blobs at that revision carry the identical
  `settings.add_filter(&format!(r#"{prefix}[\\/][^\s"']*"#), "[TMP]");`. Round
  one did not miss a copy; it applied the same incomplete pattern to both,
  which is exactly why "either separator" failed identically on each. Impact:
  the premise and the remedy are separate verdicts. Here the remedy was
  independently sound — `AGENTS.md` lists duplicated code as a refactoring
  trigger, and two copies each guarded only by their own copy of the same cases
  can drift apart while both stay green — so the finding was repaired on its
  own merits with the false premise recorded rather than inherited. A
  reviewer's stated *reason* being wrong is not by itself a reason to reject
  the *request*; the two are verified separately, against source.

- Observation: the scrutineer's closing recommendation — add a `slow-timeout`
  override for `cli_configuration_fixture_compiles` — is one the repository has
  already considered and refused, in the very file it would be written in.
  `.config/nextest.toml:51-70` grants that test a group slot and then argues
  that its budget stays at the profile's five periods because the child build
  is awaited inside the test body, so the queue wait counts against the
  allowance: a wider budget "would cover the delay, and would not remove the
  contention that causes it". The same comment records the measured spread
  (171.4 s quiet, 709.2 s on a cold tree, terminated at the cap on a contended
  one) and the ordering that caps any per-test allowance at 720 s. Impact: the
  recommendation is declined on the file's own evidence, and the standing
  disposition stays **re-run, do not repair**. This is worth recording because
  the suggestion will recur every time this test times out, and re-deriving the
  refusal each time is wasted work.

- Observation: the canonical gate-log path names the **branch**, not the run, so
  a later local gate run silently destroys the earlier batch's evidence at the
  same path. Evidence: after the seven-gate batch at `8cf507c2`, repairing two
  Markdown defects and re-running `check-fmt`, `markdownlint`, `typecheck`,
  `lint`, and `nixie` with the prescribed `tee` template overwrote all five
  logs in place; the per-gate durations the batch had reported then had no
  surviving log to cite. The fresh runs prove the exit codes and the counts
  quoted above, but they do not prove the durations, so the durations were
  removed rather than quoted from a narrative alone. Impact: a batch's numeric
  detail is only durable if its logs are copied aside before any re-run, since
  the template in `AGENTS.md` deliberately isolates work per branch rather than
  per revision. Two defences, either sufficient: copy the batch's `.out` and
  `.exit` files to a revision-suffixed name immediately on receipt, or quote in
  the plan only figures the surviving logs still show.

- Observation: `ManifestLoadOptions<'a>` is **invariant** in `'a`, because it
  stores `Option<&'a mut dyn FnMut(ManifestLoadStage)>` and `&mut` is invariant
  in its referent. Consequence: a constructor cannot return a value whose
  lifetime outlives its own borrow unless it is generic in that lifetime, so
  `manifest_query` is
  `fn manifest_query<'options>() -> ManifestLoadOptions<'options>` rather than
  returning a `'static`-flavoured value, and `ManifestParse` gained a second
  lifetime (`<'a, 'name>`) so the manifest-name borrow is not forced to match
  the options borrow. Evidence: the natural signature failed to compile before
  the split. Impact: an options struct carrying a borrowed observer is
  invariant by construction; plan for two lifetimes at the parse boundary
  rather than reaching for a clone or a boxed closure.

- Observation: an OBL-1 seeded fault that removes the only use of a helper is
  observed as a **compile error**, not as failing rows. Evidence: deleting the
  `register_legacy_boolean_formatter(env)` call made `-D dead-code` reject the
  build with "function `register_legacy_boolean_formatter` is never used" (exit
  101), and routing every full render through `RenderMode::ManifestQuery` did
  the same to `RenderMode::Full`. Each fault was then reseeded so the item
  stayed used — `let _ = register_legacy_boolean_formatter;` and a temporary
  `let _ = RenderMode::Full;` rider — which produced the intended 11 failing
  rows. Impact: a seeded fault must keep its subject live, or the run proves
  only that the compiler notices dead code. A first attempt that exits 101 is
  not evidence about the rows and must be redesigned, not quoted.

- Observation: the OBL-12 obligation asked for a `tests/ui` fixture compiled by
  `trybuild`, which C7 forbids as a new dependency. The substitute is an
  integration test in `tests/` that imports the public entry point from outside
  the crate and loads a manifest. Evidence:
  `tests/manifest_load_options_embedder.rs` passes and would fail to compile if
  the surface were not `pub`. Limitation: it proves the surface is reachable
  and compiles for an external crate; it cannot prove that some *other* item
  stayed private. That negative is covered by the conformance check against the
  plan's item list, not by this test.

- Observation: the OBL-2 table's sixth column, "may carry overlays", has no
  production consumer until EP-M4, so a non-`cfg` accessor would be dead code
  under `-D warnings`. It is therefore `#[cfg(test)]` for now. Impact: EP-M4
  must remove the gate when the overlay decision is consumed; until then the
  table still asserts a stated answer for every variant, which is the property
  OBL-2 exists to protect.

- Observation: both public constructors' doctests load a manifest against the
  ambient workspace, so they run in `make test`'s doctest pass rather than
  being illustrative only. Impact: a doctest added to this module is a
  behavioural check with a real working-directory dependency; it must avoid
  `env()`, `glob()`, and any helper whose result depends on where the test runs.

- Observation: adding a new module can fail a gate that never mentions the
  module's subject. Evidence: `src/manifest/loader_options.rs` is loaded by the
  manifest loader, so the Kani harness closure reached it, and
  `test_scope_covers_the_harness_closure` — a workflow-contract test, not a
  Rust test — failed at `61e6cf102` until the path was added to
  `tools/kani/proof-scope.toml`. The same class was already recorded for
  EP-M1's relocated classifier. Impact: "which gate covers this file?" is the
  wrong question for a derived whole-graph property. Ask which whole-repository
  inventories the new file enters — harness closure, surface inventory, module
  line-count caps, compiled-source roots — and run those gates even when the
  file's own subject is unrelated to them.

- Observation: the plan's item list and OBL-11 disagree about
  `TestModeRefusal`'s visibility. The `Interfaces and dependencies` section
  lists it among the crate-internal additions, but OBL-11's artefact
  `tests/manifest_load_options/corpus_fidelity.rs` is an external integration
  test that must downcast the error source and call `operation()`. Evidence:
  the type is re-exported as `netsuke::stdlib::TestModeRefusal`, and the corpus
  test compiles and passes against it. Impact: a public error type is now part
  of the surface, and EP-M5 must reconcile it in the design. The alternative —
  classifying refusals by marker text alone — would have made the test unable
  to distinguish a refusal from any other error carrying the same text, which
  is exactly what OBL-4 exists to prevent.

- Observation: the OBL-3 and OBL-4 rows could not share the planned
  `src/stdlib/restricted_helpers/tests.rs` artefact. Evidence: that file was
  already close to the 400-line cap, and the inventory scan, reachable-probe,
  typed-source, and locale rows add roughly 380 lines. They live in
  `src/stdlib/restricted_helpers/surface_contracts/`, declared beside
  `mod tests;`. Impact: the plan's artefact path names are corrected by this
  entry; the coverage is unchanged.

- Observation: there is no "clock absent" state to observe through the public
  loader, so OBL-7's default-clock row measures the injected-clock branch
  instead. Evidence: `StdlibConfig::from_current_dir()` always supplies a
  clock, so a test load built from it carries the system clock;
  `test_load_refuses_now_with_the_default_clock` therefore loads through the
  default configuration and requires `now()` to be refused. The seeded fault
  that drops the `is_system()` branch renders an ambient timestamp into the
  manifest, which is the evidence the row measures the branch rather than the
  helper's absence. Impact: the row proves "the ambient clock is refused", not
  "a clockless load is refused"; the latter is not constructible.

- Observation: OBL-11's stated non-vacuity seed is inapplicable to this
  corpus. The plan asks for `register_for_test` to register the real `hash`
  filter and for the corpus run to fail on the file using it, but no corpus
  manifest evaluates `hash`. Evidence: grepping `tests/data/` for `hash` finds
  no manifest use, and a probe run under the fault reported no new failure.
  Substitute: register the real `shell` filter, which is the Process family's
  only refusal in this corpus, so the aggregate fails with
  `no corpus manifest refused from these families: ["process"]`. Impact: the
  family aggregates are only as strong as the corpus's coverage of each family;
  the substitution is decisive here, but a future family with no corpus user
  needs its own fixture rather than the documented seed.

- Observation: as with EP-M2, an OBL-3/4/6/7 seeded fault written naively is a
  **compile error** under `-D warnings`, not a failing row. Evidence: removing
  `disabled_env_reader`'s only import failed the build, and deleting the
  refusal loop's only callers produced `dead_code` errors for
  `register_filter_refusal` and `register_test_refusal`. Each fault was
  reshaped to keep its subject live — the refusal loop stayed and the query
  registration was appended after it, and the reader fault called
  `super::process_env_reader()` instead of importing the default. Impact: a
  first attempt that exits 101 says nothing about the rows and must be
  redesigned, matching the EP-M2 finding.

- Observation: a seeded registration-order fault exposed two vacuous EP-M4
  rows that the green run could not. With the overlays registered before the
  stdlib, `passthrough_overlay_returns_the_shadowed_result` still passed,
  because the stdlib's own `timedelta` produced the same rendered command the
  comparison expected, and `overlay_now_sees_the_refusing_stub` still found a
  `now` refusal in the error chain, because the stub itself had answered.
  Impact: an equality-or-error assertion against a helper that exists without
  the overlay cannot distinguish "the overlay ran" from "the overlay never
  ran". Repaired by making each closure bump an `Arc<AtomicUsize>` and
  asserting exactly one dispatch; the re-seeded fault then failed three rows.
  Date/Author: 2026-11-01, implementation agent in EP-M4.

- Observation: the EP-M4 red transcript does not match the plan's predicted
  error text. The plan expected
  `error[E0432]: unresolved import netsuke::manifest::TemplateOverlays`; the
  run produced three `E0432` lines naming `crate::manifest::…` paths and one
  `E0599` for the missing `TestSurface::with_overlays`. Impact: the red
  transcript is evidence of absence, not a fixed string; a future plan should
  predict the crate-internal path shape and treat a missing builder as its own
  failure rather than part of the import error. Date/Author: 2026-11-01,
  implementation agent in EP-M4.

- Observation: a lint repair can pass every gate on the pinned toolchain and
  still fail the Whitaker stage, because Whitaker builds the crate with its own
  older nightly. Impact: `clippy::map_unwrap_or` and its "obvious" replacement
  are not interchangeable here — `Option::map_or_default` is unstable below the
  pin. Evidence: `make lint` exited 2 at `0e23c1c04` with
  `error[E0658]: use of unstable library feature 'result_option_map_or_default'`
  at `src/manifest/test_surface/overlays.rs:273`, while `make lint-clippy` was
  green on the same bytes. Action: use the let-else form the neighbouring
  function already used, and keep the comment that records why. Date/Author:
  2026-11-01, implementation agent in EP-M4.

- Observation: the branch no longer merges cleanly into `origin/main`
  (`e5d57c58`). Both sides split the same localization key table: this branch
  moved `MANIFEST_*` into `src/localization/manifest_keys.rs`, while `main`
  moved `CLAP_ERROR_*` into `clap_error_keys.rs` and added a hand-assembled
  `ALL_KEYS` union in `src/localization/mod.rs`. Impact: PR #807 reports
  `mergeable: CONFLICTING` and needs a rebase whose `mod.rs` merge must be
  audited by hand, because a clean merge would leave `ALL_KEYS` covering three
  tables while four exist. `tests/build_l10n_keys_tests.rs` and
  `tests/build_l10n_audit_tests.rs` are the oracles for that resolution.
  Date/Author: 2026-11-01, implementation agent in EP-M4.

## Decision log

- Decision: `TestModeRefusal` and its `operation()` accessor are public,
  re-exported as `netsuke::stdlib::TestModeRefusal`. Rationale: OBL-11 is an
  external integration test and must name the type; the plan's crate-internal
  listing was written before that artefact was fixed. EP-M5 records the type in
  the design's surface. Date/Author: 2026-10-31, implementation agent in EP-M3.

- Decision: the EP-M3 inventory and refusal rows live in
  `src/stdlib/restricted_helpers/surface_contracts/`, not in the planned
  `tests.rs`. Rationale: the existing `tests.rs` is near the 400-line cap, and
  the new rows are a coherent unit about the test surface rather than more
  query-refusal cases. Date/Author: 2026-10-31, implementation agent in EP-M3.

- Decision: those rows live in
  `src/stdlib/restricted_helpers/surface_contracts/`, a directory module, not
  in a `test_mode_tests/` directory. Rationale: the module-layout contract
  rejects a shared `test` prefix between `test_mode.rs` and a sibling directory
  unless the collision is coincidental, and this one is not — the directory
  tests that file. Renaming keeps both the contract and its docstring truthful,
  where an exception entry would have had to argue a coincidence the code
  contradicts. The declaration is `#[cfg(test)]`, so no production path moves.
  Date/Author: 2026-11-01, implementation agent in EP-M3.

- Decision: OBL-11's non-vacuity seed is the real `shell` filter rather than
  the documented `hash` filter. Rationale: no corpus manifest uses `hash`, so
  the documented seed cannot make the aggregate fail; `shell` is the Process
  family's only refusal and does. Date/Author: 2026-10-31, implementation agent
  in EP-M3.

- Decision: `register_for_test` decides `now()` from
  `config.clock().is_system()` rather than from a separate flag on
  `TestSurface`. Rationale: "was a clock injected" is already expressible in
  the configuration's clock provider, and a second source of truth could
  disagree with the one the template actually reads. Date/Author: 2026-10-31,
  implementation agent in EP-M3.

- Decision: `StdlibRegistration` stays a crate-private enum, extended to
  `AmbientFull`, `Full(Box<StdlibConfig>)`, `ManifestQuery`, and
  `Test(Box<TestSurface>)`. The public surface is `ManifestLoadOptions` with
  constructors `full(config, environment)` and `test(surface)`; crate-private
  `ambient(environment)` and `manifest_query()` serve the wrappers. Rationale:
  the roadmap asks to extend the existing enum, not to publish it. Keeping it
  private avoids exposing `ManifestQuery` (crate-internal today) and freezing
  variant payloads before roadmap 7.4.1 settles sandbox needs. The lazy ambient
  default becomes a named variant instead of `None`, so every mode is listed in
  one enum (C5). There is no public `Default`: a default silently selecting the
  ambient stdlib and process environment would be a trap. Date/Author:
  2026-09-27, planning agent after panel review.

- Decision (Deviation from design §4.3): overlays live in the test surface,
  not in `ManifestLoadOptions`, and `Test` carries `Box<TestSurface>`
  (`StdlibConfig`, optional `EnvReader`, `TemplateOverlays`) instead of
  `Box<StdlibConfig>`. Rationale: an overlay is a capability grant (a `fetch`
  overlay stands in for the network), so it belongs with the mode that decides
  capabilities (C5). The build path then cannot carry overlays at all, which
  makes the loader-level half of I7 hold by construction. The design's
  "ordinary parameter, not a test-only `cfg`" requirement still holds (C9).
  Roadmap 7.1.3's spike runs under a test surface, which loses nothing because
  it pins MiniJinja behaviour. Alternative considered: the design's options
  field, accepted in every mode; rejected because it lets a caller overlay a
  build and splits capability decisions across two places. Date/Author:
  2026-09-27, planning agent after panel review.

- Decision (Deviation from design §4.3): `ManifestLoadOptions` has private
  fields and constructors, carries a `ManifestEnvironment` (reader plus access
  policy) instead of `Option<EnvReader>`, and carries `ManifestBudgetLimits`.
  Rationale: the codebase bundles reader and policy so no caller injects one
  without the other; every production load is budgeted (ADR-0050); private
  fields let 7.1.3 add macro substitutions without breaking callers. For a test
  load, the reader lives in the `TestSurface` (owned `EnvReader`, default
  `disabled_env_reader`, policy `EnvAccessPolicy::default()`), so query options
  have no environment to contradict. Date/Author: 2026-09-27, planning agent
  after panel review.

- Decision: `env()` and `glob()` are registered once, at step 3, by the manifest
  loader, according to `StdlibRegistration`: real for `AmbientFull` and `Full`;
  refusing with the query text for `ManifestQuery`; for `Test`, `env()` is real
  over the surface's reader and `glob()` refuses. The stdlib's restricted
  registrations stop registering `env` and `glob`. Rationale: today the loader
  registers them and query registration overwrites them, splitting one decision
  across two modules (C5). The query stub text is unchanged (C2). Date/Author:
  2026-09-27, planning agent after panel review.

- Decision: test mode's surface is deny-by-default, built from an inventory.
  Real: lexical path filters, collection filters, `timedelta`, the legacy
  Boolean formatter, `env()` over the surface's reader, and `now()` when the
  config's clock was supplied through `with_clock`. Refusing: `glob`, `fetch`,
  `shell`, `grep`, `contents`, `realpath`, `expanduser`, `size`, `linecount`,
  `hash`, `digest`, `which` (function and filter), `command_available`, every
  entry of `FILE_TESTS`, and `now()` with the default system clock. Refusing
  stubs accept any arguments, so the widest real call shape still reaches the
  refusal. Rationale: UX design §1 promises "no network, no wall-clock
  dependency, no mutation of the project root"; design C4 makes an unmocked
  impure call an error. The functions among these (`glob`, `which`, `fetch`,
  `command_available`, `now`) remain replaceable by overlays; filters and tests
  are not mockable in the first version (UX §8.6). Roadmap 7.4.1 later replaces
  the `glob`, file-test, and file-reading filter stubs with sandbox-rooted
  adapters. Recorded in ADR-0060. Date/Author: 2026-09-27, planning agent.

- Decision (Deviation from design §5.5): `which` and `expanduser` refuse under
  test, although §5.5's table implies they run against an empty `PATH` and a
  missing home. Of `StdlibConfig`'s settings, test mode honours only the clock
  in 7.1.2; the workspace root becomes relevant when 7.4.1 roots `glob`, the
  file tests, and the file-reading filters at it. Rationale: `which` with an
  empty `PATH` yields a misleading "not found" rather than a refusal pointing
  at doubles, and `expanduser` exposes a host-shaped answer. Refusal keeps the
  rule "unmocked impure call is an error" uniform. Date/Author: 2026-09-27,
  planning agent after panel review.

- Decision: test-mode refusals are MiniJinja `InvalidOperation` errors whose
  message is the Fluent key `stdlib.test_mode.helper_unavailable` (argument
  `operation`), beginning with the verbatim code
  `[netsuke::jinja::test_mode::unavailable]` in every catalogue, and whose
  source is a typed `TestModeRefusal { operation }`. Code recognizes them by
  downcasting the source, as `src/manifest/budget/adapter.rs` does for budget
  exhaustion. The query marker and its substring check are unchanged.
  Rationale: a translated marker cannot be matched by substring in 34 of 35
  locales (C6); 7.3.2 and 7.5.5 need the operation name as data. Date/Author:
  2026-09-27, planning agent after panel review.

- Decision: `classify_query_evaluation` runs only when the load is a
  `ManifestQuery`; the flag reaches expansion from
  `StdlibRegistration::classifies_query_refusals`. The classifier first moves to
  `src/manifest/jinja_macros/query_classification.rs` (EP-M1). Rationale:
  makes the silent-`conditional` failure structurally impossible outside query
  mode, whatever a future stub's message says. Date/Author: 2026-09-27,
  planning agent after panel review.

- Decision: overlays are registered after manifest macros and before
  `expand_foreach_with_budget`. Each overlay receives an `OverlayCall` exposing
  the MiniJinja `State`, the positional and keyword arguments, and the shadowed
  function (`Option<Value>`, captured from `Environment::globals()` at
  registration). Names are validated first, against one namespace computed
  before step 3: overlays may not use `env` (the environment seam is the
  surface's reader, which keeps `EnvAccessPolicy` in force), any name beginning
  `__netsuke_` (an overlay named `__netsuke_manifest_macro_imports` would erase
  every macro import), a manifest `vars` key, or a manifest macro name.
  Validation is skipped entirely when there are no overlays, so no other mode's
  error order moves. A rejected overlay raises a typed
  `TemplateOverlayError::NameCollision { name, kind }` with localized text under
  `manifest.overlay.name_collision` and the code
  `[netsuke::manifest::overlay::collision]`. Rationale: design §4.3 order; the
  namespace computed up front is reused by 7.1.3, whose macro substitutions are
  a separate field and a separate mechanism (design §5.4), so rejecting
  function overlays on macro names does not block its fallback. A typed error
  lets 7.5.5 tell a test-suite mistake from a subject-manifest failure.
  Date/Author: 2026-09-27, planning agent after panel review.

- Decision: `TemplateOverlays` ships with the `functions` map only;
  `macro_substitutions` is added by roadmap 7.1.3. Rationale: a macro
  substitution must also rewrite the macro-import prelude (design §5.4), which
  7.1.3 spikes. Private fields make the addition non-breaking. Date/Author:
  2026-09-27, planning agent.

- Decision: the public options entry is string-based:
  `from_str_with_options(yaml, name, options)`. Path loads go through a
  crate-private
  `query.rs::from_path_with_options(path, registration_for, options)`, where
  `registration_for` is a closure turning the opened workspace into a
  `StdlibRegistration`. `ManifestLoadMode` is deleted. Rationale: a path load's
  `StdlibConfig` is derived from the workspace it opens, so a caller cannot
  supply it up front; a closure keeps one list of modes (C5). Keeping the path
  entry crate-private, beside `query.rs` as design §4.2 and §13 place it,
  serves 7.5.3 (which validates subject paths before `open_manifest_workspace`)
  without publishing an ambient-path API. The string entry leaves the manifest
  root unset, so a `Full` load's `glob()` base is the process directory exactly
  as `from_str` behaves today; the `Test` glob base arrives with 7.4.1's
  sandbox adapter, taken from the surface's `StdlibConfig` workspace root.
  Date/Author: 2026-09-27, planning agent after panel review.

- Decision: the predicate gating expansion-report metrics and the
  budget-exhaustion metric is `records_expansion_and_budget_telemetry`,
  `pub(crate)`, false for `ManifestQuery` and `Test`. Other loader telemetry
  continues to fire in every mode. Recorded as ADR-0050 Addendum E. Rationale:
  those counters are described as "normal manifest loading" metrics; counting
  test runs would distort usage evidence. The narrower name says exactly what
  is gated. Date/Author: 2026-09-27, planning agent after panel review.

- Decision: test mode renders with `RenderMode::Full`.
  Rationale: a test must see exactly what a build renders (I4). Date/Author:
  2026-09-27, planning agent.

- Decision: the differential corpus reuses `tests/data/*.yml` (for example
  `glob.yml`, `jinja_env.yml`, `jinja_is.yml`) and adds only the fixtures no
  existing file covers, under `tests/data/manifest_load_options/`:
  `boolean_interpolation.yml`, `clock_now.yml`, `uses_fetch.yml`, and
  `uses_shell.yml`. The harness walks a directory at run time, so 7.1.4 can
  point it at `examples/`. Rationale: panel scope review; roadmap 7.1.4 owns the
  `examples/` run and its RFC evidence. Date/Author: 2026-09-27, planning
  agent after panel review.

- Deviation: three further fixtures join the corpus, taking the new set to
  seven. `pure.yml` is the baseline an ordinary load is read against;
  `malformed.yml` pins the parse-failure path, which no existing fixture
  exercises through the *loader* rather than through a schema check; and
  `reads_env.yml` pins the injected-reader plumbing, which no existing
  `jinja_env*.yml` fixture does because those are shaped for helper-level tests
  rather than for a loader that supplies its own reader. Rationale: the corpus
  is what the equivalence oracle compares, so a path with no fixture is a path
  the refactor can change unobserved — the three additions close paths the
  original four left open. `uses_shell.yml` is used only where the surface
  refuses `shell`, so no row executes a command. Date/Author: 2026-10-15,
  implementation agent under the `sdlc-implementation` workflow.

- Decision: invariant I7 is discharged here at the loader level: the build
  cannot carry overlays, and every wrapper's canonical output is unchanged. The
  "manifest with a `tests` block" half depends on roadmap 7.2.1's schema field
  and is deferred there. Date/Author: 2026-09-27, planning agent.

- Decision: `ortho_config` is not used, and the users' guide is not changed.
  Rationale: no command-line flag, configuration key, environment variable, or
  user-visible behaviour changes; `netsuke test` wiring (roadmap 7.6) is where
  `ortho_config` layering and localized help apply. The developers' guide gains
  the loader-entry convention instead. Date/Author: 2026-09-27, planning agent.

- Decision: a new ADR, ADR-0060, records the deny-by-default test surface and
  the refusal-code contract; telemetry goes to ADR-0050 Addendum E; the options
  shape goes to design §4.3 only. Rationale: the surface decision is hard to
  reverse and spans network, process, filesystem, and clock, which ADR-0008
  (environment seams) does not cover. At the time of writing ADR-0039 and
  ADR-0040 were claimed on unmerged branches; both have since merged, and the
  allocation has been re-swept at each rebase. Re-check at every rebase.
  Date/Author: 2026-09-27, planning agent after panel review.

- Decision: the six file-reading filters (`contents`, `size`, `linecount`,
  `hash`, `digest`, `realpath`) refuse under test in 7.1.2, and roadmap 7.4.1
  is extended to replace those stubs with sandbox-rooted adapters, alongside the
  `glob()` and file-test adapters it already owned. Roadmap 7.4.1 and
  technical design §5.5 were updated in this plan's pull request. Rationale:
  filters cannot be doubled in the first version (UX §8.6), so without sandbox
  adapters a manifest that reads or hashes a file could never be tested. The
  alternatives, a new 7.4.x item or permanent refusal, were rejected. The 7.1.2
  implementation is unchanged by the decision: these filters refuse until 7.4.1
  lands. A related follow-on remains out of scope here (C2): query mode still
  runs `FILE_TESTS` against the real filesystem, and the query stubs for `hash`,
  `digest`, and `contents` reject keyword arguments before refusing.
  Date/Author: 2026-09-27, decided by the user; recorded by the planning agent.

- Decision: the temporary-path snapshot pattern moves to
  `test_support::temp_path_filter` and both snapshot trees bind it, rather than
  each keeping a private copy. Rationale: the two copies sat in crates that
  cannot reach each other — one is `#[cfg(test)]` code inside the library, the
  other is a separate integration crate — so an edit to either copy could not
  be seen by the other copy's tests, and both suites would stay green while the
  patterns diverged. Siting the pattern in `test_support` also lets the
  extracted rows run once, in a crate both trees already depend on, instead of
  the same five cases existing twice. **Scope and re-use policy:** the shared
  unit owns the *pattern* only; each crate keeps its own `insta::Settings` and
  its own `add_temp_dir_filter`, because snapshot directory and module-prefix
  handling legitimately differ between the trees. Any future snapshot filter
  that rewrites a path builds its pattern here and does not restate it. The
  policy is recorded in `docs/developers-guide.md` under *The temporary-path
  snapshot filter*. Rationale against the alternatives: a drift-detecting
  contract test would have to read both copies as text and compare them, which
  adds a check to preserve a duplication rather than removing it, and a sweep
  confirmed these were the only two path-rewriting snapshot filters in the tree
  (every other `add_filter` is a MiniJinja template filter, a different API on
  a different object). Constraint check: C7 (no new crate dependency) holds —
  `regex` was already resolved in `Cargo.lock` as a dev-dependency of the root
  crate, so the change adds a second consumer, not a crate. This followed a
  review finding whose stated reason was false (see `Surprises & discoveries`);
  the decision stands on the duplication, not on that reason. Date/Author:
  2026-10-16, decided by the implementation agent as the disposition of a
  review finding; recorded here for the approver.

- Decision: `ManifestLoadOptions` has **no public `Default`**. The public
  constructors are `full(config, environment)` and the crate-private `ambient`,
  `manifest_query`, and `deferred`. Rationale: a default would have to pick an
  ambient stdlib root, an ambient environment reader, and a registration, so an
  embedder that wrote `..Default::default()` would silently inherit the process
  state this milestone exists to make explicit. Omitting the impl makes that
  mistake a compile error instead of a review note. Date/Author: 2026-10-31,
  implementation agent in EP-M2.

- Decision: the loader owns `env()` and `glob()` registration, driven by
  `StdlibRegistration`'s dispositions, while the refusal *shapes* still come
  from `restricted_helpers`. Rationale: whether a load may read the host is a
  property of the load, not of the library surface (C5), so leaving these two
  registrations inside the stdlib entry points would reintroduce a second mode
  decision; reusing the inventoried stubs keeps every query refusal message
  byte-identical (C2). Date/Author: 2026-10-31, implementation agent in EP-M2.

- Decision: `ManifestLoadOptions::deferred` exists as a placeholder whose
  registration is always replaced by `from_path_with_options`'s
  `registration_for` closure, and its doc comment says so. Rationale: the
  alternative, an `Option<StdlibRegistration>` field, would push an unreachable
  `None` arm into the loader core for a value the core never observes. The cost
  is one builder that must not be used directly, which the doc comment makes
  explicit. Date/Author: 2026-10-31, implementation agent in EP-M2.

- Decision: query loads share one process-wide `disabled_env_reader` behind a
  `OnceLock` instead of constructing a reader per load. Rationale: the reader
  is stateless, the query registration installs refusal stubs so it is never
  consulted, and a `'static` reader lets `manifest_query` be generic in its
  options lifetime (see `Surprises & discoveries` on invariance). Date/Author:
  2026-10-31, implementation agent in EP-M2.

- Decision: `StdlibRegistration::accepts_overlays` is `#[cfg(test)]` until
  EP-M4 consumes it. Rationale: the decision must be stated per variant now, so
  that adding a variant without an overlay answer fails the table, but an
  accessor with no production caller is dead code under the repository's
  `-D warnings` policy. EP-M4 removes the gate when the overlay machinery needs
  the value. Date/Author: 2026-10-31, implementation agent in EP-M2.

- Decision: OBL-12 is discharged by an external-crate integration test rather
  than a `tests/ui` fixture with `trybuild`, because C7 forbids a new
  dependency. Rationale and limitation are recorded in
  `Surprises & discoveries`; the compile reachability is proven, the privacy
  negative is covered by the conformance check. Date/Author: 2026-10-31,
  implementation agent in EP-M2.

- Decision: the one stale `ManifestLoadMode::ManifestQuery` citation in
  `docs/developers-guide.md` was corrected inside EP-M2 rather than deferred to
  EP-M5. Rationale: this milestone deletes the type, so leaving a live guide
  naming it would make EP-M2's own documentation false on merge. EP-M5 still
  owns the section's full narrative and the design/ADR reconciliation.
  Date/Author: 2026-10-31, implementation agent in EP-M2.

- Decision: `TemplateOverlayError` derives `Debug` and hand-writes `Display`
  instead of using `thiserror`. Rationale: the message must carry the localized
  `manifest.overlay.name_collision` text with `name` and `kind` interpolated
  through `LocalizedMessage::with_arg`, which `thiserror`'s attribute cannot
  express; the same reason the repository's other localized errors implement
  `Display` by hand. Date/Author: 2026-11-01, implementation agent in EP-M4.

- Decision: `OverlayNameKind` renders hyphenated, untranslated tokens
  (`environment-seam`, `loader-internal`, `manifest-variable`,
  `manifest-macro`) into the collision message. Rationale: the token names the
  namespace, not the prose around it, so translating it per catalogue would
  make a diagnostic harder to match; only the surrounding sentence is
  localized. Date/Author: 2026-11-01, implementation agent in EP-M4.

- Decision: overlays reach the loader through `StdlibRegistration::overlays()`
  and a `.cloned()` taken before `register_stdlib` consumes the registration.
  Rationale: `register_stdlib` takes the registration by value, so the
  capability must be read first; the clone copies an `Arc` handle per callable
  and the name map, not the dispatch closures, which matches the Polonius-era
  policy of stating a clone's cost rather than hiding it. A second capability
  field on the registration enum would duplicate the mode that already decides
  the grant. Date/Author: 2026-11-01, implementation agent in EP-M4.

- Decision: `src/manifest/test_surface.rs` became the directory module
  `src/manifest/test_surface/{mod,overlays}.rs`, and the overlay tests became
  `src/manifest/tests/overlays/{mod,namespaces,registration}.rs`. Rationale:
  the combined module exceeded the 400-line cap and the house layout is
  `foo/mod.rs` plus plain `mod child;`; splitting by name-validation versus
  dispatch keeps each file a single concern. Date/Author: 2026-11-01,
  implementation agent in EP-M4.

- Decision (Deviation from Stage D's stated filename): EP-M5 mints
  `docs/adrs/adr-0070-deny-by-default-test-mode-stdlib.md`; the plan's
  `adr-0060-…` serial is stale. Rationale:
  `docs/adrs/adr-0060-readable-inline-recipe-progress.md` already exists on
  this branch and on `origin/main`, so reusing `0060` would create the
  duplicate-serial conflation the repository's ADR conventions forbid. A sweep
  of all 83 `origin` heads found `adr-0069` the highest serial in use and
  `adr-0070` free on every head. The deviation changes only the filename: the
  decision, its scope, and its index entry in `docs/contents.md` are as
  approved. Date/Author: 2026-11-01, implementation agent in EP-M4.

## Outcomes & retrospective

Delivered in full. The loader now has one options-carrying entry point,
`from_str_with_options`, over a crate-private four-variant
`StdlibRegistration`; the pre-existing entry points are thin wrappers. A
`TestSurface` grants a deny-by-default stdlib whose availability is decided by
one `SURFACE` inventory, refuses withheld helpers with a localized diagnostic
carrying a typed `TestModeRefusal` source, and lets a test replace functions —
including `glob` — with `TemplateOverlays`, whose name validation and
registration order are pinned by tests. The build path is unchanged, and the
differential property over generated pure manifests holds between `Full` and
`Test`.

Evidence: EP-M0's characterization snapshots, EP-M1's byte-identical
extractions, EP-M2's wrapper and entry-point suites, EP-M3's `Test` mode and
differential property, EP-M4's overlay namespace and registration tests with
two seeded faults proving non-vacuity, and EP-M5's documentation and roadmap
closure. Every milestone ended with the full gate set run by `scrutineer`; the
EP-M4 batch's three findings were repaired and re-verified (see `Progress`).

Retrospective:

- The plan's ADR serial was stale. Checking the target namespace before
  claiming a number — the serial `0060` was already in use on `main` — cost one
  sweep and prevented a duplicate-serial conflation. Recorded as a deviation,
  and EP-M5 minted `0070`.
- A gate batch is cheap to start and expensive to repeat. After the batch
  reported three findings, repairing and re-verifying each one directly was far
  faster than another full cycle, and identified two alarming results as
  invocation artefacts rather than defects.
- The design's claim that MiniJinja offers no way to retrieve a registered
  function was wrong. Enumerating `Environment::globals` reaches it, which is
  what makes a passthrough overlay possible; the documentation now says so and
  roadmap 7.1.3 can build macro substitution on the same capture.

Remaining work is outside this item: macro substitution (7.1.3), sandbox-rooted
adapters for `glob`, the file tests, and the file-reading filters (7.4.1), and
the runner slices that consume the surface.

## Conformance basis

Upstream artefacts, at `main` commit `486b876c`:

- RFC 0007, `docs/rfcs/rfc-0007-netsukefile-testing-framework.md` — scope.
- Technical design, `docs/netsuke-test-framework-technical-design.md` (status
  Draft): TD-4.2 (extend `StdlibRegistration`; reuse the refusal mechanism and
  `disabled_env_reader`), TD-4.3 (options, overlays, registration order steps
  1–6, unconditional compilation), TD-5.2 (clock owned by `StdlibConfig`),
  TD-5.5 (refusing stubs under test), TD-7 (spy passthrough premise), TD-I4,
  TD-I5, TD-I7.
- UX design, `docs/netsuke-test-framework-ux-design.md`: UX-1 (deterministic
  by default), UX-8.6 (what can be mocked).
- Roadmap item 7.1.2, `docs/roadmap.md`: RM-a (options and overlays, wrappers),
  RM-b (`Test` mode), RM-c (reuse the diagnostic shape and
  `disabled_env_reader`), RM-d (registration order), RM-e (differential tests
  for I4 and I7).
- ADR-0008 (environment seams), ADR-0010 (glob capability scoping),
  ADR-0050 (bounded template evaluation and query telemetry suppression).
- Governing standards: `AGENTS.md`, `docs/documentation-style-guide.md`.

There is no terms-of-reference document for phase 7; RFC 0007 plays that role.
Proposed deviations from TD-4.3, TD-5.5, and TD-7 are recorded in
`Decision log` and require the approver's acceptance.

Trace chains:

```plaintext
RM-a, TD-4.3        -> EP-M2 -> OBL-1 (wrapper equivalence), OBL-2 (mode table)
RM-b, TD-4.2, UX-1  -> EP-M3 -> OBL-3 (inventory), OBL-4 (refusals), OBL-6 (env), OBL-7 (clock)
RM-c, TD-4.2        -> EP-M1, EP-M3 -> OBL-4, OBL-6, OBL-10 (query unchanged)
RM-d, TD-4.3, TD-7  -> EP-M4 -> OBL-8 (names), OBL-9 (order and passthrough)
RM-e, TD-I4         -> EP-M3 -> OBL-5 (fidelity property), OBL-11 (corpus)
RM-e, TD-I7         -> EP-M2, EP-M4 -> OBL-1, OBL-2 (build cannot carry overlays)
TD-I5 (part)        -> EP-M3 -> OBL-4, OBL-6, OBL-7
```

TD-I5 is advanced, not discharged: roadmap 7.5.1 owns the end-to-end I5 tests
through pipeline actions.

## Verification plan

### Axioms (assumed, not verified)

- AXIOM-1. MiniJinja 2.24.0 replaces a global of the same name on
  `add_function`/`add_global` (source-verified; OBL-9 exercises it).
- AXIOM-2. MiniJinja resolves globals when a template evaluates, so any global
  registered before `expand_foreach_with_budget` is visible to every `foreach`,
  `when`, and rendered field.
- AXIOM-3. A closure of shape `Fn(&State, Rest<Value>) -> Result<Value, Error>`
  receives every positional argument and keyword arguments as a trailing
  `Kwargs` value; `Value::call` invokes a captured function value.
- AXIOM-4. A source attached with `minijinja::Error::with_source` survives to
  the loader's `anyhow` chain and can be downcast (the budget code relies on
  this; EP-M3 verifies it for refusals and stops under `Tolerances` if not).
- AXIOM-5. `serde_json_canonicalizer` output is deterministic for equal
  values; `serde_json::Value` object equality ignores key order.
- AXIOM-6. `proptest`, `insta`, and `rstest` behave as documented.

### Invariants and lemmas

- OBL-1 — wrapper equivalence.
  Statement: for each of the eleven public entry points and the crate entry
  `from_path_for_manifest_query_with_limits`, over the fixtures declared
  applicable to it, the canonical result (canonical JSON of the manifest on
  success; the `{:#}` error chain on failure) equals the result before the
  refactor. Method: characterization `insta` snapshots recorded in EP-M0 from
  the unchanged code; re-checked at every later milestone under C12. Rationale:
  the pre-refactor code is the only trustworthy oracle; comparing a wrapper
  with the core afterwards would be equal by construction. Domain: one row per
  entry point. Applicability: every entry uses only pure fixtures plus
  `jinja_env.yml` with an injected reader; `from_str` and other
  process-environment entries use only fixtures that never call `env()`;
  entries accepting a `StdlibConfig` receive a fixed clock and an empty
  `path_override`; no Full-mode row loads a fixture calling `fetch`, `shell`, or
  `which`. A malformed-YAML fixture and a budget-exhausting fixture cover
  error text. Artefact: public entries in
  `tests/manifest_load_options/wrapper_equivalence/` (since EP-M0 split into
  `mod.rs` for the string rows and `path_rows.rs` for the workspace-opening
  ones); the crate-internal query entry in
  `src/manifest/tests/wrapper_equivalence.rs`. Both use `insta::Settings` with
  `set_snapshot_path` to fixed directories
  (`tests/snapshots/manifest_load_options/`,
  `src/snapshots/manifest_load_options/`),
  `set_prepend_module_to_snapshot (false)`, and filters replacing
  temporary-directory paths with `[TMP]`. Evidence: `make test` green at EP-M0;
  afterwards
  `git diff --exit-code -- tests/snapshots/manifest_load_options src/snapshots/manifest_load_options`
  passes and no snapshot is pending. Non-vacuity: in EP-M2, temporarily delete
  the `register_legacy_boolean_formatter` call in
  `src/stdlib/register.rs::register_with_config`, and separately route
  `render_manifest_with_budget` through `RenderMode::ManifestQuery` in
  `src/manifest/render/mod.rs`; each must fail a named row. Revert both. EP-M2
  evidence: the baseline is 15 of 15 rows green. Fault A failed 11 named rows
  and passed only the two budget-exhaustion rows,
  `path_entry_point_cannot_inject_a_clock`, and `from_path_with_limits_row`;
  fault B failed the same 11 and passed the same four, which is the expected
  shape because those four never render a Boolean. Both faults were reverted
  and the baseline re-ran green; logs are named in `Progress`.

- OBL-2 — mode decision table.
  Statement: for each registration (`AmbientFull`, `Full`, `ManifestQuery`,
  `Test`), the `env()` disposition, `glob()` disposition, render mode,
  `records_expansion_and_budget_telemetry`, `classifies_query_refusals`, and
  "may carry overlays" are exactly as in `Decision log`. Method: exhaustive
  `rstest` table over the four variants. Rationale: finite and small; the table
  is exhaustive, so bounded model checking would add nothing. Artefact:
  `src/manifest/tests/loader_options.rs`. Evidence: red (compile failure)
  before EP-M2; green after. Non-vacuity: every row asserts all six outputs;
  swapping the `Test` and `Full` arms of
  `records_expansion_and_budget_telemetry` fails two rows. EP-M2 evidence: the
  table covers the three implemented variants at `ecfeb6c67` and passes; the
  `Test` row arrives with EP-M3, which is why EP-M2's acceptance evidence names
  OBL-2 "without `Test` rows". The sixth column is `#[cfg(test)]` until EP-M4
  consumes it (see `Decision log`).

- OBL-3 — surface inventory completeness.
  Statement: every helper name registered anywhere under `src/stdlib/` appears
  in `restricted_helpers::SURFACE` with a disposition for query and test mode;
  every refusing entry is registered as a stub under test; no name resolves to
  MiniJinja's unknown-function, -filter, or -test error under test. Method: a
  source-scanning unit test reading `src/stdlib/**/*.rs` (via
  `CARGO_MANIFEST_DIR` and `cap_std`) for `add_function("…"`, `add_filter("…"`,
  and `add_test("…"` literals, compared with `SURFACE` including its file-test
  rows; plus an `rstest` probe per inventory row loading a one-line manifest
  through `from_str_with_options` in test mode. Rationale: MiniJinja cannot
  list filters or tests, so the source is the only complete witness. Artefact:
  `src/stdlib/restricted_helpers/tests.rs`. Evidence: red before EP-M3; green
  after. Non-vacuity: add a scratch `env.add_filter("scratch", …)` to
  `src/stdlib/collections` and confirm the scan fails naming it; delete the
  `digest` stub and confirm its probe fails with `UnknownFilter`. Revert both.

- OBL-4 — test-mode refusals are named, coded, typed, and never classified.
  Statement: each refusing helper under test, called with the widest argument
  shape its real counterpart accepts, raises an error whose source downcasts to
  `TestModeRefusal { operation }` with the helper's name, whose text begins with
  `[netsuke::jinja::test_mode::unavailable]` in every catalogue tested, and
  which fails the load when raised inside `when`, including via a macro called
  from `when`. Method: `rstest` over the refusing inventory; an `insta`
  snapshot of the en-US text for one helper using the `en_localizer` fixture and
  `normalize_fluent_isolates`; one row under a non-en-US locale asserting the
  code; one behavioural scenario. Artefact:
  `src/stdlib/restricted_helpers/tests.rs`, `src/manifest/tests/test_mode.rs`,
  `tests/features/manifest_load_options.feature`. Evidence: red before EP-M3;
  green after. Non-vacuity: seeded faults — (a) build the test stub with the
  query marker text and remove the query-only gate: the `when` row must fail
  because the load succeeds with `conditional: true`; (b) restore the gate but
  keep the query text: the downcast row must fail.

- OBL-5 — Full/Test fidelity on pure manifests (loader-level I4).
  Statement: for every manifest built only from helpers real under test, with
  one fixed clock injected into both configurations, the manifest (as
  `serde_json::Value`), the `BuildGraph` (compared through its existing
  `GraphView` projection), and the Ninja text from `Full` equal those from
  `Test`. Method: `proptest` via a manually driven `TestRunner` with
  `ProptestConfig { cases: 128, .. }`, as in
  `src/ninja_gen/property_tests/ninja_oracle.rs`, over a grammar valid by
  construction: one to four targets, optional `vars`, one optional macro,
  `foreach` over literal lists of up to three strings, `when` comparisons,
  Boolean interpolation, lexical path filters, collection filters, `timedelta`,
  and `now()`. The closure increments per-construct counters. Rationale:
  interactions (for example Boolean formatting inside a `foreach` item) are an
  open input space; the oracle is the other mode. Artefact:
  `tests/manifest_load_options/fidelity_property.rs`. Evidence: red before
  EP-M3; green after, with every Full load succeeding (a Full failure is a
  generator bug and fails the test) and every construct counter at or above 10
  after the run. Non-vacuity: the counters prove each construct was reached;
  seeded fault — omit the Boolean formatter from `register_for_test` and
  confirm a shrunk counterexample containing a Boolean interpolation.

- OBL-6 — test mode reads no host environment by default.
  Statement: a test load with no reader answers `env('PATH')` as not set
  although `PATH` is set in the test process; with a surface reader, `env()`
  returns the reader's value. Method: named unit tests; no environment mutation
  (C8). Artefact: `src/manifest/tests/test_mode.rs`. Evidence: red before
  EP-M3; green after. Non-vacuity: seeded fault — default the surface to
  `process_env_reader()`; the `PATH` test must fail.

- OBL-7 — `now()` under test.
  Statement: with `with_clock(fixed_clock(t))`, `now()` returns `t`; with
  `with_clock(system_clock())`, `now()` is real (explicit injection, recorded
  in ADR-0060); with the default clock, `now()` refuses with the test code.
  Method: three named unit tests. Artefact: `src/manifest/tests/test_mode.rs`.
  Evidence: red before EP-M3; green after. Non-vacuity: seeded fault — register
  `now()` unconditionally; the default clock row must fail.

- OBL-8 — overlay names are validated.
  Statement: an overlay named `env`, `__netsuke_manifest_macro_imports` (or any
  `__netsuke_` name), a manifest `vars` key, or a manifest macro name fails the
  load with `TemplateOverlayError::NameCollision` naming it and its kind; a
  stdlib name (for example `glob`) or a fresh name is accepted. Method:
  `rstest` table, one row per kind. Artefact: `src/manifest/tests/overlays.rs`.
  Evidence: red before EP-M4; green after. Non-vacuity: removing the check
  fails every rejection row, and the `__netsuke_manifest_macro_imports` row
  additionally shows macros vanishing.

- OBL-9 — overlay order, shadowing, and passthrough.
  Statement: an overlay replaces the same-named refusing stub or real helper
  and is visible to `foreach`, `when`, and rendered fields;
  `OverlayCall::shadowed` is the function it replaced; empty overlays equal no
  overlays. Method: named tests — an overlay `glob` drives a `foreach`; an
  overlay `which` filters a target in `when`; an overlay used in a command
  renders; a passthrough overlay on `timedelta` returns the shadowed function's
  result; an overlay on `now` sees the refusing stub as its shadowed function;
  a load with `TemplateOverlays::default()` equals one without overlays.
  Artefact: `src/manifest/tests/overlays.rs`. Evidence: red before EP-M4; green
  after. Non-vacuity: seeded fault — register overlays before the stdlib; the
  `glob` test must report the test refusal instead.

- OBL-10 — manifest query unchanged.
  Statement: every query refusal message and every `help targets` output is
  byte-identical to before. Method: EP-M0 snapshots of each query refusal (one
  row per helper), written with a fixed `set_snapshot_path` so EP-M1's move
  does not rename them, plus the existing `tests/runner_help_targets_tests.rs`
  and `tests/features/help_targets.feature`. Artefact: EP-M0 places the rows in
  `src/stdlib/register/refusal_snapshots.rs` (a `#[path]` test child of
  `register.rs`); EP-M1 moves them unchanged into
  `src/stdlib/restricted_helpers/tests.rs`. Evidence: green at EP-M0 and under
  C12 afterwards. Non-vacuity: change one character of the query marker; the
  rows must fail.

- OBL-11 — corpus fidelity-or-refusal.
  Statement: for every corpus manifest, loading with the same injected reader
  and fixed clock in both modes either gives equal results (canonical-equal
  manifests, or identical error text) or the test load fails with a
  `TestModeRefusal` whose operation is in the refusing inventory; a test load
  never succeeds with a different manifest. Method: one test walking
  `tests/data/` and `tests/data/manifest_load_options/` at run time with
  `cap_std`, asserting per file and then asserting that the run saw at least
  one equal-result file and at least one refusal from each family (network,
  process, filesystem, host lookup, clock). Artefact:
  `tests/manifest_load_options/corpus_fidelity.rs`. Evidence: red before EP-M3;
  green after. 7.1.4 later adds `examples/`. Non-vacuity: the aggregate family
  assertion; seeded fault — make `register_for_test` register the real `hash`
  filter; the corpus run must fail on the file using it.

- OBL-12 — public surface compiles for an embedder.
  Statement: an external crate can build `ManifestLoadOptions::full`,
  `ManifestLoadOptions::test`, a `TestSurface` with overlays, and call
  `from_str_with_options`; it cannot name `StdlibRegistration`. Method: a
  `trybuild` compile-pass fixture following
  `tests/ui/manifest_environment_embedder_pass.rs`, registered in the existing
  UI harness. Artefact: `tests/ui/manifest_load_options_embedder_pass.rs`.
  Evidence: red before EP-M2 (fixture fails to compile); green after EP-M4.
  Non-vacuity: the fixture exercises every public constructor and accessor.

Telemetry wiring: `src/manifest/tests/expansion_telemetry.rs` and
`src/manifest/tests/budget/mod.rs` already observe the full and query paths;
EP-M3 adds one `DebuggingRecorder` row showing a test load emits no expansion
or budget metric.

### Methods deliberately not used

- Kani. The only finite decision logic (OBL-2) is four rows, which the table
  enumerates exhaustively; there is no arithmetic, `unsafe`, or bounded state
  machine for a model checker to strengthen.
- Verus. No repository-owned lemma over an unbounded domain is introduced.
  Fidelity (OBL-5) is a property of MiniJinja rendering, a third-party
  interface treated as an axiom; proving it would require modelling MiniJinja.
  Differential testing against the real engine is the proportionate method.
- `loom`/`shuttle`. No concurrency is introduced.
- `cargo-mutants`. Not required; the seeded faults above are the targeted
  mutation evidence.

## Milestones and plateaus

Each milestone ends with the full gate set run sequentially by `scrutineer`:
`make check-fmt`, `make typecheck`, `make lint`, `make doc-coverage`,
`make test`, plus `make markdownlint` and `make nixie` whenever Markdown
changed. Never commit a red gate. Red evidence is observed, recorded in
`Artefacts and notes`, and committed together with its green change. Feature
files land in the same commit as their steps (strict compile-time validation).
After each milestone, perform the conformance check listed and update
`Progress`.

### EP-M0 — characterization baseline

- Outcome: canonical snapshots of every wrapper and of every query refusal
  exist and pass against the unchanged code.
- Requirements: RM-e, TD-I7 (oracle for OBL-1, OBL-10).
- Acceptance evidence: gates green; snapshots committed; no production file
  changed.
- Conformance check: tests only; snapshot paths are fixed by
  `set_snapshot_path`.
- Recovery: delete the new tests and snapshots.
- Remaining gaps: everything else.
- Compatibility decision: none.

### EP-M1 — extractions

- Outcome: query stubs and the surface inventory live in
  `src/stdlib/restricted_helpers/mod.rs` behind a private `RestrictedSurface`
  (initially `ManifestQuery` only); `classify_query_evaluation` and
  `QueryEvaluation` live in
  `src/manifest/jinja_macros/query_classification.rs`. The query-refusal rows
  move unchanged to `src/stdlib/restricted_helpers/tests.rs`. Behaviour is
  byte-identical. The directory form, not the flat `restricted_helpers.rs` the
  earlier draft named, is required — but by the **layout rule**, not by the
  contract function the earlier draft cited.
  `docs/repository-layout.md:409-419` states that a shared name prefix
  represents a module hierarchy, that its children go under a directory module
  using `mod.rs`, and that `#[path]` is kept "only when a specific requirement
  still needs it". An earlier draft attributed the prohibition to
  `prefixed_files_beside_directory()`, which was false: that function's glob is
  `f"{directory.name}_*.rs"` and so catches only an underscore-prefixed
  *sibling* (`restricted_helpers_tests.rs`), never a bare
  `restricted_helpers.rs` beside `restricted_helpers/`. Running both contract
  functions against synthetic trees confirms the flat-plus-`#[path]` shape
  passes them (the live `register.rs` + `register/` pair is the same shape and
  passes today); `sibling_prefix_groups` is the function that would reject a
  true sibling, and even it accepts a `#[path]` child. The directory form is
  still the right choice; cite the rule, not the function.
- Requirements: RM-c (mechanism made reusable); C10 headroom.
- `SURFACE` content, enumerated read-only at `71f26e1f` so the move can be
  checked against it:
  - **Refused under query — 23 entries.** From
    `src/stdlib/register/query_helpers.rs`: the six always-disabled helpers
    (`env`, `glob`, `fetch`, `shell`, `grep`, `contents`); the seven
    `FILE_TESTS` members (`dir`, `file`, `symlink`, `pipe`, `block_device`,
    `char_device`, `device`); and the ten host-dependent helpers (`which` — as
    both a filter and a function, two registrations — `command_available`,
    `now`, `realpath`, `expanduser`, `size`, `linecount`, `hash`, `digest`).
    Re-verified 2026-10-15 against the EP-M0 oracle,
    `src/stdlib/register/refusal_snapshots.rs`, whose 23 rows are numbered
    `=== 00 env … ===` through `=== 22 device … ===` and are the cheapest place
    to re-count.
  - **Permitted under query — 11 entries**, grouped by the registration
    function that adds them inside `register_manifest_query`
    (`src/stdlib/register.rs:165`). The order below is the order each function
    registers in; the four functions themselves run in the order listed:
    - `path::register_query_filters` → `register_lexical_filters`:
      `basename`, `dirname`, `with_suffix`, `relative_to`.
    - `collections::register_filters`: `uniq`, `flatten`, `compact`,
      `group_by`.
    - `recipe_text::register_filters`, quoting for the host-default dialect:
      `shell_quote`, `shell_join`.
    - `time::register_query_functions`: `timedelta`.
    The count is the *names* the enumeration lists, re-verified against source
    on 2026-10-15. An earlier draft said 15, which its own name list
    contradicted; the name list was right and the number wrong. That draft also
    said "in registration order" over an alphabetized list, which the grouping
    above replaces with an order that is actually the source's.
  - **Trap:** `realpath` and `size` are *not* dual-surface. They appear only in
    `register_filters` (the build surface); reading them as query-permitted
    comes from letting a `grep -A30` run past the end of
    `register_query_filters` into `register_filters`. The query side is
    lexical-only by construction, because `register_query_filters` calls
    `register_lexical_filters` alone.
- Acceptance evidence: gates green; C12 diff clean; both source files well
  under 400 lines.
- Conformance check: refactor only; no public change.
- Recovery: revert the refactor commits (one per extraction).
- Remaining gaps: the test surface.
- Compatibility decision: none (private code).
- **The extraction invalidates a stated rationale, so carry a replacement.**
  `classify_query_evaluation`'s doc comment
  (`src/manifest/jinja_macros/mod.rs:124-126`) argues for keeping the adapter
  *where it is*: "Keeping the adapter here confines that implementation detail
  to the Jinja boundary; manifest expansion consumes only [`QueryEvaluation`]."
  Moving it to `query_classification.rs` contradicts that sentence, so the
  relocated code needs a rationale for the new home rather than a copy of the
  old one. The parent is at **394 lines** against AGENTS.md's 400-line cap —
  six lines of slack — so this extraction is load-bearing for C10 and the
  parent cannot absorb the code instead. `QueryEvaluation`'s own doc block
  moves with it and carries the same issue.
- **Wiring the `is_manifest_query_disabled_error` re-export.** Three hops must
  survive the move: `src/stdlib/mod.rs:33` re-exports it from `register`,
  `register.rs:29` re-exports it from `query_helpers`, and
  `src/manifest/jinja_macros/mod.rs:129` calls it through `crate::stdlib`. The
  move only forces re-pointing `register.rs:29`; the other two are by-name and
  keep working. The EP-M0 refusal rows in `register/refusal_snapshots.rs:44`
  import `super::{is_manifest_query_disabled_error, register_manifest_query}`
  and move to `restricted_helpers/tests.rs` in the same commit, so their
  `super::` path changes too.
- **`FILE_TESTS` visibility.** It is a private `const` in `register.rs` today
  and is imported by `query_helpers.rs:16` for the stubs and consumed at
  `register.rs` by the real registrations. The plan's decision stands: either
  widen its visibility or subsume it in `SURFACE`, which is preferable because
  `SURFACE` is the single list that stops a test reaching one surface and
  staying unregistered on the other.
- **Kani scope must move in the same commit.** `tools/kani/proof-scope.toml`
  lists `src/stdlib/register/query_helpers.rs` at **:162** and
  `src/stdlib/register.rs` at `:161`. The scope is a derived closure of the
  `#[kani::proof]` harnesses and
  `tests/workflow_contracts/kani_proof_scope_test.py` rejects both a missing
  file and an entry covering nothing a harness reaches, so the moved file's
  entry must be re-derived. Run `make test-workflow-contracts` to validate the
  updated list against that closure; `make test-kani-scope-wrapper` is an
  unrelated systemd-run wrapper test and does not derive proof-source paths.
  Read-only recon taken 2026-10-15 while a gate batch held the candidate
  frozen; no `restricted_helpers` symbol existed in the tree at that point, so
  EP-M1 is genuinely unstarted.
- **Placement of the new module**, from `docs/repository-layout.md:409-439`: a
  shared name prefix means a directory module with `mod.rs` and plain
  `mod child;` declarations (dropping the parent prefix from filenames).
  `restricted_helpers/` therefore needs a `mod.rs`, and `config/` is the
  canonical shape to copy: `mod.rs` plus `tests.rs` plus plain children, no
  `#[path]`.

  The test file is `restricted_helpers/tests.rs`, **not**
  `restricted_helpers/tests/mod.rs`. Both shapes are live and neither is
  contract-enforced, so the choice needs a reason. Measured across `src/`: 38
  files are named `tests.rs`, 18 directories are named `tests/` of which 17
  hold a `mod.rs`, and 8 of those 17 hold a single child. File count therefore
  does not decide it — a one-child `tests/mod.rs` is common. Three things do
  decide it. First, the moved file already names its destination:
  `refusal_snapshots.rs:25-26` records that "EP-M1 can move this file into
  `src/stdlib/restricted_helpers/tests.rs`". Second, `tests.rs` is the majority
  shape, and 18 directories under `src/` consist of nothing but `mod.rs` and
  `tests.rs` — so the shape has ample precedent, `src/stdlib/config/` among it.
  Third, `refusal_snapshots.rs` is reached by `#[path]` today
  (`register.rs:39`), so a plain `mod tests;` retires that declaration, the
  direction `docs/repository-layout.md:418-420` asks for. The snapshot is
  indifferent either way: `stable_snapshot_settings("manifest_load_options")`
  pins `src/snapshots/manifest_load_options/` and
  `set_prepend_module_to_snapshot(false)` drops the module path from the name,
  so `refusals.snap` keeps its directory and its name under any placement. A
  second test file under the module is the change that would earn a
  `tests/mod.rs`.

  A justified `#[path]` additionally needs a comment stating the reason and an
  entry in the support-module ownership inventory; note that today's
  `register/query_helpers.rs` has **no** inventory entry, which is a
  pre-existing gap rather than one this milestone must close.

- **Fate of `src/stdlib/register/`**, which the placement bullet above settles:
  the directory holds exactly two files, `query_helpers.rs` and
  `refusal_snapshots.rs`, and both move under EP-M1. The `#[path]` declarations
  at `register.rs:27` and `:39` are therefore deleted, the emptied directory is
  removed, and `register.rs` stays a flat file with no children — **no `mod.rs`
  conversion is forced on it**. Two items move with the code:
  `MANIFEST_QUERY_DISABLED_HELPER_MARKER` (`register.rs:52`) belongs to the
  query-disabled vocabulary and follows it into `restricted_helpers/mod.rs`, and
  `FILE_TESTS` (`register.rs:228`/`:240`) must stay reachable by both the real
  registrations (`register.rs:253`) and the moved stubs, which is the plan's
  stated case for subsuming it in `SURFACE` rather than widening its visibility.

### EP-M2 — options entry point and thin wrappers

- Outcome: `ManifestLoadOptions`, `from_str_with_options`, the
  `AmbientFull`/`Full`/`ManifestQuery` registration with its query methods,
  `env`/`glob` registration by the loader, the query-only classifier gate,
  crate-private `from_path_with_options`, and every existing entry point as a
  wrapper. `evaluate_manifest` is split into `build_template_environment` and
  an expansion-and-render helper.
- Requirements: RM-a (options, wrappers), TD-4.3, C3, C5.
- Acceptance evidence: OBL-1 (C12 diff clean), OBL-2 (without `Test` rows),
  the `from_str_with_options` doctest, and OBL-10 green.
- Conformance check: only listed public items added; no existing signature
  changed; `ManifestLoadMode` deleted.
- Recovery: revert the milestone's commits; EP-M0 and EP-M1 stand alone.
- Remaining gaps: `Test`, overlays.
- Compatibility decision: the wrappers are the roadmap's stated requirement
  (about 70 callers across tests, benches, and the runner), not a shim.

### EP-M3 — `StdlibRegistration::Test`

- Outcome: `TestSurface` (without overlays), `ManifestLoadOptions::test`,
  `stdlib::register_for_test`, `TestModeRefusal`, the Fluent key in all 35
  catalogues, the test default reader, and the first two behavioural scenarios
  with their steps.
- Requirements: RM-b, RM-c, TD-4.2, TD-5.2, TD-5.5 (as amended), UX-1;
  advances TD-I5.
- Acceptance evidence: OBL-2 (all rows), OBL-3, OBL-4, OBL-5, OBL-6, OBL-7,
  OBL-11 green; telemetry row green; locale audit green.
- Conformance check: refusing set equals the inventory; C12 diff clean.
- Recovery: revert the milestone's commits; EP-M2 stands alone.
- Remaining gaps: overlays.
- Compatibility decision: none.

### EP-M4 — template overlays

- Status: implemented and locally verified on 2026-11-01; see `Progress`.
- Outcome: `TemplateOverlays`, `OverlayCallable`, `OverlayCall`,
  `TemplateOverlayError`, the collision key in all catalogues, registration
  after macros and before expansion, the third behavioural scenario, and the
  embedder fixture.
- Requirements: RM-a (overlays), RM-d, TD-4.3 step 5, TD-7, C9.
- Acceptance evidence: OBL-8, OBL-9, OBL-12 green; C12 diff clean.
- Conformance check: order matches TD-4.3 steps 1–6; no `cfg` gate.
- Recovery: revert the milestone's commits.
- Remaining gaps: macro substitution (7.1.3).
- Compatibility decision: none.

### EP-M5 — documentation, ADR, and roadmap closure

- Outcome: design §4.2, §4.3, §5.5, and §7 reconciled; ADR-0060 written and
  indexed in `docs/contents.md`; ADR-0050 Addendum E; developers' guide,
  `docs/netsuke-design.md`, and `docs/polonius.md` updated; roadmap 7.1.2 and
  its five bullets ticked; this plan set to `COMPLETE`.
- Requirements: design §15; AGENTS.md documentation rules.
- Acceptance evidence: Markdown gates and `make check-fmt` green.
- Conformance check: every deviation in `Decision log` appears in the design or
  an ADR.
- Recovery: revert documentation commits.
- Remaining gaps: none for 7.1.2; the sandbox adapters belong to 7.4.1.
- Compatibility decision: none.

## Plan of work

### Stage A — orientation (no code changes)

Read the files in `Context and orientation`. Run the full gate set on the
untouched branch through `scrutineer` and record the baseline. Re-confirm
ADR-0060 is free immediately before minting it, by sweeping `docs/adrs/` across
every remote branch: the number is only valid at the sweep, and `main` can take
it while this branch sits open.

### Stage B — characterization (EP-M0) — complete

Add the new fixtures under `tests/data/manifest_load_options/` (the four
planned plus three recorded as a deviation; see `Decision log`). Create
`tests/manifest_load_options_tests.rs` declaring `mod manifest_load_options;`
and `tests/manifest_load_options/mod.rs` declaring `wrapper_equivalence` (later
also `fidelity_property` and `corpus_fidelity`), each file under 400 lines. In
`wrapper_equivalence.rs`, write one `rstest` per public entry point over its
applicable fixtures, snapshotting canonical JSON or the error chain with the
insta settings from OBL-1. (That file outgrew the 400-line cap once the rows
became observable, so it was split during EP-M0 into
`wrapper_equivalence/mod.rs` plus `wrapper_equivalence/path_rows.rs`.) Path
entries copy fixtures into a `tempfile` directory. Add the crate-internal query
rows in `src/manifest/tests/wrapper_equivalence.rs` and the query-refusal rows
in `src/stdlib/register/refusal_snapshots.rs`. Review every new snapshot by
reading it, then accept it; this is the only acceptance the characterization
snapshots ever receive. The refusal rows record one document rather than one
snapshot per helper, so a reviewer reading for "does every row name the helper
it should?" can read them comparatively — twenty-two files differing only in
their leading word would hide the one thing worth seeing. The two internal
modules reached by `#[path]` are `register/query_helpers.rs` and
`register/refusal_snapshots.rs`; the reason is that their parent sits at the
400-line cap and they are split out to hold it. An earlier draft added that
neither `register/` nor `manifest/tests/` holds a `mod.rs`, which is false for
the second: `src/manifest/tests/mod.rs` exists and `src/manifest/mod.rs:249`
declares it with a plain `mod tests;`. Only `register/` is a `#[path]` case;
the fact that matters for EP-M1 is that `register/` stops being one, because
both its files leave and `register.rs` reverts to a flat file with no children.

### Stage C — implementation (EP-M1 to EP-M4)

EP-M1. Move the query stubs, `manifest_query_operation_error`,
`is_manifest_query_disabled_error`, and the marker from
`src/stdlib/register.rs` to `src/stdlib/restricted_helpers/mod.rs`. Introduce
`const SURFACE: &[HelperEntry]` (name, kind — function, filter, or test — and
dispositions for query and test) and a private
`enum RestrictedSurface { ManifestQuery }` whose stub registration iterates
`SURFACE`, producing today's exact text. Keep
`is_manifest_query_disabled_error` reachable through the existing
`src/stdlib/mod.rs` re-export. In a separate commit, move
`classify_query_evaluation` and `QueryEvaluation` to
`src/manifest/jinja_macros/query_classification.rs`.

EP-M2. Red: add `src/manifest/tests/loader_options.rs` (OBL-2 rows for the
three existing modes), the `from_str_with_options` doctest, and the OBL-12
fixture; observe compile failures. Green: create
`src/manifest/loader_options.rs` with `ManifestLoadOptions`, its constructors
and builders, `from_str_with_options`, and
`register_host_helpers(env, registration, environment, manifest_root)`, which
registers `env()` and `glob()` per C5. Rename the private
`StdlibRegistration::None`-equivalent to `AmbientFull` and add the query
methods. Remove `env`/`glob` from `RestrictedSurface::ManifestQuery`. Change
`ManifestParse` to hold `name`, `manifest_root`, and a `ManifestLoadOptions`.
Split `evaluate_manifest` into `build_template_environment` (steps 2–6) and
`expand_and_render` (steps 7–8). Thread `classifies_query_refusals` into the
expansion context and gate the classifier. Rewrite every `from_str*` entry and
`from_str_with_limits` as wrappers. Replace `ManifestLoadMode` and
`from_path_with_registration` with `from_path_with_options`. Replace both
`matches!(…, ManifestQuery)` sites. Run the OBL-1 seeded faults, record their
failures, and revert them.

EP-M3. Red: add OBL-3, OBL-4, OBL-6, OBL-7 tests, the OBL-5 property, the
OBL-11 corpus test, the telemetry row, and scenarios 1–2 with their steps;
observe failures. Green: add `TestSurface` (config and optional reader) and
`StdlibRegistration::Test(Box<TestSurface>)`; add `TestModeRefusal` and
`RestrictedSurface::Test`, whose stubs take `Rest<Value>` so any call shape
reaches the refusal; add `pub(crate) fn register_for_test(env, config)` in
`src/stdlib/restricted_helpers/mod.rs` installing the Boolean formatter, the
pure helpers, `now()` when `!config.clock().is_system()`, and the test stubs
from `SURFACE`. Update `WallClock::is_system`'s documentation. Add
`STDLIB_TEST_MODE_HELPER_UNAVAILABLE` to `src/localization/keys.rs` and the
message to all 35 catalogues in the same commit, following the translators'
guide (keep `netsuke test`, helper names, and the bracketed code untranslated;
begin `ar`, `fa`, and `he` messages with U+200F).

EP-M4. Red: add `src/manifest/tests/overlays.rs` (OBL-8, OBL-9), scenario 3
with its steps, and extend the OBL-12 fixture; observe failures. Green: create
`src/manifest/test_surface.rs` holding `TestSurface` (moved from EP-M3's
location if needed), `TemplateOverlays`, `OverlayCallable`, `OverlayCall`,
`TemplateOverlayError`, `overlay_namespace(doc)` (vars keys and macro names,
computed before step 3 and reused by the existing reserved-name check), and
`register_overlays`. Call validation and registration in
`build_template_environment` after macro registration. Add the collision key to
`keys.rs` and every catalogue in one commit.

### Stage D — documentation and closure (EP-M5)

Update `docs/netsuke-test-framework-technical-design.md`: §4.2 (current
anchors), §4.3 (options shape, `TestSurface`, overlays in the test surface,
`AmbientFull`, string entry plus crate-private path entry, deferred
`macro_substitutions`, overlay name rules), §5.5 (the full refusing inventory
and the `which`/`expanduser` deviation; the six file-reading filters are
already recorded there), and §7 (shadowed-function capture is possible). Write
`docs/adrs/adr-0070-deny-by-default-test-mode-stdlib.md` (surface, refusal code
and typed source, `is_system` as injection; serial `0070` rather than the
`0060` first written here, per the Decision log) and index it in
`docs/contents.md`. Add Addendum E to
`docs/adrs/adr-0050-bound-manifest-template-evaluation.md`. Add a "Manifest
loader entry points" subsection to `docs/developers-guide.md` (new callers use
`from_str_with_options`; do not add another `from_path_with_*` rung; how
overlays and the surface inventory work, and that a new stdlib helper needs a
`SURFACE` row). Record `ManifestLoadOptions<'a>` in `docs/polonius.md` (borrowed
`ManifestEnvironment` and stage observer; no clones). Add one sentence to the
manifest-loading section of `docs/netsuke-design.md` naming the load modes.
Tick roadmap 7.1.2 and its bullets. Run `make fmt`, then the Markdown gates.

## Concrete steps

Run everything from the repository root,
`/home/leynos/.lody/repos/github---leynos---netsuke/worktrees/0965d3fb-38f0-46dc-a907-0cd999d8a910`.
Gates are run by `scrutineer`, one at a time, each teeing to
`/tmp/$ACTION-netsuke-7-1-2-options-carrying-manifest-loader-entry-point.out`.

Focused loops use the gate `RUSTFLAGS`, so they share the build cache with the
gates rather than invalidating it:

```sh
export RUSTFLAGS="-D warnings -Zthreads=8 -Clink-arg=-fuse-ld=mold"
cargo nextest run --workspace --all-features --test manifest_load_options_tests \
  2>&1 | tee /tmp/focus-netsuke-7-1-2.out
cargo nextest run --workspace --all-features --lib -E 'test(/manifest::tests::(loader_options|test_mode|overlays)/)' \
  2>&1 | tee -a /tmp/focus-netsuke-7-1-2.out
```

Expected red transcripts (abridged):

```plaintext
EP-M2: error[E0432]: unresolved import `netsuke::manifest::ManifestLoadOptions`
EP-M3: error[E0599]: no function or associated item named `test` found for struct `ManifestLoadOptions`
EP-M4: error[E0432]: unresolved import `netsuke::manifest::TemplateOverlays`
```

Expected green transcript after EP-M4 (abridged):

```plaintext
     Summary [ …s] N tests run: N passed, 0 skipped
```

## Validation and acceptance

Behavioural specification, `tests/features/manifest_load_options.feature`,
reusing the existing steps "the manifest has targets named …", "parsing the
manifest fails", and "the error message contains …"
(`tests/bdd/steps/manifest/`), with new steps in
`tests/bdd/steps/manifest_load_options.rs` and new `TestWorld` slots for the
build result, test result, and overlays:

```gherkin
Feature: Options-carrying manifest loading

  Scenario: A pure manifest loads identically in build and test modes
    Given the manifest file "tests/data/manifest_load_options/boolean_interpolation.yml"
    And a fixed clock at "2026-06-08T12:00:00Z"
    When the manifest is loaded in build mode and in test mode
    Then both loads produce the same Ninja file

  Scenario: Test mode refuses a process helper
    Given the manifest file "tests/data/manifest_load_options/uses_shell.yml"
    When the manifest is loaded in test mode
    Then parsing the manifest fails
    And the error message contains "[netsuke::jinja::test_mode::unavailable]"
    And the error message contains "shell"

  Scenario: A template overlay stands in for glob under test
    Given the manifest file "tests/data/glob.yml"
    And a template overlay "glob" returning "glob_files/a.txt" and "glob_files/b.txt"
    When the manifest is loaded in test mode
    Then the manifest has targets named "a.out, b.out"
```

Scenarios 1 and 2 land in EP-M3 and scenario 3 in EP-M4, each with its steps.
They are acceptance tests: strict compile-time validation prevents committing a
feature before its steps, so their red stage is the missing-step compile error
observed before the steps are written. `tests/data/glob.yml` names each target
by rewriting `glob_files/<stem>.txt` to `<stem>.out`, so the overlay values
above yield `a.out` and `b.out`.

Quality criteria:

- Tests: `make test` green; every new test passes; no pre-existing snapshot or
  scenario changed; C12 diff clean.
- Verification: OBL-1 to OBL-12 discharged with their non-vacuity evidence
  recorded in `Artefacts and notes`.
- Lint and types: `make check-fmt`, `make typecheck`, `make lint` green.
- Documentation: `make markdownlint`, `make nixie` green; `make doc-coverage` at
  or above 80 per cent.

## Idempotence and recovery

Every step is re-runnable, except that characterization snapshots are accepted
once (C12). Each milestone is a set of commits that can be reverted
independently, newest first. If a gate fails midway, fix forward within the
three-attempt tolerance or return to the last green commit with `git restore`.
Never use bare `git stash`; the stash is shared across worktrees.

## Artefacts and notes

Record here, as work proceeds: the baseline gate results; each red transcript;
each seeded-fault failure (OBL-1, OBL-2, OBL-3, OBL-4, OBL-5, OBL-6, OBL-7,
OBL-8, OBL-9, OBL-10, OBL-11); and the OBL-5 construct counters.

### Stage A baseline (2026-10-14)

Candidate frozen at `0b2a366b2bd4749fe8688112b8070d3071219b59`;
`git status --short` empty before the first gate and after every gate. Logs at
`/tmp/$ACTION-netsuke-7-1-2-options-carrying-manifest-loader-entry-point.out`.
Gates ran sequentially in the plan's order, under `scrutineer`'s exclusive
ownership.

| Gate                | Result | Evidence                                             |
| ------------------- | ------ | ---------------------------------------------------- |
| `make check-fmt`    | PASS   | ruff 250 files formatted; mdtablefix 183 unchanged   |
| `make typecheck`    | PASS   | 0 errors, 1144 s                                     |
| `make lint`         | PASS   | clippy, whitaker, ruff, pylint, yamllint, actionlint |
| `make doc-coverage` | PASS   | aggregate 4988/5045, 98.87% ≥ 80%                    |
| `make test`         | RED    | one nextest timeout at 300.012 s; doctest never ran  |
| `make markdownlint` | PASS   | spelling clean; 0 issues in 183 files                |
| `make nixie`        | PASS   | all diagrams validated                               |

The red gate, in its own words:

```plaintext
     TIMEOUT [ 300.012s] (3913/3928) netsuke-build::command_env_ui_tests cli_configuration_fixture_compiles
  Summary [ 481.703s] 3913/3928 tests run: 3912 passed (1 slow), 1 timed out, 6 skipped
error: test run failed
make: *** [Makefile:230: test-nextest] Error 100
```

This is load attribution, and the awaiting call is **inside** the test body:
`tests/command_env_ui_tests.rs:86-94` spawns `cargo check` and awaits it with
`.output()?`, so nextest's per-test clock covers the admission-queue wait. A
wider allowance would cover the delay; what it cannot do is remove the
contention. `Tolerances`' `Iterations` trigger does not apply, because there is
no root cause in this candidate to fix, and `.config/nextest.toml` already
documents this exact signature for this exact test and deliberately keeps the
budget at five periods rather than "moving to an allowance the delay can still
exceed". Four independent checks agree the branch cannot be the cause: the diff
against `origin/main` is three Markdown files under `docs/`; a diff of the
failing test, its nextest policy, `src/`, and `tests/` between `origin/main`
and this head is empty; the gate log holds 60 compile-slot waits, 573
sccache-unreachable notices and zero admission failures; and
`build-limits-status` read `capacity 4, held 4, healthy, sccache unreachable`,
with a foreign `build-limits-rustc` census of 61 to 80 processes across two
probes around the gate (the pool is shared, so the figure moves second to
second). Inside the same run, the sibling nested-Cargo fixture
`build_module_slice_ui_tests::production_build_module_slice_has_expected_boundary`
passed at 168.114 s, 132 s under the same cap, which measures the contention
rather than arguing it from history.

**Disposition: OPEN, re-run rather than repaired.** A red gate is not committed
past; this one is re-run once, at a later milestone, when the host is quieter.
If it then passes, this record stands as the explanation of the first red. The
doctest half of `make test` is unverified by this baseline, not failed.

### EP-M0 gate runs

Candidate at `1d75efce`, `git status --short` empty before the first gate and
after every gate. Same log template and same sequential discipline as the Stage
A baseline.

| Gate                           | Result | Evidence                                              |
| ------------------------------ | ------ | ----------------------------------------------------- |
| `make check-fmt`               | PASS   | no reformat required                                  |
| `make doc-coverage`            | PASS   | aggregate 4988/5045, 98.87% ≥ 80% (880 s)             |
| `make test-workflow-contracts` | PASS   | 1271 passed, 3 skipped, then 27 passed (131 s)        |
| `make nixie`                   | PASS   | "All diagrams validated successfully", Figure 2 incl. |
| `make markdownlint`            | PASS   | 0 issues in 183 files; spelling clean (22 s, 10 s)    |
| `make lint`                    | RED    | two `conditional_max_n_branches` at `lint-whitaker`   |
| `make test`                    | RED    | one nextest timeout at 300.008 s; doctest never ran   |

Two reds, two different causes, and neither is a defect the earlier records did
not predict:

- **`make lint`.** Whitaker rejected the two multi-branch conditions in
  `tests/manifest_load_options/fixtures/fixture.rs` at `:156` and `:161`. Both
  were introduced by `fbf221f0`, which is also the commit whose clippy repair
  produced the surrounding structs; the checks that would have caught it before
  it was committed do not exist, because Clippy has no analogue of this rule.
  Repaired in `9191699b`, then the `lint-whitaker` stage was re-run **in
  isolation** against the repaired tree and exited 0 with zero diagnostics,
  having compiled the previously failing `manifest_load_options_tests` target
  through the dylint driver. Because `make lint` aborts at its first failing
  stage, `yamllint`, `actionlint`, and every Python lint stage are **UNRUN** for
  `1d75efce` and must be re-run against the repaired head. The eight clippy
  findings that `fbf221f0` repaired and the two Whitaker findings here are the
  same failure mode twice in one milestone: a stage that aborts conceals every
  check behind it.

- **`make test`.** `command_env_ui_tests::cli_configuration_fixture_compiles`
  timed out at the repo's own 300-second cap:

  ```plaintext
  TIMEOUT [ 300.008s] (3928/3943) netsuke-build::command_env_ui_tests cli_configuration_fixture_compiles
  Summary [ 613.491s] 3928/3943 tests run: 3927 passed (1 slow), 1 timed out, 6 skipped
  ```

  This is the same row, the same signature, and the same load attribution as
  the Stage A baseline, and every `src/` file this branch edits is test-only:
  `src/stdlib/register.rs` gains a `#[cfg(test)]` child declaration,
  `src/manifest/tests/mod.rs` gains a `mod` line inside a tests module, and
  `src/snapshot_test_support.rs` is gated behind `#[cfg(test)]` at
  `src/lib.rs:30`. No production item changed. The log again holds 107
  compile-slot waits and 574 sccache-unreachable notices and zero admission
  failures. The SLOW row differs from the baseline's, which is what one expects
  from queue order and not from a candidate.

**Disposition: `lint` repair committed and its stage re-verified in isolation;
the full gate set must be re-run at `9191699b` before the milestone is called
green. The `make test` timeout is re-run, not repaired.**

### EP-M0 gate re-run at `ca7ed7d4`

The repaired head, with the plan records above included in the candidate. Same
log template, same sequential discipline, same frozen-tree rule.

| Gate                           | Result | Evidence                                                |
| ------------------------------ | ------ | ------------------------------------------------------- |
| `make check-fmt`               | PASS   | 3 s; 250 files formatted, 183 unchanged                 |
| `make typecheck`               | PASS   | 169 s; 0 errors                                         |
| `make lint`                    | PASS   | 814 s; **every stage reached** (see below)              |
| `make doc-coverage`            | PASS   | 397 s; aggregate 4988/5045, 98.87% ≥ 80%                |
| `make test`                    | RED    | 1723 s; one nextest timeout at 300.011 s; doctest UNRUN |
| `make markdownlint`            | PASS   | 30 s; 0 issues in 183 files                             |
| `make nixie`                   | PASS   | 1 s; "All diagrams validated successfully!"             |
| `make spelling`                | PASS   | 16 s; `typos.toml` current, no rewrite                  |
| `make test-workflow-contracts` | PASS   | 156 s; 1271 passed, 3 skipped; then 27 passed           |

**The lint blind spot is closed.** All four stages were reached and passed:
`lint-clippy` (`cargo doc`, `cargo clippy`), `lint-whitaker` for both
`netsuke-build` and `test_support` with **zero diagnostics**, then
`lint-workflow-scripts`, and finally `lint-python` through its full cascade —
Ruff, Pylint 10.00/10, the df12 house lints 10.00/10, `ambrleaks` clean, and
`interrogate` at 100%. `yamllint` and `actionlint` both ran and passed. Every
stage that the `1d75efce` abort concealed is now exercised. The stage that ran
1977 s under queue contention took 814 s here with the same content.

**The `make test` red is load attribution, and this run supplies the proof the
earlier ones could only argue.** The timeout has moved to a **third** victim —
`build_module_slice_ui_tests::production_build_module_slice_has_expected_boundary`
at `TIMEOUT [300.011s]`, having grown through `SLOW` at 60/120/180/240 s. All
three victims across the three runs sit in the `nested-cargo-builds` group
(`.config/nextest.toml:99-101`), which spawns child Cargo builds and is
serialized, so its members queue behind one another; a candidate defect cannot
change which member of that group reaches a fixed wall-clock bound.
`command_env_ui_tests::cli_configuration_fixture_compiles` — twice the victim —
does not appear in this run's log, but **it cannot be claimed to have passed**:
`test-nextest` fails fast, and 16 of 3943 rows were never scheduled. Checked
against the runner's own accounting, every passing row prints a `PASS` line
(3926 rows for 3926 reported passes), so an absent row is an unscheduled row.
The census agrees: 256 compile-slot waits with a queue depth reaching **240
ahead**, 1108 sccache-unreachable notices, and zero `build-limits:` admission
failures. The failing row is unchanged from `origin/main`.

**Disposition of this run.** `doctest` remains **UNRUN** (not failed) and 16
nextest rows were never scheduled, because `test-nextest` fails fast at
`Makefile:230`. `make test` is to be re-run once on a quieter host, which
executes `doctest` automatically as its second half. The timeout is not
repaired: `.config/nextest.toml:57-70` documents this signature for this group
and deliberately keeps the budget at five periods rather than "moving to an
allowance the delay can still exceed". EP-M0 is green on every gate whose
verdict the candidate can decide, with `make test`'s doctest half the one
outstanding item.

## Interfaces and dependencies

No new dependencies. At the end of EP-M4 these public items exist, re-exported
from `netsuke::manifest`:

```rust
/// Inputs to one manifest load beyond the YAML text and its name.
pub struct ManifestLoadOptions<'a> { /* private fields */ }

impl<'a> ManifestLoadOptions<'a> {
    /// Load with the complete build stdlib and explicit environment inputs.
    pub fn full(config: StdlibConfig, environment: ManifestEnvironment<'a>) -> Self;
    /// Load with the deny-by-default test surface.
    pub fn test(surface: TestSurface) -> Self;
    /// Supply resource ceilings.
    #[must_use]
    pub fn with_budget_limits(self, limits: ManifestBudgetLimits) -> Self;
    /// Observe loader stages.
    #[must_use]
    pub fn with_stage_observer(self, on_stage: &'a mut dyn FnMut(ManifestLoadStage)) -> Self;
}

/// Load a manifest string with explicit options.
pub fn from_str_with_options(
    yaml: &str,
    name: &ManifestName,
    options: ManifestLoadOptions<'_>,
) -> anyhow::Result<NetsukeManifest>;

/// The capabilities granted to a manifest under `netsuke test`.
pub struct TestSurface { /* config, env_reader: Option<EnvReader>, overlays */ }

impl TestSurface {
    /// Start from a stdlib configuration; `now()` is real only if its clock was injected.
    pub fn new(config: StdlibConfig) -> Self;
    /// Answer `env()` from this reader instead of refusing every lookup.
    #[must_use]
    pub fn with_env_reader(self, reader: EnvReader) -> Self;
    /// Install template overlays.
    #[must_use]
    pub fn with_overlays(self, overlays: TemplateOverlays) -> Self;
}

/// Test-supplied template functions, registered after stdlib and manifest
/// macros and before `foreach` expansion.
#[derive(Clone, Default)]
pub struct TemplateOverlays { /* functions: IndexMap<String, OverlayCallable> */ }

impl TemplateOverlays {
    /// Add or replace a function overlay.
    #[must_use]
    pub fn with_function(self, name: impl Into<String>, callable: OverlayCallable) -> Self;
    /// Report whether no overlay is present.
    pub fn is_empty(&self) -> bool;
}

/// A callable standing in for a template function.
#[derive(Clone)]
pub struct OverlayCallable(/* Arc<dyn Fn(&OverlayCall<'_, '_>) -> Result<Value, Error> + Send + Sync> */);

impl OverlayCallable {
    /// Wrap a dispatch closure.
    pub fn new<F>(dispatch: F) -> Self
    where
        F: Fn(&OverlayCall<'_, '_>) -> Result<minijinja::Value, minijinja::Error>
            + Send
            + Sync
            + 'static;
}

/// One invocation of an overlay; private fields keep it extensible.
pub struct OverlayCall<'call, 'env> { /* state, args, shadowed */ }

impl<'call, 'env> OverlayCall<'call, 'env> {
    /// The template state at the call site.
    pub fn state(&self) -> &'call minijinja::State<'call, 'env>;
    /// Positional arguments, with keyword arguments as a trailing `Kwargs` value.
    pub fn args(&self) -> &'call [minijinja::Value];
    /// The function this overlay replaced, if any.
    pub fn shadowed(&self) -> Option<&'call minijinja::Value>;
}

/// Why a set of overlays was rejected.
#[derive(Debug, thiserror::Error)]
pub enum TemplateOverlayError {
    /// An overlay name is reserved or already used by the manifest.
    #[error("…localized, begins with [netsuke::manifest::overlay::collision]…")]
    NameCollision { name: String, kind: OverlayNameKind },
}

/// What an overlay name collided with.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OverlayNameKind { EnvironmentSeam, Internal, ManifestVariable, ManifestMacro }
```

One further public item is re-exported from `netsuke::stdlib` rather than
`netsuke::manifest`: `TestModeRefusal`, the error source a test load attaches
to every refusal, with `operation()` naming the refused helper. OBL-11's
external integration test downcasts to it, so it is public by requirement; see
`Surprises & discoveries`.

Crate-internal additions: the `StdlibRegistration` variants `AmbientFull` and
`Test(Box<TestSurface>)` and its methods
`records_expansion_and_budget_telemetry` and `classifies_query_refusals`;
`ManifestLoadOptions::{ambient, manifest_query}`;
`query::from_path_with_options`; `loader_options::register_host_helpers`;
`restricted_helpers::{SURFACE, RestrictedSurface, register_for_test}`;
`jinja_macros::query_classification`; and the Fluent keys
`stdlib.test_mode.helper_unavailable` and `manifest.overlay.name_collision`.

Each overlay is installed in `build_template_environment` as follows, where
`shadowed` was read from `Environment::globals()` before registration:

```rust
env.add_function(name, move |state: &State, args: Rest<Value>| {
    callable.call(&OverlayCall::new(state, &args, shadowed.as_ref()))
});
```

## Revision note

2026-09-27, after the expert-panel review. What changed: overlays moved from
the options into a new `TestSurface` payload of the `Test` mode;
`StdlibRegistration` stays crate-private with a named `AmbientFull` variant and
no public `Default`; `env()`/`glob()` registration became a single loader
decision; refusals gained a verbatim bracketed code and a typed source, and the
query classifier is gated to query mode; overlays receive the shadowed function
through an extensible `OverlayCall`; `env` and `__netsuke_*` joined the
reserved overlay names; the crate-private path entry and closure factory
replaced `ManifestLoadMode`; snapshots became canonical with fixed paths and a
no-re-accept rule; OBL-1 was split by visibility, OBL-3 became a source scan,
OBL-5 uses counters and requires every Full load to succeed, OBL-11 walks the
corpus at run time, and a compile-pass fixture pins the public surface; the
behavioural feature shrank to three scenarios landing with their steps; the
corpus reuses `tests/data`; milestones fell from seven to six; tolerances were
recomputed from Table 1; the telemetry predicate was narrowed and moved to an
ADR-0050 addendum (then numbered 018). Why: all three panel reviews returned
"revise" with evidence-backed findings (factual corrections, non-vacuity gaps,
a translation-unsafe marker, a scope ceiling breached by construction, and
forward-compatibility of the overlay API). At that point, the plan still
awaited approval of the recorded deviations and an answer to the open question
on file-reading filters. The user settled that question on 2026-09-27 and
approved the plan and deviations on 2026-10-14.

2026-09-27, after the user's decision on the open question. What changed: the
open question became a `Decision log` entry. Roadmap 7.4.1 now also roots the
`contents`, `size`, `linecount`, `hash`, `digest`, and `realpath` filters at
the sandbox, and technical design §5.5 names them as a third group of ambient
bypasses. Why: the user chose to extend 7.4.1 rather than add an item or accept
permanent refusal. Effect on remaining work: none on the 7.1.2 implementation,
which refuses these filters until 7.4.1 lands; EP-M5 no longer needs to add the
filters to §5.5, and has no remaining roadmap gap. At that point, approval of
the recorded deviations remained pending; the user approved them on 2026-10-14.

2026-10-25, before the requested rebase and resumed implementation. What
changed: the header now says `APPROVED`, recording the user’s explicit
instruction; the existing approval of the listed deviations remains in force.
Why: establish approval before the history rewrite. Effect on remaining work:
the rebase completed onto `origin/main` at `de81d5c3`, and the header now
returns to `IN PROGRESS` as EP-M1 resumes; milestone scope and acceptance
criteria remain unchanged.

2026-10-25, after rebasing. What changed: refreshed obsolete ADR identifiers,
documentation paths, and plan counts; recorded the new rebase and its audit.
Why: the target branch had advanced and moved the ADR layout since the prior
plan revision. Effect on remaining work: accepted implementation scope and
milestones are unchanged; candidate gates remain pending, and EP-M1 is next.
