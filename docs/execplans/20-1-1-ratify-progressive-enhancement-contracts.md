# 20.1.1. Ratify the progressive-enhancement contracts and version gates

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: DRAFT

This plan is approval-gated. Do not begin implementation until the user
explicitly approves it. Approval of this plan also ratifies the proposed
decisions recorded under `Decision log`, except for any decision the approver
names as struck or amended.

## Purpose / big picture

Roadmap item 20.1.1 in `docs/roadmap-progressive-enhancement.md` asks for the
five progressive-enhancement proposals, RFCs 0021 to 0025, to be reviewed and
their outstanding schema questions settled before any of their syntax is
implemented. It also asks for one version-allocation protocol that the
structured-command work (12.1.1), the include work (16.1.1), and the bundle
work (17.1.1) can share without waiting for one another, and for explicit rules
on who owns the command "operation union", how Netsuke reports which optional
features it supports, and how it rejects syntax it does not support.

After this work, three things are observable:

1. Two new Architectural Decision Records (ADRs) exist and are accepted. The
   first, ADR-041, fixes what `netsuke_version` means, how later schema
   versions are allocated, and how persisted formats are versioned. The second,
   ADR-042, records the shallow-end contract, ownership of the command
   operation union, feature-keyed capability reporting, and a resolution or an
   explicit owner for every outstanding decision listed in RFCs 0021 to 0025.
   Those five RFCs change status from `Proposed` to `Accepted`.
2. `docs/netsuke-design.md` contains one authoritative schema-version registry
   that reserves every planned top-level key and operation key and shows which
   manifest schema version admits each feature. Today every row is "reserved",
   and the only admitted schema is `1.0`.
3. Netsuke itself begins enforcing the version gate that every later feature
   depends on. Today the binary accepts any syntactically valid SemVer string in
   `netsuke_version`, including `"2.0.0"`, and it does not even read the
   version until after Jinja templates have been evaluated. After this work, a
   manifest declaring a schema version this release does not support fails
   immediately after YAML parsing with a localized diagnostic carrying the
   stable code `netsuke::manifest::schema_version`, and no template, `env()`
   lookup, stdlib command helper, or Ninja process runs. A manifest declaring
   `"1.0.0"`, which is every manifest in the quickstart, the examples, and the
   test corpus except two accidental fixtures, behaves exactly as before.

The third outcome is the only behavioural change. It is included because the
RFC set requires that "older readers must reject new syntax with a useful
version remedy rather than ignore it" (RFC 0022 section 8). The binaries that
will be the "older readers" of schema `1.1` are the releases shipped between
now and that feature's delivery. Unless the reader-side gate ships first, those
binaries will evaluate a newer manifest's templates and then fail with a
generic unknown-field error, or, worse, succeed on a manifest whose new keys
happen to be absent from the evaluated subset. A reviewer who prefers to keep
20.1.1 documentation-only may strike milestone EP-M3 at approval; see decision
D-15.

## Constraints

- Do not implement until the user explicitly approves this plan.
- Preserve the shallow end. The quickstart manifest in RFC 0025 section 2 and
  `docs/quickstart.md` must parse, generate, and build with identical output.
  No existing manifest declaring `netsuke_version: "1.0.0"` may change parse
  result, diagnostics, generated Ninja text, or execution behaviour.
- Do not implement any syntax proposed by RFCs 0021 to 0025 or by RFCs 0001,
  0002, 0003, 0004, or 0029. No `inputs`, `states`, `artefacts`,
  `contention_classes`, `maturity`, `includes`, `bundles`, or `host_facts` key,
  and no `require_state`, `ensure_state`, `prepare_state`, `clean_owned`,
  `produces`, or `contention` field, may be parsed or admitted by code in this
  change. Reservation happens in documentation only.
- Do not add a feature registry type, capability-reporting JSON field, CLI
  flag, configuration key, or `context`/`check` command. Those belong to
  20.2.1, phase 5, and issue `#592` respectively.
- Do not change the status of RFCs 0001 to 0004 or RFC 0029. Their
  ratification belongs to 12.1.1, 16.1.1, 17.1.1, and RFC 0029's own roadmap
  item. This plan may add cross-references to them only in roadmap text.
- The version gate must be pure domain policy with no I/O. It must not read
  the process environment, the filesystem, or configuration. Follow the
  environment-injection rules in `AGENTS.md` and ADR-008: tests must not mutate
  the process environment.
- Keep semantic error facts typed and render them only at the diagnostics
  boundary through Fluent, as ADR-035 requires. Every new Fluent key must exist
  in all 35 catalogues under `locales/`, with the machine-readable code inside
  the message text, or `tests/build_l10n_audit_tests.rs` fails.
- Do not change the public signature of `netsuke::manifest::from_str`,
  `from_str_with_env`, `from_path`, or `ManifestError`. The new diagnostic is
  carried as the `source` of the existing `ManifestError::Parse` variant.
- No new dependencies. `semver`, `rstest`, `rstest-bdd`, `proptest`, `insta`,
  `googletest`, and `pretty_assertions` are already available; confirm each in
  `Cargo.toml` before use.
- No file may exceed 400 lines. Every new module starts with a `//!` comment,
  and every new function carries a `///` comment, as `AGENTS.md` requires.
- ADR numbers must be re-swept across every remote branch immediately before
  the ADR files are created. At planning time ADR-039 and ADR-040 are taken on
  other branches, ADR-030 and ADR-031 are unused but unexplained, and ADR-041
  and ADR-042 are free.

## Tolerances (exception triggers)

- Scope: if EP-M3 needs to touch more than 12 Rust or test files, excluding
  the 35 locale catalogues and snapshot files, or more than 500 net lines of
  Rust, stop and escalate.
- Interface: if any public function signature or public enum variant must
  change, stop and escalate.
- Compatibility: if any fixture, example, README fence, or snapshot outside
  `tests/data/jinja_is.yml` and `tests/data/jinja_is_missing.yml` declares a
  version the gate would reject, stop and list it before changing it. If a
  downstream canary manifest (issue `#598`, PR `#780`) is found to declare
  anything other than `1.0.x`, stop and escalate.
- Ordering: if the gate cannot run before `register_manifest_vars` without
  restructuring `evaluate_manifest` beyond inserting one call, stop and
  escalate.
- Decisions: if a reviewer strikes or amends a decision in the `Decision log`,
  record the amendment, leave the affected ADR at `Proposed` with the item under
  `Outstanding decisions`, and continue with the remaining milestones.
- Iterations: if the focused tests still fail after three fix attempts in one
  milestone, stop and escalate.
- ADR numbering: if ADR-041 or ADR-042 has been taken by the time EP-M1
  starts, take the next free numbers, update every reference in this plan, and
  note the change in `Decision log`; this does not require escalation.

## Risks

- Risk: the version gate rejects a manifest in the wild that declares a
  version other than `1.0.x`, such as the `0.1.0` found in two test fixtures.
  Severity: medium. Likelihood: low. Mitigation: every documentation example
  and the quickstart use `1.0.0`; the diagnostic names the supported range and
  the exact remedy; the users' guide gains a section; the change is recorded
  under the next release's notes.

- Risk: ratified schema decisions later prove wrong during implementation of
  phases 21 to 25. Severity: medium. Likelihood: medium. Mitigation: ADR-042
  separates schema decisions, which it settles, from implementation contracts,
  which it assigns to a named roadmap task. Nothing is admitted by code, so a
  later ADR can supersede a decision before release without migration.

- Risk: 12.1.1, 16.1.1, or 17.1.1 is being planned concurrently and allocates
  `1.1.0` on its own. Severity: medium. Likelihood: medium. Mitigation: ADR-041
  makes allocation happen at admission merge time, not at ratification, so a
  concurrent plan's provisional number is harmless; the roadmap entries for
  those tasks gain a one-line pointer to ADR-041.

- Risk: translating three new messages into 34 non-English catalogues
  introduces poor translations. Severity: low. Likelihood: medium. Mitigation:
  follow `docs/translators-guide.md`; keep messages short and built from
  existing catalogue vocabulary (`netsuke_version`, "manifest", "supported");
  keep identifiers and version numbers untranslated.

- Risk: moving the version check earlier changes the diagnostic produced for
  the existing `tests/data/invalid_version.yml` and for a missing
  `netsuke_version`. Severity: low. Likelihood: high. Mitigation: that is
  intended; EP-M3 updates the affected assertions deliberately, and a snapshot
  shows the new diagnostic.

- Risk: `scenarios!` in `tests/bdd_tests.rs` sweeps the whole
  `tests/features` directory, so a red scenario fails the whole BDD target.
  Severity: low. Likelihood: high. Mitigation: observe red locally with a
  scenario filter, and commit only at green.

## Progress

- [x] (2026-09-27) Renamed the branch to
  `20-1-1-ratify-progressive-enhancement-contracts` and set its upstream.
- [x] (2026-09-27) Reconnaissance: roadmap coordination points, current
  manifest-version handling, ADR process, and prior art.
- [x] (2026-09-27) First draft of this plan.
- [ ] Expert review of the draft and revision.
- [ ] Plan approved by the user.
- [ ] EP-M0: re-sweep ADR numbers; record baseline gate results.
- [ ] EP-M1: ADR-041 and the design-document schema registry.
- [ ] EP-M2: ADR-042, RFC status changes, and roadmap cross-references.
- [ ] EP-M3: reader-side schema version gate (red, green, refactor).
- [ ] EP-M4: users' guide, developers' guide, contents, roadmap completion.

## Surprises & discoveries

- Observation: Netsuke has no version gate at all. `NetsukeManifest` declares
  `netsuke_version: semver::Version` (`src/ast/mod.rs`), and the only failure
  is a SemVer syntax error. `"2.0.0"` is accepted today. Evidence: no
  `VersionReq`, supported-version constant, or comparison exists under `src/`;
  `tests/data/invalid_version.yml` fails only because `"1"` is not full SemVer.
  Impact: every RFC's "older readers reject new syntax" requirement is
  currently unmet by the shipped reader. This motivates EP-M3.

- Observation: the version is read after template evaluation. In
  `evaluate_manifest` (`src/manifest/mod.rs`), YAML is parsed into a
  `serde_json::Value`, then variables and macros are registered, `foreach` and
  `when` are expanded through MiniJinja, and only then is the value
  deserialized into `NetsukeManifest`. Evidence: `evaluate_manifest` calls
  `register_manifest_vars`, `register_manifest_macros_with_budget`, and
  `expand_foreach_with_budget` before `serde_json::from_value(doc)`. Impact: an
  unsupported manifest can reach `env()`, `glob()`, the stdlib command helpers
  in `src/stdlib/command`, and the network helpers in `src/stdlib/network`
  before it is rejected. The gate must run immediately after YAML parsing.

- Observation: two test fixtures declare `netsuke_version: "0.1.0"`, and the
  snapshot-testing guide shows `"0.1"`, which is not valid SemVer. The
  test-framework design document shows `"1.2.0"` as an illustrative future
  version. Evidence: `git grep` over the tree. Impact: the fixtures must move to
  `1.0.0` in EP-M3. The guide example is already invalid and is corrected in
  EP-M4. The design-only test-framework document is left alone, but ADR-041
  notes that its number is illustrative.

- Observation: no RFC in `docs/rfcs/` has ever left `Proposed`.
  Evidence: every RFC preamble reads `Status: Proposed`. Impact: RFCs 0021 to
  0025 will be the first accepted RFCs. The style guide permits a later process
  to promote an RFC; this plan's approval is that process, and each accepted
  RFC names its ratifying ADRs.

- Observation: `context` and `check` do not exist yet. `Commands` in
  `src/cli/command.rs` has `build`, `clean`, `graph`, `generate`, and `help`.
  Evidence: `src/cli/command.rs`; `netsuke check` is in open PR `#621` (issue
  `#592`). Impact: capability reporting can be specified here, but its first
  observable surface is the diagnostic itself. The JSON reporting shape stays
  with 20.2.1.

- Observation: Ninja accepts `depth = 0` for a pool and treats it as an
  unlimited pool, rejects negative depths, rejects an undeclared pool name, and
  refuses a build file whose `ninja_required_version` exceeds its own version.
  Evidence: probe with Ninja 1.11.1 in `/tmp/ninja-probe-netsuke`:
  `ninja: error: build.ninja:5: unknown pool name 'nope'`,
  `ninja: error: build.ninja:2: invalid pool depth`, and
  `ninja: fatal: ninja version (1.11.1) incompatible with build file
  ninja_required_version version (99.0).`
  Impact: RFC 0024's rejection of capacity zero is load-bearing, because
  passing zero through would silently remove the limit. Recorded in D-11.

## Decision log

All decisions below are proposed. Approval of this plan accepts them unless the
approver strikes or amends one. Each names the ADR that records it.

- Decision D-1 (ADR-041): `netsuke_version` is the manifest schema version,
  not the Netsuke release version. It uses SemVer syntax. A reader supports
  exactly one major version, `1`, and a contiguous range of minor versions from
  `0` to a release-specific maximum, which is `0` today. The patch component is
  never allocated for syntax and is ignored for admission, so `1.0.3` is
  admitted. Build metadata is ignored, as SemVer requires. Any prerelease
  identifier is rejected, because schema versions have no prereleases. Major
  `0` and majors above `1` are rejected. Rationale: the design document already
  calls it "the version of the Netsuke schema", and every example uses `1.0.0`.
  Rejecting prereleases and ignoring patch keeps the admitted set expressible
  as the SemVer requirement `>=1.0.0, <1.(MAX+1).0`, which gives the tests an
  independent oracle. Date/Author: 2026-09-27, planning agent.

- Decision D-2 (ADR-041): admission is checked on the literal
  `netsuke_version` value immediately after YAML parsing and before variable
  registration, macro registration, `foreach`/`when` expansion, or any other
  template evaluation. A missing key, a non-string value, and an invalid SemVer
  string are rejected at the same point. The value is never templated.
  Rationale: an older reader must not evaluate a newer manifest's templates,
  which can call `env()`, `glob()`, stdlib command helpers, and network
  helpers. Rejecting before evaluation is the only way to guarantee that an
  unsupported manifest has no side effects. Date/Author: 2026-09-27, planning
  agent.

- Decision D-3 (ADR-041): version allocation is by admission, not by
  ratification. Ratification reserves names in the design-document registry. A
  schema minor version is allocated only when the change that makes a feature's
  syntax admissible merges to `main`. Every feature admitted between two
  releases shares the same next minor version, so a release introduces at most
  one new schema minor. A feature's admission is the last change in its
  delivery: its syntax stays rejected as unknown until its validation is
  complete. Provisional numbers in RFCs (RFC 0001 section 19.2's `1.1.0`, RFC
  0003's `manifest: ">=1.1.0, <2.0.0"` example) are illustrative. Rationale:
  this lets 12.1.1, 16.1.1, 17.1.1, and phases 21 to 25 ratify in any order and
  deliver in any order without renumbering, and without the bundle work having
  to exist first. Date/Author: 2026-09-27, planning agent.

- Decision D-4 (ADR-041): unsupported syntax is rejected through a fixed
  ladder, evaluated in this order, and never ignored:
  1. The declared schema version is outside the reader's supported range:
     reject before evaluation with a reader remedy ("this Netsuke supports
     schema 1.0 to 1.N; upgrade Netsuke or declare a supported version").
  2. The declared version is supported, but a key belongs to a registered
     feature admitted in a later minor than the one declared: reject with an
     author remedy naming the feature ID and its minimum version. This rung is
     implemented by the first feature that is admitted, not by this plan.
  3. The key is unknown to every registered feature: keep the existing serde
     unknown-field error.
  4. The syntax is known but unsupported in context, such as `contention` on
     a rule or a state operation in an incremental target: reject with a
     feature-specific unsupported-construct diagnostic.
  Rationale: this gives each audience the remedy it can act on and keeps
  version bumps as the opt-in for new vocabulary. Date/Author: 2026-09-27,
  planning agent.

- Decision D-5 (ADR-041): there is no per-feature opt-in list in the
  manifest, in the style of Cargo's `cargo-features = [...]`. Raising
  `netsuke_version` is the only opt-in. Rationale: a feature list would add
  vocabulary to every advanced manifest and duplicate the version signal; the
  version already tells an older reader to stop. Date/Author: 2026-09-27,
  planning agent.

- Decision D-6 (ADR-041): persisted formats are versioned independently of
  the manifest schema. The action plan of RFC 0001 (owned by 12.3.1) carries an
  integer `format_version`, starting at `1`, bumped on any change to its
  encoding or meaning, and readers accept only an exact match, failing closed
  before spawning with a "regenerate" remedy. State records of RFC 0021 are
  stored under a versioned directory namespace (for example `state/v1/`) so
  that different Netsuke versions never overwrite each other's records; a
  record whose version is unknown to the reader counts as absent evidence,
  never as success. The JSON output envelope (`SCHEMA_VERSION` in
  `src/json_envelope.rs`) evolves additively within version `1`, with the
  details owned by 20.2.1. Dyndep sidecars keep ADR-012's contract.
  Date/Author: 2026-09-27, planning agent.

- Decision D-7 (ADR-041 and ADR-042): capability reporting is keyed by stable
  feature identifiers, not by version numbers alone. The identifiers are
  `structured-commands`, `includes`, `bundles`, `host-facts`, `inputs`,
  `contention`, `states`, `artefacts`, and `maturity`. Diagnostics for rungs 1,
  2, and 4 name the feature identifier where one applies. 20.2.1 specifies the
  JSON reporting shape in the existing envelope; phase 5's `context` command
  exposes it. Maturity rule identifiers map to feature identifiers, so a policy
  that selects a rule for an unsupported feature fails with a version or
  capability remedy rather than being skipped. Date/Author: 2026-09-27,
  planning agent.

- Decision D-8 (ADR-042): ownership of the command operation union. Phase 12
  (RFC 0001) owns the single closed command union: its abstract syntax tree
  (AST), parser, action-plan codec, and runner dispatch. RFC 0021 contributes
  the `require_state`, `ensure_state`, and `prepare_state` variants and RFC
  0023 contributes `clean_owned`; each owns its variant's validation and
  semantics but not a private parser, codec, or runner. These four single-key
  names are reserved now in the union's key namespace alongside RFC 0001's
  rule-reference and script items. The operations exist only in structured
  command form, so phases 23 and 24 depend on 12.1.2 and 12.3.1. Date/Author:
  2026-09-27, planning agent.

- Decision D-9 (ADR-042): the shallow-end contract in RFC 0025 section 2 is a
  release acceptance requirement for every schema-`1.x` release. Optional
  top-level keys default to empty. Their absence produces no message at any
  verbosity. Declaring a feature on one node never requires declarations on
  another. 20.1.2 supplies the fixtures that enforce this. Date/Author:
  2026-09-27, planning agent.

- Decision D-10 (ADR-042, RFC 0021 schema): state `kind` is the closed set
  `directory`, `file`, `python-venv`, and `custom`. When `probe.external` is
  present, `probe.protocol` is required and is the closed set `nagios`, so a
  future protocol is additive. `probe.timeout_seconds` and
  `probe.max_output_bytes` are positive integers defaulting to 10 and 65 536.
  State operations are accepted only in always-run nodes: actions, which are
  already phony, and targets with `always: true`. Operator probe policy uses
  the flat, trust-aware configuration pattern of ADR-021: `probe_external_deny`
  (Boolean, default `false`; a project may tighten but not loosen),
  `probe_timeout_ceiling_seconds`, and `probe_output_ceiling_bytes`. Deferred
  to named owners: the `python-venv` inspection contract and any
  interpreter-constraint field (23.1.2), the lease implementation (20.2.1 and
  20.2.3), record retention limits (23.2.2), and executable-identity
  restriction (23.3.2). Date/Author: 2026-09-27, planning agent.

- Decision D-11 (ADR-042, RFC 0024 schema): `capacity` is an integer from 1
  to the maximum accepted `jobs` value (currently 64, per
  `docs/sample-netsuke.toml`) and shares that constant, because a larger class
  cannot constrain a Netsuke-driven Ninja run. Zero is rejected because Ninja
  treats `depth = 0` as unlimited. The operator ceiling is the flat key
  `contention_capacity_ceiling`, which a project may lower but not raise.
  Public class names are never emitted into Ninja text directly: lowering goes
  through ADR-014's backend escaping seam to a reserved `netsuke_pool_` prefix
  with an injective, declaration-order-independent encoding, whose exact
  spelling 22.1.2 fixes and property-tests. `console` and the `netsuke_pool_`
  prefix are reserved class names. A `contention` field on a rule is rejected
  with an unsupported-construct diagnostic in the first version; action and
  target annotations only. Date/Author: 2026-09-27, planning agent.

- Decision D-12 (ADR-042, RFC 0023 schema): the canonical spellings are the
  top-level key `artefacts` and the CLI option `--artefact`, matching the RFC
  and the project's en-GB-oxendict convention. There is no `artifacts` alias;
  24.1.1 adds a "did you mean" hint for it. `kind` defaults to `file`, `role`
  defaults to `generated`, and `create_parent` defaults to `false`. Ownership
  conflict rules: an artefact root may not equal, contain, or lie inside
  another artefact root; a directory root may not contain a protected path (the
  manifest, loaded configuration files, `.git`, Netsuke runtime directories,
  local runtime scripts, or any path named in a target's `sources`); an
  artefact equal to an existing target output is valid only with the same
  producer and a compatible object kind. Operator limits use flat keys
  `clean_max_entries`, `clean_max_depth`, `clean_max_bytes`, and
  `clean_max_seconds`, with project tightening only. Deferred to named owners:
  platform deletion primitives (24.2.1), resource-lease identity (20.2.1), and
  default numeric limits (24.1.2, set from measurement). Date/Author:
  2026-09-27, planning agent.

- Decision D-13 (ADR-042, RFC 0022 schema): the integer contract is signed
  64-bit, which is the range YAML and `serde_json` already carry; the path
  contract is the UTF-8 lexical path grammar of RFC 0003 section 6, extracted
  by 17.1.3 and shared rather than duplicated. `--input NAME=VALUE` is a
  repeatable option registered once in ADR-016's canonical command metadata and
  attached to every subcommand that loads a manifest; it is not a top-level
  global. The `inputs` Jinja namespace exists only when a manifest declares
  `inputs`. Profile overlay order stays with phase 5, which must reconcile it
  with RFC 0022 section 5's precedence before 21.2.1. Date/Author: 2026-09-27,
  planning agent.

- Decision D-14 (ADR-042, RFC 0025 schema): project policy lives in the
  manifest's root `maturity.rules` list and is untrusted: it can strengthen but
  never weaken an operator floor. Operator floors come only from trusted
  configuration layers, using ADR-021's trust classification; their exact
  configuration shape belongs to 25.2.1. Severity is `off < warn < error` and
  combines monotonically. Selected-closure inspection metadata belongs to
  25.2.2, and the inventory boundary to 25.1.1 with issue `#592`. Date/Author:
  2026-09-27, planning agent.

- Decision D-15 (scope): EP-M3, the reader-side gate, is in scope for 20.1.1
  because the roadmap item ratifies "version gates" and the gate has to ship
  before any feature for older readers to behave as the RFCs require. The gate
  enforces rung 1 of D-4 only. A reviewer may strike EP-M3 at approval; in that
  case EP-M4 omits the users' guide section and ADR-041 names 12.1.2 as the
  gate's owner. Date/Author: 2026-09-27, planning agent.

- Decision D-16 (verification): no Kani harness and no Verus proof. The
  admission predicate is a loop-free comparison over `semver::Version`, whose
  prerelease identifiers are heap strings; a property test with an independent
  `semver::VersionReq` oracle exercises the same domain more cheaply.
  `docs/formal-verification-methods-in-netsuke.md` limits Verus to proof
  kernels outside the manifest layer. See `Verification plan`. Date/Author:
  2026-09-27, planning agent.

## Outcomes & retrospective

Not started. Complete this section at each milestone boundary and at
completion, reconciling any discovery with the artefacts in `Conformance basis`.

## Context and orientation

Netsuke is a build-system compiler. It reads a YAML manifest called a
`Netsukefile`, expands its Jinja templates, builds an intermediate
representation, and writes a Ninja build file, which it then runs with Ninja.
`docs/netsuke-design.md` is the architecture reference; section 2.2 lists the
top-level manifest keys and section 3.3 describes the ingestion stages.

Terms used in this plan:

- Schema version: the value of `netsuke_version` at the top of a manifest.
  It names the manifest language version, not the Netsuke release.
- Reader: a Netsuke binary loading a manifest. An older reader is a binary
  released before some schema feature existed.
- Admission: the decision, before any evaluation, whether this reader
  accepts a manifest's declared schema version.
- Feature identifier: a stable kebab-case name for one optional vocabulary,
  such as `states`, used in diagnostics and capability reports.
- Operation union: the closed set of item shapes allowed inside a structured
  `command:` block, defined by RFC 0001.
- Shallow end: the smallest useful Netsuke manifest and workflow, which must
  keep working without any optional feature.

The proposals under ratification are:

- `docs/rfcs/0021-managed-states-and-probes.md`: optional `states`, built-in
  probes, Nagios-style external probes, and three state operations.
- `docs/rfcs/0022-typed-task-inputs.md`: optional typed root `inputs` and
  `--input NAME=VALUE`.
- `docs/rfcs/0023-artefact-ownership-and-scoped-cleanup.md`: optional
  `artefacts`, `produces`, `clean_owned`, and `clean --artefact`.
- `docs/rfcs/0024-named-contention-classes.md`: optional `contention_classes`
  lowered to Ninja pools.
- `docs/rfcs/0025-progressive-enhancement-and-maturity-policies.md`: the
  shallow-end contract and opt-in `maturity` policies.

Each ends with an "Alternatives and outstanding decisions" section whose final
paragraph lists what must be ratified before implementation. Every item in
those paragraphs is resolved by a decision above or assigned to a named task.

The code touched by EP-M3 is small:

- `src/manifest/mod.rs`: `evaluate_manifest` parses YAML into `doc`, a
  `serde_json::Value` aliased as `ManifestValue`, then evaluates templates,
  then deserializes. The gate call goes directly after the YAML parse.
- `src/manifest/diagnostics/mod.rs`: `ManifestError::Parse { source, message }`
  carries a boxed `miette::Diagnostic`. `DataDiagnostic` shows the pattern for
  a coded source diagnostic.
- `src/localization/keys.rs`: the `define_keys!` block declares Fluent keys.
- `locales/*/messages.ftl`: 35 catalogues; the build-time audit checks that
  every key exists in each with the same variables.
- `tests/data/`: manifest fixtures; `tests/ast_tests/manifest_files.rs` runs
  the invalid-fixture table; `tests/features/manifest.feature` holds the
  manifest BDD scenarios, whose steps live under `tests/bdd/steps/`;
  `tests/assert_cmd_tests.rs` holds subprocess end-to-end tests.

## Conformance basis

Upstream artefacts, at `main` commit `ebcedaef`:

- `docs/roadmap-progressive-enhancement.md`, item 20.1.1 and its three
  sub-bullets, identified here as RM-a (review, resolve, record), RM-b
  (coordinate version allocation), and RM-c (operation-union ownership,
  capability reporting, and rejection of unsupported syntax).
- RFC 0025 sections 2 and 3 (SE: the shallow-end contract; OWN: relationship
  to existing plans). RFC 0022 section 8 (OLD-READER: older readers reject new
  syntax with a version remedy). RFC 0021 sections 8 and 10, RFC 0022 section
  9, RFC 0023 section 9, RFC 0024 section 9, and RFC 0025 section 10 (OD-21 to
  OD-25: the outstanding-decision paragraphs).
- RFC 0001 section 19.2 (provisional `1.1.0`) and section 17.4 (versioned
  action plans); RFC 0003's descriptor example; RFC 0029's statement that
  `host_facts` needs an additive version allocated at implementation time.
- Roadmap items 12.1.1 and 12.3.1 in `docs/roadmap.md`, and 16.1.1 and
  17.1.1 in `docs/roadmap-composition.md`.
- ADR-008 (environment seams), ADR-012 (dyndep retention), ADR-014 (backend
  escaping seam), ADR-016 (canonical CLI metadata), ADR-021 (trust-aware policy
  merge), ADR-035 (typed semantic facts, proposed).
- Governing standards: `AGENTS.md`, `docs/documentation-style-guide.md`,
  `docs/translators-guide.md`, and
  `docs/formal-verification-methods-in-netsuke.md`.
- There is no Terms of Reference document on `main`; PR `#786` proposes one.
  This plan does not depend on it.

Trace links:

```plaintext
RM-a -> OD-21..OD-25 -> D-10..D-14 -> ADR-042 -> EP-M2 -> RFC status lines + ADR-042 resolution table
RM-b -> RFC0001-19.2, RFC0003, RFC0029 -> D-1, D-3, D-6 -> ADR-041 -> EP-M1 -> design registry table
RM-c -> RFC0025-2/3 -> D-4, D-7, D-8, D-9 -> ADR-041/ADR-042 -> EP-M1/EP-M2
OLD-READER -> D-1, D-2, D-4 rung 1, D-15 -> EP-M3 -> tests::schema_version (OBL-1..OBL-4)
SE -> D-9 -> EP-M3 regression evidence (OBL-3) -> 20.1.2 fixtures (later)
```

## Verification plan

EP-M1, EP-M2, and EP-M4 change documentation only and introduce no runtime
invariant. Their verification is the Markdown gates plus a reviewer check that
every item in OD-21 to OD-25 appears in ADR-042's resolution table with either
a decision or an owner. EP-M3 introduces the obligations below.

Axioms relied on:

- AXIOM-1: `semver::Version::parse` implements SemVer 2.0.0 syntax, and
  `semver::VersionReq::matches` excludes prerelease versions unless a
  comparator names a prerelease on the same major, minor, and patch. This is the
  `semver` crate's documented behaviour; it is not re-verified.
- AXIOM-2: `serde_saphyr::from_str` into `serde_json::Value` performs no
  template evaluation. Jinja evaluation happens only through the MiniJinja
  environment built afterwards in `evaluate_manifest`.
- AXIOM-3: the injected `EnvReader` passed to `from_str_with_env` is the only
  path by which template `env()` reads variables, so counting its calls
  observes whether templates were evaluated.

Obligations:

- Obligation OBL-1, admission partition: `admit(v)` succeeds exactly when
  `v.major == 1`, `v.minor <= SUPPORTED_MAX_MINOR`, and `v.pre` is empty; patch
  and build metadata never affect the outcome. Method: an `rstest` table plus a
  `proptest` property against an independent oracle. Rationale: the table pins
  the named equivalence classes a reader will meet; the property checks the
  whole generated domain against a formulation that shares no code with the
  implementation. Domain: table rows `1.0.0`, `1.0.7`, `1.0.0+build.5`, `1.1.0`,
  `2.0.0`, `0.1.0`, `0.0.0`, `1.0.0-rc.1`, and `1.1.0-alpha`; generated
  versions with each numeric part drawn from `0..=3` or `any::<u64>()`,
  prereleases present or absent, build metadata present or absent. Artefact:
  unit tests in `src/manifest/schema_version.rs` (or a
  `schema_version_tests.rs` sibling if the module would exceed 400 lines).
  Evidence: `cargo nextest run --workspace --all-features schema_version`
  passes; the oracle is
  `VersionReq::parse(">=1.0.0, <1.{SUPPORTED_MAX_MINOR + 1}.0")` applied to the
  version with build metadata cleared. Non-vacuity: the generator weights small
  numbers so both admitted and rejected cases occur; the property records
  `prop_assume!`-free classification counts through `proptest`'s `prop_oneof!`
  branches and the table itself contains witnesses of both outcomes. Negative
  control: changing `<=` to `<` in the minor comparison rejects `1.0.0` and
  must fail row one; dropping the prerelease check admits `1.0.0-rc.1` and must
  fail the oracle.

- Obligation OBL-2, rejection precedes evaluation: for any manifest whose
  declared version is missing, non-string, invalid, or unsupported, loading
  fails with the `netsuke::manifest::schema_version` code and the injected
  environment reader is called zero times, regardless of the rest of the
  document. Method: a named red test first, then a `proptest` property.
  Rationale: this is the safety property that motivates the gate; examples
  alone cannot show that arbitrary bodies, including unknown keys, never win.
  Domain: manifests with a generated rejected version and a body containing a
  `foreach` whose expression calls `env('NETSUKE_SENTINEL')`, plus up to three
  generated unknown top-level keys. Artefact:
  `tests/schema_version_gate_tests.rs`. Evidence: before EP-M3's green step,
  the named test fails because the sentinel reader records one or more calls
  and the error code is the structure error; after, it passes. Non-vacuity: the
  red run proves the sentinel is reachable today. A sibling test with `1.0.0`
  asserts that the sentinel is called at least once, so the counter is live.
  Negative control: moving the gate call after `expand_foreach_with_budget`
  must make the property fail.

- Obligation OBL-3, supported manifests unchanged: every manifest declaring
  a `1.0.x` version loads, generates, and builds as before. Method: the
  existing suite as a regression oracle. Rationale: the corpus of fixtures,
  examples, README fences, and snapshots is the shallow-end evidence available
  before 20.1.2. Artefact: the existing tests and `insta` snapshots; no
  snapshot outside the new ones may change. Evidence: `make test` passes, and
  `git status --porcelain '*.snap'` shows only the new snapshot files.
  Non-vacuity: the corpus contains more than 150 manifests declaring `1.0.0`;
  the two `0.1.0` fixtures are deliberately migrated.

- Obligation OBL-4, observable command-line behaviour: running
  `netsuke generate` against a manifest declaring `1.1.0` exits non-zero,
  prints the localized diagnostic with the stable code, and writes no Ninja
  file; with `--json` (or `json = true`), the diagnostic document carries the
  same code. Method: an `assert_cmd` subprocess test and an `insta` snapshot of
  the JSON diagnostic. Artefact: a new case in `tests/assert_cmd_tests.rs` (or
  a sibling module if that file is at its size limit) and a new snapshot beside
  the existing `diagnostic_json` snapshots. Evidence: red before the gate,
  because the manifest is accepted today; green after. Non-vacuity: a paired
  `1.0.0` run in the same test writes the Ninja file.

Residual gap: rungs 2 to 4 of D-4 are not implemented here and therefore not
verified here. The first admitted feature owns their tests.

## Plan of work

Stage A, orientation and baseline (EP-M0). Re-sweep ADR numbers across every
remote branch, confirm that 041 and 042 are free, and record the baseline gate
results so later failures can be attributed.

Stage B, ratification documents (EP-M1 and EP-M2). Write ADR-041 and the
registry, then ADR-042, the RFC status changes, and the roadmap
cross-references. Each milestone ends with the full gate set and a commit.

Stage C, the gate (EP-M3). Add the red tests and observe their failure,
implement the pure admission policy and its diagnostic, migrate the two `0.1.0`
fixtures, and refactor. Gate and commit.

Stage D, documentation and close-out (EP-M4). Update the users' guide,
developers' guide, design document ingestion section, and contents index; mark
20.1.1 done; complete this plan's living sections.

## Milestones and plateaus

### EP-M0: baseline

- Outcome: numbers confirmed and baseline gates recorded; no file changes
  other than this plan's `Progress`.
- Acceptance evidence: the sweep command output in `Artefacts and notes`;
  scrutineer's baseline log paths.
- Recovery: none needed.

### EP-M1: ADR-041 and the schema registry

- Outcome: `docs/adr-041-manifest-schema-versions-and-feature-admission.md`
  exists with `Status: Accepted` (dated, with a one-sentence summary),
  recording D-1 to D-7 and D-15, the options considered (per-feature opt-in
  list, inferred minimum version, allocation at ratification, allocation at
  admission), and prior art: Ninja's `ninja_required_version`, Cargo's
  `cargo-features`, and Terraform's `required_version`.
  `docs/netsuke-design.md` section 2.2 gains a
  `#### Schema versions and feature admission` subsection containing the
  registry table: feature identifier, owning RFC and roadmap task, reserved
  top-level keys, reserved fields and operation keys, admitted schema version
  (all "reserved" except the `1.0` core), and status. The `netsuke_version`
  bullet in section 2.2 links to it.
- Requirements: RM-b, RM-c (rejection and capability parts), OLD-READER.
- Acceptance evidence: the registry lists `includes`, `bundles`,
  `host_facts`, `inputs`, `contention_classes`, `states`, `artefacts`, and
  `maturity` as reserved top-level keys; `contention`, `produces` as reserved
  node fields; and `require_state`, `ensure_state`, `prepare_state`,
  `clean_owned` as reserved operation keys. Markdown gates pass.
- Conformance check: no code changed; no RFC status changed yet.
- Recovery: revert the milestone commit.
- Remaining gaps: ADR-042 content; the gate.
- Compatibility decision: none.

### EP-M2: ADR-042, RFC acceptance, and roadmap coordination

- Outcome: `docs/adr-042-progressive-enhancement-contracts.md` exists with
  `Status: Accepted`, recording D-7 to D-14, and containing a resolution table
  with one row per outstanding item from OD-21 to OD-25 (item, RFC and section,
  decision or deferral, owning roadmap task). RFCs 0021 to 0025 change their
  preamble status to `Accepted` and add a `- **Ratified by:**` bullet linking
  ADR-041 and ADR-042; their outstanding-decision paragraphs gain one closing
  sentence pointing to ADR-042's resolution table. No other RFC prose changes.
  `docs/roadmap.md` item 12.1.1 and `docs/roadmap-composition.md` items 16.1.1
  and 17.1.1 gain one sentence each: "Allocate schema versions by admission
  under ADR-041." `docs/roadmap-progressive-enhancement.md`'s "Contract
  ownership and integration boundaries" section links both ADRs.
  `docs/contents.md` lists both ADRs in ascending order.
- Requirements: RM-a, RM-c (operation-union ownership), SE.
- Acceptance evidence: every "ratify" item quoted in OD-21 to OD-25 appears in
  the resolution table; the reviewer can check this with the grep in
  `Concrete steps`. Markdown gates pass.
- Conformance check: RFCs 0001 to 0004 and 0029 unchanged; no code changed.
- Recovery: revert the milestone commit.
- Remaining gaps: the gate.
- Compatibility decision: none.

### EP-M3: reader-side schema version gate

- Outcome: `src/manifest/schema_version.rs` defines the pure policy and its
  typed rejection facts; `evaluate_manifest` calls it directly after YAML
  parsing; rejection renders through three new Fluent keys in all 35
  catalogues; the two `0.1.0` fixtures declare `1.0.0`.
- Requirements: OLD-READER, D-1, D-2, D-4 rung 1, D-15.
- Acceptance evidence: OBL-1 to OBL-4 discharged as described in
  `Verification plan`; BDD scenario passes; `make check-fmt`, `make typecheck`,
  `make lint`, `make doc-coverage`, and `make test` pass.
- Conformance check: no public signature changed; no dependency added; no
  syntax from any RFC admitted; the gate reads no environment or filesystem.
- Recovery: revert the milestone commit; the documentation milestones remain
  valid because ADR-041 then names 12.1.2 as the gate's owner (D-15).
- Remaining gaps: rungs 2 to 4 of D-4, owned by the first admitted feature.
- Compatibility decision: the only deployed state is users' manifests. Those
  declaring `1.0.x` are unaffected; others receive a diagnostic with a remedy.
  No compatibility layer is added.

### EP-M4: guides, index, and completion

- Outcome: the users' guide explains `netsuke_version` and the new
  diagnostic; the developers' guide explains how a future feature is reserved,
  admitted, and allocated a version; section 3.3 of the design document lists
  admission as the first ingestion stage;
  `docs/snapshot-testing-in-netsuke-using-insta.md` uses `"1.0.0"`; the roadmap
  marks 20.1.1 and its three sub-bullets done; this plan becomes `COMPLETE`.
- Acceptance evidence: gates pass; the roadmap checkboxes read `[x]`.
- Recovery: revert the milestone commit.
- Remaining gaps: none within 20.1.1.

## Concrete steps

Run everything from the repository root,
`/home/leynos/.lody/repos/github---leynos---netsuke/worktrees/5fe5acf9-eb2b-4b75-9130-bc62cb51da74`
at planning time.

EP-M0. Sweep ADR numbers on every branch:

```bash
git fetch origin
for b in $(git branch -r | grep -v HEAD); do
  git ls-tree --name-only "$b" docs/ | grep -oE 'adr-0[0-9]{2}'
done | sort -u | tail -5
```

Expected at planning time: the last lines are `adr-038`, `adr-039`, and
`adr-040`. Then ask `scrutineer` for the baseline gate run (check-fmt,
typecheck, lint, doc-coverage, test, markdownlint, nixie) and note the log
paths under `Artefacts and notes`.

EP-M1 and EP-M2. Write the ADRs using the ADR template in
`docs/documentation-style-guide.md` (sections: Status, Date, Context and
problem statement, Decision drivers, Options considered, Decision outcome,
Goals and non-goals, Known risks and limitations, Outstanding decisions).
Delegate the mechanical RFC preamble edits and `docs/contents.md` entries to
`scribe`. Run `make fmt` after every Markdown edit, then ask `scrutineer` for
the full gate set. To check the resolution table's coverage:

```bash
grep -n 'ratify\|Ratify' docs/rfcs/002[1-5]-*.md
```

Every hit in the final "outstanding decisions" paragraph of each RFC must
correspond to a row in ADR-042's table. Commit each milestone separately.

EP-M3, red. Add, without production changes:

1. The `rstest` table and `proptest` property for OBL-1, written against the
   intended `schema_version::admit` signature; this fails to compile, which is
   the expected red for a new unit.
2. `tests/schema_version_gate_tests.rs` with the named OBL-2 test
   `unsupported_version_is_rejected_before_template_evaluation`. Run:

   ```bash
   cargo nextest run --workspace --all-features \
     -E 'test(unsupported_version_is_rejected_before_template_evaluation)' \
     2>&1 | tee /tmp/red-netsuke-20-1-1.out
   ```

   Expected: one failure reporting a non-zero sentinel call count.
3. The OBL-4 subprocess test, expected to fail because `generate` succeeds.
4. A BDD scenario in `tests/features/manifest.feature`:

   ```gherkin
   Scenario: Parsing fails for a manifest declaring an unsupported schema version
     Given the manifest file "tests/data/unsupported_schema_version.yml" is parsed
     When the parsing result is checked
     Then parsing the manifest fails
     And the manifest error code is "netsuke::manifest::schema_version"
   ```

   with a new fixture `tests/data/unsupported_schema_version.yml` declaring
   `netsuke_version: "1.1.0"` and one hello-world target. Add the
   `the manifest error code is` step only if no equivalent exists under
   `tests/bdd/steps/manifest`. Observe red with a name filter; do not commit
   red.

EP-M3, green:

1. Create `src/manifest/schema_version.rs` with a `//!` header and:

   ```rust
   /// Highest schema minor version this reader admits under major version 1.
   pub(crate) const SUPPORTED_MAX_MINOR: u64 = 0;

   /// Reasons a declared schema version cannot be admitted.
   #[derive(Debug, Clone, PartialEq, Eq)]
   pub(crate) enum SchemaVersionRejection {
       Missing,
       NotAString,
       Invalid { declared: String, reason: String },
       Unsupported { declared: semver::Version },
   }

   /// Decide whether this reader admits a parsed schema version.
   pub(crate) fn admit(declared: &semver::Version)
       -> Result<(), SchemaVersionRejection>;

   /// Read and admit the literal `netsuke_version` from a parsed document.
   pub(crate) fn admit_document(doc: &ManifestValue)
       -> Result<(), SchemaVersionRejection>;
   ```

   `admit` is the pure policy; `admit_document` only extracts the literal and
   delegates. Neither performs I/O.
2. In `src/manifest/diagnostics/mod.rs`, add a coded source diagnostic
   `SchemaVersionDiagnostic` with
   `#[diagnostic(code(netsuke::manifest::schema_version))]` and a mapping from
   `SchemaVersionRejection` to it, following `DataDiagnostic` and
   `map_data_error`.
3. Declare `MANIFEST_SCHEMA_VERSION_MISSING`,
   `MANIFEST_SCHEMA_VERSION_INVALID`, and `MANIFEST_SCHEMA_VERSION_UNSUPPORTED`
   in `src/localization/keys.rs`, and add the messages to every
   `locales/*/messages.ftl`, with the code
   `[netsuke::manifest::schema_version]` inside the text and variables `$name`,
   `$declared`, `$supported` as needed. Example `en-US` text for the
   unsupported case:

   The message is one line in the catalogue; it is wrapped here for width:

   ```plaintext
   [netsuke::manifest::schema_version] { $name } declares netsuke_version
   { $declared }, but this Netsuke supports schema versions { $supported }.
   Upgrade Netsuke, or declare a supported version.
   ```

4. In `evaluate_manifest`, directly after the `serde_saphyr::from_str` call,
   call `schema_version::admit_document(&doc)` and map a rejection into
   `ManifestError::Parse` with the `MANIFEST_PARSE` summary.
5. Change `tests/data/jinja_is.yml` and `tests/data/jinja_is_missing.yml` to
   `netsuke_version: "1.0.0"`.
6. Re-run the focused tests, then ask `scrutineer` for the full gate set.
   Review new snapshots with `cargo insta review` before accepting them.

EP-M3, refactor: keep `evaluate_manifest` under the 70-line `too_many_lines`
ceiling and `src/manifest/mod.rs` under 400 lines; extract a helper if the
insertion pushes either over. Re-run the focused tests and the full gate set,
then commit.

EP-M4. Edit the guides as described in the milestone, run `make fmt`, ask
`scrutineer` for the full gate set, mark the roadmap entries done, set this
plan's status to `COMPLETE`, and commit.

## Validation and acceptance

Quality criteria:

- Tests: `make test` passes. The new tests named under OBL-1 to OBL-4 and the
  new BDD scenario fail before EP-M3's green step and pass after it.
- Verification: OBL-1 to OBL-4 discharged with the evidence listed.
- Lint and types: `make check-fmt`, `make typecheck`, `make lint`, and
  `make doc-coverage` pass after every milestone.
- Documentation: `make markdownlint` and `make nixie` pass after every
  milestone.
- Security: an unsupported manifest performs no template evaluation, as
  OBL-2 demonstrates.

Behavioural acceptance, observable by hand after EP-M3:

```bash
mkdir -p /tmp/schema-gate-netsuke && cd /tmp/schema-gate-netsuke
printf 'netsuke_version: "1.1.0"\ntargets:\n  - name: hello.txt\n    command: "echo hi > hello.txt"\n' > Netsukefile
netsuke generate; echo "exit=$?"
```

Expected: a diagnostic containing `netsuke::manifest::schema_version` and the
supported range `1.0`, `exit=` followed by a non-zero status, and no Ninja
output. Replacing `1.1.0` with `1.0.0` prints the generated Ninja file and
exits zero.

Each gate is run sequentially by `scrutineer`, never in parallel, with output
captured under
`/tmp/$ACTION-netsuke-20-1-1-ratify-progressive-enhancement-contracts.out`.

## Idempotence and recovery

Every step is repeatable. Markdown edits are reapplied by re-running
`make fmt`. Each milestone is a separate commit, so any milestone can be
reverted with `git revert` without disturbing earlier ones. If EP-M3 is
reverted, ADR-041's D-15 paragraph must be edited to name 12.1.2 as the gate
owner in the same revert series. The `/tmp` probe directories may be deleted at
any time.

## Artefacts and notes

Ninja behaviour probe (Ninja 1.11.1), supporting D-11:

```plaintext
ninja: error: build.ninja:5: unknown pool name 'nope'
[1/1] true                      # depth = 0 accepted: unlimited pool
ninja: error: build.ninja:2: invalid pool depth
ninja: fatal: ninja version (1.11.1) incompatible with build file ninja_required_version version (99.0).
```

Prior art consulted through Firecrawl:

- Ninja manual: pools were introduced in Ninja 1.1; the `console` pool in
  1.5 has depth 1 and grants the terminal; `ninja_required_version` makes an
  older Ninja refuse a newer build file.
- Nagios plugin guidelines: exit statuses 0 OK, 1 WARNING, 2 CRITICAL, and 3
  UNKNOWN; codes outside that range are out of range; plugins are expected to
  finish within about ten seconds; performance data follows `|`. This matches
  RFC 0021 section 6 and supports D-10's defaults.
- Cargo unstable features: a manifest opts into unstable syntax with
  `cargo-features = [...]`. D-5 records why Netsuke does not copy this.
- Terraform `required_version`: a configuration declares the tool versions it
  accepts, and older tools refuse it. Netsuke's analogue is admission by schema
  version rather than by tool version.

## Interfaces and dependencies

No new crates. `semver` provides `Version` and `VersionReq`; `proptest`,
`rstest`, `rstest-bdd`, `insta`, `googletest`, and `pretty_assertions` provide
testing. `ortho_config` is deliberately not touched: the gate has no
configuration and no operator override, because an override would let an older
reader evaluate a manifest it cannot understand.

At the end of EP-M3 these crate-private items exist:

- `crate::manifest::schema_version::SUPPORTED_MAX_MINOR: u64`.
- `crate::manifest::schema_version::SchemaVersionRejection`.
- `crate::manifest::schema_version::admit`, taking `&semver::Version` and
  returning `Result<(), SchemaVersionRejection>`.
- `crate::manifest::schema_version::admit_document`, taking `&ManifestValue`
  and returning `Result<(), SchemaVersionRejection>`.
- A `SchemaVersionDiagnostic` source diagnostic with code
  `netsuke::manifest::schema_version`, carried by the existing
  `ManifestError::Parse`.

Hexagonal boundary: `admit` is domain policy with no I/O and no knowledge of
YAML, Fluent, or the CLI. `admit_document` is the narrow translation from the
parsed document shape. Rendering to Fluent text happens only in
`src/manifest/diagnostics`. No port or adapter is introduced, because the gate
needs no infrastructure.

Signposts for the implementer:

- Documentation: `docs/netsuke-design.md` (sections 2.2 and 3.3),
  `docs/documentation-style-guide.md` (ADR and RFC templates),
  `docs/rust-testing-with-rstest-fixtures.md`, `docs/rstest-bdd-users-guide.md`,
  `docs/reliable-testing-in-rust-via-dependency-injection.md`,
  `docs/rust-doctest-dry-guide.md`, `docs/ortho-config-users-guide.md` (for why
  no configuration is added), `docs/snapshot-testing-in-netsuke-using-insta.md`,
  `docs/translators-guide.md`, and
  `docs/formal-verification-methods-in-netsuke.md`.
- Skills: `execplans` (keeping this plan current), `rust-router`, then
  `rust-errors` for the rejection type and `rust-unit-testing` and `proptest`
  for OBL-1 and OBL-2; `hexagonal-architecture` for the boundary note above;
  `arch-decision-records` for the ADRs; `en-gb-oxendict-style` for prose;
  `codegraph-mcp` for callers of `evaluate_manifest`.
- Agents: `scrutineer` runs every gate; `scribe` performs mechanical
  documentation edits; `wyvern` performs read-only lookups.

## Revision note

Initial draft, 2026-09-27. Pending expert review.
