# Introduce the options-carrying manifest loader entry point (7.1.2)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: DRAFT

This plan must be approved before implementation begins. No production code may
change until the user explicitly approves it.

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

1. One options-carrying entry point,
   `netsuke::manifest::from_str_with_options`, accepts a `ManifestLoadOptions`
   value naming the stdlib surface, the environment reader, the resource
   budget, a stage observer, and a set of `TemplateOverlays`. Every existing
   entry point becomes a thin wrapper that builds default options and forwards
   to it.
2. The stdlib surface enum `StdlibRegistration` gains a third variant, `Test`,
   beside `Full` and `ManifestQuery`. A test load renders pure helpers exactly
   as a build does, reads `now()` only from an injected clock, reads `env()`
   only from an injected reader (refusing every lookup by default), and refuses
   every other host-observing helper with a localized diagnostic that names the
   helper.
3. A caller can register *template overlays* — named callables that replace
   same-named template functions — after the stdlib and manifest macros are
   registered and before `foreach` expansion runs.

A novice can observe success as follows. A new test loads a manifest whose
`foreach` iterates `glob('src/*.c')`, first in test mode with no overlay (the
load fails with a diagnostic naming `glob` and saying it is unavailable under
`netsuke test`), then with an overlay for `glob` returning `["a.c", "b.c"]`
(the load succeeds and yields targets `a.o` and `b.o`). A differential property
test generates hundreds of pure manifests and shows that build mode and test
mode produce byte-identical Ninja output for every one. Every pre-existing
snapshot and behavioural test passes unchanged, which demonstrates that the
build path did not move.

There is no user-visible change. No command-line flag, configuration key, or
output changes. `netsuke test` itself arrives in later roadmap items.

## Definitions

Terms used throughout, defined here so that no prior reading is required.

- **Manifest loader.** The code under `src/manifest/` that parses YAML,
  builds a MiniJinja environment, expands `foreach`/`when`, deserializes into
  `NetsukeManifest` (`src/ast/mod.rs`), and renders string fields.
- **Stdlib.** Netsuke's library of Jinja functions, filters, and tests,
  registered into a MiniJinja `Environment` by `src/stdlib/register.rs`.
- **Stdlib surface / registration mode.** Which stdlib helpers are real,
  which are refusing stubs, and what configuration backs them. Selected by the
  private enum `StdlibRegistration` in `src/manifest/mod.rs`.
- **Refusing stub.** A helper registered under its real name whose only
  behaviour is to raise a MiniJinja `InvalidOperation` error naming itself.
  Refusing stubs beat leaving a helper unregistered: under strict-undefined
  MiniJinja an unregistered helper produces a generic "unknown function" error,
  whereas a stub produces a located, actionable diagnostic.
- **Template overlay.** A caller-supplied callable registered as a MiniJinja
  global function *after* the stdlib and manifest macros, so it replaces any
  same-named function. Test doubles (roadmap 7.3.1) will be delivered as
  overlays.
- **Environment reader (`EnvReader`).** The closure type backing the `env()`
  template helper (`src/manifest/env_reader.rs`). `ManifestEnvironment` bundles
  a reader with the `EnvAccessPolicy` evaluated before each read.
- **Clock provider.** The closure type backing `now()`, owned by
  `StdlibConfig` since roadmap 7.1.1 (`src/stdlib/time/clock.rs`).
- **Differential test.** A test that runs two code paths over the same input
  and asserts their outputs are identical, so neither path needs a handwritten
  expected value.
- **Characterization snapshot.** An `insta` snapshot recorded from the
  *unchanged* code before a refactor, used afterwards as the oracle proving the
  refactor preserved behaviour.
- **Driving port / driven port.** Hexagonal-architecture vocabulary. A driving
  port is how callers invoke the domain (here, the loader entry point); a
  driven port is what the domain needs from the outside (here, the environment
  reader, clock, and template helpers).

## Context and orientation

Assume no prior knowledge of this repository. This section describes the code
as it stands on `main` at commit `ebcedaef`. Line numbers are indicative;
anchor on the named items, because line numbers drift.

### The loader core

`src/manifest/mod.rs` owns the private function `evaluate_manifest`, which is
the only place a manifest is evaluated. It receives a private bundle,
`ManifestParse`, holding the diagnostic name, an `Option<StdlibRegistration>`,
the environment reader and access policy, an optional manifest root (anchoring
relative `glob()` patterns), an optional expansion-report observer, and
resource-budget limits. Its order of work is:

1. notify `ManifestLoadStage::InitialYamlParsing`; charge the source budget;
   parse YAML with `serde_saphyr` into a `serde_json::Value` tree;
2. create a MiniJinja `Environment` with strict undefined behaviour;
3. register `env()` over the injected reader and policy, and `glob()` over
   the manifest root;
4. register the stdlib according to `StdlibRegistration`:
   `Full(config)` calls `stdlib::register_with_config`, `ManifestQuery` calls
   `stdlib::register_manifest_query`, and `None` calls `stdlib::register`
   (which builds an ambient configuration from the current directory);
5. register manifest `vars` as globals (`src/manifest/registration.rs`);
6. notify `TemplateExpansion`; register manifest macros
   (`src/manifest/jinja_macros/mod.rs`);
7. run `expand_foreach_with_budget`, which evaluates `foreach` and `when`
   against the raw value tree;
8. notify `FinalRendering`; deserialize into `NetsukeManifest`; render
   string fields (`src/manifest/render.rs`, whose `RenderMode::ManifestQuery`
   skips command rendering); validate recipes.

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
`from_path_with_policy_and_environment_and_limits`. The last is the only
production caller's choice (`src/runner/generation.rs`,
`load_manifest_for_build_with_limits`). Two of the seven have no callers at
all. Each path loader ends in
`src/manifest/query.rs::from_path_with_registration`, which opens a
capability-scoped workspace, reads the file, and builds a `StdlibRegistration`
from a second, private enum, `ManifestLoadMode` (`Full(NetworkPolicy)` or
`ManifestQuery`). `query.rs` also exposes the crate entry
`from_path_for_manifest_query_with_limits`, used by `netsuke help targets`
(`src/runner/generation.rs::load_manifest_with_limits`).

### The restricted-load precedent

`netsuke help targets` established a side-effect-free load. Its pieces are:

- `src/stdlib/register.rs::register_manifest_query`, which registers lexical
  path filters, collection filters, and clock-independent time helpers, then
  registers refusing stubs for `env`, `glob`, `fetch`, `shell`, `grep`,
  `contents`, `which` (function and filter), `command_available`, `now`,
  `realpath`, `expanduser`, `size`, `linecount`, `hash`, and `digest`;
- `manifest_query_operation_error(operation)` in the same file, which builds
  the stub error from a hard-coded English marker
  (`MANIFEST_QUERY_DISABLED_HELPER_MARKER`);
- `is_manifest_query_disabled_error`, which recognizes that marker. It is
  consumed by `src/manifest/jinja_macros/mod.rs::classify_query_evaluation`,
  which turns a disabled-helper error during `when` evaluation into
  `QueryEvaluation::QueryDisabled` instead of failing;
- `src/manifest/env_reader.rs::disabled_env_reader`, a reader that answers
  every lookup with `EnvReadError::NotPresent`.

`src/stdlib/register.rs` is 398 lines long; the repository caps code files at
400 lines, so any new registration code must live elsewhere.

### What the full stdlib touches on the host

`register_with_config` installs a legacy Boolean formatter (so `true` renders as
`true`, not `True`), file tests (`dir`, `file`, `symlink`, and others) that
open parent directories with ambient authority
(`src/stdlib/path/fs_utils.rs::parent_dir`), path filters of which `realpath`,
`size`, `contents`, `linecount`, `hash`, and `digest` read the *ambient*
filesystem relative to the process working directory
(`src/stdlib/path/filters.rs`), `expanduser` (reads the configured home),
`which` and `command_available` (search `PATH`, which falls back to the host
`PATH` when `StdlibConfig::path_override` is unset), `now()` (reads the
configured clock), `fetch` (network), and `shell`/`grep` (spawn processes). The
manifest query mode does *not* install the Boolean formatter, which is one
reason a query render is not a faithful build render.

### The governing design

`docs/netsuke-test-framework-technical-design.md` §4.2 and §4.3 specify this
task. The design predates several loader changes (the `ManifestEnvironment`
bundle, `ManifestBudgetLimits`, the manifest root, and the query-mode telemetry
suppression recorded in ADR-018), so its sketch of `ManifestLoadOptions` must
be reconciled with the current code; `Decision log` records each reconciliation
and milestone EP-M6 writes them back into §4.3.

### Files a novice will touch

| Path                                                               | Change                                                                      |
| ------------------------------------------------------------------ | --------------------------------------------------------------------------- |
| `src/manifest/load_options.rs` (new)                               | `ManifestLoadOptions`, public `StdlibRegistration`, `from_str_with_options` |
| `src/manifest/overlays.rs` (new)                                   | `TemplateOverlays`, `OverlayCallable`, overlay registration and checks      |
| `src/manifest/mod.rs`                                              | route `evaluate_manifest` through the options; register overlays            |
| `src/manifest/budget_adapter.rs`                                   | ask the registration whether to record telemetry                            |
| `src/manifest/parse_with_config.rs`, `path_loaders.rs`, `query.rs` | become thin wrappers over the options entry                                 |
| `src/manifest/env_reader.rs`                                       | widen `disabled_env_reader` to `pub(crate)`                                 |
| `src/stdlib/restricted_helpers.rs` (new)                           | query and test refusing stubs, extracted from `register.rs`                 |
| `src/stdlib/register.rs`                                           | delegate restricted registration; add `register_for_test`                   |
| `src/stdlib/mod.rs`                                                | re-exports                                                                  |
| `src/localization/keys.rs`, `locales/*/messages.ftl`               | test-mode refusal and overlay-collision messages                            |
| `src/manifest/tests/*.rs`, `tests/*.rs`, `tests/features/*`        | unit, property, differential, and behavioural tests                         |
| `docs/…`                                                           | design, ADR, developers' guide, Polonius register, roadmap                  |

*Table 1: Files in scope.*

### The integration-test wiring contract

`tests/integration_test_wiring_tests.rs` fails if a `tests/*/mod.rs` tree is
not declared from some top-level `tests/*.rs` file, or if Cargo's discovered
test targets differ from the `tests/*.rs` files on disk. A new top-level
`tests/manifest_load_options_tests.rs` is automatically a target; any
subdirectory module it uses must be declared with `mod`. The behavioural suite
is `tests/bdd_tests.rs`, whose `scenarios!("tests/features", …)` collects
*every* `.feature` file in the directory.

### Relevant documentation and skills

Read before starting: `AGENTS.md`;
`docs/netsuke-test-framework-technical-design.md` §§4, 5, 12–14;
`docs/netsuke-test-framework-ux-design.md` §§7–8 (seams and what can be mocked);
`docs/rfcs/0007-netsukefile-testing-framework.md`; `docs/netsuke-design.md`
(compiler pipeline); `docs/adr-008-environment-seam-taxonomy.md`;
`docs/adr-010-scope-glob-capability-to-literal-prefix.md`;
`docs/adr-018-bound-manifest-template-evaluation.md`; `docs/polonius.md`;
`docs/rust-testing-with-rstest-fixtures.md`; `docs/rstest-bdd-users-guide.md`;
`docs/rust-doctest-dry-guide.md`;
`docs/reliable-testing-in-rust-via-dependency-injection.md`;
`docs/snapshot-testing-in-netsuke-using-insta.md`;
`docs/localization-styleguide.md`; `docs/translators-guide.md`;
`docs/developers-guide.md`; `docs/documentation-style-guide.md`;
`docs/ortho-config-users-guide.md` (for the reason recorded in `Decision log`
that this task adds no configuration surface); and the completed plan
`docs/execplans/7-1-1-clock-provider-seam.md`, which introduced the clock seam
this plan consumes.

Skills to load: `rust-router` first, then `rust-types-and-apis` (options
struct, public enum), `nll-to-polonius` (the borrowed `ManifestEnvironment` and
stage observer inside the options), `rust-unit-testing` (rstest tables,
googletest matchers), `proptest` (differential generators), `rust-errors`
(localized refusal errors), `hexagonal-architecture` (keeping capability policy
out of adapters), `execplans` (maintaining this document), and `en-gb-oxendict`
for prose.

## Constraints

These are hard invariants. Violating one requires escalation, not a workaround.

- C1. The build path's observable behaviour must not change. Every
  pre-existing test, snapshot (`tests/snapshots/**`, `src/snapshots/**`), and
  behavioural scenario must pass without modification. A changed pre-existing
  snapshot is a stop condition, not something to accept.
- C2. `netsuke help targets` behaviour must not change, including the exact
  text of its refusal message, which `is_manifest_query_disabled_error` matches
  by substring.
- C3. Existing public entry points keep their exact signatures and semantics
  and become thin wrappers; none is removed or renamed in this task, even the
  two without callers.
- C4. Test mode never consults ambient state: no host environment variable,
  no system clock, no ambient filesystem path, no network, and no process spawn
  is reachable from a test-mode load unless a caller explicitly injects it (an
  environment reader, a clock provider, or an overlay).
- C5. Capabilities for a load mode are decided in one place,
  `StdlibRegistration`. No parallel mode enum may decide which helpers are real.
  (`ManifestLoadMode` may survive only as a private factory that *produces* a
  `StdlibRegistration` for path loads.)
- C6. User-facing refusal text for test mode goes through Fluent
  (`src/localization/keys.rs` and every `locales/*/messages.ftl` catalogue).
- C7. No new crate dependency. `indexmap`, `minijinja`, `proptest`, `insta`,
  `googletest`, `pretty_assertions`, `rstest`, and `rstest-bdd` are already
  present.
- C8. No in-process environment mutation in tests (`AGENTS.md`); use
  injected readers.
- C9. The overlay hook is compiled unconditionally, not behind `cfg(test)`,
  because the test runner is production code of the shipped binary (technical
  design §4.3).
- C10. No code file exceeds 400 lines; every module starts with `//!`; every
  function carries `///` documentation.
- C11. Respect `POLONIUS(...)` and `POLONIUS-REFUSED(...)` tags; do not add
  defensive clones to satisfy the borrow checker without first compiling the
  borrowing form.

## Tolerances (exception triggers)

- Scope: stop and escalate if the change touches more than 30 files
  excluding the 35 locale catalogues and `.snap` files, or exceeds 1,800 net
  added lines excluding locale catalogues and snapshots.
- Interface: stop if any *existing* public signature must change, or if a new
  public item beyond those named in `Interfaces and dependencies` seems
  necessary.
- Dependencies: stop if any new crate (including a dev-dependency) seems
  necessary.
- Behaviour: stop if any pre-existing snapshot, doctest, or behavioural
  scenario needs editing to pass (C1, C2).
- Iterations: stop if a gate still fails after three fix attempts for the
  same root cause.
- Design: stop and set `Status: BLOCKED` if implementation evidence shows the
  technical design's registration order (§4.3 steps 1–6) cannot be followed, or
  if MiniJinja does not replace a same-named global on re-registration.
- Ambiguity: stop and present options if a helper's classification (real or
  refusing under test) is not settled by `Decision log`.

## Risks

- Risk: Converting entry points to wrappers subtly changes error ordering or
  text, for example by constructing an ambient `StdlibConfig` before YAML
  parsing where today it is built lazily after parsing. Severity: high.
  Likelihood: medium. Mitigation: keep the registration optional internally so
  the ambient default is still resolved at step 4; characterization snapshots
  of error text, recorded before any production edit (EP-M0), catch drift.

- Risk: A test-mode refusal is mistaken for a query-mode refusal by
  `classify_query_evaluation`, silently dropping a `when` entry instead of
  failing. Severity: high. Likelihood: medium (the easy implementation reuses
  the query marker). Mitigation: test mode uses a distinct marker and message
  key; obligation OBL-4 asserts that no test-mode refusal satisfies
  `is_manifest_query_disabled_error`.

- Risk: `src/stdlib/register.rs` is at 398 lines.
  Severity: medium. Likelihood: certain. Mitigation: EP-M1 extracts restricted
  registration into `src/stdlib/restricted_helpers.rs` as a
  behaviour-preserving refactor before any feature work.

- Risk: Adding Fluent keys to 35 catalogues is error-prone.
  Severity: medium. Likelihood: medium. Mitigation: follow
  `docs/translators-guide.md`; the existing `tests/locale_catalogue_tests.rs`
  and `tests/localization_tests.rs` parity checks fail on a missing or
  malformed key.

- Risk: The differential property generator produces mostly invalid
  manifests, making the Full-versus-Test comparison vacuous. Severity: medium.
  Likelihood: medium. Mitigation: generate from a grammar that is valid by
  construction, record `prop_assert` classification counts, and require that at
  least 90 per cent of cases load successfully (OBL-5 non-vacuity).

- Risk: A function overlay named like a manifest macro only half-shadows it,
  because `register_macro` also adds an `{% from … import … %}` line to every
  rendered template, and template-local imports beat globals. Severity: medium.
  Likelihood: high if unguarded. Mitigation: reject overlay names that collide
  with a manifest macro or variable (OBL-8); macro substitution is roadmap
  7.1.3's separate mechanism.

- Risk: The technical design's field-level sketch diverges from the
  reconciled shape, confusing later roadmap items. Severity: low. Likelihood:
  certain. Mitigation: EP-M6 updates §4.3 in the same change set (design §15
  requires it) and ADR-041 records the substantive decisions.

- Risk: A peer session pushes to this branch, or `main` moves, during
  implementation. Severity: low. Likelihood: low. Mitigation: re-check
  `git log origin/<branch>` before every push; rebase deliberately and rerun
  every gate afterwards.

## Progress

- [x] (2026-09-27) Renamed the branch to
  `7-1-2-options-carrying-manifest-loader-entry-point` and set its upstream.
- [x] (2026-09-27) Reconnaissance of the loader, query mode, stdlib
  registration, and test infrastructure.
- [x] (2026-09-27) Drafted this ExecPlan.
- [ ] Expert-panel review and revision of the draft.
- [ ] User approval of the plan.
- [ ] EP-M0 — characterization baseline.
- [ ] EP-M1 — extract restricted helper registration.
- [ ] EP-M2 — options entry point and thin wrappers.
- [ ] EP-M3 — `StdlibRegistration::Test`.
- [ ] EP-M4 — template overlays.
- [ ] EP-M5 — differential fidelity and behavioural suites.
- [ ] EP-M6 — documentation, ADR, and roadmap closure.

## Surprises & discoveries

- Observation: The technical design's line citations are stale, and the
  loader has grown since the design was written: `ManifestEnvironment`,
  `ManifestBudgetLimits`, the manifest root, a private `ManifestParse` bundle,
  and a second private mode enum, `ManifestLoadMode`, all exist. Evidence:
  `src/manifest/mod.rs`, `src/manifest/query.rs` at `ebcedaef`. Impact:
  `ManifestLoadOptions` must carry the environment bundle and budget limits, and
  `ManifestLoadMode` must be demoted to a factory (C5).

- Observation: `realpath`, `size`, `contents`, `linecount`, `hash`, and
  `digest` read the ambient filesystem relative to the process working
  directory, not through `StdlibConfig`'s workspace root. The technical design
  §5.5 names only `glob()` and the file tests as ambient bypasses, and roadmap
  7.4.1 provides sandbox adapters for only those two. Evidence:
  `src/stdlib/path/filters.rs::register_filters`,
  `src/stdlib/path/path_utils.rs::current_dir_utf8`. Impact: test mode must
  refuse all six filters (C4). Whether they later gain sandbox adapters is a
  roadmap gap to raise with the user; this plan does not widen 7.4.1.

- Observation: manifest-query registration does not install the legacy
  Boolean formatter that `register_with_config` installs. Evidence:
  `src/stdlib/register.rs`. Impact: test mode cannot simply reuse query
  registration; it must install the formatter, or Boolean interpolation would
  differ from a build and break invariant I4.

- Observation: MiniJinja 2.24.0 `add_function` is `add_global` with a function
  value, and `add_global` is a map insert, so a later registration replaces an
  earlier one of the same name. Functions and globals share one namespace.
  Evidence: `minijinja-2.24.0/src/environment.rs`, `add_function` and
  `add_global`. Impact: overlays shadow stdlib functions by registration order
  alone, and can equally shadow manifest `vars`, which motivates the collision
  check.

## Decision log

- Decision: `ManifestLoadOptions` carries a `ManifestEnvironment` (reader plus
  access policy) instead of the design's bare `Option<EnvReader>`. Rationale:
  the codebase bundles reader and policy deliberately so that no caller injects
  a reader without also deciding the access policy
  (`src/manifest/env_reader.rs`). The field is optional; `None` selects the
  mode's default (Full: process reader and default policy; ManifestQuery and
  Test: `disabled_env_reader`). Date/Author: 2026-09-27, planning agent.

- Decision: `ManifestLoadOptions` also carries `ManifestBudgetLimits`, which
  the design predates. Rationale: every production load is budgeted (ADR-018);
  an options entry that could not set limits would push callers back to the
  wrapper zoo. Date/Author: 2026-09-27, planning agent.

- Decision: `ManifestLoadOptions` fields are private, set through a
  constructor taking the registration plus `with_*` builder methods, rather
  than the design's public fields. Rationale: private fields let the type grow
  (7.1.3 adds macro substitutions) without breaking struct-literal callers, and
  keep the mode-dependent defaults in one constructor. The difference is
  mechanical and is written back into design §4.3. Date/Author: 2026-09-27,
  planning agent.

- Decision: the registration is `Option<StdlibRegistration>` internally, and
  `ManifestLoadOptions::default()` means "the ambient full stdlib, resolved at
  registration time". Rationale: `from_str` and `from_str_with_env_and_policy`
  today register the ambient stdlib lazily, after YAML parsing; building a
  `StdlibConfig` eagerly in the wrapper would reorder errors (a malformed
  manifest in an unreadable working directory would report the directory, not
  the YAML). Preserving the lazy default keeps C1 without a special case.
  Date/Author: 2026-09-27, planning agent.

- Decision: `TemplateOverlays` ships with the `functions` map only;
  `macro_substitutions` is added by roadmap 7.1.3. Rationale: design §5.4 shows
  that a macro substitution must rewrite the `MACRO_IMPORTS_GLOBAL` prelude as
  well as the global, which is exactly what 7.1.3 spikes. Adding an
  unimplemented field here would be dead surface; the private-field shape above
  makes the later addition non-breaking. Date/Author: 2026-09-27, planning
  agent.

- Decision: the options entry point is string-based,
  `from_str_with_options(yaml, name, options)`. Path loads keep their private
  `from_path_with_registration`, which now builds a `ManifestLoadOptions` after
  opening the workspace. No public `from_path_with_options` is added.
  Rationale: a path load's `StdlibConfig` is derived from the workspace it
  opens (root directory and path), so a caller cannot supply it up front; and a
  test-mode load must never open an ambient path (C4), so the test runner
  (7.5.1) will read the subject manifest from its sandbox and call the string
  entry. `ManifestLoadMode` survives as a private factory producing a
  `StdlibRegistration` (C5). Date/Author: 2026-09-27, planning agent.

- Decision: test mode's surface is deny-by-default. Real under test: lexical
  path filters, collection filters, `timedelta`, the legacy Boolean formatter,
  `env()` over the effective reader, and `now()` *only* when the `StdlibConfig`
  carries an injected clock. Refusing under test: `glob`, `fetch`, `shell`,
  `grep`, `contents`, `realpath`, `expanduser`, `size`, `linecount`, `hash`,
  `digest`, `which` (function and filter), `command_available`, every file
  test, and `now()` with the system clock. Rationale: UX design §1 promises "no
  network, no wall-clock dependency, no mutation of the project root";
  technical design C4 says an unmocked impure call is an error. `which`/
  `command_available` fall back to the host `PATH` unless overridden, and the
  six file-reading filters and file tests read the ambient filesystem. Function
  helpers (`glob`, `which`, `fetch`, `command_available`, `now`) can still be
  replaced by overlays, which is how the UX design's doubles will reach them;
  filters and Jinja tests are not mockable in the first version (UX design
  §8.6). Roadmap 7.4.1 later replaces the `glob` and file-test stubs with
  sandbox-rooted adapters. Recorded as ADR-041. Date/Author: 2026-09-27,
  planning agent.

- Decision: test-mode loads are telemetry-free, like manifest queries: no
  expansion-report metrics and no budget-exhaustion metric. Rationale: those
  counters are described as "normal manifest loading" metrics
  (`src/manifest/loading.rs`); counting test runs would distort usage evidence.
  The decision is expressed as one predicate,
  `StdlibRegistration::records_load_telemetry`, replacing both
  `matches!(…, ManifestQuery)` sites. Recorded in ADR-041 and cross-referenced
  from ADR-018's context. Date/Author: 2026-09-27, planning agent.

- Decision: test mode renders with `RenderMode::Full`.
  Rationale: `RenderMode::ManifestQuery` skips command rendering; a test must
  see exactly what a build renders (I4). Date/Author: 2026-09-27, planning
  agent.

- Decision: generalize the refusal *mechanism* but keep two markers. The
  query marker text stays byte-identical (C2); test mode gets its own Fluent
  key, `stdlib.test_mode.helper_unavailable`, with an `operation` argument, and
  its own stable marker so the two are never confused. Rationale: roadmap 7.1.2
  says "reuse the `manifest_query_operation_error` diagnostic shape"; design
  §4.2 says "reuses the mechanism with its own message keys". Localizing the
  existing query message is out of scope. Date/Author: 2026-09-27, planning
  agent.

- Decision: overlay names that collide with a manifest `vars` key or a
  manifest macro name fail the load with a localized structural error.
  Rationale: see the half-shadow risk. Overlays are for callables; a collision
  is an authoring error that would otherwise behave differently in `foreach`
  expressions and rendered fields. Date/Author: 2026-09-27, planning agent.

- Decision: overlays are accepted in every registration mode, as the design's
  "ordinary parameter" wording requires; no production caller passes any.
  Rationale: restricting overlays to `Test` would be a design deviation, and
  7.1.3's spike needs overlays over a `Full` registration. Build-path
  neutrality is shown by OBL-1 and OBL-9 rather than by a type restriction.
  Date/Author: 2026-09-27, planning agent.

- Decision: 7.1.2 builds the differential harness over a curated fixture
  corpus under `tests/data/manifest_load_options/`; running it over `examples/`
  and recording the evidence in RFC 0007 remains roadmap 7.1.4. Rationale:
  keeps this task's scope to the seam and avoids pre-empting the dogfooding
  item. The harness takes a corpus directory so 7.1.4 only points it elsewhere.
  Date/Author: 2026-09-27, planning agent.

- Decision: invariant I7 is discharged here at the loader level only: the
  production build entry routes through `from_str_with_options` with empty
  overlays and unchanged outputs. The "manifest with a `tests` block" half of
  I7 depends on the schema field added by roadmap 7.2.1 and is deferred there.
  Rationale: there is no `tests` block to be neutral about yet. Date/Author:
  2026-09-27, planning agent.

- Decision: `ortho_config` is not used.
  Rationale: this task adds no command-line flag, configuration key, or
  environment variable; `netsuke test` wiring (roadmap 7.6) is where
  `ortho_config` layering and localized help apply. Date/Author: 2026-09-27,
  planning agent.

- Decision: no users' guide change.
  Rationale: no user-visible behaviour changes. The developers' guide gains the
  loader-entry convention instead. Date/Author: 2026-09-27, planning agent.

## Outcomes & retrospective

Not started.

## Conformance basis

Upstream artefacts, at `main` commit `ebcedaef`:

- RFC 0007, `docs/rfcs/0007-netsukefile-testing-framework.md` — scope.
- Technical design, `docs/netsuke-test-framework-technical-design.md`
  (status Draft): TD-4.2 (extend `StdlibRegistration`; reuse the refusal
  mechanism and `disabled_env_reader`), TD-4.3 (options struct, overlays,
  registration order steps 1–6, unconditional compilation), TD-5.2 (clock owned
  by `StdlibConfig`), TD-5.5 (refusing stubs under test), TD-I4 (semantic
  fidelity), TD-I5 (no ambient environment, network, or execution), TD-I7
  (build-path neutrality).
- UX design, `docs/netsuke-test-framework-ux-design.md`: UX-1 (deterministic
  by default), UX-8.6 (what can be mocked).
- Roadmap item 7.1.2, `docs/roadmap.md`: RM-a (options and overlays, wrappers),
  RM-b (`Test` variant), RM-c (reuse diagnostic shape and
  `disabled_env_reader`), RM-d (registration order), RM-e (differential tests
  for I4 and I7).
- ADR-008 (environment seams), ADR-010 (glob capability scoping), ADR-018
  (bounded template evaluation and query telemetry suppression).
- Governing standards: `AGENTS.md`, `docs/documentation-style-guide.md`.

There is no terms-of-reference document for phase 7; RFC 0007 plays that role.

Trace chains:

```plaintext
RM-a, TD-4.3  -> EP-M2 -> OBL-1 (wrapper equivalence), OBL-2 (mode table)
RM-b, TD-4.2  -> EP-M3 -> OBL-3 (surface completeness), OBL-4 (refusals)
RM-c, TD-4.2  -> EP-M1, EP-M3 -> OBL-4, OBL-6 (env default), OBL-10 (query unchanged)
UX-1, TD-5.2  -> EP-M3 -> OBL-7 (clock)
RM-d, TD-4.3  -> EP-M4 -> OBL-8 (collisions), OBL-11 (ordering)
RM-e, TD-I4   -> EP-M3, EP-M5 -> OBL-5 (Full/Test fidelity), OBL-12 (corpus fidelity-or-refusal)
RM-e, TD-I7   -> EP-M2, EP-M4 -> OBL-1, OBL-9 (overlay neutrality)
TD-I5 (part)  -> EP-M3 -> OBL-4, OBL-6, OBL-7
```

TD-I5 is advanced, not discharged: roadmap 7.5.1 owns the end-to-end I5 tests
through pipeline actions.

## Verification plan

### Axioms (assumed, not verified)

- AXIOM-1. MiniJinja 2.24.0 replaces a global of the same name on
  `add_function`/`add_global` (source-verified; OBL-11 also exercises it).
- AXIOM-2. MiniJinja resolves globals when a template evaluates, not when it is
  compiled, so any global registered before `expand_foreach_with_budget` is
  visible to every `foreach`, `when`, and rendered field.
- AXIOM-3. A closure of shape `Fn(&State, Rest<Value>) -> Result<Value, Error>`
  receives every positional argument, and keyword arguments as a trailing
  `Kwargs` value (`minijinja::value::Rest`).
- AXIOM-4. `serde_json::Value` equality and `ninja_gen::generate` output are
  deterministic for equal inputs.
- AXIOM-5. `proptest` and `insta` behave as documented.

### Invariants and lemmas

- OBL-1 — wrapper equivalence.
  Statement: for every pre-existing public entry point and every corpus
  manifest, the result after the refactor (the serialized `NetsukeManifest` on
  success, or the full error chain text on failure) equals the result before
  it. Method: characterization `insta` snapshots recorded in EP-M0 from the
  unchanged code, re-run after EP-M2, EP-M3, and EP-M4. Rationale: the
  pre-refactor code is the only trustworthy oracle for "did not change"; a
  post-refactor comparison between wrapper and core would be equal by
  construction and so vacuous. Domain: `from_str`, `from_str_with_env`,
  `from_str_with_env_and_policy`, `from_str_with_env_and_config`, `from_path`,
  `from_path_with_policy_and_environment_and_limits`, and
  `from_path_for_manifest_query_with_limits`, over the curated corpus (pure,
  macro-using, `foreach`/`when`, Boolean interpolation, `env()` with an
  injected reader, a malformed YAML file, a budget-exhausting file, and a
  query-refused helper). Artefact: `tests/manifest_load_options_tests.rs` module
  `wrapper_equivalence`, snapshots under
  `tests/snapshots/manifest_load_options/`. Evidence: `make test` green at
  EP-M0 (snapshots accepted); green again at EP-M2 with zero snapshot changes.
  Non-vacuity: seeded fault in EP-M2 — temporarily drop
  `register_legacy_boolean_formatter` from the full path and confirm the
  Boolean snapshot fails; temporarily route `from_str` through
  `RenderMode::ManifestQuery` and confirm the command snapshot fails. Revert
  both.

- OBL-2 — mode decision table.
  Statement: for each registration (`Full`, `ManifestQuery`, `Test`, and the
  ambient default) and each environment choice (`None`, `Some`), the effective
  environment reader, render mode, telemetry flag, and refusal classifier are
  exactly those in `Decision log`. Method: exhaustive `rstest` table over the
  eight combinations, asserting on the small pure query methods on
  `StdlibRegistration` and `ManifestLoadOptions`. Rationale: the domain is
  finite and small; a table is exhaustive, so bounded model checking would add
  nothing. Artefact: `src/manifest/tests/load_options.rs`. Evidence: fails to
  compile before EP-M2 (red), passes after. Non-vacuity: every row asserts all
  four outputs; a mutation swapping the `Test` and `Full` telemetry arms fails
  two rows.

- OBL-3 — test surface completeness.
  Statement: every function, filter, and test name registered by the full
  stdlib is also registered under test mode, either as the real helper or as a
  refusing stub; none resolves to MiniJinja's unknown-function, -filter, or
  -test error. Method: an `rstest` table over the stdlib inventory, probing
  each name by rendering a minimal template in both modes; plus a set
  comparison showing the full registration's `Environment::globals()` names are
  a subset of the test registration's. Rationale: a missing stub silently
  changes a refusal into a generic error; the inventory makes each name an
  explicit row. Artefact: `src/stdlib/restricted_helpers_tests.rs` (declared as
  a `#[path]` child, following the Whitaker test-module split convention).
  Evidence: red before EP-M3 (test mode absent), green after. Non-vacuity:
  delete one stub (for example `digest`) and confirm its row fails with
  `UnknownFilter`; the set comparison fails if a new stdlib function is added
  without a test-mode decision.

- OBL-4 — test-mode refusals are located, named, localized, and not
  query-classified. Statement: each refusing helper under test raises
  `InvalidOperation` whose text contains the helper name and the localized
  `stdlib.test_mode.helper_unavailable` message, and
  `is_manifest_query_disabled_error` returns `false` for it; a refusal inside a
  `when` expression fails the load rather than filtering the entry. Method:
  `rstest` over the refusing set; one `insta` snapshot of the en-US message for
  a representative helper; one behavioural scenario. Artefact:
  `src/stdlib/restricted_helpers_tests.rs`, `src/manifest/tests/test_mode.rs`,
  `tests/features/manifest_load_options.feature`. Evidence: red before EP-M3,
  green after. Non-vacuity: seeded fault — build the test stub with the query
  marker and confirm the `when` case wrongly succeeds and the classifier
  assertion fails.

- OBL-5 — Full/Test fidelity on pure manifests (loader-level I4).
  Statement: for every manifest that uses only helpers real under test and a
  fixed clock injected into both configurations, the `NetsukeManifest`,
  `BuildGraph`, and Ninja text from `Full` equal those from `Test`. Method:
  `proptest` over a grammar of valid manifests: one to four targets, optional
  `vars`, one optional macro, `foreach` over literal lists of up to three
  strings, `when` comparisons, Boolean interpolation, lexical path filters,
  collection filters, `timedelta`, and `now()`; 256 cases per run. Rationale:
  the property ranges over an open input space where handwritten rows would
  miss interactions (for example Boolean formatting inside a `foreach` item);
  the oracle is the other mode, so no expected value is computed by hand.
  Domain: bounded as above; the bound admits every construct at least once
  within 256 cases. Artefact: `tests/manifest_load_options_tests.rs` module
  `fidelity_property`. Evidence: red (compile failure) before EP-M3; green
  after. Non-vacuity: record `prop_assert` classification counts per construct
  and require at least 90 per cent successful loads; seeded fault — omit the
  Boolean formatter from test registration and confirm a counterexample with a
  Boolean interpolation is found and shrunk.

- OBL-6 — test mode reads no host environment by default.
  Statement: a test-mode load with no environment supplied answers every
  `env()` lookup as not present, including `PATH`, which is always set in the
  test process; with an injected reader, `env()` returns the reader's value.
  Method: named unit tests; no environment mutation (C8). Rationale: `PATH` is
  a witness that would leak under the full default. Artefact:
  `src/manifest/tests/test_mode.rs`. Evidence: red before EP-M3, green after.
  Non-vacuity: seeded fault — default test mode to `process_env_reader` and
  confirm the `PATH` test fails.

- OBL-7 — `now()` under test.
  Statement: with an injected clock, `now()` returns the injected instant; with
  the system clock, `now()` refuses. Method: two named unit tests using
  `fixed_clock`. Artefact: `src/manifest/tests/test_mode.rs`. Evidence: red
  before EP-M3, green after. Non-vacuity: seeded fault — register `now()`
  unconditionally and confirm the system-clock test fails.

- OBL-8 — overlay collisions are rejected.
  Statement: an overlay whose name equals a manifest `vars` key or a manifest
  macro name fails the load with the localized collision message naming it; an
  overlay with any other name is registered. Method: `rstest` table (var
  collision, macro collision, stdlib-name shadowing allowed, fresh name
  allowed). Artefact: `src/manifest/tests/overlays.rs`. Evidence: red before
  EP-M4, green after. Non-vacuity: every row asserts the outcome and, for
  errors, the offending name; removing the check fails both collision rows.

- OBL-9 — overlay neutrality.
  Statement: loading with empty overlays equals loading with no overlays; and
  for any overlay set whose names are disjoint from every name the manifest
  references and from its vars and macros, the load result is unchanged. Method:
  `proptest`, reusing OBL-5's generator plus a generator of overlay names
  drawn from a pool disjoint from the grammar's identifiers. Artefact:
  `tests/manifest_load_options_tests.rs` module `overlay_neutrality`. Evidence:
  red before EP-M4, green after. Non-vacuity: seeded fault — make overlay
  registration also clear the stdlib `collections` filters and confirm
  counterexamples appear.

- OBL-10 — manifest query unchanged.
  Statement: every query refusal message and every `help targets` output is
  byte-identical to before. Method: EP-M0 snapshot of each query refusal
  message (one row per helper) plus the existing
  `tests/runner_help_targets_tests.rs` and
  `tests/features/help_targets.feature`. Artefact:
  `src/stdlib/restricted_helpers_tests.rs` (moved with the code in EP-M1; the
  snapshot file name must be preserved or re-pointed deliberately, because
  `insta` names snapshots after the module path). Evidence: green at EP-M0 and
  after every later milestone. Non-vacuity: change one character of the query
  marker and confirm failure.

- OBL-11 — overlay registration order.
  Statement: an overlay replaces a same-named stdlib function or refusing stub,
  and is visible to `foreach`, `when`, and rendered fields. Method: named
  tests. Under test mode, an overlay `glob` returning two paths drives a
  `foreach` (proves after-stdlib and before-expansion); an overlay `which` used
  in a `when` filters a target; an overlay used in a command field renders.
  Under full mode, an overlay `now` replaces the real clock helper. Artefact:
  `src/manifest/tests/overlays.rs`. Evidence: red before EP-M4, green after.
  Non-vacuity: seeded fault — register overlays before the stdlib and confirm
  the `glob` test reports the test-mode refusal instead.

- OBL-12 — corpus fidelity-or-refusal.
  Statement: for every corpus manifest, a test-mode load either equals the
  full-mode load (same fixed clock) or fails with a test-mode refusal naming a
  helper from the refusing set; it never succeeds with a different result.
  Method: `rstest` over the files in `tests/data/manifest_load_options/`, with
  `#[files]`-style enumeration so new fixtures join automatically. Artefact:
  `tests/manifest_load_options_tests.rs` module `corpus_fidelity`. Evidence:
  red before EP-M3, green after; 7.1.4 later points the same harness at
  `examples/`. Non-vacuity: the corpus contains at least one pure file
  (equality branch) and at least one refusing file per refusal family (network,
  process, filesystem, host lookup, clock); the test asserts both branches were
  taken.

### Methods deliberately not used

- Kani. The only finite decision logic (OBL-2) is eight rows, which the
  `rstest` table enumerates exhaustively; there is no arithmetic, `unsafe`, or
  bounded state machine for a model checker to add confidence to.
- Verus. This task introduces no contractual business rule or lemma over an
  unbounded domain whose kernel is repository-owned: fidelity (OBL-5) is a
  property of MiniJinja rendering, a third-party interface treated as an axiom,
  and cannot be proved without modelling MiniJinja. The honest method is
  differential testing against the real engine.
- `loom`/`shuttle`. No concurrency is introduced.
- `cargo-mutants`. Not required; the per-obligation seeded faults above are
  the targeted mutation evidence.

## Milestones and plateaus

Each milestone ends with the full gate set run sequentially through the
`scrutineer` agent: `make check-fmt`, `make typecheck`, `make lint`,
`make test`, plus `make markdownlint` and `make nixie` whenever Markdown
changed. Each milestone is one or more commits; never commit a red gate.
Red-stage evidence is observed and recorded in `Progress` before the green
change, and the red test and its green implementation are committed together.

### EP-M0 — characterization baseline

- Outcome: the curated corpus and wrapper-equivalence snapshots exist and
  pass against the unchanged loader; query refusal messages are snapshotted.
- Requirements: RM-e, TD-I7 (oracle for OBL-1, OBL-10).
- Acceptance evidence: `make test` green; new snapshots committed; no
  production file changed.
- Conformance check: tests only; no interface change.
- Recovery: delete the new test file and snapshots.
- Remaining gaps: everything else.
- Compatibility decision: none.

### EP-M1 — extract restricted helper registration

- Outcome: query-mode stubs live in `src/stdlib/restricted_helpers.rs`,
  parameterized by a private `RestrictedSurface` (initially only
  `ManifestQuery`); `src/stdlib/register.rs` delegates. Behaviour is
  byte-identical.
- Requirements: RM-c (mechanism made reusable); C10 headroom.
- Acceptance evidence: gates green; OBL-10 snapshots unchanged;
  `register.rs` well under 400 lines.
- Conformance check: refactor only; `is_manifest_query_disabled_error` still
  matches; no public change.
- Recovery: revert the single refactor commit.
- Remaining gaps: the test surface.
- Compatibility decision: none (private code).

### EP-M2 — options entry point and thin wrappers

- Outcome: public `ManifestLoadOptions`, public `StdlibRegistration`
  (variants `Full` and `ManifestQuery`), and `from_str_with_options` exist;
  every existing entry point forwards to the options path; `ManifestLoadMode`
  is a private factory; `records_load_telemetry` and the render-mode choice are
  methods on `StdlibRegistration`.
- Requirements: RM-a (options; wrappers), TD-4.3, C3, C5.
- Acceptance evidence: OBL-1 unchanged snapshots; OBL-2 table green;
  doctest for `from_str_with_options` green.
- Conformance check: only the public items in `Interfaces and dependencies`
  were added; no existing signature changed.
- Recovery: revert the milestone's commits; EP-M0/EP-M1 stand alone.
- Remaining gaps: `Test`, overlays.
- Compatibility decision: the wrappers are required by roadmap 7.1.2 and have
  ~70 callers across tests, benches, and the runner; they are the stated
  requirement, not a compatibility shim.

### EP-M3 — `StdlibRegistration::Test`

- Outcome: `StdlibRegistration::Test(Box<StdlibConfig>)` and
  `stdlib::register_for_test` implement the deny-by-default surface; test
  refusals are localized in all 35 catalogues; the test default environment is
  `disabled_env_reader`.
- Requirements: RM-b, RM-c, TD-4.2, TD-5.2, TD-5.5, UX-1; advances TD-I5.
- Acceptance evidence: OBL-3, OBL-4, OBL-5, OBL-6, OBL-7, OBL-12 green;
  locale parity tests green.
- Conformance check: the refusing set equals `Decision log`; no build-path
  snapshot changed.
- Recovery: revert the milestone's commits; EP-M2 stands alone.
- Remaining gaps: overlays.
- Compatibility decision: none.

### EP-M4 — template overlays

- Outcome: `TemplateOverlays` and `OverlayCallable` exist; overlays register
  after manifest macros and before `expand_foreach_with_budget`; collisions
  with vars and macros fail with a localized error.
- Requirements: RM-a (overlays), RM-d, TD-4.3 step 5, C9.
- Acceptance evidence: OBL-8, OBL-9, OBL-11 green; OBL-1 unchanged.
- Conformance check: registration order matches TD-4.3 steps 1–6; overlay
  code is not `cfg`-gated.
- Recovery: revert the milestone's commits.
- Remaining gaps: macro substitution (7.1.3).
- Compatibility decision: none.

### EP-M5 — behavioural suite

- Outcome: `tests/features/manifest_load_options.feature` and its step module
  exercise the seam through the library, as the existing
  `tests/features/manifest.feature` does.
- Requirements: AGENTS.md behavioural-test requirement; RM-b, RM-d.
- Acceptance evidence: the scenarios below pass under `make test`.
- Conformance check: steps use injected readers only (C8).
- Recovery: revert.
- Remaining gaps: none in code.
- Compatibility decision: none.

### EP-M6 — documentation, ADR, and roadmap closure

- Outcome: design §4.2, §4.3, and §5.5 reconciled; ADR-041 written;
  developers' guide, `docs/netsuke-design.md`, and `docs/polonius.md` updated;
  roadmap 7.1.2 and its five bullets ticked; this plan set to `COMPLETE`.
- Requirements: design §15 synchronization; AGENTS.md documentation rules.
- Acceptance evidence: `make markdownlint`, `make nixie`, `make check-fmt`
  green.
- Conformance check: every `Decision log` entry that changes the design is
  reflected in the design or ADR-041.
- Recovery: revert documentation commits.
- Remaining gaps: the roadmap gap on file-reading filters is raised with the
  user, not silently added.
- Compatibility decision: none.

## Plan of work

### Stage A — orientation (no code changes)

Read the files in `Context and orientation`. Run the full gate set on the
untouched branch through `scrutineer` and record the baseline in `Progress`, so
later failures can be attributed. Confirm the next free ADR number by listing
`docs/adr-*` on every remote branch (at planning time ADR-039 and ADR-040 are
claimed on unmerged branches, so this plan uses ADR-041).

### Stage B — characterization (EP-M0)

Create `tests/data/manifest_load_options/` with small YAML fixtures, one
concern each: `pure_foreach.yml`, `pure_when.yml`, `pure_macro.yml`,
`boolean_interpolation.yml`, `env_injected.yml`, `clock_now.yml`,
`malformed.yml`, `budget_exhausting.yml`, and one refusing fixture per family:
`uses_glob.yml`, `uses_fetch.yml`, `uses_shell.yml`, `uses_contents.yml`,
`uses_which.yml`. Create `tests/manifest_load_options_tests.rs` with module
`wrapper_equivalence`, which for each listed pre-existing entry point loads
each applicable fixture and snapshots `serde_json::to_string_pretty` of the
result or the `{:#}` error chain. String entry points use an injected reader
and, for `from_str_with_env_and_config`, a fixed clock and empty
`path_override`, so snapshots are host-independent. Path entry points copy
fixtures into a `tempfile` directory first. Accept the snapshots with
`cargo insta review` only after reading each one.

Add a query-refusal snapshot table to the existing stdlib tests (it moves in
EP-M1).

### Stage C — implementation (EP-M1 to EP-M5)

EP-M1. Move `register_disabled_query_helpers`, its two helpers,
`manifest_query_operation_error`, `is_manifest_query_disabled_error`, and the
marker constant from `src/stdlib/register.rs` into a new
`src/stdlib/restricted_helpers.rs`. Introduce a private
`enum RestrictedSurface { ManifestQuery }` whose
`operation_error(&self, operation)` produces today's exact text.
`register_manifest_query` calls
`restricted_helpers::register_disabled(env, RestrictedSurface::ManifestQuery)`.
Keep `is_manifest_query_disabled_error`'s path stable for its callers via the
existing `src/stdlib/mod.rs` re-export.

EP-M2. Red: add `src/manifest/tests/load_options.rs` (OBL-2) and a
`from_str_with_options` doctest; observe compile failure. Green: create
`src/manifest/load_options.rs` holding `pub enum StdlibRegistration` (moved from
`mod.rs`), its query methods, `pub struct ManifestLoadOptions<'a>`, and
`pub fn from_str_with_options`. Change the private `ManifestParse` to hold
`name`, `manifest_root`, and a `ManifestLoadOptions`, deriving the expansion
observer from `records_load_telemetry`. Rewrite `from_str`, `from_str_with_env`,
`from_str_with_env_and_policy`, `from_str_with_env_and_config`, and
`from_str_with_limits` to build options and call the core. Rewrite
`query.rs::from_path_with_registration` so `ManifestLoadMode` produces a
`StdlibRegistration` and the function builds options. Replace both
`matches!(…, ManifestQuery)` sites with methods. Run OBL-1 and confirm zero
snapshot changes; run the two seeded faults, record their failures, and revert
them.

EP-M3. Red: add OBL-3, OBL-4, OBL-6, OBL-7 tests, the OBL-5 property, and the
OBL-12 corpus test; observe failure. Green: add `Test(Box<StdlibConfig>)`; add
`RestrictedSurface::Test` whose operation error uses the new Fluent key and a
distinct marker; add `pub(crate) fn register_for_test(env, config)` in
`src/stdlib/register.rs` (or a sibling if the line cap requires) that installs
the Boolean formatter, the query-safe pure helpers, `now()` over the config's
clock when it is not the system clock, and the test refusing set. Make
`disabled_env_reader` `pub(crate)` and select it as test mode's default
environment. Add the key to `src/localization/keys.rs` and every
`locales/*/messages.ftl` following `docs/translators-guide.md` (Netsuke
identifiers such as `netsuke test` and helper names stay untranslated).

EP-M4. Red: add `src/manifest/tests/overlays.rs` (OBL-8, OBL-11) and the OBL-9
property; observe failure. Green: create `src/manifest/overlays.rs` with
`TemplateOverlays`, `OverlayCallable`, `check_overlay_collisions` (reads the raw
`vars` object and the parsed macro names), and `register_overlays`. In
`evaluate_manifest`, call the check and registration immediately after
`register_manifest_macros_with_budget` and before `expand_foreach_with_budget`.
Add the collision key to all catalogues.

EP-M5. Add `tests/features/manifest_load_options.feature` (below) and
`tests/bdd/steps/manifest_load_options.rs`, declared from
`tests/bdd/steps/mod.rs`, reusing the existing `TestWorld` and manifest fixture
helpers where they fit.

### Stage D — documentation and closure (EP-M6)

Update `docs/netsuke-test-framework-technical-design.md` §4.2 (current file
anchors), §4.3 (reconciled struct shape, `Option` registration default, string
entry point, deferred `macro_substitutions`, overlay collisions), and §5.5 (the
full refusing set, including the six file-reading filters, and the telemetry
decision). Write `docs/adr-041-deny-by-default-test-mode-stdlib.md` covering
the deny-by-default surface, telemetry-free test loads, and the distinct
refusal marker, and link it from the design and from ADR-018's
related-decisions list. Add a "Manifest loader entry points" subsection to
`docs/developers-guide.md` saying new callers use `from_str_with_options`
rather than adding another `from_path_with_*` rung, and describing the overlay
contract. Record the borrow classification of `ManifestLoadOptions<'a>` (a
borrowed `ManifestEnvironment` and a borrowed stage observer, no clones) in
`docs/polonius.md`. Add one sentence to the manifest-loading section of
`docs/netsuke-design.md` naming the three registration modes. Tick roadmap
7.1.2 and its bullets. Run `make fmt` after Markdown edits, then the Markdown
gates.

## Concrete steps

Run everything from the repository root,
`/home/leynos/.lody/repos/github---leynos---netsuke/worktrees/0965d3fb-38f0-46dc-a907-0cd999d8a910`.
Gates are run by `scrutineer`, one at a time, each teeing to
`/tmp/$ACTION-netsuke-7-1-2-options-carrying-manifest-loader-entry-point.out`.

Focused loops during development (not substitutes for gates):

```sh
cargo nextest run --test manifest_load_options_tests 2>&1 | tee /tmp/focus-netsuke-7-1-2.out
cargo nextest run --lib manifest::tests::load_options 2>&1 | tee -a /tmp/focus-netsuke-7-1-2.out
cargo insta test --review --test manifest_load_options_tests
```

Expected red transcript at the start of EP-M2 (abridged):

```plaintext
error[E0432]: unresolved import `crate::manifest::ManifestLoadOptions`
```

Expected green transcript after EP-M5 (abridged):

```plaintext
     Summary [ …s] N tests run: N passed, 0 skipped
```

## Validation and acceptance

Behavioural specification, `tests/features/manifest_load_options.feature`:

```gherkin
Feature: Options-carrying manifest loading

  Scenario: A pure manifest loads identically in build and test modes
    Given the manifest fixture "pure_foreach.yml"
    And a fixed clock at "2026-06-08T12:00:00Z"
    When the manifest is loaded in build mode
    And the manifest is loaded in test mode
    Then both loads produce the same Ninja file

  Scenario: Test mode refuses a process helper
    Given the manifest fixture "uses_shell.yml"
    When the manifest is loaded in test mode
    Then loading fails because "shell" is unavailable under netsuke test

  Scenario: Test mode does not read the host environment by default
    Given a manifest whose target description is "{{ env('PATH') }}"
    When the manifest is loaded in test mode
    Then loading fails because "PATH" is not set

  Scenario: A template overlay stands in for glob under test
    Given the manifest fixture "uses_glob.yml"
    And a template overlay "glob" returning "a.c" and "b.c"
    When the manifest is loaded in test mode
    Then the manifest defines targets "a.o" and "b.o"

  Scenario: An overlay may not share a name with a manifest variable
    Given the manifest fixture "pure_foreach.yml"
    And a template overlay named after its variable "sources"
    When the manifest is loaded in test mode
    Then loading fails naming the overlay "sources"
```

Acceptance: every scenario fails before its milestone (EP-M3 or EP-M4) and
passes after; `make test` passes with every pre-existing test and snapshot
untouched.

Quality criteria:

- Tests: `make test` green; the new unit, property, differential, and
  behavioural tests pass; no pre-existing snapshot changed.
- Verification: OBL-1 to OBL-12 discharged with their non-vacuity evidence
  recorded in `Artefacts and notes`.
- Lint and types: `make check-fmt`, `make typecheck`, `make lint` green.
- Documentation: `make markdownlint` and `make nixie` green;
  `make doc-coverage` stays at or above 80 per cent.

## Idempotence and recovery

Every step is re-runnable. Snapshot acceptance is idempotent. Each milestone is
a set of commits that can be reverted independently, newest first. If a gate
fails halfway through a milestone, fix forward within the three-attempt
tolerance or reset the working tree to the last green commit with `git restore`
(never `git stash` without a unique tag; the stash is shared across worktrees).

## Artefacts and notes

Record here, as work proceeds: the baseline gate results, each red transcript,
each seeded-fault failure (OBL-1, OBL-3, OBL-4, OBL-5, OBL-6, OBL-7, OBL-9,
OBL-10, OBL-11), and the OBL-5 classification counts.

## Interfaces and dependencies

No new dependencies. At the end of EP-M4 these items exist, re-exported from
`netsuke::manifest`:

```rust
/// Selects the stdlib surface available while rendering a manifest.
pub enum StdlibRegistration {
    /// The complete stdlib used for a normal build.
    Full(Box<StdlibConfig>),
    /// The side-effect-free surface used by `netsuke help targets`.
    ManifestQuery,
    /// The deny-by-default surface used by `netsuke test`.
    Test(Box<StdlibConfig>),
}

impl StdlibRegistration {
    /// Report whether loads in this mode emit normal-loading telemetry.
    pub const fn records_load_telemetry(&self) -> bool;
}

/// Inputs to one manifest load beyond the YAML text and its name.
pub struct ManifestLoadOptions<'a> { /* private fields */ }

impl<'a> ManifestLoadOptions<'a> {
    /// Build options for an explicit stdlib surface with mode defaults.
    pub fn new(registration: StdlibRegistration) -> Self;
    /// Supply the environment reader and access policy for `env()`.
    pub fn with_environment(self, environment: ManifestEnvironment<'a>) -> Self;
    /// Supply resource ceilings.
    pub fn with_budget_limits(self, limits: ManifestBudgetLimits) -> Self;
    /// Observe loader stages.
    pub fn with_stage_observer(self, on_stage: &'a mut dyn FnMut(ManifestLoadStage)) -> Self;
    /// Install template overlays.
    pub fn with_overlays(self, overlays: TemplateOverlays) -> Self;
}

impl Default for ManifestLoadOptions<'_> { /* ambient full stdlib, resolved lazily */ }

/// Load a manifest string with explicit options.
pub fn from_str_with_options(
    yaml: &str,
    name: &ManifestName,
    options: ManifestLoadOptions<'_>,
) -> anyhow::Result<NetsukeManifest>;

/// Callable standing in for a template function.
#[derive(Clone)]
pub struct OverlayCallable(/* Arc<dyn Fn(&State<'_, '_>, &[Value]) -> Result<Value, Error> + Send + Sync> */);

impl OverlayCallable {
    /// Wrap a dispatch closure.
    pub fn new<F>(dispatch: F) -> Self
    where
        F: Fn(&minijinja::State<'_, '_>, &[minijinja::Value])
            -> Result<minijinja::Value, minijinja::Error>
            + Send
            + Sync
            + 'static;
}

/// Test-supplied substitutions applied after stdlib and manifest-macro
/// registration and before `foreach` expansion.
#[derive(Clone, Default)]
pub struct TemplateOverlays { /* functions: IndexMap<String, OverlayCallable> */ }

impl TemplateOverlays {
    /// Add or replace a function overlay.
    #[must_use]
    pub fn with_function(self, name: impl Into<String>, callable: OverlayCallable) -> Self;
    /// Report whether no overlay is present.
    pub fn is_empty(&self) -> bool;
}
```

Crate-internal additions: `stdlib::register_for_test`,
`restricted_helpers::{RestrictedSurface, register_disabled}`,
`manifest::env_reader::disabled_env_reader` widened to `pub(crate)`, and the
Fluent keys `stdlib.test_mode.helper_unavailable` and
`manifest.overlay.name_collision`.

Where `evaluate_manifest` registers overlays, each `OverlayCallable` is
installed as follows:

```rust
env.add_function(name, move |state: &State, args: Rest<Value>| {
    callable.call(state, &args)
});
```
