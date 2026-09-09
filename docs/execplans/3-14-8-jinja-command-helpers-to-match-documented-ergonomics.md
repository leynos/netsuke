# Make Jinja command helpers match the documented ergonomics (3.14.8)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: DRAFT

## Purpose / big picture

Netsuke manifests are YAML documents whose string fields are rendered as Jinja
templates before the build graph is built. Netsuke's design documents promise
four template helpers that do not exist in the code: an `env()` function that
accepts a fallback value, a filter that makes an arbitrary string safe to paste
into a shell recipe, a filter that turns a list into a safe command fragment,
and a filter that drops empty entries from a list. Today `env('X')` fails the
whole build when `X` is unset, and there is no supported way to quote a value,
so manifest authors reach for shell parameter expansion such as
`${RUSTFLAGS:+$RUSTFLAGS }` inside recipes. That is exactly the construct
Netsuke's Ninja backend has to escape around, and it does not work at all on
Windows, where recipes run under Windows PowerShell.

After this change a manifest author can write, in a `Netsukefile`:

```yaml
targets:
  - name: build-stamp
    command: >-
      RUSTFLAGS={{ [base_flags, env('RUSTFLAGS', default='')]
        | compact | join(' ') | shell_quote }}
      cargo build && touch {{ outs }}
```

and observe that:

1. `netsuke generate` succeeds whether or not `RUSTFLAGS` is set in the
   environment.
2. When `RUSTFLAGS` is unset, the generated `build.ninja` contains
   `RUSTFLAGS='-D warnings'` — one shell word, no empty argument, and no
   `${...:+...}` expansion.
3. When `RUSTFLAGS` is set to `-C target-cpu=native --cfg 'a b'`, the generated
   command still contains exactly one shell word for the value, and running the
   build passes that exact string to `cargo` rather than word-splitting it.
4. The `shell_escape` filter that the user guide currently describes as "not
   implemented" no longer appears anywhere; the guide names `shell_quote`, and
   `shell_quote` exists.

The observable acceptance is behavioural, not structural: a documented,
executed example in `docs/stdlib-yaml-and-jinja-guide.md` builds this exact
manifest and asserts the generated Ninja text.

## Context and orientation

Read this section before touching anything. It assumes no prior knowledge of
this repository.

### What Netsuke does

Netsuke reads a YAML manifest (`Netsukefile`), renders every string field as a
Jinja template using the [MiniJinja](https://docs.rs/minijinja) crate, lowers
the result into an intermediate representation (IR), and writes a `build.ninja`
file that the [Ninja](https://ninja-build.org/) build tool executes. "Jinja" is
a template language; a *filter* is written `value | name(arguments)` and a
*function* is written `name(arguments)`.

### Where the template helpers live

There are three registration surfaces, all reachable from
`src/manifest/mod.rs::from_str_named` (lines 107-178):

1. `src/manifest/mod.rs:131-138` registers `env` and `glob` directly on the
   `minijinja::Environment`. These two names are manifest-loader-owned, not
   part of the standard library, and are listed in `RESERVED_VAR_NAMES`
   (`src/manifest/mod.rs:197`) so a manifest `vars:` entry cannot shadow them.
2. `src/stdlib/register.rs::register_with_config` (line 101) wires the full
   standard library: file tests, path filters, collection filters, time
   functions, network functions, command wrappers, and the `which` family.
3. `src/stdlib/register.rs::register_manifest_query` (line 135) wires a
   restricted, side-effect-free subset used when rendering discovery metadata
   for `netsuke help targets`. It does not merely omit the unsafe helpers: it
   re-registers each one with a stub that always fails
   (`register_always_disabled_query_helpers`, line 172, and
   `register_host_dependent_query_helpers`, line 213). `env` is one of those
   stubs, at `src/stdlib/register.rs:173-176`.

There is **no test asserting parity** between surfaces 2 and 3. Adding a helper
to one and forgetting the other is caught only by review. This plan adds
targeted coverage for the four helpers it introduces; a general parity test is
out of scope.

### How `env()` works today

`src/manifest/mod.rs:129-133`:

```rust
let reader = Arc::clone(env_reader);
jinja.add_function("env", move |var_name: String| {
    env_var_with(&var_name, |key| reader(key))
});
```

`EnvReader` is `Arc<dyn Fn(&str) -> Result<String, EnvReadError> + Send + Sync>`
(`src/manifest/env_reader.rs:56`). It exists because Netsuke forbids reading
`std::env::var` outside a thin injected seam; see
`docs/adr-008-environment-seam-taxonomy.md`, which names this exact site as the
canonical `Arc`-closure seam (the `Arc` is required because MiniJinja's
`add_function` demands `Send + Sync`).

`env_var_with` (`src/manifest/env_reader.rs:90-111`) maps the two failure modes:

```rust
Err(EnvReadError::NotPresent) => {
    tracing::debug!(failure_kind = "not_present", "manifest env lookup failed");
    Err(Error::new(
        ErrorKind::UndefinedError,
        localization::message(keys::MANIFEST_ENV_MISSING).to_string(),
    ))
}
Err(EnvReadError::NotUnicode) => {
    tracing::debug!(failure_kind = "not_unicode", "manifest env lookup failed");
    Err(Error::new(
        ErrorKind::InvalidOperation,
        localization::message(keys::MANIFEST_ENV_INVALID_UTF8).to_string(),
    ))
}
```

Neither message names the variable, deliberately: the doc comment at
`src/manifest/env_reader.rs:83-89` explains that variable names "routinely
identify credentials". Two `insta` snapshots pin the rendered strings exactly,
at `tests/manifest_env_tests.rs:108-122`:

```text
undefined value: A required environment variable is not set. (in <string>:1)
invalid operation: An environment variable contains invalid UTF-8. (in <string>:1)
```

An existing unit test, `empty_value_is_returned_rather_than_treated_as_missing`
(`src/manifest/tests/env_function.rs`), pins that an empty string is a *value*,
not an absence.

### Which shell actually runs a recipe

This is the single most important fact for the quoting design, and it is not
what a reader would assume.

`src/recipe_shell.rs` defines:

```rust
pub enum RecipeShell {
    /// Use the host POSIX shell through Ninja's ordinary Unix execution path.
    Posix,
    /// Use Windows PowerShell with an encoded script argument.
    PowerShell,
    /// Use an explicitly selected Bash compatibility runtime on Windows.
    Bash,
}

impl RecipeShell {
    pub(crate) const fn host_default() -> Self {
        if cfg!(windows) { Self::PowerShell } else { Self::Posix }
    }
}
```

On Windows, Netsuke wraps every recipe as
`powershell.exe -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass
-EncodedCommand <base64>`
(`src/ninja_gen_recipe_shell.rs:16-17`). It never uses `cmd.exe`.
`docs/users-guide.md:330-348` ("Windows legacy recipe contract") states this
normatively. On Unix the command is emitted bare for Ninja to run through its
own POSIX path.

A Windows user may set `NETSUKE_WINDOWS_SHELL=bash` to select an explicit Bash
compatibility runtime (`src/runner/recipe_shell.rs:24-58`). The runner resolves
this once, before dispatch (`src/runner/mod.rs:149`), and threads the result
through `ExecutionContext.graph_generation.recipe_shell` into IR lowering
(`src/runner/generation.rs:109`) and Ninja generation
(`src/runner/generation.rs:132`).

Consequently, POSIX `sh` quoting is correct for `RecipeShell::Posix` and
`RecipeShell::Bash`, and **wrong** for `RecipeShell::PowerShell`.

### The quoting that already exists

There are three distinct quoting paths today. They are not interchangeable and
`docs/developers-guide.md:445-458` already documents the distinction.

1. `src/ir/cmd_interpolate/substitution.rs::quote_path` (lines 132-151) quotes
   `{{ ins }}` and `{{ outs }}` for the selected `RecipeShell`:

   ```rust
   fn quote_path(path: &Utf8PathBuf, shell: RecipeShell) -> String {
       if shell == RecipeShell::PowerShell {
           return format!("'{}'", path.as_str().replace('\'', "''"));
       }
       // Utf8PathBuf guarantees UTF-8, and shell quoting should preserve it.
       let bytes: Vec<u8> = path.as_str().quoted(Sh);
       match String::from_utf8(bytes) { /* ... */ }
   }
   ```

   This is the semantics the new template helpers need.

2. `src/stdlib/command/quote.rs::quote` is `#[cfg(windows)]`/
   `#[cfg(not(windows))]` and produces `cmd.exe` quoting on Windows. It
   supports the `shell` and `grep` template filters, which *spawn a process*
   through the platform shell at manifest-render time. It is not exposed to
   templates and must keep its `cmd.exe` behaviour.

3. `shell_single_quote` in the Ninja command-list renderer produces a canonical
   single-quoted `eval` payload. `docs/developers-guide.md:449-454` explicitly
   says to keep it local and not generalize it.

The `shell-quote` crate is pinned at `Cargo.toml:137` with
`default-features = false, features = ["sh"]`, so only the `Sh` dialect is
compiled in. `Sh::quoted` returns `Vec<u8>` (not `String`), which is why both
call sites convert.

### What the documentation currently promises

`docs/netsuke-design.md` §4.4 (lines 1233-1307) specifies
`env(var_name, default: Option<String>)` and says the `default` argument is
"planned". §4.5 (lines 1309-1334) specifies three unimplemented filters:

- `| shell_escape`: "takes a string or list and escapes it for safe inclusion
  as a single argument in a shell command … a non-negotiable security feature".
- `| shell_join`: "accepts a list of arguments and returns one shell-safe
  command fragment. Each list element is quoted as a separate argument."
- `| compact`: "removes empty strings and null values while preserving order.
  It supports patterns such as constructing `RUSTFLAGS` from an optional user
  override without handwritten shell tests."

`docs/users-guide.md:490` says "The `shell_escape` filter described in older
drafts is not implemented in beta3."
`docs/stdlib-yaml-and-jinja-guide.md:318-320` says `env(name)` has "no
default-value argument".

`docs/rfcs/0006-ansible-inspired-template-standard-library.md:1393-1409`
supersedes the `shell_escape` name:

> `text | shell_quote(dialect='sh')` … **This is the same capability as the
> `shell_escape` helper documented but unimplemented today**, which roadmap task
> 3.14.8 exists to resolve. That task remains the owner and ships first; this
> RFC contributes only the canonical name and the `dialect` argument.

`docs/netsuke-design.md:650-658` already writes the target ergonomics into an
illustrative manifest:

```yaml
RUSTFLAGS: >-
  {{ [rust_flags, env('RUST_FLAGS', default='')] | compact | join(' ') }}
```

### Localization is a hard gate

Every user-facing string is a Fluent message. Adding one means:

1. adding a constant to the `define_keys!` block in
   `src/localization/keys.rs`, and
2. adding the message to **all 35** catalogues under
   `locales/<tag>/messages.ftl` with an identical `{ $variable }` name set.

`build.rs:291` calls `build_l10n_audit::audit_localization_keys()`
unconditionally. Missing a catalogue fails `cargo build` — not a test, the
build — with one `- missing in <tag>: <key>` line per locale. See
`docs/developers-guide.md:285-302` and `docs/translators-guide.md`.

### Testing infrastructure

- Unit and integration tests run under `cargo-nextest` (`make test-nextest`),
  which gives each test its own process. Doctests run separately
  (`make doctest`). `make test` is both.
- `proptest` is used widely. The house idiom is a `proptest!` block opening
  with an inner attribute that sets `ProptestConfig { cases: 128, .. }` — see
  `src/ninja_gen_property_tests/dependency_only.rs` in full. Regression seeds
  live under `proptest-regressions/`, mirroring the `src/` path.
- `rstest-bdd` feature files live in `tests/features/*.feature` and are
  auto-discovered by `scenarios!("tests/features", ...)` in
  `tests/bdd_tests.rs`. **New `.feature` files need no registration**; new step
  *modules* need a `mod` line in `tests/bdd/steps/mod.rs`.
  `tests/features/stdlib.feature` and `tests/bdd/steps/stdlib/` are the
  existing template-helper suite.
- `insta` snapshots live in `src/snapshots/<subdir>/` and
  `tests/snapshots/<subdir>/`, with the path set explicitly via
  `Settings::set_snapshot_path`.
- `googletest` matchers (`assert_that!(x, contains_substring(y))`) and
  `pretty_assertions::assert_eq` are both in use; see
  `src/cli/discovery_layer_tests.rs:10-11` and lines 140-178.
- **Documented examples are executed.** A fenced block in `README.md`,
  `docs/users-guide.md`, or `docs/stdlib-yaml-and-jinja-guide.md` must be
  preceded by `<!-- tested-example: <id> -->`. The loader is
  `tests/documentation_examples/mod.rs`; the id must also be added to
  `EXPECTED_EXAMPLE_IDS` in `tests/documentation_examples_tests.rs:18-61` or
  the suite fails with "documented example registry drifted". YAML examples are
  run as real manifests via `manifest_workspace(id)`.
- `tests/integration_test_wiring_tests.rs` asserts every `tests/<dir>/mod.rs`
  tree is declared by some top-level `tests/*.rs`, and that Cargo discovers
  every top-level integration-test source.

### Non-negotiable house rules that bear on this work

From `AGENTS.md` and `docs/adr-008-environment-seam-taxonomy.md`:

- In-process environment mutation is forbidden in tests. Inject `EnvReader`,
  `mockable::Env`, or a narrow closure seam. `serial_test` and process-wide
  locks are not an escape hatch.
- No file may exceed 400 lines.
- Every module needs a `//!` comment; every function, public or private, needs
  a `///` comment. `make doc-coverage` enforces an 80% aggregate.
- The toolchain enables the Polonius borrow checker and the next-generation
  trait solver. Do not add NLL-era workarounds (double lookups,
  `entry(key.clone())`, defensive `.clone()`), and do not add `-Z` flags.
- Markdown prose wraps at 80 columns; code blocks at 120. `make check-fmt`
  enforces `mdtablefix`-canonical Markdown. `make markdownlint` runs `typos`
  with en-GB-oxendict (Oxford) spelling; backtick technical identifiers rather
  than widening the accepted-word list.

### Applicability of `ortho_config`

`ortho_config` provides layered configuration with localized help. This change
adds no command-line flag and no configuration key: the recipe-shell selection
it depends on is already resolved from `NETSUKE_WINDOWS_SHELL` in
`src/runner/recipe_shell.rs`, which is a runner-level environment seam, not a
configuration layer. `ortho_config` is therefore **not used** by this work. If
a reviewer believes a `dialect` default belongs in project configuration, that
is a scope change: stop and escalate rather than adding a configuration key.

## Conformance basis

There is no Terms of Reference artefact in this repository. The upstream
artefacts are:

| Identifier      | Artefact                                                     | Revision anchor      |
| --------------- | ------------------------------------------------------------ | -------------------- |
| `RM-3.14.8`     | `docs/roadmap.md` lines 282-295                              | at commit `81d44f89` |
| `RM-6.8.3`      | `docs/roadmap.md` lines 1103-1110                            | at commit `81d44f89` |
| `DD-4.4`        | `docs/netsuke-design.md` §4.4, lines 1233-1307               | at commit `81d44f89` |
| `DD-4.5`        | `docs/netsuke-design.md` §4.5, lines 1309-1334               | at commit `81d44f89` |
| `DD-2.6`        | `docs/netsuke-design.md` §2.6, lines 630-700                 | at commit `81d44f89` |
| `RFC-0006-8.9`  | `docs/rfcs/0006-…md` lines 1393-1409                         | at commit `81d44f89` |
| `RFC-0006-13.3` | `docs/rfcs/0006-…md` line 1834                               | at commit `81d44f89` |
| `ADR-008`       | `docs/adr-008-environment-seam-taxonomy.md`                  | Accepted 2026-08-06  |
| `ADR-014`       | `docs/adr-014-backend-text-escaping-seam.md`                 | Accepted             |
| `UG-WIN`        | `docs/users-guide.md:330-348` Windows legacy recipe contract | at commit `81d44f89` |

Predecessors `2.2.4` (archived,
`docs/archive/roadmap-completed-foundations.md:125`) and `3.14.4`
(`docs/roadmap.md:246`) are both complete, so `RM-3.14.8` is unblocked.

Trace chain:

```plaintext
RM-3.14.8 -> DD-4.4 -> EP-M1 -> tests::manifest_env::env_default_cases
RM-3.14.8 -> DD-4.5 (compact) -> EP-M2 -> tests::std_filter::compact_property
RM-3.14.8 -> DD-4.5 (shell_escape) + RFC-0006-8.9 -> EP-M3, EP-M4
    -> tests::shell_quote::sh_roundtrip_property
RM-3.14.8 -> DD-4.5 (shell_join) -> EP-M4 -> tests::shell_join::shlex_roundtrip
UG-WIN + ADR-014 -> EP-M3, EP-M5 -> tests::shell_quote::dialect_follows_recipe_shell
RM-3.14.8 (RUSTFLAGS) -> DD-2.6 -> EP-M6
    -> tests::documentation_examples::stdlib-optional-rustflags-manifest
RM-6.8.3 -> EP-M3 (name and dialect adopted early) -> ADR-021
```

## Constraints

These are hard invariants. Violating one requires escalation, not a workaround.

1. The rendered text of the two existing `env()` diagnostics must not change.
   `tests/manifest_env_tests.rs:108-122` holds `insta` inline snapshots of
   both; they must still pass **without being re-accepted**. Absence with no
   default stays `ErrorKind::UndefinedError`; a non-UTF-8 value stays
   `ErrorKind::InvalidOperation` *even when a default is supplied*.
2. An environment variable whose value is the empty string is a value, not an
   absence. `default` must never replace it.
3. Generated Ninja text for every existing manifest must remain byte-identical.
   No `.snap` file under `tests/snapshots/ninja/` may change.
4. Exactly one implementation of recipe-shell word quoting may exist. The
   `cmd.exe` quoting in `src/stdlib/command/quote.rs` and the `eval`-payload
   `shell_single_quote` are separate, documented paths and must keep their
   current behaviour; do not fold them in and do not add a fourth.
5. No test may call `std::env::set_var` or `remove_var`, directly or through a
   helper, and no production code may call `std::env::var`/`var_os` outside an
   injected seam. `clippy.toml` already denies these.
6. Every new user-facing string is a Fluent message present in all 35
   catalogues with a matching `{ $variable }` set.
7. Public API changes are permitted (the crate is pre-1.0 and unreleased at
   1.0), but no compatibility alias, facade, or deprecated entry point may be
   added. Update every caller in the same change.
8. No file exceeds 400 lines. No `-Z` compiler flag is added anywhere.
9. `make check-fmt`, `make typecheck`, `make lint`, `make doc-coverage`, and
   `make test` must all pass at every milestone boundary.

## Tolerances (exception triggers)

Stop and escalate — do not improvise — when any of these is reached.

1. **Scope**: more than 45 non-generated files changed, or more than 900 net
   added lines outside `locales/` and `docs/`. (The 35 catalogues alone
   contribute roughly 175 lines; they do not count against this.)
2. **Interface**: a public signature outside `src/manifest/` and
   `src/stdlib/config/` must change; or `RESERVED_VAR_NAMES` must grow.
3. **Dependencies**: any new entry in `Cargo.toml`. The plan needs none:
   `shell-quote`, `shlex`, `proptest`, `rstest`, `rstest-bdd`, `insta`,
   `googletest`, `pretty_assertions`, and `mockable` are all present.
4. **Behaviour**: any existing `.snap` file requires re-acceptance, or any
   existing test requires modification beyond adding cases.
5. **Iterations**: a gate still fails after 3 focused fix attempts on the same
   milestone.
6. **Time**: a single milestone exceeds 6 hours of work.
7. **Ambiguity**: a second reading of `RFC-0006-8.9` or `DD-4.5` would produce a
   materially different template surface.
8. **Verification**: the real-shell round-trip property (OBL-SH-ROUNDTRIP)
   cannot be made to run in the sandbox, or its negative control passes.

## Risks

- **R1 — Translation burden.** Five new message keys across 35 catalogues is
  ~175 hand-written lines, and `build.rs` fails the build for any omission.
  Severity: medium. Likelihood: high. Mitigation: add all five keys and all
  catalogue entries in one commit per milestone that introduces them; follow
  `docs/localization-styleguide.md` and `docs/translators-guide.md` §5 for
  variable usage; run `cargo build` (not just `cargo check`) to trigger the
  audit early.
- **R2 — Refactoring `quote_path` changes generated Ninja.** Extracting the
  quoting into a shared module could alter bytes. Severity: high. Likelihood:
  low. Mitigation: the extraction is a pure move. Before and after, run
  `make test-nextest` and confirm
  `git status --short tests/snapshots src/snapshots` is empty. Constraint 3
  makes any diff an escalation.
- **R3 — RFC 0006 is being restructured concurrently.** Branch
  `6-1-1-split-rfc-0006-set-into-focused-child-rfcs-and-task` exists on the
  remote and touches the same RFC. Severity: medium. Likelihood: medium.
  Mitigation: keep the RFC 0006 edit to the two paragraphs at lines 1393-1409
  and 1834; rebase onto `origin/main` immediately before requesting review; if
  the RFC has been split by then, apply the same edit to whichever child RFC
  owns §8.9 and record the redirection in `Decision log`.
- **R4 — Host-dependent filter output breaks snapshots.** `shell_quote` with no
  explicit `dialect` produces different text on Windows. Severity: medium.
  Likelihood: high if unguarded. Mitigation: every snapshot and every
  documented example passes an explicit `dialect`, or is gated with
  `#[cfg(unix)]`. The host-default behaviour is covered only by tests that
  assert *which dialect was selected*, not its output.
- **R5 — Subprocess property tests are slow.** Spawning `/bin/sh` once per
  generated case multiplies cost. Severity: low. Likelihood: medium.
  Mitigation: cap the subprocess property at `cases: 64`; keep the pure
  `shlex`-based properties at the house default of 128. If the file exceeds 10
  seconds under nextest, move it behind `#[ignore]` **only** with an
  escalation, never silently.
- **R6 — Documentation gates.** `typos` enforces Oxford spelling over Markdown;
  `mdtablefix` enforces canonical tables; `make fmt` reformats unrelated files
  if they were already non-canonical. Severity: low. Likelihood: medium.
  Mitigation: run `make fmt` then inspect `git diff --stat`; if files unrelated
  to this change are reformatted, commit that reformatting separately and note
  it in `Surprises & discoveries`.
- **R7 — Doc-comment coverage.** Every new function, public or private, needs a
  `///` comment or `make doc-coverage` drops below 80%. Severity: low.
  Likelihood: medium. Mitigation: write the doc comment with the function, not
  afterwards.
- **R8 — Manifest-query surface drift.** There is no parity test between
  `register_with_config` and `register_manifest_query`. Severity: medium.
  Likelihood: medium. Mitigation: EP-M2 and EP-M4 each add an explicit
  manifest-query test for the helper they introduce, and EP-M1 adds one proving
  the disabled `env` stub still reports "disabled", not an argument-count
  error, when called with `default=`.

## Decision log

- **Decision D1**: Ship the helper as `shell_quote`, not `shell_escape`, and
  rewrite every `shell_escape` reference in `docs/netsuke-design.md` §4.5,
  `docs/users-guide.md:490`, `docs/netsuke-design.md:686-689`, and
  `docs/netsuke-design.md:3664-3667` to name `shell_quote`. Rationale:
  `RFC-0006-13.3` states that roadmap 3.14.8 "owns the shell-quoting capability
  and ships first" and that the RFC "contributes the canonical name
  `shell_quote` and the `dialect` argument; the roadmap task should adopt them
  so the two do not diverge". `RM-6.8.3` repeats this. Shipping `shell_escape`
  now would put a known-superseded name on the public template surface and
  force a breaking rename later. `RM-3.14.8` explicitly permits "implement **or
  remove**" the `shell_escape` name; this removes it by superseding it.
  Date/Author: 2026-09-08, planning session, confirmed by the requester.
- **Decision D2 (proposed deviation from `RFC-0006-8.9`)**: `shell_quote` and
  `shell_join` accept `dialect='sh'` **and** `dialect='powershell'`, and
  default to the dialect implied by the active `RecipeShell` rather than always
  to `sh`. Rationale: `RFC-0006-8.9` says "`dialect` currently accepts only
  `sh`, matching the single `shell-quote` feature Netsuke enables." That
  premise is false in the code as it stands. `UG-WIN` and
  `src/recipe_shell.rs:19-26` establish that the default Windows recipe
  interpreter is Windows PowerShell, and
  `src/ir/cmd_interpolate/substitution.rs:132-151` already implements a second,
  non-`shell-quote` dialect for exactly that case. Shipping an `sh`-only filter
  would emit POSIX quoting into a PowerShell recipe, which is a silent
  injection-safety defect in the very helper whose stated purpose (`DD-4.5`) is
  to be "a non-negotiable security feature to prevent command injection
  vulnerabilities". Affected identifiers: `RFC-0006-8.9`, `RFC-0006-13.3`,
  `RM-6.8.3`, `DD-4.5`. Impacts: `docs/rfcs/0006-…md` §8.9 must be amended to
  record the two-dialect set and the host-default rule; `RM-6.8.3` is reduced
  to a no-op. Options considered: (a) two dialects with a host default —
  chosen; (b) `sh` only, raising a template error on a PowerShell host —
  rejected because it makes the filter unusable in default Windows manifests;
  (c) `sh` only with no platform awareness — rejected as silently unsafe.
  Approving authority: the requester, who selected option (a) and D1 explicitly
  before this plan was written. Date/Author: 2026-09-08, planning session.
- **Decision D3**: `dialect='bash'` is **not** accepted. `RecipeShell::Bash`
  maps to the `sh` dialect, because `shell_quote`'s `Sh` output is documented
  as "a lowest common denominator for Bash, Z Shell, and `/bin/sh`-like
  shells". Rationale: the `shell-quote` crate's `Bash` type emits a *different*,
  `$'...'`-style encoding that Netsuke does not compile in (`Cargo.toml:137`
  enables only `sh`). Accepting the name now would lock in a meaning that a
  future `bash` dialect would have to break. Date/Author: 2026-09-08, planning
  session.
- **Decision D4**: `env(name, default=none)` is equivalent to omitting
  `default`, and a non-string `default` fails with MiniJinja's own
  argument-type error rather than a Netsuke message. Rationale:
  `kwargs.get::<Option<String>>("default")?` gives precisely this behaviour,
  and the existing `which` family already relies on MiniJinja's own
  `"unknown keyword argument"` text (`tests/stdlib_which_tests.rs:361-364`).
  This avoids a sixth message key across 35 catalogues for a case a manifest
  author reaches only by mistake. Date/Author: 2026-09-08, planning session.
- **Decision D5**: `compact` drops `none`, undefined, and the empty string, and
  keeps `0`, `false`, `[]`, and `{}`. Rationale: `DD-4.5` says exactly "removes
  empty strings and null values while preserving order". Dropping
  falsy-but-present values would silently discard a meaningful `0` from a flag
  list. Date/Author: 2026-09-08, planning session.
- **Decision D6**: `shell_quote` and `shell_join` are registered on the
  manifest-query surface as *working* helpers, not disabled stubs. Rationale:
  they are pure — they read no clock, filesystem, network, process, or
  environment state — so they disclose nothing about the host. `compact` is
  pure for the same reason and lands automatically, because
  `collections::register_filters` is already called by both surfaces
  (`src/stdlib/register.rs:158` and `:169`). Date/Author: 2026-09-08, planning
  session.
- **Decision D7**: `ortho_config` is not used. See "Applicability of
  `ortho_config`" above. Date/Author: 2026-09-08, planning session.

## Interfaces and dependencies

Be prescriptive. At the end of this work the following must exist.

### `src/recipe_shell/` (module promoted from `src/recipe_shell.rs`)

`src/recipe_shell/mod.rs` keeps the `RecipeShell` enum and `host_default`
unchanged, and gains:

```rust
mod quoting;

pub(crate) use quoting::quote_word;
```

`src/recipe_shell/quoting.rs`:

```rust
/// Quote one string as a single word for `shell`, without validating it.
///
/// The caller owns the policy question of which inputs are quotable; this
/// function only encodes. `quote_path` relies on that split so path quoting
/// keeps its existing total behaviour.
pub(crate) fn quote_word(shell: RecipeShell, value: &str) -> String;
```

The body is moved verbatim from
`src/ir/cmd_interpolate/substitution.rs::quote_path`, generalized from
`&Utf8PathBuf` to `&str`. `quote_path` becomes a one-line caller. The module
doc comment on `src/recipe_shell/mod.rs` is updated from "data-only" to record
that it now also owns the interpreter's word-quoting rule, and states that it
must stay below IR lowering, Ninja rendering, and the standard library.

### `src/stdlib/shell/` (new)

`src/stdlib/shell/dialect.rs`:

```rust
/// The shell dialect a template helper quotes for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ShellDialect {
    /// POSIX `sh` word quoting, also correct for Bash and Z Shell.
    Sh,
    /// Windows PowerShell single-quoted string quoting.
    PowerShell,
}

impl ShellDialect {
    /// Names accepted by the `dialect` keyword argument, in error order.
    pub(crate) const ACCEPTED: [&'static str; 2] = ["sh", "powershell"];

    /// Select the dialect implied by an active recipe interpreter.
    pub(crate) const fn for_recipe_shell(shell: RecipeShell) -> Self;

    /// Parse one `dialect` keyword argument, case-insensitively.
    pub(crate) fn parse(raw: &str) -> Option<Self>;

    /// Map back to the recipe interpreter whose quoting rule this dialect is.
    pub(crate) const fn recipe_shell(self) -> RecipeShell;
}
```

`src/stdlib/shell/policy.rs`:

```rust
/// Why a value cannot be quoted for a shell recipe.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum QuotePolicyError {
    /// The value contains NUL, carriage return, or line feed.
    ContainsControlCharacter,
}

/// Quote one string for `dialect`, rejecting values a recipe cannot carry.
///
/// # Errors
///
/// Returns [`QuotePolicyError::ContainsControlCharacter`] when `value` contains
/// `\0`, `\r`, or `\n`. A Ninja `command =` line is single-line by
/// construction, so such a value could never survive lowering.
pub(crate) fn quote_for_recipe(
    dialect: ShellDialect,
    value: &str,
) -> Result<String, QuotePolicyError>;

/// Quote each element of `values` and join them with one space.
///
/// # Errors
///
/// Propagates [`QuotePolicyError`] from any element.
pub(crate) fn join_for_recipe<'a>(
    dialect: ShellDialect,
    values: impl IntoIterator<Item = &'a str>,
) -> Result<String, QuotePolicyError>;
```

`quote_for_recipe` validates, then delegates to
`crate::recipe_shell::quote_word`. This is the whole of the "no second quoting
implementation" guarantee: `policy.rs` owns *whether*, `quoting.rs` owns *how*.

`src/stdlib/shell/mod.rs`:

```rust
/// Register the pure shell-text filters on an environment.
pub(crate) fn register_filters(env: &mut Environment<'_>, default: ShellDialect);
```

It registers exactly two filters:

- `value | shell_quote(dialect=<name>)`
- `values | shell_join(dialect=<name>)`

Both read `dialect` with `kwargs.get::<Option<String>>("dialect")?`, fall back
to `default`, and finish with `kwargs.assert_all_used()?`, matching the
ordering in `src/stdlib/which/mod.rs:82-92`.

### `src/stdlib/collections.rs`

`register_filters` gains one line, and the file gains one function:

```rust
env.add_filter("compact", |values: Value| compact_filter(&values));

/// Drop null, undefined, and empty-string members, preserving order.
///
/// # Errors
///
/// Returns an error when a non-sequence value cannot be iterated.
fn compact_filter(values: &Value) -> Result<Value, Error>;
```

If `src/stdlib/collections.rs` would exceed 400 lines, move `compact_filter`
and its tests into `src/stdlib/collections/compact.rs` and convert
`collections.rs` into a directory module.

### `src/stdlib/config/mod.rs`

```rust
/// Recipe interpreter whose quoting rules the shell filters follow.
recipe_shell: RecipeShell,

/// Select the recipe interpreter the shell filters quote for.
#[must_use]
pub fn with_recipe_shell(mut self, shell: RecipeShell) -> Self;

/// Return the recipe interpreter the shell filters quote for.
pub(crate) const fn recipe_shell(&self) -> RecipeShell;
```

`StdlibConfig::new` initializes it to `RecipeShell::host_default()`.

`RecipeShell` is already publicly nameable — `src/lib.rs:27` declares
`pub mod recipe_shell;` and the enum is `pub` — so `with_recipe_shell` needs no
visibility widening. `RecipeShell::host_default` stays `pub(crate)`, which is
sufficient because `StdlibConfig::new` is in-crate.

### `src/stdlib/register.rs`

- `register_read_only_helpers` calls
  `shell::register_filters(env, ShellDialect::for_recipe_shell(config.recipe_shell()))`.
- `register_query_helpers` calls
  `shell::register_filters(env, ShellDialect::for_recipe_shell(RecipeShell::host_default()))`.
- The disabled `env` stub changes to:

  ```rust
  env.add_function(
      "env",
      |_variable: String, _kwargs: Kwargs| -> Result<String, Error> {
          Err(manifest_query_operation_error("env"))
      },
  );
  ```

### `src/manifest/mod.rs`

```rust
let reader = Arc::clone(env_reader);
jinja.add_function("env", move |var_name: String, kwargs: Kwargs| {
    let fallback = kwargs.get::<Option<String>>("default")?;
    kwargs.assert_all_used()?;
    env_var_with_default(&var_name, fallback, |key| reader(key))
});
```

### `src/manifest/env_reader.rs`

```rust
/// Read one environment variable, substituting `fallback` only for absence.
///
/// # Errors
///
/// Returns an `UndefinedError` when the variable is absent and `fallback` is
/// `None`, and an `InvalidOperation` error when the value is not valid UTF-8 —
/// the latter regardless of `fallback`, because a present-but-undecodable value
/// is a configuration fault, not an absence.
pub(super) fn env_var_with_default(
    name: &str,
    fallback: Option<String>,
    read_env: impl FnOnce(&str) -> Result<String, EnvReadError>,
) -> Result<String, Error>;
```

`env_var_with` is **removed**, not kept as an alias (constraint 7); its two
existing call sites become `env_var_with_default(name, None, read_env)`.

### New localization keys

Added to `src/localization/keys.rs` in the `STDLIB_*` group, immediately after
the `COMMAND_*` block:

```rust
STDLIB_SHELL_QUOTE_NOT_STRING => "stdlib.shell.quote.not_string",
STDLIB_SHELL_QUOTE_CONTROL_CHARACTER => "stdlib.shell.quote.control_character",
STDLIB_SHELL_DIALECT_INVALID => "stdlib.shell.dialect_invalid",
STDLIB_SHELL_JOIN_NOT_SEQUENCE => "stdlib.shell.join.not_sequence",
STDLIB_SHELL_JOIN_ITEM_NOT_STRING => "stdlib.shell.join.item_not_string",
```

English text (`locales/en-GB/messages.ftl` and `locales/en-US/messages.ftl`):

```text
stdlib.shell.quote.not_string = shell_quote expects a string, received { $kind }.
stdlib.shell.quote.control_character = A value containing a null byte, carriage return, or line feed cannot be quoted.
stdlib.shell.dialect_invalid = Unknown shell dialect { $dialect }; expected one of { $accepted }.
stdlib.shell.join.not_sequence = shell_join expects a sequence, received { $kind }.
stdlib.shell.join.item_not_string = shell_join item { $index } is { $kind }, not a string.
```

The `{ $variable }` name set must be identical in all 35 catalogues; wording
and order may differ. Follow `docs/localization-styleguide.md`.

## Verification plan

Verification is co-designed with the implementation: the split between
`policy.rs` (whether a value may be quoted) and `quoting.rs` (how it is
encoded) exists precisely so the encoding obligation can be discharged against
a real shell while the policy obligation stays a cheap total function.

### Non-trivial axioms

- **AX-1**: `shell_quote::Sh` emits text that a POSIX `sh` decodes back to the
  input byte string. This is a third-party contract; it is *exercised*, not
  proven, by OBL-SH-ROUNDTRIP running a real `/bin/sh`.
- **AX-2**: Windows PowerShell decodes a single-quoted string by collapsing each
  `''` to `'` and treating every other character literally. Exercised against
  real `powershell.exe` only on Windows hosts; discharged against an explicit
  inverse model elsewhere. Residual gap recorded below.
- **AX-3**: `shlex::split` implements POSIX word splitting faithfully enough to
  serve as an oracle for "this text is exactly one word".
  `docs/formal-verification-methods-in-netsuke.md:277` records that whether
  `shlex::split` is part of the semantic acceptance contract or only a guard is
  an open question; this plan uses it only as a *test oracle*, alongside the
  real-shell round trip, never as the sole evidence.
- **AX-4**: MiniJinja's `Kwargs::get::<Option<T>>` yields `None` for an absent
  key and for an explicit `none`, and an error for a type mismatch;
  `assert_all_used` rejects unconsumed keys. Exercised by the unknown-keyword
  cases.
- **AX-5**: A Ninja `command =` value is single-line, so rejecting `\r` and `\n`
  loses no expressible manifest.

Verus is not used: `docs/roadmap.md:428-433` records the accepted phase-1
boundary that Verus is "optional and proof-kernel-only". Kani is not used
either: the invariants below are over unbounded UTF-8 strings, where a bounded
model checker would explore a strictly weaker domain than the property tests,
and the strongest available evidence — differential execution against the real
`/bin/sh` — is outside any model checker's reach. Both exclusions are choices,
not omissions.

### Obligations

**OBL-SH-ROUNDTRIP** — POSIX quoting is faithful.

- Obligation: for every `s` containing no `\0`, `\r`, or `\n`, running
  `sh -c 'printf %s ' + quote_for_recipe(Sh, s)` writes exactly `s` to stdout.
- Method: property test executing a real `/bin/sh` subprocess, `#[cfg(unix)]`.
- Rationale: this is the only method that tests the actual composition of
  Netsuke's policy layer with the third-party encoder and a real shell. A pure
  unit test would restate `shell-quote`'s own tests.
- Domain: a `proptest` string strategy deliberately biased toward shell
  metacharacters. Draw characters from an explicit alphabet containing ASCII
  letters, digits, space, tab, the single quote, the double quote, the dollar
  sign, the backtick, the backslash, the asterisk, the question mark, the
  semicolon, the ampersand, the vertical bar, the angle brackets, the round,
  square, and curly brackets, the hash, the tilde, and the exclamation mark,
  plus a slice of non-ASCII UTF-8. Lengths run from zero to twenty-four
  characters, filtered only for the three forbidden control characters.
- Artefact: `tests/shell_filter_property_tests.rs`.
- Evidence: `cargo nextest run --test shell_filter_property_tests`. Before
  `quote_for_recipe` exists the test fails to compile; after a naive
  implementation it must fail on the first metacharacter case.
- Non-vacuity: the strategy is classified with `proptest`'s
  `prop_assume!`-free design plus an explicit assertion that, across a fixed
  seeded run, at least one generated case contained a single quote, at least
  one contained a dollar sign, and at least one contained a space — asserted by
  a companion deterministic test over a hand-written witness table so the
  property cannot pass vacuously on an all-alphanumeric sample. **Negative
  control**: a sibling `#[test]` applies a deliberately broken quoter (wrap in
  `"` only) to the witness `$HOME 'x'` through the same subprocess harness and
  asserts the harness *rejects* it. If that control ever passes, the harness is
  not measuring anything and the obligation is undischarged.

**OBL-PS-ROUNDTRIP** — PowerShell quoting is faithful.

- Obligation: for every `s` containing no `\0`, `\r`, or `\n`,
  `decode_powershell_single_quoted(quote_for_recipe(PowerShell, s)) == s`.
- Method: property test against an explicit inverse model, plus a
  `#[cfg(windows)]` real-`powershell.exe` round trip of the same shape.
- Rationale: CI for this repository runs Linux and Windows. On Linux the model
  is the only available oracle; on Windows the real interpreter discharges AX-2
  directly.
- Domain: the same string strategy as OBL-SH-ROUNDTRIP.
- Artefact: `tests/shell_filter_property_tests.rs`.
- Evidence: the Linux run proves model conformance; the Windows CI job proves
  the model matches the interpreter.
- Non-vacuity: the model must be written as a *decoder* (strip the outer quotes,
  collapse `''`), never by calling the encoder. A negative control feeds the
  decoder a string quoted with the POSIX encoder and asserts it does **not**
  round-trip.
- Residual gap: on a non-Windows host, AX-2 rests on the model. Stated here
  rather than hidden.

**OBL-ONE-WORD** — a quoted value is exactly one shell word.

- Obligation: for every admissible `s`,
  `shlex::split(&quote_for_recipe(Sh, s)?) == Some(vec![s.to_owned()])`.
- Method: property test (pure, no subprocess), `cases: 128`.
- Rationale: this is the literal claim `RM-3.14.8` makes — "optional
  `RUSTFLAGS` construction without shell parameter expansion". It is also the
  property the IR's own `shlex` guard depends on.
- Domain: as above, plus the empty string as an explicit boundary witness.
- Artefact: `tests/shell_filter_property_tests.rs`.
- Non-vacuity: an assertion that `shlex::split` returns two or more words for
  the *unquoted* form of at least one witness (`a b`), proving the oracle
  distinguishes quoted from unquoted.

**OBL-JOIN-SPLIT** — `shell_join` is the inverse of word splitting.

- Obligation: for every list `xs` of admissible strings,
  `shlex::split(&join_for_recipe(Sh, xs)?) == Some(xs)`.
- Method: property test, `cases: 128`.
- Domain: `prop::collection::vec(word_strategy(), 0..6)`, including the empty
  list (which must yield the empty string) and lists containing empty strings.
- Artefact: `tests/shell_filter_property_tests.rs`.
- Non-vacuity: classify and assert that the sample includes at least one list
  with an element containing a space and at least one with an empty element;
  negative control asserts a naive `xs.join(" ")` fails the same property.

**OBL-COMPACT** — `compact` is an order-preserving filter with a stable
predicate.

- Obligation: for every input sequence `xs`, `compact(xs)` (a) contains no
  `none`, undefined, or empty-string member; (b) is a subsequence of `xs`; (c)
  satisfies `compact(compact(xs)) == compact(xs)`; and (d) equals `xs` when
  `xs` contains no droppable member.
- Method: property test, `cases: 128`.
- Rationale: four connected invariants over an unbounded domain; a table test
  would not establish the subsequence or idempotence claims.
- Domain: a `prop_oneof!` strategy producing `Value::UNDEFINED`,
  `Value::from(())`, `Value::from("")`, `Value::from(0)`, `Value::from(false)`,
  and non-empty strings, collected into vectors of length 0..8.
- Artefact: `tests/std_filter_tests/collection_filters.rs`.
- Non-vacuity: assert that the generated sample contains at least one droppable
  and one retained member across the run, and add an explicit witness case for
  `[0, false, '', none, 'x']` yielding `[0, false, 'x']` — which fails
  immediately under a naive truthiness-based implementation. That naive
  implementation is the negative control.

**OBL-ENV-DEFAULT** — the default substitutes for absence only.

- Obligation: with an injected `EnvReader`, `env(n, default=d)` returns the
  variable's value when present (including when that value is `""`), returns
  `d` when absent, raises `UndefinedError` with the unchanged message when
  absent and `d` is omitted or `none`, and raises `InvalidOperation` with the
  unchanged message when the value is not UTF-8 *even when `d` is supplied*.
- Method: parameterized `rstest` over the finite five-case partition, plus the
  two existing `insta` snapshots re-run unchanged.
- Rationale: the domain is a genuinely finite partition of reader outcomes
  crossed with default presence; enumeration is exhaustive.
- Domain: `{present-nonempty, present-empty, absent, not-unicode}` ×
  `{no default, default=none, default='fallback'}`, minus the impossible
  combinations — 12 cases, all enumerated.
- Artefact: `tests/manifest_env_tests.rs` and
  `src/manifest/tests/env_function.rs`.
- Non-vacuity: the reader is an injected closure that records the key it was
  asked for, so a test asserting `default` was returned also asserts the reader
  was actually consulted — an implementation that returned `default` without
  reading fails. The unchanged `insta` snapshots are the negative control for
  diagnostic drift: any wording change fails them.

**OBL-DIALECT-TOTAL** — dialect selection is total and its error is truthful.

- Obligation: every `RecipeShell` variant maps to exactly one `ShellDialect`;
  every name in `ShellDialect::ACCEPTED` parses; every other name fails with an
  error naming the rejected value and enumerating exactly
  `ShellDialect::ACCEPTED`.
- Method: exhaustive parameterized test over the three-variant `RecipeShell`
  and the two-element `ACCEPTED` list, plus a test asserting the rendered error
  text contains every element of `ACCEPTED` and nothing else from a near-miss
  list (`bash`, `cmd`, `zsh`, `pwsh`).
- Rationale: the domain is finite and small; exhaustive enumeration is the
  strongest available evidence.
- Artefact: `src/stdlib/shell/dialect.rs` `#[cfg(test)] mod tests`.
- Non-vacuity: the near-miss list guarantees the "enumerates exactly" assertion
  can fail; `bash` in particular is the name D3 deliberately rejects.

**OBL-NINJA-STABLE** — extracting `quote_word` changes no generated output.

- Obligation: the Ninja text generated for every existing manifest fixture is
  byte-identical before and after EP-M3.
- Method: the existing `insta` snapshot suite under `tests/snapshots/ninja/`
  and `src/snapshots/`, re-run without re-acceptance.
- Artefact: existing.
- Evidence: `make test-nextest`, then
  `git status --short src/snapshots tests/snapshots` prints nothing.
- Non-vacuity: the suite already fails when generation changes — it caught the
  3.14.7 dollar-escaping work. To confirm it is live for this refactor,
  temporarily alter `quote_word` to emit a double quote instead of a single
  quote, observe at least one snapshot fail, then revert. Record the failing
  snapshot name in `Artefacts and notes`.

**OBL-QUERY-SURFACE** — the manifest-query surface stays coherent.

- Obligation: under `register_manifest_query`, `compact`, `shell_quote`, and
  `shell_join` render successfully, and `env('X', default='y')` fails with the
  "disabled while rendering `netsuke help targets`" marker — not with an
  argument-count error.
- Method: parameterized `rstest` rendering each template against an environment
  built by `crate::stdlib::register_manifest_query`.
- Rationale: R8 records that nothing else enforces this; the `env` stub's arity
  changes in EP-M1 and would otherwise fail with the wrong diagnostic.
- Artefact: `tests/stdlib_manifest_query_tests.rs` (new; must be a top-level
  `tests/*.rs` so `tests/integration_test_wiring_tests.rs` discovers it).
- Non-vacuity: assert the *specific* marker text via
  `stdlib::is_manifest_query_disabled_error`-equivalent substring, not merely
  "an error occurred". Negative control: assert that the same template under
  `register_with_config` does **not** produce that marker.

### Behavioural specification (rstest-bdd)

Added to `tests/features/stdlib.feature`. The steps already exist in
`tests/bdd/steps/stdlib/` — `a stdlib workspace`,
`I render the stdlib template {template:string} without context`,
`the stdlib output equals {expected:string}`, and
`the stdlib error contains {fragment:string}` — so **no new step module is
needed** and `tests/bdd/steps/mod.rs` is untouched. Confirm each step's exact
wording in `tests/bdd/steps/stdlib/{rendering,assertions,workspace}.rs` before
writing the feature; if a needed step is missing, add it to the existing module
rather than creating a new one.

```gherkin
  Scenario: compact drops empty and null members but keeps zero
    Given a stdlib workspace
    When I render the stdlib template "{{ [0, '', none, 'x'] | compact | join(',') }}" without context
    Then the stdlib output equals "0,x"

  Scenario: shell_quote makes a metacharacter-bearing value one sh word
    Given a stdlib workspace
    When I render the stdlib template "{{ \"a b '$HOME'\" | shell_quote(dialect='sh') }}" without context
    Then the stdlib output equals "'a b '\\''$HOME'\\'''"

  Scenario: shell_join quotes each element separately
    Given a stdlib workspace
    When I render the stdlib template "{{ ['-C', 'target-cpu=native', 'a b'] | shell_join(dialect='sh') }}" without context
    Then the stdlib output equals "-C target-cpu=native 'a b'"

  Scenario: shell_quote rejects an unknown dialect and names the accepted set
    Given a stdlib workspace
    When I render the stdlib template "{{ 'x' | shell_quote(dialect='bash') }}" without context
    Then the stdlib error contains "powershell"

  Scenario: shell_quote rejects a value containing a line feed
    Given a stdlib workspace
    When I render the stdlib template "{{ 'a\nb' | shell_quote(dialect='sh') }}" without context
    Then the stdlib error contains "line feed"
```

The expected `sh` output strings above are the `shell-quote` crate's
*fragmented* form (`foo' bar'` rather than `'foo bar'`); confirm each against a
scratch run before committing the feature file, and correct the feature to
whatever the crate actually emits. Do not adjust the implementation to match a
guessed string.

## Plan of work

### Stage A — orient and confirm (no code changes)

Read, in order: this plan's "Context and orientation"; `AGENTS.md`;
`docs/adr-008-environment-seam-taxonomy.md`; `docs/netsuke-design.md` §§2.6,
4.4, 4.5; `docs/rfcs/0006-ansible-inspired-template-standard-library.md` §§8.9
and 13; `docs/users-guide.md:330-348` and `:458-495`;
`docs/stdlib-yaml-and-jinja-guide.md` in full;
`docs/rust-testing-with-rstest-fixtures.md`; `docs/rstest-bdd-users-guide.md`;
`docs/rust-doctest-dry-guide.md`;
`docs/reliable-testing-in-rust-via-dependency-injection.md`;
`docs/documentation-style-guide.md`; `docs/localization-styleguide.md`;
`docs/translators-guide.md` §§4-5.

Load these skills before writing code: `rust-router` (then whichever single
follow-on it routes to — most likely `rust-types-and-apis` for the
`ShellDialect` surface and `rust-errors` for the policy error),
`rust-unit-testing`, `proptest`, `hexagonal-architecture`,
`arch-decision-records` (for ADR-021), `en-gb-oxendict`, and `commit-message`.

Then confirm three facts against the working tree, because the plan depends on
them:

1. `grep -n "shell_escape\|shell_join\|compact" -r src/` returns nothing that is
   a Jinja helper.
2. `ls docs/adr-02*.md` shows `adr-020` as the highest, and
   `git ls-remote --heads origin` plus
   `git ls-tree -r --name-only origin/<branch> -- docs/` for each in-flight
   branch shows no `adr-021`. If 021 is taken, use the next free number and
   update every reference in this plan.
3. `cargo tree -i shell-quote` shows the `sh` feature only.

Go/no-go: if any of the three is false, stop and escalate.

### Stage B — red tests

Each milestone below opens with its failing test. Do not write production code
before observing the intended failure.

### Stage C — implementation with verification

Milestones EP-M1 to EP-M5.

### Stage D — documentation, reconciliation, and wide validation

Milestone EP-M6.

## Milestones and plateaus

Every milestone ends with
`make check-fmt && make typecheck && make lint &&
make doc-coverage && make test`
green and a commit. No milestone introduces a compatibility alias, facade, or
deprecated entry point: the crate is pre-1.0, has no external consumers, and
every caller is in-tree, so each interface is updated together with all of its
callers (see constraint 7).

### EP-M1 — `env(name, default=...)`

- Identifier and outcome: `env` accepts an optional `default` keyword argument;
  the manifest-query stub accepts the same shape; both existing diagnostics are
  unchanged.
- Requirements: `RM-3.14.8` bullet 1, `DD-4.4`.
- Red: add the twelve `OBL-ENV-DEFAULT` cases to `tests/manifest_env_tests.rs`
  and the unit cases to `src/manifest/tests/env_function.rs`. Run
  `cargo nextest run --test manifest_env_tests` and observe failures citing an
  unexpected keyword argument.
- Green: rename `env_var_with` to `env_var_with_default` with the `fallback`
  parameter; update `src/manifest/mod.rs:131` to take `Kwargs`; update the
  disabled stub at `src/stdlib/register.rs:173-176`. Add the `OBL-QUERY-SURFACE`
  `env` case to the new `tests/stdlib_manifest_query_tests.rs`.
- Refactor: keep `env_var_with_default` under 40 lines; extract the fallback
  decision into a named predicate if the match grows a third arm.
- Acceptance evidence: `cargo nextest run --test manifest_env_tests` passes;
  the two `insta` inline snapshots pass **without** `INSTA_FORCE_UPDATE`;
  `git status --short` shows no `.snap` change.
- Conformance check: `DD-4.4`'s "It returns an error if the variable is
  undefined and no `default` is provided, or if the variable contains invalid
  UTF-8" is satisfied exactly; no new public interface beyond the helper's
  keyword argument; no new dependency; trace links current.
- Recovery: revert the single commit; `env()` returns to its one-argument form.
- Remaining gaps: documentation of `default=` (EP-M6).
- Compatibility decision: none required.

### EP-M2 — `compact`

- Identifier and outcome: `values | compact` is available on both registration
  surfaces and drops null, undefined, and empty-string members while preserving
  order.
- Requirements: `RM-3.14.8` bullet 3 (partial), `DD-4.5`.
- Red: add the `OBL-COMPACT` property and the explicit witness case to
  `tests/std_filter_tests/collection_filters.rs`; add the `compact` scenario to
  `tests/features/stdlib.feature`. Observe the unknown-filter failure.
- Green: implement `compact_filter` in `src/stdlib/collections.rs` beside
  `uniq_filter` and `flatten_filter`, and register it in `register_filters`.
  Non-sequence input errors through `values.try_iter()?`, exactly as `uniq`
  does, so no new message key is needed.
- Refactor: if `src/stdlib/collections.rs` passes 400 lines, split it into a
  directory module as described under "Interfaces and dependencies".
- Acceptance evidence: `cargo nextest run -E 'test(compact)'` passes; the naive
  truthiness implementation (negative control) fails the witness case.
- Conformance check: `DD-4.5`'s wording is matched exactly; `compact` reaches
  the manifest-query surface for free because `collections::register_filters`
  is shared — confirm by adding the `compact` case to
  `tests/stdlib_manifest_query_tests.rs`.
- Recovery: revert; the filter disappears with no other effect.
- Remaining gaps: documentation (EP-M6).
- Compatibility decision: none required.

### EP-M3 — shared recipe-shell quoting seam

- Identifier and outcome: a single `quote_word` implementation exists in
  `src/recipe_shell/quoting.rs`; `quote_path` delegates to it; `ShellDialect`
  and `StdlibConfig::with_recipe_shell` exist; generated Ninja is unchanged.
  This milestone registers **no** new template helper — it is a pure structural
  plateau, safe to stop at.
- Requirements: `RM-6.8.3`'s "no second quoting implementation is introduced";
  `ADR-014`.
- Red: add `OBL-DIALECT-TOTAL` tests in `src/stdlib/shell/dialect.rs`; run the
  OBL-NINJA-STABLE non-vacuity check (deliberately break `quote_word`, observe
  a named snapshot fail, revert) and record the snapshot name.
- Green: promote `src/recipe_shell.rs` to `src/recipe_shell/mod.rs`; add
  `quoting.rs` with the body moved from `quote_path`; reduce `quote_path` to a
  delegating call; add `src/stdlib/shell/dialect.rs`; add the `recipe_shell`
  field and builder to `StdlibConfig`.
- Refactor: update the `//!` comment on `src/recipe_shell/mod.rs` to state its
  new responsibility and its position below IR, Ninja, and the standard library.
- Acceptance evidence: `make test` passes;
  `git status --short src/snapshots tests/snapshots` prints nothing.
- Conformance check: exactly one recipe-shell quoting implementation remains;
  `src/stdlib/command/quote.rs` and `shell_single_quote` are untouched; no
  persisted or wire format changed; `RecipeShell` visibility widened only as
  far as `with_recipe_shell` requires.
- Recovery: the extraction is a pure move; revert restores the prior file.
- Remaining gaps: the filters themselves (EP-M4); the runner still injects
  `host_default()` rather than the resolved shell (EP-M5).
- Compatibility decision: none required — `from_str_with_env_and_config` and
  `StdlibConfig` are pre-1.0 and every caller is in-tree.

### EP-M4 — `shell_quote` and `shell_join`

- Identifier and outcome: both filters are registered on both surfaces, with
  the five new message keys present in all 35 catalogues.
- Requirements: `RM-3.14.8` bullets 2 and 3; `DD-4.5`; `RFC-0006-8.9` as
  amended by D2.
- Red: write `tests/shell_filter_property_tests.rs` with OBL-SH-ROUNDTRIP,
  OBL-PS-ROUNDTRIP, OBL-ONE-WORD, OBL-JOIN-SPLIT and both negative controls;
  add the four `shell_*` scenarios to `tests/features/stdlib.feature`. Observe
  compilation failure, then unknown-filter failures.
- Green: add `src/stdlib/shell/policy.rs` and `src/stdlib/shell/mod.rs`; wire
  `shell::register_filters` into both `register_read_only_helpers` and
  `register_query_helpers`; add the five keys to `src/localization/keys.rs` and
  to all 35 `locales/<tag>/messages.ftl` files.
- Refactor: keep each of `mod.rs`, `policy.rs`, and `dialect.rs` well under 400
  lines; if the filter closures grow past a few lines, extract named functions
  so each carries its own `///` comment.
- Acceptance evidence: `cargo build` succeeds (proving the localization audit
  passes); `cargo nextest run --test shell_filter_property_tests` passes; the
  BDD scenarios pass; both negative controls fail as designed when the broken
  implementation is substituted.
- Conformance check: the registered names match `RFC-0006-8.9` as amended; no
  second quoting implementation; both filters are pure and therefore correctly
  available to manifest queries (D6); trace links updated.
- Recovery: revert; the filters and their keys disappear together.
- Remaining gaps: documentation (EP-M6); `NETSUKE_WINDOWS_SHELL=bash` still
  yields the PowerShell default (EP-M5).
- Compatibility decision: none required.

### EP-M5 — inject the resolved recipe shell

- Identifier and outcome: on a Windows host with `NETSUKE_WINDOWS_SHELL=bash`,
  `shell_quote` with no explicit `dialect` produces `sh` quoting, matching the
  interpreter that will actually run the recipe.
- Requirements: `UG-WIN`; closes the last correctness hole in D2.
- Red: add a test that builds a `StdlibConfig` with
  `.with_recipe_shell(RecipeShell::Bash)`, renders `{{ "a b" | shell_quote }}`,
  and asserts `sh` quoting. It fails while the runner still passes
  `host_default()` — assert at the *runner* seam, not only at the config seam,
  so the test is not tautological.
- Green: thread `ExecutionContext.graph_generation.recipe_shell` from
  `src/runner/mod.rs:149` through the manifest-generation path into the
  `StdlibConfig` built in `src/manifest/query.rs:73-74`. If the parameter list
  of a manifest-loading function would exceed four parameters, group them into
  a named struct, per `AGENTS.md`.
- Refactor: ensure `src/manifest/query.rs` stays under 400 lines.
- Acceptance evidence: `make test` passes; the new test fails when the runner
  plumbing is reverted while the config seam is kept — record that transcript.
- Conformance check: `StdlibConfig` and the manifest-loading signatures are the
  only interfaces widened; no configuration key added (D7); no new dependency.
- Recovery: revert this commit only; EP-M4's behaviour (host-default dialect)
  remains correct for every host that does not set `NETSUKE_WINDOWS_SHELL`.
- Remaining gaps: none.
- Compatibility decision: none required — pre-1.0, all callers in-tree.

### EP-M6 — documentation, ADR, and reconciliation

- Identifier and outcome: every document that described these helpers as
  planned or unimplemented now describes what ships, a worked `RUSTFLAGS`
  example is executed by the test suite, ADR-021 records D1-D3, and the roadmap
  entry is ticked.
- Requirements: all of `RM-3.14.8`; `RM-3.14.8` bullet 4 specifically.
- Work:
  1. Write `docs/adr-021-canonical-recipe-shell-quoting-surface.md` following
     the Y-Statement shape of `docs/adr-008-environment-seam-taxonomy.md`
     (`# Architecture decision record (ADR): …`, then `## Status`, `## Date`,
     `## Context and problem statement`, `## Decision`, `## Consequences`).
     Record D1, D2, and D3 with their evidence. Add it to `docs/contents.md`.
  2. `docs/netsuke-design.md` §4.4: replace "The `default` argument is planned;
     the current implementation only accepts the variable name" with the shipped
     contract, including the empty-string rule and the non-UTF-8 rule.
  3. `docs/netsuke-design.md` §4.5: rename `shell_escape` to `shell_quote`,
     record the two-dialect set and the host-default rule, drop "planned" from
     `shell_join` and `compact`, and link ADR-021.
  4. `docs/netsuke-design.md:686-689` and `:3664-3667`: rename `shell_escape`.
  5. `docs/users-guide.md:490`: replace the "not implemented in beta3" sentence
     with a description of `shell_quote`, its default dialect, and the pointer
     to `docs/stdlib-yaml-and-jinja-guide.md`. Cross-reference
     `docs/users-guide.md:330-348` so a Windows reader understands why the
     default differs.
  6. `docs/users-guide.md:774-775`: replace the sentence beginning "Beta3 does
     not accept a default argument" with the shipped `env(name, default=…)`
     contract, including the empty-string and non-UTF-8 rules.
  7. `docs/stdlib-yaml-and-jinja-guide.md`: add `compact` to "Transform
     collections"; add a new "Build shell recipe text" section documenting
     `shell_quote` and `shell_join` with their dialect rules and purity; update
     the `env(name)` bullet at lines 318-320. Add the tested example:

     ```markdown
     <!-- tested-example: stdlib-optional-rustflags-manifest -->
     ```

     carrying the `RUSTFLAGS` manifest from "Purpose / big picture", with an
     explicit `dialect='sh'` so it is host-independent (R4).
  8. Register `stdlib-optional-rustflags-manifest` in `EXPECTED_EXAMPLE_IDS`
     (`tests/documentation_examples_tests.rs:18-61`, alphabetical) and add it as
     a `#[case]` to `documented_manifest_generates_ninja`.
  9. `docs/developers-guide.md`: extend the quoting-paths paragraph at lines
     445-458 to name the new fourth path — `src/recipe_shell/quoting.rs` as the
     single recipe-shell word quoter used by both `quote_path` and the
     `shell_quote`/`shell_join` filters — and state that
     `src/stdlib/command/quote.rs` and `shell_single_quote` remain distinct.
     Also record the "add a helper to both registration surfaces" convention and
     the 35-catalogue message rule as it applies to stdlib helpers.
  10. `docs/repository-layout.md`: add `src/stdlib/shell/` and note the
      `src/recipe_shell/` promotion.
  11. `docs/rfcs/0006-ansible-inspired-template-standard-library.md` §§8.9 and
      13.3: record the amended dialect set and note that 3.14.8 delivered it.
      Keep the edit to those two locations (R3).
  12. `CHANGELOG.md`: one entry under the unreleased heading, following the
      Common Changelog style already used in the file.
  13. `docs/roadmap.md:282-295`: tick 3.14.8 and all four sub-bullets; rewrite
      the trailing `Note:` to describe what shipped. Add a one-line note to
      `RM-6.8.3` (lines 1103-1110) recording that 3.14.8 delivered the canonical
      name and the dialect argument, and that only the wider RFC 0006 dialect
      set remains. Do **not** tick 6.8.3.
- Acceptance evidence: `make markdownlint`, `make nixie`, `make check-fmt`, and
  `make test` all pass; `cargo nextest run --test documentation_examples_tests`
  passes, proving the `RUSTFLAGS` manifest generates valid Ninja.
- Conformance check: no `shell_escape` reference remains
  (`grep -rn "shell_escape" docs/ src/` returns nothing); every upstream
  identifier in `Conformance basis` is either satisfied or has a recorded,
  accepted deviation; the RFC 0006 amendment is the only upstream document
  changed.
- Recovery: documentation-only; revert individually.
- Remaining gaps: none for `RM-3.14.8`.
- Compatibility decision: none required.

## Concrete steps

All commands run from the repository root,
`/home/leynos/.lody/repos/github---leynos---netsuke/worktrees/c72b2360-e95a-451a-b637-1e84cf3eb4b8`,
on branch `3-14-8-jinja-command-helpers-to-match-documented-ergonomics`.

Capture every long-running gate through `tee`, using the project's log naming
convention so output survives truncation:

```sh
make test 2>&1 | tee "/tmp/test-$(get-project)-$(git branch --show-current).out"
```

Delegate full gate runs to the `scrutineer` subagent rather than running them
in the planning context; read the cited log on failure instead of re-running.

Per-milestone loop:

```sh
# 1. Red: write the failing test, then run only it.
cargo nextest run --test manifest_env_tests 2>&1 \
  | tee /tmp/red-3-14-8.out

# Expect, before implementation:
#   FAIL [   0.012s] netsuke::manifest_env_tests env_default_substitutes_for_absence
#   ... unknown keyword argument `default`

# 2. Green: implement, then re-run the same focused command.
cargo nextest run --test manifest_env_tests 2>&1 | tee /tmp/green-3-14-8.out
#   Summary [   0.9s] 14 tests run: 14 passed

# 3. Confirm no snapshot drifted.
git status --short src/snapshots tests/snapshots
#   (expect no output)

# 4. Full gates, sequentially — never in parallel.
make check-fmt && make typecheck && make lint && make doc-coverage && make test

# 5. Commit.
git add -A && git commit
```

For EP-M4, run `cargo build` before `make test`: the localization audit is a
build-script failure, and finding a missing catalogue entry there is far faster
than through the test suite.

```sh
cargo build 2>&1 | tee /tmp/l10n-3-14-8.out
# A missing catalogue looks like:
#   error: localization audit failed:
#   - missing in ar: stdlib.shell.quote.not_string
#   - missing in cs: stdlib.shell.quote.not_string
```

## Validation and acceptance

Quality criteria — what "done" means:

- **Tests**: `make test` passes. The new suites
  `tests/shell_filter_property_tests.rs` and
  `tests/stdlib_manifest_query_tests.rs` pass, and the five new
  `tests/features/stdlib.feature` scenarios pass through `tests/bdd_tests.rs`.
  `tests/documentation_examples_tests.rs` passes with the new example id.
- **Verification**: OBL-SH-ROUNDTRIP, OBL-PS-ROUNDTRIP, OBL-ONE-WORD,
  OBL-JOIN-SPLIT, OBL-COMPACT, OBL-ENV-DEFAULT, OBL-DIALECT-TOTAL,
  OBL-NINJA-STABLE, and OBL-QUERY-SURFACE are each discharged, with their
  negative controls observed failing at least once and recorded in
  `Artefacts and notes`. AX-2's residual gap on non-Windows hosts is stated in
  ADR-021.
- **Lint and typecheck**: `make check-fmt`, `make typecheck`, `make lint`, and
  `make doc-coverage` all exit zero. `make markdownlint` and `make nixie` pass.
- **Performance**: no benchmark threshold applies.
  `tests/shell_filter_property_tests.rs` must complete within 10 seconds under
  `cargo nextest run`; if it does not, reduce the subprocess property's `cases`
  before considering `#[ignore]`, and escalate if 32 cases is still too slow.
- **Security**: `shell_quote`'s soundness is the security property, and
  OBL-SH-ROUNDTRIP plus OBL-ONE-WORD are its evidence. Confirm that no new
  message text includes an environment variable name or value, matching the
  deliberate omission at `src/manifest/env_reader.rs:83-89`.

Behavioural acceptance, verifiable by hand:

```sh
cd "$(mktemp -d)"
cat > Netsukefile <<'EOF'
netsuke_version: "1.0.0"
vars:
  base_flags: "-D warnings"
targets:
  - name: stamp.txt
    command: >-
      printf '%s\n' "RUSTFLAGS={{ [base_flags, env('RUSTFLAGS', default='')]
        | compact | join(' ') | shell_quote(dialect='sh') }}" > {{ outs }}
defaults:
  - stamp.txt
EOF
netsuke --progress never generate --output build.ninja
grep RUSTFLAGS build.ninja
```

Expect, with `RUSTFLAGS` unset:

```plaintext
  command = printf '%s\n' "RUSTFLAGS=-D' 'warnings" > stamp.txt
```

and with `RUSTFLAGS='-C target-cpu=native'` exported, a single quoted word
containing both the base flags and the override, with no `${...:+...}`
expansion anywhere. Confirm the exact quoted spelling against the `shell-quote`
crate's fragmented output rather than against this transcript; update this
section with the real transcript once observed.

## Idempotence and recovery

Every step is re-runnable. `make` targets are pure checks or in-place
formatters. The only step that mutates tracked files unexpectedly is
`make fmt`, which may reformat Markdown that was already non-canonical; commit
such reformatting separately.

Each milestone is one commit, so `git revert <sha>` restores the previous
plateau. EP-M3 is a pure move, so reverting it cannot change generated output.
No step writes outside the repository except `tee` logs under `/tmp`.

If the localization audit fails mid-edit, the tree still builds once every
catalogue has the key; there is no partial state to clean up.

## Progress

- [x] (2026-09-08) Reconnaissance complete: `env()` implementation and seam,
      registration surfaces, recipe-shell semantics, existing quoting paths,
      localization gate, test and documented-example infrastructure.
- [x] (2026-09-08) Design decisions D1-D7 recorded; D1 and D2 confirmed by the
      requester.
- [x] (2026-09-08) ExecPlan drafted.
- [ ] Community-of-experts review applied.
- [ ] Plan approved by the requester.
- [ ] EP-M1 `env(name, default=...)`.
- [ ] EP-M2 `compact`.
- [ ] EP-M3 shared recipe-shell quoting seam.
- [ ] EP-M4 `shell_quote` and `shell_join`.
- [ ] EP-M5 inject the resolved recipe shell.
- [ ] EP-M6 documentation, ADR-021, roadmap tick.

## Surprises & discoveries

- Observation: Netsuke runs Windows recipes under Windows PowerShell, not
  `cmd.exe`, and `src/ir/cmd_interpolate/substitution.rs` already implements a
  second quoting dialect for it. Evidence: `src/recipe_shell.rs:19-26`,
  `src/ninja_gen_recipe_shell.rs:16-17`, `docs/users-guide.md:330-348`,
  `src/ir/cmd_interpolate/substitution.rs:132-151`. Impact: invalidates
  `RFC-0006-8.9`'s premise that `sh` is the only dialect Netsuke can quote for;
  drives D2 and the extra EP-M5 milestone.
- Observation: `src/stdlib/command/quote.rs` produces `cmd.exe` quoting on
  Windows, but it is never exposed to templates — it supports the `shell` and
  `grep` filters, which spawn a process through the platform shell. Evidence:
  `src/stdlib/command/mod.rs:81-111` registers only `shell` and `grep`;
  `format_command` in `src/stdlib/command/filters.rs:126-149` is the only
  consumer. Impact: it is the wrong quoter to reuse for recipe text, despite
  its name.
- Observation: a new message key must be added to all 35 catalogues or
  `cargo build` fails through `build.rs:291`. Evidence:
  `build_l10n_audit/compare.rs:159-172`;
  `tests/build_l10n_audit_tests.rs:136-147`;
  `docs/developers-guide.md:285-302`. Impact: dominates the mechanical effort;
  drives R1 and the "run `cargo build` first" step in EP-M4.
- Observation: there is no parity test between `register_with_config` and
  `register_manifest_query`; the disabled `env` stub's arity is maintained by
  hand. Evidence: `src/stdlib/register.rs:173-176`; no enumeration test found.
  Impact: drives R8 and OBL-QUERY-SURFACE. A general parity test is worth a
  future roadmap item but is out of scope here.
- Observation: fenced examples in `README.md`, `docs/users-guide.md`, and
  `docs/stdlib-yaml-and-jinja-guide.md` are executed, and their identifiers are
  pinned by a hand-maintained registry. Evidence:
  `tests/documentation_examples/mod.rs:15-21`;
  `tests/documentation_examples_tests.rs:18-61,143`. Impact: the `RUSTFLAGS`
  documentation is real acceptance evidence, not prose.

## Outcomes & retrospective

To be completed at EP-M6. Before setting this plan to `COMPLETE`, reconcile
every discovery against the `Conformance basis`:

- D2 is a deviation from `RFC-0006-8.9`. It must be recorded in ADR-021 and the
  RFC amended, or the plan stays `BLOCKED`.
- `RM-6.8.3` is materially reduced by D1 and D2. Record the reduction as a note
  on that roadmap entry; do not tick it, because its `dialect` value set is
  wider than what ships here.
- If EP-M5's plumbing proves larger than tolerance 1 allows, stop, record the
  measurement, and propose splitting it into its own roadmap item rather than
  shipping the host-default-only behaviour silently.

## Artefacts and notes

To be filled during implementation. Required entries:

1. The `red` transcript for EP-M1 showing the unknown-keyword failure.
2. The name of the snapshot that failed during the OBL-NINJA-STABLE
   non-vacuity check, and the transcript showing it passing again after revert.
3. The transcript of each negative control failing as designed
   (naive double-quote quoter, naive `join(" ")`, naive truthiness `compact`,
   POSIX-quoted input fed to the PowerShell decoder).
4. The real `shell-quote` output for the witness `a b '$HOME'`, used to correct
   the BDD expectations and the "Validation and acceptance" transcript.
5. `git status --short src/snapshots tests/snapshots` showing no output at each
   milestone boundary.

## Revision note

- 2026-09-08: initial draft. Scope, milestones, and verification obligations
  derived from `RM-3.14.8`, `DD-4.4`, `DD-4.5`, `DD-2.6`, and `RFC-0006-8.9`
  after full reconnaissance of the template-helper registration surfaces, the
  recipe-shell contract, the localization gate, and the documented-example
  machinery. Decisions D1 and D2 were confirmed by the requester before
  drafting; D2 is a proposed deviation from `RFC-0006-8.9` and is the principal
  item requiring approval. Remaining work is unchanged: the plan awaits
  approval before any implementation begins.
