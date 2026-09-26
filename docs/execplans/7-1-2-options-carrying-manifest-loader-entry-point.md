# Introduce the options-carrying manifest loader entry point (7.1.2)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: DRAFT

This plan must be approved before implementation begins. No production code may
change until the user explicitly approves it. Approval also accepts the
technical-design deviations listed under `Decision log` (entries marked
"Deviation") and should answer the open question recorded there about
file-reading filters under test.

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
  template helper (`src/manifest/env_reader.rs`). `ManifestEnvironment` bundles
  a reader with the `EnvAccessPolicy` evaluated before each read.
- **Clock provider.** The closure type backing `now()`, owned by `StdlibConfig`
  since roadmap 7.1.1 (`src/stdlib/time/clock.rs`).
- **Query classification.** In `netsuke help targets`, a `when` expression that
  calls a disabled helper does not fail; `classify_query_evaluation`
  (`src/manifest/jinja_macros/mod.rs`) recognizes the query refusal and the
  entry is kept with `conditional: true`.
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
as it stands on `main` at commit `ebcedaef`. Line numbers are indicative;
anchor on the named items, because line numbers drift.

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
   fields (`src/manifest/render.rs`, whose `RenderMode::ManifestQuery` skips
   command rendering); validate recipes.

The function body is about 68 lines; `clippy.toml` caps functions at 70 lines
and cognitive complexity at 9, so it must be split before it grows.
`src/manifest/budget_adapter.rs::from_str_named` wraps `evaluate_manifest` and
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
- `manifest_query_operation_error(operation)` in the same file, which builds
  the stub error from a hard-coded English marker
  (`MANIFEST_QUERY_DISABLED_HELPER_MARKER`). The query stubs for `hash`,
  `digest`, and `contents` accept fewer arguments than the real filters, so a
  call with keyword arguments fails on arity instead of refusing;
- `is_manifest_query_disabled_error`, which recognizes that marker by
  substring. `classify_query_evaluation` applies it in *every* mode today, and
  a match turns the `when` result into `WhenResolution::Conditional`: the entry
  is kept and marked `conditional: true` (`src/manifest/expand/mod.rs`);
- `src/manifest/env_reader.rs::disabled_env_reader` (`pub(super)`), a reader
  that answers every lookup with `EnvReadError::NotPresent`.

`src/stdlib/register.rs` is 398 lines and `src/manifest/jinja_macros/mod.rs` is
394 lines; the repository caps code files at 400 lines, so new code in either
area needs an extraction first.

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
bundle, `ManifestBudgetLimits`, the manifest root, and ADR-018's query-mode
telemetry suppression), and its public-field sketch leaves overlays outside the
mode that grants them. `Decision log` records each reconciliation; EP-M5 writes
them back into the design.

### Files a novice will touch

| Path                                                                    | Change                                                                   |
| ----------------------------------------------------------------------- | ------------------------------------------------------------------------ |
| `src/manifest/load_options.rs` (new)                                    | `ManifestLoadOptions`, `from_str_with_options`, host-helper registration |
| `src/manifest/test_surface.rs` (new)                                    | `TestSurface`, `TemplateOverlays`, `OverlayCallable`, `OverlayCall`      |
| `src/manifest/mod.rs`                                                   | `StdlibRegistration::{AmbientFull, Test}`; split `evaluate_manifest`     |
| `src/manifest/budget_adapter.rs`                                        | ask the registration whether to record telemetry                         |
| `src/manifest/parse_with_config.rs`, `path_loaders.rs`                  | thin wrappers over the options entry                                     |
| `src/manifest/query.rs`                                                 | `from_path_with_options`; `ManifestLoadMode` replaced by a factory       |
| `src/manifest/jinja_macros/query_classification.rs` (new)               | classifier extracted and gated on query mode                             |
| `src/manifest/jinja_macros/mod.rs`, `src/manifest/expand/*`             | pass the query-classification flag                                       |
| `src/stdlib/restricted_helpers.rs` (new)                                | surface inventory, query and test refusing stubs                         |
| `src/stdlib/register.rs`, `src/stdlib/mod.rs`                           | delegate restricted registration; `register_for_test`                    |
| `src/stdlib/time/clock.rs`                                              | re-document `WallClock::is_system` as a policy input                     |
| `src/localization/keys.rs`, `locales/*/messages.ftl`                    | two new keys                                                             |
| `src/manifest/tests/*.rs`, `src/stdlib/restricted_helpers_tests.rs`     | unit, inventory, and internal differential tests                         |
| `tests/manifest_load_options_tests.rs`, `tests/manifest_load_options/*` | public differential and property tests                                   |
| `tests/features/manifest_load_options.feature`, `tests/bdd/…`           | behavioural scenarios, steps, and `TestWorld` slots                      |
| `tests/ui/manifest_load_options_embedder_pass.rs`                       | compile-pass fixture pinning the public surface                          |
| `docs/…`                                                                | design, ADR-041, ADR-018 addendum, guides, contents, roadmap             |

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
`docs/rfcs/0007-netsukefile-testing-framework.md`; `docs/netsuke-design.md`;
`docs/adr-008-environment-seam-taxonomy.md`;
`docs/adr-010-scope-glob-capability-to-literal-prefix.md`;
`docs/adr-018-bound-manifest-template-evaluation.md`; `docs/polonius.md`;
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
  EP-M1 onwards `git diff --exit-code` over their directories must pass.

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
  the same change set, and ADR-041 records the substantive decisions.

- Risk: a peer session pushes to this branch or `main` moves.
  Severity: low. Likelihood: low. Mitigation: fetch before every push; after
  any rebase, re-check ADR numbering and rerun every gate.

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
- [ ] User approval of the plan, the listed deviations, and the open question.
- [ ] EP-M0 — characterization baseline.
- [ ] EP-M1 — extractions (restricted helpers, query classifier).
- [ ] EP-M2 — options entry point and thin wrappers.
- [ ] EP-M3 — `StdlibRegistration::Test`.
- [ ] EP-M4 — template overlays.
- [ ] EP-M5 — documentation, ADR, and roadmap closure.

## Surprises & discoveries

- Observation: the technical design's line citations are stale, and the loader
  has grown since the design was written: `ManifestEnvironment`,
  `ManifestBudgetLimits`, the manifest root, a private `ManifestParse` bundle,
  and a second private mode enum, `ManifestLoadMode`, all exist. Evidence:
  `src/manifest/mod.rs`, `src/manifest/query.rs` at `ebcedaef`. Impact:
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
  version, a manifest using them could never be tested. Raised as the open
  question in `Decision log`.

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
  `globals`, `pub(crate) fn get_filter`), `src/value/mod.rs` (`Value::call`).
  Impact: overlays receive the shadowed function now (spy passthrough), and the
  surface inventory must be source-scanned rather than read from the
  environment. EP-M5 corrects design §7.

- Observation: a query refusal inside `when` does not drop an entry; it keeps
  it with `conditional: true`, and the classifier runs in every mode. Evidence:
  `src/manifest/expand/mod.rs` (`WhenResolution::Conditional`),
  `src/manifest/jinja_macros/mod.rs::classify_query_evaluation`. Impact: a
  misclassified test refusal would silently change the manifest; hence the
  query-only gate.

- Observation: loader telemetry other than expansion reports and budget
  exhaustion (template-render and macro metrics, the `env()` lookup counter)
  fires in every mode, including query mode. Evidence:
  `src/manifest/jinja_macros/telemetry.rs`, `src/manifest/env_reader.rs`.
  Impact: "telemetry-free" was too broad; the predicate is named for exactly
  what it gates.

- Observation: `WallClock::is_system` is documented as "recorded for
  diagnostics", and `with_clock(system_clock())` yields `is_system == false`.
  Evidence: `src/stdlib/time/clock.rs`. Impact: test mode treats any clock
  supplied through `with_clock` as explicit injection (C4); the field's
  documentation is updated to say it is a policy input.

## Decision log

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
  without the other; every production load is budgeted (ADR-018); private
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
  the `glob` and file-test stubs with sandbox-rooted adapters. Recorded in
  ADR-041. Date/Author: 2026-09-27, planning agent.

- Decision (Deviation from design §5.5): `which` and `expanduser` refuse under
  test, although §5.5's table implies they run against an empty `PATH` and a
  missing home. Of `StdlibConfig`'s settings, test mode honours only the clock
  in 7.1.2; the workspace root becomes relevant when 7.4.1 roots `glob` and the
  file tests at it. Rationale: `which` with an empty `PATH` yields a misleading
  "not found" rather than a refusal pointing at doubles, and `expanduser`
  exposes a host-shaped answer. Refusal keeps the rule "unmocked impure call is
  an error" uniform. Date/Author: 2026-09-27, planning agent after panel review.

- Decision: test-mode refusals are MiniJinja `InvalidOperation` errors whose
  message is the Fluent key `stdlib.test_mode.helper_unavailable` (argument
  `operation`), beginning with the verbatim code
  `[netsuke::jinja::test_mode::unavailable]` in every catalogue, and whose
  source is a typed `TestModeRefusal { operation }`. Code recognizes them by
  downcasting the source, as `src/manifest/budget_adapter.rs` does for budget
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
  continues to fire in every mode. Recorded as ADR-018 Addendum E. Rationale:
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

- Decision: invariant I7 is discharged here at the loader level: the build
  cannot carry overlays, and every wrapper's canonical output is unchanged. The
  "manifest with a `tests` block" half depends on roadmap 7.2.1's schema field
  and is deferred there. Date/Author: 2026-09-27, planning agent.

- Decision: `ortho_config` is not used, and the users' guide is not changed.
  Rationale: no command-line flag, configuration key, environment variable, or
  user-visible behaviour changes; `netsuke test` wiring (roadmap 7.6) is where
  `ortho_config` layering and localized help apply. The developers' guide gains
  the loader-entry convention instead. Date/Author: 2026-09-27, planning agent.

- Decision: a new ADR, ADR-041, records the deny-by-default test surface and
  the refusal-code contract; telemetry goes to ADR-018 Addendum E; the options
  shape goes to design §4.3 only. Rationale: the surface decision is hard to
  reverse and spans network, process, filesystem, and clock, which ADR-008
  (environment seams) does not cover. ADR-039 and ADR-040 are claimed on
  unmerged branches (`jm5/kani-change-scoped-gate`, `6-1-1-split-rfc-0006-…`);
  re-check at every rebase. Date/Author: 2026-09-27, planning agent after panel
  review.

- Open question for the approver: the six file-reading filters (`contents`,
  `size`, `linecount`, `hash`, `digest`, `realpath`) refuse under test in this
  plan, and filters cannot be doubled in the first version, so manifests using
  them cannot be tested. Options: extend roadmap 7.4.1 to root these filters at
  the sandbox; add a new 7.4.x item; or accept permanent refusal. This plan's
  implementation is the same under all three; only the roadmap text differs. A
  related follow-on, out of scope here (C2): query mode still runs `FILE_TESTS`
  against the real filesystem, and the query stubs for `hash`, `digest`, and
  `contents` reject keyword arguments before refusing. Date/Author: 2026-09-27,
  planning agent.

## Outcomes & retrospective

Not started.

## Conformance basis

Upstream artefacts, at `main` commit `ebcedaef`:

- RFC 0007, `docs/rfcs/0007-netsukefile-testing-framework.md` — scope.
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
- ADR-008 (environment seams), ADR-010 (glob capability scoping), ADR-018
  (bounded template evaluation and query telemetry suppression).
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
  `tests/manifest_load_options/wrapper_equivalence.rs`; the crate-internal
  query entry in `src/manifest/tests/wrapper_equivalence.rs`. Both use
  `insta::Settings` with `set_snapshot_path` to fixed directories
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
  `src/manifest/render.rs`; each must fail a named row. Revert both.

- OBL-2 — mode decision table.
  Statement: for each registration (`AmbientFull`, `Full`, `ManifestQuery`,
  `Test`), the `env()` disposition, `glob()` disposition, render mode,
  `records_expansion_and_budget_telemetry`, `classifies_query_refusals`, and
  "may carry overlays" are exactly as in `Decision log`. Method: exhaustive
  `rstest` table over the four variants. Rationale: finite and small; the table
  is exhaustive, so bounded model checking would add nothing. Artefact:
  `src/manifest/tests/load_options.rs`. Evidence: red (compile failure) before
  EP-M2; green after. Non-vacuity: every row asserts all six outputs; swapping
  the `Test` and `Full` arms of `records_expansion_and_budget_telemetry` fails
  two rows.

- OBL-3 — surface inventory completeness.
  Statement: every helper name registered anywhere under `src/stdlib/` appears
  in `restricted_helpers::SURFACE` with a disposition for query and test mode;
  every refusing entry is registered as a stub under test; no name resolves to
  MiniJinja's unknown-function, -filter, or -test error under test. Method: a
  source-scanning unit test reading `src/stdlib/**/*.rs` (via
  `CARGO_MANIFEST_DIR` and `cap_std`) for `add_function("…"`, `add_filter("…"`,
  and `add_test("…"` literals and the `FILE_TESTS` table, compared with
  `SURFACE`; plus an `rstest` probe per inventory row loading a one-line
  manifest through `from_str_with_options` in test mode. Rationale: MiniJinja
  cannot list filters or tests, so the source is the only complete witness.
  Artefact: `src/stdlib/restricted_helpers_tests.rs`. Evidence: red before
  EP-M3; green after. Non-vacuity: add a scratch `env.add_filter("scratch", …)`
  to `src/stdlib/collections` and confirm the scan fails naming it; delete the
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
  `src/stdlib/restricted_helpers_tests.rs`, `src/manifest/tests/test_mode.rs`,
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
  `src/ninja_gen_property_tests/ninja_oracle.rs`, over a grammar valid by
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
  in ADR-041); with the default clock, `now()` refuses with the test code.
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
  `src/stdlib/register_query_snapshots.rs` (a `#[path]` test child of
  `register.rs`); EP-M1 moves them unchanged into
  `src/stdlib/restricted_helpers_tests.rs`. Evidence: green at EP-M0 and under
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
`src/manifest/tests/budget.rs` already observe the full and query paths; EP-M3
adds one `DebuggingRecorder` row showing a test load emits no expansion or
budget metric.

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
  `src/stdlib/restricted_helpers.rs` behind a private `RestrictedSurface`
  (initially `ManifestQuery` only); `classify_query_evaluation` and
  `QueryEvaluation` live in
  `src/manifest/jinja_macros/query_classification.rs`. Behaviour is
  byte-identical.
- Requirements: RM-c (mechanism made reusable); C10 headroom.
- Acceptance evidence: gates green; C12 diff clean; both source files well
  under 400 lines.
- Conformance check: refactor only; no public change.
- Recovery: revert the refactor commits (one per extraction).
- Remaining gaps: the test surface.
- Compatibility decision: none (private code).

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

- Outcome: design §4.2, §4.3, §5.5, and §7 reconciled; ADR-041 written and
  indexed in `docs/contents.md`; ADR-018 Addendum E; developers' guide,
  `docs/netsuke-design.md`, and `docs/polonius.md` updated; roadmap 7.1.2 and
  its five bullets ticked; this plan set to `COMPLETE`.
- Requirements: design §15; AGENTS.md documentation rules.
- Acceptance evidence: Markdown gates and `make check-fmt` green.
- Conformance check: every deviation in `Decision log` appears in the design or
  an ADR.
- Recovery: revert documentation commits.
- Remaining gaps: the open question's roadmap outcome, as the approver decides.
- Compatibility decision: none.

## Plan of work

### Stage A — orientation (no code changes)

Read the files in `Context and orientation`. Run the full gate set on the
untouched branch through `scrutineer` and record the baseline. Confirm that
ADR-041 is still free by listing `docs/adr-*` on every remote branch.

### Stage B — characterization (EP-M0)

Add the four new fixtures under `tests/data/manifest_load_options/`. Create
`tests/manifest_load_options_tests.rs` declaring `mod manifest_load_options;`
and `tests/manifest_load_options/mod.rs` declaring `wrapper_equivalence` (later
also `fidelity_property` and `corpus_fidelity`), each file under 400 lines. In
`wrapper_equivalence.rs`, write one `rstest` per public entry point over its
applicable fixtures, snapshotting canonical JSON or the error chain with the
insta settings from OBL-1. Path entries copy fixtures into a `tempfile`
directory. Add the crate-internal query rows in
`src/manifest/tests/wrapper_equivalence.rs` and the query-refusal rows in
`src/stdlib/register_query_snapshots.rs`. Review every new snapshot by reading
it, then accept it with `cargo insta accept`; this is the only acceptance the
characterization snapshots ever receive.

### Stage C — implementation (EP-M1 to EP-M4)

EP-M1. Move the query stubs, `manifest_query_operation_error`,
`is_manifest_query_disabled_error`, and the marker from
`src/stdlib/register.rs` to `src/stdlib/restricted_helpers.rs`. Introduce
`const SURFACE: &[HelperEntry]` (name, kind — function, filter, or test — and
dispositions for query and test) and a private
`enum RestrictedSurface { ManifestQuery }` whose stub registration iterates
`SURFACE`, producing today's exact text. Keep
`is_manifest_query_disabled_error` reachable through the existing
`src/stdlib/mod.rs` re-export. In a separate commit, move
`classify_query_evaluation` and `QueryEvaluation` to
`src/manifest/jinja_macros/query_classification.rs`.

EP-M2. Red: add `src/manifest/tests/load_options.rs` (OBL-2 rows for the three
existing modes), the `from_str_with_options` doctest, and the OBL-12 fixture;
observe compile failures. Green: create `src/manifest/load_options.rs` with
`ManifestLoadOptions`, its constructors and builders, `from_str_with_options`,
and `register_host_helpers(env, registration, environment, manifest_root)`,
which registers `env()` and `glob()` per C5. Rename the private
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
`src/stdlib/restricted_helpers.rs` installing the Boolean formatter, the pure
helpers, `now()` when `!config.clock().is_system()`, and the test stubs from
`SURFACE`. Update `WallClock::is_system`'s documentation. Add
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
`macro_substitutions`, overlay name rules), §5.5 (full refusing inventory
including the six file-reading filters and `which`/`expanduser`), and §7
(shadowed-function capture is possible). Write
`docs/adr-041-deny-by-default-test-mode-stdlib.md` (surface, refusal code and
typed source, `is_system` as injection) and index it in `docs/contents.md`. Add
Addendum E to `docs/adr-018-bound-manifest-template-evaluation.md`. Add a
"Manifest loader entry points" subsection to `docs/developers-guide.md` (new
callers use `from_str_with_options`; do not add another `from_path_with_*`
rung; how overlays and the surface inventory work, and that a new stdlib helper
needs a `SURFACE` row). Record `ManifestLoadOptions<'a>` in `docs/polonius.md`
(borrowed `ManifestEnvironment` and stage observer; no clones). Add one
sentence to the manifest-loading section of `docs/netsuke-design.md` naming the
load modes. Tick roadmap 7.1.2 and its bullets. Run `make fmt`, then the
Markdown gates.

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
cargo nextest run --workspace --all-features --lib -E 'test(/manifest::tests::(load_options|test_mode|overlays)/)' \
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

Crate-internal additions: the `StdlibRegistration` variants `AmbientFull` and
`Test(Box<TestSurface>)` and its methods
`records_expansion_and_budget_telemetry` and `classifies_query_refusals`;
`ManifestLoadOptions::{ambient, manifest_query}`;
`query::from_path_with_options`; `load_options::register_host_helpers`;
`restricted_helpers::{SURFACE, RestrictedSurface, register_for_test, TestModeRefusal}`;
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
ADR-018 addendum. Why: all three panel reviews returned "revise" with
evidence-backed findings (factual corrections, non-vacuity gaps, a
translation-unsafe marker, a scope ceiling breached by construction, and
forward-compatibility of the overlay API). Effect on remaining work: the plan
awaits approval of the recorded deviations and an answer to the open question
on file-reading filters.
