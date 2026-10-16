# Add structured recipe environment mappings (3.14.9)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: DRAFT

The plan awaits approval, including the approver's choice under decision AD-1.
Approval does not by itself authorize code: milestone EP-M0 is an entry gate
that holds implementation until roadmap item 15.1.1 is resolved, unless the
approver chooses option B of AD-1. The single exception is the pre-gate fix
PF-1, which repairs a defect in shipped code and may land at once.

There is no `PLANS.md` in this repository; this file is the whole plan. The
`Status:` line above is parsed by `tests/execplan_status_contract_tests.rs` and
must stay one bare value from the closed set `DRAFT`, `APPROVED`, `IN PROGRESS`,
`BLOCKED`, and `COMPLETE`.

## Purpose / big picture

A Netsuke manifest describes a build as YAML with Jinja templates, and Netsuke
turns it into a Ninja build file. Today a manifest author who needs a recipe to
run with a particular environment variable must write shell code inside the
recipe: `RUSTFLAGS='-D warnings' cargo build` on POSIX, or
`$env:RUSTFLAGS = '-D warnings'; cargo build` under Windows PowerShell. Adding
a directory to `PATH` is worse: `PATH="/opt/tool/bin${PATH:+:$PATH}"` is
correct only for a POSIX shell, is easy to get subtly wrong (an empty `PATH`
entry means "the current directory"), and has to be written again, differently,
for Windows.

After this change a rule, a target, or an action may carry an `env` mapping:

```yaml
netsuke_version: "1.0.0"
rules:
  - name: test
    command: cargo nextest run
    env:
      CARGO_TERM_COLOR: always
      PATH:
        prepend: "{{ tool_bin }}"
targets:
  - name: test-report.xml
    rule: test
    env:
      RUSTFLAGS: -D warnings
      CARGO_HOME:
        default: .cargo-home
      CI:
        unset: true
```

When Ninja runs that target, the recipe sees `CARGO_TERM_COLOR=always`,
`RUSTFLAGS=-D warnings`, `CARGO_HOME` set to `.cargo-home` only if the caller
had not already set it, `CI` removed, and `PATH` with the rendered `tool_bin`
directory placed in front of whatever `PATH` the build inherited, joined with
`:` under the POSIX recipe shell and `;` under Windows PowerShell. The author
writes no shell syntax and no Ninja syntax to get any of this.

Observable success:

1. `netsuke build` on Linux, for a manifest like the one above, runs a recipe
   that writes its environment to a file, and the file shows exactly the five
   effects listed above while every other inherited variable is unchanged.
2. The same manifest run in Windows CI under the default PowerShell recipe
   shell shows the same effects, with `;` as the `PATH` separator.
3. `netsuke manifest -` prints a `build.ninja` whose command for that target
   contains a shell-specific environment prologue and no inherited environment
   value; running it under two environments that differ on every bound name
   prints byte-identical output.
4. Every existing manifest without `env` produces a byte-identical
   `build.ninja` and identical action hashes, so the existing Ninja snapshot
   suite passes unchanged.
5. Malformed mappings fail before Ninja starts, with a localized diagnostic
   naming the rule or target and the variable: `{}` and
   `{ value: a, default: b }` are rejected, as are an invalid or reserved
   variable name, a path entry containing the separator, a non-string value, and
   `unset: false`. No diagnostic echoes a value.

## Context and orientation

This section explains the parts of the repository that the work touches. Read
it before anything else. Every path and premise here was true at `56c58027`;
EP-M0 re-verifies them.

### The pipeline from manifest to Ninja

`src/manifest/mod.rs` (`evaluate_manifest`) parses YAML into a JSON value,
expands `foreach` and `when` on `targets` and `actions`
(`src/manifest/expand/`), deserializes the result into the typed abstract
syntax tree (AST) in `src/ast/` with `serde_json::from_value`, and then renders
Jinja templates in string fields (`src/manifest/render/mod.rs`). `render_rule`
renders a rule's `description` and recipe; `render_target` renders a target's
`vars`, `description`, `name`, `sources`, `deps`, `order_only_deps`, and
recipe. A `RenderMode::ManifestQuery` mode renders discovery metadata only and
skips recipe bodies. `serde_json::from_value` errors carry no manifest path, so
they cannot name the rule or target at fault; semantic checks therefore belong
in a typed validation pass after deserialization.

The AST types are `Rule` (`src/ast/mod.rs`, derives `Clone`), `Target`
(`src/ast/target.rs`, not `Clone`), and `Recipe` (`src/ast/mod.rs`), all with
`#[serde(deny_unknown_fields)]` and all public with public fields. Actions are
not a separate type: the top-level `actions` list deserializes into `Target`
values with `phony` forced on (`deserialize_actions` in `src/ast/mod.rs`). An
`env` field on `Target` therefore serves targets and actions alike.

The intermediate representation (IR) lives in `src/ir/`. `BuildGraph`,
`Action`, and `BuildEdge` are public types in `src/ir/graph/mod.rs`. An
`Action` holds `recipe`, `description`, `depfile`, `deps_format`, `pool`, and
`restat`, and derives `Serialize` only. `src/ir/from_manifest/mod.rs` lowers
the AST: `process_rules` keeps each rule as an `Arc<Rule>`, and
`process_targets` walks `manifest.actions.iter().chain(&manifest.targets)`. For
a target that references a rule, it clones the rule's recipe, interpolates the
`{{ ins }}` and `{{ outs }}` placeholders for that target, and calls
`register_action` in `src/ir/from_manifest/support/mod.rs`. `ActionBindings` in
that module carries only the `ins` and `outs` views and the shell.
`register_action` builds the `Action`, hashes it with `ActionHasher::hash`
(`src/hasher.rs`, SHA-256 of canonical JSON of the whole serialized `Action`),
and inserts it only when the hash is new. The hash becomes the Ninja rule name.
Target-level data such as `deps` lives on the `BuildEdge`; data that changes
the command must live on the `Action`, because the command belongs to the Ninja
rule. There are 46 `Action { .. }` literals in 22 files, including public
doctests in `src/hasher.rs` and `src/ir/mod.rs` and the `Action` listing in
`docs/netsuke-design.md`.

The Ninja backend is `src/ninja_gen/`. `write_action_rules` and
`NamedAction::write_into` (`src/ninja_gen/mod.rs`) write one `rule <hash>`
block per action. `NamedAction::shell_text` builds the completed shell text: a
scalar command as written, a POSIX command list as brace groups joined by `&&`
(`src/ninja_gen/command_list/`), and a POSIX script as
`/bin/sh -e -c "printf %b '…' | /bin/sh -e"`. `RecipeShell::command_value`
(`src/ninja_gen/recipe_shell.rs`) turns that text into a Ninja binding: for
`Posix` it doubles every `$` through `escape_ninja_value`
(`src/ninja_gen/escape.rs`, roadmap 3.14.7, ADR-014); for `PowerShell` it
prefixes `$ErrorActionPreference = 'Stop'` and exit-code handling, then
Base64-encodes the script as UTF-16LE for `powershell.exe -EncodedCommand`
(`NinjaValue::from_encoded`, so no `$` reaches Ninja), falling back to a
response file for long scripts; for `Bash` it emits
`bash.exe -e -c "<windows-argument-quoted text>"`. The dyndep emitter in
`src/ninja_gen/dyndep/` reuses `write_action_rules`, so an action-level change
reaches both emission paths.

### Which shell runs a recipe

`RecipeShell` (`src/recipe_shell.rs`, a data-only module documented as lying
below both IR lowering and Ninja rendering) has three variants. `Posix` is the
default everywhere except Windows: Ninja itself passes the command to
`/bin/sh -c`. `PowerShell` is the Windows default (Windows PowerShell 5.1).
`Bash` is an explicitly selected Windows compatibility runtime (Git Bash or
MSYS2), chosen through `NETSUKE_WINDOWS_SHELL` and resolved in
`src/runner/recipe_shell/mod.rs`. Adapters extend `RecipeShell` from their own
modules (for example `impl RecipeShell` in `src/ninja_gen/recipe_shell.rs`).
`BuildGraph::from_manifest_for_shell` and `generate_with_shell` take the shell
as separate parameters, so Windows output is testable on Linux, and so a graph
lowered for one shell could today be emitted for another.

### Quoting and escaping seams that already exist

`src/shell_word.rs` (roadmap 3.14.8, ADR-041) is the one sanctioned word quoter:
`quote_word(dialect, value)` returns one shell word, and
`is_recipe_admissible(value)` rejects newline, carriage return, and NUL. The
Ninja escape is applied once, to completed shell text, by the type boundary
`ShellText` to `NinjaValue`. This plan relies on both: values are quoted by
`quote_word` when the prologue is built, and the prologue plus recipe is
escaped for Ninja once, afterwards (or Base64-encoded under PowerShell). That
ordering is why 3.14.7 and 3.14.8 are prerequisites.

The PowerShell arm of `quote_word` doubles only the ASCII apostrophe
(`value.replace('\'', "''")`). PowerShell's tokenizer also treats U+2018,
U+2019, U+201A, and U+201B as single-quote characters, so a value containing
one of them closes the string early. This is a defect in shipped code (it
already affects `shell_quote(dialect='powershell')`), fixed by PF-1.

`src/ninja_gen/command_list/mod.rs` reserves shell variables named `_netsuke_*`
for status propagation, and ADR-034 leaves `$in` and `$out` in script recipes
to the shell.

### Environment seams and the `env()` helper

ADR-008 (`docs/adr-008-environment-seam-taxonomy.md`) forbids in-process
environment mutation in tests and requires injected environment readers in
production. The Jinja `env()` helper (`src/manifest/env/reader.rs`) reads the
process environment through an `EnvReader` while the manifest is evaluated,
under the access policy of ADR-026. The manifest `env` mapping in this plan is
a different thing: it describes the environment of a recipe's child process
when Ninja runs it, and Netsuke never reads the variables it names. This plan
adds no environment read at all.

`src/runner/process/environment.rs` holds `CommandEnv`, the override set
Netsuke applies when it spawns Ninja itself, and a host-gated `env_names_eq`;
`src/stdlib/which/env/path_support.rs` splits `PATH` for the `which` resolver;
`src/runner/process/redaction.rs` redacts secrets by variable name. None is
reused for recipe environments, because recipe environments are applied by the
recipe shell rather than by Netsuke's own process spawn, and name comparison
must follow the recipe shell rather than the compiling host (D6). EP-M0 repeats
this sweep and records the outcome in the developers' guide.

### What is not there yet

Nothing in `src/` mentions `EnvValue`, `EnvOperation`, or `EnvBinding`; the AST
rejects `env` as an unknown field today. There is no structured command block
(`invoke`), no hidden action runner, and no action plan: RFC 0001's phase 12 is
unimplemented except the ratifying ADR-043. Roadmap 15.1.1 has not produced the
follow-on RFC this item requires. There is no `make proptest` target; property
tests run inside `make test`. Continuous integration (CI) runs tests on Ubuntu
(`.github/workflows/ci.yml`) and on `windows-latest` with Ninja installed
(`.github/workflows/ci-windows.yml`); macOS appears only in release packaging,
so no macOS test lane exists. Verus runs only through `make verus`, outside CI,
and one proof file exists: `verus/canonical_value/main.rs`.

### Terms used in this plan

- *Binding*: one variable name paired with one operation.
- *Operation*: one of `value` (set exactly), `default` (set only when the
  variable is absent), `prepend` or `append` (add one entry to a path list), and
  `unset` (remove the variable).
- *Level*: where a binding is declared. Level 1 is the inherited process
  environment, level 2 a rule, level 3 a target or action, and level 4 an RFC
  0001 structured command block. Higher levels win.
- *Prologue*: the shell statements Netsuke places before a recipe so that the
  recipe runs with the merged bindings applied.
- *Environment dialect*: the per-shell rules that govern bindings: how names
  compare, which names are legal, the path-list separator (if any), and whether
  an empty value is observable.
- *Path-list separator*: the character between entries of a variable such as
  `PATH`; `:` on POSIX systems and `;` on Windows.
- *Model mirror*: a test-only Rust transcription of the Verus specification,
  sharing no code with production, used as the oracle for tests.

## Conformance basis

Upstream artefacts, at repository revision `56c58027` (`origin/main` when this
plan was drafted, 2026-10-16):

- `docs/terms-of-reference.md`: goals G2 (progressive enhancement), G4
  (deterministic plans), G6 (visible impurity), G7 (safe command construction),
  G8 (actionable failures), G9 (platform parity), G10 (replace monster
  Makefiles); section 2.3 (trust and purity); hard constraint "Ninja is the
  execution backend" (section 8.1).
- `docs/roadmap.md` item 3.14.9 and its four sub-items, named here RM-1 (parse
  rule, target, and action `env`), RM-2 (merge rule then target or action
  bindings during IR generation), RM-3 (emit backend-specific environment setup
  without Ninja syntax in the manifest contract), and RM-4 (test platform
  path-list separators). Prerequisites: 3.14.7 (done), 3.14.8 (done), 15.1.1
  (open; see GATE-15.1.1).
- `docs/netsuke-design.md` section 2.6, "Structured environment mapping"
  (DES-ENV-SCHEMA) and "Environment layering and conflicts" (DES-ENV-MERGE).
- `docs/rfcs/0001-structured-command-blocks.md` section 11.1 reserved
  precedence (RFC1-11.1), section 11.2 key and value validation (RFC1-11.2),
  section 11.4 deferral (RFC1-11.4), and section 17.4 runner rules (RFC1-17.4).
- ADR-008 (environment seams), ADR-009 (bounded redacted telemetry), ADR-014
  (backend text escaping), ADR-026 (manifest environment access policy; reads
  only), ADR-034 (script `$in` and `$out`), ADR-035 (semantic compiler
  boundaries; proposed), ADR-036 (layered architecture enforcement; proposed),
  ADR-041 (canonical recipe shell quoting surface), ADR-043 (consolidated
  structured-command execution contract).
- GATE-15.1.1: roadmap 15.1.1, "Decide whether enclosing and object-valued
  environments are needed". Its success condition is "a follow-on RFC defines
  ordered merges and portable path-list operations, or a recorded rejection
  leaves 3.14.9 deferred". It requires 14.3.3, which requires 14.3.1, which
  exercises structured-command overlay shadowing; so under option A of AD-1 the
  RFC 0001 runner and roadmap 12.2.2 will exist before this work starts. No
  such RFC exists at `56c58027`.

Trace links:

```plaintext
TOR G2 G9 G10 -> RM-1 -> DES-ENV-SCHEMA -> EP-M4 -> recipe_env.feature, T-PARSE, T-DIAG
TOR G4 G6     -> RM-2 -> DES-ENV-MERGE, RFC1-11.1 -> EP-M3, EP-M4 -> V-MERGE, V-ADMIT, P-GEN-PURE
TOR G7 G9     -> RM-3 -> ADR-014, ADR-041 -> PF-1, EP-M5 -> P-SHELL-REFINES, snapshots
TOR G9        -> RM-4 -> DES-ENV-SCHEMA -> EP-M3, EP-M5 -> V-PATHLIST, separator matrix
RFC1-11.1     -> EP-M6 (option A only) -> V-LAYER
TOR G8        -> all -> EP-M7 -> users' guide, ADR, roadmap tick
```

Gaps between the design and this plan, each recorded as a decision rather than
silently filled: the design does not say how bindings take effect under Ninja
(D1), how a failing binding behaves (D2), what "absent" means for `default`
(D3), what happens to an empty inherited path list (D4), which names are legal
(D5), how Windows case-insensitivity interacts with merging (D6), what the
`Bash` runtime does (D7), what an empty value means under PowerShell (D8), or
how values are typed and rendered (D9). The design's example still uses the
superseded `exec.program` syntax, and its `Action` listing will lack `env`;
EP-M7 corrects both.

## Constraints

These are hard invariants. Violating one is a stop-and-escalate event.

1. GATE-15.1.1 holds unless the approver has chosen option B of AD-1 in
   writing. If 15.1.1's follow-on RFC defines semantics that differ from this
   plan (for example ordered composition of a rule `prepend` with a target
   `append`), the RFC wins and this plan is revised and re-approved before code
   is written.
2. No Ninja syntax in the manifest contract and none in the IR. Values in the
   manifest are literal text after Jinja rendering; the IR holds plain
   bindings; only `src/ninja_gen/` doubles dollars (ADR-014).
3. Generation reads no environment. Lowering and emission are pure functions of
   the manifest and the selected `RecipeShell`. A `prepend`, `append`, or
   `default` is resolved by the recipe shell at run time, never by Netsuke at
   generation time, so `build.ninja` never contains an inherited value (G4,
   G6). The new domain module takes no `EnvReader`, `mockable::Env`, or
   environment snapshot in any signature.
4. A manifest without `env` produces byte-identical Ninja output and identical
   action hashes. `Action.env` is skipped in serialization when empty.
5. Every shell word Netsuke writes for a value or a path entry goes through
   `shell_word::quote_word`. No second quoter is introduced (ADR-041). Before
   any PowerShell prologue is emitted, PF-1 has made the PowerShell arm double
   all five single-quote code points (U+0027 and U+2018 to U+201B).
6. A variable name is spliced into shell syntax unquoted, so names are
   validated against a grammar that needs no quoting in any supported shell,
   and against a reserved-name list (D5). No name that fails validation reaches
   the backend.
7. Every plateau either honours `env` or rejects it with a typed error. No
   commit may parse `env` and silently ignore it.
8. Tests never mutate the harness process environment (`AGENTS.md`,
   ADR-008). Shell-level and end-to-end tests observe environments in child
   processes configured with `env_clear` and `Command::env`.
9. Diagnostics, `Debug` output, tracing, and metrics never carry a binding
   value, and tracing and metrics never carry a binding name (ADR-009). The
   action hash is a digest over values, so it is not logged beside binding
   events either.
10. Files stay under 400 lines; every module starts with `//!`; every function
    carries `///` docs; public items carry doctests where useful;
    `make doc-coverage` stays at or above 80%. Files already near the limit
    (`src/manifest/render/mod.rs` 397 lines, `src/localization/keys.rs` 396,
    `src/ninja_gen/mod.rs` 386, `src/ninja_gen/recipe_shell.rs` 374,
    `src/ir/from_manifest/support/mod.rs` 370) are split in a separate,
    behaviour-preserving commit before a feature commit touches them.
11. No compatibility shims: the AST, IR, and backend change together, and every
    `Action { .. }` literal is updated in the same commit.
12. Verus work stays outside Cargo, under `verus/`, uses no `assume`, `admit`,
    `external_body`, or added axiom, and is run with `make verus`.
13. Every commit leaves `make check-fmt`, `make lint`, and `make test` green,
    and also `make markdownlint` and `make nixie` when it touches Markdown.

## Tolerances (exception triggers)

Stop, record the situation in `Decision log`, set `Status: BLOCKED`, and ask
for direction when any of these occurs:

- At EP-M0, 15.1.1 has no merged RFC authorizing this surface and AD-1 is not
  option B.
- The 15.1.1 RFC, or any ADR accepted after `56c58027`, contradicts decisions
  D1 to D9.
- A named seam no longer exists on `origin/main` in the form described in
  `Context and orientation`, and the replacement changes a milestone's design
  rather than merely its path.
- A manifest without `env` changes any existing snapshot, action hash, or
  generated byte.
- P-SHELL-REFINES cannot be made to pass on every POSIX shell available to
  Linux CI (`dash` as `/bin/sh` and `bash --posix`), or on the PowerShell and
  Bash legs in Windows CI.
- Windows CI behaviour differs from AXIOM-4, AXIOM-5, or AXIOM-7 in a way that
  changes the user-visible semantics.
- A milestone's diff exceeds roughly 1,200 changed lines excluding snapshots
  and locale catalogues, or the work needs a new runtime dependency.
- `make verus` cannot discover a second proof crate without changing
  `rust-prover-tools` itself.
- A required gate fails three times on the same milestone for different
  reasons, or compile admission (`build-limits`) refuses work; the latter is
  reported, never worked around.

## Risks

- R1, gate never clears. 15.1.1 needs 14.3.3, the end of the structured-command
  programme, and may end in a recorded rejection. Mitigation: the gate is
  explicit (EP-M0); AD-1 offers option B; PF-1 is separated so the security fix
  is not held hostage.
- R2, plan staleness. Months may pass before implementation, and named seams
  and their justifications go stale. Mitigation: EP-M0 re-verifies every named
  path and premise against `origin/main` and revises this plan before code.
- R3, PowerShell empty values. In Windows PowerShell 5.1, assigning `''` to
  `$env:NAME` removes the variable; PowerShell 7.5 and later can store an empty
  value. Mitigation: AXIOM-4, the `empty_is_absent` dialect flag, D8, a Windows
  CI observation, and a note that any future `pwsh` recipe shell reopens D8.
- R4, MSYS2 environment handling under `Bash`. MSYS2 converts `PATH` at process
  boundaries, converts other path lists heuristically (`MSYS2_ENV_CONV_EXCL`),
  keeps shell variables case-sensitive, and passes both `Path` and `PATH` to
  native children when both exist. Mitigation: D7 restricts `Bash` to
  upper-case names and rejects path-list operations; a Windows CI `Bash` leg
  observes the remaining operations; if that leg cannot be built, D7's fallback
  rejects all `env` under `Bash`.
- R5, failing bindings. Under bash-as-`sh`, `unset UID` returns non-zero but the
  shell continues; dash exits. Mitigation: D2's join, `{ prologue; } || exit`,
  stops the recipe in both, and D5 rejects the known read-only names.
- R6, rebuild semantics surprise users. Ninja reruns an edge when its command
  changes, so editing an `env` mapping rebuilds affected targets (desired), but
  a change to the inherited `PATH` does not (Ninja tracks no environment).
  Mitigation: state both in the users' guide.
- R7, hash churn. Mitigation: constraint 4 and P-HASH-STABLE.
- R8, Verus registration unknown. How `rust-prover-tools verus run` discovers
  proofs is not documented in this tree. Mitigation: EP-M0 checks it; tolerance
  if it needs tool changes.
- R9, command-length limits. Linux caps one `sh -c` argument at 128 KiB
  (`MAX_ARG_STRLEN`); Windows caps a variable at 32,767 characters, and
  PowerShell's encoded command has its own budget. Mitigation: the existing
  PowerShell size check and response-file fallback count the prologue; tests
  near the boundary; users' guide states the limits.
- R10, secrets in plain text. Values are written verbatim into `build.ninja`,
  into `netsuke manifest -` output, and into Ninja's `FAILED:` line and its
  `-v`, `-t commands`, and `-t compdb` output. Base64 under PowerShell is
  encoding, not secrecy. Mitigation: the threat model, constraint 9, and
  users'-guide guidance never to render a credential into a value.
- R11, macOS is not tested in CI. `/bin/sh` on macOS is bash 3.2.
  Mitigation: run P-SHELL-REFINES under `bash --posix` on Linux CI, run it on
  macOS locally where available, and record macOS as a residual gap.

## Decision log

Decisions D1 to D9 and AD-1 are proposals for the approver. Each records the
alternatives considered. The panel review of 2026-10-16 (see
`Surprises & discoveries`) revised D2, D5, D6, D7, and D8.

- AD-1 (approver decision, open). How to treat GATE-15.1.1.
  - Option A (recommended): hold implementation until 15.1.1 resolves with an
    RFC that authorizes this surface. Path: `DRAFT` → approval → EP-M0 sets
    `BLOCKED` until the RFC merges → plan revised against the RFC and
    re-approved → `IN PROGRESS`. Because 15.1.1 requires 14.3.3, the RFC 0001
    runner exists by then, so EP-M6 (runner integration, V-LAYER) is
    mandatory under option A.
  - Option B: accept a deviation in which this plan's ADR stands in for
    15.1.1's decision for levels 2 and 3 only. Path: `DRAFT` → `APPROVED` →
    `IN PROGRESS` at EP-M0. EP-M6 and V-LAYER are out of scope and recorded as
    remaining gaps; level-4 composition and ordered cross-level operations stay
    with 15.1.1. The same change amends `docs/roadmap.md` 3.14.9 (its
    requirement and its "deferred by RFC 0001 §11.4" note), 15.1.1, the
    historical traceability bullet that says these operations are deferred
    behind 15.1.1, and RFC 0001 section 11.4.
  - Rationale for A: the roadmap and RFC 0001 section 11.4 defer these
    operations deliberately and call for canary evidence, and the requester
    listed 15.1.1 as a requirement. B delivers the ergonomics sooner at the
    cost of fixing merge semantics before that evidence exists.
- D1, transport: a shell prologue. Bindings take effect through statements in
  the recipe shell's own language, placed before the recipe inside the single
  command Ninja runs. Alternatives: (a) resolve operations at generation time
  from Netsuke's environment and emit exact values; rejected because it writes
  inherited values such as the whole `PATH` into `build.ninja`, makes the plan
  depend on the environment (G4, G6), and is wrong for `netsuke generate`
  output run later. (b) Wrap each recipe in a hidden Netsuke subcommand that
  applies bindings through process APIs, as CMake's `cmake -E env --modify` and
  Meson's serialized executable wrapper do; rejected for legacy recipes because
  Ninja hands POSIX commands to `sh -c` and Windows commands to
  `CreateProcess`, so the wrapper would have to re-create each shell's parsing,
  and RFC 0001 section 17.4 keeps purely legacy recipes on direct Ninja command
  text. (c) Ninja build-level variable bindings; rejected because Ninja
  variables are not process environment variables. A structured command block
  run by the RFC 0001 action runner instead applies the same IR bindings
  through process APIs (EP-M6).
- D2, prologue placement, shape, and failure. The prologue is prefixed to the
  completed `ShellText` in `NamedAction::shell_text`, before
  `RecipeShell::command_value`, for every shell. Under `Posix` and `Bash` the
  form is `{ s1 && s2 && …; } || exit; BODY`: a failing statement exits the
  recipe shell with that statement's status, and the body follows after `;` so
  that a body containing `&`, `||`, `;`, or a trailing `# comment` keeps its
  meaning and still runs in the same shell with the bindings applied. (The
  first draft's `prologue && body` was wrong: `P && a & b` runs `b` without the
  bindings, and `P && a || b` runs `b` when `P` fails.) Under `Bash` the
  prologue is inside the `bash.exe -e -c` argument, because no outer shell runs
  on Windows. For a POSIX `script:` recipe the prologue sits outside the
  `printf %b` payload, so `%b` never reinterprets backslashes in a value; the
  inner `/bin/sh -e` inherits the exported variables. Under `PowerShell` the
  prologue is newline-joined before the body and lands after the existing
  `$ErrorActionPreference = 'Stop'` line without editing `power_shell_script`,
  so a failing statement terminates the script. POSIX statements use assignment
  followed by `export`, never `export NAME=value`, so no shell field-splits an
  assignment.
- D3, `default` means absent, not empty. `default` sets the variable only when
  it is not present; a present but empty variable is kept. This matches
  `env(name, default=...)` from 3.14.8 and POSIX `${N+x}`. The asymmetry with
  D4 is deliberate and documented with an example: CI systems often export
  empty variables (GitHub Actions `env: X: ''`), and `default` does not fire
  for them.
- D4, empty inherited path list. `prepend p` on an absent or empty variable
  yields exactly `p`, and likewise for `append`. An empty POSIX `PATH` entry
  means the current directory, so producing `p:` would silently add one.
  Meson's environment object behaves the same way for an unset variable.
  `prepend` and `append` are not idempotent.
- D5, name grammar and reserved names. A name matches `[A-Za-z_][A-Za-z0-9_]*`
  (`[A-Z_][A-Z0-9_]*` under `Bash`, see D7). This is the POSIX portable name
  grammar; every such name is also a valid unbraced `$env:NAME` in PowerShell,
  so names never need quoting, and it excludes exported-function names such as
  `BASH_FUNC_x%%`. Names are literal: keys are not Jinja-rendered. Reserved and
  rejected: `IFS` (it changes how the body itself is split), any name starting
  `_netsuke_` (owned by the command-list wrapper), `in` and `out` (owned by
  script recipes under ADR-034), and bash's read-only or special names `UID`,
  `EUID`, `PPID`, `SHELLOPTS`, `BASHOPTS`, and `BASH_VERSINFO`. Comparison
  against the reserved list uses the dialect's name comparison. Other names
  that alter the recipe shell (`CDPATH`, `ENV`, `BASH_ENV`, `POSIXLY_CORRECT`,
  `PS4`, `OPTIND`, and `PATH` itself, which changes command lookup) are
  permitted and documented. RFC 0001 section 11.2's looser rule suits a
  process-API transport; a future runner may relax this grammar for structured
  blocks only.
- D6, name equivalence follows the recipe shell. Under `PowerShell` names
  compare ASCII-case-insensitively, as Windows environment names do; under
  `Posix` and `Bash` they compare exactly (bash variables are case-sensitive
  even on Windows, and D7's upper-case grammar removes the ambiguity there).
  Within one level, two names equal under the comparison are a manifest error;
  exact duplicates cannot come from YAML, because the JSON map collapses them,
  so that case is reachable only from constructed values and is tested there.
  Across levels, the higher level's binding replaces the lower one and its
  spelling survives. Because D5 restricts names to ASCII, ASCII case folding is
  exact. The comparison is part of the environment dialect, not a
  `cfg!(windows)` test, so both behaviours are tested on any host.
- D7, the `Bash` runtime. Under `RecipeShell::Bash`, names must be upper case,
  so that a binding cannot create a second, differently cased copy of an
  inherited Windows variable (AXIOM-7), and `prepend` and `append` are rejected
  with a diagnostic suggesting `value` or the default PowerShell shell. Inside
  MSYS2 bash, `PATH` is already POSIX-form and colon-separated, so the
  separator check would reject `C:\tools` anyway; the stronger reason for
  rejection is that conversion of other path lists is heuristic. `value`,
  `default`, and `unset` are supported and observed by a Windows CI `Bash` leg.
  Fallback, if that leg cannot be built: reject every `env` binding under
  `Bash`.
- D8, empty values under PowerShell. Accept `value: ""` and `default: ""` on
  every shell. Under `PowerShell`, an empty value is unobservable (AXIOM-4), so
  `value: ""` behaves as `unset` and `default: ""` as a no-op; the dialect flag
  `empty_is_absent` makes this explicit in the model, and the users' guide
  states it. Alternative: reject empty values on Windows; rejected because the
  motivating `RUSTFLAGS` example legitimately renders to `""`.
- D9, values are literal and typed. Deserialization is structural only: each
  `env` entry is kept as a source-shaped value (a string, or a mapping), and
  every semantic check runs in one typed validation pass that knows the owning
  rule or target. A value must be a string after rendering; numbers and
  booleans are rejected with a "quote it" remedy, as 3.14.8 rejects a non-string
  `default=`. `unset` accepts only `true`. `prepend` and `append` take one
  non-empty entry, not a list. Values are rendered in the entry's ordinary
  field context, which has no `ins` or `outs`; using them is a strict-undefined
  render error. Values are not rendered in `RenderMode::ManifestQuery`,
  matching recipe bodies, because they are execution data.

## Threat model

The manifest author is not the attacker: a `Netsukefile` already runs arbitrary
commands (terms of reference section 2.3), so `env` grants nothing a recipe
line cannot. A binding affects one Ninja edge's recipe shell and its children
only; nothing persists to other edges, to Ninja, or to Netsuke. Netsuke reads
no variable at generation time. At run time each `prepend`, `append`, or
`default` reads only its own variable inside the child shell, so the mapping is
not an ADR-026 read port and cannot widen that policy. No operator policy over
`env` writes is offered, because one would be theatre against arbitrary
commands.

The attacker controls a value that a trusted manifest renders into a binding:
the output of `env()`, `glob()`, `fetch()`, file contents, or a branch name.
The defences are that names are validated literals (D5) and never come from
rendering; every value and path entry is exactly one `quote_word` word, so
`$OTHER`, `$(…)`, backticks, and `$env:OTHER` stay literal; PF-1 closes the
PowerShell typographic-quote breakout; Ninja escaping is applied exactly once
under `Posix` and `Bash`, and not at all under PowerShell, whose Base64 payload
contains no `$`; and a failing prologue fails the edge (D2).

Values are not confidential (R10). The guarantee covers what is rendered into a
value, not what the author writes in the recipe body.

## Verification plan

### Abstract model

The specification is independent of shells and of Rust types.

- A dialect `d` supplies `key_d(name)` (the name, or its ASCII upper-case form
  under PowerShell), an optional separator `sep_d` (`:` for `Posix`, `;` for
  `PowerShell`, none for `Bash`), and a flag `empty_is_absent_d` (true only for
  `PowerShell`). In the Verus model, keys are an abstract sort already
  quotiented by `key_d`; that ASCII folding is an equivalence is tested, not
  proved.
- An environment `E` is a finite partial map from keys to string values.
- An operation is `Set(v)`, `Default(v)`, `Prepend(p)`, `Append(p)`, or
  `Unset`. A level is a finite map from keys to operations.
- `apply1(op, cur)`, where `cur` is an optional string: `Set(v)` gives `v`;
  `Default(v)` gives `cur` if present, else `v`; `Prepend(p)` gives `p` if
  `cur` is absent or empty, else `p ++ sep ++ cur`; `Append(p)` gives `p` if
  `cur` is absent or empty, else `cur ++ sep ++ p`; `Unset` gives absent.
- `merge(lo, hi)` is the right-biased union of two levels by key.
- `apply(level, E)` maps every key `k` to `apply1(level(k), E(k))` when `level`
  binds `k`, and to `E(k)` otherwise.
- `split(s, sep)` is total, with `split("") = [""]`; `entries(s)` is the empty
  sequence when `s` is absent or empty, else `split(s, sep)`.
- `obs_d(E)` is what a child process can observe: `E` itself for `Posix` and
  `Bash`, and `E` with empty values removed for `PowerShell`.
- `admissible_d(level)` holds when every key is a D5 name not reserved under
  `d`, every value and path entry is recipe-admissible (no newline, carriage
  return, or NUL), every path entry is non-empty and free of `sep_d`, and
  `sep_d` absent implies no `Prepend` or `Append`.

### Obligations

Each obligation names its method, artefact, command, evidence, and non-vacuity
check.

- V-ADMIT (validation is exact). Validation and lowering succeed for a level
  under dialect `d` if and only if `admissible_d(level)` holds and keys are
  unique under `key_d`. Method: a proptest over raw maps (names from a pool
  including reserved, lower-case, and malformed names; values including
  separators and control characters) comparing the validator's verdict with an
  independent predicate in the model mirror, in both directions; plus `rstest`
  tables naming each rejection. Non-vacuity: the generator is stratified so
  that each rejection reason and the accept case occur in every run.
- V-MERGE (merge is right-biased and canonical). `merge(lo, hi)(k)` is `hi(k)`
  when `hi` binds `k`, else `lo(k)`; the domain is the union; the output is
  key-unique and sorted by `key_d`. Method: a Verus proof that a sorted-vector
  merge walk over folded keys refines `union_prefer_right` (in the style of
  `runtime_run` in `verus/canonical_value/main.rs`); a bounded-exhaustive test
  of the production `merge_levels` against the model mirror over names
  `{a, A, b}` and every operation or absence (six choices per name, so 216 per
  level and 46,656 pairs) under both comparisons; googletest matchers on
  representative cases. Non-vacuity: witness `proof fn`s with overlapping and
  disjoint keys; a seeded left-biased mutant of the test oracle path must make
  the bounded test fail, recorded in `Artefacts and notes`.
- V-ORDER (sequential execution equals the map semantics). Executing the
  bindings of a key-unique level one statement at a time, in key order, gives
  `apply(level, E)`; hence the sorted prologue order is semantics-preserving
  and deterministic. Method: Verus lemma `lemma_sequential_apply_matches_map`
  by induction on the sequence, whose step uses the frame property of a single
  binding; corollary `lemma_permutation_invariant`. Non-vacuity: a two-binding
  witness satisfies the distinct-keys precondition; a sequence with a repeated
  key is shown by a witness to violate the conclusion, and V-ADMIT shows the
  validator rejects such levels.
- V-PATHLIST (path-list shape). Under the admissibility preconditions, for
  `Prepend(p)`: `entries(result) = [p] ++ entries(cur)`; for `Append(p)`:
  `entries(result) = entries(cur) ++ [p]`; the number of empty entries is
  preserved (so none is introduced). This holds even when `cur` is `"a::b"`,
  `":"`, or ends with a separator. Method: Verus lemmas `lemma_prepend_entries`
  and `lemma_append_entries`, inducting on `p` with a front-recursive `split`;
  an `rstest` separator matrix (`:` and `;` crossed with absent, empty,
  one-entry, multi-entry, and empty-entry inherited values). Non-vacuity:
  witnesses `p = "a:b"` (contains the separator) and `p = ""` violate the
  conclusion, demonstrating both preconditions are necessary, and V-ADMIT
  rejects both.
- V-LAYER (levels compose in RFC 0001 order; option A only). The child
  environment of a structured unit is `overlay4(apply(merge(l2, l3), E))`, where
  `overlay4` is the level-4 exact overlay followed by `temp_dir` bindings. A
  companion `lemma_merge_is_not_sequential` exhibits `Prepend(a)` at level 2 and
  `Prepend(b)` at level 3, where replacement gives `b:cur` but sequential
  application would give `b:a:cur`, documenting that merge replaces rather than
  composes so the choice can be checked against the 15.1.1 RFC. Method: Verus.
  Non-vacuity: a key bound at all three levels and a key bound only at level 2.
- P-SHELL-REFINES (each prologue means `apply`, as observed). For every
  admissible level and inherited environment `E` (key-unique under `key_d`),
  running the generated prologue followed by an environment printer in the real
  recipe shell yields `obs_d(apply(level, E))`, compared modulo the shell-owned
  variables `PWD`, `OLDPWD`, `SHLVL`, and `_`. Method:
  - Stratified cases: an `rstest` table over the 15 cells (five operations ×
    absent, empty, and non-empty inherited values), each run through proptest
    varying only value contents, so every cell is exercised in every run.
  - Value alphabet: printable ASCII, spaces, `'`, `"`, `$`, `\`, a trailing
    `\`, `` ` ``, `!`, `*`, `;`, `&`, `~/x`, `a:~b`, `a=b`, U+2018 to U+201B,
    and other non-ASCII scalars. Generated names come from a pool excluding
    shell-special names; `PATH` is tested in a dedicated `rstest` case.
  - Body shapes: each case runs under a scalar body, a command list, and a
    script, the last command of each printing the environment, so a prologue
    applied once per group or lost in a background job is caught. Scalar bodies
    include `a & b`, `a; b`, `a || b`, and `cmd # c`.
  - Shells: on Linux CI, `/bin/sh` (dash on Ubuntu) and `bash --posix`;
    a subset goes through the real Ninja oracle
    (`src/ninja_gen/property_tests/ninja_oracle.rs`, `ninja -t commands`) and
    includes `prepend` and `default`, so `$$` escaping is in the loop. On
    Windows CI, `powershell.exe` and, for `value`, `default`, and `unset`, the
    `Bash` leg, with reduced case counts.
  - Printer: `/usr/bin/env` by absolute path on POSIX (values cannot contain
    newlines, so line parsing at the first `=` is exact); a PowerShell
    enumeration of `Env:` on Windows.
  - Oracle: the model mirror, never production code.
  - Non-vacuity: a test-only alternate renderer that drops the `${N:+…}` guard
    is run through the same harness, and the test asserts that
    `TestRunner::run` returns `Err`; a second test asserts a readonly binding
    under `bash --posix` followed by `false || echo ran` never prints `ran`.
- P-GEN-PURE (generation reads no environment). Structural argument:
  constraint 3 forbids environment-reading parameters in the domain module, and
  the existing environment-access lint applies. Test: a subprocess
  `netsuke manifest -` run twice with environments that differ on every bound
  name and on `PATH`; the outputs are byte-identical. Non-vacuity: the test
  asserts the two environments do differ on every bound name.
- P-HASH-STABLE (no churn). An `Action` with no bindings serializes exactly as
  before. Method: a named unit test asserting a recorded hash for a fixed
  `env`-free action, and the unchanged `tests/snapshots/ninja/*.snap` suite.
  Non-vacuity: a second test asserts that adding one binding changes the hash.
- P-ONCE (the prologue occurs exactly once per command). Method: `insta`
  snapshots for a scalar command, a command list, a script, and a PowerShell
  recipe large enough to take the response-file path, under each shell.
- P-NO-LEAK (values never leak). A sentinel secret value is used in each
  rejection path that involves a value (inadmissible value, separator in a path
  entry, empty path entry). Method: assert the sentinel is absent from rendered
  diagnostics, `Debug` output of `EnvBinding` and `EnvOp` (which redact), and
  captured tracing.
- T-PARSE and T-DIAG (schema and diagnostics). Finite partitions: the five
  operation forms, the scalar shorthand, zero keys, two keys, unknown key,
  non-string scalar, `unset: false`, invalid and reserved names, duplicates
  under the case-insensitive comparison, path entries containing each
  separator, empty path entry, `prepend` under `Bash`, and lower-case names
  under `Bash`. Method: `rstest` tables; `insta` snapshots of rendered
  diagnostics in English; `rstest-bdd` scenarios. These are finite and
  normative; a proof would add nothing.

### Non-trivial axioms

- AXIOM-1. POSIX `sh` semantics for assignment, `export`, `unset`, brace
  groups, `|| exit`, `${N+x}`, and `${N:+word}` (POSIX.1-2024 Shell Command
  Language). Exercised by P-SHELL-REFINES on dash and `bash --posix`; macOS
  bash 3.2 is a residual gap (R11).
- AXIOM-2. Ninja passes a POSIX `command` unchanged after its own `$`
  unescaping to `/bin/sh -c`, and on Windows to `CreateProcess`. From the Ninja
  manual; exercised by the Ninja oracle.
- AXIOM-3. A child process inherits the exported environment of the shell that
  starts it, and the `$env:` changes of a PowerShell session.
- AXIOM-4. In Windows PowerShell 5.1, assigning `''` or `$null` to `$env:NAME`
  removes the variable, and an inherited empty variable is present
  (`$null -eq $env:N` is false) but falsy. Observed in Windows CI.
- AXIOM-5. Windows environment names compare case-insensitively.
- AXIOM-6. `shell_word::quote_word` round-trips every recipe-admissible string
  in its dialect. For `Sh` this rests on `shell-quote` 0.7.2 and 3.14.8's
  tests; for PowerShell it holds only after PF-1, whose regression test covers
  all five single-quote code points.
- AXIOM-7. MSYS2 bash treats shell variables case-sensitively, and passes every
  exported variable, including case variants, to native children. Observed in
  the Windows CI `Bash` leg.

### Relationship between proof and tests

The chain from specification to observed behaviour is:

1. The Verus specification in `verus/recipe_env/main.rs` proves V-MERGE,
   V-ORDER, V-PATHLIST, and (option A) V-LAYER about the model.
2. The model mirror, `src/ir/env/tests/model.rs`, transcribes the Verus `spec`
   functions clause by clause, imports nothing from `ir::env`, and carries a
   cross-reference comment on each function naming its Verus counterpart (and
   vice versa). Its correspondence to the Verus text is established by review;
   hand-written literal expectations in `rstest` tables check the mirror
   independently.
3. The production merge and validator are checked against the mirror by the
   bounded-exhaustive test and by V-ADMIT.
4. The production prologue renderer is checked against the mirror by
   P-SHELL-REFINES on real shells.

`make verus` is outside CI, so drift between the Verus file and the code is
guarded in CI only by steps 2 to 4. There is no production `apply_bindings`
under option B, because nothing in production applies bindings to an
environment value; under option A, EP-M6 adds one to the runner and checks it
against the mirror.

### Where Verus is warranted, and where it is not

The merge and application rules are the contract users rely on to predict their
recipe's environment, and they are the part RFC 0001 section 11.4 says needs
"ordered merge semantics". The algorithmic kernels (the sorted merge walk, the
sequential statement semantics, and path-list splitting) are small, pure, and
unbounded in the number of bindings and the length of values, so a proof over
all inputs is cheap relative to its value.
`docs/formal-verification-methods-in-netsuke.md` says Verus is not the right
first tool for the manifest or command-interpolation layers; this kernel is
neither, being an algebra over maps and strings with no template or shell
content, and the ADR (EP-M1) records that rebuttal. `docs/developers-guide.md`
accepts two Verus kernels today; approving this plan accepts a third, recorded
in EP-M3.

Lemmas that would merely restate a definition (that `union_prefer_right` is
right-biased, or that `apply` leaves unbound keys alone) are not presented as
theorems; they are used as unfolding steps inside the substantive lemmas.

Verus is not used for parsing, diagnostics, name validation, the prologue
renderer, or Ninja emission. Parsing and diagnostics are finite partitions; the
renderer's correctness depends on real shell behaviour, which no Rust proof can
establish. Kani is not used: the risk sits in heap- and string-heavy Rust that
Kani handles poorly, and the bounded-exhaustive merge test gives exhaustive
coverage of the small name space directly. If an existing harness in
`src/ir/from_manifest/verification.rs` reaches the new code,
`tools/kani/proof-scope.toml` is updated so the scope contract stays green.
`cargo-mutants` may be run over `src/ir/env/` and `src/ninja_gen/recipe_env.rs`
as an audit of the suite; it is optional.

### Proof sketch (hypotheses, to be refined during EP-M3)

The model represents a level as a `Map<K, Op>` and as a `Seq<(K, Op)>` with
distinct, sorted keys, related by `to_map`. Expected results:

1. `runtime_merge_refines_union`: a walk over two sorted, key-unique sequences
   that emits the higher level's entry on a tie produces a sorted, key-unique
   sequence whose `to_map` is `union_prefer_right` of the inputs. Induction on
   the combined length, carrying sortedness and the map correspondence as the
   invariant.
2. `lemma_sequential_apply_matches_map`: induction on the sequence length; the
   step unfolds `apply1` for the head key and uses distinctness to show the
   tail never rebinds it.
3. `lemma_permutation_invariant`: two distinct-key sequences with equal
   `to_map` give equal results, from result 2 applied to both.
4. `lemma_prepend_entries` and `lemma_append_entries`: induction on `p`, using
   "`p` contains no `sep`" so that the first split point of `p ++ sep ++ cur`
   lies just after `p`; the empty-entry count follows.
5. `lemma_merge_is_not_sequential` and `lemma_layering` (option A): the first
   by a concrete witness, the second from results 1 and 2, treating the exact
   overlay as a level of `Set` operations.

Result 4 is the most likely to need supporting facts about sequence splitting;
those are left until the verification conditions are visible. Witness
`proof fn`s construct inputs satisfying every `requires` clause and call each
lemma, so no lemma is vacuously true.

## Test strategy

Unit tests use `rstest` fixtures and `#[case]` tables. Assertions on structured
values use `googletest` matchers (for example `elements_are!` over the merged
bindings and `matches_pattern!` on error variants); comparisons of prologue and
Ninja text use `pretty_assertions::assert_eq` so differences are legible.
`insta` snapshots cover the multivariant output: the same manifest emitted under
`Posix`, `PowerShell`, and `Bash`, and the English rendering of each
diagnostic. Property tests use `proptest` with bounded case counts where a case
spawns processes. Behavioural tests use `rstest-bdd`. End-to-end tests drive the
`netsuke` binary through `assert_cmd` with an isolated environment. Kani is
not used (see the verification plan). New top-level test files are checked by
`tests/integration_test_wiring_tests.rs`, so each must be wired as that test
expects.

### Behavioural specification

The feature file is `tests/features/recipe_env.feature`, auto-discovered by the
`scenarios!` call in `tests/bdd_tests.rs`. Steps live in
`tests/bdd/steps/recipe_env.rs`, wired through `tests/bdd/steps/mod.rs`. The
POSIX execution scenarios live in `tests/features_unix/recipe_env.feature`,
registered by the `#[cfg(unix)]` `scenarios!` call in the same file.

```gherkin
Feature: Structured recipe environment mappings

  Scenario: A rule and a target environment are merged per variable
    Given a manifest whose rule sets CARGO_TERM_COLOR to "always" and RUSTFLAGS to "-g"
    And a target using that rule that sets RUSTFLAGS to "-D warnings"
    When the manifest is lowered for the POSIX recipe shell
    Then the target's action binds CARGO_TERM_COLOR to "always"
    And the target's action binds RUSTFLAGS to "-D warnings"

  Scenario: An operation object must contain exactly one operation
    Given a manifest whose target environment maps CC to an object with value and default
    When the manifest is loaded
    Then loading fails with the environment operation count diagnostic for CC

  Scenario: A variable name must be a portable identifier
    Given a manifest whose target environment binds "MY-VAR"
    When the manifest is loaded
    Then loading fails with the invalid environment name diagnostic

  Scenario: Path-list operations use the recipe shell's separator
    Given a manifest whose target prepends "/opt/tool/bin" to PATH
    When Ninja output is generated for the POSIX recipe shell
    Then the target's command prepends with ":"
    When Ninja output is generated for the PowerShell recipe shell
    Then the target's command prepends with ";"

  Scenario: The Bash compatibility shell rejects path-list operations
    Given a manifest whose target appends "C:\tools" to PATH
    When the manifest is lowered for the Bash recipe shell
    Then lowering fails with the unsupported path-list diagnostic for PATH

  Scenario: A manifest without env mappings is unchanged
    Given the minimal example manifest
    When Ninja output is generated for the POSIX recipe shell
    Then the output matches the existing snapshot
```

```gherkin
Feature: Recipe environments take effect at run time

  Scenario: A built target observes its merged environment
    Given a workspace whose target writes its environment to env.txt
    And the target prepends "sentinel-bin" to PATH and unsets CI
    And Netsuke runs with PATH set to the test's Ninja directory and CI "true"
    When the target is built
    Then env.txt shows PATH beginning with "sentinel-bin:"
    And env.txt has no CI entry

  Scenario: A default does not replace a present variable
    Given a workspace whose target sets a default "fallback" for TOOL_MODE
    And Netsuke runs with TOOL_MODE ""
    When the target is built
    Then env.txt shows TOOL_MODE as ""

  Scenario: A value replaces an inherited variable
    Given a workspace whose target sets RUSTFLAGS to "-D warnings"
    And Netsuke runs with RUSTFLAGS "-g"
    When the target is built
    Then env.txt shows RUSTFLAGS as "-D warnings"
```

The end-to-end steps use `test_support::netsuke::run_netsuke_in_with_env`,
which clears the environment and replaces `PATH`; they pass `NETSUKE_NINJA`
explicitly so Netsuke still finds Ninja, and they use sentinel directory names
rather than real system directories.

## Plan of work

The work is a pre-gate fix, an entry gate, and seven milestones. Each milestone
ends in a coherent, validated state with all gates green, and states its
assigned requirements, end state, acceptance evidence, conformance check,
recovery path, and remaining gaps. Red-green-refactor applies throughout: each
milestone first adds the smallest failing test that specifies its behaviour,
records the failure in `Progress`, then makes it pass. Commit after each green
step; a step that changes `Action` or another widely constructed type updates
every caller in the same commit.

### PF-1, PowerShell single-quote class (independent of the gate)

Assigned: constraint 5, AXIOM-6, ADR-041. This repairs shipped behaviour and
may land immediately as its own branch and pull request, whatever AD-1's
outcome.

- Red: add `rstest` cases to the tests in `src/shell_word.rs` for U+2018,
  U+2019, U+201A, and U+201B, asserting each is doubled inside the
  PowerShell-quoted word, and a regression test that
  `shell_quote(dialect='powershell')` on `x’; Write-Output pwned; ’` renders a
  single literal word.
- Green: make the PowerShell arm of `quote_word` double every character in the
  single-quote class.
- End state: the PowerShell quoter round-trips all five code points.
- Acceptance: the new tests pass; `make check-fmt`, `make lint`, `make test`.
- Conformance check: ADR-041 still describes one quoter; amend its text if it
  names only the ASCII apostrophe.
- Recovery: revert the single commit.
- Remaining gaps: none for this plan; a Windows CI execution test is added in
  EP-M5.

### EP-M0, entry gate and seam re-verification (no code)

Assigned: GATE-15.1.1, R1, R2, R8.

1. `git fetch origin`, then `git rev-list --count HEAD..origin/main`; rebase if
   behind.
2. Read roadmap 15.1.1. If it has no merged RFC authorizing this surface and
   AD-1 is not option B, set `Status: BLOCKED` with a sentence beneath it,
   record the date in `Progress`, and stop.
3. If 15.1.1 produced an RFC, read it in full and compare it with D1 to D9 and
   the model. Revise this plan to match, record each change in the revision
   note, and obtain approval of the revision.
4. For every path named in `Context and orientation`, run
   `git ls-tree origin/main <path>` and re-read the function named. Confirm or
   correct the premise, not just the path. Repeat the equivalents sweep:
   `rg -n 'join_paths|split_paths|env_names_eq|PathSeparator|EnvBinding' src`.
5. Confirm whether RFC 0001's runner and its per-unit environment overlay
   (roadmap 12.2.2) exist; under option A they must.
6. Confirm how `make verus` discovers proof crates (R8), and that
   `make install-verus` works.
7. Check whether a manifest schema version gate (roadmap 12.1.2) exists; if it
   does, decide with the approver whether `env` needs a minimum
   `netsuke_version`.
8. Confirm that PF-1 has landed.

- End state: this plan is current against `origin/main` and either `BLOCKED` or
  `IN PROGRESS`.
- Acceptance: the `Progress` entry for EP-M0 lists each check and its result.
- Conformance check: AD-1 option and the RFC (if any) recorded in
  `Conformance basis`.
- Recovery: nothing is changed but this file.
- Remaining gaps: everything below.

### EP-M1, architecture decision record (documentation only)

Assigned: D1 to D9, AD-1 outcome, the third Verus kernel.

1. Allocate an ADR number. Numbers are spent by open branches as well as by
   `main`, so sweep every remote branch and open pull request:

   ```bash
   git fetch --all --prune -q
   for ref in $(git for-each-ref --format='%(refname)' refs/remotes/origin); do
     git ls-tree -r --name-only "$ref" -- docs/ 2>/dev/null \
       | grep -oE 'docs/adr-[0-9]{3}-' | sed 's|docs/adr-||; s|-$||'
   done | sort -u | tail -n 5
   gh pr list --state open --limit 200 --json headRefName,title
   ```

   Take a number above the highest allocation, not merely the lowest gap.
2. Write `docs/adr-NNN-structured-recipe-environment-mappings.md` with status
   `Proposed`, recording D1 to D9 and the Verus rebuttal. Under option A it
   cites and implements the 15.1.1 RFC rather than restating its decisions.
3. Index it in `docs/contents.md`; reference it from
   `docs/netsuke-design.md` section 2.6.

- End state: the decisions are reviewable before any code exists.
- Acceptance: `make check-fmt`, `make markdownlint`, `make nixie`, and
  `make test` (the execplan status contract reads `docs/execplans/`).
- Conformance check: the ADR names DES-ENV-SCHEMA, DES-ENV-MERGE, RFC1-11.1,
  RFC1-11.4, and GATE-15.1.1.
- Recovery: revert the commit.
- Remaining gaps: the ADR moves to `Accepted` in EP-M7.

### EP-M2, preparatory file splits (behaviour-preserving)

Assigned: constraint 10.

Split, in separate commits, each file the later milestones must grow and that
is within roughly 30 lines of the 400-line limit: `src/manifest/render/mod.rs`
(for example moving description and field rendering to a submodule),
`src/localization/keys.rs` (preserving the macro-generated `ALL_KEYS`),
`src/ninja_gen/mod.rs`, `src/ninja_gen/recipe_shell.rs`, and
`src/ir/from_manifest/support/mod.rs`. Use the house layout: a module that
gains children becomes `foo/mod.rs`.

- End state: identical behaviour, every touched file comfortably under 400
  lines.
- Acceptance: unchanged snapshots; `make check-fmt`, `make lint`, `make test`,
  `make doc-coverage`.
- Conformance check: no public item moved without a re-export that existing
  callers already use; `docs/repository-layout.md` updated if a listed path
  moved (then `make markdownlint` and `make nixie`).
- Recovery: revert the split commits.
- Remaining gaps: none.

### EP-M3, domain model, model mirror, and the Verus kernel

Assigned: RM-2 (semantics), RM-4 (separator), V-MERGE, V-ORDER, V-PATHLIST,
V-ADMIT (domain half). No user-visible surface.

- Red: `src/ir/env/tests/` with `model.rs` (the mirror), `merge.rs` (the
  bounded-exhaustive merge test and named cases), and `validate.rs` (V-ADMIT
  tables and proptest). They fail to compile or fail because the types do not
  exist.
- Green:
  1. Add `src/ir/env/mod.rs` with public, documented types: `EnvName`
     (validated newtype, `AsRef<str>`), `EnvText` (recipe-admissible text),
     `PathEntry` (non-empty, admissible, separator checked against a dialect),
     `EnvOp` (`Set(EnvText)`, `Default(EnvText)`, `Prepend(PathEntry)`,
     `Append(PathEntry)`, `Unset`) with a redacting `Debug`, `EnvBinding`,
     `EnvDialect` (`comparison`, `separator: Option<char>`,
     `empty_is_absent`, `upper_case_names`), and `EnvBindings` with private
     fields, constructed only by validation or `merge_levels`, sorted by folded
     key, storing its `EnvDialect` with `#[serde(skip)]`.
  2. Add `impl RecipeShell { pub fn env_dialect(self) -> EnvDialect }` in
     `src/ir/env/mod.rs`, following the adapter-extends-the-type precedent, so
     `src/recipe_shell.rs` stays data-only.
  3. Add `validate_level(raw, dialect) -> Result<EnvBindings, EnvMappingError>`
     and `merge_levels(lower, higher) -> EnvBindings` (both levels must share a
     dialect). `EnvMappingError` is a `thiserror` enum of facts only, for
     example `{ name, kind: EnvErrorKind }`, with no stored message; the owner
     (rule or target) is attached by the caller in EP-M4.
  4. Write `verus/recipe_env/main.rs` with the model and results 1 to 4 of the
     proof sketch, plus witness `proof fn`s. Its module comment states the
     residual gap between model and code, as `verus/canonical_value/main.rs`
     does.
  5. Record the third accepted kernel in `docs/developers-guide.md` and
     `docs/formal-verification-methods-in-netsuke.md`, replacing their "two are
     accepted" sentences.
- End state: a verified, tested domain kernel with no caller.
- Acceptance: the new tests pass, including the 46,656-pair exhaustive test;
  the seeded oracle mutant is shown to fail (recorded in `Artefacts and notes`);
  `make verus` output, captured to `/tmp/verus-netsuke-<branch>.out`, names the
  `recipe_env` proof with a non-zero verified count and zero errors; the
  code-only check
  `rg -n '\b(assume|admit)\s*\(|external_body' verus/recipe_env/` prints
  nothing; `make check-fmt`, `make lint`, `make test`, `make doc-coverage`,
  `make markdownlint`, `make nixie`.
- Conformance check: V-MERGE, V-ORDER, and V-PATHLIST discharged; the domain
  module's signatures take no environment reader (constraint 3).
- Recovery: revert the milestone's commits; the Verus file can be reverted
  alone.
- Remaining gaps: no manifest surface; no emission.

### EP-M4, schema, rendering, validation, and lowering with a fail-closed guard

Assigned: RM-1, RM-2 (lowering), T-PARSE, T-DIAG, V-ADMIT (manifest half),
P-HASH-STABLE, P-NO-LEAK, the first feature file.

- Red: `tests/ast_tests/env.rs`, wired into `tests/ast_tests.rs` with
  `#[path = "ast_tests/env.rs"] mod env;`; lowering cases in
  `tests/ir_from_manifest_tests.rs`; P-HASH-STABLE in `tests/hasher_tests.rs`;
  `insta` snapshots of each English diagnostic; the scenarios in
  `tests/features/recipe_env.feature`.
- Green:
  1. Add `src/ast/env.rs` (or `src/ast/env/mod.rs` if it grows): `EnvMap`, an
     `IndexMap<String, EnvEntry>` (`indexmap` is already a dependency) that is
     `Clone`, `Debug`, `Serialize`, and `Deserialize`, where `EnvEntry` keeps
     the source shape (a string, or a mapping of raw YAML values). The scalar
     shorthand is kept distinguishable only for diagnostics.
  2. Add `#[serde(default)] env: EnvMap` to `Rule` and `Target`; update every
     struct literal.
  3. Render string values (scalar shorthand and the five operation keys) in
     `src/manifest/render/` in `RenderMode::Full` only.
  4. Run one typed validation pass after rendering, beside `validate_recipes`,
     that turns each `EnvMap` into `EnvBindings` under the selected shell's
     dialect, attaching the owner as `EnvOwner::Rule(name)` or
     `EnvOwner::Target(display name)`. Rule levels are validated once in
     `process_rules`, so unused rules are checked and errors are not repeated
     per target. Render each fact to Fluent at the manifest and IR boundary;
     add keys in `src/localization/keys.rs` and every
     `locales/<lang>/messages.ftl` (35 catalogues) following the policy enforced
     by `build_l10n_keys_tests`, `locale_catalogue_tests`, and
     `locale_stub_strictness_tests`.
  5. Add `env: EnvBindings` to `ir::Action` with
     `#[serde(skip_serializing_if = "EnvBindings::is_empty")]`, and update all
     46 literals. In `process_targets`, merge the rule's level with the
     target's and pass the result to `register_action` as its own argument,
     not through `ActionBindings`.
  6. In `src/ninja_gen/`, reject any action with non-empty `env` with a typed
     `NinjaGenError` ("environment mappings are not yet emitted"), so this
     plateau fails closed (constraint 7).
- End state: `env` is parsed, validated, merged, and hashed, and a manifest
  using it fails generation with a clear error.
- Acceptance: the tests above pass; existing snapshots unchanged;
  `make check-fmt`, `make lint`, `make test`, `make doc-coverage`.
- Conformance check: RM-1 and RM-2 discharged; P-HASH-STABLE green; no
  diagnostic echoes a value (P-NO-LEAK).
- Recovery: revert the milestone's commits.
- Remaining gaps: emission (EP-M5).

### EP-M5, backend prologue emission

Assigned: RM-3, RM-4, P-SHELL-REFINES, P-ONCE, P-GEN-PURE, the end-to-end
feature file.

- Red: `insta` snapshots in a new snapshot test for one manifest emitted under
  `Posix`, `PowerShell`, and `Bash`, covering every operation, a command list,
  a script, and a PowerShell response-file recipe; P-SHELL-REFINES in
  `src/ninja_gen/property_tests/recipe_env.rs`; the end-to-end test in
  `tests/recipe_env_e2e_tests.rs` and the scenarios in
  `tests/features_unix/recipe_env.feature`; P-GEN-PURE.
- Green:
  1. Add `src/ninja_gen/recipe_env.rs`, which compiles `EnvBindings` into
     prologue text for a `RecipeShell`, quoting every value and path entry
     with `shell_word::quote_word(shell.dialect(), …)`. Starting forms, with
     `q` the quoted word:

     ```plaintext
     POSIX and Bash, joined as `{ s1 && s2 && …; } || exit; BODY`:
       value:   N=q && export N
       default: { [ -n "${N+x}" ] || { N=q && export N; }; }
       prepend: N=q"${N:+:$N}" && export N
       append:  N="${N:+$N:}"q && export N
       unset:   unset N

     PowerShell, one statement per line before BODY:
       value:   $env:N = q
       default: if ($null -eq $env:N) { $env:N = q }
       prepend: $env:N = if ($env:N) { q + ';' + $env:N } else { q }
       append:  $env:N = if ($env:N) { $env:N + ';' + q } else { q }
       unset:   $env:N = $null
     ```

     The snapshots and P-SHELL-REFINES decide the final text.
  2. Prefix the prologue in `NamedAction::shell_text` as D2 describes, and
     remove EP-M4's guard. Reject an `EnvBindings` whose dialect differs from
     the emission shell's with a typed error, so a graph lowered for `Posix`
     cannot be emitted for `Bash`.
  3. Emit a `tracing::debug!` event per action with the count of bindings per
     operation kind, and no names, values, or action hash.
  4. Add the Windows CI coverage: PowerShell and `Bash` legs of
     P-SHELL-REFINES and a `#[cfg(windows)]` end-to-end twin that confirms
     AXIOM-4 and AXIOM-7. If the `Bash` leg cannot be built, apply D7's
     fallback and record it.
- End state: the feature works end to end on Linux and Windows.
- Acceptance: snapshots reviewed and accepted with `cargo insta review`;
  P-SHELL-REFINES passes on dash and `bash --posix` with the mutant rejected;
  the end-to-end tests pass on Linux and in Windows CI; P-GEN-PURE passes;
  `make check-fmt`, `make lint`, `make test`, `make doc-coverage`.
- Conformance check: RM-3 and RM-4 discharged; no Ninja syntax in the IR; the
  prologue occurs once per command.
- Recovery: revert the milestone's commits; EP-M4's guard returns with them.
- Remaining gaps: structured blocks (EP-M6, option A); macOS (R11).

### EP-M6, structured-command runner integration (option A only)

Assigned: RFC1-11.1, V-LAYER. Under option B, record "out of scope" in
`Progress` and list V-LAYER as a remaining gap.

1. Carry each action's merged level-2 and level-3 bindings into the versioned
   action plan for structured units, bumping the plan schema version as RFC
   0001 section 17.4 requires.
2. Add a production `apply_bindings` in the runner's module, computing the
   child environment as `apply` over the runner's injected inherited
   environment, then the level-4 exact overlay, then `temp_dir` bindings (RFC
   0001 sections 11.1 and 11.3). Bindings apply to the child only, never to the
   runner's own configuration; a bare program is resolved against the effective
   child `PATH`, as roadmap 12.2.2 specifies.
3. Test `apply_bindings` against the model mirror, and observe the result with
   the runner's existing environment-reporting helper executable.
4. Add `lemma_merge_is_not_sequential` and `lemma_layering` to the Verus file.

- End state: a structured block under a rule with `env` observes levels 2 to 4
  in order.
- Acceptance: the tests pass; `make verus` as in EP-M3; `make check-fmt`,
  `make lint`, `make test`, `make doc-coverage`.
- Conformance check: RFC1-11.1 precedence and RFC1-17.4 plan rules hold.
- Recovery: revert the milestone's commits.
- Remaining gaps: whatever the 15.1.1 RFC defers.

### EP-M7, documentation and roadmap

Assigned: G8 documentation, reconciliation.

1. `docs/users-guide.md`: a "Recipe environments" section near "Rules and
   recipes" covering the five operations, the shorthand, the merge rule, the
   name grammar and reserved names, names that alter the recipe shell, the
   separator per shell, the `Bash` restrictions, Windows empty-value behaviour,
   the `default` and `prepend` asymmetry, rebuild behaviour (R6), length limits
   (R9), the difference from the `env()` helper, and the secrets warning:
   values are written into `build.ninja` and printed when a recipe fails, so
   never render a credential into a value; recipes already inherit the
   operator's environment, `unset` removes a variable, and the ADR-026 policy
   governs `env()` reads. Each fenced example carries a `tested-example`
   marker, is added to `EXPECTED_EXAMPLE_IDS`, and is validated by
   `make test-documentation-contracts`.
2. `docs/netsuke-design.md` section 2.6: mark the environment mapping as
   implemented, replace the stale `exec.program` example with a `command`
   example, add `env` to the `Action` listing, and reference the ADR.
3. `docs/developers-guide.md`: where bindings are rendered, validated, merged,
   and emitted; the re-use policy for `ir::env` (the single owner of binding
   semantics; any backend or runner must consume `EnvBindings` and be checked
   against the model mirror rather than re-implement the rules; the prologue
   renderer is the only shell compiler for bindings); why `CommandEnv`,
   `env_names_eq`, and the `which` path support are not reused; and the Verus
   kernel. Confirm the new modules satisfy ADR-036's layering checks if they
   have landed.
4. `docs/repository-layout.md`: the new modules and `verus/recipe_env/`.
5. The ADR: status `Accepted`.
6. `docs/roadmap.md`: tick 3.14.9's sub-items and the item itself, and update
   the 3.14.11 note that names 3.14.9.
7. Reconcile discoveries with every artefact in `Conformance basis`, then set
   `Status: COMPLETE`.

- End state: documentation matches behaviour; the roadmap entry is done.
- Acceptance: `make check-fmt`, `make markdownlint`, `make nixie`,
  `make test-documentation-contracts`, `make test-workflow-contracts` (if a
  workflow changed in EP-M5), `make test`, `make lint`, `make doc-coverage`.
- Conformance check: every trace link has acceptance evidence.
- Recovery: revert the documentation commits.
- Remaining gaps: recorded in `Outcomes & retrospective`.

## Concrete steps

Run every command from the repository root. Capture output with `tee` under
`/tmp`, one gate at a time, never in parallel:

```bash
BRANCH="$(git branch --show-current)"
make check-fmt 2>&1 | tee "/tmp/check-fmt-netsuke-${BRANCH}.out"
make lint 2>&1 | tee "/tmp/lint-netsuke-${BRANCH}.out"
make test 2>&1 | tee "/tmp/test-netsuke-${BRANCH}.out"
make doc-coverage 2>&1 | tee "/tmp/doc-coverage-netsuke-${BRANCH}.out"
make markdownlint 2>&1 | tee "/tmp/markdownlint-netsuke-${BRANCH}.out"
make nixie 2>&1 | tee "/tmp/nixie-netsuke-${BRANCH}.out"
make install-verus 2>&1 | tee "/tmp/install-verus-netsuke-${BRANCH}.out"
make verus 2>&1 | tee "/tmp/verus-netsuke-${BRANCH}.out"
```

`make test` compiles Rust through the shared compile-admission pool; a build
that prints `[build-limits] … waiting for a compile slot` is queued, not hung. A
`build-limits:` refusal is reported to the user, not worked around.

Focused runs during red-green cycles:

```bash
cargo nextest run --workspace --all-targets --all-features -E 'test(recipe_env)'
cargo insta review
```

Expected focused output after EP-M5, abbreviated:

```plaintext
PASS [ 0.4s] netsuke ninja_gen::property_tests::recipe_env::posix_prologue_refines_model
PASS [ 2.1s] netsuke::recipe_env_e2e_tests built_target_observes_merged_environment
```

Commit messages name the milestone, for example
`Merge rule and target env bindings during lowering (3.14.9, EP-M4)`.

## Validation and acceptance

The feature is accepted when all of these hold:

- The scenarios in both feature files pass.
- `netsuke build` of the purpose example on Linux and in Windows CI shows the
  expected effects in the recipe's observed environment.
- `netsuke manifest -` output for that example is byte-identical under two
  environments that differ on every bound name.
- The full existing Ninja snapshot suite passes with no snapshot changes.
- `make verus` verifies `verus/recipe_env/main.rs` with a non-zero verified
  count and zero errors, and the code-only `rg` check finds no `assume`,
  `admit`, or `external_body`.
- `make check-fmt`, `make lint`, `make test`, and `make doc-coverage` pass at
  every milestone; `make markdownlint`, `make nixie`, and
  `make test-documentation-contracts` pass after EP-M7.

## Idempotence and recovery

Every milestone is one or more commits and has its own recovery line above; to
roll back, `git revert` the milestone's commits in reverse order. Snapshot
acceptance is repeatable with `cargo insta review`. The Verus proof is outside
Cargo and can be reverted alone. If a gate fails half-way, fix and re-run that
gate only, then continue the sequence.

## Interfaces and dependencies

No new runtime dependency. The new types are public, because `Action`, `Rule`,
and `Target` are public with public fields and a private type in a public field
fails `private_interfaces` under `-D warnings`. Shapes, all hypotheses to be
refined by the proof and tests:

```rust
// src/ast/env.rs
pub struct EnvMap(IndexMap<String, EnvEntry>);
pub enum EnvEntry { Scalar(String), Object(serde_json::Map<String, serde_json::Value>) }

// src/ir/env/mod.rs
pub struct EnvName(String);
pub struct EnvText(String);
pub struct PathEntry(String);
pub enum NameComparison { Exact, AsciiCaseInsensitive }
pub struct EnvDialect {
    pub comparison: NameComparison,
    pub separator: Option<char>,
    pub empty_is_absent: bool,
    pub upper_case_names: bool,
}
pub enum EnvOp { Set(EnvText), Default(EnvText), Prepend(PathEntry), Append(PathEntry), Unset }
pub struct EnvBinding { name: EnvName, op: EnvOp }
pub struct EnvBindings { bindings: Vec<EnvBinding>, dialect: EnvDialect }
pub enum EnvOwner { Rule(String), Target(String) }
pub struct EnvMappingError { name: String, kind: EnvErrorKind }

impl RecipeShell { pub fn env_dialect(self) -> EnvDialect; }
pub fn validate_level(raw: &EnvMap, dialect: EnvDialect) -> Result<EnvBindings, EnvMappingError>;
pub fn merge_levels(lower: &EnvBindings, higher: &EnvBindings) -> EnvBindings;
```

`ir::Action` gains `pub env: EnvBindings`. Any change to these shapes is
recorded in `Decision log`.

Hexagonal placement: `src/ast/env.rs` is inbound schema and keeps the source
shape; `src/ir/env/` is the domain model and the only owner of binding
semantics, and reads no environment; `src/ninja_gen/recipe_env.rs` is the
outbound adapter that compiles the model into shell text; under option A, the
runner is a second outbound adapter applying the same model through process
APIs. Adapters depend on the domain, never the reverse, and
`src/recipe_shell.rs` stays data-only.

## Approval gate

Approval of this plan covers:

- the choice under AD-1, with its consequences for EP-M0, EP-M6, and the
  roadmap;
- decisions D1 to D9, and the user-visible semantics they imply;
- landing PF-1 now, independently of the gate;
- accepting a third Verus kernel, `verus/recipe_env/main.rs`;
- writing the ADR as `Proposed` in EP-M1, to be accepted in EP-M7.

Silence is not approval. Under option A, approval leads to `Status: BLOCKED` at
EP-M0 until 15.1.1 resolves; under option B, to `APPROVED` and then
`IN PROGRESS`.

## Signposts

Documents: `docs/netsuke-design.md` (sections 2.6 and 5.4),
`docs/terms-of-reference.md`, `docs/rfcs/0001-structured-command-blocks.md`
(sections 11 and 17.4), ADR-008, ADR-009, ADR-014, ADR-026, ADR-034, ADR-035,
ADR-036, ADR-041, ADR-043, `docs/rust-testing-with-rstest-fixtures.md`,
`docs/rust-doctest-dry-guide.md`,
`docs/reliable-testing-in-rust-via-dependency-injection.md`,
`docs/ortho-config-users-guide.md` (no configuration surface is added, so
OrthoConfig is unaffected; consult it only if a CLI or configuration knob is
proposed), `docs/rstest-bdd-users-guide.md`,
`docs/formal-verification-methods-in-netsuke.md`,
`docs/documentation-style-guide.md`, and `docs/developers-guide.md` sections
"Ninja text-escaping seam", "Injected and child-process environments",
"Property-based testing with proptest", and "How to add or update behavioural
tests".

Skills: `execplans`, `rust-router`, `hexagonal-architecture`,
`rust-types-and-apis` (newtypes and API shape), `rust-errors` (typed
diagnostics), `rust-unit-testing`, `proptest`, `verus`, `nextest`,
`en-gb-oxendict-style` for documentation.

Prior art: CMake's `cmake -E env --modify` operations `set`, `unset`,
`path_list_prepend`, and `path_list_append` (CMake 3.25), which choose `:` or
`;` by platform; Meson's `environment()` object, whose `prepend` and `append`
use the host path separator by default and set the bare value when the variable
is unset; the Ninja manual's statement that POSIX commands run under `sh -c`,
where `VAR=value cmd` sets environment variables, and Windows commands run
through `CreateProcess`.

## Progress

- [x] 2026-10-16: Branch renamed to
  `3-14-9-structured-recipe-environment-mappings`.
- [x] 2026-10-16: Reconnaissance of AST, IR, Ninja backend, environment seams,
  verification tooling, and documentation.
- [x] 2026-10-16: Prior-art research (CMake, Meson, Ninja manual).
- [x] 2026-10-16: First draft written.
- [x] 2026-10-16: Five-lens expert review (shell semantics, formal
  verification, architecture, security, governance) and revision.
- [ ] Approval, including AD-1.
- [ ] PF-1.
- [ ] EP-M0 entry gate.
- [ ] EP-M1 to EP-M7.

## Surprises & discoveries

- 2026-10-16: GATE-15.1.1 is open, and it depends on 14.3.3, the end of the
  structured-command programme. Nothing in phase 12 is implemented beyond
  ADR-043. The roadmap note "does not block the exact command-block overlay
  delivered by 12.2.2" describes the reverse dependency, not this one.
- 2026-10-16: `RecipeShell::Bash` is a Windows-only compatibility runtime, so
  bindings under it carry MSYS2 conversion and case hazards (R4, D7).
- 2026-10-16: The design's example in section 2.6 still uses the superseded
  `exec.program` syntax.
- 2026-10-16: The review panel found that `quote_word`'s PowerShell arm
  doubles only the ASCII apostrophe, though PowerShell also closes a
  single-quoted string at U+2018 to U+201B. This affects shipped
  `shell_quote(dialect='powershell')` and became PF-1.
- 2026-10-16: The panel showed by probe that `prologue && body` changes the
  meaning of scalar bodies containing `&`, `||`, or `;`, and that bash in POSIX
  mode continues after a failed `unset` of a read-only variable while dash
  exits. D2 now uses `{ prologue; } || exit; body`.
- 2026-10-16: The first draft's oracle for the shell property was production
  code, which made the chain from proof to behaviour circular; the model mirror
  replaces it. Its V-LAYER statement was false read sequentially; restated.
- 2026-10-16: CI has no macOS test lane, and the spelling gate rejects `AX-n`
  identifiers, so axioms are labelled `AXIOM-n`.

## Outcomes & retrospective

Not started.

## Artefacts and notes

None yet.

## Revision note

2026-10-16: First draft.

2026-10-16: Revised after a five-lens expert review. The gate logic now blocks
under option A and keys EP-M6 to AD-1. Milestones were reordered domain-first,
with a docs-only ADR milestone, preparatory file splits, and a fail-closed
backend guard so no plateau ignores `env`. Added PF-1, a threat model, a test
strategy, an approval gate section, per-milestone fields, the model mirror,
per-shell observation, V-ADMIT, P-GEN-PURE, P-ONCE, and P-NO-LEAK. D2, D5, D6,
D7, and D8 changed as recorded above. Remaining work is unchanged in scope
except that EP-M6 is mandatory under option A.
