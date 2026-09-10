# Add the clock provider seam to the stdlib time module (7.1.1)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: COMPLETE

## Purpose / big picture

Netsuke manifests may call the Jinja function `now()` to obtain the current UTC
timestamp. Today that function reads the host wall clock directly, so nothing
that renders a manifest containing `now()` can be made repeatable: a test can
only assert that the rendered value lies within a few seconds of the real clock.

After this change the wall-clock read becomes an *injectable seam*. A caller
that builds a `StdlibConfig` may supply a **clock provider** — a shared closure
returning the instant that `now()` should report — and every `now()` call in
that Jinja environment will return exactly that instant. A caller that supplies
nothing keeps today's behaviour precisely: `now()` reads the host clock.

Concretely, after this change a novice can observe the following. Given a test
that builds a `StdlibConfig` with a clock provider fixed at
`2026-06-08T12:00:00Z` and renders the template `{{ now().iso8601 }}`, the
rendered output is the exact string `2026-06-08T12:00:00Z`, every time, on
every machine. Rendering `{{ now(offset='+02:30').iso8601 }}` under the same
fixed clock yields exactly `2026-06-08T14:30:00+02:30` — the same instant,
re-expressed in another offset.

This is a prerequisite refactor for the Netsukefile testing framework (roadmap
phase 7), which needs a deterministic clock before it can run manifest tests.
It is deliberately scoped as a deliverable in its own right: nothing in this
plan adds a `netsuke test` command, a test dialect, or a `src/testing` module.

There is no user-visible change. A manifest author sees identical `now()`
behaviour before and after.

## Definitions

Terms used throughout, defined here so no prior reading is required.

- **Seam.** A place where behaviour that would otherwise read the ambient
  process environment (variables, clock, filesystem, network) is instead
  supplied by the caller as a value or closure. Netsuke's seam vocabulary is
  fixed by `docs/adr-008-environment-seam-taxonomy.md`.
- **Clock provider.** The seam introduced here: a shared, thread-safe closure
  returning a `time::OffsetDateTime` representing "now".
- **Ambient / ambient fallback.** Reading the real host clock, which is what
  happens when no provider is explicitly supplied.
- **MiniJinja.** The template engine Netsuke embeds (crate `minijinja`,
  the lockfile resolves 2.24.0). Manifest bodies are Jinja templates rendered
  by it.
- **Stdlib.** Netsuke's library of Jinja filters and functions registered into
  a MiniJinja `Environment`; it lives under `src/stdlib/`.
- **`StdlibConfig`.** The struct at `src/stdlib/config/mod.rs:21` that carries
  every knob the stdlib registration needs (workspace root, network policy,
  `PATH` overrides, home directory, and — after this change — the clock).
- **Manifest-query mode.** A restricted registration used by
  `netsuke help targets` that deliberately refuses host-dependent helpers,
  including `now()`. See `src/stdlib/register.rs:135`.
- **Driven port / adapter.** Hexagonal-architecture vocabulary. A *driven
  port* is an interface the domain requires from infrastructure; an *adapter*
  implements it. `ClockProvider` is a driven port; the host-clock closure is
  its production adapter.

## Context and orientation

Assume no prior knowledge of this repository. The relevant code is small and
concentrated.

### Where the clock is read today

`src/stdlib/time/mod.rs` owns the `now()` and `timedelta()` Jinja functions.
The current implementation reads the clock inline:

```rust
/// Register time helpers with the environment.
pub(crate) fn register_functions(env: &mut Environment<'_>) {
    env.add_function("now", |kwargs: Kwargs| now(&kwargs));
    register_query_functions(env);
}

/// Register time helpers whose output does not depend on the current clock.
pub(crate) fn register_query_functions(env: &mut Environment<'_>) {
    env.add_function("timedelta", |kwargs: Kwargs| timedelta(&kwargs));
}

fn now(kwargs: &Kwargs) -> Result<Value, Error> {
    let offset_spec: Option<String> = kwargs.get("offset")?;
    kwargs.assert_all_used()?;

    let mut timestamp = OffsetDateTime::now_utc();
    if let Some(raw) = offset_spec {
        let parsed = parse_offset(&raw)?;
        timestamp = timestamp.to_offset(parsed);
    }

    Ok(Value::from_object(TimestampValue::new(timestamp)))
}
```

`OffsetDateTime::now_utc()` at `src/stdlib/time/mod.rs:62` is the single
wall-clock read for `now()`. It comes from the `time` crate (version 0.3.44);
the repository depends on neither `chrono` nor `jiff`.

Note that `register_functions` currently takes no configuration at all. Every
other stdlib submodule already receives some:
`path::register_filters(env, config.home_directory().clone())`,
`which::register(env, which_config)`,
`network::register_functions(env, impure, network_config)`, and
`command::register(env, impure, command_config)`. The time module is the
outlier, which is precisely the ADR-008 gap this work closes.

### How registration is wired

`src/stdlib/register.rs` orchestrates registration. The relevant function:

```rust
pub fn register_with_config(
    env: &mut Environment<'_>,
    config: StdlibConfig,
) -> anyhow::Result<StdlibState> {
    register_legacy_boolean_formatter(env);
    let state = StdlibState::default();
    register_read_only_helpers(env, &config);
    time::register_functions(env);
    let impure = state.impure_flag();
    let (network_config, command_config) = config.into_components();
    network::register_functions(env, Arc::clone(&impure), network_config);
    command::register(env, impure, command_config);
    Ok(state)
}
```

Two details matter. First, `time::register_functions(env)` is called *before*
`config.into_components()` consumes the configuration, so the clock can be read
from `&config` by reference and cloned, exactly as `home_directory()` already
is. Second, `register()` (the no-argument entry point at
`src/stdlib/register.rs:60`) builds a default `StdlibConfig` and delegates to
`register_with_config`, so it inherits whatever default the clock field takes.

A separate, restricted path exists for manifest queries:

```rust
pub(crate) fn register_manifest_query(env: &mut Environment<'_>) -> StdlibState {
    let state = StdlibState::default();
    register_query_helpers(env);
    time::register_query_functions(env);
    register_disabled_query_helpers(env);
    state
}
```

That path registers only the clock-independent `timedelta()`, and separately
installs a refusing stub for `now()` at `src/stdlib/register.rs:231`:

```rust
env.add_function("now", |_kwargs: Kwargs| -> Result<Value, Error> {
    Err(manifest_query_operation_error("now"))
});
```

This refusal is user-documented at `docs/users-guide.md:1178`, which states
that queries reject "the clock-dependent `now()` function". **The seam must not
leak a clock into manifest-query mode.** That is a hard constraint below, and
currently it has no regression test — this plan adds one.

### The configuration struct

`src/stdlib/config/mod.rs:19-48` defines:

```rust
/// Configuration for registering Netsuke's standard library helpers.
#[derive(Debug, Clone)]
pub struct StdlibConfig {
    workspace_root: Arc<Dir>,
    workspace_root_path: Option<Utf8PathBuf>,
    fetch_cache_relative: Utf8PathBuf,
    network_policy: NetworkPolicy,
    fetch_max_response_bytes: u64,
    command_max_output_bytes: u64,
    command_max_stream_bytes: u64,
    which_cache_capacity: NonZeroUsize,
    workspace_skip_dirs: Vec<String>,
    path_override: Option<OsString>,
    pathext_override: Option<OsString>,
    command_path_override: Option<OsString>,
    home_directory: HomeDirectory,
}
```

There is no `Default` implementation, derived or manual. Construction goes
through the fallible `StdlibConfig::new(Dir)` or
`StdlibConfig::from_current_dir()`. Every knob is then set by a consuming
builder taking `self` by value and returning `Self` or `anyhow::Result<Self>`,
for example `with_home_override` at `src/stdlib/config/mod.rs:239` and
`with_command_path_override` at `src/stdlib/config/mod.rs:203`. New knobs
follow that idiom.

`#[derive(Debug, Clone)]` on this struct is load-bearing for this plan; see
`Decision log` entry D2.

### The seam precedent to copy

`src/manifest/env_reader.rs` is the closest existing analogue and should be
read before implementing. It defines the type alias, its production supplier,
and a disabled variant:

```rust
/// Thread-safe environment reader supplied to the `env()` Jinja helper.
pub type EnvReader = Arc<dyn Fn(&str) -> Result<String, EnvReadError> + Send + Sync>;

/// Construct the process-backed environment reader used by production loads.
#[must_use]
pub fn process_env_reader() -> EnvReader {
    let env = DefaultEnv;
    Arc::new(move |key| env.raw(key).map_err(EnvReadError::from))
}
```

Both the type alias and the supplier carry rustdoc examples that run as
doctests. The clock seam mirrors this shape, naming, and documentation density.

### Files a novice will touch

| Path                                              | Role                                                                                                             |
| ------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| `src/stdlib/time/clock.rs`                        | New. The `ClockProvider` port, its `system_clock()` and `fixed_clock()` adapters, and the `WallClock` container. |
| `src/stdlib/time/mod.rs`                          | Declares `mod clock;`, re-exports the port, threads the clock into `now()`.                                      |
| `src/stdlib/time/tests.rs`                        | Existing unit tests; fixture updated and new cases added.                                                        |
| `src/stdlib/config/mod.rs`                        | `StdlibConfig` gains the `clock` field, `with_clock`, and `clock()`.                                             |
| `src/stdlib/config_tests.rs`                      | Unit coverage for the new builder and accessor.                                                                  |
| `src/stdlib/mod.rs`                               | Re-exports the port, its adapters, and `ClockInstant` on the public stdlib surface.                              |
| `src/stdlib/register.rs`                          | Passes the clock to `time::register_functions`.                                                                  |
| `tests/std_filter_tests/time_functions.rs`        | New. Integration coverage through real `StdlibConfig` registration.                                              |
| `tests/std_filter_tests/support.rs`               | Gains a `stdlib_env_with_clock` helper.                                                                          |
| `tests/std_filter_tests.rs`                       | Wires the new integration module (see the wiring contract below).                                                |
| `tests/features/stdlib_time.feature`              | New deterministic scenarios.                                                                                     |
| `tests/bdd/steps/stdlib/rendering.rs`             | New `Given` step; `RenderConfig` gains a clock.                                                                  |
| `docs/adr-008-environment-seam-taxonomy.md`       | Addendum recording the seam classification.                                                                      |
| `docs/developers-guide.md`                        | Seam ownership rules and module boundary entry.                                                                  |
| `docs/netsuke-test-framework-technical-design.md` | §5.2 updated to record implemented state.                                                                        |
| `docs/roadmap.md`                                 | Mark 7.1.1 done, at the very end.                                                                                |

### The integration-test wiring contract

`tests/integration_test_wiring_tests.rs` enforces that every module tree under
`tests/` is reachable from a Cargo test target, and that Cargo's discovered
test targets exactly match the `tests/*.rs` files on disk. For this plan the
practical consequence is narrow and mechanical: the new file
`tests/std_filter_tests/time_functions.rs` must be declared in
`tests/std_filter_tests.rs`, whose existing contents are a sorted list of
`#[path]` declarations:

```rust
#[path = "std_filter_tests/support.rs"]
mod support;
#[path = "std_filter_tests/which_filter_common.rs"]
mod which_filter_common;
```

Add, in sorted position:

```rust
#[path = "std_filter_tests/time_functions.rs"]
mod time_functions;
```

No new top-level `tests/*.rs` file is created by this plan, so the Cargo
target-discovery half of the contract is unaffected.

### Relevant documentation and skills

Read these before starting; each is cited where it bears on a specific step.

- `docs/adr-008-environment-seam-taxonomy.md` — the three sanctioned seam
  shapes and the rule for choosing between them. Mandatory.
- `docs/netsuke-test-framework-technical-design.md` §5.2 — the specification
  this plan implements, including the exact `ClockProvider` type. Mandatory.
- `docs/reliable-testing-in-rust-via-dependency-injection.md` — the project's
  rationale for injected seams over ambient reads.
- `docs/rust-testing-with-rstest-fixtures.md` — fixture and `#[case]` idioms.
- `docs/rust-doctest-dry-guide.md` — doctest conventions; the new public type
  alias and supplier both carry runnable examples.
- `docs/developers-guide.md`, sections "Environment and template ports" and
  "Internal support module boundaries" — where the new prose belongs.
- `docs/rstest-bdd-users-guide.md` — step-definition conventions for the new
  `Given` step.
- `docs/documentation-style-guide.md` — ADR and prose conventions.
- Skills: `rust-router` (routing), `hexagonal-architecture` (port/adapter
  boundary), `rust-unit-testing` (assertion shape), `proptest` (the offset
  property), `rust-types-and-apis` (the newtype and alias decision),
  `arch-decision-records` (the ADR addendum), `execplans` (this document).

`docs/ortho-config-users-guide.md` is *not* applicable here and no
`ortho_config` change is required; see `Decision log` entry D6.

## Constraints

Hard invariants. Violating any of these requires escalation, not a workaround.

1. **C1 — no behaviour change without a provider.** A `StdlibConfig` on which
   `with_clock` was never called must produce a `now()` that reads the host
   clock, with identical output shape, offset handling, and error messages to
   the current implementation.
2. **C2 — manifest-query mode keeps refusing `now()`.** The restricted
   registration at `src/stdlib/register.rs:135` must not receive, construct, or
   consult a clock. The refusing stub at `src/stdlib/register.rs:231` and its
   diagnostic must be unchanged. `docs/users-guide.md:1178` must remain true
   without edit.
3. **C3 — the seam type is as specified.** The technical design fixes the
   port as `Arc<dyn Fn() -> OffsetDateTime + Send + Sync>`. Do not substitute a
   trait object, a generic parameter, or a different time type. `Send + Sync`
   is required because MiniJinja's `add_function` demands it; this is a
   compiler-enforced constraint, not a preference.
4. **C4 — `StdlibConfig` remains `Debug` and `Clone`.** The derive at
   `src/stdlib/config/mod.rs:20` must survive. Widening it to a handwritten
   thirteen-field `Debug` is not acceptable; see D2.
5. **C5 — no new external dependencies.** `time`, `minijinja`, `rstest`,
   `googletest`, `pretty_assertions`, `proptest`, and `rstest-bdd` are all
   already present. Adding a crate is an escalation trigger.
6. **C6 — the clock has exactly one owner.** `StdlibConfig` is the sole place
   a clock is stored, per technical design §5.2. Do not add a parallel clock
   parameter to `register()`, to manifest loading, or to any other entry point.
   Note honestly that this is a *convention over a single call site*, not a
   type-enforced property: `time::register_functions` is `pub(crate)` and takes
   an owned `WallClock`, and EP-M1 deliberately calls it with
   `WallClock::default()` before `StdlibConfig` is involved. What the compiler
   does enforce is the crate boundary — no external caller can construct a
   `WallClock` at all.
7. **C7 — no ambient-state mutation in tests.** Per ADR-008's 2026-09-01
   addendum, `EnvLock`, `CwdGuard`, and `EnvVarGuard` are retired and no
   sanctioned test mutates the harness process. Determinism comes from
   injection; no new test may use `#[serial]` or mutate process globals.
8. **C8 — scope.** This plan delivers roadmap item 7.1.1 only. It must not
   add `src/testing/`, a `netsuke test` command, `ManifestLoadOptions`,
   `TemplateOverlays`, or a `StdlibRegistration::Test` variant. Those are 7.1.2
   and later.

## Tolerances (exception triggers)

Stop and escalate — do not improvise — when any threshold is reached.

- **Scope.** More than 20 files changed, or more than 600 net lines added
  across the whole change. This is a small, well-understood seam; substantial
  overrun means the design was wrong.
- **Interface.** Any change to a *public* signature other than the three
  additions this plan sanctions (`stdlib::ClockProvider`,
  `stdlib::ClockInstant`, `stdlib::system_clock`, `stdlib::fixed_clock`, and
  `StdlibConfig::with_clock` plus its crate-private accessor). In particular, if
  `StdlibConfig::new` or `into_components` must change shape, stop.
- **Dependencies.** Any new entry in `Cargo.toml`. Stop.
- **Iterations.** A given gate still failing after 3 fix attempts. Stop and
  report the log path.
- **Lint thresholds.** If satisfying `clippy.toml`'s
  `cognitive-complexity-threshold = 9`, `too-many-lines-threshold = 70`, or
  `too-many-arguments-threshold = 4` requires restructuring code outside
  `src/stdlib/time/` and `src/stdlib/config/`, stop.
- **Determinism.** If any newly added test proves flaky — passing and failing
  across repeated runs of the same commit — stop. A flaky test in a plan whose
  entire purpose is determinism indicates the seam is not actually threaded
  through the path under test.
- **Ambiguity.** If the technical design and the observed code disagree in a
  way that changes the seam's shape or ownership, stop and present options.

## Risks

- **Risk: `Arc<dyn Fn>` is not `Debug`, breaking `StdlibConfig`'s derive.**
  Severity: high. Likelihood: certain (this *will* happen on first compile).
  Mitigation: this is anticipated and designed for — the `WallClock` newtype
  with a handwritten `Debug` absorbs it, so `StdlibConfig` keeps its derive.
  See D2. The precedent is `CommandEnv` at
  `src/runner/process/command_env.rs:73`.

- **Risk: the clock is captured once at registration rather than consulted per
  call.** Severity: high. Likelihood: low, because
  `Interfaces and dependencies` hands the implementor the correct body
  outright. A natural but wrong implementation reads the provider while
  building the closure and bakes the resulting instant in. Under the
  fixed-clock tests this defect is invisible, because a fixed provider returns
  the same value either way. Mitigation: the sequenced-provider test (`OBL-3`)
  is the designated negative control. Keep it even though the immediate
  likelihood is low — its value is as a regression guard for later refactors,
  when the correct body is no longer sitting in front of whoever is editing it.

- **Risk: an injected clock reaches a real build path.** Severity: high.
  Likelihood: low now, rising at 7.1.2. `StdlibConfig` is `Clone` and the
  provider is a shared `Arc`, so once the test framework threads a
  harness-built configuration into code shared with `netsuke build`, a leaked
  clock would stamp a frozen instant into generated outputs. The worst form is
  silent: a build file that renders byte-identically every time stops
  triggering Ninja rebuilds, so CI goes green on stale artefacts. Mitigation:
  `WallClock::is_system()` and the labelled `Debug` make the installed clock
  observable rather than invisible; the default path is additionally protected
  because `StdlibConfig::new` enumerates every field, so the compiler forces
  `WallClock::default()` there. Revisit this risk explicitly at 7.1.2.

- **Risk: an injected provider returns a non-UTC instant.** Severity: medium.
  Likelihood: medium. `now()` is documented to yield UTC and `system_clock()`
  does, but an arbitrary provider need not — a fixture built from a local-time
  literal would make the harness assert behaviour production never exhibits.
  Mitigation: `WallClock::read` normalizes with `to_offset(UtcOffset::UTC)`,
  and OBL-1 carries a non-UTC-provider case that mutation 6 must break.

- **Risk: the seam accidentally reaches manifest-query mode.** Severity:
  medium. Likelihood: low. Mitigation: C2, plus the new regression tests
  (OBL-5). Note the subtlety recorded there: because the refusing stub is
  registered *after* the permissive helpers and MiniJinja's `add_function` is
  last-write-wins, a leak into `register_query_functions` would be masked by
  the stub. The mitigation is therefore two tests, not one — the second
  asserting `now` is undefined after the permissive half alone. A single "query
  mode refuses `now`" test would give false assurance.

- **Risk: `time::register_functions` also registers `timedelta` via
  `register_query_functions`.** Severity: low. Likelihood: medium. Threading a
  clock naively could push the clock into the query-function path. Mitigation:
  change only the `now` registration; leave `register_query_functions`
  signature untouched.

- **Risk: the existing `now_defaults_to_utc` unit test and the BDD step
  `assert_stdlib_output_is_utc_timestamp` compare against the real clock with
  3-second and 5-second tolerances.** Severity: low. Likelihood: low.
  Mitigation: keep both. They are the ambient-fallback coverage C1 requires,
  and their tolerances remain appropriate *because* those paths have no
  injected clock. Do not "tighten" them; see D5.

- **Risk: doctest breakage.** Severity: low. Likelihood: medium. The new
  public alias and supplier carry rustdoc examples, and `make test` runs
  doctests via a separate `doctest` target that nextest cannot execute.
  Mitigation: run `make test` (not just `cargo nextest run`) at each milestone.

- **Risk: Markdown gate rejects the documentation edits.** Severity: low.
  Likelihood: medium. `make check-fmt` enforces canonical Markdown formatting
  over all `.md` files, including tables. Mitigation: run `make fmt` before
  `make check-fmt`, and compare table *cells* rather than rendered rows.

## Conformance basis

Upstream artefacts governing this work, at the revisions present in the working
tree at branch point `924cb215`:

- `docs/rfcs/0006-ansible-inspired-template-standard-library.md` §3.3 —
  records the original gap: "**`now` has no injected clock seam.** It calls
  `OffsetDateTime::now_utc()` directly. The time helpers proposed here are pure
  and do not need the seam, but the gap is recorded because it bounds how far
  time behaviour can be tested deterministically."
- `docs/rfcs/0006-ansible-inspired-template-standard-library.md` §16 open
  question 7 — "**Does `now` need an injected clock seam?** … a future slice
  that wants deterministic time tests will have to answer it." This plan is
  that slice, and closing it answers the question affirmatively.
- `docs/rfcs/0007-netsukefile-testing-framework.md` — records the gap:
  "a clock seam for `now()` (it calls the system clock directly)".
- `docs/netsuke-test-framework-technical-design.md` §5.2 "Clock (new seam)" —
  the normative specification, including the exact port type and the statement
  that the seam "lives in `StdlibConfig` … and is a prerequisite refactor
  deliverable in its own right".
- `docs/netsuke-test-framework-technical-design.md` §13 — names
  `src/stdlib/time/` (clock seam) and `src/stdlib/config/` (clock in
  `StdlibConfig`) as the modules touched.
- `docs/netsuke-test-framework-technical-design.md` §14 phase 1 — places this
  work in the first phase, alongside the overlay spike and the loader refactor.
- `docs/netsuke-test-framework-technical-design.md` §15 — requires this design
  document to be updated in step when its decisions are implemented.
- `docs/adr-008-environment-seam-taxonomy.md` — the seam taxonomy and the
  requirement to record a new seam's classification.
- `docs/roadmap.md` item 7.1.1 — the four acceptance bullets.
- `docs/users-guide.md:1178` — the user-facing statement that manifest queries
  reject `now()`, which must remain true.

There is no separate Terms of Reference document for this work; the roadmap
item and technical design §5.2 together serve that role. Do not invent one.

Trace chain:

```plaintext
RFC0006-3.3-GAP-clock -> RFC0006-Q7 -> TDD-5.2-ClockSeam -> EP-M4 -> docs/rfcs/0006 update
RFC0007-GAP-clock -> TDD-5.2-ClockSeam -> ROADMAP-7.1.1 -> EP-M1 -> OBL-1, OBL-2, OBL-3, OBL-4
RFC0007-GAP-clock -> TDD-5.2-ClockSeam -> ROADMAP-7.1.1 -> EP-M2 -> OBL-6, OBL-7
TDD-5.2-ClockSeam(single owner) -> ROADMAP-7.1.1(bullet 1) -> EP-M2 -> tests::std_filter_tests::time_functions
ADR008-TAXONOMY -> ROADMAP-7.1.1(bullet 4) -> EP-M4 -> docs/adr-008 addendum
USERGUIDE-1178(query refuses now) -> C2 -> EP-M2 -> OBL-5
ROADMAP-7.1.1(bullet 2, preserve behaviour) -> C1 -> EP-M1 -> OBL-4
```

Roadmap 7.1.1's four bullets map as follows. Bullet 1 (register `now()` through
an injected `ClockProvider` held in `StdlibConfig`) is discharged by EP-M1 and
EP-M2. Bullet 2 (preserve behaviour with no provider) is OBL-4. Bullet 3 (test
injected value, repeated calls, and ambient fallback, all registered through
`StdlibConfig`) is OBL-1, OBL-2, and OBL-4, exercised through `StdlibConfig` in
EP-M2. Bullet 4 (record the classification per ADR-008) is EP-M4.

## Verification plan

The change is small but it introduces genuine invariants, and two of them have
plausible implementations that satisfy the obvious tests while being wrong. The
obligations below are chosen so that each can fail when the implementation is
wrong, and each names the mutation it must reject.

### Axioms (assumed, not verified)

These are third-party or platform behaviours treated as given. Do not write
tests for them.

- **AXIOM-1.** `time::OffsetDateTime::now_utc()` returns the host wall clock in
  UTC. The `time` crate's correctness is assumed.
- **AXIOM-2.** `OffsetDateTime::to_offset(o)` preserves the absolute instant and
  changes only the representation, so
  `t.to_offset(o).unix_timestamp() == t.unix_timestamp()` for every valid `o`.
  This is the documented contract of the `time` crate.
- **AXIOM-3.** MiniJinja's `Environment::add_function` requires its closure to
  be `Send + Sync + 'static` and may invoke it from any thread. This is what
  forces the `Arc` shape (ADR-008 states the same for `EnvReader`).
- **AXIOM-4.** `Arc<dyn Fn() -> T + Send + Sync>` is `Clone` and `Send + Sync`,
  and cloning shares one underlying closure rather than duplicating it.
- **AXIOM-5.** `minijinja::Environment::compile_expression(..).eval(..)` invokes
  a registered function once per textual occurrence of a call in the expression.

Repository-owned logic that builds on AXIOM-2 and AXIOM-3 *is* verified: OBL-6
exercises offset preservation against the real `time` interface through the
real registration path rather than against a stub.

### Invariants and lemmas

**OBL-1 — Injected instant is reported verbatim.**

- Obligation: with a provider returning fixed instant `T`, evaluating `now()`
  yields a timestamp equal to `T`, including its UTC offset.
- Method: parameterized unit test (`rstest`) plus an integration test through
  `StdlibConfig`.
- Rationale: a finite, fully enumerable partition — there is one behaviour to
  pin. Property testing would add nothing.
- Domain: at least three distinct fixed instants, including one far from the
  present day so the assertion cannot accidentally pass against the real clock,
  **and one provider returning a non-UTC instant** such as
  `datetime!(2026-06-08 17:30:00 +05:30)`, whose rendered result must still
  carry `UtcOffset::UTC` and the same absolute instant.
- Artefact: `src/stdlib/time/tests.rs::now_uses_injected_clock`;
  `tests/std_filter_tests/time_functions.rs::now_uses_configured_clock`.
- Evidence: fails before the change with a compile error (no `with_clock`
  exists), passes after. Assertion is exact equality via
  `googletest::assert_that!(captured, eq(fixed))`.
- Non-vacuity: the fixed instant is `2026-06-08T12:00:00Z`, which differs from
  the real clock by far more than any plausible test-runtime skew. The
  designated mutation is "ignore the provider and call
  `OffsetDateTime::now_utc()`"; that mutation makes the assertion fail by
  years, not milliseconds. The non-UTC case guards a distinct hazard: nothing
  else in the plan pins the *offset* of the injected path. `system_clock()`
  yields UTC, but an arbitrary provider need not, and without normalization a
  fixture built from a local-time literal would make `now()` render a non-`Z`
  timestamp — harness and production diverging precisely where the seam exists
  to make them agree. Mutation 6 removes the normalization and this case
  rejects it.

**OBL-2 — Repeated calls under a fixed provider agree.**

- Obligation: within one Jinja environment, two `now()` evaluations under a
  fixed provider return the same instant.
- Method: parameterized unit test plus integration test.
- Rationale: this is roadmap bullet 3's explicit "repeated `now()` calls
  returning it" requirement, and it is the property a manifest author actually
  relies on.
- Domain: two sequential evaluations in one environment, and one expression
  containing two `now()` calls.
- Artefact: `src/stdlib/time/tests.rs::now_repeats_the_injected_instant`.
- Evidence: `assert_that!(first, eq(second))` and both equal to the fixed
  instant.
- Non-vacuity: the mutation "read the ambient clock" is rejected because two
  ambient reads separated by template evaluation may differ, and both differ
  from the fixed instant regardless. Note this obligation alone is weak — it is
  satisfied by the *wrong* implementation described in OBL-3 — which is why
  OBL-3 exists.

**OBL-3 — The provider is consulted on every call, not captured once.**

- Obligation: the registered `now` function invokes the provider closure once
  per `now()` evaluation. It must not read the provider while building the
  closure and cache the resulting instant.
- Method: unit test with a *sequenced* provider — a closure over a shared
  counter that returns a different, predetermined instant on each invocation.
- Rationale: this is the designated negative control for the highest-risk
  defect in the change. No fixed-clock test can detect it, because a fixed
  provider returns the same value whether it is called once or a thousand
  times. Only a provider whose output varies distinguishes the two
  implementations.
- Domain: three successive `now()` evaluations against a provider yielding
  `T1`, `T2`, `T3` in order; plus an invocation-count assertion.
- Artefact: `src/stdlib/time/tests.rs::now_reads_the_provider_on_every_call`
  and `src/stdlib/time/tests.rs::now_invokes_the_provider_once_per_call`.
- Evidence: the three evaluations return `T1`, `T2`, `T3` respectively, and
  the recorded invocation count is exactly 3.
- Non-vacuity: the mutation "capture the instant at registration time" makes
  all three evaluations return `T1` and the count 1 — rejected on both
  assertions. The mutation "read the provider twice per call" (plausible if the
  offset branch re-reads) makes the count 6 — rejected by the count assertion.
  Both mutations are concrete and should be tried by hand once, in a scratch
  commit that is then discarded, to confirm the tests actually fail.

**OBL-4 — Ambient fallback preserves current behaviour.**

- Obligation: a `StdlibConfig` on which `with_clock` was never called yields a
  `now()` that reads the host clock and returns a UTC-offset timestamp.
- Method: parameterized unit test retained from the current suite, plus an
  integration test constructing a `StdlibConfig` without `with_clock`.
- Rationale: this is constraint C1 and roadmap bullet 2. A tolerance-based
  comparison against the real clock is the only available oracle for an ambient
  read, and it is adequate: the failure mode being guarded against is "the
  default is a frozen or wrong instant", which a seconds-scale tolerance
  detects immediately.
- Domain: the default-constructed configuration; assertion that the rendered
  instant is within 3 seconds of `OffsetDateTime::now_utc()` and carries
  `UtcOffset::UTC`.
- Artefact: `src/stdlib/time/tests.rs::now_defaults_to_utc` (existing,
  retained with its fixture updated);
  `tests/std_filter_tests/time_functions.rs::now_without_a_clock_reads_the_host`.
- Evidence: passes before and after the change, unchanged in substance.
- Non-vacuity: the mutation "default the clock to a fixed epoch instant"
  (a real hazard, since `Default` must be handwritten for a closure-bearing
  type) is rejected — such a default is decades from now. This is the specific
  reason the tolerance is seconds and not, say, a day.

**OBL-5 — Manifest-query mode never acquires a clock-backed `now()`.**

- Obligation: under manifest-query registration, `now()` returns the
  manifest-query operation error; and the permissive half of that registration,
  `time::register_query_functions`, does not define `now` at all.
- Method: two parameterized unit tests — one against the full
  `register_manifest_query` environment, one against a bare `Environment` to
  which only `register_query_functions` has been applied.
- Rationale: constraint C2, currently unguarded — no test in the repository
  asserts the refusal today.
- Domain: `now()` and `now(offset='+02:00')` under query registration; `now`
  absent under `register_query_functions` alone.
- Artefact: new cases beside the existing `manifest_query_environment` fixture
  at `src/manifest/expand_tests.rs:34`, which is `pub(super)` and already
  builds the restricted environment, so no visibility widening is needed.
- Evidence: the query-mode evaluation returns `Err` matching the
  `manifest_query_operation_error("now")` shape; the bare-environment
  evaluation fails as an *unknown function*, not as a refusal.
- Non-vacuity: **the obvious formulation of this obligation is vacuous, and
  the second test exists because of it.** `register_manifest_query`
  (`src/stdlib/register.rs:135`) calls `time::register_query_functions` and
  *then* `register_disabled_query_helpers`, which installs the refusing stub
  (`src/stdlib/register.rs:231`). MiniJinja's `add_function` is last-write-wins
  over the globals map, so a leaked clock-backed `now` registered in the
  earlier call would be silently overwritten by the stub — and a test that only
  asserts "query mode refuses `now`" would still pass while the leak existed.
  Asserting that `now` is *undefined* after the permissive half alone is what
  actually detects it. The implementor must confirm the last-write-wins reading
  against MiniJinja 2.24 before relying on it; if writes are not last-wins,
  record the finding and keep both assertions regardless. A witness that the
  tests reach real code: the same cases assert `timedelta()` still succeeds in
  query mode, so the environment is populated and the failure is specific to
  `now`.

**OBL-6 — Offset application preserves the injected instant.**

- Obligation: for every valid UTC offset `o`, `now(offset=o)` under a provider
  fixed at `T` yields a timestamp whose absolute instant equals `T` and whose
  offset equals `o`. Formally:
  `render(now(offset=o)).unix_timestamp() == T.unix_timestamp()` and
  `render(now(offset=o)).offset() == o`.
- Method: property test (`proptest`) over generated offsets, plus explicit
  `#[case]` boundaries.
- Rationale: this is an invariant over a range of inputs — the offset domain —
  rather than a finite partition, so a property test is the proportionate
  method. It is also the only obligation that distinguishes "re-express the
  instant in another offset" (correct) from "shift the instant by the offset"
  (wrong, and an easy mistake). The repository already uses `proptest` for
  offset parsing in this very file
  (`parse_offset_accepts_hours_below_a_civil_day`), so the generator domain is
  established precedent.
- Domain: offsets generated over the valid civil range, strictly less than one
  day in magnitude — hours in `-23..=23`, minutes and seconds in `0..=59` —
  matching the existing `parse_offset` property's domain. Explicit boundary
  cases: `Z`, `+00:00`, `+02:30`, `-05:00`, `+23:59:59`, `-23:59:59`.
- Artefact: `src/stdlib/time/tests.rs::now_offset_preserves_the_instant`
  (property) and `now_applies_offset_to_the_injected_instant` (cases).
- Evidence: `proptest` reports the configured case count with no shrunk
  counterexample; the regression file under `proptest-regressions/` gains no
  new entry.
- Non-vacuity: the generator must be shown to reach both signs and a non-zero
  minute component — assert this by including the explicit boundary cases above
  as ordinary `#[case]` tests alongside the property, so a degenerate generator
  producing only `+00:00` cannot leave the invariant untested. The designated
  mutation is replacing `timestamp.to_offset(parsed)` with an arithmetic shift
  such as `timestamp + Duration::seconds(offset)`; that mutation preserves
  `offset()` but breaks `unix_timestamp()`, and is rejected by the first
  conjunct. A second mutation — dropping the offset application entirely —
  preserves `unix_timestamp()` but breaks `offset()`, and is rejected by the
  second conjunct. Both conjuncts are therefore load-bearing; neither may be
  dropped.

**OBL-7 — The seam reaches `now()` through real registration.**

- Obligation: a clock supplied via `StdlibConfig::with_clock` is observable in
  a template rendered through `stdlib::register_with_config` — that is, the
  wiring from configuration to registered function actually connects.
- Method: integration test through the public API, plus a behavioural
  (`rstest-bdd`) scenario through the stdlib rendering steps.
- Rationale: roadmap bullet 3 requires the tests be "registered through
  `StdlibConfig`". This mirrors the two-layer argument the developers' guide
  makes for `EnvReader`: unit tests cover the leaf function, but only an
  integration test proves the provider actually *reaches* the registered Jinja
  function. Covering the leaf alone would leave the wiring untested, which is
  the whole point of the seam.
- Domain: one fixed instant rendered as `{{ now().iso8601 }}` and as
  `{{ now(offset='+02:30').iso8601 }}`.
- Artefact: `tests/std_filter_tests/time_functions.rs`;
  `tests/features/stdlib_time.feature` scenarios "A fixed clock makes now()
  deterministic" and "A fixed clock renders now() with an offset at the same
  instant".
- Evidence: exact string equality — `2026-06-08T12:00:00Z` and
  `2026-06-08T14:30:00+02:30` respectively.
- Non-vacuity: the mutation "store the clock in `StdlibConfig` but never pass
  it to `time::register_functions`" compiles cleanly and passes every unit test
  in `src/stdlib/time/tests.rs` (which registers the clock directly), yet fails
  these tests. That is precisely the wiring gap this obligation exists to
  close, and it is the reason the integration layer is mandatory rather than
  optional.

### Methods deliberately not used

- **Bounded model checking (Kani).** Rejected. The change introduces no
  `unsafe` code, no arithmetic-overflow surface of its own, and no bounded
  state machine. The only arithmetic is `to_offset`, which belongs to the
  `time` crate and is AXIOM-2. A Kani harness here would either restate AXIOM-2
  or verify third-party internals, both of which this project's plans forbid.
- **Formal proof (Verus).** Rejected. No lemma is introduced whose guarantee
  must hold over all admissible inputs beyond what OBL-6's property test covers
  within the closed, finite offset domain. The offset domain is finite and
  small (fewer than 2×86400 values); a property test over it is not
  meaningfully weaker than a proof, and a proof would rest entirely on AXIOM-2
  anyway.
- **State-machine model checking.** Rejected. There is no protocol,
  concurrency, or ordering property. The provider is `Send + Sync` and pure
  from the seam's perspective; OBL-3's counter is the only stateful fixture and
  its assertions are direct.
- **Snapshot testing (`insta`).** Rejected for this change. Snapshots earn
  their keep when output format is multivariant. `now()` renders one ISO-8601
  form, already covered by exact string equality in OBL-7, which is more
  legible than a snapshot file.

## Milestones and plateaus

Each milestone ends in a repository state that compiles, passes all gates, and
is safe to stop at.

### EP-M0 — Red tests (prototyping/red stage)

- Outcome: failing tests that specify the seam, committed before any
  production change.
- Requirements: none discharged; establishes the Red stage for OBL-1, OBL-2,
  OBL-3, OBL-6.
- Acceptance evidence: `make test` fails, and it fails *for the intended
  reason* — a compile error naming `with_clock` / `ClockProvider` as not found,
  not an unrelated failure.
- Conformance check: no production code touched; no public interface changed.
- Recovery: `git revert` the single commit.
- Remaining gaps: everything.
- Compatibility decision: none required.

Note on the Red stage: because the missing API causes a *compile* failure
rather than a test failure, the whole test target fails to build. That is an
acceptable Red signal here — the failure is unambiguous and names the missing
symbol. Record the exact compiler error in `Artefacts and notes`. Do not use
`#[ignore]` to sidestep it, and do not stub the API just to get a runtime
failure; that would weaken the Red evidence.

### EP-M1 — The port, its adapter, and the leaf function

- Outcome: `src/stdlib/time/clock.rs` exists with `ClockProvider`,
  `ClockInstant`, `system_clock()`, `fixed_clock()`, and `WallClock`, **and
  `src/stdlib/mod.rs` already re-exports the public items**; `now()` reads
  through a `WallClock` rather than calling `OffsetDateTime::now_utc()`
  directly; `time::register_functions` accepts a `WallClock`. `StdlibConfig` is
  not yet involved, so registration passes `WallClock::default()`.

  The `src/stdlib/mod.rs` re-export belongs to this milestone, not the next one:
  `clock.rs` carries rustdoc examples that `use netsuke::stdlib::…`, and
  `make test` runs doctests. Deferring the re-export would leave EP-M1 failing
  on an unresolved import, so it would not be a plateau at all.
- Requirements: advances ROADMAP-7.1.1 bullets 1 and 2; discharges OBL-1,
  OBL-2, OBL-3, OBL-4 (unit layer), OBL-6.
- Acceptance evidence: `make test` passes; the unit tests in
  `src/stdlib/time/tests.rs` named in the obligations above all pass; the
  pre-existing `now_defaults_to_utc`, `now_applies_custom_offset`,
  `now_accepts_utc_shorthand`, and `now_rejects_invalid_offset` still pass
  unmodified in substance.
- Conformance check: the port type matches technical design §5.2 verbatim
  (C3); `StdlibConfig` still derives `Debug` and `Clone` (C4); no new
  dependency (C5); manifest-query registration untouched (C2).
- Recovery: the milestone is one or two commits; revert to return to EP-M0.
- Remaining gaps: the clock is not yet configurable by callers — `StdlibConfig`
  has no `with_clock`, so OBL-7 is not yet dischargeable.
- Compatibility decision: none. `time::register_functions` is `pub(crate)`;
  its callers are updated in the same commit. No shim, no defaulted overload.

### EP-M2 — Configuration ownership and integration coverage

- Outcome: `StdlibConfig` owns the clock; `with_clock` and the accessor exist;
  `register_with_config` threads it through; integration and behavioural tests
  prove the wiring.
- Requirements: discharges ROADMAP-7.1.1 bullets 1, 2, and 3; discharges
  OBL-4 (integration layer), OBL-5, OBL-7.
- Acceptance evidence: `make test` passes; the new
  `tests/std_filter_tests/time_functions.rs` cases pass; the two new
  `stdlib_time.feature` scenarios pass with exact string equality; the new
  query-refusal test passes.
- Conformance check: the clock has exactly one owner (C6); manifest-query mode
  still refuses `now()`, now with a regression test (C2); the users' guide
  statement at line 1178 remains accurate without edit; no test mutates ambient
  state or uses `#[serial]` (C7); scope has not crept into 7.1.2 territory (C8).
- Recovery: revert to EP-M1, which is itself a coherent plateau (the seam
  works; only caller configurability is absent).
- Remaining gaps: documentation.
- Compatibility decision: none. `StdlibConfig` gains a field and a builder;
  both are additive and every construction site continues to compile because
  the field is populated by `new()`. No external consumer exists — `netsuke` is
  a binary crate whose library surface is pre-1.0 and consumed only by its own
  tests.

### EP-M3 — Gate hardening

- Outcome: all four gates green with no warnings.
- Requirements: none new; protects everything already discharged.
- Acceptance evidence: `make check-fmt`, `make typecheck`, `make lint`, and
  `make test` each exit zero, with logs captured under `/tmp`.
- Conformance check: `missing_docs`, `missing_docs_in_private_items`,
  `must_use_candidate`, and `needless_pass_by_value` are all `deny` at
  workspace level, so every new item — the alias, the supplier, the newtype,
  its fields, the builder, and the accessor — carries rustdoc and, where
  applicable, `#[must_use]`.
- Recovery: fixes are local; re-run the failing gate only.
- Remaining gaps: documentation.
- Compatibility decision: none.

### EP-M4 — Documentation and seam classification

- Outcome: the seam is recorded where ADR-008 and the technical design require.
- Requirements: discharges ROADMAP-7.1.1 bullet 4.
- Acceptance evidence: `docs/adr-008-environment-seam-taxonomy.md` has a dated
  addendum classifying the clock seam and an `Implementation references` entry
  for `src/stdlib/time/clock.rs`; `docs/developers-guide.md` documents
  ownership and the module boundary; technical design §5.2 records the
  implemented state; RFC 0006 §16 open question 7 is marked resolved with a
  pointer to the ADR-008 addendum; `make check-fmt` passes over the Markdown.
- Conformance check: technical design §15's synchronization requirement is
  satisfied; ADR-008's `Consequences` section — which requires the ADR and the
  developers' guide sections to stay consistent — is honoured by editing both
  in one commit; `docs/contents.md` needs no new entry because no new document
  file is created; `docs/users-guide.md` is deliberately unchanged (D4). RFC
  0006 §3.3's recorded gap is now closed, and §16 question 7 answered.
- Recovery: documentation-only; revert freely.
- Remaining gaps: the roadmap checkbox, which is EP-M5.
- Compatibility decision: none.

### EP-M5 — Roadmap closure

- Outcome: `docs/roadmap.md` item 7.1.1 and its four sub-bullets marked `[x]`.
- Requirements: closes ROADMAP-7.1.1.
- Acceptance evidence: the roadmap entry reads `- [x] 7.1.1.` with all four
  nested boxes ticked; `make check-fmt` passes.
- Conformance check: every bullet is genuinely satisfied by a named artefact
  from `Verification plan`; no bullet is ticked on intent alone.
- Recovery: documentation-only.
- Remaining gaps: none. Plan status becomes `COMPLETE` after the reconciliation
  described in `Outcomes & retrospective`.
- Compatibility decision: none.

## Interfaces and dependencies

Prescriptive. These signatures must exist at the end of EP-M2.

In a new file `src/stdlib/time/clock.rs`:

```rust
//! Wall-clock seam for the stdlib `now()` helper.

use std::{fmt, sync::Arc};

use time::OffsetDateTime;

use std::{fmt, sync::Arc};

use time::{OffsetDateTime, UtcOffset};

/// Re-exported so an external caller can name a provider's return type
/// without adding its own `time` dependency.
pub use time::OffsetDateTime as ClockInstant;

/// Thread-safe wall-clock source supplied to the `now()` Jinja helper.
///
/// The provider is an `Arc` rather than a `Box` for two reasons, both
/// binding: `minijinja` requires registered functions to be `Send + Sync`,
/// and `StdlibConfig` derives `Clone`, which `Box<dyn Fn>` cannot satisfy.
/// ADR-008 records the same shape for the manifest environment reader.
pub type ClockProvider = Arc<dyn Fn() -> OffsetDateTime + Send + Sync>;

/// Construct the host-backed clock provider used by production renders.
#[must_use]
pub fn system_clock() -> ClockProvider {
    Arc::new(OffsetDateTime::now_utc)
}

/// Construct a provider that always reports `instant`.
#[must_use]
pub fn fixed_clock(instant: OffsetDateTime) -> ClockProvider {
    Arc::new(move || instant)
}

/// Wall-clock source held by `StdlibConfig` and captured at registration.
///
/// Named `WallClock` to keep it distinct from the monotonic-clock vocabulary
/// already in the crate: `monotony::MonotonicClock`, the `Clock` generic
/// parameter in `src/runner/process/mod.rs`, and the private
/// `type MonotonicClock` in `src/status_timing.rs`.
#[derive(Clone)]
pub(crate) struct WallClock {
    /// Provider consulted on every `now()` evaluation.
    provider: ClockProvider,
    /// Whether this is the ambient host clock, recorded for diagnostics.
    is_system: bool,
}

impl WallClock {
    /// Wrap `provider` as the clock backing `now()`.
    pub(crate) fn new(provider: ClockProvider) -> Self {
        Self { provider, is_system: false }
    }

    /// Read the current instant, normalized to UTC.
    ///
    /// Normalization is part of the contract, not a convenience: `now()` is
    /// documented to yield a UTC timestamp, and an injected provider is free
    /// to return any offset. Without this the harness could assert behaviour
    /// production never exhibits.
    pub(crate) fn read(&self) -> OffsetDateTime {
        (self.provider)().to_offset(UtcOffset::UTC)
    }

    /// Whether the ambient host clock is installed.
    pub(crate) const fn is_system(&self) -> bool {
        self.is_system
    }
}

impl Default for WallClock {
    fn default() -> Self {
        Self { provider: system_clock(), is_system: true }
    }
}

/// Report the clock's provenance without pretending a closure is printable.
///
/// The label makes a wrongly wired clock self-diagnosing: an injected clock that
/// never reached registration, or an ambient clock where a test expected an
/// injected one, is visible in any `{:?}` of the surrounding config.
impl fmt::Debug for WallClock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WallClock")
            .field("source", &if self.is_system { "system" } else { "injected" })
            .finish_non_exhaustive()
    }
}
```

`is_system` is not decoration. It is the only signal that distinguishes a
production clock from an injected one at runtime; without it a leaked test
clock on a build path is undetectable (see the pre-mortem in `Risks`).

Both `ClockProvider` and `system_clock` carry runnable rustdoc examples,
mirroring `src/manifest/env_reader.rs`. A suitable example for the alias:

```rust
/// # Examples
///
/// ```rust
/// use netsuke::stdlib::{ClockProvider, fixed_clock};
/// use time::macros::datetime;
///
/// let instant = datetime!(2026-06-08 12:00:00 UTC);
/// let clock: ClockProvider = fixed_clock(instant);
/// assert_eq!(clock(), instant);
/// ```
```

In `src/stdlib/time/mod.rs`:

```rust
mod clock;
pub(crate) use self::clock::WallClock;
pub use self::clock::{ClockInstant, ClockProvider, fixed_clock, system_clock};

/// Register time helpers with the environment.
pub(crate) fn register_functions(env: &mut Environment<'_>, clock: WallClock) {
    env.add_function("now", move |kwargs: Kwargs| now(&kwargs, &clock));
    register_query_functions(env);
}

fn now(kwargs: &Kwargs, clock: &WallClock) -> Result<Value, Error> {
    let offset_spec: Option<String> = kwargs.get("offset")?;
    kwargs.assert_all_used()?;

    let mut timestamp = clock.read();
    if let Some(raw) = offset_spec {
        let parsed = parse_offset(&raw)?;
        timestamp = timestamp.to_offset(parsed);
    }

    Ok(Value::from_object(TimestampValue::new(timestamp)))
}
```

`register_query_functions` keeps its existing signature and body. That is not
an oversight — it is C2.

In `src/stdlib/config/mod.rs`, `StdlibConfig` gains one field and two methods:

```rust
    /// Wall-clock source backing the `now()` helper.
    clock: WallClock,
```

```rust
    /// Replace the wall-clock source backing `now()`.
    #[must_use]
    pub fn with_clock(mut self, provider: ClockProvider) -> Self {
        self.clock = WallClock::new(provider);
        self
    }

    /// Wall-clock source backing the `now()` helper.
    pub(crate) const fn clock(&self) -> &WallClock {
        &self.clock
    }
```

`StdlibConfig::new` initializes the field with `WallClock::default()`. The
accessor is `const fn` and carries no `#[must_use]`, matching the sibling
`home_directory` accessor at `src/stdlib/config/mod.rs:245`: the workspace
denies `missing_const_for_fn`, and `must_use_candidate` fires only on exported
items, so `#[must_use]` on a `pub(crate)` getter is inert noise.
`into_components` is **not** changed: the clock is read by reference before
that call consumes the configuration, exactly as `home_directory()` is.

In `src/stdlib/register.rs`, one line changes:

```rust
    time::register_functions(env, config.clock().clone());
```

In `src/stdlib/mod.rs`, extend the existing re-export list so callers outside
the crate can build a provider:

```rust
pub use self::time::{ClockInstant, ClockProvider, fixed_clock, system_clock};
```

In `tests/std_filter_tests/support.rs`, beside `stdlib_env_with_home`:

```rust
    pub(crate) fn stdlib_env_with_clock(
        provider: netsuke::stdlib::ClockProvider,
    ) -> Result<Environment<'static>> {
        let config = StdlibConfig::from_current_dir()?.with_clock(provider);
        stdlib_env_with_config(config).map(|(env, _)| env)
    }
```

No `Cargo.toml` change is permitted (C5).

## Plan of work

### Stage A — orientation (no code changes)

Read, in order: technical design §5.2; ADR-008 in full, including its addendum
sections; `src/stdlib/time/mod.rs`; `src/manifest/env_reader.rs`;
`src/stdlib/config/mod.rs` lines 1–120; `src/stdlib/register.rs` lines 55–145;
`src/stdlib/time/tests.rs`; `src/runner/process/command_env.rs` lines 55–85 for
the manual-`Debug` precedent.

Confirm three facts against the working tree before writing code, because the
whole design rests on them: that `StdlibConfig` derives `Debug`; that
`time::register_functions` is called before `config.into_components()`; and that
`register_manifest_query` installs a refusing `now` stub. If any has changed,
stop — the seam's shape may need revisiting.

Validation for this stage: no build required; you have simply read the code.

### Stage B — red tests

Write the failing tests first. In `src/stdlib/time/tests.rs`, add the fixtures
and cases for OBL-1, OBL-2, OBL-3, and OBL-6, referencing the not-yet-existing
`WallClock`, `ClockProvider`, and the two-argument `register_functions`. Update
the existing `env` fixture to the new signature.

The existing fixture is:

```rust
#[fixture]
fn env() -> Environment<'static> {
    let mut env = Environment::new();
    register_functions(&mut env);
    env
}
```

It becomes:

```rust
#[fixture]
fn env() -> Environment<'static> {
    let mut env = Environment::new();
    register_functions(&mut env, WallClock::default());
    env
}

/// Environment whose `now()` always reports `instant`.
fn env_with_fixed_clock(instant: OffsetDateTime) -> Environment<'static> {
    let mut env = Environment::new();
    register_functions(&mut env, WallClock::new(fixed_clock(instant)));
    env
}

/// Environment whose `now()` reports `first`, then each element of `rest` in
/// turn, saturating at the last, and recording how many times the provider was
/// consulted.
///
/// Taking `first` separately makes non-emptiness a type-level precondition.
fn env_with_sequenced_clock(
    first: OffsetDateTime,
    rest: &[OffsetDateTime],
) -> (Environment<'static>, Arc<AtomicUsize>) {
    let mut instants = vec![first];
    instants.extend_from_slice(rest);
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&calls);
    let provider: ClockProvider = Arc::new(move || {
        let index = counter.fetch_add(1, Ordering::SeqCst);
        instants
            .get(index)
            .or_else(|| instants.last())
            .copied()
            .unwrap_or(first)
    });
    let mut env = Environment::new();
    register_functions(&mut env, WallClock::new(provider));
    (env, calls)
}
```

The sequenced fixture is the OBL-3 negative control. Three details are
deliberate. It saturates rather than panicking past the end, so an over-reading
implementation fails on the *count* assertion with a legible message rather
than panicking opaquely inside MiniJinja evaluation. It takes `first` plus
`rest` rather than a `Vec`, which makes non-emptiness a type-level precondition
and removes the `instants.len() - 1` underflow an empty vector would cause. And
it reads with `get(..).or_else(..).copied()` rather than `instants[..]`,
because the workspace denies `clippy::indexing_slicing` (`Cargo.toml:215`), and
`clippy.toml`'s `allow-expect-in-tests` keys on `#[test]`-attributed functions,
which a plain helper is not.

Assert the invocation count against the number of evaluations the test actually
performs, not a hardcoded literal. Hardcoding `3` silently makes AXIOM-5
load-bearing, so a wrong assumption about MiniJinja's evaluation behaviour
would surface as an apparent implementation bug rather than as the axiom
violation it is.

Use `googletest::prelude::*` with `assert_that!` for the equality assertions and
`pretty_assertions::assert_eq` where a plain equality diff is clearer, per the
conventions in `src/cli/discovery_layer_tests.rs`. Keep `#[rstest]` as the
outermost test attribute, followed by `#[case]` attributes, matching that file.

Validation:
`make test 2>&1 | tee /tmp/test-netsuke-7-1-1-clock-provider-seam.out` must
fail with a compile error naming the missing items. Record the error text in
`Artefacts and notes`. Commit.

### Stage C — implementation

Create `src/stdlib/time/clock.rs` exactly as specified in
`Interfaces and dependencies`. Declare and re-export it from
`src/stdlib/time/mod.rs`. Change `register_functions` and `now` to take the
clock. Update `src/stdlib/register.rs` to pass `WallClock::default()` for now —
`StdlibConfig` is not yet involved. This completes EP-M1.

Then add the `clock` field, `with_clock`, and `clock()` to `StdlibConfig`;
initialize the field in `new()`; re-export `ClockProvider` and `system_clock`
from `src/stdlib/mod.rs`; change `src/stdlib/register.rs` to pass
`config.clock().clone()`. Add the integration tests and their support helper,
wire the new module into `tests/std_filter_tests.rs`, and add the BDD scenarios
and step. This completes EP-M2.

For the BDD work specifically: `RenderConfig` and `extract_render_config` in
`tests/bdd/steps/stdlib/rendering.rs:28` gain a `clock: Option<OffsetDateTime>`
member sourced from a new `TestWorld` field, and `render_template_with_context`
applies it alongside the existing policy, home, and limit overrides:

```rust
    if let Some(instant) = render_cfg.clock {
        config = config.with_clock(Arc::new(move || instant));
    }
```

The new step follows the file's existing attribute conventions:

```rust
#[given("the stdlib clock is fixed at {instant:string}")]
pub(crate) fn given_stdlib_clock_fixed_at(world: &TestWorld, instant: &str) -> Result<()> {
    let parsed = parse_iso_timestamp(instant)?;
    world.stdlib_clock.set_value(parsed);
    Ok(())
}
```

Add to `tests/features/stdlib_time.feature`:

```gherkin
  Scenario: A fixed clock makes now() deterministic
    Given a stdlib workspace
    And the stdlib clock is fixed at "2026-06-08T12:00:00Z"
    When I render the stdlib template "{{ now().iso8601 }}" without context
    Then the stdlib output equals "2026-06-08T12:00:00Z"

  Scenario: A fixed clock renders now() with an offset at the same instant
    Given a stdlib workspace
    And the stdlib clock is fixed at "2026-06-08T12:00:00Z"
    When I render the stdlib template "{{ now(offset='+02:30').iso8601 }}" without context
    Then the stdlib output equals "2026-06-08T14:30:00+02:30"
```

Both reuse the existing `Then the stdlib output equals` step, so no new
assertion step is needed. Scenarios are auto-discovered by
`scenarios!("tests/features", ...)` in `tests/bdd_tests.rs`; no registration is
required.

Leave the existing scenario "Rendering now() yields a UTC timestamp" and its
5-second-tolerance step untouched: it is the ambient-fallback behavioural
coverage, and its tolerance is correct for a path with no injected clock.

Validation after each of EP-M1 and EP-M2: `make test`, teeing to
`/tmp/test-netsuke-7-1-1-clock-provider-seam.out`. Commit at each milestone.

### Stage D — gates, documentation, and closure

Run the full gate sequence (EP-M3), then write the documentation (EP-M4), then
mark the roadmap (EP-M5).

For EP-M4, the ADR-008 addendum entry should read along these lines, matching
the file's existing dated-addendum format and its plain, decision-first prose:

> ### 2026-09-08: Stdlib clock seam
>
> The stdlib `now()` helper reads its instant through an injected
> `ClockProvider`, an `Arc<dyn Fn() -> OffsetDateTime + Send + Sync>` held by
> `StdlibConfig` and captured by the registered Jinja function. It takes the
> `EnvReader` shape, not a narrow closure and not `mockable::Env`, for the
> same reason `EnvReader` does: `minijinja` requires registered functions to
> be `Send + Sync`, so a borrowed closure parameter cannot satisfy the bound.
> `StdlibConfig` is the clock's single owner; `system_clock()` is the sole
> production supplier and the only place `OffsetDateTime::now_utc` is called
> for `now()`. Manifest-query registration receives no clock and keeps its
> refusing `now` stub.
>
> The taxonomy is being applied here to an ambient input that is *not* an
> environment variable. This ADR's context section is written about
> `clippy.toml`'s ban on `std::env::var` and friends, and no lint forbids
> reading the clock; the shape rubric transfers, the original scope does not.
> Note also that `StdlibConfig` now holds two ambient seams in two shapes —
> `home_directory: HomeDirectory`, a resolved value, and the clock, a closure.
> The clock's shape is the one this rubric prescribes for a `Send + Sync`
> registration point; `HomeDirectory` is the outlier, and the two should not be
> "harmonized" without revisiting this entry. `ClockProvider` also puts
> `time::OffsetDateTime` on netsuke's public surface, so a `time` 0.4 bump is a
> breaking library-API change.
>
> `mockable::Clock` was not used: it is typed in `chrono`, which this
> workspace does not depend on, so adopting it would add a second date-time
> crate to render one timestamp. `monotony`, already a dependency, abstracts
> only monotonic elapsed time and has no wall-clock type.

Note the ADR's `Implementation references` list should gain an entry for
`src/stdlib/time/clock.rs`, and its `Consequences` section already requires
that the developers' guide sections stay consistent — so update "Environment
and template ports" in the same commit.

Validation: `make check-fmt` after every Markdown edit; run `make fmt` first if
it complains.

## Concrete steps

All commands run from the repository root,
`/home/leynos/.lody/repos/github---leynos---netsuke/worktrees/351bfe82-830e-43c4-843d-afc411661788`.

Confirm the branch before starting:

```bash
git branch --show-current
```

```plaintext
7-1-1-clock-provider-seam
```

Gate commands, always teed so long output can be reviewed after the fact:

```bash
make check-fmt 2>&1 | tee /tmp/check-fmt-netsuke-7-1-1-clock-provider-seam.out
make typecheck 2>&1 | tee /tmp/typecheck-netsuke-7-1-1-clock-provider-seam.out
make lint      2>&1 | tee /tmp/lint-netsuke-7-1-1-clock-provider-seam.out
make test      2>&1 | tee /tmp/test-netsuke-7-1-1-clock-provider-seam.out
```

Run them sequentially, never in parallel — the build cache is shared and
concurrent cargo invocations contend on the package-cache lock. Do not create
an isolated Cargo cache; if another job holds the lock, wait.

To run only the time tests while iterating:

```bash
cargo nextest run --all-features -E 'test(/stdlib::time/)' 2>&1 \
  | tee /tmp/test-netsuke-7-1-1-clock-provider-seam.out
```

Expected on success, approximately:

```plaintext
    Summary [   0.412s] 18 tests run: 18 passed, 0 skipped
```

To run only the new integration and behavioural coverage:

```bash
cargo nextest run --all-features -E 'binary(std_filter_tests) or binary(bdd_tests)'
```

Doctests are not executed by nextest and must be run through `make test`, which
chains `test-nextest` and `doctest`. The new rustdoc examples on
`ClockProvider` and `system_clock` are only exercised there.

Commit after each milestone, using the repository's file-based commit-message
convention rather than `-m`.

## Validation and acceptance

Acceptance is behavioural, not structural.

**Red evidence.** Before any production change, `make test` fails to compile
`src/stdlib/time/tests.rs` with errors naming `WallClock`, `ClockProvider`, and
the arity of `register_functions`. Capture the first three compiler errors
verbatim in `Artefacts and notes`.

**Green evidence, unit layer.** After EP-M1, `make test` passes.
`now_uses_injected_clock` renders a timestamp exactly equal to
`2026-06-08T12:00:00Z`. `now_reads_the_provider_on_every_call` observes three
distinct instants from three evaluations and an invocation count of exactly 3.
`now_defaults_to_utc` still passes against the real clock.

**Green evidence, integration layer.** After EP-M2, rendering
`{{ now().iso8601 }}` in an environment built by
`StdlibConfig::from_current_dir()?.with_clock(fixed)` and registered through
`stdlib::register_with_config` produces the exact string
`2026-06-08T12:00:00Z`. Rendering `{{ now(offset='+02:30').iso8601 }}` under
the same configuration produces exactly `2026-06-08T14:30:00+02:30`. The two
new Gherkin scenarios pass.

**Regression evidence.** `now()` under `register_manifest_query` still returns
the manifest-query operation error, while `timedelta()` under the same
registration still succeeds.

**Mutation evidence (required, run once and discarded).** In a scratch commit
that is never pushed, apply each of the six designated mutations in turn and
confirm the named test fails for the intended reason:

1. Replace `clock.read()` with `OffsetDateTime::now_utc()` →
   `now_uses_injected_clock` fails.
2. Hoist the clock read out of `now` into `register_functions`, baking the
   instant into the closure → `now_reads_the_provider_on_every_call` fails.
3. Replace `timestamp.to_offset(parsed)` with an arithmetic shift →
   `now_offset_preserves_the_instant` fails on the `unix_timestamp` conjunct.
4. Store the clock on `StdlibConfig` but pass `WallClock::default()` in
   `register_with_config` → the integration and BDD tests fail while every unit
   test still passes.
5. Delete the refusing `now` stub at `src/stdlib/register.rs:231` → the
   query-mode refusal case in OBL-5 fails. This is the mutation OBL-5's *first*
   test catches; its second test exists for the leak this one hides.
6. Remove the UTC normalization from `WallClock::read` → the
   non-UTC-provider case in OBL-1 fails on the offset assertion.

Then `git reset --hard` back to the real commit. Record the six outcomes in
`Artefacts and notes`. Mutation 4 is the most important: it is the only one
that demonstrates the integration layer is load-bearing.

**Quality criteria — what "done" means.**

- Tests: `make test` exits zero, covering nextest and doctests.
- Verification: OBL-1 through OBL-7 discharged, each by its named artefact,
  with the four mutations rejected.
- Lint and typecheck: `make lint` and `make typecheck` exit zero with warnings
  denied.
- Formatting: `make check-fmt` exits zero, including the Markdown gate.
- Performance: no benchmark threshold applies. One `Arc` clone per stdlib
  registration and one virtual call per `now()` evaluation are immaterial
  against template rendering; do not add a benchmark.
- Security: none applicable. The seam removes an ambient read rather than
  adding one, and exposes no new host state.

**Quality method.** The four `make` gates, run sequentially, plus the one-off
mutation exercise above. Prefer delegating the full gate run to the
`scrutineer` subagent, which runs the gates sequentially, captures each log
under `/tmp`, and returns a bounded report.

## Idempotence and recovery

Every step is re-runnable. The `make` gates are read-only with respect to
tracked files except `make fmt`, which rewrites formatting deterministically —
running it twice changes nothing the second time.

No step is destructive. There is no migration, no persisted format, and no data
to back up. Recovery at any point is `git revert` of the milestone commit or
`git reset --hard` to the previous milestone; each milestone is a coherent,
gate-passing plateau.

The mutation exercise in `Validation and acceptance` is the only step that
deliberately breaks the tree. Perform it on a scratch commit and discard it with
`git reset --hard`; never push it. If interrupted mid-exercise, `git status`
will show the mutation, and `git checkout -- <file>` restores it.

Working-tree cleanliness: this plan adds no build artefacts, no temporary files
inside the repository, and no `/tmp` output other than the gate logs named
above, which are disposable.

## Progress

- [x] EP-M0 — Red tests committed; compile failure captured.
- [x] EP-M1 — `clock.rs`, threaded `now()`, unit obligations discharged;
  merged with EP-M2's configuration ownership (see Surprises & discoveries),
  and OBL-5 added.
- [x] EP-M2 — integration and BDD coverage over the real registration path.
- [x] EP-M3 — All four gates green, plus the repository's Markdown and diagram
  gates and the first CodeRabbit pass (zero findings).
- [x] EP-M4 — ADR-008 addendum plus `Implementation references` entry,
  developers' guide "Environment and template ports", technical design §5.2,
  RFC 0006 §3.3 and §16 question 7; `make check-fmt` green.
- [x] EP-M5 — Roadmap 7.1.1 and its four sub-bullets marked done, each mapped
  to a named artefact; `Outcomes & retrospective` completed and the plan set to
  `COMPLETE`.

## Surprises & discoveries

Recorded during planning; extend during implementation.

- Observation: `mockable`, already a dependency and already the source of the
  `Env` seam trait, does export a `Clock` trait with a `MockClock`. Evidence:
  `https://docs.rs/mockable/3.0.0/mockable/trait.Clock.html` declares
  `pub trait Clock: Send + Sync`, with required methods
  `fn local(&self) -> DateTime<Local>` and `fn utc(&self) -> DateTime<Utc>`.
  Impact: it is unusable here — it is typed in `chrono`, and `chrono` appears
  nowhere in `Cargo.lock`. Adopting it would add a second date-time crate
  purely to render one timestamp, and would require converting to
  `time::OffsetDateTime` at the boundary. Recorded so a future reviewer does
  not re-raise it. See D3.

- Observation: `monotony` (a dependency, used for elapsed-time telemetry) has
  a `test-util` feature with deterministic clocks, which looks like a
  ready-made answer. Evidence: `https://docs.rs/monotony/1.0.0/monotony/`
  exports `MonotonicClock`, `MonotonicClockExt`, `StdMonotonicClock`, and
  `test_util`; its own summary is "Monotonic clock abstractions for
  deterministic elapsed-time measurement". Impact: not applicable — it
  abstracts monotonic elapsed time, not wall-clock calendar instants. `now()`
  needs an `OffsetDateTime`. See D3.

- Observation: no test anywhere in the repository asserts that `now()` is
  refused in manifest-query mode, despite `docs/users-guide.md:1178` promising
  it. Evidence: the refusing stub at `src/stdlib/register.rs:231` has no
  corresponding test; searching the test tree for query-mode `now()` coverage
  returns nothing. Impact: this plan adds that regression test (OBL-5). It is a
  pre-existing gap, not one introduced here, but the clock seam is exactly the
  change that could breach it silently.

- Observation: RFC 0006 already recorded this exact gap and left it as a
  numbered open question, which no roadmap item or design document
  cross-references. Evidence:
  `docs/rfcs/0006-ansible-inspired-template-standard-library.md` §3.3 records
  "`now` has no injected clock seam", and §16 question 7 asks "Does `now` need
  an injected clock seam? … a future slice that wants deterministic time tests
  will have to answer it." Impact: RFC 0006 joins the conformance basis, and
  closing its open question becomes an EP-M4 deliverable. Without this, the
  work would ship leaving a stale open question in an upstream artefact.

- Observation: ADR-008's jurisdiction is narrower than its title implies — it
  is scoped to environment *variables*, and no lint forbids reading the clock.
  Evidence: the ADR's context section is written entirely about `clippy.toml`'s
  ban on `std::env::var` and friends; `clippy.toml`'s `disallowed-methods` list
  names only `std::env::*` and `std::env::set_current_dir`. Impact: applying
  the taxonomy to a clock is an extension that must be stated rather than
  assumed. See D11.

- Observation: `StdlibConfig` derives `Debug`, which no closure-bearing field
  can satisfy. Evidence: `src/stdlib/config/mod.rs:20`. Impact: forced the
  `WallClock` newtype rather than a bare `Option<ClockProvider>` field. See D2.

- Observation: the EP-M1/EP-M2 split is not a compiling plateau, so the two
  milestones landed as one commit. Evidence: `WallClock::new` is called only
  from `#[cfg(test)]` code until `StdlibConfig` owns the clock, and
  `is_system()` had a `Debug` impl that read the field directly, so
  `RUSTFLAGS="-D warnings" cargo check --workspace --all-targets --all-features`
  failed with:

  ```plaintext
  error: associated items `new` and `is_system` are never used
     --> src/stdlib/time/clock.rs:100:25
      |
   98 | impl WallClock {
      | -------------- associated items in this implementation
   99 |     /// Wrap `provider` as the clock backing `now()`.
  100 |     pub(crate) const fn new(provider: ClockProvider) -> Self {
      |                         ^^^
  ...
  118 |     pub(crate) const fn is_system(&self) -> bool {
      |                         ^^^^^^^^^
      = note: `-D dead-code` implied by `-D warnings`
  ```

  Impact: EP-M1 carries EP-M2's configuration ownership, and EP-M2 is now the
  integration and behavioural layer. Silencing the lint was rejected: the
  workspace denies `allow_attributes`, and a suppressive attribute would have
  hidden the real signal — that the seam had no production consumer yet.
  Routing the `Debug` impl through `self.is_system()` rather than through the
  field supplies the accessor's caller, so what the label reports and what the
  accessor returns cannot drift.

- Observation: OBL-5's second case cannot be written where the plan placed it.
  Evidence: `time::register_query_functions` is `pub(crate)` inside
  `src/stdlib/time/mod.rs` and `stdlib::time` is a private module, so
  `crate::stdlib::time::register_query_functions` is unreachable from
  `src/manifest/expand_tests.rs`. Re-exporting it at `stdlib` would itself be
  an unused import in non-test builds, which the workspace also denies. Impact:
  both OBL-5 cases live in `src/stdlib/time/tests.rs`, where both registration
  halves are already in scope. The plan's intent — two paired cases, no
  visibility widening — is met; only the file changed.

- Observation: the `Given` step for the fixed clock belongs in
  `tests/bdd/steps/stdlib/config.rs`, not `rendering.rs` as the plan proposed.
  Evidence: every other stdlib `given` step that sets a `RenderConfig` field
  (`the stdlib fetch response limit is ...`,
  `the stdlib command output limit is ...`) is in `config.rs`, while
  `rendering.rs` holds only `when` steps. Impact: the step follows the
  established convention; `RenderConfig.clock` and its consumption stay in
  `rendering.rs`.

- Observation: `now()`'s rendered spelling is fixed by `time`'s well-known
  format, so the new string assertions were written from the format contract
  rather than fitted to captured output. Evidence: `format_offset_datetime`
  formats through `Iso8601::DEFAULT`, documented as "separators (such as `-` and
  `:`) are included" and "the UTC offset has precision to the minute";
  `format_offset` writes a bare `Z` when `offset.is_utc()`; and the formatter
  strips a zero fractional part. Impact: the expectations are
  `2026-06-08T12:00:00Z` for UTC and `2026-06-08T14:00:00+02:00` for an instant
  re-expressed at `+02:00`.

- Observation: a manifest-query refusal assertion cannot use
  `Error::to_string()`. Evidence: `Display for Error` writes the kind and the
  detail joined by a colon, so the refusal renders as
  `invalid operation: now is disabled while rendering ...`, which fails a
  `starts_with("now is disabled")` assertion *against correct behaviour*.
  Impact: the OBL-5 cases assert on `Error::detail()`, which returns the
  message without the kind prefix.

- Observation: `now()` normalizing its provider is observable only through the
  rendered string, because `Z` is emitted only for a UTC offset. Evidence:
  `format_offset` returns early with `b"Z"` when `offset.is_utc()`. Impact: the
  integration case for a provider carrying `+05:30` proves normalization by
  rendering `2026-06-08T12:00:00Z`; a separate offset-attribute assertion would
  have been redundant, so none was added.

- Observation: `clippy::shadow_reuse` is denied workspace-wide, so a step
  function may not reuse its capture name for a parsed binding. Evidence:
  `make lint` failed with

  ```plaintext
  error: `instant` is shadowed
    --> tests/bdd/steps/stdlib/config.rs:20:9
     |
  20 |     let instant = parse_iso_timestamp(instant)?;
     |         ^^^^^^^
     = note: requested on the command line with `-D clippy::shadow-reuse`
  ```

  Impact: the parsed binding is named `parsed`, which also matches the existing
  convention in `tests/bdd/steps/stdlib/assertions.rs`. Note that
  `RUSTFLAGS="-D warnings" cargo check --all-targets` does *not* catch this;
  only Clippy does, so a clean `cargo check` is not evidence that the lint gate
  will pass.

- Observation: the Whitaker suite caps a module at 400 lines, and the cap
  applies per file rather than recursively. Evidence: `make lint` failed with
  `error: Module tests spans 472 lines, exceeding the allowed 400.` at
  `src/stdlib/time/mod.rs:240:5`. Reading the lint's implementation confirms
  `module_max_lines` spans each out-of-line module across its own file only, so
  sibling modules declared in `mod.rs` each get their own budget. Impact:
  `src/stdlib/time/tests.rs` was split into `clock_tests.rs` (the seam: where
  `now()` reads its instant, plus the C2 query-mode guarantees), `tests.rs`
  (clock-independent behaviour: offset parsing, formatting, `timedelta`), and
  `tests_support.rs` (the fixtures and helpers both need), all three declared
  under `#[cfg(test)]` in `src/stdlib/time/mod.rs`. This mirrors the existing
  `network` and `command` test layout. The test count is unchanged at 49, so
  the split is behaviour-preserving.

- Observation: this branch predates the Makefile's `GO_BIN` curation, so
  `make lint` cannot find `actionlint`. Evidence:
  `make: actionlint: No such file or directory` /
  `Makefile:204: github-actions-lint` while `/home/leynos/go/bin/actionlint`
  exists. Impact: `make lint` must be run as
  `PATH="$HOME/go/bin:$PATH" make lint` on this branch; the other gates are
  unaffected. This is a branch-state artefact of the base commit, not something
  this change introduced, and it disappears once the branch is rebased past the
  `GO_BIN` commit.

- Observation: the plan's mutation 3, written literally as
  `timestamp + Duration::seconds(offset)`, is a *no-op* and therefore does not
  kill the test it names. Evidence: `OffsetDateTime::replace_offset` preserves
  the **wall-clock** time, not the instant — measured directly,
  `datetime!(2026-06-08 12:00:00 UTC).replace_offset(+05:30)` yields
  `12:00:00 +05:30` with `unix_timestamp` 1780900200, which is 19 800 s
  *before* the original. Adding `Duration::seconds(19_800)` back moves the
  instant forward by exactly that much, so the two errors cancel and
  `now_offset_preserves_the_instant` passes. Impact: the mutation was re-run as
  `(timestamp + Duration::seconds(offset_seconds)).to_offset(parsed)`, which
  moves the instant while still setting the requested offset; that version is
  rejected with minimal failing input `+00:00:01`. The distinction matters
  beyond the exercise: `to_offset` (preserve the instant) and `replace_offset`
  (preserve the wall time) are near-miss names, and the seam's contract is the
  former. `WallClock::read` and `now()` both use `to_offset`, and the property
  test now pins that choice.

- Observation: RFC 0006 §3.3's first recorded gap — "sixteen names registered in
  the full environment are absent from the manifest-query environment
  altogether" — is stale, independently of this change. Evidence:
  `register_disabled_query_helpers` (`src/stdlib/register.rs`) now stubs
  fifteen names: the original six (`env`, `glob`, `fetch`, `shell`, `grep`,
  `contents`) plus nine added by the `netsuke help targets` work (`realpath`,
  `expanduser`, `size`, `linecount`, `hash`, `digest`, `which` in both
  namespaces, `command_available`, `now`). Only the seven file tests (`dir`,
  `file`, `symlink`, `pipe`, `block_device`, `char_device`, `device`) are still
  absent, because `register_manifest_query` never calls `register_file_tests`.
  The totals reconcile exactly: 6 + 16 = 22 = 15 stubs + 7 absent. Impact: the
  same count also bound RFC 0006 §14.1's slice-0 deliverable, which promised a
  stub for "every one of the sixteen"; both were corrected in place rather than
  worked around, per `Outcomes & retrospective`. Slice 0's remaining stub work
  is the seven file tests. Out of scope for 7.1.1 but recorded here because
  leaving a known-false claim beside the edit would have been worse; no runtime
  behaviour changes.

- Observation: the ADR-008 addendum heading date is 2026-09-11, not the
  `2026-09-08` written in Stage D. Evidence: the plan's text prescribed the
  heading "along these lines" and the file's convention is that each addendum
  is dated when it is written (`2026-08-30`, `2026-09-01`, `2026-08-25` are all
  landing dates, and the ADR was untouched between planning and this commit).
  Impact: the entry records the date the classification was actually added, so
  a later reader comparing it with `git log` sees the same day. The prescribed
  body text is otherwise used verbatim, with `time::OffsetDateTime` on the
  public surface noted as prescribed.

- Observation: the spelling gate is not part of `make check-fmt`. Evidence: the
  Makefile declares `markdownlint: spelling`, and `spelling` runs
  `scripts/typos_rollout_check.py` plus `typos` over every Markdown file; a
  branch can therefore be `check-fmt`-green and still fail on prose. Two words
  this change added were rejected: `hand-written` (flagged at
  `docs/netsuke-test-framework-technical-design.md:280`) and `mis-wired`
  (`docs/execplans/7-1-1-clock-provider-seam.md`). Impact: `typos` splits on
  the hyphen, so `mis` is read as a standalone word and reported as a
  misspelling of `miss`; the fix is to avoid the hyphen, not to add an ignore.
  Both are now `handwritten` and `wrongly wired`. Markdown gates must be run
  through `make markdownlint`, not `make check-fmt` alone.

## Decision log

- **D1 — Port shape follows the `EnvReader` precedent verbatim.**
  Decided: `ClockProvider = Arc<dyn Fn() -> OffsetDateTime + Send + Sync>`.
  Rationale: fixed by technical design §5.2, and independently correct under
  ADR-008's taxonomy. The taxonomy offers three shapes — a narrow closure
  parameter, the `mockable::Env` trait, or an `Arc` closure — chosen by
  call-site count and by whether the registration point requires `Send + Sync`.
  MiniJinja's `add_function` requires `Send + Sync`, which ADR-008 itself calls
  "a real constraint, not a preference". That argument is necessary but **not
  sufficient**, and this plan should not rest on it alone:
  `Box<dyn Fn() -> OffsetDateTime + Send + Sync>` satisfies MiniJinja's bound
  too. The decisive constraint is `Clone`. `StdlibConfig` derives it
  (`src/stdlib/config/mod.rs:20`), a `Box` cannot provide it, and C4 forbids
  dropping the derive — so `Arc` is forced by two independent constraints
  rather than the one ADR-008 names. A narrow closure parameter cannot be
  captured by a registered function at all; a trait object would add
  indirection with no extra test-surface benefit for a single-call-site
  boundary. Date/Author: 2026-09-08, planning; strengthened after design review.

- **D2 — Wrap the provider in a `WallClock` newtype with a handwritten `Debug`
  .** Decided: `StdlibConfig` holds `clock: WallClock`, not
  `clock: Option<ClockProvider>`. Rationale: `StdlibConfig` derives `Debug`
  (C4), and `Arc<dyn Fn>` is not `Debug`, so a bare field would force a
  handwritten thirteen-field `Debug` on `StdlibConfig` that would drift every
  time a knob is added. A one-field newtype confines the handwritten impl to
  the one type that needs it. The repository already does exactly this for
  `CommandEnv` (`src/runner/process/command_env.rs:73`), which uses
  `debug_struct(..).finish_non_exhaustive()`. Choosing a non-`Option` field
  whose `Default` is `system_clock()` also mirrors `EnvReader` — which has no
  `Option` either, using `process_env_reader()` as the production supplier —
  and avoids an `Option` branch in `now()`, which the workspace's
  `option_if_let_else = "deny"` lint would scrutinize. The public vocabulary
  remains `ClockProvider` exactly as the design specifies; `WallClock` is the
  container, not a replacement for the port type. Date/Author: 2026-09-08,
  planning.

- **D3 — Do not use `mockable::Clock` or `monotony`.**
  Decided: hand-roll the provider on the `time` crate. Rationale: evidenced in
  `Surprises & discoveries`. `mockable::Clock` sits behind that crate's `clock`
  feature, which this workspace does not enable, and it is `chrono`-typed —
  `chrono` appears nowhere in `Cargo.lock`. Adopting it would mean enabling a
  new feature, adding a second date-time crate, and converting
  `chrono::DateTime<Utc>` to `time::OffsetDateTime` at the boundary, all to
  render one timestamp. That breaches C5. `monotony` covers monotonic elapsed
  time only (`MonotonicClock::now() -> Instant`) and has no wall-clock type, so
  it cannot express a UTC calendar instant at all. Both were checked against
  their published API documentation rather than assumed. Date/Author:
  2026-09-08, planning.

- **D4 — `docs/users-guide.md` is deliberately not modified.**
  Decided: no users' guide change. Rationale: the change is invisible to
  manifest authors. `now()` accepts the same arguments, returns the same
  values, and fails the same way. The one users' guide sentence that mentions
  `now()` — line 1178, stating that manifest queries reject "the clock-dependent
  `now()` function" — remains true and is protected by C2 and OBL-5. Editing
  it would imply a behaviour change that has not occurred. If a reviewer
  disagrees, the correct response is to add the users' guide entry when 7.2
  exposes `given.clock.now` to authors, not now. Date/Author: 2026-09-08,
  planning.

- **D5 — Keep the existing tolerance-based ambient assertions.**
  Decided: `now_defaults_to_utc` (3-second tolerance) and the BDD step
  `assert_stdlib_output_is_utc_timestamp` (5-second tolerance) are retained
  unchanged. Rationale: they are the ambient-fallback coverage C1 and OBL-4
  require. A tolerance is not a defect on a path that deliberately reads the
  real clock; it is the only available oracle. Tightening them would make them
  flaky without testing anything new. The determinism this plan delivers
  belongs to the *injected* path, which gets exact-equality assertions instead.
  Date/Author: 2026-09-08, planning.

- **D6 — No `ortho_config` involvement.**
  Decided: the clock is not a layered configuration option. Rationale:
  `ortho_config` governs user-facing, layered CLI and file configuration. The
  clock provider is an internal test seam — a closure, not a serializable value
  — with no command-line flag, no configuration-file key, and no localized
  help. Technical design §5.2 places it in `StdlibConfig`, which is
  registration wiring, not the `ortho_config`-derived CLI configuration struct.
  Exposing a clock override to end users would be a behaviour change nobody has
  asked for and would breach C8. Date/Author: 2026-09-08, planning.

- **D7 — Two test layers, both mandatory.**
  Decided: unit tests in `src/stdlib/time/tests.rs` *and* integration tests
  through `StdlibConfig`. Rationale: the developers' guide makes this argument
  explicitly for `EnvReader`, and it applies unchanged: unit tests at the leaf
  cannot prove the provider reaches the registered function, and an
  implementation that stores the clock but never passes it to
  `time::register_functions` passes every unit test. Roadmap bullet 3
  independently requires the tests be "registered through `StdlibConfig`".
  OBL-7's non-vacuity argument is built on precisely this mutation.
  Date/Author: 2026-09-08, planning.

- **D8 — Verification stops at property tests; no Kani or Verus.**
  Decided: `proptest` for the offset invariant, parameterized tests elsewhere.
  Rationale: recorded in full under "Methods deliberately not used". The change
  adds no `unsafe`, no bounded state machine, and no lemma independent of the
  `time` crate's documented `to_offset` contract (AXIOM-2). A Kani harness or
  Verus proof here would restate an axiom, which this project's plan standard
  explicitly calls a vacuous discharge. Date/Author: 2026-09-08, planning.

- **D9 — The query-refusal regression tests live beside the existing
  manifest-query fixture.** Decided: put them next to
  `manifest_query_environment` in `src/manifest/expand_tests.rs:34`. Rationale:
  an earlier draft left this to the implementor, which was a mistake. OBL-5 is
  the sole guard for C2 and is exactly the item that gets dropped when a
  milestone runs long, so leaving its location under-specified put the plan's
  most fragile obligation at the greatest risk. The fixture already exists, is
  `pub(super)`, and already calls `register_manifest_query`, so no visibility
  widening is needed and there is no real judgement call to delegate.
  Date/Author: 2026-09-08, planning; revised after design review.

- **D10 — Rejected: a resolved-value enum in the `HomeDirectory` shape.**
  Considered: `enum WallClock { Ambient, Fixed(OffsetDateTime) }`, mirroring
  `HomeDirectory` (`src/stdlib/config_types.rs:24`), the sibling seam in this
  very module, which ADR-008 describes as the pattern that "injects a resolved
  *value* rather than a closure". It would derive `Debug` and `Clone` for free,
  removing D2's handwritten impl entirely. This is the strongest alternative
  and a reviewer should expect it to have been weighed. Decided: rejected, in
  favour of the `Arc` closure. Rationale: the decisive reason is cohesion, not
  testability. An earlier draft of this plan claimed the enum makes OBL-3
  *unwriteable*; **that claim was wrong and has been withdrawn.** An enum could
  carry a `Sequence(Arc<Mutex<..>>)` variant and preserve the negative control
  exactly. The real objection is narrower and survives scrutiny: preserving
  OBL-3 under the enum requires adding a *test-only variant to a production
  type*, which re-implements a closure badly and forces every `match` site to
  grow an arm servicing a case production never takes. `HomeDirectory` earns
  its enum because all three of its variants — `Ambient`, `Missing`,
  `Explicit` — are production states; a `Sequence` variant would not be.
  Secondarily, a closure keeps a future advancing or scripted clock expressible
  without another redesign. Technical design §5.2's normativity is a **tiebreak
  here, not the argument**. It cannot be decisive on its own while D2
  simultaneously adds a `WallClock` container the design does not name and
  resolves that by amending the design at EP-M4; invoking "normative by exact
  type" against the enum while treating the newtype as a mechanical addition
  would be applying the same rule two ways. Independent corroboration: every
  comparable library models a clock as a *callable*, never a resolved value —
  Java's `java.time.Clock` (with `Clock.fixed` beside `Clock.systemUTC`), Go's
  `clockwork`, and Rust's `quanta` and `mock_instant`. The `Debug` objection
  the enum answers is real but cheap: the repository already has the
  newtype-with-handwritten-`Debug` idiom in `CommandEnv`
  (`src/runner/process/command_env.rs:73`). If a reviewer still prefers the
  enum, that is an upstream change to technical design §5.2 and must be settled
  before EP-M1, not during it. Date/Author: 2026-09-08, planning; rationale
  repaired after design review.

- **D11 — Extending ADR-008's jurisdiction from environment variables to the
  clock is itself a decision, and is recorded as one.** Decided: classify the
  clock seam under ADR-008 via a dated addendum, rather than writing a separate
  ADR. Rationale: ADR-008's stated problem is narrower than its title suggests.
  Its context section is written entirely about `clippy.toml`'s ban on
  `std::env::var` and friends, and no lint currently forbids
  `OffsetDateTime::now_utc()`. The taxonomy therefore does not automatically
  claim jurisdiction over clocks, and this plan must not silently assume it
  does. An addendum is nonetheless the right vehicle: the ADR's decision text
  is a rubric about *seam shapes chosen by call-site count and `Send + Sync`
  need*, which transfers to any ambient input, and the file already accretes
  dated addenda for exactly this kind of extension — see its 2026-08-30
  "Manifest glob base seam" entry, which likewise covers a
  non-environment-variable boundary. Roadmap 7.1.1 bullet 4 also directs the
  classification to ADR-008 by name. A separate ADR would fragment one rubric
  across two documents. The addendum must therefore say explicitly that the
  taxonomy is being applied to a non-environment-variable ambient input, so a
  later reader is not misled about the original scope. Whether `clippy.toml`
  should also disallow `OffsetDateTime::now_utc` outside the seam is
  deliberately **out of scope** here: such a lint would fail the build at sites
  this plan does not touch, including `src/stdlib/time/tests.rs` and
  `tests/bdd/steps/stdlib/assertions.rs`, both of which legitimately read the
  real clock as a test oracle. Record it as a follow-up rather than doing it.
  Date/Author: 2026-09-08, planning.
- **D12 — `fixed_clock()` ships beside `system_clock()`.**
  Decided: add a second public adapter constructing a constant provider.
  Rationale: `Arc::new(move || instant)` would otherwise be handwritten at five
  sites in this plan alone — the unit fixture, the sequenced fixture, the
  integration support helper, the BDD step, and the doctest. Every comparable
  library pairs a fixed constructor with the system one (D10). It costs three
  lines and removes the most-repeated incantation in the change. Date/Author:
  2026-09-08, planning; added after design review.

- **D13 — `time::OffsetDateTime` enters netsuke's public API, and `time` is
  re-exported so callers need not depend on it directly.** Decided: export
  `ClockInstant` as an alias for `time::OffsetDateTime` alongside
  `ClockProvider`. Rationale: this is the first `time` type on netsuke's public
  surface — `grep -rn "pub .*OffsetDateTime" src/` currently returns nothing —
  and it is a deliberate departure from the `EnvReader` precedent D1 otherwise
  follows. `EnvReader` owns `EnvReadError` precisely so it does not expose the
  adapter's `VarError`, a property ADR-008 calls out. The clock cannot do the
  same: its whole purpose is to yield a calendar instant, and inventing a
  netsuke-owned timestamp type to wrap one would be ceremony with no
  beneficiary. The consequence must be stated rather than inherited silently: a
  `time` 0.4 bump becomes a breaking change to netsuke's library API. That is
  acceptable — the crate is pre-1.0, ships as a binary, and its only library
  consumers are its own tests — but it belongs in the ADR-008 addendum so a
  later reader is not surprised. Date/Author: 2026-09-08, planning; added after
  design review.

## Outcomes & retrospective

Complete. The stdlib `now()` helper reads its instant through an injected
`ClockProvider` owned by `StdlibConfig`, every evaluation reads the provider
again, and the no-provider path is behaviourally unchanged:
`WallClock::default` installs `system_clock()`, the ambient adapter.
Manifest-query registration still refuses `now`. The change lands in
`src/stdlib/time/clock.rs`, `src/stdlib/time/mod.rs`,
`src/stdlib/config/mod.rs`, the registration path in `src/stdlib/register.rs`,
one integration module, and one BDD feature; unit tests stayed at 49 across the
test-module split.

Reconciliation against the conformance basis:

- Technical design §5.2 now records the implemented state, including the
  `WallClock` container (D2) the design did not name; §15's synchronization
  requirement is satisfied in the same change set as the governing ADR.
- ADR-008's addendum records the classification, states that the taxonomy is
  applied to a non-environment-variable ambient input (D11), and its
  `Implementation references` list names `src/stdlib/time/clock.rs` along with
  the config and registration call sites.
- RFC 0006 §16 question 7 is answered and its §3.3 gap entry records the gap
  closed, both pointing at the addendum. This plan was the "future slice" the
  question anticipated, so leaving it open would have been a stale upstream
  artefact.
- `docs/developers-guide.md`'s "Environment and template ports" section and
  ADR-008 were edited in one commit, as ADR-008's `Consequences` requires.
- Every roadmap 7.1.1 bullet maps to a named artefact: registration through
  `StdlibConfig` to `config/mod.rs` and `register.rs`; ambient preservation to
  `WallClock::default` plus the integration and BDD ambient coverage; the
  injected-value, repeated-call, and ambient-fallback tests to `clock_tests.rs`,
  `tests/std_filter_tests/time_functions.rs`, and `stdlib_time.feature`; the
  classification to the ADR-008 addendum.
- No discovery falsified an assumption in the technical design or RFC 0007, so
  neither needed a correction beyond §5.2's proposal-to-implemented rewrite.

Upstream changes and deviations, all recorded above or in
`Surprises & discoveries`:

- The ADR-008 addendum is dated 2026-09-11 rather than the plan's
  `2026-09-08`, matching the file's convention of dating each entry when it is
  written.
- RFC 0006 §3.3's first gap and §14.1's slice-0 deliverable were corrected:
  nine of the sixteen names recorded as absent from manifest-query registration
  have since been stubbed, leaving the seven file tests. This is a
  documentation correction only, with no runtime change; it was made in place
  because the stale claim sits in the same bullet as the edit this plan
  required.
- `make lint` on this branch requires `PATH="$HOME/go/bin:$PATH"`, because the
  base commit predates the Makefile's `GO_BIN` curation. The workaround
  disappears once the branch is rebased past that commit.
- RFC 0007's "what is missing" list, and one sentence in its architecture
  section, also recorded the clock seam as absent; both now record it as
  supplied by 7.1.1. Same rationale as the RFC 0006 correction: leaving a
  known-false claim in the governing RFC would outlive the change.

Follow-on work, not part of this plan:

- Roadmap item 7.1.2 wires the runner's `given.clock.now` input to this seam;
  the BDD suite uses an explicit clock fixture step until then.
- RFC 0006 slice 0's remaining stub work is the seven file tests (`dir`,
  `file`, `symlink`, `pipe`, `block_device`, `char_device`, `device`).

Retrospective:

- The seam's contract turned on a near-miss API distinction
  (`to_offset` versus `replace_offset`) that only measurement settled. The
  mutation exercise earned its keep by surfacing it; a plan that had trusted
  the first mutation's pass/fail reading would have recorded the wrong lesson.
- Denied lints are not uniformly visible. `clippy::shadow_reuse` failed the
  gate but not `cargo check --all-targets` with `-D warnings`, and the
  module-size cap is per file rather than recursive. Both are recorded so later
  milestones budget for the real gate, not a proxy.
- Test-module splits that preserve the test count are a cheap way to satisfy
  the per-file cap without weakening coverage.

## Artefacts and notes

To be populated during implementation. Required entries:

1. The Red-stage compiler error from Stage B (first three errors, verbatim).

   ```plaintext
   error[E0425]: cannot find type `ClockProvider` in this scope
     --> src/stdlib/time/tests.rs:54:19
      |
   54 |     let provider: ClockProvider = Arc::new(move || {
      |                   ^^^^^^^^^^^^^ not found in this scope

   error[E0433]: cannot find type `WallClock` in this scope
     --> src/stdlib/time/tests.rs:30:34
      |
   30 |     register_functions(&mut env, WallClock::default());
      |                                  ^^^^^^^^^ use of undeclared type `WallClock`

   error[E0061]: this function takes 1 argument but 2 arguments were supplied
     --> src/stdlib/time/tests.rs:30:5
      |
   30 |     register_functions(&mut env, WallClock::default());
      |     ^^^^^^^^^^^^^^^^^^           -------------------- unexpected argument #2
      |
   note: function defined here
     --> src/stdlib/time/mod.rs:43:15
      |
   43 | pub(crate) fn register_functions(env: &mut Environment<'_>) {
      |               ^^^^^^^^^^^^^^^^^^
   help: remove the extra argument
   ```

   `make test-nextest` then reported the following, with exit code 101:

   ```plaintext
   error: could not compile `netsuke-build` (lib test) due to 8 previous errors
   ```

   All eight name the missing seam items or the changed arity; no unrelated
   failure appeared. Full log:
   `/tmp/test-netsuke-7-1-1-clock-provider-seam-m0.out`.

2. The `make test` summary line at EP-M1 and EP-M2. At EP-M1 the seam's own
   suites were run directly, since EP-M1's commit is a plateau rather than the
   gate milestone:

   ```plaintext
   # cargo nextest run -E 'test(stdlib::time)'
   Summary [0.040s] 49 tests run: 49 passed, 2688 skipped

   # cargo nextest run -E 'binary(std_filter_tests)'
   Summary [5.041s] 72 tests run: 72 passed, 0 skipped
   ```

   The full-suite `make test` summary is recorded at EP-M3.

3. The full-suite `make test` summary at EP-M3, verbatim:

   ```plaintext
   Summary [  59.616s] 2830 tests run: 2830 passed, 3 skipped
   test result: ok. 86 passed; 0 failed; 25 ignored; 0 measured; 0 filtered out; finished in 0.02s
   test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
   test result: ok. 32 passed; 0 failed; 6 ignored; 0 measured; 0 filtered out; finished in 0.01s
   ```

   Log: `/tmp/test-netsuke-7-1-1-clock-provider-seam.out`.

4. Public API coverage at EP-M3, since the seam adds exported items. Verbatim:

   ```plaintext
   test_support lib                             403/403   100.00%
   netsuke-build lib                           3541/3576   99.02%
   netsuke-build bin (netsuke)                  115/115   100.00%
   aggregate                                   4059/4094   99.15%
   ok: doc-comment coverage 99.15% meets the 80.00% threshold.
   ```

   Log: `/tmp/doc-coverage-netsuke-7-1-1-clock-provider-seam.out`.

5. First CodeRabbit pass, run at EP-M3 over `e682ac98` and `ccf63eb0`:
   `coderabbit review --agent` completed normally (no rate limiting) with 16
   changed files reviewed and zero findings. Log:
   `/tmp/coderabbit-netsuke-7-1-1-clock-provider-seam.out`.

   The other gates in the same run were green: `make check-fmt`,
   `make typecheck`, `make lint` (with `PATH="$HOME/go/bin:$PATH"`),
   `make markdownlint` (133 files, 0 errors) and `make nixie`.

6. The six mutation outcomes from `Validation and acceptance`. Each mutation was
   applied to the working tree, the named test run, and the file reverted with
   `git checkout -- <path>`; the tree at HEAD is unmutated. Per-mutation revert
   was used in place of one scratched-up commit because the mutations are not
   mutually compatible — M1 and M2 both rewrite `register_functions`, and M3
   and M6 both edit the read path — so a single commit containing "all six" is
   not constructible. Log:
   `/tmp/mutations-netsuke-7-1-1-clock-provider-seam.log`.

   | #   | Mutation                                           | Named test                                | Outcome                                                                |
   | --- | -------------------------------------------------- | ----------------------------------------- | ---------------------------------------------------------------------- |
   | 1   | `clock.read()` → `time::OffsetDateTime::now_utc()` | `now_uses_injected_clock`                 | rejected: all 4 cases fail                                             |
   | 2   | Instant baked in at registration                   | `now_reads_the_provider_on_every_call`    | rejected: 1/1 fails                                                    |
   | 3   | Offset applied as an arithmetic shift              | `now_offset_preserves_the_instant`        | rejected: minimal failing input `sign="+", hour=0, minute=0, second=1` |
   | 4   | Registration passes `WallClock::default()`         | unit / integration / BDD                  | rejected by the integration layer only                                 |
   | 5   | Refusing `now` stub deleted                        | `manifest_query_registration_refuses_now` | rejected: both cases fail                                              |
   | 6   | UTC normalization removed from `WallClock::read`   | `now_uses_injected_clock`                 | rejected: `case_4_non_utc` fails, the 3 UTC cases pass                 |

   Mutation 4 is the load-bearing one, and it behaved exactly as predicted:
   `cargo nextest run -E 'test(stdlib::time)'` reported
   `Summary [0.084s] 49 tests run: 49 passed, 2697 skipped` — every unit test
   still green — while `binary(std_filter_tests)` failed and both
   `stdlib_time_a_fixed_clock*` scenarios failed. Unit coverage alone cannot
   detect an incorrectly wired registration.

   Mutation 5 also confirms the two OBL-5 cases are not redundant: with the
   stub gone, `query_functions_do_not_define_now` still passes
   (`Summary [0.010s] 2 tests run: 2 passed`) because the permissive half was
   never registering `now` in the first place. Only the refusal case catches
   the deletion, and only the absence case catches a leak of `now` into the
   permissive half.

   Applying mutation 3 left a genuine `proptest` shrink
   (`sign = "+", hour = 0, minute = 0, second = 1`) that was committed to
   `proptest-regressions/stdlib/time/clock_tests.txt`, following the precedent
   of `proptest-regressions/stdlib/path/home_tests.txt`, which records a
   mutation-derived seed the same way. It passes against the unmutated code.

7. The EP-M4 documentation edit, one commit over four files, plus a follow-up
   commit for RFC 0007 and the spelling gate. ADR-008 gains the dated addendum
   `### 2026-09-11: Stdlib clock seam` and an `Implementation references` entry
   for `src/stdlib/time/clock.rs`; `docs/developers-guide.md` gains the clock
   paragraph in "Environment and template ports" (same commit, because ADR-008's
   `Consequences` requires the two to stay consistent); technical design §5.2
   moves from proposal to implemented state and names `WallClock`; RFC 0006
   §3.3 records the `now` gap as closed and §16 question 7 as resolved, both
   pointing at the addendum. `make check-fmt` was red on the first pass,
   `make fmt` was run and touched only those four files, and the re-run was
   green.

   The follow-up commit corrects RFC 0007's gap list and its "two seams are
   added" sentence, and applies the spelling gate's two findings:
   `hand-written` and `mis-wired` are both rejected by `typos`, which splits on
   the hyphen and then reads `mis` as a misspelling. The `mis-wired` wording
   also lived in the `WallClock` doc comment and in this plan's sketch of it,
   so all three were changed together. That commit touches
   `src/stdlib/time/clock.rs`, so the code gates were re-run over it.

8. The final gate transcript tails for `check-fmt`, `typecheck`, `lint`, and
   `test`.

Keep each excerpt short — the summary line and the failing assertion, not the
whole log. The full logs live at the `/tmp` paths named in `Concrete steps`.
