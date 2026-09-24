# Add the manifest-facing shell selection vocabulary (11.2.1)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: DRAFT

This plan has been through an expert design review and revision. It awaits user
approval; do not begin implementation until the user explicitly approves it.

## Purpose / big picture

Netsuke is growing a structured-command recipe form (RFC 0001) in which each
command block may say which shell, if any, interprets its `invoke` text.
ADR-019 and RFC 0011 fix the manifest contract: the block's `shell` field is
either a Boolean or a registry name. `false` (or an absent field) means direct
argument-vector (argv) execution with no shell, `true` means the host's
platform-default shell, and a string such as `bash` or `dash` names an
allow-listed shell.

This task adds the vocabulary for that contract as plain data types, and
nothing else. After the change a Netsuke developer can:

1. enumerate the four built-in shells (`sh`, `bash`, `pwsh`, `powershell`) and,
   for either host family (Unix-like or Windows), ask whether each is
   supported, which executable it names and whether that is an absolute path or
   a bare name, which fixed arguments precede the shell source, and which
   built-in `true` selects;
2. parse any string into a validated `ShellName` that accepts exactly the
   ADR-019 grammar `[a-z][a-z0-9_-]{0,62}`, so configured names such as `dash`
   are representable while paths, upper-case text, and templates are not; and
3. decode a YAML or JSON `shell` value into a three-way `ShellSelection`
   (`Direct`, `PlatformDefault`, `Named(ShellName)`), rejecting every value
   that is neither a Boolean nor a valid name.

Nothing that Netsuke executes changes. No manifest accepts a `shell` field yet,
because the structured-command abstract syntax tree (AST) from RFC 0001 does
not exist; wiring happens in roadmap task 11.3.1. Success is observable through
the new unit, property, snapshot, doctest, and behavioural tests. Each new test
is shown to fail against a deliberately wrong stub and to pass against the real
implementation, while every existing test continues to pass unchanged.

Terms used throughout:

- _Built-in shell_: one of the four shells Netsuke itself knows about, listed
  in ADR-019 Table 1. Built-in names are reserved.
- _Configured shell_: a shell added by trusted operator configuration in task
  11.2.2. This task only makes its name representable.
- _Host family_: the coarse operating-system class that decides built-in
  support. There are exactly two: Unix-like and Windows.
- _Fixed arguments_: the interpreter arguments Netsuke places before the
  rendered shell source, for example `-c` for `sh`.
- _Front-end_: Netsuke's manifest loader, `evaluate_manifest` in
  `src/manifest/mod.rs`, which parses YAML into a `serde_json::Value` and later
  deserializes typed AST values from that value.
- _Build-script slice_: the handful of library source files that `build.rs`
  recompiles by `#[path]` to generate manual pages (`build.rs`, lines 30 to
  70). Any file in that slice must compile standalone without unused items.

## Constraints

These are hard invariants. Violating one requires escalation, not a workaround.

- Do not change command execution. Do not modify `src/ast/`, `src/ir/`,
  `src/ninja_gen/`, `src/ninja_gen_recipe_shell.rs`, `src/runner/`,
  `src/manifest/`, `src/cli/`, `src/stdlib/`, `src/recipe_shell.rs`, or
  `build.rs`. The only permitted production edits are the new
  `src/shell_selection/` module and a single `pub mod shell_selection;` line in
  `src/lib.rs`.
- Do not wire the new types into any AST, IR, configuration, or command-line
  surface. Roadmap 11.2.1 says the types stay data-only until RFC 0001's AST
  exists (11.3.1), and `CliConfig` definitions belong to 11.2.2.
  `ShellSelection` must never appear in `src/ir/` or `src/ninja_gen/`, now or
  later: the IR carries only a resolved shell (ADR-019, "Lowering and execution
  intermediate representation").
- Do not read the process environment, the filesystem, or `PATH`. Executable
  resolution, `PATHEXT` normalization, and probing belong to 11.2.3. The only
  ambient input permitted is the compile-time target family (`cfg!(windows)`),
  read in exactly one function.
- Keep `src/shell_selection/name.rs` free of `crate::` and `super::` imports;
  it may import only `std`, `serde`, and `thiserror`, because 11.2.2 will need
  to add it to the build-script slice (Decision D1).
- Preserve ADR-019 exactly: the four built-in names, their supported hosts,
  executables, resolution kinds, fixed arguments, the `true` host mapping (`sh`
  on Unix-like, `powershell` on Windows), and the `ShellName` grammar. Any wish
  to diverge is an architecture deviation (see `Decision log`).
- Add no new dependency. `serde`, `serde_json`, `serde-saphyr`, and
  `thiserror` are existing runtime dependencies; `rstest`, `rstest-bdd`,
  `rstest-bdd-macros`, `googletest`, `pretty_assertions`, `insta`, `proptest`,
  and `regex` are existing dev-dependencies (`Cargo.toml` `[dev-dependencies]`).
- Keep every file at or under 400 lines (AGENTS.md; Whitaker
  `module_max_lines`). Begin every module with a `//!` comment; give every
  function, method, enum variant, and struct or variant field a `///` comment
  (`missing_docs_in_private_items` is denied); put public-item examples in
  doctests.
- No in-process environment mutation in tests (AGENTS.md, ADR-008). This task
  needs no environment at all.
- Use en-GB-oxendict spelling (`-ize`, `-yse`, `-our`) in comments and
  documentation.
- Do not add a `-Z` Polonius or next-solver directive; the pinned toolchain
  enables both.
- Never commit a Red state; Red stubs exist only in the working tree.

## Tolerances (exception triggers)

Measure scope from the commit that records this plan's approval, excluding the
ExecPlan file itself.

- Scope: stop and escalate if implementation needs more than 26 changed files
  or more than 2,600 net added lines including tests, snapshots, and
  documentation, or if any file outside the permitted set in `Constraints`
  needs a production edit.
- Interface: stop and escalate if any existing public item's signature must
  change. Adding the new `pub mod shell_selection` is approved by this plan.
- Dependencies: stop and escalate if a new crate or a new feature on an
  existing crate appears necessary.
- Contract: stop and escalate if ADR-019, RFC 0011, or RFC 0001 appears wrong
  or ambiguous in a way that changes the types (for example, a built-in's
  argument list); record a proposed deviation in `Decision log` and set the
  status to `BLOCKED`.
- Iterations: stop and escalate if a gate still fails after three fix attempts
  for the same root cause. Lint findings listed in the Green checklist below
  are expected and do not count as attempts.
- Time: stop and escalate if a single milestone exceeds four hours of active
  work.
- Localization: stop and escalate if any gate (for example a Fluent audit or a
  Whitaker lint) demands localized text for the new error type; the plan
  deliberately defers localization (Decision D5).
- Behavioural harness: stop and escalate if the repository's first
  `Scenario Outline` cannot be bound by `scenarios!` after the EP-M2 spike.

## Risks

- Risk: YAML 1.1 Boolean spellings. The front-end uses `serde-saphyr` 1.2.0
  with default options, whose untyped path trims the scalar and then maps any
  ASCII case of `true`, `yes`, `y`, `on` to `true` and `false`, `no`, `n`,
  `off` to `false` before any typed deserializer runs. Unquoted `shell: yes`
  therefore means `PlatformDefault`, and `shell: n` means `Direct`. Severity:
  medium. Likelihood: certain. Mitigation: pin the behaviour with front-end
  parity tests, document it in the design document, and hand it to 11.2.2 and
  12.1.1; do not change the front-end here (Decision D7).
- Risk: quoted Boolean-looking names. `shell: "true"` is the string `true`,
  which satisfies the ADR-019 grammar, so it decodes to `Named("true")`. If
  11.2.2 later let an operator define a shell called `on`, an unquoted `on`
  would silently mean the platform default instead. Severity: low now, medium
  from 11.2.2. Likelihood: low. Mitigation: pin the behaviour in tests and hand
  11.2.2 a firm reservation recommendation (Decision D8).
- Risk: the new module drifts from ADR-019 Table 1. Severity: high (the table
  is an authority boundary). Likelihood: low. Mitigation: an exact per-shell
  `rstest` table plus an `insta` snapshot rendering the table from code.
- Risk: a future fifth built-in is added to the enum but not to `ALL`.
  Severity: medium. Likelihood: low. Mitigation: a test compares `ALL` with a
  handwritten list and calls a helper whose exhaustive `match` fails to compile
  (E0004) for an unlisted variant. A fixed array length alone does not enforce
  this.
- Risk: lint waves. The workspace denies `must_use_candidate`,
  `missing_errors_doc`, `missing_const_for_fn`, `missing_docs_in_private_items`,
  `string_slice`, `indexing_slicing`, and `unwrap_used`, and Whitaker limits
  conditionals to two branches; `make lint` stops at the first failing
  sub-gate. Severity: low. Likelihood: high. Mitigation: the Green checklist in
  `Plan of work` pre-empts each known finding.
- Risk: Windows-only failure. `std::path::Path::is_absolute("/bin/sh")` is
  false on Windows, and CI runs `make test` on Windows. Severity: medium.
  Likelihood: medium. Mitigation: shape checks avoid `std::path` (O5).
- Risk: the repository's first `Scenario Outline`. `scenarios!` does not run
  the compile-time step validation that `#[scenario]` does, so a step typo
  surfaces only at run time. Severity: low. Likelihood: medium. Mitigation:
  pinned step patterns and a one-row spike first (EP-M2).
- Risk: build-script slice. Task 11.2.2 must compile `name.rs` inside
  `build.rs`, where any unused inherent method becomes a dead-code error.
  Severity: medium for 11.2.2. Likelihood: medium. Mitigation: `name.rs` stays
  import-free and minimal (Decision D1), and the hand-off names the risk.

## Progress

- [x] (2026-09-24) Renamed the branch to
  `11-2-1-add-the-manifest-facing-shell-selection-vocabulary` and set its
  upstream.
- [x] (2026-09-24) Reconnaissance of ADR-019, RFC 0011, RFC 0001, the roadmap,
  `RecipeShell`, `DependencyOrder`, `HostPattern`, the manifest front-end,
  `build.rs`, and the test infrastructure.
- [x] (2026-09-24) Drafted this ExecPlan.
- [x] (2026-09-24) Expert design review (structure and cost; contracts and
  alternatives; failure modes and viability). All three panels returned
  "proceed with conditions"; every condition is folded into this revision.
- [ ] User approval of the plan.
- [ ] EP-M0: baseline gates recorded.
- [ ] EP-M1: host family, built-in registry, and `ShellName` landed with
  tests.
- [ ] EP-M2: `ShellSelection` codec, parity tests, properties, and behavioural
  scenarios landed.
- [ ] EP-M3: documentation, roadmap entry marked done, plan `COMPLETE`.

## Surprises & discoveries

- Observation: manifests are not deserialized directly from YAML.
  Evidence: `evaluate_manifest` in `src/manifest/mod.rs` parses YAML into
  `ManifestValue` (`serde_json::Value`) with `serde_saphyr::from_str`, expands
  `foreach`, and only then calls `serde_json::from_value::<NetsukeManifest>`.
  Impact: the selector's decoder sees JSON value kinds. Tests decode through
  the same two stages, and YAML 1.1 Boolean coercion happens before the decoder
  runs (Risk 1).
- Observation: no stage renders Jinja inside arbitrary string values before
  typed deserialization. Expansion evaluates only `foreach` and `when` and
  writes only into `vars` (`src/manifest/expand/`); rendering runs on the typed
  AST afterwards. Evidence: design review (contracts panel) reading
  `src/manifest/expand/mod.rs` and `src/manifest/mod.rs`. Impact:
  `shell: "{{ x }}"` reaches the decoder as literal text and the name grammar
  rejects it. A `foreach` cannot vary the shell per item unless 11.3.1
  deliberately makes `shell` a rendered field.
- Observation: every Fluent key exists in all 35 locale catalogues under
  `locales/`, but no gate forbids literal `#[error("…")]` text; 44 such sites
  exist (for example `src/manifest/env_policy/mod.rs`). Evidence:
  `host_pattern.empty` appears in 35 `messages.ftl` files; the localization
  audit only compares declared keys with catalogues. Impact: deferring
  localization (Decision D5) is consistent with repository practice and trips
  no gate.
- Observation: `build.rs` recompiles a named slice of `src/cli/` and
  `src/host_pattern.rs`, and refuses module-wide dead-code suppression.
  Evidence: `build.rs`, lines 30 to 70. Impact: 11.2.2's `CliConfig` field will
  pull `ShellName` into that slice, which shapes `name.rs` now (Decision D1).
- Observation: Netsuke has no host-platform type; `cfg!(windows)` is used
  directly, and `RecipeShell::host_default` is only testable on Windows.
  Evidence: `src/recipe_shell.rs`; the injected-Boolean precedent is
  `validate_recipe_shell_with(is_windows: bool, …)` in
  `src/runner/recipe_shell.rs`. Impact: Decision D2 introduces an injected
  host-family value so both host tables are tested on every platform.

## Decision log

- Decision D1: place the vocabulary in a new feature module
  `src/shell_selection/`, exported as `pub mod shell_selection` from
  `src/lib.rs`, rather than in `src/ast/`, `src/ir/`, `src/cli/`, or a
  `src/shell/` directory. Rationale: `ShellName` and `BuiltInShell` are shared
  by three later consumers (the configuration definitions in 11.2.2, the
  resolver in 11.2.3, and the AST in 11.3.1). Placing them in `src/ast/` would
  make `src/cli/` depend on the manifest AST, and ADR-019 keeps them outside
  the execution IR. `shell` alone would collide with the MiniJinja `shell()`
  helper and with `RecipeShell`. A leaf module mirrors `src/recipe_shell.rs`,
  which sits below both lowering and rendering. Items are `pub` because the AST
  that will carry `ShellSelection` is public and because the behavioural tests
  in `tests/` are an external crate. The crate is pre-1.0, so no compatibility
  promise attaches. Dependency rules: `src/shell_selection/` holds the
  vocabulary only; the 11.2.3 registry and resolver live in a sibling module
  (for example `src/shell_resolution/`) and take definitions, not `CliConfig`;
  `src/cli/` may depend on `src/shell_selection/`, never the reverse. Inside
  the module, `built_in.rs` depends on `name.rs`, never the reverse, so
  `From<BuiltInShell> for ShellName` lives in `built_in.rs`. `name.rs` imports
  only `std`, `serde`, and `thiserror`, so 11.2.2 can add it to the
  build-script slice by `#[path]` without dragging in the registry or the
  selector. Date/Author: 2026-09-24, planning agent; amended after design
  review.
- Decision D2: model the host family as `HostFamily { UnixLike, Windows }`
  (`shell_selection::HostFamily`) and pass it into every host-dependent query.
  `HostFamily::current()` is the single `cfg!(windows)` read. `HostFamily`
  lives in `built_in.rs`, beside its only consumer. Rationale: RFC 0011 section
  11 requires host-independent table tests for every built-in and host rule,
  which the `RecipeShell` pattern (`cfg!` inside `host_default`) cannot
  provide. An enum rather than a Boolean follows AGENTS.md's newtype guidance,
  and the plan's glossary term "host family" gives the name. Non-Windows
  targets are Unix-like, matching `RecipeShell::host_default`. The target
  family is fixed at compile time, so it is not ambient runtime input of the
  kind ADR-008 governs and needs no `mockable` seam. Scope and reuse policy:
  `HostFamily` belongs to structured-command shell selection only. The 11.2.3
  resolver and 11.3.1 lowering may consume it; legacy `RecipeShell`, MiniJinja
  helpers, and the `which` subsystem must not adopt it without a separate
  decision. Date/Author: 2026-09-24, planning agent; renamed from `ShellHost`
  after design review.
- Decision D3: `BuiltInShell` exposes `ALL`, `name`, `lookup(&ShellName)`,
  `is_supported_on(HostFamily)`, `executable()`, `fixed_args()`, and
  `host_default(HostFamily)`, plus `Display`;
  `From<BuiltInShell> for ShellName` also exists. `BuiltInShell` has no
  `FromStr` and no serde implementation, and `ShellName` does not implement
  `Borrow<str>`. Rationale: all raw text enters through `ShellName::parse`, so
  there is one parser for the grammar. `lookup` answers "is this validated name
  a built-in?" with `Option`, which is exactly the "built-ins first, then
  configured" query 11.2.3 needs (ADR-019 addendum, commit `73c0c0bf`); it is
  written as a search of `ALL` comparing `name()`, so `name()` is the single
  spelling table. Manifests and configuration carry names, never
  `BuiltInShell`, so serde on it would add an unused second spelling.
  `Borrow<str>` would let a registry look names up by unvalidated text, undoing
  the single-parser rule. `executable()` returns
  `BuiltInExecutable::{AbsolutePath, BareName}`: these are the same two
  executable forms ADR-019 allows for configured shells, so 11.2.3 resolves
  both sources with one vocabulary. `powershell` is
  `BareName("powershell.exe")`; ADR-019 Table 1 says it resolves through
  "trusted host `PATH` and `PATHEXT`", and the resolver appends no `PATHEXT`
  suffix to an executable that already has an extension. This plan carries data
  only and leaves that step to 11.2.3. Date/Author: 2026-09-24, planning agent;
  amended after design review.
- Decision D4: implement the `ShellName` grammar with a handwritten validator
  that walks `text.chars()`; use the `regex` crate only in tests, as an
  independent oracle. Rationale: `regex` is a dev-dependency only, adding it at
  runtime would breach the dependency tolerance, and precedent
  (`src/host_pattern.rs`) validates by hand. An independent oracle makes the
  property test meaningful rather than a restatement. Parsing is exact: no
  trimming, no case folding. Error precedence is fixed so every input has one
  deterministic error:
  1. `Empty` for the empty string;
  2. `TooLong { length }` when the UTF-8 byte length exceeds 63, checked
     before any character is examined so the work is bounded (every accepted
     name is ASCII, so bytes equal characters there);
  3. `InvalidFirstCharacter { character }` when the first character is not
     `a` to `z`; and
  4. `InvalidCharacter { character, char_index }` for the first later
     character outside `a` to `z`, `0` to `9`, `_`, and `-`, where
     `char_index` counts characters from zero.
  Character classes use `matches!(c, 'a'..='z' | '0'..='9' | '_' | '-')`, which
  satisfies Whitaker's two-branch conditional limit. Date/Author: 2026-09-24,
  planning agent; precedence added after design review.
- Decision D5: `ShellNameError` is a typed `thiserror` enum whose variants carry
  structured, bounded data, with a plain English `Display` for developers.
  Localization is deferred to the serde adapters (`ShellName`'s `Deserialize`
  implementation and the `ShellSelection` visitor) when the first consumer that
  shows the error to a user lands: configuration loading (11.2.2) or manifest
  loading (11.3.1). Rationale: ADR-019 requires typed internal classification
  converted once to a localized diagnostic at a boundary, following ADR-005.
  Serde's `Error::custom` reduces any error to text, so the only place the
  typed error is still available is inside the adapters; they are therefore the
  conversion boundary, and a later task changes them to emit a
  `LocalizedMessage` while `ShellNameError` stays unlocalized. No user can
  trigger this error until 11.2.2 or 11.3.1, and the right wording depends on
  context those tasks own. Adding Fluent keys now would cost 35 catalogue edits
  for text nobody sees, and no gate requires it (see
  `Surprises & discoveries`). The `Display` strings and the visitor's
  `expecting` text are pinned by an `insta` snapshot so any interim consumer
  sees a stable contract. The error never echoes the whole input, so
  diagnostics stay bounded. Date/Author: 2026-09-24, planning agent; boundary
  clarified after design review.
- Decision D6: `ShellSelection` implements `Deserialize` by hand through
  `deserialize_any` and a `Visitor` that accepts only Booleans and strings, and
  implements `Serialize` symmetrically (`Direct` to `false`, `PlatformDefault`
  to `true`, `Named` to its string). `Default` is `Direct`. Rationale: a private
  `#[serde(untagged)]` helper with `try_from` could map the two Booleans, but
  untagged deserialization replaces the precise cause with "data did not match
  any variant". A visitor gives serde's standard
  `invalid type: …, expected a Boolean or a shell name` for every other kind
  and passes the `ShellNameError` text through for invalid strings. Explicit
  `null` is rejected rather than treated as absent: `#[serde(default)]` applies
  only when the key is missing, and a bare `shell:` is a YAML null. `Serialize`
  is needed because `NetsukeManifest` derives it (`src/ast/mod.rs`) and because
  OrthoConfig merging in 11.2.2 goes through `serde_json::to_value`
  (`src/cli/merge/mod.rs`); `serde-saphyr`'s serializer quotes YAML 1.1 Boolean
  spellings, so `Named("n")` round-trips. Date/Author: 2026-09-24, planning
  agent; rationale corrected after design review.
- Decision D7: pin, but do not change, the front-end's YAML 1.1 Boolean
  coercion. Rationale: the coercion applies to every Boolean field in every
  manifest; changing `serde-saphyr` options is a manifest-wide compatibility
  decision that roadmap 12.1.1 (contract consolidation) owns. This task records
  the exact mapping, including tagged scalars and case variants, in tests and
  in the design document so 12.1.1 decides with evidence. Date/Author:
  2026-09-24, planning agent.
- Decision D8: keep the ADR-019 grammar exactly; do not reserve
  Boolean-looking names in `ShellName`, and do not add a Boolean-spelling
  helper in this task. Rationale: narrowing the grammar would be an
  architecture deviation that also affects configuration names, with no
  consumer in this task. A misroute is only possible once an operator can
  define a name such as `on`, so reservation at configuration time (11.2.2)
  prevents it completely; without such a configured name, a quoted `"on"` fails
  closed later as an unknown shell. A shared `is_yaml11_boolean_spelling`
  helper was proposed in review and not adopted: it has no caller in this task,
  and an unused inherent method in `name.rs` would become a dead-code error in
  the 11.2.2 build-script slice. The eight spellings are recorded in the design
  document and pinned by O11, and the hand-off to 11.2.2 is firm. Date/Author:
  2026-09-24, planning agent; amended after design review.
- Decision D9: verification uses `rstest` tables for the finite registry,
  `proptest` for the grammar and codec invariants, two `insta` snapshots (the
  rendered registry table, and the error and `expecting` text), and
  `rstest-bdd` scenarios for the decoding contract. It uses neither Kani nor
  Verus. Rationale: the registry is four variants by two hosts, which a table
  enumerates exhaustively. The grammar is a regular language whose best
  available independent check is a regular-expression engine; a Kani harness
  would compare the validator against a second handwritten predicate, which
  restates the property rather than proving it. The validator has no indexing,
  arithmetic, or `unsafe` code, so bounded model checking adds no memory-safety
  evidence. No lemma or contractual business logic is introduced that needs an
  unbounded proof, and Verus is outside every gate under the 4.1.3 scope
  boundary. The host-default behavioural scenario from the first draft is
  dropped: it was a pure function already covered by O4 and the snapshot, and
  it added two fields to the shared `TestWorld`. Date/Author: 2026-09-24,
  planning agent; amended after design review.
- Decision D10: documentation targets. Record decisions D1 to D8, including
  the YAML 1.1 Boolean table, in a new `docs/netsuke-design.md` section 5.3
  subsection, "Structured-command shell selection vocabulary", placed
  immediately before "Legacy recipe shell selection". Document the internal
  interface, the dependency rules, and the `HostFamily` scope and reuse policy
  in `docs/developers-guide.md` under "Command and recipe lowering". List
  `src/shell_selection/` in `docs/repository-layout.md`. Make no
  `docs/users-guide.md` change, because users can observe no behaviour; 11.3.3
  owns user guidance. Add no `docs/polonius.md` entry, because the only
  borrow-returning accessors (`as_str`, `fixed_args`) are trivial field or
  static borrows with no lookup or get-or-create shape. Add no new ADR: the
  decisions refine ADR-019 without changing it. Date/Author: 2026-09-24,
  planning agent.

## Outcomes & retrospective

Not yet started. On completion, record here the delivered API, the gate
results, and these hand-offs:

- to 11.2.2: add `src/shell_selection/name.rs` to the build-script slice by
  `#[path]`, as `host_pattern` is, and confirm no inherent method is dead
  there; place the reserved-name check where it does not pull `built_in.rs`
  into the slice; localize `ShellNameError` inside `ShellName`'s `Deserialize`
  implementation, naming the configuration key and definition index; and
  resolve explicitly whether to reserve the eight lower-case YAML 1.1 Boolean
  spellings (`true`, `false`, `yes`, `no`, `y`, `n`, `on`, `off`) as configured
  names alongside the built-ins, through an ADR-019 addendum. This plan
  recommends reserving them;
- to 11.2.3: build the resolver in a sibling module, not inside
  `shell_selection`; consume `BuiltInExecutable` and `HostFamily`; `BareName`
  entries have no path separator, and `powershell.exe` already carries an
  extension, so no `PATHEXT` suffix is appended to it;
- to 11.3.1: add `#[serde(default)] pub shell: ShellSelection` to the command
  block and `pub use` the type from `src/ast/`; localize the selector's errors
  inside its visitor; avoid `#[serde(untagged)]` on the enclosing command-item
  type, or it will swallow the selector's precise errors; decide explicitly
  whether `shell` is a rendered field (as things stand a `foreach` cannot vary
  it per item, and users must be told); and keep `ShellSelection` out of the IR
  and out of the action plans versioned by 12.3.1, so `true` never hashes
  identically across hosts that resolve it differently;
- to 12.1.1: decide whether the manifest front-end should adopt strict Boolean
  parsing, given the pinned YAML 1.1 mapping.

## Context and orientation

Netsuke is a Rust build-system front-end that compiles a YAML manifest (a
Netsukefile) into a Ninja build file. The crate is both a library
(`src/lib.rs`) and a binary. Relevant files:

- `docs/adr-019-structured-command-shell-selection.md` is the governing
  decision. Table 1 lists the built-ins; the addendum separates `BuiltInShell`
  from `ShellName` and gives resolution precedence to built-ins.
- `docs/rfcs/0011-allow-listed-structured-command-shells.md` sections 4, 5.1,
  6.1, and 11 give the schema, the registry, the AST sketch, and the test
  strategy.
- `docs/rfcs/0001-structured-command-blocks.md` section 10.1 amends the
  `shell` field to `Boolean | ShellName`; section 17.1 sketches
  `ShellSelection` and `BuiltInShell`.
- `src/recipe_shell.rs` holds `RecipeShell`, the legacy recipe-wide
  interpreter: a data-only leaf module with a `host_default` constructor. It is
  not reused here (ADR-019 forbids sharing the registry with legacy recipes),
  but its placement and style are the model.
- `src/ast/dependency_order.rs` holds `DependencyOrder`, a small serde-derived
  manifest enum with a documented YAML example. It shows the AST-to-IR
  boundary: the IR owns a separate value and converts from the AST value in
  `src/ir/from_manifest.rs`.
- `src/host_pattern.rs` holds `HostPattern`, the best precedent for a validated
  string newtype: `parse` with a `# Errors` section, `TryFrom<&str>`,
  `TryFrom<String>`, `FromStr`, and a `Deserialize` that reads a `String` and
  maps a parse error with `serde::de::Error::custom`.
- `build.rs`, lines 30 to 70, shows the build-script slice.
- `src/manifest/mod.rs`, function `evaluate_manifest`, is the manifest
  front-end.
- `src/snapshot_test_support.rs` provides `snapshot_settings(subdir)`, which
  roots snapshots at `src/snapshots/<subdir>/`; every existing snapshot
  directory uses it.
- `tests/bdd_tests.rs` is the single behavioural harness. It calls
  `rstest_bdd_macros::scenarios!("tests/features", fixtures = [world:
  TestWorld])`,
  so a new `.feature` file under `tests/features/` is discovered
  automatically, and generated tests are named `<feature_stem>_<scenario>`.
  Step definitions live in `tests/bdd/steps/`, and each new step module must be
  declared in `tests/bdd/steps/mod.rs`. Scenario state lives on `TestWorld` in
  `tests/bdd/fixtures/mod.rs`, in `rstest_bdd::Slot<T>` fields.
- `tests/integration_test_wiring_tests.rs` fails if a test module tree is not
  wired into a Cargo test target.
- CI runs the test gate on Linux and Windows (`.github/workflows/ci.yml`,
  `.github/workflows/ci-windows.yml`).

Skills and documents to consult while implementing:

- skills: `execplans` (this format), `rust-router` then `rust-types-and-apis`
  (newtypes, enum API shape) and `rust-unit-testing` (rstest, googletest,
  pretty_assertions, insta), `proptest` (strategies and non-vacuity),
  `rust-verification` (why Kani and Verus are not used here),
  `hexagonal-architecture` (keep the vocabulary a pure domain leaf with no
  adapter dependencies), `codegraph-mcp` (confirm no caller changes), and
  `en-gb-oxendict` for prose;
- documents: `docs/netsuke-design.md` section 5.3,
  `docs/rust-testing-with-rstest-fixtures.md`, `docs/rstest-bdd-users-guide.md`
  (Scenario Outline, Examples, and placeholders),
  `docs/rust-doctest-dry-guide.md`,
  `docs/reliable-testing-in-rust-via-dependency-injection.md` (why the host
  family is injected), `docs/snapshot-testing-in-netsuke-using-insta.md`,
  `docs/adr-005-typed-which-resolve-error.md`,
  `docs/adr-008-environment-seam-taxonomy.md`, `docs/whitaker-users-guide.md`,
  `docs/documentation-style-guide.md`, and `docs/ortho-config-users-guide.md`.
  OrthoConfig is not used by this task: configuration-layer shell definitions
  are 11.2.2's scope, and nothing here is layered configuration.

Prior art confirms the shape of the registry. GitHub Actions' `shell` keyword
offers a finite built-in set (`bash`, `sh`, `pwsh`, `powershell`, `cmd`,
`python`) with fixed invocation templates and a host-dependent default, and
lets workflow authors supply a custom `command [options] {0}` template.[^1]
ADR-019 deliberately withholds that last capability from manifests and gives it
to trusted operator configuration instead. None of the common alternatives
(Dockerfile exec and shell forms, `just`'s recipe-wide `set shell`, Taskfile's
embedded interpreter) lets one manifest Boolean switch between direct and shell
execution, so the three-way selector is Netsuke-specific.

## Conformance basis

Upstream artefacts, at the revision on `main` at commit `dcac7634`:

- `docs/roadmap.md` task 11.2.1 (and its successors 11.2.2, 11.2.3, 11.2.4,
  and 11.3.1 for scope boundaries).
- ADR-019 (Accepted, 2026-09-02) including both 2026-09-02 addenda.
- RFC 0011 (Proposed) sections 4.1, 5.1, 5.2 (name grammar only), 6.1, 9,
  and 11.
- RFC 0001 (Proposed) sections 10.1 and 17.1.
- ADR-005 (typed internal errors converted once) and ADR-008 (environment
  seam taxonomy), as governing standards.
- AGENTS.md code-style, testing, and documentation rules.

There are no Terms of Reference or technical-design revision identifiers beyond
these documents. Traced items:

- `RM-11.2.1-A`: finite `BuiltInShell` with `sh`, `bash`, `pwsh`,
  `powershell`.
- `RM-11.2.1-B`: validated `ShellName` retaining configured names such as
  `dash`.
- `RM-11.2.1-C`: host availability, fixed arguments, parsing, and host-default
  mapping.
- `RM-11.2.1-D`: Boolean-or-name selection with three intent-preserving
  variants; every other YAML type rejected.
- `RM-11.2.1-E`: data-only; no AST wiring; command execution unchanged.
- `ADR019-T1`: ADR-019 Table 1 (names, hosts, executables, resolution, fixed
  arguments) and the `true` mapping.
- `ADR019-GRAMMAR`: `[a-z][a-z0-9_-]{0,62}`.

Trace chains:

```plaintext
RM-11.2.1-A, ADR019-T1 -> EP-M1
  -> built_in tests: built_in_names_are_exactly_the_adr_019_set, all_lists_every_variant
RM-11.2.1-B, ADR019-GRAMMAR -> EP-M1
  -> name tests: accepts_*, rejects_*, error_precedence
  -> name_property_tests: parse_agrees_with_regex_oracle, name_round_trips
RM-11.2.1-C, ADR019-T1 -> EP-M1
  -> built_in tests: support_matrix, fixed_invocations, host_default_*
  -> snapshot: built_in_registry_table
RM-11.2.1-D -> EP-M2
  -> selection tests: decodes_*, rejects_*, front_end_parity
  -> selection_property_tests; snapshot: error_texts
  -> tests/features/shell_selection_vocabulary.feature
RM-11.2.1-E -> EP-M1, EP-M2
  -> scope check (git diff); no ShellSelection in src/ir or src/ninja_gen; unchanged make test
```

## Verification plan

The change introduces a finite registry, a regular-language validator, and a
decoding function from JSON value kinds to intents. The obligations below cover
them. Each states why a wrong implementation would fail it.

### Axioms

External assumptions, not verified here:

- AXIOM-1: `serde-saphyr` 1.2.0 with default options, when deserializing an
  untyped value from a plain scalar, trims it with Rust's Unicode `trim()`
  (which also strips U+00A0), then returns a Boolean for any ASCII case of
  `true`, `yes`, `y`, `on`, `false`, `no`, `n`, and `off` (`parse_scalars.rs`,
  `parse_yaml11_bool`). Quoted scalars stay strings; plain `~`, `null`, and
  empty scalars become null; tags change the path (`src/de/deserializer.rs`,
  `deserialize_any`). The parity tests exercise this boundary against the real
  crate rather than assuming it.
- AXIOM-2: `serde_json::Value`'s `deserialize_any` calls `visit_bool` for
  Booleans, the string visitors for strings, `visit_unit` for null, number
  visitors for numbers, `visit_seq` for arrays, and `visit_map` for objects.
  Netsuke enables `serde_json`'s `preserve_order` feature but not
  `arbitrary_precision`.
- AXIOM-3: the `regex` crate correctly decides membership of
  `\A[a-z][a-z0-9_-]{0,62}\z`; `\z` is a true end-of-text anchor.
- AXIOM-4: `cfg!(windows)` is true exactly when compiling for a Windows target.
- AXIOM-5: ADR-019 Table 1 is the authoritative registry.

### Obligations

#### O1: finite and complete built-in vocabulary

`BuiltInShell::ALL` contains each variant exactly once, and the set of `name()`
values is exactly `{sh, bash, pwsh, powershell}`.

- Method: parameterized test plus a compile-time exhaustiveness guard; the
  domain has four elements, so enumeration is exhaustive.
- Artefact: `src/shell_selection/built_in_tests.rs`,
  `built_in_names_are_exactly_the_adr_019_set` and `all_lists_every_variant`.
  The latter compares `ALL` with the handwritten list
  `[Sh, Bash, Pwsh, PowerShell]` and calls, for each element, a test helper
  whose `match` has no wildcard arm and returns the expected name.
- Non-vacuity: every variant is a witness. Removing a variant from `ALL` fails
  the list comparison; adding a variant without updating the helper fails to
  compile with E0004, which is the mechanism that forces the test to be
  revisited.

#### O2: built-in names are valid registry names

For every `b` in `ALL`, `ShellName::parse(b.name())` succeeds and equals
`ShellName::from(b)`.

- Method: parameterized test. `From<BuiltInShell> for ShellName` constructs
  without re-validation; this lemma justifies that.
- Artefact: `built_in_tests.rs`, `built_in_names_satisfy_the_grammar`.
- Non-vacuity: a misspelt name such as `Bash` in `name()` fails the parse.

#### O3: lookup round trip and exclusivity

`lookup(&ShellName::from(b)) == Some(b)` for every built-in, and
`lookup(&n) == None` for every valid name outside the four.

- Method: parameterized test for the first half; property test for the
  second.
- Domain: generated valid names from the grammar, filtered to exclude the four
  built-in names (the filter rejects about one sample in 62,000, driven by
  `sh`), plus explicit rows `dash`, `zsh`, `cmd`, `sh2`, `shh`, and `bashx`.
- Artefact: `built_in_tests.rs` and
  `src/shell_selection/name_property_tests.rs`,
  `lookup_is_none_for_non_built_in_names`.
- Non-vacuity: `dash` is a mandatory witness named by the roadmap. A `lookup`
  that matches on prefix fails the `sh2` and `bashx` rows.

#### O4: host support and defaults match ADR-019

The support matrix is exactly `sh` on Unix-like only; `bash` and `pwsh` on both;
`powershell` on Windows only. `host_default(UnixLike) == Sh`,
`host_default(Windows) == PowerShell`, and `host_default(h).is_supported_on(h)`
for both hosts.

- Method: parameterized 4-by-2 truth table and an `insta` snapshot.
- Artefact: `built_in_tests.rs`, `support_matrix`, `host_default_*`, and
  snapshot `built_in_registry_table`, which renders every row of Table 1 (name,
  hosts, executable kind and value, fixed arguments) from code. It is stored at
  `src/snapshots/shell_selection/netsuke__shell_selection__built_in__tests__built_in_registry_table.snap`
  through `snapshot_settings("shell_selection").bind(…)`.
- Non-vacuity: all eight cells are asserted, four true and four false.
  Flipping any cell fails exactly one case.

#### O5: fixed invocations

Executables and arguments match ADR-019 exactly: `sh` is
`AbsolutePath("/bin/sh")` with `["-c"]`; `bash` is `BareName("bash")` with
`["--noprofile", "--norc", "-c"]`; `pwsh` is `BareName("pwsh")` and
`powershell` is `BareName("powershell.exe")`, both with
`["-NoLogo", "-NoProfile", "-NonInteractive", "-Command"]`. Every fixed
argument is non-empty and NUL-free; every `AbsolutePath` starts with `/`; no
`BareName` contains `/` or `\`. The shape checks deliberately avoid `std::path`
and `MAIN_SEPARATOR`, whose answers differ on Windows.

- Method: parameterized test with `pretty_assertions::assert_eq` on whole
  slices.
- Artefact: `built_in_tests.rs`, `fixed_invocations` and
  `invocation_shapes_are_well_formed`.
- Non-vacuity: argument order is asserted, so swapping `--norc` and
  `--noprofile` fails.

#### O6: host family tripwire

On the compiling host, `HostFamily::current() == HostFamily::Windows` if and
only if `RecipeShell::host_default() == RecipeShell::PowerShell`.

- Method: named unit test on every CI platform.
- Artefact: `built_in_tests.rs`,
  `current_host_agrees_with_legacy_recipe_default`.
- Non-vacuity: both sides read `cfg!(windows)`, so this is an alignment
  tripwire against a future change to either default rather than independent
  evidence. CI exercises the Linux and Windows sides; a local run exercises
  one. This is a recorded residual gap.

#### O7: grammar

For every string `s`, `ShellName::parse(s)` is `Ok` if and only if `s` matches
`\A[a-z][a-z0-9_-]{0,62}\z`, and on success `as_str() == s`.

- Method: property test against the `regex` oracle, plus a boundary and
  precedence table.
- Domain: four generators combined with `prop_oneof!`:
  1. valid names from `proptest::string::string_regex("[a-z][a-z0-9_-]{0,62}")`;
  2. near misses built by mutating a valid name, where every mutation must
     change the string: upper-case one _letter_ position (index 0 is always a
     letter); prefix a digit, `_`, or `-`; insert one of `/`, `\`, `.`, a
     space, NUL, `é`, or `{` using `String::insert` at a character boundary; or
     extend to 64 characters;
  3. printable ASCII strings of length 0 to 70; and
  4. arbitrary Unicode strings.
- Artefact: `name_property_tests.rs`, `parse_agrees_with_regex_oracle`, and
  boundary cases in `src/shell_selection/name_tests.rs`: empty; lengths 1, 63,
  and 64; each forbidden first character; `dash`, `/bin/bash`, `bash.exe`,
  `C:\Windows\System32\bash.exe`, `{{ shell }}`, `bash` with a leading space,
  and `Bash`. Precedence rows in `error_precedence`: 70 characters with an
  upper-case letter at position 2 gives `TooLong`; 70 copies of `é` gives
  `TooLong { length: 140 }`; `Bash` gives `InvalidFirstCharacter('B')`; `é`
  gives `InvalidFirstCharacter('é')`; and `ba/sh` gives
  `InvalidCharacter { character: '/', char_index: 2 }`.
- Evidence: default 256 cases per property; failures persist under
  `proptest-regressions/`.
- Non-vacuity: a deterministic companion test, written as
  `#[test] fn … -> Result<(), String>` following
  `src/ir/cycle_property_tests.rs`, builds
  `proptest::test_runner::TestRunner::deterministic()` and draws 1,000 samples
  from each generator with `strategy.new_tree(&mut runner)?.current()`
  (importing `proptest::strategy::ValueTree`). It asserts that the valid
  generator yields only accepted strings, the near-miss generator yields only
  rejected strings, and the printable-ASCII generator yields both classes.
  Valid strings make up only about 0.6% of that last generator, so "both
  classes" is seed-dependent; the test's comment says so. Negative controls
  exercised during Red-Green: changing the length limit to 64 fails the
  64-character boundary case; allowing upper case fails `Bash`.

#### O8: name round trips

`ShellName::parse(n.as_str()) == Ok(n)`, and JSON and YAML
serialize-then-deserialize return `n`, for every valid `n`.

- Method: property test, plus explicit rows for `yes`, `n`, `on`, `null`, and
  `true`, which sampling reaches rarely and which are the rows at risk from
  YAML 1.1 quoting.
- Artefact: `name_property_tests.rs`, `name_round_trips` and
  `boolean_like_names_round_trip`.
- Non-vacuity: the generator is O7's valid-name strategy, whose acceptance is
  established there; the explicit rows fail if the serializer stops quoting.

#### O9: selector decoding is exact

Decoding a JSON value `v` yields `Direct` if `v` is `false`, `PlatformDefault`
if `v` is `true`, `Named(n)` if `v` is a string for which `ShellName::parse`
gives `n`, and an error in every other case (invalid-name strings, null,
numbers, arrays, objects).

- Method: parameterized test over every JSON kind, and a property test over
  generated `serde_json::Value`s.
- Domain: a recursive `Value` strategy of depth at most 2 covering all six
  kinds, with strings drawn from O7's generators and numbers from `i64` and
  finite `f64` ranges (`Number::from_f64` rejects NaN and infinities).
- Artefact: `src/shell_selection/selection_tests.rs` (`decodes_*`,
  `rejects_*`) and `selection_property_tests.rs`,
  `decoding_is_ok_exactly_for_booleans_and_valid_names`.
- Non-vacuity: the property compares against an oracle computed independently
  from the value's kind and the regex, and a deterministic companion (as in O7)
  asserts every kind appears. A decoder that treats `null` as `Direct` fails
  the null case; one that accepts numbers fails the `1` case.

#### O10: selection round trip

`decode(encode(s)) == s` for every selection, through `serde_json` and through
`serde-saphyr`.

- Method: property test plus O8's explicit Boolean-like names wrapped in
  `Named`.
- Artefact: `selection_property_tests.rs`, `selection_round_trips`.
- Non-vacuity: the generator produces all three variants, which the
  deterministic companion asserts.

#### O11: front-end parity and YAML 1.1 pinning

Each YAML input is placed in mapping position, as `shell: <text>`, and decoded
into a test-only wrapper `Probe { #[serde(default)] shell: ShellSelection }`
both through the front-end's two stages (`serde_saphyr::from_str::<Value>` then
`serde_json::from_value::<Probe>`) and directly with
`serde_saphyr::from_str::<Probe>`. An empty mapping `{}` checks the absent-key
default. The test uses `serde_json::Value` directly, not
`crate::manifest::ManifestValue`, so the module keeps no test link to
`manifest`.

- Method: parameterized test with one row per input. Each row states the
  result for each path; where the paths agree the row asserts parity, and where
  they differ the row pins both and the difference is recorded under
  `Surprises & discoveries` without any production change.
- Artefact: `selection_tests.rs`, `front_end_parity`.
- Rows:

  | Input                                                                           | Expected result                |
  | ------------------------------------------------------------------------------- | ------------------------------ |
  | absent key (`{}`)                                                               | `Direct`                       |
  | `false`, `False`, `no`, `n`, `N`, `off`, `oFf`                                  | `Direct`                       |
  | `true`, `TRUE`, `yes`, `yEs`, `y`, `Y`, `on`, `ON`                              | `PlatformDefault`              |
  | `yes` followed by U+00A0 (no-break space)                                       | `PlatformDefault`              |
  | `bash`, `dash`                                                                  | `Named` with the text          |
  | `"true"`, `"false"`, `'yes'`, `"n"`                                             | `Named` with the unquoted text |
  | bare `shell:`, `~`, `null`, `Null`, `1`, `0x10`, `""`, `[bash]`, `{name: bash}` | error                          |
  | `Bash`, `/bin/bash`, `"{{ shell }}"`                                            | error                          |

  _Table: O11 inputs and expected results._

  Tagged-scalar characterization rows are confirmed against the real crates at
  Red and then pinned: `!!str true` (expected `Named("true")`), `! yes`
  (non-specific tag, expected `Named("yes")`), `!custom yes` (expected
  `PlatformDefault`), `!!bool true` (expected to fail at stage one), and `.inf`
  (expected to fail at stage one).
- Non-vacuity: every row is a witness with a concrete expected value, and both
  paths run the real crates (AXIOM-1, AXIOM-2).

#### O12: stable error and `expecting` text

The `Display` of each `ShellNameError` variant and the decoder's error for a
number are pinned.

- Method: `insta` snapshot, because error wording is multivariant output whose
  consistency matters to later consumers (Decision D5).
- Artefact: `selection_tests.rs`, `error_texts`, stored under
  `src/snapshots/shell_selection/`.
- Non-vacuity: the snapshot contains one line per variant plus the
  `expected a Boolean or a shell name` text, so a change to any of them fails.

#### O13: no execution change

No file outside the permitted set in `Constraints` changes; `ShellSelection` is
absent from `src/ir/` and `src/ninja_gen/`; `name.rs` has no `crate::` or
`super::` import; and the full existing suite passes.

- Method: the scope and import checks in `Concrete steps`, and `make test`.
- Non-vacuity: `make test` runs the existing command-lowering and Ninja
  snapshot suites.

### Residual gaps

O6 exercises only one side per host. No Kani or Verus obligation exists
(Decision D9). Localization of errors is deferred (Decision D5). Broader
cross-cutting properties over merged definitions and `PATH` states belong to
11.2.4.

## Plan of work

### Stage A: baseline (no code changes)

Confirm the working tree is clean and on the task branch. Run the gates once to
record a green baseline; if any gate fails on the untouched tree, stop and
report it, because later failures would otherwise be ambiguous.

### EP-M1: host family, built-in registry, and names

File layout under `src/shell_selection/`:

- `mod.rs`: module comment, `mod` declarations, and `pub use` lines.
- `name.rs`: `ShellName` and `ShellNameError`; declares
  `#[cfg(test)] #[path = "name_tests.rs"] mod tests;` and
  `#[cfg(test)] #[path = "name_property_tests.rs"] mod property_tests;`.
- `built_in.rs`: `HostFamily`, `BuiltInExecutable`, `BuiltInShell`, and
  `From<BuiltInShell> for ShellName`; declares
  `#[cfg(test)] #[path = "built_in_tests.rs"] mod tests;`.

Add `pub mod shell_selection;` to `src/lib.rs` between `pub mod runner;` and the
`#[cfg(test)] mod snapshot_test_support;` declaration, keeping alphabetical
order.

Red, first pass (reject-everything stubs): write the tests for obligations O1
to O8. Write each production item with its final signature and bodies that
return a deliberately wrong value: `lookup` returns `None`, `parse` returns
`Err(ShellNameError::Empty)`, `is_supported_on` returns `false`, and
`fixed_args` returns `&[]`. Name unused parameters with a leading underscore
(`_text`), because the gate flags turn `unused_variables` into errors. Run the
focused tests and record that the acceptance-side tests (`accepts_*`,
`fixed_invocations`, `support_matrix` true cells, properties, lookup round
trip) fail on their assertions.

Red, second pass (accept-everything stubs): switch the stubs so `parse` wraps
any text, `lookup` returns `Some(Sh)`, and `is_supported_on` returns `true`.
Record that the rejection-side tests (`rejects_*`, `error_precedence`, the
exclusivity half of O3, `support_matrix` false cells) now fail. Any test that
passes under both stubs is vacuous and must be fixed before Green.

Green: implement the bodies as specified in `Interfaces and dependencies`,
applying this lint checklist as you go:

- `#[must_use]` on `current`, `name`, `lookup`, `is_supported_on`,
  `executable`, `fixed_args`, `host_default`, `as_str`, and `into_inner`,
  placed after the doc comment (Whitaker `function_attrs_follow_docs`);
- a `# Errors` section on `ShellName::parse`;
- `///` on every variant and every error field;
- `const fn` wherever Clippy's `missing_const_for_fn` asks (including
  `as_str`);
- `matches!` for character classes;
- no `&s[..]` slicing or `[i]` indexing, even in tests (use `char_indices`,
  iterators, and `String::insert`);
- `expect` only inside `#[test]` bodies, never in helpers, and no `unwrap`;
- `Result`-returning doctests ending with `Ok::<(), ShellNameError>(())` or
  hidden `fn main() -> Result<…>` lines per `docs/rust-doctest-dry-guide.md`.

Write the registry snapshot test with
`crate::snapshot_test_support::snapshot_settings("shell_selection").bind(||
insta::assert_snapshot!("built_in_registry_table", rendered))`,
review the pending snapshot against ADR-019 Table 1, and accept it.

Refactor: remove duplication, confirm every file is under 400 lines, rerun the
focused tests, then the gates. Commit.

### EP-M2: selection codec and behaviour

Add `src/shell_selection/selection.rs` (`ShellSelection`, its `Serialize`, and
its visitor) with sibling `selection_tests.rs` and
`selection_property_tests.rs`, declared by `#[path]` as in EP-M1.

Spike: before writing the whole feature, add only the first outline with one
Examples row and its step module, run the behavioural command in
`Concrete steps`, and confirm the scenario is generated and bound. This is the
repository's first `Scenario Outline`; if it cannot be bound, stop and escalate
(Tolerances).

Red: write obligations O9 to O12 and the full feature file. Run two stub passes
as in EP-M1: a visitor that rejects everything with a `Serialize` that always
writes `false`, then a visitor that maps every string to `Named` without
validation and every other kind to `Direct`. Record which tests fail under each.

Green: implement the visitor. `expecting` writes `a Boolean or a shell name`;
`visit_bool` maps `false` and `true`; `visit_str` calls `ShellName::parse` and
maps errors with `E::custom` (serde's defaults route `visit_string` and
`visit_borrowed_str` here). Every other visitor method keeps serde's default,
which reports `invalid type`. `Serialize` writes a Boolean or the name. Confirm
the tagged characterization rows of O11 and pin them.

Behavioural specification, `tests/features/shell_selection_vocabulary.feature`:

```gherkin
Feature: Structured-command shell selection vocabulary
  A structured command's shell selector records whether the author wants
  direct execution, the host's default shell, or a named shell. Decoding the
  selector never resolves or runs a shell.

  Scenario Outline: A Boolean or a valid name decodes to the author's intent
    Given the shell selector YAML <yaml>
    When the shell selector is decoded
    Then the shell selection is <intent>

    Examples:
      | yaml   | intent                  |
      | false  | direct execution        |
      | true   | the platform default    |
      | bash   | the named shell "bash"  |
      | dash   | the named shell "dash"  |
      | "true" | the named shell "true"  |

  Scenario Outline: Any other selector is rejected
    Given the shell selector YAML <yaml>
    When the shell selector is decoded
    Then decoding the shell selector fails

    Examples:
      | yaml      |
      | 1         |
      | null      |
      | [bash]    |
      | /bin/bash |
      | Bash      |
      | ""        |
```

Step definitions live in a new `tests/bdd/steps/shell_selection.rs`, declared in
`tests/bdd/steps/mod.rs`, with these pinned patterns so that no generic
placeholder can shadow a literal step:

- `#[given("the shell selector YAML {yaml}")]`, a generic placeholder with no
  `:string` type, so cells such as `"true"` and `""` keep their quotes and
  reach the YAML parser verbatim;
- `#[when("the shell selector is decoded")]`;
- `#[then("the shell selection is direct execution")]`;
- `#[then("the shell selection is the platform default")]`;
- `#[then("the shell selection is the named shell {name:string}")]`; and
- `#[then("decoding the shell selector fails")]`.

The `When` step builds the document `shell: <yaml>` and decodes it through the
same two stages as the front-end into a small wrapper struct defined in the
step module. Add two fields to `TestWorld` in `tests/bdd/fixtures/mod.rs`:
`shell_selector_yaml: Slot<String>` and
`shell_selection_outcome: Slot<Result<ShellSelection, String>>`. Steps return
`anyhow::Result<()>` and resolve failures with `?`, not `expect`, because
helpers outside `#[test]` bodies may not panic; an infallible step that still
returns `Result` carries `#[expect(clippy::unnecessary_wraps, reason = "…")]`,
following `tests/bdd/steps/accessibility_preferences.rs`.

Refactor, then run the gates and commit.

### EP-M3: documentation and closure

Write the documentation described in Decision D10. Mark roadmap task 11.2.1 as
done by changing `- [ ] 11.2.1.` to `- [x] 11.2.1.` in `docs/roadmap.md`.
Update this plan's `Progress`, `Surprises & discoveries`,
`Outcomes & retrospective`, and status. Run `make fmt` immediately after each
Markdown edit, then the Markdown gates and the full gates. Commit.

## Milestones and plateaus

- EP-M0, baseline. Outcome: a recorded green baseline on the unmodified tree.
  Requirements: none. Acceptance: every gate log exits zero. Conformance check,
  recovery, and compatibility: not applicable. Remaining gaps: everything.
- EP-M1, host family, built-in registry, and names. Outcome: `HostFamily`,
  `BuiltInShell`, `BuiltInExecutable`, `ShellName`, and `ShellNameError` exist
  with doctests, unit tests, the registry snapshot, and grammar properties.
  Requirements: `RM-11.2.1-A`, `-B`, `-C`, `-E`; `ADR019-T1`, `ADR019-GRAMMAR`.
  Acceptance: obligations O1 to O8 pass; gates green. Conformance check: Table
  1 unchanged; no file outside the permitted set touched; `name.rs` has no
  `crate::` or `super::` import; no dependency added; `HostFamily` scope
  recorded; trace links current. Recovery: revert the milestone commit to
  return to the baseline. Remaining gaps: the selection codec. Compatibility
  decision: none (new, pre-1.0 items).
- EP-M2, selection codec and behaviour. Outcome: `ShellSelection` decodes and
  encodes exactly, with front-end parity, properties, error-text snapshots, and
  behavioural scenarios. Requirements: `RM-11.2.1-D`, `-E`. Acceptance: O9 to
  O13 pass; the two outlines pass in `tests/bdd_tests.rs`; gates green.
  Conformance check: the selection type is not referenced from `src/ast/`,
  `src/ir/`, or `src/ninja_gen/`; YAML 1.1 behaviour is pinned rather than
  altered; trace links current. Recovery: revert the milestone commit; EP-M1
  remains a coherent plateau. Remaining gaps: documentation. Compatibility
  decision: none.
- EP-M3, documentation and closure. Outcome: design, developer, and layout
  documents describe the vocabulary; the roadmap marks 11.2.1 done; the plan is
  `COMPLETE`. Requirements: AGENTS.md documentation rules. Acceptance: Markdown
  gates and the full gates green. Conformance check: documentation agrees with
  ADR-019; hand-offs recorded. Recovery: revert the documentation commit.
  Remaining gaps: the hand-offs listed in `Outcomes & retrospective`.
  Compatibility decision: none.

## Concrete steps

Run every command from the repository root. Capture gate output with `tee` and
read the log rather than the truncated terminal. Never run two gates at once;
the build cache relies on sequential runs. Delegating the full gate sequence to
the `scrutineer` agent is preferred.

Baseline and full gates:

```bash
BRANCH="$(git branch --show-current)"
make check-fmt 2>&1 | tee "/tmp/check-fmt-netsuke-${BRANCH}.out"
make typecheck 2>&1 | tee "/tmp/typecheck-netsuke-${BRANCH}.out"
make lint 2>&1 | tee "/tmp/lint-netsuke-${BRANCH}.out"
make doc-coverage 2>&1 | tee "/tmp/doc-coverage-netsuke-${BRANCH}.out"
make test 2>&1 | tee "/tmp/test-netsuke-${BRANCH}.out"
```

Each ends with exit status zero. `make test` prints a nextest summary line of
the form `Summary [ …s] N tests run: N passed, … skipped` followed by the
doctest results.

Focused runs reuse the gate's `RUSTFLAGS` and `RUSTDOCFLAGS` composition so
Cargo does not rebuild with a different fingerprint. On Linux the standard
flags are `-Zthreads=8 -Clink-arg=-fuse-ld=mold`; drop the linker flag on other
hosts.

```bash
GATE_FLAGS="${RUSTFLAGS:+$RUSTFLAGS }-D warnings -Zthreads=8 -Clink-arg=-fuse-ld=mold"
RUSTFLAGS="$GATE_FLAGS" cargo nextest run --workspace --all-targets --all-features \
  -E 'test(/shell_selection/)' 2>&1 | tee "/tmp/focused-netsuke-${BRANCH}.out"
```

Expected at Red: named failures such as
`netsuke shell_selection::built_in::tests::support_matrix::case_1` with a
`pretty_assertions` diff. Expected at Green: every selected test passes.

Behavioural scenarios only (generated scenario tests are named after the
feature file stem, so the filter matches them):

```bash
RUSTFLAGS="$GATE_FLAGS" cargo nextest run --workspace --all-features --test bdd_tests \
  -E 'test(/shell_selection/)' 2>&1 | tee "/tmp/bdd-netsuke-${BRANCH}.out"
```

Doctests for the new module:

```bash
RUSTFLAGS="$GATE_FLAGS" RUSTDOCFLAGS="--cfg docsrs -D warnings" \
  cargo test --workspace --doc --all-features shell_selection \
  2>&1 | tee "/tmp/doctest-netsuke-${BRANCH}.out"
```

Snapshot review after adding a snapshot test:

```bash
cargo insta pending-snapshots
cargo insta accept
```

Scope and boundary checks before each milestone commit:

```bash
git --no-pager diff --no-ext-diff --stat HEAD
rg -n 'use (crate|super)::' src/shell_selection/name.rs
rg -n 'ShellSelection' src/ir src/ninja_gen src/ast
```

Expected: the diff lists only `src/lib.rs`, files under `src/shell_selection/`,
files under `src/snapshots/shell_selection/`, `proptest-regressions/` (only if
a failure was found and fixed),
`tests/features/shell_selection_vocabulary.feature`,
`tests/bdd/steps/shell_selection.rs`, `tests/bdd/steps/mod.rs`,
`tests/bdd/fixtures/mod.rs`, and, in EP-M3, `docs/` files. Both `rg` commands
print nothing.

Markdown gates after documentation edits:

```bash
make fmt 2>&1 | tee "/tmp/fmt-netsuke-${BRANCH}.out"
make markdownlint 2>&1 | tee "/tmp/markdownlint-netsuke-${BRANCH}.out"
make nixie 2>&1 | tee "/tmp/nixie-netsuke-${BRANCH}.out"
```

Commit after each green milestone with an imperative subject, for example
`Add the built-in shell registry and ShellName`, and gate each commit.

## Validation and acceptance

Acceptance is behavioural:

- `ShellName::parse("dash")` returns a name whose `as_str()` is `dash`, and
  `BuiltInShell::lookup` of it is `None`; `ShellName::parse("/bin/bash")`
  returns `InvalidFirstCharacter('/')`. Both appear as doctests on `ShellName`.
- `BuiltInShell::host_default(HostFamily::Windows)` is
  `BuiltInShell::PowerShell`, whose `fixed_args()` are `-NoLogo`, `-NoProfile`,
  `-NonInteractive`, `-Command`, and whose `executable()` is
  `BareName("powershell.exe")`.
- Decoding `shell: true` gives `PlatformDefault`; `shell: bash` gives
  `Named("bash")`; `shell: 1` gives an error whose text contains
  `expected a Boolean or a shell name`.

Red-Green-Refactor evidence to record in `Artefacts and notes` per milestone:
the focused command and the failing tests for both Red passes; the same command
passing at Green; the gates passing after Refactor.

Quality criteria:

- Tests: `make test` passes, including every new unit, property, snapshot,
  doctest, and behavioural test; every new test failed under at least one Red
  pass for the asserted reason.
- Verification: obligations O1 to O13 discharged as described, with the
  deterministic non-vacuity companions passing.
- Lint and types: `make check-fmt`, `make typecheck`, and `make lint` pass;
  `make doc-coverage` does not fall below its threshold.
- Documentation: `make markdownlint` and `make nixie` pass.
- Security: the vocabulary accepts no path or template as a name (O7, O11).

## Idempotence and recovery

Every step is re-runnable. Test and gate commands do not mutate tracked files
except `make fmt`, which only reformats. If `cargo insta accept` accepts a
wrong snapshot, delete the `.snap` file and rerun the test to regenerate the
pending snapshot. If a proptest failure writes a file under
`proptest-regressions/`, keep it (it is the regression seed) and commit it with
the fix. Red stubs are never committed. To abandon a milestone, reset the
branch to the previous milestone commit; each milestone is a coherent plateau.

## Artefacts and notes

The registry snapshot should render Table 1 from code in roughly this shape
(the accepted snapshot is authoritative once reviewed against ADR-019):

```plaintext
name        unix-like  windows  executable                  fixed arguments
sh          yes        no       absolute-path /bin/sh       -c
bash        yes        yes      bare-name bash              --noprofile --norc -c
pwsh        yes        yes      bare-name pwsh              -NoLogo -NoProfile -NonInteractive -Command
powershell  no         yes      bare-name powershell.exe    -NoLogo -NoProfile -NonInteractive -Command
unix-like default: sh
windows default: powershell
```

Red-Green-Refactor transcripts will be appended here during implementation.

## Interfaces and dependencies

No new dependency. At the end of EP-M2 these items exist in
`netsuke::shell_selection` (re-exported from `src/shell_selection/mod.rs`). Doc
comments are abbreviated here; the real code carries `///` on every item and
variant, `#[must_use]` where the Green checklist requires it, and doctests on
public items.

```rust
// src/shell_selection/built_in.rs

/// The host family that decides built-in shell support.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HostFamily {
    UnixLike,
    Windows,
}

impl HostFamily {
    pub const ALL: [Self; 2];
    pub const fn current() -> Self;
}

/// How a built-in shell's executable is located.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BuiltInExecutable {
    /// A fixed absolute path used without searching `PATH`.
    AbsolutePath(&'static str),
    /// A bare executable name searched on the trusted host `PATH` by 11.2.3.
    BareName(&'static str),
}

/// The finite built-in shell registry from ADR-019 Table 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BuiltInShell {
    Sh,
    Bash,
    Pwsh,
    PowerShell,
}

impl BuiltInShell {
    pub const ALL: [Self; 4];
    pub const fn name(self) -> &'static str;
    pub fn lookup(name: &ShellName) -> Option<Self>; // Self::ALL.into_iter().find(…)
    pub const fn is_supported_on(self, host: HostFamily) -> bool;
    pub const fn executable(self) -> BuiltInExecutable;
    pub const fn fixed_args(self) -> &'static [&'static str];
    pub const fn host_default(host: HostFamily) -> Self;
}
// Also: impl Display for BuiltInShell; impl From<BuiltInShell> for ShellName.

// src/shell_selection/name.rs (imports only std, serde, thiserror)

/// A validated shell registry name matching `[a-z][a-z0-9_-]{0,62}`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ShellName(String);

impl ShellName {
    pub const MAX_LEN: usize = 63;
    pub fn parse(text: &str) -> Result<Self, ShellNameError>;
    pub const fn as_str(&self) -> &str;
    pub fn into_inner(self) -> String;
}
// Also: TryFrom<&str>, TryFrom<String>, FromStr, AsRef<str>, Display,
// Serialize (as a string), Deserialize (String, then parse, then
// serde::de::Error::custom). No Borrow<str> (Decision D3).

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ShellNameError {
    #[error("a shell name must not be empty")]
    Empty,
    #[error("a shell name must be at most 63 bytes, but has {length}")]
    TooLong { length: usize },
    #[error("a shell name must start with a lower-case ASCII letter, not {character:?}")]
    InvalidFirstCharacter { character: char },
    #[error(
        "a shell name may contain only lower-case ASCII letters, digits, '_', and '-', \
         but has {character:?} at character {char_index}"
    )]
    InvalidCharacter { character: char, char_index: usize },
}

// src/shell_selection/selection.rs

/// The manifest author's shell intent for one structured command block.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum ShellSelection {
    #[default]
    Direct,
    PlatformDefault,
    Named(ShellName),
}
// Also: Serialize and a handwritten Deserialize (Decision D6).
```

The error message wording above is indicative; settle it at Green and pin it
with O12's snapshot. Use `concat!()` rather than a backslash continuation if a
literal exceeds the line limit, per AGENTS.md.

`ShellSelection` has no method that resolves anything. Resolution into an
executable path belongs to 11.2.3, and the IR's `ResolvedShell` belongs to
11.3.1.

[^1]: GitHub Docs, "Workflow syntax for GitHub Actions",
    `jobs.<job_id>.steps[*].shell`,
    <https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax>,
    retrieved 2026-09-24.

## Revision note

- 2026-09-24: initial draft for review.
- 2026-09-24: revised after a three-panel expert design review. Renamed
  `ShellHost` to `HostFamily` and moved it into `built_in.rs`; renamed the
  executable variants to `AbsolutePath` and `BareName`; fixed the dependency
  direction inside the module and outward to 11.2.2 and 11.2.3, including the
  build-script slice constraint on `name.rs`; specified `ShellNameError`
  precedence and character indexing; relocated the localization boundary to the
  serde adapters; expanded the YAML 1.1 and tagged-scalar pinning table and
  moved it to mapping position; added the error-text snapshot (O12) and the IR
  non-leak check (O13); replaced `std::path` absoluteness with a POSIX shape
  check; fixed the near-miss generator and named the deterministic companion
  API; added a second, accept-everything Red pass so rejection tests are proven
  to fail; added the lint checklist, pinned behavioural step patterns and a
  spike, and dropped the host-default scenario; corrected the CI platform list,
  snapshot location, doctest flags, and tolerances; and firmed up the hand-offs
  to 11.2.2, 11.2.3, 11.3.1, and 12.1.1. None of these changes alters the
  ADR-019 contract; the remaining work is unchanged in shape (three
  implementation milestones after approval).
