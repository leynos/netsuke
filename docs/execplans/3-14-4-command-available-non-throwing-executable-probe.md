# 3.14.4. Promote `command_available` to a first-class non-throwing executable probe

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

## Purpose / big picture

Roadmap item [`3.14.4`](../roadmap.md) elevates
`command_available(name, **kwargs)` from collateral output of
[`3.14.2`](3-14-2-top-level-flow-control-expansion.md) into a documented,
hardened MiniJinja predicate that manifest authors can rely on for conditional
planning. The 3.14.2 work registered the function and added the happy-path
coverage needed to keep complementary action branches selecting exactly one
branch. 3.14.4 closes the resulting contract debt: it commits to a written
predicate contract (kwarg matrix, return type, error class), replaces the
internal string-matched absence detector with a typed `ResolveError`
discriminant, fills the kwarg-and-platform coverage matrix, and adds the
property-test invariant tying `command_available` to the `which` filter.

After this change a manifest author can read a single section of
[`docs/netsuke-design.md`](../netsuke-design.md) and the user guide and know
exactly which kwargs are accepted, what absence and misuse each produce, and
how the predicate interacts with workspace fallback and Windows `PATHEXT`. A
contributor inspecting the stdlib can pattern-match a `ResolveError::NotFound`
variant rather than grepping for an error code, and a future feature such as
3.14.8 (`env(name, default=...)`) can adopt the same typed-failure pattern
without re-discovering it.

Observable success is twofold. First, the new parametrized `rstest` cases,
behavioural scenarios, and `proptest` invariant all pass on Linux, macOS, and
Windows runners. Second, `docs/users-guide.md`, `docs/netsuke-design.md §4.5`,
and `docs/developers-guide.md` each contain a dedicated `command_available`
description that names every accepted kwarg, its default, and the two failure
modes (`netsuke::jinja::which::args` for misuse, `false` for absence). The
branch closes with roadmap item `3.14.4` marked done.

## Constraints

These invariants must hold throughout the work. Violating any constraint
triggers escalation, not a workaround.

- Preserve the three acceptance bullets from `docs/roadmap.md §3.14.4`. The
  predicate must continue to reuse the `which` resolver and cache, return
  `false` rather than raise `netsuke::jinja::which::not_found` for absent
  commands, and surface `netsuke::jinja::which::args` for invalid options.
- Keep manifest-time evaluation strictly pre-AST. `command_available` may only
  be evaluated by the existing `expand_foreach` pipeline in
  `src/manifest/expand.rs`; no AST, IR, Ninja, or runtime layer may inspect the
  predicate or its options. ADR-003
  (`docs/adr-003-actions-foreach-when-scope.md`) already pins this; do not
  weaken it.
- Hexagonal boundary: the public registration site
  (`src/stdlib/which/mod.rs::register`) is the only place that translates a
  resolver result into a `minijinja::Value` or `minijinja::Error`. The typed
  `ResolveError` must not leak into manifest, AST, IR, or CLI modules.
- Reuse the existing `WhichResolver`, `WhichOptions`, `EnvSnapshot`, and
  workspace skip list. Do not introduce a parallel resolver, cache, or `PATH`
  walker.
- Use existing `ortho_config` integration if any new configuration knob is
  required (`docs/ortho-config-users-guide.md`). Do not add a parallel
  configuration loader or untranslated help path. No new public CLI flag is
  expected.
- No new external crates without explicit approval. The Rust `which` crate is
  already an indirect dependency through the existing resolver; do not add the
  crate directly.
- No `unsafe` code.
- Every Rust source file stays under the 400-line cap from `AGENTS.md`. Refactor
  `src/stdlib/which/error.rs` and `src/stdlib/which/mod.rs` rather than add
  bulk to them.
- Localization keys come from `locales/en-US/messages.ftl` and
  `locales/es-ES/messages.ftl`. Reuse the existing `stdlib.which.*` keys
  wherever possible; add new keys only when a diagnostic genuinely changes
  shape, and update both locales in lockstep.
- Use en-GB Oxford spelling in prose, except for external API names,
  established computing terms (`serialization`, `deserialization`), and
  verbatim identifiers.
- Unit tests use `rstest` with parametrized cases for shared structure.
  Behavioural tests use `rstest-bdd` and the existing scenario-scoped env guard
  (`tests/bdd/helpers/env_mutation.rs`). Property tests use `proptest`. No
  global env mutation outside the existing helpers.
- Run `make check-fmt`, `make lint`, and `make test` (in that order, no
  parallelism) before each commit. Run `coderabbit review --agent` after each
  major milestone and clear concerns before the next milestone begins.
- Do not mark roadmap `3.14.4` done until the implementation, documentation,
  CodeRabbit pass, and full quality gates all succeed.

## Tolerances (exception triggers)

These thresholds bound autonomous work. Stop and escalate on breach; do not
work around them.

- Scope: stop if implementation requires touching more than 14 files or adding
  more than ~700 net lines of code (excluding documentation and tests). The
  typed-error refactor is the largest expected piece; a larger surface area
  signals scope creep.
- Public API: stop if any public Rust type, CLI flag, configuration key,
  manifest field, or localization key visible to downstream crates must change.
  The `WhichConfig`, `WhichResolver` (`pub(crate)`), and `StdlibConfig` shapes
  may evolve internally but their crate-public methods must not.
- Roadmap overlap: stop if completing 3.14.4 inevitably also closes any of
  3.14.5, 3.14.8, or 3.14.11. Those items remain separate deliverables and must
  keep their own coverage and documentation.
- New dependencies: stop and request approval before adding any crate, Cargo
  feature, external tool, Kani harness, or Verus setup.
- Test flakes: stop after two focused fix attempts on any PATH-dependent test.
  Redesign around `path_override` injection rather than chase host-specific
  symptoms.
- Validation: stop after two focused fix attempts to make `make check-fmt`,
  `make lint`, or `make test` pass. Record the failing command and the
  `/tmp/...` log file in the `Decision Log`.
- Review: stop if `coderabbit review --agent` reports unresolved concerns
  after a major milestone. Address them before proceeding; if a concern
  conflicts with this plan, escalate.
- Verification rigour: stop if the proposed `proptest` invariant requires a
  shrinker-defeating custom strategy. Use the simpler "consistency with
  `which`" property described below rather than inventing a new contract.
- Diagnostic drift: stop if any existing user-facing diagnostic message
  changes by even a single character relative to the Milestone 1 baseline
  snapshots. Preserving message text byte-for-byte is non-negotiable; if a
  change is unavoidable, escalate before accepting it.

## Risks

- Risk: replacing the string-matched `is_not_found_error` with a typed
  discriminant touches every `WhichResolver::resolve` caller and could ripple
  into unrelated modules. Severity: high. Likelihood: medium. Mitigation:
  introduce the typed variant *behind* the existing `minijinja::Error` so the
  external shape stays identical; convert callers in a single milestone with
  red-then-green tests.

- Risk: `command_available` already exists and many edge cases pass today by
  accident, so new coverage may discover latent bugs (especially around
  workspace fallback, `cwd_mode="always"` on POSIX, and `PATHEXT` on Windows).
  Severity: medium. Likelihood: medium. Mitigation: write each parametrized
  case in red first and confirm the failure mode before fixing it, so a quiet
  regression cannot hide behind a green suite.

- Risk: Windows-only paths (`PATHEXT`, drive letters, backslash normalization)
  are difficult to exercise on the Linux CI runner the agent uses for local
  validation. Severity: medium. Likelihood: high. Mitigation: gate the
  Windows-specific cases with `#[cfg(windows)]`, write them so they compile on
  every platform, and rely on the project's Windows CI matrix to execute them.
  Use `direct_candidates` and `is_direct_path` from
  `src/stdlib/which/lookup/mod.rs` as the contract under test.

- Risk: a property test over `(name, options, PATH)` triples could over-shrink
  into pathological inputs that the resolver was never designed to handle (very
  long paths, non-UTF-8 segments). Severity: medium. Likelihood: medium.
  Mitigation: scope the strategy to printable ASCII names, bounded PATH lists,
  and the four `WhichOptions` fields with their parsed value set; document the
  scope in the test module.

- Risk: the message-construction path moves through a `From` impl rather than
  inline `Error::new` calls, which makes a one-character drift (a stray full
  stop, a re-ordered variable, a locale capitalization change) easy to ship
  undetected. Severity: high. Likelihood: medium. Mitigation: capture `insta`
  snapshots of every existing `which` diagnostic in Milestone 1 *before* the
  refactor and treat any drift as a Milestone 2 failure (see the
  `Diagnostic drift` tolerance).

- Risk: changes to `src/stdlib/which/error.rs` could change the human-readable
  message text and silently break user scripts that grep for it. Severity:
  medium. Likelihood: low. Mitigation: preserve the existing diagnostic-code
  prefixes (`netsuke::jinja::which::not_found`, `netsuke::jinja::which::args`)
  verbatim. Snapshot the rendered message via the existing `insta`
  infrastructure if a change is unavoidable.

- Risk: BDD env mutations may race with other parallel tests if the existing
  guard is dropped. Severity: high. Likelihood: low. Mitigation: route every
  PATH change through `TestWorld::track_env_var` and the existing scenario
  guard in `tests/bdd/helpers/env_mutation.rs`; never call `std::env::set_var`
  directly inside test code.

## Relevant context

The manifest load pipeline lives in `src/manifest/mod.rs`. `from_str_named`
parses YAML into a `ManifestValue`, registers the stdlib (`env`, `glob`, and the
`command_available`/`which` family) on a MiniJinja `Environment`, then calls
`expand_foreach` before deserializing the typed `NetsukeManifest`. That is the
architectural boundary 3.14.4 must preserve.

`src/manifest/expand.rs` exposes `expand_foreach` which evaluates `when`
expressions through MiniJinja. The function is already capable of running
`command_available(...)` and `not command_available(...)` thanks to the 3.14.2
work; this plan does not change `expand.rs` unless red tests prove a behaviour
gap.

The stdlib resolver lives in `src/stdlib/which`:

- `mod.rs` registers the `which` filter, the `which` function, and the
  `command_available` function. `command_available_with` (lines 104–121) is the
  predicate's current dispatch wrapper; it calls
  `WhichResolver::resolve(name, &options)` and inspects the returned
  `minijinja::Error` by string-matching the `netsuke::jinja::which::not_found`
  code via `error::is_not_found_error`. This is the leak the typed refactor
  closes.
- `error.rs` defines `NOT_FOUND_CODE`, `is_not_found_error`, `not_found_error`,
  `direct_not_found`, and `args_error`. All four helpers wrap their text in a
  `minijinja::Error` whose `ErrorKind` is `InvalidOperation`. The refactor
  introduces a private `ResolveError` enum (`NotFound { command, kind }`,
  `Args { detail }`, `Io { source }`, optionally `EmptyPathAndDirless`) and
  reshapes the existing helpers to construct from it at the registration
  boundary.
- `cache.rs` defines `WhichResolver`, which currently returns
  `Result<Vec<Utf8PathBuf>, minijinja::Error>`. After the refactor it returns
  `Result<Vec<Utf8PathBuf>, ResolveError>`; the registration site converts to
  `minijinja::Error` once.
- `lookup/mod.rs` is the search engine; `handle_miss` is where absence is
  recognized. The refactor changes the return type but not the algorithm.
- `options.rs` defines `WhichOptions` and `CwdMode`. No change required.
- `lookup/workspace/` implements the workspace fallback walker. No change
  required.

The behavioural surface lives in `tests/`:

- `tests/stdlib_which_tests.rs` carries integration coverage for both the
  filter and the predicate (4 `command_available` cases today).
- `tests/data/actions_command_available_absent.yml` and
  `tests/data/actions_command_available_invalid.yml` exercise complementary
  action branches through manifest parsing.
- `tests/features/manifest.feature` (lines 132–144) and
  `tests/features/manifest_subcommand.feature` (lines 25–30) drive end-to-end
  manifest behaviour through `rstest-bdd`.
- `tests/bdd/steps/conditional_manifest.rs` owns the temporary workspace
  builder and the scenario-scoped PATH guard via `TestWorld::track_env_var`.
  The guard cooperates with `tests/bdd/helpers/env_mutation.rs::EnvLock` to
  serialize env changes across parallel scenarios.

The user-facing documentation entry points are `docs/users-guide.md`
(`Executable Discovery (which)` section around line 502),
`docs/netsuke-design.md §4.5 Executable discovery filter (which)` (around line
1203 with the `command_available` paragraphs at 1265–1279), and
`docs/developers-guide.md` "Manifest processing helpers" (around line 710, with
a single bullet at 748–752 mentioning the predicate).

Skills and documents to keep open while working:

- `leta`: use `leta show`, `leta refs`, `leta grep`, and `leta calls` for code
  navigation; never browse files manually when a symbol name exists.
- `rust-router`: route Rust-specific questions to the smallest follow-on
  skill. Likely follow-ons are `rust-errors` for the typed `ResolveError` shape,
  `rust-types-and-apis` for the discriminant design, and
  `hexagonal-architecture` for the resolver/registration split.
- `hexagonal-architecture`: keep the typed error enum on the resolver port
  and translate it to `minijinja::Error` only at the registration adapter.
- `proptest`: design the consistency strategy with bounded inputs and pinned
  regression files.
- `execplans`: keep this plan current.
- `commit-message` and `pr-creation`: prepare commits and the draft PR.
- `en-gb-oxendict`: enforce British Oxford spelling in prose.
- `code-review`: run after each milestone as a self-check before
  `coderabbit review --agent`.
- `docs/roadmap.md`
- `docs/netsuke-design.md`
- `docs/users-guide.md`
- `docs/developers-guide.md`
- `docs/ortho-config-users-guide.md`
- `docs/rstest-bdd-users-guide.md`
- `docs/rust-testing-with-rstest-fixtures.md`
- `docs/reliable-testing-in-rust-via-dependency-injection.md`
- `docs/documentation-style-guide.md` for the new ADR template
- `docs/execplans/3-14-2-top-level-flow-control-expansion.md` for the 3.14.2
  groundwork

## Prior art

The agent team's Firecrawl pass surveyed 14 ecosystems. Three precedents
dominate; the rest support the same shape from different angles.

- **Just** ships sibling `which(name)` (returns empty string when absent) and
  `require(name)` (errors out) sharing one resolver. This is the closest
  template for Netsuke's split between the throwing `which` filter and the
  non-throwing `command_available` function, and validates keeping the kwarg
  surface symmetric across the two. Source:
  [`just.systems/man/en/built-in-functions.html`](https://just.systems/man/en/built-in-functions.html).
- **Meson** `find_program('name', required: false)` returns an
  `external_program` whose `.found()` is `false`. The `required` flag flips a
  single resolver between throw and silent absence. This matches the Netsuke
  decision to expose two named entry points (filter vs predicate) rather than a
  `required=false` kwarg on `which`. Source:
  [`mesonbuild.com`](https://mesonbuild.com/Reference-manual.html).
- **Bazel** `repository_ctx.which(program)` returns `None` for absence inside
  the manifest-evaluation phase. The "absent is a value" rule applies to a
  declarative repository context that is morally identical to Netsuke's
  manifest expansion phase. Source:
  [`bazel.build/rules/lib/builtins/repository_ctx`](https://bazel.build/rules/lib/builtins/repository_ctx).

Reinforcing precedents: POSIX `command -v` distinguishes `127` (not found) from
`>0` (misuse), the Rust `which` crate exposes `Error::CannotFindBinaryPath` as
a typed discriminant, Python `shutil.which` returns `None` rather than raising,
the npm `which` package toggles via `{ nothrow: true }`, GitHub Actions
expressions return empty strings instead of aborting, CMake `find_program` uses
a `<VAR>-NOTFOUND` sentinel, cargo-make's `condition_script` skips on non-zero
exit, and Taskfile.dev splits `preconditions` (hard) from `status`/`if` (soft).

The research also surfaced one open question: should `command_available` treat
`CannotGetCurrentDirAndPathListEmpty` (or its Netsuke analogue, a failed
`EnvSnapshot::capture` with both `PATH` and a usable cwd missing) as `false` or
as a hard error? Prior art (CMake, Bazel, Meson) treats "no place to search" as
absence rather than misuse; the Decision Log must pin this explicitly before
Milestone 2 ships.

## Implementation plan

Work proceeds in four milestones. Each milestone ends with `make check-fmt`,
`make lint`, `make test`, and `coderabbit review --agent`, with logs teed to
`/tmp` per the `AGENTS.md` command policy. Commit at the end of each milestone,
and earlier when the change is naturally atomic.

### Milestone 1: Lock the predicate contract with red tests

Confirm the active branch is
`3-14-4-command-available-non-throwing-executable-probe` and the worktree is
clean. Run the baseline suites once, teeing output:

```sh
set -o pipefail
PROJECT=$(basename "$(git rev-parse --show-toplevel)")
BRANCH=$(git branch --show-current)
cargo test --all-targets --all-features --test stdlib_which_tests \
  2>&1 | tee "/tmp/baseline-stdlib-which-${PROJECT}-${BRANCH}.out"
cargo test --all-targets --all-features manifest::expand \
  2>&1 | tee "/tmp/baseline-expand-${PROJECT}-${BRANCH}.out"
cargo test --all-targets --all-features --test bdd_tests command_available \
  2>&1 | tee "/tmp/baseline-bdd-command-available-${PROJECT}-${BRANCH}.out"
```

Add the following failing tests before any production-code change. Each item is
a single parametrized rstest unless otherwise noted; group them by file to keep
diffs reviewable.

Before adding new cases, capture an `insta` baseline of every existing
user-visible `which` diagnostic. Add a new module
`tests/std_filter_tests/which_diagnostic_snapshot_tests.rs` that renders each
error path (`not_found`, `direct_not_found`, `args_error` with an empty command,
`args_error` with an invalid `cwd_mode`, `args_error` with an unknown keyword)
and asserts the rendered message via `insta::assert_snapshot!`. Check the
generated `.snap` files in. Milestone 2 treats any change to these snapshots as
a Milestone 2 failure rather than a snapshot update.

In `tests/stdlib_which_tests.rs`, add cases that pin the kwarg-and-platform
matrix:

- direct-path inputs that resolve `false`: absolute path that does not exist
  (POSIX and Windows variants under `#[cfg]`), and `./missing` under the
  workspace.
- direct-path inputs that resolve `true`: absolute path to a fixture binary
  the test writes; verify the predicate returns `true` and that
  `canonical=true` over a symlinked binary also returns `true`.
- empty-PATH workspace fallback: write a fixture binary into the workspace
  root, set `path_override` to an empty `OsString`, leave `cwd_mode` defaulted,
  and assert the predicate returns `true`. Pair this with a negative case where
  the fixture is absent.
- `fresh=true` cache bypass: render the predicate twice across an
  environment mutation, then assert the second render reflects the change (use
  the existing `path_override` to swap `PATH`, not global `set_var`).
- `cwd_mode` triad: parametrize over `"auto"`, `"always"`, `"never"`. Pair
  each mode with a fixture-present and fixture-absent case so eight cases cover
  the matrix.
- argument-validation regressions: `command_available("")`,
  `command_available("   ")`, `command_available("tool", cwd_mode="invalid")`,
  and `command_available("tool", unexpected=true)` must each raise
  `netsuke::jinja::which::args`. Use a single parametrized error case set.
- `all=true` semantics: assert that `command_available("tool", all=true)`
  returns the same boolean as `command_available("tool", all=false)` for both
  present and absent fixtures, pinning the contract that `all` does not affect
  the bool return.

In `src/manifest/expand_test_cases/condition_cases.rs`, add a parametrized case
for target-level `when: command_available(...)` using the same local
deterministic MiniJinja function pattern already used at lines 276–314 (so the
test does not depend on the host PATH). Cover the truthy and falsy branches so
the manifest filter behaviour matches actions.

In `src/stdlib/which/lookup/tests.rs` (or, if the file grows past the 400-line
cap, a new sibling module `lookup/command_available_tests.rs`), add a property
test using `proptest`:

```rust
proptest! {
    #[test]
    fn command_available_agrees_with_which(
        name in "[a-z][a-z0-9_-]{0,15}",
        options in arb_which_options(),
        path_dirs in proptest::collection::vec(arb_path_dir(), 0..4),
    ) {
        let resolver = build_resolver(&path_dirs);
        let which_result = resolver.resolve(&name, &options).ok();
        let predicate = run_command_available(&resolver, &name, &options).unwrap();
        prop_assert_eq!(
            predicate,
            which_result.is_some_and(|matches| !matches.is_empty())
        );
    }
}
```

Document the strategy bounds in the test module's `//!` comment and check any
generated regression file into `src/stdlib/which/proptest-regressions/`. The
directory may be absent on first run; CI must tolerate it and only create
entries on first failure. Do not pre-create the directory with a placeholder
file.

In `tests/features/manifest.feature` and the supporting steps under
`tests/bdd/steps/conditional_manifest.rs`, add a scenario "Target-level command
availability selects the preferred target" that mirrors the existing
action-level scenario at `tests/features/manifest_subcommand.feature:25-30`.
Add a new manifest fixture under `tests/data/` (for example
`targets_command_available.yml`) reusing the existing PATH guard.

Expected state after Milestone 1: every new test fails (or, where the current
implementation accidentally passes, fails one of the documented edge cases,
typically the typed-error pattern match or the workspace-fallback case).

Run `coderabbit review --agent` on the red diff. Resolve concerns before
proceeding.

### Milestone 2: Introduce the typed `ResolveError`

Create a new module `src/stdlib/which/resolve_error.rs` that owns the typed
result type. Keep the existing message-formatting helpers in
`src/stdlib/which/error.rs`; the two files split responsibilities — `error.rs`
remains the *adapter* that renders human-readable text and localization
arguments, and `resolve_error.rs` is the *port* that names absence, misuse, and
I/O conditions structurally. This split protects the 400-line cap on `error.rs`
and keeps the new typed surface easy to find.

In `src/stdlib/which/resolve_error.rs`, define:

```rust
pub(super) enum ResolveError {
    NotFound { command: String, dirs: Vec<Utf8PathBuf>, cwd_mode: CwdMode },
    DirectNotFound { command: String, path: Utf8PathBuf },
    Args { detail: String },
    Canonicalise { path: Utf8PathBuf, source: std::io::Error },
    CanonicaliseNonUtf8,
    WorkspaceNonUtf8 { command: String, path: String },
}
```

The variants exactly correspond to the existing `Error` constructions in
`error.rs`. Move the existing constructors (`not_found_error`,
`direct_not_found`, `args_error`, the `canonicalise` error site, and the
workspace non-UTF-8 site in `lookup/workspace/mod.rs`) so they return
`ResolveError` rather than `minijinja::Error`. Delete `is_not_found_error`
entirely; nothing should string-match the error message after this milestone.

Provide `impl From<ResolveError> for minijinja::Error` in `resolve_error.rs`
that preserves the diagnostic codes and message text byte-for-byte against the
Milestone 1 snapshots. The variant-to-code mapping is fixed by the existing
diagnostics and must match this table exactly:

| `ResolveError` variant | diagnostic code prefix              | Fluent message key                                    |
| ---------------------- | ----------------------------------- | ----------------------------------------------------- |
| `NotFound`             | `netsuke::jinja::which::not_found`  | `stdlib.which.not_found` (plus the cwd-mode hint key) |
| `DirectNotFound`       | `netsuke::jinja::which::not_found`  | `stdlib.which.direct_not_found`                       |
| `Args`                 | `netsuke::jinja::which::args`       | `stdlib.which.args_error`                             |
| `Canonicalise`         | (no code prefix; current behaviour) | `stdlib.which.canonicalise_failed`                    |
| `CanonicaliseNonUtf8`  | (no code prefix; current behaviour) | `stdlib.which.canonicalise_non_utf8`                  |
| `WorkspaceNonUtf8`     | (no code prefix; current behaviour) | `stdlib.which.workspace_non_utf8`                     |

If a snapshot diff appears during this milestone, that is the
`Diagnostic drift` tolerance breach — escalate rather than update the snapshot.

Change `WhichResolver::resolve` in `src/stdlib/which/cache.rs` and the internal
helpers in `src/stdlib/which/lookup/mod.rs` to return
`Result<Vec<Utf8PathBuf>, ResolveError>`. Update the existing tests inside
those modules to consume the typed error. Propagation across modules stays
within `src/stdlib/which/*`; nothing outside the directory should observe the
change.

Update `src/stdlib/which/mod.rs::register` so the `which` filter, the `which`
function, and `command_available` each:

1. validate the command string and call
   `WhichResolver::resolve(...) -> Result<Vec<_>, ResolveError>`;
2. for the filter and function, convert `ResolveError` into
   `minijinja::Error` via `From` and return as before;
3. for `command_available`, pattern-match on `ResolveError::NotFound { .. }`
   and `ResolveError::DirectNotFound { .. }` to return `Value::from(false)`;
   propagate every other variant by converting to `minijinja::Error`.

The "no place to search" case (the Netsuke analogue of the Rust `which` crate's
`CannotGetCurrentDirAndPathListEmpty`, surfaced when both `PATH` is empty and
the cwd cannot contribute) is treated as `NotFound` and therefore coerced to
`false`. This follows CMake `-NOTFOUND`, Bazel `repository_ctx.which` `None`,
and Meson `.found() == false`. The decision is pinned in the `Decision Log`;
the ADR records it as a non-reversible contract.

Add an instrumentation breadcrumb at the resolver entry. Wrap
`WhichResolver::resolve` in a
`tracing::trace_span!("stdlib.which.resolve", command = %name, cache_hit = tracing::field::Empty)`
and record the `cache_hit` field once the cache lookup outcome is known. Do
not register a global subscriber; the resolver only emits spans and events.
This is the template later stdlib helpers will adopt; document the convention
in the developers' guide update.

Extract a small testable helper:

```rust
pub(super) fn is_command_available(
    result: Result<Vec<Utf8PathBuf>, ResolveError>,
) -> Result<bool, ResolveError> { ... }
```

so `mod.rs` only translates between MiniJinja types and the helper, and the
helper can be unit-tested without a MiniJinja `Environment`.

Run the focused suites:

```sh
set -o pipefail
PROJECT=$(basename "$(git rev-parse --show-toplevel)")
BRANCH=$(git branch --show-current)
cargo test --all-targets --all-features --test stdlib_which_tests \
  2>&1 | tee "/tmp/stdlib-which-after-typed-error-${PROJECT}-${BRANCH}.out"
cargo test --all-targets --all-features which::lookup \
  2>&1 | tee "/tmp/lookup-after-typed-error-${PROJECT}-${BRANCH}.out"
cargo test --all-targets --all-features which::cache \
  2>&1 | tee "/tmp/cache-after-typed-error-${PROJECT}-${BRANCH}.out"
```

All Milestone 1 red tests should now go green. The diagnostic-code prefixes
must remain identical; if a snapshot diff shows a message-text drift, fix the
source rather than accept the snapshot.

If the typed refactor crosses the 14-file or 700-line tolerance, stop and ask.
If it stays inside, run `coderabbit review --agent` and resolve concerns before
Milestone 3.

### Milestone 3: Documentation and ADR

Update documentation only after the implementation is green:

- `docs/users-guide.md`: rewrite the `command_available` paragraph (currently
  lines 508–541) into a dedicated subsection. State the kwarg matrix in a table
  that names every accepted kwarg, its default, and its effect on the bool
  return; the table must explicitly say "`all` is accepted for kwarg symmetry
  with `which` but does not change the bool return". Contrast the `which`
  filter (raises on absence) with the predicate (returns `false` on absence),
  and add a worked workspace-fallback example showing how `cwd_mode="auto"`
  lets a manifest discover a project-local tool when `PATH` is empty. Reuse
  en-GB Oxford spelling.
- `docs/netsuke-design.md §4.5`: expand the `command_available` paragraphs
  around line 1265 into a subsection "Executable Availability Predicate" that
  documents the typed-error contract (without leaking the enum name — describe
  it as "an internal resolver result"), the kwarg matrix, the workspace
  fallback interaction, and the `EmptyPathAndDirless` decision recorded in
  Milestone 2.
- `docs/developers-guide.md`: replace the bullet at lines 748–752 with a
  short subsection "Executable availability predicate" describing the
  resolver/registration split, the `is_command_available` helper, and the rule
  that absence detection lives in the resolver port and never in manifest, AST,
  IR, or CLI code. End the subsection with a one-line breadcrumb: "The
  `ResolveError` → `minijinja::Error` boundary and the
  `trace_span!("stdlib.<helper>.resolve", ...)` instrumentation are the
  template for future stdlib helpers such as `env` (roadmap 3.14.8) and
  `shell_join`; mirror the `From` impl and the absence-coercion helper."
- New ADR `docs/adr-004-typed-which-resolve-error.md` (or the next free ADR
  number; check `docs/contents.md` and `docs/adr-*` to avoid collision)
  following the template at `docs/documentation-style-guide.md`. Capture: the
  problem (string-matched error detection at the registration boundary), the
  decision (private `ResolveError` enum in the stdlib `which` module), the
  rationale (hexagonal seam, removes accidental coupling to message text,
  mirrors the Rust `which` crate's typed error), the consequences (all resolver
  callsites use pattern matching; the `From` impl preserves the on-the-wire
  diagnostic codes), and the alternatives rejected (keep string match; promote
  `ResolveError` to the public API; introduce a separate resolver for the
  predicate).
- `docs/contents.md`: index the new ADR. If `contents.md` already references
  the affected guides (`users-guide.md`, `netsuke-design.md`,
  `developers-guide.md`) by subsection, update those entries to mention the new
  `command_available` material so the documentation index stays navigable.
- `docs/roadmap.md`: in Milestone 4, mark `3.14.4` done. Do not mark any
  other item.

Validate Markdown and diagrams:

```sh
set -o pipefail
PROJECT=$(basename "$(git rev-parse --show-toplevel)")
BRANCH=$(git branch --show-current)
make fmt 2>&1 | tee "/tmp/fmt-${PROJECT}-${BRANCH}.out"
make markdownlint 2>&1 | tee "/tmp/markdownlint-${PROJECT}-${BRANCH}.out"
make nixie 2>&1 | tee "/tmp/nixie-${PROJECT}-${BRANCH}.out"
```

If `make fmt` rewrites unrelated Markdown, inspect the diff and keep only the
changes relevant to this task. Run `coderabbit review --agent` after the
documentation commit and clear concerns.

### Milestone 4: Full validation, commit, push, and PR

Run the required commit gates sequentially:

```sh
set -o pipefail
PROJECT=$(basename "$(git rev-parse --show-toplevel)")
BRANCH=$(git branch --show-current)
make check-fmt 2>&1 | tee "/tmp/check-fmt-${PROJECT}-${BRANCH}.out"
make lint 2>&1 | tee "/tmp/lint-${PROJECT}-${BRANCH}.out"
make test 2>&1 | tee "/tmp/test-${PROJECT}-${BRANCH}.out"
```

Inspect the diff with `git diff --stat origin/main...HEAD`, then commit with
`git commit -F /tmp/commit-msg-...` using a temporary message file (per the
`commit-message` skill). Commit the typed-error refactor, the new tests, the
documentation update, and the ADR separately so review history stays narrative.

Push the branch with upstream tracking:

```sh
git push -u origin 3-14-4-command-available-non-throwing-executable-probe
```

Open or update the draft pull request with the title:

```plaintext
Promote command_available to non-throwing executable probe (3.14.4)
```

The PR body must identify this execplan
(`docs/execplans/3-14-4-command-available-non-throwing-executable-probe.md`)
and include a `## References` section containing the Lody session link
`https://lody.ai/leynos/sessions/${LODY_SESSION_ID}`.

## Validation plan

Focused validation:

- `cargo test --all-targets --all-features --test stdlib_which_tests`
- `cargo test --all-targets --all-features which::lookup`
- `cargo test --all-targets --all-features which::cache`
- `cargo test --all-targets --all-features manifest::expand`
- `cargo test --all-targets --all-features --test bdd_tests command_available`
- `cargo test --all-targets --all-features --test bdd_tests manifest_time`

Repository gates (sequential, no parallelism):

- `make fmt`
- `make markdownlint`
- `make nixie`
- `make check-fmt`
- `make lint`
- `make test`

Reviewer gate after each major milestone:

- `coderabbit review --agent`

Cross-platform gate before merge:

- The repository's Windows CI matrix must report green for the new
  `#[cfg(windows)]` cases (`PATHEXT`, drive-letter direct paths, backslash
  normalization). The agent runs on Linux and cannot execute these locally; do
  not tick the roadmap entry until the Windows CI job on the open PR passes. If
  the Windows job is missing, treat that as a Tolerance breach and escalate.

All long-running commands tee output to `/tmp/<action>-<project>-<branch>.out`
following the policy in `AGENTS.md`.

Acceptance behaviour a reviewer can verify:

1. `cargo test --all-targets --all-features --test stdlib_which_tests`
   reports the expanded parametrized matrix (direct path, workspace fallback,
   `fresh`, `cwd_mode` triad, `all`, invalid kwargs) all passing.
2. `cargo test --all-targets --all-features --test bdd_tests command_available`
   passes both the existing action-level scenario and the new target-level
   scenario.
3. `rg "is_not_found_error" src/` returns no matches.
4. `rg "NOT_FOUND_CODE" src/stdlib/which/` returns only the `From`
   implementation that constructs the user-facing diagnostic.
5. `make check-fmt`, `make lint`, and `make test` succeed.
6. `docs/adr-004-typed-which-resolve-error.md` exists and is linked from
   `docs/contents.md` and `docs/netsuke-design.md §4.5`.
7. `docs/roadmap.md §3.14.4` is marked `[x]`.

## Idempotence and recovery

Every test in the new matrix uses scenario-scoped fixtures
(`tempfile::tempdir`) and `path_override` instead of global env mutation;
rerunning the suite leaves no host state. The typed `ResolveError` change is
internal to the `which` module, so reverting it is a single
`git revert <commit>` away. If `coderabbit review --agent` flags a concern
after a commit, fix it in a follow on commit; do not amend a pushed commit.

If `make fmt` rewrites broad swathes of unrelated Markdown, inspect each hunk
and discard anything outside `docs/users-guide.md`, `docs/netsuke-design.md`,
`docs/developers-guide.md`, `docs/contents.md`, `docs/roadmap.md`, and the new
ADR before committing.

## Interfaces and dependencies

In `src/stdlib/which/resolve_error.rs` (new module), define:

```rust
pub(super) enum ResolveError {
    NotFound { command: String, dirs: Vec<Utf8PathBuf>, cwd_mode: CwdMode },
    DirectNotFound { command: String, path: Utf8PathBuf },
    Args { detail: String },
    Canonicalise { path: Utf8PathBuf, source: std::io::Error },
    CanonicaliseNonUtf8,
    WorkspaceNonUtf8 { command: String, path: String },
}
```

`ResolveError` remains framework-agnostic data. It does not import MiniJinja or
localization helpers. The MiniJinja adapter in `src/stdlib/which/mod.rs`
implements `From<ResolveError> for minijinja::Error`, preserving the existing
diagnostic text and code prefixes at the registration boundary.

In `src/stdlib/which/cache.rs`, change:

```rust
impl WhichResolver {
    pub(crate) fn resolve(
        &self,
        command: &str,
        options: &WhichOptions,
    ) -> Result<Vec<Utf8PathBuf>, ResolveError> { ... }
}
```

In `src/stdlib/which/mod.rs`, retain the existing public registration function
signature:

```rust
pub(crate) fn register(env: &mut Environment<'_>, config: WhichConfig);
```

and introduce:

```rust
pub(super) fn is_command_available(
    result: Result<Vec<Utf8PathBuf>, ResolveError>,
) -> Result<bool, ResolveError>;
```

No public Rust API outside `src/stdlib/which/` changes.

## Artefacts and notes

Reference artefacts from the 3.14.2 work, all of which can be inspected with
`leta show`:

- `src/stdlib/which/mod.rs::command_available_with`
- `src/stdlib/which/error.rs::is_not_found_error`
- `src/manifest/expand_test_cases/condition_cases.rs::expand_static_action_when_supports_complementary_command_available_branches`
- `tests/bdd/steps/conditional_manifest.rs::command_available_actions_workspace`

The Firecrawl prior-art notes in this plan supersede any general web search; no
new external research is expected during implementation. If a contributor
discovers a precedent that contradicts the chosen semantics, capture it in
`Surprises & Discoveries` and `Decision Log` rather than silently changing the
implementation.

## Progress

- [ ] (DRAFT) Loaded `leta`, `rust-router`, `hexagonal-architecture`, and
  `execplans` skills. Created the `leta` workspace for this checkout.
- [ ] (DRAFT) Surveyed the existing implementation in `src/stdlib/which/*`
  and current test coverage in `tests/stdlib_which_tests.rs`,
  `tests/features/manifest*.feature`, and
  `tests/bdd/steps/conditional_manifest.rs`.
- [ ] (DRAFT) Ran a parallel agent team: an Explore agent for the codebase
  audit and a general-purpose agent driving the `firecrawl` MCP server for
  prior art on non-throwing executable probes.
- [ ] (DRAFT) Drafted this ExecPlan.
- [x] (2026-06-05) Ran the `logisphere-experts` review on the draft and
  integrated all `[BLOCKING]` and `[STRONG]` action items plus selected
  `[NICE]` improvements. The review's open question on
  `CannotGetCurrentDirAndPathListEmpty` is now closed in the Decision Log,
  variant-to-code mapping is documented in the *Implementation plan*, the
  destination module `src/stdlib/which/resolve_error.rs` is named, the Windows
  CI gate and `tracing` span are committed, and the `insta` baseline is
  scheduled before any production-code edit.
- [x] (2026-06-11T22:22:03Z) User explicitly requested implementation of
  this plan. Treat that request as approval to proceed, superseding the draft
  approval wait.
- [x] (2026-06-11T22:22:03Z) Confirmed the branch is
  `3-14-4-command-available-non-throwing-executable-probe`, the worktree is
  clean, and the branch is rebased onto `origin/main` from the prior rebase
  task.
- [x] (2026-06-11T22:32:21Z) Milestone 1 test baseline added and validated.
  Captured baseline suites in
  `/tmp/baseline-stdlib-which-netsuke-3-14-4-command-available-non-throwing-executable-probe.out`,
  `/tmp/baseline-expand-netsuke-3-14-4-command-available-non-throwing-executable-probe.out`,
  and
  `/tmp/baseline-bdd-command-available-netsuke-3-14-4-command-available-non-throwing-executable-probe.out`.
  Added diagnostic snapshots, the stdlib predicate matrix, target-level
  manifest expansion coverage, and BDD target-level branch coverage.
- [x] (2026-06-11T22:32:21Z) Ran focused Milestone 1 validation:
  `/tmp/which-diagnostic-snapshots-verify-netsuke-3-14-4-command-available-non-throwing-executable-probe.out`,
  `/tmp/stdlib-which-milestone1-netsuke-3-14-4-command-available-non-throwing-executable-probe.out`,
  `/tmp/expand-milestone1-netsuke-3-14-4-command-available-non-throwing-executable-probe.out`,
  `/tmp/bdd-action-command-available-milestone1-netsuke-3-14-4-command-available-non-throwing-executable-probe.out`,
  and
  `/tmp/bdd-target-command-available-milestone1-netsuke-3-14-4-command-available-non-throwing-executable-probe.out`.
  All passed.
- [x] (2026-06-11T22:32:21Z) Ran deterministic gates before CodeRabbit:
  `make check-fmt`, `make lint`, and `make test`. Logs:
  `/tmp/check-fmt-netsuke-3-14-4-command-available-non-throwing-executable-probe.out`,
  `/tmp/lint-netsuke-3-14-4-command-available-non-throwing-executable-probe.out`,
  and
  `/tmp/test-netsuke-3-14-4-command-available-non-throwing-executable-probe.out`.
  `make lint` initially caught `expect_err` and an unnecessary `Result` in
  `tests/which_diagnostic_snapshot_tests.rs`; the helper now uses an explicit
  `match` and the rerun passed.
- [x] (2026-06-11T22:57:15Z) Milestone 1 CodeRabbit review completed
  after one stuck attempt and one long-running retry. The successful retry log
  is
  `/tmp/coderabbit-milestone1-netsuke-3-14-4-command-available-non-throwing-executable-probe.out`;
  CodeRabbit reported `findings: 0`.
- [x] (2026-06-11T22:38:44Z) First Milestone 1 CodeRabbit attempt reached
  sandbox setup and then produced no output for several minutes. Terminated
  only this session's `coderabbit review --agent` process (`2700605`) after
  confirming other agents had separate CodeRabbit processes running. Retrying
  with output teed to `/tmp`.
- [x] (2026-06-11T23:05:44Z) Re-ran pre-commit validation for the Milestone 1
  test-baseline commit after recording the CodeRabbit result. `make fmt`,
  `make markdownlint`, `make check-fmt`, `make lint`, and `make test` passed.
  Logs:
  `/tmp/fmt-precommit-milestone1-netsuke-3-14-4-command-available-non-throwing-executable-probe.out`,
  `/tmp/markdownlint-precommit-milestone1-netsuke-3-14-4-command-available-non-throwing-executable-probe.out`,
  `/tmp/check-fmt-precommit-milestone1-netsuke-3-14-4-command-available-non-throwing-executable-probe.out`,
  `/tmp/lint-precommit-milestone1-netsuke-3-14-4-command-available-non-throwing-executable-probe.out`,
  and
  `/tmp/test-precommit-milestone1-netsuke-3-14-4-command-available-non-throwing-executable-probe.out`.
- [x] (2026-06-11T23:06:09Z) Milestone 2 typed resolver error refactor
  implemented inside `src/stdlib/which/*`. Added `ResolveError`, removed the
  string-matched `is_not_found_error` path, added `is_command_available`, and
  wrapped resolver entry in `trace_span!("stdlib.which.resolve", ...)` with a
  recorded `cache_hit` field.
- [x] (2026-06-11T23:06:09Z) Ran focused Milestone 2 validation. Logs:
  `/tmp/stdlib-which-after-typed-error-netsuke-3-14-4-command-available-non-throwing-executable-probe.out`,
  `/tmp/lookup-after-typed-error-netsuke-3-14-4-command-available-non-throwing-executable-probe.out`,
  `/tmp/cache-after-typed-error-netsuke-3-14-4-command-available-non-throwing-executable-probe.out`,
  and
  `/tmp/which-diagnostic-snapshots-after-typed-error-netsuke-3-14-4-command-available-non-throwing-executable-probe.out`.
  All passed; the diagnostic snapshots had no drift.
- [x] (2026-06-12T00:25:04Z) Ran full Milestone 2 validation and CodeRabbit
  review after the typed resolver refactor. `make markdownlint`,
  `make check-fmt`, `make lint`, and `make test` passed after fixing Clippy
  findings. Logs:
  `/tmp/markdownlint-milestone2-source-fix-netsuke-3-14-4-command-available-non-throwing-executable-probe.out`,
  `/tmp/check-fmt-milestone2-source-fix-netsuke-3-14-4-command-available-non-throwing-executable-probe.out`,
  `/tmp/lint-milestone2-source-fix-netsuke-3-14-4-command-available-non-throwing-executable-probe.out`,
  and
  `/tmp/test-milestone2-source-fix-netsuke-3-14-4-command-available-non-throwing-executable-probe.out`.
  The final CodeRabbit review log is
  `/tmp/coderabbit-milestone2-final-netsuke-3-14-4-command-available-non-throwing-executable-probe.out`;
  CodeRabbit reported `findings: 0`.
- [x] (2026-06-12T00:28:43Z) Milestone 3 documentation update drafted. Added
  ADR-005, expanded the user guide, design document, and developers' guide,
  indexed the ADR in `docs/contents.md`, and marked roadmap item 3.14.4
  complete without changing adjacent roadmap items.
- [x] (2026-06-12T00:29:42Z) Ran Milestone 3 documentation validation.
  `make fmt`, `make markdownlint`, and `make nixie` passed. Logs:
  `/tmp/fmt-docs-milestone3-netsuke-3-14-4-command-available-non-throwing-executable-probe.out`,
  `/tmp/markdownlint-docs-milestone3-netsuke-3-14-4-command-available-non-throwing-executable-probe.out`,
  and
  `/tmp/nixie-docs-milestone3-netsuke-3-14-4-command-available-non-throwing-executable-probe.out`.
- [x] (2026-06-12T00:48:00Z) Milestone 3 documentation CodeRabbit review
  completed after the documentation commit. Log:
  `/tmp/coderabbit-milestone3-docs-netsuke-3-14-4-command-available-non-throwing-executable-probe.out`.
  CodeRabbit reported `findings: 0`.
- [x] (2026-06-12T00:49:36Z) Ran final repository validation. `make check-fmt`,
  `make test`, `make typecheck`, and `make lint` passed. Logs:
  `/tmp/check-fmt-final-netsuke-3-14-4-command-available-non-throwing-executable-probe.out`,
  `/tmp/test-final-netsuke-3-14-4-command-available-non-throwing-executable-probe.out`,
  `/tmp/typecheck-final-netsuke-3-14-4-command-available-non-throwing-executable-probe.out`,
  and
  `/tmp/lint-final-netsuke-3-14-4-command-available-non-throwing-executable-probe.out`.
- [x] (2026-06-13T00:00:00Z) Addressed post-PR review warnings. Closed the
  execplan status gap, moved MiniJinja/localised `ResolveError` rendering into
  the `src/stdlib/which/mod.rs` adapter boundary, removed unbounded `command`
  fields from resolver/workspace tracing, and added low-cardinality metrics for
  cache outcomes and resolution outcomes. Focused validation passed with
  `/tmp/typecheck-pr-warning-fix-netsuke-3-14-4-command-available-non-throwing-executable-probe.out`;
  final validation passed with
  `/tmp/check-fmt-pr-warning-fix-final-netsuke-3-14-4-command-available-non-throwing-executable-probe.out`,
  `/tmp/lint-pr-warning-fix-final-netsuke-3-14-4-command-available-non-throwing-executable-probe.out`,
  `/tmp/typecheck-pr-warning-fix-final2-netsuke-3-14-4-command-available-non-throwing-executable-probe.out`,
  `/tmp/markdownlint-nixie-pr-warning-fix-final-netsuke-3-14-4-command-available-non-throwing-executable-probe.out`,
  and
  `/tmp/test-pr-warning-fix-final-netsuke-3-14-4-command-available-non-throwing-executable-probe.out`.

Once implementation begins, each milestone gets its own checked entry with a
UTC timestamp, the commands run, the log paths under `/tmp/`, and a one-line
result. Split a partially completed milestone into "done" and "remaining"
entries rather than blur progress.

## Surprises & discoveries

None recorded yet. Append observations during implementation with the evidence
(file path, line, command output) and the impact on this plan.

- 2026-06-11: `tests/std_filter_tests/` is a support directory rather than a
  Cargo integration-test target;
  `cargo test --all-targets --all-features --test std_filter_tests -- --list`
  did not execute a target named `std_filter_tests`. Impact: the Milestone 1
  diagnostic snapshot tests must be added as a root integration test file so
  Cargo actually runs them.
- 2026-06-11: `src/manifest/expand_test_cases/condition_cases.rs` is already
  414 lines before this work. Impact: target-level `command_available` unit
  coverage will go in a new sibling module instead of growing the over-limit
  file.
- 2026-06-11: `StdlibConfig::with_path_override` fixes the override when the
  MiniJinja environment is registered, so it cannot model a later PATH change
  inside the same resolver cache. Impact: the `fresh=true` cache-bypass test
  must use the existing scenario-safe `EnvLock` and `VarGuard` helpers rather
  than `path_override`.
- 2026-06-11: `which` filter unknown-keyword diagnostics are currently only
  reached after a successful resolver lookup because `resolve_with` calls
  `kwargs.assert_all_used()` after `resolver.resolve(...)`. Impact: the
  diagnostic snapshot fixture writes a real `tool` in the workspace so the
  existing user-visible unknown-keyword diagnostic is captured rather than a
  not-found diagnostic.
- 2026-06-11: `EnvSnapshot::capture` had two existing error sites outside the
  execplan's original `ResolveError` sketch: failing to read the process cwd
  and a non-UTF-8 process cwd. Impact: the implementation keeps those as typed
  `CwdResolve` and `CwdNonUtf8` variants rather than coercing them into
  argument errors, preserving the previous user-visible diagnostics.
- 2026-06-12: CodeRabbit repeatedly stalled for several minutes at sandbox
  setup before advancing. Impact: later review runs should wait on the same
  process rather than spawning duplicate reviews unless the process is clearly
  wedged; the successful Milestone 2 final review eventually reported
  `findings: 0`.
- 2026-06-13: The first PR-ready review caught that `ResolveError` was typed
  but still owned MiniJinja conversion and localized rendering. Impact:
  `ResolveError` is now data plus low-cardinality category only; MiniJinja
  conversion lives in `src/stdlib/which/mod.rs`.
- 2026-06-13: The same review flagged unbounded command names in resolver
  tracing. Impact: resolver and workspace fallback telemetry now emits cache
  and error categories without recording arbitrary command strings.

## Decision log

- 2026-06-05: scoped the plan to keep the typed `ResolveError` enum private
  to `src/stdlib/which/*`. Rationale: a public typed error would change the
  crate's API surface and tighten the contract beyond what 3.14.4 needs;
  internal use is enough to remove the string-matched leak.
- 2026-06-05: chose to add an ADR for the typed-error decision. Rationale:
  the refactor is hard to reverse, touches every callsite of
  `WhichResolver::resolve`, and matches the criteria in `arch-decision-records`
  ("hard to reverse, will outlive its author").
- 2026-06-05: deferred a Kani harness for the resolver. Rationale: the
  bounded property is well-suited to `proptest` (small, generated inputs over
  the option matrix); a full bounded model check would not add justifiable
  coverage given the resolver's heavy filesystem dependency.
- 2026-06-05: chose `Result<Vec<Utf8PathBuf>, ResolveError>` over
  `Result<WhichOutcome, ResolveError>` (where `WhichOutcome::NotFound` would be
  data rather than an error). Rationale: the `which` filter and function must
  still raise on absence, so a "found-or-not-found" outcome type would force
  every existing caller to re-raise `NotFound` itself; concentrating that
  translation in one `From` impl plus one predicate matcher keeps the refactor
  narrow and matches the Rust `which` crate's own
  `Error::CannotFindBinaryPath`-as-typed-error shape.
- 2026-06-05: chose `Result<Vec<Utf8PathBuf>, ResolveError>` over
  `Result<Option<Vec<Utf8PathBuf>>, ResolveError>`. Rationale: the `Option`
  shape drops the `dirs` and `cwd_mode` metadata carried by
  `ResolveError::NotFound`, which the `which` filter's diagnostic uses to
  render the PATH preview and the platform-specific cwd hint. Preserving that
  metadata is a hard requirement of the `Diagnostic drift` tolerance.
- 2026-06-05: pinned the "no place to search" case (Netsuke's analogue of
  the Rust `which` crate's `CannotGetCurrentDirAndPathListEmpty`) to coerce to
  `false` in `command_available`. Rationale: CMake `find_program` records
  `<VAR>-NOTFOUND`, Bazel `repository_ctx.which` returns `None`, and Meson
  `find_program(required: false)` returns `external_program.found() == false`
  in the same situation. Treating it as `which::args` would surprise authors of
  feature-detection manifests.
- 2026-06-05: scoped the new diagnostic-format insurance to `insta`
  snapshots captured before any production-code edit. Rationale: the refactor
  moves message construction through a `From` impl, a path where a
  single-character drift is easy to ship; locking the on-the-wire text in
  Milestone 1 means Milestone 2 cannot accidentally rewrite a user-visible
  diagnostic.
- 2026-06-11: treat the user's implementation request as explicit approval
  to execute this ExecPlan. Rationale: the plan's approval gate blocks only
  silent or inferred execution; the user directly asked to proceed and added
  review and commit cadence requirements.
- 2026-06-11: place diagnostic snapshot tests in a root integration-test file
  instead of `tests/std_filter_tests/which_diagnostic_snapshot_tests.rs`.
  Rationale: Cargo only builds root files under `tests/` as integration test
  targets; putting the snapshots solely under the existing support directory
  would leave the baseline unexecuted.
- 2026-06-11: use `EnvLock` and `VarGuard` for the `fresh=true` predicate
  cache test. Rationale: this is the existing project mechanism for safe
  environment mutation, and the requested `path_override` approach cannot
  express a post-registration PATH change.
- 2026-06-13: moved `From<ResolveError> for minijinja::Error` from
  `src/stdlib/which/resolve_error.rs` to `src/stdlib/which/mod.rs`. Rationale:
  the resolver error model should be framework-agnostic data at the domain
  boundary; only the MiniJinja registration adapter should render localized
  messages or choose `ErrorKind`.
- 2026-06-13: added the `metrics` facade dependency and instrumented
  `WhichResolver` with counters for cache outcomes and resolution outcomes.
  Rationale: the existing code emitted tracing only; low-cardinality metrics
  make not-found/error rates and cache hit rate observable without installing a
  global recorder in the library.
- 2026-06-13: removed command names from resolver and workspace fallback
  tracing. Rationale: manifest input is unbounded and may contain arbitrary
  user data; cache outcome, result, and error category are sufficient for
  operational triage without high-cardinality fields.

## Outcomes & retrospective

Roadmap item 3.14.4 is complete. `command_available(name, **kwargs)` now reuses
the `which` resolver and cache, returns `false` for both PATH-search and
direct-path absence, and continues to raise the existing `which::args`
diagnostics for misuse. The implementation no longer string-matches rendered
error text; `WhichResolver::resolve` returns a typed `ResolveError`, and the
MiniJinja adapter boundary converts that typed error into the existing
localized diagnostics.

The user-facing contract is documented in `docs/users-guide.md` and
`docs/netsuke-design.md`, with the internal typed-error decision captured in
ADR-005 and indexed from `docs/contents.md`. `docs/roadmap.md` marks 3.14.4
complete, and this execplan status now matches that checkbox.

Validation covered unit, behavioural, property, snapshot, lint, typecheck,
formatting, Markdown, and Mermaid gates during the milestone work. A post-PR
review then identified two architectural/observability refinements: keep
`ResolveError` framework-agnostic, and avoid high-cardinality command fields in
telemetry. Both refinements were applied before merge readiness, with focused
typechecking and full gates passing before the final follow-up commit.

The main lesson for related items (`3.14.5`, `3.14.8`, `3.14.11`) is that typed
stdlib helper errors should be split from adapter rendering from the start.
Future helpers should expose data and low-cardinality categories from their
domain modules, translate into MiniJinja only in the registration module, and
pair tracing spans with metrics counters when runtime behaviour affects
operator diagnosis.
