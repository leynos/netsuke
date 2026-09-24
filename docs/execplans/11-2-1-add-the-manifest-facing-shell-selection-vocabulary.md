# Add the manifest-facing shell selection vocabulary (11.2.1)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: DRAFT

This plan awaits approval. Do not begin implementation until the user
explicitly approves it.

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
   supported, what executable it names, how that executable is located, which
   fixed arguments precede the shell source, and which built-in `true` selects;
2. parse any string into a validated `ShellName` that accepts exactly the
   ADR-019 grammar `[a-z][a-z0-9_-]{0,62}`, so configured names such as `dash`
   are representable while paths, upper-case text, and templates are not; and
3. decode a YAML or JSON `shell` value into a three-way `ShellSelection`
   (`Direct`, `PlatformDefault`, `Named(ShellName)`), rejecting every value
   that is neither a Boolean nor a valid name.

Nothing that Netsuke executes changes. No manifest accepts a `shell` field yet,
because the structured-command abstract syntax tree (AST) from RFC 0001 does
not exist; wiring happens in roadmap task 11.3.1. Success is observable through
the new unit, property, snapshot, and behavioural tests, all of which fail
before the change and pass after it, while every existing test continues to
pass unchanged.

Terms used throughout:

- *Built-in shell*: one of the four shells Netsuke itself knows about, listed
  in ADR-019 Table 1. Built-in names are reserved.
- *Configured shell*: a shell added by trusted operator configuration in task
  11.2.2. This task only makes its name representable.
- *Host family*: the coarse operating-system class that decides built-in
  support. There are exactly two: Unix-like and Windows.
- *Fixed arguments*: the interpreter arguments Netsuke places before the
  rendered shell source, for example `-c` for `sh`.
- *Front-end*: Netsuke's manifest loader in `src/manifest/mod.rs`, which parses
  YAML into a `serde_json::Value` and later deserializes typed AST values from
  that value.

## Constraints

These are hard invariants. Violating one requires escalation, not a workaround.

- Do not change command execution. Do not modify `src/ast/`, `src/ir/`,
  `src/ninja_gen/`, `src/ninja_gen_recipe_shell.rs`, `src/runner/`,
  `src/manifest/`, `src/cli/`, `src/stdlib/`, or `src/recipe_shell.rs`. The
  only permitted production edits are the new `src/shell_selection/` module and
  a single `pub mod shell_selection;` line in `src/lib.rs`.
- Do not wire the new types into any AST, IR, configuration, or command-line
  surface. Roadmap 11.2.1 says the types stay data-only until RFC 0001's AST
  exists (11.3.1), and `CliConfig` definitions belong to 11.2.2.
- Do not read the process environment, the filesystem, or `PATH`. Executable
  resolution, `PATHEXT` normalization, and probing belong to 11.2.3. The only
  ambient input permitted is the compile-time target family (`cfg!(windows)`),
  read in exactly one function.
- Preserve ADR-019 exactly: the four built-in names, their supported hosts,
  executables, resolution kinds, fixed arguments, the `true` host mapping (`sh`
  on Unix-like, `powershell` on Windows), and the `ShellName` grammar. Any wish
  to diverge is an architecture deviation (see `Decision log`).
- Add no new dependency. `serde`, `serde_json`, `serde-saphyr`, and
  `thiserror` are existing runtime dependencies; `rstest`, `rstest-bdd`,
  `googletest`, `pretty_assertions`, `insta`, `proptest`, and `regex` are
  existing dev-dependencies (`Cargo.toml` `[dev-dependencies]`).
- Keep every file at or under 400 lines (AGENTS.md; Whitaker's
  `module_max_lines`). Begin every module with a `//!` comment; give every
  function and method, public or private, a `///` comment; put public-item
  examples in doctests.
- No in-process environment mutation in tests (AGENTS.md, ADR-008). This task
  needs no environment at all.
- Use en-GB-oxendict spelling (`-ize`, `-yse`, `-our`) in comments and
  documentation.
- Do not add a `-Z` Polonius or next-solver directive; the pinned toolchain
  enables both.

## Tolerances (exception triggers)

- Scope: stop and escalate if implementation needs more than 22 changed files
  or more than 1,500 net added lines including tests and documentation, or if
  any file outside the permitted set in `Constraints` needs a production edit.
- Interface: stop and escalate if any existing public item's signature must
  change. Adding the new `pub mod shell_selection` is approved by this plan.
- Dependencies: stop and escalate if a new crate or a new feature on an
  existing crate appears necessary.
- Contract: stop and escalate if ADR-019, RFC 0011, or RFC 0001 appears wrong
  or ambiguous in a way that changes the types (for example, a built-in's
  argument list); record a proposed deviation in `Decision log` and set the
  status to `BLOCKED`.
- Iterations: stop and escalate if a gate still fails after three fix attempts
  for the same root cause.
- Time: stop and escalate if a single milestone exceeds four hours of active
  work.
- Localization: stop and escalate if any gate (for example a Fluent audit or a
  Whitaker lint) demands localized text for the new error type; the plan
  deliberately defers localization (Decision D5).

## Risks

- Risk: YAML 1.1 Boolean spellings. The manifest front-end uses `serde-saphyr`
  1.2.0 with default options, whose untyped path maps plain `y`, `yes`, `on`,
  `n`, `no`, and `off` (any case), as well as `true` and `false`, to Booleans
  before any typed deserializer runs. Unquoted `shell: yes` therefore means
  `PlatformDefault`, and `shell: n` means `Direct`, not the names `yes` or `n`.
  Severity: medium. Likelihood: high (it is certain behaviour). Mitigation: pin
  the behaviour with front-end parity tests, document it in the design
  document, and hand it to 12.1.1 as an input; do not change the front-end here
  (Decision D7).
- Risk: quoted Boolean-looking names. `shell: "true"` is the string `true`,
  which satisfies the ADR-019 grammar, so it decodes to `Named("true")`, not
  `PlatformDefault`. Severity: low now, medium once 11.2.2 lets operators
  define names. Likelihood: low. Mitigation: pin the behaviour in tests and
  record the reservation question for 11.2.2 (Decision D8).
- Risk: the new module drifts from ADR-019 Table 1. Severity: high (the table
  is an authority boundary). Likelihood: low. Mitigation: an exact per-shell
  `rstest` table plus an `insta` snapshot rendering the table from code.
- Risk: a future fifth built-in is added to the enum but not to `ALL`, leaving
  it invisible to enumeration-based checks. Severity: medium. Likelihood: low.
  Mitigation: a test whose exhaustive `match` fails to compile (E0004) for an
  unlisted variant and then checks membership in `ALL`. A fixed array length
  alone does not enforce this.
- Risk: the untyped `deserialize_any` path used by the selector decoder
  behaves differently when 11.3.1 deserializes the AST directly from YAML
  instead of via `serde_json::Value`. Severity: medium. Likelihood: low.
  Mitigation: decode every table case through both paths and assert parity.
- Risk: the 400-line cap on test files. Severity: low. Likelihood: medium.
  Mitigation: one sibling test file per production file, declared with an
  explicit `#[path]` attribute.

## Progress

- [x] (2026-09-24) Renamed the branch to
  `11-2-1-add-the-manifest-facing-shell-selection-vocabulary` and set its
  upstream.
- [x] (2026-09-24) Reconnaissance of ADR-019, RFC 0011, RFC 0001, the roadmap,
  `RecipeShell`, `DependencyOrder`, `HostPattern`, the manifest front-end, and
  the test infrastructure.
- [x] (2026-09-24) Drafted this ExecPlan.
- [ ] Design review by a community-of-experts panel and revision.
- [ ] User approval of the plan.
- [ ] EP-M0: baseline gates recorded.
- [ ] EP-M1: host family, built-in registry, and `ShellName` landed with
  tests.
- [ ] EP-M2: `ShellSelection` codec, parity tests, properties, and behavioural
  scenarios landed.
- [ ] EP-M3: documentation, roadmap entry marked done, plan `COMPLETE`.

## Surprises & discoveries

- Observation: manifests are not deserialized directly from YAML.
  Evidence: `src/manifest/mod.rs` parses YAML into `ManifestValue`
  (`serde_json::Value`) with `serde_saphyr::from_str`, expands `foreach`, and
  only then calls `serde_json::from_value::<NetsukeManifest>`. Impact: the
  selector's decoder sees JSON value kinds (Boolean, string, number, null,
  array, object). Tests must decode through the same two stages, and YAML 1.1
  Boolean coercion happens before the decoder runs (Risk 1).
- Observation: every Fluent key exists in all 35 locale catalogues under
  `locales/`. Evidence: `host_pattern.empty` appears in 35 `messages.ftl`
  files. Impact: localizing an error that no user can yet see would cost 35
  catalogue edits now and possibly rework later; this informed Decision D5.
- Observation: Netsuke has no host-platform type; `cfg!(windows)` is used
  directly, and `RecipeShell::host_default` is only testable on Windows.
  Evidence: `src/recipe_shell.rs`; the injected-Boolean precedent is
  `validate_recipe_shell_with(is_windows: bool, …)` in
  `src/runner/recipe_shell.rs`. Impact: Decision D2 introduces an injected
  host-family value so both host tables are tested on every platform.

## Decision log

- Decision D1: place the vocabulary in a new feature module
  `src/shell_selection/`, exported as `pub mod shell_selection` from
  `src/lib.rs`, rather than in `src/ast/`, `src/ir/`, or `src/cli/`. Rationale:
  `ShellName` and `BuiltInShell` are shared by three later consumers (the
  configuration definitions in 11.2.2, the resolver in 11.2.3, and the AST in
  11.3.1). Placing them in `src/ast/` would make `src/cli/` depend on the
  manifest AST, and ADR-019 keeps them outside the execution IR. A leaf module
  mirrors `src/recipe_shell.rs`, which sits below both lowering and rendering.
  Items are `pub` because the AST that will carry `ShellSelection` is public
  and because the behavioural tests in `tests/` are an external crate. The
  crate is pre-1.0, so no compatibility promise attaches. 11.3.1 will `pub use`
  `ShellSelection` from `src/ast/`. Date/Author: 2026-09-24, planning agent.
- Decision D2: model the host family as `ShellHost { UnixLike, Windows }` and
  pass it into every host-dependent query. `ShellHost::current()` is the single
  `cfg!(windows)` read. Rationale: RFC 0011 section 11 requires
  host-independent table tests for every built-in and host rule, which the
  `RecipeShell` pattern (`cfg!` inside `host_default`) cannot provide. An enum
  rather than a Boolean follows AGENTS.md's newtype guidance. Non-Windows
  targets are Unix-like, matching `RecipeShell::host_default`. The target
  family is fixed at compile time, so this is not ambient runtime input of the
  kind ADR-008 governs, and it needs no `mockable` seam. Scope and reuse policy:
  `ShellHost` belongs to structured-command shell selection only. The 11.2.3
  resolver and 11.3.1 lowering may consume it; legacy `RecipeShell`, MiniJinja
  helpers, and the `which` subsystem must not adopt it without a separate
  decision. Date/Author: 2026-09-24, planning agent.
- Decision D3: `BuiltInShell` exposes `ALL`, `name`, `lookup(&ShellName)`,
  `is_supported_on(ShellHost)`, `executable()`, `fixed_args()`, and
  `host_default(ShellHost)`, plus `Display` and
  `From<BuiltInShell> for ShellName`. It has no `FromStr` and no serde
  implementation. Rationale: all raw text enters through `ShellName::parse`, so
  there is one parser for the grammar; `lookup` answers "is this validated name
  a built-in?" with `Option`, which is exactly the "built-ins first, then
  configured" query 11.2.3 needs (ADR-019 addendum, commit `73c0c0bf`).
  Manifests and configuration carry names, never `BuiltInShell`, so serde
  support would add an unused second spelling. `executable()` returns
  `BuiltInExecutable::{FixedPath, SearchPath}` so 11.2.3 knows whether to
  search the trusted `PATH` without re-deriving ADR-019's "Resolution" column.
  Date/Author: 2026-09-24, planning agent.
- Decision D4: implement the `ShellName` grammar as a handwritten ASCII byte
  validator; use the `regex` crate only in tests, as an independent oracle.
  Rationale: `regex` is a dev-dependency only, adding it at runtime would
  breach the dependency tolerance, and precedent (`src/host_pattern.rs`)
  validates by hand. An independent oracle makes the property test meaningful
  rather than a restatement. Parsing is exact: no trimming, no case folding.
  Date/Author: 2026-09-24, planning agent.
- Decision D5: `ShellNameError` is a typed `thiserror` enum whose variants carry
  structured, bounded data (`Empty`, `TooLong { length }`,
  `InvalidFirstCharacter { character }`,
  `InvalidCharacter { character, index }`) with a plain English `Display` for
  developers. Localization is deferred to the first boundary that shows the
  error to a user: configuration loading (11.2.2) and manifest loading
  (11.3.1). Rationale: ADR-019 requires typed internal classification converted
  to a localized diagnostic once at an application boundary, following ADR-005.
  No user can trigger this error until 11.2.2 or 11.3.1 wires it, and the final
  message wording depends on context those tasks own (configuration key or
  manifest location). The error never echoes the whole input, so diagnostics
  stay bounded. This is a hand-off obligation recorded in
  `Outcomes & retrospective`. Date/Author: 2026-09-24, planning agent.
- Decision D6: `ShellSelection` implements `Deserialize` by hand through
  `deserialize_any` and a `Visitor` that accepts only Booleans and strings, and
  implements `Serialize` symmetrically (`Direct` to `false`, `PlatformDefault`
  to `true`, `Named` to its string). `Default` is `Direct`. Rationale:
  `#[serde(untagged)]` cannot map the two Boolean values onto two distinct unit
  variants, and its "did not match any variant" error hides the cause. A
  visitor gives serde's standard
  `invalid type: …, expected a Boolean or a shell name` message for every other
  kind, including explicit `null`, which is rejected rather than treated as
  absent. Absence becomes `Direct` through `#[serde(default)]` when 11.3.1 adds
  the field. Date/Author: 2026-09-24, planning agent.
- Decision D7: pin, but do not change, the front-end's YAML 1.1 Boolean
  coercion. Rationale: the coercion applies to every Boolean field in every
  manifest; changing `serde-saphyr` options is a manifest-wide compatibility
  decision that roadmap 12.1.1 (contract consolidation) owns. This task records
  the exact mapping in tests and in the design document so 12.1.1 decides with
  evidence. Date/Author: 2026-09-24, planning agent.
- Decision D8: keep the ADR-019 grammar exactly; do not reserve
  Boolean-looking names such as `true`, `yes`, or `n` in `ShellName`.
  Rationale: narrowing the grammar would be an architecture deviation with no
  consumer in this task. Reservation of configured names is 11.2.2's remit
  (ADR-019 already reserves the built-in names there). The question is recorded
  as a hand-off, and the quoted-string behaviour is pinned by tests.
  Date/Author: 2026-09-24, planning agent.
- Decision D9: verification uses `rstest` tables for the finite registry,
  `proptest` for the grammar and codec invariants, one `insta` snapshot for the
  rendered registry table, and `rstest-bdd` scenarios for the decoding
  contract. It uses neither Kani nor Verus. Rationale: the registry is four
  variants by two hosts, which a table enumerates exhaustively. The grammar is
  a regular language whose best available independent check is a
  regular-expression engine; a Kani harness would compare the validator against
  a second handwritten predicate, which restates the property rather than
  proving it. The validator has no indexing, arithmetic, or `unsafe` code, so
  bounded model checking adds no memory-safety evidence. No lemma or
  contractual business logic is introduced that needs an unbounded proof, and
  Verus is outside every gate under the 4.1.3 scope boundary. See
  `Verification plan`. Date/Author: 2026-09-24, planning agent.
- Decision D10: documentation targets. Record decisions D1 to D8 in a new
  `docs/netsuke-design.md` subsection, "Structured-command shell selection
  vocabulary", placed immediately before "Legacy recipe shell selection" in
  section 5.3. Document the internal interface and the `ShellHost` scope and
  reuse policy in `docs/developers-guide.md` under "Command and recipe
  lowering". List `src/shell_selection/` in `docs/repository-layout.md`. Make no
  `docs/users-guide.md` change, because users can observe no behaviour; 11.3.3
  owns user guidance. Add no `docs/polonius.md` entry, because the only
  borrow-returning accessors (`as_str`, `fixed_args`) are trivial field or
  static borrows with no lookup or get-or-create shape. Add no new ADR: the
  decisions refine ADR-019 without changing it. Date/Author: 2026-09-24,
  planning agent.

## Outcomes & retrospective

Not yet started. On completion, record here the delivered API, the gate
results, and these hand-offs:

- to 11.2.2: localize `ShellNameError` at configuration loading, naming the
  configuration key and definition index; decide whether configured names that
  YAML 1.1 or TOML readers could confuse with Booleans (`true`, `yes`, `n`, and
  so on) should be reserved alongside the built-in names;
- to 11.2.3: consume `BuiltInExecutable` and `ShellHost`; `SearchPath`
  entries are bare names, and `powershell.exe` already carries an extension, so
  `PATHEXT` is not appended to it;
- to 11.3.1: add `#[serde(default)] pub shell: ShellSelection` to the command
  block, `pub use` the type from `src/ast/`, and localize the selector's
  deserialization errors through the manifest diagnostic boundary;
- to 12.1.1: decide whether the manifest front-end should adopt strict
  Boolean parsing, given the pinned YAML 1.1 mapping.

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
  interpreter. It is a data-only leaf module with a `host_default` constructor.
  It is not reused here (ADR-019 forbids sharing the registry with legacy
  recipes), but its placement and style are the model.
- `src/ast/dependency_order.rs` holds `DependencyOrder`, a small serde-derived
  manifest enum with a documented YAML example. It shows the AST-to-IR
  boundary: the IR owns a separate value and converts from the AST value in
  `src/ir/from_manifest.rs`.
- `src/host_pattern.rs` holds `HostPattern`, the best precedent for a validated
  string newtype: `parse`, `TryFrom<&str>`, `TryFrom<String>`, `FromStr`, and a
  `Deserialize` that reads a `String` and maps a parse error with
  `serde::de::Error::custom`.
- `src/manifest/mod.rs`, function `evaluate_manifest`, is the manifest
  front-end described in `Surprises & discoveries`.
- `tests/bdd_tests.rs` is the single behavioural harness. It calls
  `rstest_bdd_macros::scenarios!("tests/features", fixtures = [world:
  TestWorld])`,
  so a new `.feature` file under `tests/features/` is discovered
  automatically. Step definitions live in `tests/bdd/steps/`, and each new step
  module must be declared in `tests/bdd/steps/mod.rs`. Scenario state lives on
  `TestWorld` in `tests/bdd/fixtures/mod.rs`, using `Slot<T>` for cloneable
  values.
- `tests/integration_test_wiring_tests.rs` fails if a test module tree is not
  wired into a Cargo test target.
- `src/snapshots/` holds source-level `insta` snapshots, named after the
  declaring module path.

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
  (Scenario Outline and Examples), `docs/rust-doctest-dry-guide.md`,
  `docs/reliable-testing-in-rust-via-dependency-injection.md` (why the host
  family is injected), `docs/snapshot-testing-in-netsuke-using-insta.md`,
  `docs/adr-005-typed-which-resolve-error.md`,
  `docs/adr-008-environment-seam-taxonomy.md`,
  `docs/documentation-style-guide.md`, and `docs/ortho-config-users-guide.md`.
  OrthoConfig is not used by this task: configuration-layer shell definitions
  are 11.2.2's scope, and nothing here is layered configuration.

Prior art confirms the shape of the registry. GitHub Actions' `shell` keyword
offers a finite built-in set (`bash`, `sh`, `pwsh`, `powershell`, `cmd`,
`python`) with fixed invocation templates and a host-dependent default, and
lets workflow authors supply a custom `command [options] {0}` template.[^1]
ADR-019 deliberately withholds that last capability from manifests and gives it
to trusted operator configuration instead.

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
  -> name tests: accepts_*, rejects_*; name_property_tests::parse_agrees_with_regex_oracle
RM-11.2.1-C, ADR019-T1 -> EP-M1
  -> built_in tests: support_matrix, fixed_invocations, host_default_*
  -> snapshot: built_in_registry_table
RM-11.2.1-D -> EP-M2
  -> selection tests: decodes_*, rejects_*, front_end_parity; selection_property_tests
  -> tests/features/shell_selection_vocabulary.feature
RM-11.2.1-E -> EP-M1, EP-M2
  -> git diff path check; unchanged make test result
```

## Verification plan

The change introduces a finite registry, a regular-language validator, and a
decoding function from JSON value kinds to intents. The obligations below cover
them. Each names why a wrong implementation would fail it.

Axioms (external assumptions, not verified here):

- AXIOM-1: `serde-saphyr` 1.2.0 with default options, when asked for an untyped
  value, returns a Boolean for plain scalars matching `true`, `false`, `y`,
  `yes`, `on`, `n`, `no`, and `off` in any case, and a string for quoted scalars
  (`serde-saphyr` `parse_scalars.rs`, `parse_yaml11_bool`). The parity tests
  exercise this boundary against the real crate rather than assuming it.
- AXIOM-2: `serde_json::Value`'s `deserialize_any` calls `visit_bool` for
  Booleans, `visit_str`/`visit_string`/`visit_borrowed_str` for strings,
  `visit_unit` for null, number visitors for numbers, `visit_seq` for arrays,
  and `visit_map` for objects.
- AXIOM-3: the `regex` crate correctly decides membership of
  `\A[a-z][a-z0-9_-]{0,62}\z`.
- AXIOM-4: `cfg!(windows)` is true exactly when compiling for a Windows target.
- AXIOM-5: ADR-019 Table 1 is the authoritative registry.

Obligations:

- Obligation O1 (finite vocabulary): `BuiltInShell::ALL` contains each variant
  exactly once, and the set of `name()` values is exactly
  `{sh, bash, pwsh, powershell}`. Method: parameterized test plus a
  compile-time exhaustiveness guard. Rationale: the domain has four elements,
  so enumeration is exhaustive. Domain: all variants. Artefact:
  `src/shell_selection/built_in_tests.rs`,
  `built_in_names_are_exactly_the_adr_019_set` and `all_lists_every_variant`.
  The latter maps each variant through an exhaustive `match` (no wildcard) to
  an expected name and asserts `ALL` contains it and has no duplicates.
  Evidence: fails before implementation (module absent); passes after.
  Non-vacuity: witness is every variant. Negative control: temporarily remove a
  variant from `ALL` during Red-Green and observe failure; adding a variant
  without updating the `match` fails to compile with E0004, which is the
  mechanism that forces the test to be revisited.
- Obligation O2 (built-in names are valid registry names): for every `b` in
  `ALL`, `ShellName::parse(b.name())` succeeds and equals `ShellName::from(b)`.
  Method: parameterized test. Rationale: `From<BuiltInShell> for ShellName` is
  infallible and constructs without re-validation; this lemma justifies that.
  Artefact: `built_in_tests.rs`, `built_in_names_satisfy_the_grammar`.
  Non-vacuity: a misspelt name such as `Bash` in `name()` fails the parse.
- Obligation O3 (lookup round trip and exclusivity):
  `lookup(&ShellName::from(b)) == Some(b)` for every built-in, and
  `lookup(&n) == None` for every valid name outside the four. Method:
  parameterized test for the first half; property test for the second. Domain:
  generated valid names from `[a-z][a-z0-9_-]{0,62}`, filtered to exclude the
  four built-in names (the filter rejects on the order of one sample in 50,000,
  so it cannot starve the generator), plus explicit `dash`, `zsh`, `cmd`, and
  `sh2`. Artefact: `built_in_tests.rs` and
  `src/shell_selection/name_property_tests.rs`,
  `lookup_is_none_for_non_built_in_names`. Non-vacuity: `dash` is a mandatory
  witness named by the roadmap. Negative control: a `lookup` that matches on
  prefix (`sh` for `sh2`) fails the `sh2` case.
- Obligation O4 (host support and defaults match ADR-019): the support matrix
  is exactly `sh` on Unix-like only; `bash` and `pwsh` on both; `powershell` on
  Windows only. `host_default(UnixLike) == Sh`,
  `host_default(Windows) == PowerShell`, and
  `host_default(h).is_supported_on(h)` for both hosts. Method: parameterized
  4-by-2 truth table and an `insta` snapshot. Artefact: `built_in_tests.rs`,
  `support_matrix`, `host_default_*`, and snapshot `built_in_registry_table`,
  which renders every row of Table 1 (name, hosts, executable kind and value,
  fixed arguments) from code. Non-vacuity: all eight cells are asserted, four
  true and four false. Negative control: flipping any cell fails exactly one
  case.
- Obligation O5 (fixed invocations): executables and arguments match ADR-019
  exactly: `sh` is `FixedPath("/bin/sh")` with `["-c"]`; `bash` is
  `SearchPath("bash")` with `["--noprofile", "--norc", "-c"]`; `pwsh` is
  `SearchPath("pwsh")` and `powershell` is `SearchPath("powershell.exe")`, both
  with `["-NoLogo", "-NoProfile", "-NonInteractive", "-Command"]`. Every fixed
  argument is non-empty and NUL-free; every `FixedPath` is absolute; every
  `SearchPath` has no path separator. Method: parameterized test with
  `pretty_assertions::assert_eq` on whole slices. Artefact: `built_in_tests.rs`,
  `fixed_invocations` and `invocation_shapes_are_well_formed`. Non-vacuity:
  argument order is asserted, so swapping `--norc` and `--noprofile` fails.
- Obligation O6 (host family alignment): on the compiling host,
  `ShellHost::current() == ShellHost::Windows` if and only if
  `RecipeShell::host_default() == RecipeShell::PowerShell`. Method: named unit
  test run on every CI platform. Rationale: keeps the new structured default
  and the legacy default reading the same target family. Artefact:
  `src/shell_selection/host_tests.rs`,
  `current_host_agrees_with_legacy_recipe_default`. Non-vacuity: CI runs Linux,
  macOS, and Windows jobs, so both sides of the equivalence are exercised
  across the matrix; locally only one side runs, which is a recorded residual
  gap.
- Obligation O7 (grammar): for every string `s`, `ShellName::parse(s)` is `Ok`
  if and only if `s` matches `\A[a-z][a-z0-9_-]{0,62}\z`, and on success
  `as_str() == s`. Method: property test against the `regex` oracle, plus a
  boundary table. Domain: four generators combined with `prop_oneof!`: valid
  names from the grammar's own pattern; near misses built by mutating a valid
  name (upper-case a character, prefix a digit, `_`, or `-`, insert `/`, `\`,
  `.`, space, NUL, `é`, or `{`, or extend to 64 characters); printable ASCII
  strings of length 0 to 70; and arbitrary Unicode strings. Artefact:
  `name_property_tests.rs`, `parse_agrees_with_regex_oracle`, and
  `src/shell_selection/name_tests.rs` boundary cases (empty, length 1, 63, and
  64, each forbidden first character, `dash`, `/bin/bash`, `bash.exe`,
  `C:\Windows\System32\bash.exe`, `{{ shell }}`, `bash` with a leading space,
  `Bash`). Evidence: run with the default 256 cases per property; failures
  persist to `proptest-regressions/`. Non-vacuity: a deterministic companion
  test draws 1,000 samples from each generator with a fixed seed and asserts
  that the valid generator yields only accepted strings, the near-miss
  generator yields only rejected strings, and the ASCII generator yields both
  classes. Negative controls exercised during Red-Green: changing the length
  limit to 64 fails the 64-character boundary case; allowing upper case fails
  the `Bash` case.
- Obligation O8 (name round trips): `ShellName::parse(n.as_str()) == Ok(n)`
  and JSON and YAML serialize-then-deserialize return `n`, for every valid `n`.
  Method: property test. Artefact: `name_property_tests.rs`,
  `name_round_trips`. Non-vacuity: generator is the valid-name strategy of O7,
  whose acceptance is established there.
- Obligation O9 (selector decoding is exact): decoding a JSON value `v` yields
  `Direct` if `v` is `false`, `PlatformDefault` if `v` is `true`, `Named(n)` if
  `v` is a string with `ShellName::parse` giving `n`, and an error in every
  other case (invalid-name strings, null, numbers, arrays, objects). Method:
  parameterized test over every JSON kind, and a property test over generated
  `serde_json::Value`s. Domain: a recursive `Value` strategy of depth at most 2
  covering all six kinds, with strings drawn from the O7 generators. Artefact:
  `src/shell_selection/selection_tests.rs` (`decodes_*`, `rejects_*`) and
  `selection_property_tests.rs`,
  `decoding_is_ok_exactly_for_booleans_and_valid_names`. Non-vacuity: the
  property classifies each case by kind with `prop_assert!` on an oracle
  computed independently from the value's kind and the regex, and the
  deterministic companion asserts every kind appears. Negative control: a
  decoder that treats `null` as `Direct` fails the null case; one that accepts
  numbers fails the `1` case.
- Obligation O10 (selector round trip): `decode(encode(s)) == s` for every
  selection, through `serde_json` and through `serde-saphyr`. Method: property
  test. Artefact: `selection_property_tests.rs`, `selection_round_trips`.
  Non-vacuity: the generator produces all three variants (asserted by the
  deterministic companion).
- Obligation O11 (front-end parity and YAML 1.1 pinning): for each YAML scalar
  in the table below, decoding through the manifest front-end's two stages
  (`serde_saphyr::from_str::<ManifestValue>` then
  `serde_json::from_value::<ShellSelection>`) and decoding directly with
  `serde_saphyr::from_str::<ShellSelection>` give the same result, and that
  result is the one listed. Method: parameterized test. Artefact:
  `selection_tests.rs`, `front_end_parity`. Table (YAML text to result):
  `false`, `False`, `no`, `n`, `off` to `Direct`; `true`, `TRUE`, `yes`, `y`,
  `on` to `PlatformDefault`; `bash`, `dash`, `"true"`, `'yes'`, `"n"` to
  `Named` with the unquoted text; `1`, `~`, `null`, `""`, `[bash]`,
  `{name: bash}`, `Bash`, `/bin/bash`, `"{{ shell }}"` to an error.
  Non-vacuity: every row is a witness; both paths run the real crates (AXIOM-1,
  AXIOM-2). Negative control: none needed beyond the rows, because each row
  asserts a concrete expected value rather than only parity.
- Obligation O12 (no execution change): no file outside the permitted set in
  `Constraints` changes, and the full existing suite passes. Method:
  `git diff --stat origin/main...HEAD` inspection and `make test`. Non-vacuity:
  `make test` runs the existing command-lowering and Ninja snapshot suites.

Residual gaps: O6 exercises only one side per host; no Kani or Verus obligation
exists (Decision D9); localization of errors is deferred (Decision D5); broader
cross-cutting properties over merged definitions and `PATH` states belong to
11.2.4.

## Plan of work

### Stage A: baseline (no code changes)

Confirm the working tree is clean and on the task branch. Run the four gates
once to record a green baseline; if any gate fails on the untouched tree, stop
and report it, because later failures would otherwise be ambiguous.

### Stage B and C for EP-M1: host family, built-in registry, and names

Red: create `src/shell_selection/mod.rs` containing only the module comment, the
`mod` declarations, and the `pub use` lines, and add
`pub mod shell_selection;` to `src/lib.rs` (keeping its existing ordering).
Create the three test files `host_tests.rs`, `built_in_tests.rs`, and
`name_tests.rs`, and `name_property_tests.rs`, with the tests named in
obligations O1 to O8. Create each production file (`host.rs`, `built_in.rs`,
`name.rs`) with its type declarations and signatures whose bodies return a
deliberately wrong value (for example `lookup` returning `None`, `parse`
returning `Err(ShellNameError::Empty)`), so the tests compile and fail for the
asserted reason rather than failing to compile. Run the focused tests and
confirm the failures.

Green: implement the bodies.

- `src/shell_selection/host.rs`: `ShellHost` with `UnixLike` and `Windows`,
  deriving `Clone, Copy, Debug, PartialEq, Eq, Hash`;
  `pub const ALL: [Self; 2]`; `pub const fn current() -> Self` reading
  `cfg!(windows)`. Declare its tests with
  `#[cfg(test)] #[path = "host_tests.rs"] mod tests;`.
- `src/shell_selection/built_in.rs`: `BuiltInShell` with `Sh`, `Bash`, `Pwsh`,
  `PowerShell`; `BuiltInExecutable` with `FixedPath(&'static str)` and
  `SearchPath(&'static str)`; the methods of Decision D3; `Display` writing
  `name()`. Each method is a single exhaustive `match`.
- `src/shell_selection/name.rs`: `ShellName(String)` with a private field,
  `MAX_LEN: usize = 63`, `parse`, `as_str`, `into_inner`, `TryFrom<&str>`,
  `TryFrom<String>` (validating then keeping the allocation), `FromStr`,
  `AsRef<str>`, `Display`, `Serialize` (as a string), `Deserialize` (read a
  `String`, then `parse`, mapping the error with `serde::de::Error::custom`),
  and `From<BuiltInShell>`. Derive
  `Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord`. `ShellNameError` as in
  Decision D5, deriving `Debug, Clone, PartialEq, Eq, thiserror::Error`;
  `InvalidCharacter.index` is the zero-based character index, and characters
  are displayed with `{:?}` so control characters are escaped.

Add the `insta` snapshot test in `built_in_tests.rs`, render the table as plain
text, review the pending snapshot, and accept it into `src/snapshots/`.

Refactor: remove duplication, confirm every file is under 400 lines, and run
the focused tests again, then the gates.

### Stage B and C for EP-M2: selection codec and behaviour

Red: add `src/shell_selection/selection.rs` declaring `ShellSelection` with a
`Deserialize` implementation that always returns an error and a `Serialize`
implementation that always writes `false`; add `selection_tests.rs` and
`selection_property_tests.rs` with obligations O9 to O11; add the feature file
and step module below. Run the focused tests and the behavioural harness and
confirm they fail on the asserted values.

Green: implement the visitor. `expecting` writes `a Boolean or a shell name`;
`visit_bool` maps `false` and `true`; `visit_str` (and, through serde's
defaults, `visit_string` and `visit_borrowed_str`) calls `ShellName::parse` and
maps errors with `E::custom`. Every other visitor method keeps serde's default,
which reports `invalid type`. `Serialize` writes a Boolean or the name.

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
      | yaml    | intent                   |
      | false   | direct execution         |
      | true    | the platform default     |
      | bash    | the named shell "bash"   |
      | dash    | the named shell "dash"   |
      | "true"  | the named shell "true"   |

  Scenario Outline: Any other selector is rejected
    Given the shell selector YAML <yaml>
    When the shell selector is decoded
    Then decoding the shell selector fails

    Examples:
      | yaml          |
      | 1             |
      | null          |
      | [bash]        |
      | /bin/bash     |
      | Bash          |
      | ""            |

  Scenario Outline: Each host has a supported default shell
    Given the <host> host family
    When the platform-default built-in shell is chosen
    Then the chosen built-in shell is <default>
    And the chosen built-in shell is supported on that host family

    Examples:
      | host      | default    |
      | Unix-like | sh         |
      | Windows   | powershell |
```

Step definitions live in a new `tests/bdd/steps/shell_selection.rs`, declared in
`tests/bdd/steps/mod.rs`. Add two fields to `TestWorld` in
`tests/bdd/fixtures/mod.rs`: `shell_selector_yaml: Slot<String>` and
`shell_selection_outcome: Slot<Result<ShellSelection, String>>` (or the closest
shape `Slot` supports; if `Slot` requires `Clone`, store the error as its
`String` rendering), plus `shell_host: Slot<ShellHost>` and
`chosen_built_in_shell: Slot<BuiltInShell>`. The `When` step decodes through
the same two stages as the front-end. Step functions return
`anyhow::Result<()>` and resolve failures with `?` rather than `expect`,
because helper functions outside `#[test]` bodies may not panic.

Refactor, then run the gates.

### Stage D for EP-M3: documentation and closure

Write the design-document subsection, the developers'-guide passage, and the
repository-layout entry described in Decision D10. Mark roadmap task 11.2.1 as
done by changing `- [ ] 11.2.1.` to `- [x] 11.2.1.` in `docs/roadmap.md`.
Update this plan's `Progress`, `Outcomes & retrospective`, and status. Run
`make fmt` (or the Markdown formatter on each changed file) immediately after
each Markdown edit, then the Markdown gates and the full gates.

## Milestones and plateaus

- EP-M0, baseline. Outcome: a recorded green baseline on the unmodified tree.
  Requirements: none. Acceptance: all four gate logs exit zero. Conformance
  check: not applicable. Recovery: none needed. Remaining gaps: everything.
  Compatibility decision: none.
- EP-M1, host family, built-in registry, and names. Outcome: `ShellHost`,
  `BuiltInShell`, `BuiltInExecutable`, `ShellName`, and `ShellNameError` exist
  with doctests, unit tests, the registry snapshot, and grammar properties.
  Requirements: `RM-11.2.1-A`, `-B`, `-C`, `-E`; `ADR019-T1`, `ADR019-GRAMMAR`.
  Acceptance: obligations O1 to O8 pass; gates green. Conformance check: Table
  1 unchanged; no file outside the permitted set touched; no dependency added;
  `ShellHost` scope recorded; trace links current. Recovery: the milestone is
  one or two commits on a branch; revert them to return to the baseline.
  Remaining gaps: the selection codec. Compatibility decision: none (new,
  pre-1.0 items).
- EP-M2, selection codec and behaviour. Outcome: `ShellSelection` decodes and
  encodes exactly, with front-end parity, properties, and behavioural
  scenarios. Requirements: `RM-11.2.1-D`, `-E`. Acceptance: O9 to O12 pass; the
  three scenarios pass in `tests/bdd_tests.rs`; gates green. Conformance check:
  the selection type is not referenced from `src/ast/`; YAML 1.1 behaviour is
  pinned rather than altered; trace links current. Recovery: revert the
  milestone's commits; EP-M1 remains a coherent plateau. Remaining gaps:
  documentation. Compatibility decision: none.
- EP-M3, documentation and closure. Outcome: design, developer, and layout
  documents describe the vocabulary; the roadmap marks 11.2.1 done; the plan is
  `COMPLETE`. Requirements: AGENTS.md documentation rules. Acceptance: Markdown
  gates and the four gates green. Conformance check: documentation agrees with
  ADR-019; hand-offs recorded. Recovery: revert documentation commits.
  Remaining gaps: the hand-offs listed in `Outcomes & retrospective`.
  Compatibility decision: none.

## Concrete steps

Run every command from the repository root,
`/home/leynos/.lody/repos/github---leynos---netsuke/worktrees/f900fc37-c6fe-4ed1-8700-ec572d9952c3`.
Capture gate output with `tee` and read the log rather than the truncated
terminal. Never run two gates at once; the build cache relies on sequential
runs.

Baseline and full gates:

```bash
BRANCH="$(git branch --show-current)"
make check-fmt 2>&1 | tee "/tmp/check-fmt-netsuke-${BRANCH}.out"
make typecheck 2>&1 | tee "/tmp/typecheck-netsuke-${BRANCH}.out"
make lint 2>&1 | tee "/tmp/lint-netsuke-${BRANCH}.out"
make test 2>&1 | tee "/tmp/test-netsuke-${BRANCH}.out"
```

Each ends with exit status zero. `make test` prints a nextest summary such as
`Summary [ …s] NNNN tests run: NNNN passed, 0 failed` followed by the doctest
summaries.

Focused red and green runs reuse the gate's `RUSTFLAGS` so Cargo does not
rebuild with a different fingerprint:

```bash
RUSTFLAGS="-D warnings -Zthreads=8 -Clink-arg=-fuse-ld=mold" \
  cargo nextest run --workspace --all-targets --all-features \
  -E 'test(/shell_selection/)' 2>&1 | tee "/tmp/focused-netsuke-${BRANCH}.out"
```

Expected at Red for EP-M1: failures such as
`shell_selection::built_in::tests::support_matrix::case_1 ... FAILED` with a
`pretty_assertions` diff. Expected at Green: every selected test passes.

Behavioural scenarios only:

```bash
RUSTFLAGS="-D warnings -Zthreads=8 -Clink-arg=-fuse-ld=mold" \
  cargo nextest run --workspace --all-features --test bdd_tests \
  -E 'test(/shell_selection/)' 2>&1 | tee "/tmp/bdd-netsuke-${BRANCH}.out"
```

Doctests for the new module:

```bash
RUSTFLAGS="-D warnings -Zthreads=8 -Clink-arg=-fuse-ld=mold" \
  cargo test --workspace --doc --all-features shell_selection \
  2>&1 | tee "/tmp/doctest-netsuke-${BRANCH}.out"
```

Snapshot review after adding the registry snapshot:

```bash
cargo insta pending-snapshots
cargo insta accept
```

Scope check before each milestone commit:

```bash
git --no-pager diff --no-ext-diff --stat origin/main...HEAD
```

Expected: only `src/lib.rs`, files under `src/shell_selection/`,
`src/snapshots/`, `proptest-regressions/` (only if a failure was found and
fixed), `tests/features/shell_selection_vocabulary.feature`,
`tests/bdd/steps/shell_selection.rs`, `tests/bdd/steps/mod.rs`,
`tests/bdd/fixtures/mod.rs`, and, in EP-M3, `docs/` files.

Markdown gates after documentation edits:

```bash
make fmt 2>&1 | tee "/tmp/fmt-netsuke-${BRANCH}.out"
make markdownlint 2>&1 | tee "/tmp/markdownlint-netsuke-${BRANCH}.out"
make nixie 2>&1 | tee "/tmp/nixie-netsuke-${BRANCH}.out"
```

Commit after each green milestone with an imperative subject, for example
`Add built-in shell registry and ShellName`, and gate each commit.

## Validation and acceptance

Acceptance is behavioural:

- `ShellName::parse("dash")` returns a name whose `as_str()` is `dash`, and
  `BuiltInShell::lookup` of it is `None`; `ShellName::parse("/bin/bash")`
  returns an error. Both appear as doctests on `ShellName`.
- `BuiltInShell::host_default(ShellHost::Windows)` is
  `BuiltInShell::PowerShell`, whose `fixed_args()` are `-NoLogo`, `-NoProfile`,
  `-NonInteractive`, `-Command`, and whose `executable()` is
  `SearchPath("powershell.exe")`.
- Decoding the YAML scalar `true` into `ShellSelection` gives
  `PlatformDefault`; `bash` gives `Named("bash")`; `1` gives an error whose
  text contains `expected a Boolean or a shell name`.

Red-Green-Refactor evidence to record in `Artefacts and notes` per milestone:
the focused command and its failing summary at Red; the same command passing at
Green; the four gates passing after Refactor.

Quality criteria:

- Tests: `make test` passes, including every new unit, property, snapshot,
  doctest, and behavioural test; the new tests fail at Red for the asserted
  reason.
- Verification: obligations O1 to O12 discharged as described, with the
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
the fix. To abandon a milestone, reset the branch to the previous milestone
commit; each milestone is a coherent plateau.

## Artefacts and notes

The registry snapshot should render Table 1 from code in this shape (the
accepted snapshot is authoritative once reviewed against ADR-019):

```plaintext
name        unix-like  windows  executable                 fixed arguments
sh          yes        no       fixed-path /bin/sh         -c
bash        yes        yes      search-path bash           --noprofile --norc -c
pwsh        yes        yes      search-path pwsh           -NoLogo -NoProfile -NonInteractive -Command
powershell  no         yes      search-path powershell.exe -NoLogo -NoProfile -NonInteractive -Command
unix-like default: sh
windows default: powershell
```

Red-Green-Refactor transcripts will be appended here during implementation.

## Interfaces and dependencies

No new dependency. At the end of EP-M2 these items exist in
`netsuke::shell_selection` (re-exported from `src/shell_selection/mod.rs`):

```rust
/// The host family that decides built-in shell support.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ShellHost {
    UnixLike,
    Windows,
}

impl ShellHost {
    pub const ALL: [Self; 2];
    pub const fn current() -> Self;
}

/// How a built-in shell's executable is located.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BuiltInExecutable {
    /// A fixed absolute path used without searching `PATH`.
    FixedPath(&'static str),
    /// A bare executable name searched on the trusted host `PATH` by 11.2.3.
    SearchPath(&'static str),
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
    pub fn lookup(name: &ShellName) -> Option<Self>;
    pub const fn is_supported_on(self, host: ShellHost) -> bool;
    pub const fn executable(self) -> BuiltInExecutable;
    pub const fn fixed_args(self) -> &'static [&'static str];
    pub const fn host_default(host: ShellHost) -> Self;
}

/// A validated shell registry name matching `[a-z][a-z0-9_-]{0,62}`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ShellName(String);

impl ShellName {
    pub const MAX_LEN: usize = 63;
    pub fn parse(text: &str) -> Result<Self, ShellNameError>;
    pub fn as_str(&self) -> &str;
    pub fn into_inner(self) -> String;
}
// Also: TryFrom<&str>, TryFrom<String>, FromStr, AsRef<str>, Display,
// Serialize, Deserialize, From<BuiltInShell>.

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ShellNameError {
    Empty,
    TooLong { length: usize },
    InvalidFirstCharacter { character: char },
    InvalidCharacter { character: char, index: usize },
}

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

The `const fn` markings are intended; if Clippy's `missing_const_for_fn`
requests `const` on further functions, apply it, and if a `const` is impossible
(for example `lookup`, which compares strings), leave it off.

`ShellSelection` has no method that resolves anything. Resolution into an
executable path belongs to 11.2.3, and the IR's `ResolvedShell` belongs to
11.3.1.

[^1]: GitHub Docs, "Workflow syntax for GitHub Actions",
    `jobs.<job_id>.steps[*].shell`,
    <https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax>,
    retrieved 2026-09-24.

## Revision note

- 2026-09-24: initial draft for review.
