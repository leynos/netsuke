# 20.1.1. Ratify the progressive-enhancement contracts and version gates

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: DRAFT

This plan is approval-gated. Do not begin implementation until the user
explicitly approves it. Approving the plan authorizes drafting the decision
records it describes. It does not accept them: acceptance happens through the
normal Architectural Decision Record (ADR) process, when the user explicitly
accepts the drafted ADRs during pull-request review (milestone EP-M5).

## Purpose / big picture

Roadmap item 20.1.1 in `docs/roadmap-progressive-enhancement.md` asks for the
five progressive-enhancement proposals, RFCs 0021 to 0025, to be reviewed and
their outstanding schema questions settled before any of their syntax is
implemented. It asks for one version-allocation protocol that the
structured-command work (12.1.1), the include work (16.1.1), and the bundle
work (17.1.1) can share without waiting for one another. It also asks for
explicit rules on who owns the command "operation union", how Netsuke reports
which optional features it supports, and how it rejects syntax it does not
support.

After this work, four things are observable:

1. Seven new ADRs exist and are accepted. ADR-041 fixes what
   `netsuke_version` means, how schema versions are allocated over a release
   lifecycle, and how unsupported syntax is rejected. ADR-042 records the
   cross-cutting contracts: the shallow end, feature and rule identifiers, the
   registered discriminators of the command operation union, persisted format
   coordination, and one merge rule for operator ceilings. ADR-043 to ADR-047
   record the schema decisions for RFCs 0021 to 0025, one ADR per RFC,
   following the RFC 0011 and ADR-019 precedent.
2. `docs/netsuke-design.md` contains the authoritative schema-version
   registry, which reserves every planned top-level key, node field, command
   discriminator, and closed vocabulary, and a resolution table that gives
   every outstanding item in RFCs 0021 to 0025 either a decision or a named
   owning task with a merge gate.
3. RFCs 0021 to 0025 name their governing ADRs, and the few sentences in them
   and in the 12.1.1 and 12.1.2 roadmap entries that contradict those ADRs are
   amended, so no accepted decision is contradicted by live text.
4. Netsuke enforces the reader-side version gate that every later feature
   depends on. Today the binary accepts any valid SemVer string in
   `netsuke_version`, including `"2.0.0"`, and it only reads the version after
   Jinja templates have been evaluated, so an unsupported manifest can reach
   `env()`, `glob()`, stdlib command helpers, and network helpers first. After
   this work, a manifest whose root mapping declares a schema version this
   release does not support fails immediately after YAML parsing, before any
   template evaluation, with a localized diagnostic whose text carries the
   stable code `netsuke::manifest::schema_version`. A manifest declaring
   `"1.0.0"`, which covers the quickstart, every example, and all but two
   accidental test fixtures, behaves exactly as before.

The fourth outcome is the only behavioural change. Its first justification is
safety: rejecting before evaluation closes today's hole in which a manifest
declaring an unknown schema is evaluated anyway. Its second is forward
compatibility: RFC 0022 section 8 requires that "older readers must reject new
syntax with a useful version remedy", and only binaries that already contain
the gate can be such readers. Roadmap item 12.1.2 remains the owner of the
per-feature minimum-version checks that build on this gate. A reviewer who
prefers to keep 20.1.1 documentation-only may strike EP-M3 at approval; see
decision D-15.

## Constraints

- Do not implement until the user explicitly approves this plan.
- Do not mark any ADR `Accepted`, or roadmap item 20.1.1 done, until the user
  has explicitly accepted the drafted ADRs in pull-request review.
- Preserve the shallow end. The quickstart manifest in RFC 0025 section 2 and
  `docs/quickstart.md` must parse, generate, and build with identical output.
  No manifest declaring `netsuke_version: "1.0.0"` may change parse result,
  diagnostics, generated Ninja text, or execution behaviour.
- Implement none of the syntax proposed by RFCs 0021 to 0025 or by RFCs 0001,
  0002, 0003, 0004, 0011, or 0029. Reserving names happens in documentation
  only. No feature registry type, capability-reporting JSON field, CLI flag,
  configuration key, or `context`/`check` command is added; those belong to
  12.1.2, 20.2.1, phase 5, and issue `#592` respectively.
- Keep RFCs 0021 to 0025 at `Status: Proposed`, as RFC 0011 remains while its
  governing ADR-019 is accepted. Add `**Governing decisions:**` preamble
  bullets instead. Do not change the status of RFCs 0001 to 0004, 0011, or
  0029; their ratification belongs to other roadmap items.
- Amend RFC prose only where it contradicts a decision in this plan, and only
  by the minimal sentence change listed in EP-M2.
- The version gate must be pure policy with no I/O. It reads no environment,
  filesystem, or configuration, and it has no operator override, because an
  override would let an older reader evaluate a manifest it cannot understand.
  Tests must not mutate the process environment (`AGENTS.md`, ADR-008).
- Keep semantic error facts typed and render them only at the diagnostics
  boundary through Fluent, as ADR-035 requires. The gate never echoes arbitrary
  manifest text; it renders only integer version components.
- Do not change the public signature of `netsuke::manifest::from_str`,
  `from_str_with_env`, `from_path`, or `ManifestError`. The new diagnostic is
  the `source` of the existing `ManifestError::Parse` variant.
- No new dependencies. `semver`, `rstest`, `rstest-bdd`, `proptest`, `insta`,
  `googletest`, and `pretty_assertions` are already dependencies; confirm each
  in `Cargo.toml` before use.
- No file may exceed 400 lines. Every new module starts with a `//!` comment.
  Every function, private item, enum variant, and field carries a `///`
  comment, because `missing_docs_in_private_items` is denied in `Cargo.toml`.
- ADR numbers must be re-swept across every remote branch and open pull
  request immediately before the files are created. At planning time ADR-039
  and ADR-040 are taken on other branches and ADR-041 to ADR-047 are free.
  Never renumber with a blind `sed`.

## Tolerances (exception triggers)

- Scope: if EP-M3 touches more than 20 files, excluding the 35 locale
  catalogues and new snapshot files, or more than 600 net lines of Rust, stop
  and escalate. The expected set is listed under `Concrete steps`.
- Interface: if any public function signature or public enum variant must
  change, stop and escalate.
- Compatibility: if any fixture, example, README fence, or snapshot other than
  `tests/data/jinja_is.yml` and `tests/data/jinja_is_missing.yml` declares a
  version the gate would reject, stop and list it before changing it. If a
  downstream canary manifest (issue `#598`, PR `#780`) declares anything other
  than `1.0.0`, stop and escalate.
- Ordering: `evaluate_manifest` is at 68 of Clippy's 70 permitted lines, so
  EP-M3 is pre-authorized to replace the YAML-parse block with one extracted
  helper, `parse_admitted_document`. Any restructuring beyond that helper
  triggers escalation.
- Existing tests: if more than the three tests named under `Concrete steps`
  need changed assertions, stop and list them.
- Decisions: if the user amends or strikes a decision during ADR review,
  revise the affected ADR and this plan, and re-request acceptance; do not flip
  that ADR to `Accepted` until the amendment is accepted.
- Iterations: if focused tests still fail after three fix attempts within one
  milestone, stop and escalate.
- ADR numbering: if any of ADR-041 to ADR-047 is taken when EP-M1 starts,
  take the next free numbers, update every reference in this plan, and record
  the change in `Decision log`; this needs no escalation.

## Risks

- Risk: authors confuse the schema version with the Netsuke release version.
  The crate is `0.1.0-beta3`, and the two `0.1.0` fixtures are in-repository
  evidence of that confusion. After a 1.x release, authors may write `1.3.0`.
  Severity: medium. Likelihood: medium. Mitigation: the diagnostic states that
  `netsuke_version` is the manifest schema version, not the release version,
  and names the canonical form `1.0.0`; the users' guide gains a section; the
  CHANGELOG records the change as breaking.

- Risk: mixed fleets at the first schema-minor release. Binaries released
  before this gate still accept `1.1.0` and evaluate it. Severity: medium.
  Likelihood: medium. Mitigation: ADR-041 states that the reader guarantee
  starts with the first release containing the gate, and forbids publishing the
  first schema minor in that same release.

- Risk: ratified schema decisions later prove wrong during phases 21 to 25.
  Severity: medium. Likelihood: medium. Mitigation: one ADR per RFC keeps each
  decision independently supersedable; deferred items carry a merge gate
  requiring an ADR amendment before their implementation merges; nothing is
  admitted by code, so a superseding ADR before release needs no migration.

- Risk: 12.1.1, 16.1.1, or 17.1.1 is planned concurrently and allocates
  `1.1.0` independently. Severity: medium. Likelihood: medium. Mitigation:
  ADR-041 allocates at admission, not at ratification, and EP-M2 amends the
  12.1.1 and 12.1.2 roadmap text and adds pointers to 16.1.1 and 17.1.1.

- Risk: new template vocabulary slips past the version. A new Jinja global or
  filter is not a manifest key, so a `1.0.0` manifest could depend on a newer
  reader. Severity: medium. Likelihood: medium. Mitigation: ADR-041 classifies
  template vocabulary introduced by a registered feature (such as the `inputs`
  namespace or host facts) as part of that feature's admission; general stdlib
  growth remains under RFC 0006's policy, and ADR-041 records this as a known
  limitation.

- Risk: moving the check earlier changes existing diagnostics. Severity: low.
  Likelihood: high. Mitigation: non-mapping roots (empty, whitespace-only, or
  comment-only files) bypass the gate and keep today's structural error, so
  `tests/yaml_error_tests.rs` and `src/diagnostic_json_excerpt_tests.rs` are
  unchanged; `src/manifest/tests/stages.rs`'s final-rendering test, which uses
  a missing version as its trigger, moves to an unknown-field trigger.

- Risk: 140 new translated catalogue entries are machine-assisted and not
  reviewed by native speakers. Severity: low. Likelihood: high. Mitigation:
  keep messages short and built from terms already in each catalogue, keep
  identifiers and version numbers untranslated, and flag the entries for
  translator review in the pull-request description.

- Risk: `scenarios!` in `tests/bdd_tests.rs` sweeps the whole
  `tests/features` directory, so a red scenario fails the whole BDD target.
  Severity: low. Likelihood: high. Mitigation: observe red locally with a name
  filter and commit only at green.

## Progress

- [x] (2026-09-27) Renamed the branch to
  `20-1-1-ratify-progressive-enhancement-contracts` and set its upstream.
- [x] (2026-09-27) Reconnaissance by a Wyvern team: roadmap coordination
  points, manifest-version handling, ADR process, and prior art.
- [x] (2026-09-27) First draft of this plan.
- [x] (2026-09-27) Expert-panel review (structure and contracts, failure
  modes, alternatives and prior art, process and viability); all four verdicts
  were "revise" or "approve with revisions".
- [x] (2026-09-27) Second draft addressing every panel finding; see
  `Revision note`.
- [ ] Plan approved by the user.
- [ ] EP-M0: re-sweep ADR numbers; record baseline gates.
- [ ] EP-M1: ADR-041 (Proposed) and the schema-version registry.
- [ ] EP-M2: ADR-042 to ADR-047 (Proposed), resolution table, RFC and roadmap
  amendments.
- [ ] EP-M3: reader-side schema version gate, users' guide, CHANGELOG.
- [ ] EP-M4: developers' guide and documentation clean-up.
- [ ] EP-M5: ADR acceptance, roadmap completion, plan completion.

## Surprises & discoveries

- Observation: Netsuke has no version gate. `NetsukeManifest` declares
  `netsuke_version: semver::Version` (`src/ast/mod.rs`), and the only failure
  is a SemVer syntax error. `"2.0.0"` is accepted today. Evidence: no
  `VersionReq`, supported-version constant, or comparison exists under `src/`;
  `tests/data/invalid_version.yml` fails only because `"1"` is not full SemVer.
  Impact: the RFCs' older-reader requirement is unmet by every shipped binary.
  Motivates EP-M3.

- Observation: the version is read after template evaluation. In
  `evaluate_manifest` (`src/manifest/mod.rs`), YAML becomes a
  `serde_json::Value`, variables and macros are registered, `foreach` and
  `when` are expanded through MiniJinja, and only then is the value
  deserialized into `NetsukeManifest`. Evidence: the call order in
  `evaluate_manifest`; every entry point (`from_path*`, `parse_with_config`,
  `query.rs`, the runner, graph, and help paths) funnels through
  `budget_adapter::from_str_named` into it. Impact: the gate belongs directly
  after YAML parsing. The only bypass is library use of the public
  `Deserialize` implementation on `NetsukeManifest`, which ADR-041 records as a
  limitation.

- Observation: YAML shapes reaching the gate. Unquoted `netsuke_version: 1.0.0`
  (used in about 40 fixtures) arrives as a JSON string; unquoted `1.0` and `1`
  arrive as numbers and are already rejected by the typed decode. Serde-saphyr
  resolves anchors, aliases, and `<<` merge keys before the gate sees the
  value. An empty or comment-only file parses to `null`. Evidence: panel review
  of `serde_saphyr` 1.2.0 options and existing fixtures such as
  `tests/data/foreach.yml`. Impact: "non-string" rejection breaks nothing; the
  gate must skip non-mapping roots to keep today's structural errors.

- Observation: the JSON diagnostic's top-level `code` is always
  `netsuke::manifest::parse` for manifest failures; inner codes appear only in
  `causes[*]` text. Evidence: `src/diagnostic_json.rs` and the
  `manifest_parse_error` snapshot. Impact: the gate's code is carried inside
  the Fluent message text, as for existing template diagnostics, and OBL-4
  asserts on `causes`.

- Observation: two test fixtures declare `"0.1.0"`; the snapshot-testing
  guide shows `"0.1"`, which is not valid SemVer; the test-framework design
  document illustrates `"1.2.0"`. Evidence: `git grep` over the tree. Impact:
  the fixtures move to `1.0.0` in EP-M3; the guide is corrected in EP-M4; the
  design-only document is left alone.

- Observation: no RFC has ever left `Proposed`; RFC 0011 records its
  accepted governing decision with a `**Governing decision:** ADR-019` preamble
  bullet while remaining `Proposed`. Evidence: RFC preambles;
  `docs/rfcs/0011-allow-listed-structured-command-shells.md`. Impact:
  ratification uses that precedent rather than inventing a new RFC status
  transition.

- Observation: `context` and `check` do not exist; `Commands` in
  `src/cli/command.rs` has `build`, `clean`, `graph`, `generate`, and `help`.
  `netsuke check` is in open PR `#621` for issue `#592`. Impact: capability
  reporting is specified here but first surfaces through diagnostics; the JSON
  reporting shape stays with 20.2.1.

- Observation: Ninja 1.11.1 treats `depth = 0` as an unlimited pool, rejects
  negative depths and undeclared pool names, and refuses a build file whose
  `ninja_required_version` exceeds its own version. Evidence: probe transcript
  under `Artefacts and notes`. Impact: RFC 0024's rejection of capacity zero is
  load-bearing (D-11).

- Observation: RFC 0001 section 15.1 admits only `invoke`, `rule`, and
  `script` as command-list discriminators and forbids `rule` and `script` as a
  direct `command:` value, while RFCs 0021 and 0023 use `ensure_state` and
  `clean_owned` as direct values. Evidence:
  `docs/rfcs/0001-structured-command-blocks.md` section 15.1; RFC 0021 section
  3; RFC 0023 section 3. Impact: D-8 registers the new discriminators and
  decides the direct-value question explicitly.

## Decision log

Decisions D-1 to D-14 are proposed content for the ADRs. They take effect only
when the user accepts the corresponding ADR in EP-M5. D-15 to D-18 are planning
decisions that take effect on plan approval.

- Decision D-1 (ADR-041), schema version semantics. `netsuke_version` is the
  manifest schema version, not the Netsuke release version. It keeps SemVer
  spelling because the whole corpus and RFC 0003's `requires.manifest` ranges
  use it, but its canonical form is exactly `MAJOR.MINOR.0` with no prerelease
  or build suffix, so in practice it is a major number plus a minor integer. A
  reader admits a declared version when its major is supported, its minor does
  not exceed that major's maximum, its patch is `0`, and it has no prerelease
  or build metadata. Today's reader supports major `1` with maximum minor `0`.
  The supported range is data, so a future major-2 reader can keep admitting
  `1.x`; a major bump is reserved for breaking changes, playing the role of a
  Rust edition. Rationale: one spelling per schema version; nothing in the
  corpus uses a non-zero patch or a suffix, so rejecting them is free and
  removes ambiguity. Date/Author: 2026-09-27, planning agent, revised after
  panel review.

- Decision D-2 (ADR-041), admission point. When the parsed YAML root is a
  mapping, its literal `netsuke_version` entry is checked immediately after
  YAML parsing and before variable registration, macro registration, `foreach`/
  `when` expansion, or any other template evaluation. A missing entry, a
  non-string value, an invalid SemVer string (parsed with
  `semver::Version::parse`, exactly as the typed decode does), and an
  unsupported version are all rejected there. A non-mapping root is not a
  manifest at all and keeps today's structural error. The value is never
  templated. Rationale: an older reader must not evaluate a manifest it does
  not understand, because templates can read the environment, glob the
  filesystem, run commands, and fetch URLs. Date/Author: 2026-09-27, planning
  agent, revised after panel review.

- Decision D-3 (ADR-041), allocation lifecycle. Ratification reserves names in
  the design-document registry; it allocates no number.
  1. Only `main` allocates. A schema minor is allocated by the change that
     makes a feature's syntax admissible (its "admission change"), which is
     the last change in that feature's delivery.
  2. The first admission change after the most recent published tag, betas
     included, raises the reader's maximum minor by one in the same commit.
     Later admission changes before the next tag reuse that minor.
  3. Publishing any tag freezes the feature set of every minor it contains.
     A frozen minor never changes meaning within its major; later changes to
     that syntax need a new minor.
  4. Release and hotfix branches never raise the maximum and never backport
     admissions.
  5. Reverting the only admission of an unpublished minor also reverts the
     maximum-minor increase.
  6. The first schema minor must not be published in the same release that
     introduces the reader gate.
  7. For unpublished `main` builds, the feature-identifier capability report,
     not the number, is authoritative.
  Provisional numbers in RFC 0001 section 19.2 and RFC 0003's descriptor
  example are illustrative. The registry records, per feature, its status
  (`reserved`, `admitted`, or `released`), its schema minor, and the first tag
  that shipped it; a contract test binds the registry to the reader's maximum.
  Rationale: 12.1.1, 16.1.1, 17.1.1, and phases 21 to 25 can ratify and deliver
  in any order without renumbering, and bundle work need not exist first. Tags,
  including betas, are what users install. Date/Author: 2026-09-27, planning
  agent, revised after panel review.

- Decision D-4 (ADR-041), rejection ladder. Schema admission events are: a
  new key, a new command discriminator, a new value in a closed vocabulary
  (such as a state `kind`, probe `protocol`, or maturity rule identifier), and
  new template vocabulary introduced by a registered feature. Unsupported
  syntax is rejected in this order and never ignored:
  1. Rung 1: the declared version is outside the reader's supported range.
     Reject before evaluation, leading with "upgrade Netsuke", naming the
     supported range, and noting that the field is the schema version.
  2. Rung 2: the declared version is supported, but the manifest uses syntax
     admitted in a later minor. Reject with the feature identifier and its
     minimum version, as an author remedy.
  3. Rung 2b: the manifest uses syntax that is reserved in the registry but
     not yet admitted by this reader. Reject naming the reserved feature.
  4. Rung 3: the syntax is unknown to every registered feature. Keep the
     existing unknown-field error.
  5. Rung 4: the syntax is known but unsupported in context, such as
     `contention` on a rule. Reject with a feature-specific diagnostic.
  Rungs 2 and 2b run as a pure pass over the raw document before evaluation, so
  `deny_unknown_fields` does not interfere. EP-M3 implements rung 1 only;
  12.1.2, or whichever feature is admitted first, implements rungs 2 and 2b.
  Date/Author: 2026-09-27, planning agent, revised after panel review.

- Decision D-5 (ADR-041), opt-in mechanism. Raising `netsuke_version` is the
  only opt-in. There is no per-feature list in the style of Cargo's
  `cargo-features`, because Netsuke's minors are additive and a list would
  duplicate the version signal. Options considered and rejected in ADR-041: a
  version floor plus reserved-key rejection with no version bump (cannot catch
  shape changes such as RFC 0001's string-to-mapping `command`, cannot catch
  unreserved keys, and lets declarations drift from actual use, as Cargo's
  unchecked `rust-version` does); tying the field to the Netsuke release
  version (would break every existing manifest under a 0.x binary); an
  informative-only version (Docker Compose's approach, which warns on unknown
  fields; unsuitable for a tool that runs commands and deletes files).
  Date/Author: 2026-09-27, planning agent, revised after panel review.

- Decision D-6 (ADR-041), nested and composed documents. Each document a
  reader loads is admitted against its own declared version. Whether an
  included fragment or bundle entry manifest must declare a version, and how it
  relates to its importer's, is assigned to 16.1.1 and 17.1.1, with the
  constraint that `requires.manifest` comparators use patch `0`. A nested
  schema integer such as RFC 0029's `host_facts.schema` is assigned to RFC
  0029's roadmap item, with the constraint that a nested version is additional
  to, never a substitute for, the manifest minor. Date/Author: 2026-09-27,
  planning agent, added after panel review.

- Decision D-7 (ADR-042), identifiers and capability reporting. Feature
  identifiers and maturity rule identifiers are separate namespaces. The
  design-document registry is the open list of feature identifiers; its initial
  rows are `structured-commands`, `named-shells`, `includes`, `bundles`,
  `host-facts`, `inputs`, `contention`, `states`, `artefacts`, and `maturity`.
  ADR-047 holds the rule-to-feature table (for example, `owned-cleanup` requires
  `artefacts`). Capability reporting is keyed by feature identifier; the JSON
  shape belongs to 20.2.1 and its command surface to phase 5's `context`. A
  policy selecting a rule whose feature the reader lacks fails with a version
  or capability remedy. Date/Author: 2026-09-27, planning agent, revised after
  panel review.

- Decision D-8 (ADR-042), the command operation union. Phase 12 (RFC 0001)
  owns the one closed union: its abstract syntax tree (AST), parser,
  action-plan codec, and runner dispatch. The union's discriminator set is
  registered: `invoke`, `rule`, and `script` from RFC 0001, plus
  `require_state`, `ensure_state`, and `prepare_state` from RFC 0021 and
  `clean_owned` from RFC 0023. RFC 0001 section 15.1's rule becomes "exactly
  one registered discriminator". Unlike `rule` and `script`, the four
  operations are valid as the direct value of `command:`, because no
  recipe-level form covers them. `states.*.prepare` and `probe.external` reuse
  phase 12's command-block type, not a private parser. Feature RFCs own their
  variants' validation and semantics but never a private parser, codec, or
  runner. 12.1.2's closed AST must treat the four keys as reserved (rung 2b),
  and phases 23 and 24 depend on 12.1.2 and 12.3.1. Date/Author: 2026-09-27,
  planning agent, revised after panel review.

- Decision D-9 (ADR-042), shallow end and persisted formats. RFC 0025 section
  2 is a release acceptance requirement for every schema-1 release: optional
  top-level keys default to empty, their absence produces no message at any
  verbosity, and one node's feature never requires declarations elsewhere
  (fixtures belong to 20.1.2). Persisted formats are versioned independently of
  the schema; this plan records coordination constraints, not their final
  design. For 12.1.1 and 12.3.1: the action plan carries its own integer format
  version, changed with any change to its encoding or meaning, and replay fails
  closed before spawning on any mismatch, with a "regenerate" remedy. For
  23.2.2: state records live under a versioned namespace; a record whose
  version the reader does not know is no evidence; with `identity` declared,
  missing evidence yields `not_ready` only after the resource has been
  inspected successfully, and otherwise `unknown`, which never authorizes
  repair; lease identity lives outside the versioned namespace so two Netsuke
  versions cannot prepare one path concurrently; and retention covers stale
  namespaces. The JSON output envelope evolves additively within version 1,
  owned by 20.2.1. Dyndep sidecars keep ADR-012. Date/Author: 2026-09-27,
  planning agent, revised after panel review.

- Decision D-10 (ADR-042), operator ceilings. Operator limits introduced by
  these RFCs follow one rule: a numeric ceiling set by a trusted layer is a
  maximum; the primary project `.netsuke.toml` (ADR-021's untrusted project
  layer) may lower it but never raise it; the effective value is the minimum,
  and provenance records both. Boolean denials follow ADR-021's tightening
  rule. Names follow `<feature>_<quantity>_ceiling[_<unit>]` for ceilings and
  `<feature>_<capability>_deny` for denials. Implementation uses OrthoConfig
  through the existing configuration seams; `docs/ortho-config-users-guide.md`
  governs field declaration. Date/Author: 2026-09-27, planning agent, added
  after panel review.

- Decision D-11 (ADR-046, RFC 0024). `capacity` is an integer from 1 to a
  schema-owned maximum of 1024, independent of the CLI `jobs` limit so that
  raising `jobs` never changes the schema; zero is rejected because Ninja treats
  `depth = 0` as unlimited. The effective capacity is the minimum of the
  declared value and the operator's `contention_capacity_ceiling`, and
  inspection reports both. Public class names never reach Ninja text directly:
  22.1.2 replaces the IR's raw `pool: Option<String>` with a typed class
  identity and lowers it through ADR-014's escaping seam to a reserved
  `netsuke_pool_` prefix with an injective, declaration-order-independent
  encoding that 22.1.2 fixes and property-tests. An edge requiring both a
  contention class and Ninja's `console` pool is rejected; no current surface
  requests `console`, so the rule has no trigger until one does. A `contention`
  field on a rule is rejected in the first version. Date/Author: 2026-09-27,
  planning agent, revised after panel review.

- Decision D-12 (ADR-045, RFC 0023). The canonical spellings are `artefacts`
  and `--artefact`; there is no `artifacts` alias, and 24.1.1 adds a suggestion
  for it. `kind` defaults to `file`, `role` to `generated`, and `create_parent`
  to `false`. An artefact root may not equal, contain, or lie inside another
  artefact root. A directory root may not contain a protected path: the
  manifest, loaded configuration files, `.git`, Netsuke runtime directories,
  local runtime scripts, or a target `sources` path that no target in the graph
  produces. An artefact equal to a target output is valid only with the same
  producer and a compatible object kind. Operator limits are
  `clean_entries_ceiling`, `clean_depth_ceiling`, `clean_bytes_ceiling`, and
  `clean_elapsed_ceiling_seconds` under D-10. Deferred with owners: platform
  deletion primitives (24.2.1), resource-lease identity (20.2.1), default
  numeric limits (24.1.2), and the non-prerequisite extensions
  (trash-and-rename, adoption receipts, wildcard collections, role selectors,
  remote delivery: no owner until a separate RFC). Date/Author: 2026-09-27,
  planning agent, revised after panel review.

- Decision D-13 (ADR-044, RFC 0022). Integer inputs use the signed 64-bit
  range and reject overflow rather than widening or wrapping.
  `--input NAME=VALUE` is a repeatable option registered once in ADR-016's
  canonical command metadata and attached to every subcommand that loads a
  manifest; it is not a top-level global. The `inputs` Jinja namespace exists
  only when a manifest declares `inputs`, and it is admitted with the `inputs`
  feature. Deferred with owners: the single owner of shared typed-parameter
  validation and the shared path grammar (20.2.1, with 17.1.3), profile overlay
  order (phase 5, before 21.2.1), and dependent defaults or custom validators
  (not before 21.3.1's canary shows demand). Date/Author: 2026-09-27, planning
  agent, narrowed after panel review.

- Decision D-14 (ADR-043 and ADR-047, RFCs 0021 and 0025). For states: `kind`
  is the closed set `directory`, `file`, `python-venv`, and `custom`; when
  `probe.external` is present, `probe.protocol` is required and is the closed
  set `nagios`; `probe.timeout_seconds` and `probe.max_output_bytes` are
  positive integers defaulting to 10 and 65 536; state operations are valid
  only in always-run nodes (actions, which are phony, and targets with
  `always: true`); operator policy is `probe_external_deny`,
  `probe_timeout_ceiling_seconds`, and `probe_output_ceiling_bytes` under D-10.
  Deferred with owners: the `python-venv` inspection contract and any
  interpreter-constraint field (23.1.2), the lease implementation (20.2.1 and
  20.2.3), record retention limits (23.2.2), executable-identity restriction
  (23.3.2), and a uv package-integrity probe (no owner until demand). For
  maturity: project policy lives in the root `maturity.rules` list and can
  strengthen but never weaken a trusted operator floor; severity is
  `off < warn < error` and combines monotonically. Deferred with owners: the
  operator-floor configuration shape (25.2.1), selected-closure inspection
  metadata (25.2.2), and the semantic linter's inventory boundary (25.1.1 with
  issue `#592`). Date/Author: 2026-09-27, planning agent, revised after panel
  review.

- Decision D-15 (scope): EP-M3 is in scope. It implements rung 1 of D-4 only.
  EP-M2 narrows roadmap item 12.1.2 to "extend ADR-041's reader gate with rungs
  2 and 2b", so the gate has exactly one owner at a time. If the reviewer
  strikes EP-M3, ADR-041 names 12.1.2 as the owner of rung 1 as well, and EP-M2
  leaves 12.1.2's title unchanged. EP-M3 may ship as a separate pull request
  from the documentation milestones; they do not depend on it. Date/Author:
  2026-09-27, planning agent, revised after panel review.

- Decision D-16 (verification): no Kani harness and no Verus proof. The
  admission predicate is a loop-free comparison over `semver::Version`, whose
  prerelease and build identifiers are heap strings; a property test with an
  independent oracle exercises the same domain more cheaply, and
  `docs/formal-verification-methods-in-netsuke.md` keeps Verus out of the
  manifest layer. See `Verification plan`. Date/Author: 2026-09-27, planning
  agent.

- Decision D-17 (process): ADRs are drafted as `Proposed` and flipped to
  `Accepted`, dated and summarized as the style guide requires, only after
  explicit user acceptance in review. RFCs 0021 to 0025 stay `Proposed` and gain
  `**Governing decisions:**` bullets, following RFC 0011. Every deferred
  resolution-table row carries the gate "ADR amendment merged before the owning
  task's code merges". Date/Author: 2026-09-27, planning agent, added after
  panel review.

- Decision D-18 (diagnostic content): the gate never echoes the declared
  string. `Unsupported` renders only the integer major and minor. `Invalid` and
  `NotAString` render no value. This keeps unbounded, possibly
  control-character-bearing manifest text out of terminal and CI output, as
  `src/manifest/env_reader.rs` already does for environment values.
  Date/Author: 2026-09-27, planning agent, added after panel review.

## Outcomes & retrospective

Not started. Complete at each milestone boundary and at completion, reconciling
every discovery with the artefacts in `Conformance basis`.

## Context and orientation

Netsuke is a build-system compiler. It reads a YAML manifest called a
`Netsukefile`, expands its Jinja templates, builds an intermediate
representation, and writes a Ninja build file, which Ninja then runs.
`docs/netsuke-design.md` is the architecture reference; section 2.2 lists the
top-level manifest keys, and section 3.3 describes the ingestion stages.

Terms used in this plan:

- Schema version: the value of `netsuke_version`. It names the manifest
  language version, not the Netsuke release.
- Reader: a Netsuke binary loading a manifest. An older reader is a binary
  released before some schema feature existed.
- Admission: the decision, before any evaluation, whether this reader
  accepts a manifest's declared schema version; for a feature, the change that
  makes its syntax acceptable.
- Feature identifier: a stable kebab-case name for one optional vocabulary,
  such as `states`, used in diagnostics and capability reports.
- Operation union: the closed set of item shapes allowed inside a structured
  `command:` value, defined by RFC 0001.
- Discriminator: the key that identifies which union member a mapping is,
  such as `invoke` or `ensure_state`.
- Shallow end: the smallest useful Netsuke manifest and workflow, which must
  keep working without any optional feature.

The proposals under ratification:

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

Each ends with an "Alternatives and outstanding decisions" section. Its final
paragraph lists what must be ratified before implementation; RFC 0021 section
10 and RFC 0022 section 8 also contain version-allocation sentences that D-3
supersedes.

Code touched by EP-M3:

- `src/manifest/mod.rs`: `evaluate_manifest` parses YAML into `doc`, a
  `serde_json::Value` aliased as `ManifestValue`, then evaluates templates,
  then deserializes. The parse block becomes `parse_admitted_document`.
- `src/manifest/diagnostics/mod.rs`: `ManifestError::Parse { source, message }`
  carries a boxed `miette::Diagnostic`; `DataDiagnostic` and `map_data_error`
  show the pattern for a coded source diagnostic.
- `src/localization/keys.rs`: the `define_keys!` block (384 lines today).
- `locales/*/messages.ftl`: 35 catalogues, audited at build time for
  matching keys and variables (`tests/build_l10n_audit_tests.rs`).
- `src/manifest/tests/stages.rs`: stage-observer tests; the pattern of
  `stage_callback_stops_after_parse_failure` is reused for OBL-2.
- `tests/features/manifest.feature` and `tests/bdd/steps/manifest/mod.rs`:
  manifest scenarios, including the `the error message contains` step.
- `tests/assert_cmd_tests.rs`: subprocess tests (157 lines);
  `src/diagnostic_json_tests.rs`: in-process JSON diagnostic snapshots.

## Conformance basis

Upstream artefacts, at `main` commit `ebcedaef`:

- `docs/roadmap-progressive-enhancement.md` item 20.1.1, whose sub-bullets are
  RM-a (review, resolve, record through ADRs), RM-b (coordinate manifest and
  persisted-plan version allocation), and RM-c (operation-union ownership,
  feature-specific capability reporting, rejection of unsupported syntax).
- RFC 0025 sections 2 and 3 (SE, OWN). RFC 0022 section 8 (OLD-READER).
  The outstanding-decision paragraphs OD-21 (RFC 0021 section 10), OD-22 (RFC
  0022 section 9), OD-23 (RFC 0023 section 9), OD-24 (RFC 0024 sections 4 and
  9), and OD-25 (RFC 0025 sections 8 and 10).
- RFC 0001 sections 15.1, 17.4, and 19.2; RFC 0003's descriptor
  `requires.manifest`; RFC 0011 and ADR-019 (precedent); RFC 0029's
  `host_facts` version statement.
- Roadmap items 12.1.1, 12.1.2, and 12.3.1 (`docs/roadmap.md`); 16.1.1 and
  17.1.1 (`docs/roadmap-composition.md`); 20.1.2 and 20.2.1.
- ADR-008, ADR-012, ADR-014, ADR-016, ADR-021, ADR-035 (proposed), ADR-036
  (proposed).
- Standards: `AGENTS.md`, `docs/documentation-style-guide.md`,
  `docs/translators-guide.md`, `docs/formal-verification-methods-in-netsuke.md`.
- There is no Terms of Reference on `main`; PR `#786` proposes one. This plan
  does not depend on it.

Trace links:

```plaintext
RM-a -> OD-21..OD-25 -> D-11..D-14, D-17 -> ADR-043..ADR-047 + design resolution table -> EP-M2 -> EP-M5 acceptance
RM-b -> RFC0001-19.2, RFC0003, RFC0029 -> D-1, D-3, D-6, D-9 -> ADR-041/ADR-042 -> EP-M1/EP-M2 -> registry + roadmap edits
RM-c -> RFC0025-2/3, RFC0001-15.1 -> D-4, D-7, D-8 -> ADR-041/ADR-042 -> EP-M1/EP-M2
OLD-READER -> D-1, D-2, D-4 rung 1, D-15, D-18 -> EP-M3 -> OBL-1..OBL-5
SE -> D-9 -> OBL-3 regression evidence -> 20.1.2 fixtures (later)
```

## Verification plan

EP-M1, EP-M2, EP-M4, and EP-M5 change documentation only and introduce no
runtime invariant. Their verification is the Markdown gates plus two review
checks: every sentence matched by
`grep -nE 'ratif|allocate|Allocate' docs/rfcs/002[1-5]-*.md` in an
outstanding-decision or compatibility section maps to a resolution-table row,
and every row with a deferral names an owning task and the D-17 merge gate.
EP-M3 introduces the obligations below.

Axioms:

- AXIOM-1: `semver::Version::parse` implements SemVer 2.0.0 syntax, and
  `semver::VersionReq` exact comparators (`=1.m.0`) match only that release,
  never a prerelease. Documented crate behaviour; not re-verified.
- AXIOM-2: `serde_saphyr::from_str` into `serde_json::Value` performs no
  template evaluation and resolves anchors, aliases, and merge keys, provided
  the crate's `properties`, `include`, and `include_fs` features stay disabled.
  EP-M3 confirms the feature set in `Cargo.toml`.
- AXIOM-3: `evaluate_manifest` reports `ManifestLoadStage` transitions
  through the optional observer before entering each stage, so an observer that
  records only `InitialYamlParsing` proves no later stage began.

Obligations:

- Obligation OBL-1, admission partition: `admit(v)` succeeds exactly when
  `v.major == 1`, `v.minor <= SUPPORTED.max_minor`, `v.patch == 0`, and both
  `v.pre` and `v.build` are empty. Method: an `rstest` table plus a `proptest`
  property with an independent oracle and a class-coverage check. Rationale:
  the table pins the classes a reader meets; the property checks the generated
  domain against a formulation sharing no code with `admit`. Domain: rows
  `1.0.0` (admit), `1.0.3`, `1.0.0+build.5`, `1.0.0-rc.1`, `1.1.0`,
  `1.1.0-alpha`, `2.0.0`, `0.1.0`, `0.0.0` (all reject); generated versions
  with each numeric part from `0..=3` or `any::<u64>()`, prerelease and build
  metadata present or absent; `cases = 512`. Artefact:
  `src/manifest/schema_version_tests.rs`, a `#[path]` child of
  `src/manifest/schema_version.rs`. Evidence:
  `cargo nextest run --workspace --all-features schema_version` passes. The
  oracle is: some `m` in `0..=SUPPORTED.max_minor` has
  `VersionReq::parse(&format!("=1.{m}.0"))` matching `v`, and `v.build` is
  empty. Non-vacuity: a deterministic test draws 1 000 values from the same
  strategy with a fixed seed through `proptest::test_runner::TestRunner` and
  asserts that at least 10% are admitted and 10% rejected. Negative controls:
  changing `<=` to `<` rejects `1.0.0` and fails row one; dropping the patch
  check admits `1.0.3`; dropping the prerelease check admits `1.0.0-rc.1`.

- Obligation OBL-2, rejection precedes evaluation: for any mapping-rooted
  manifest whose `netsuke_version` is missing, non-string, invalid, or
  unsupported, loading fails with a diagnostic whose text contains
  `netsuke::manifest::schema_version`, the stage observer records exactly
  `[InitialYamlParsing]`, and the injected environment reader is called zero
  times, whatever the rest of the document contains. Method: named red tests,
  then a `proptest` property. Rationale: this is the safety property behind
  D-2; examples alone cannot show that arbitrary bodies never win. Domain: a
  generated rejected version (or none, or a non-string), a body containing
  `foreach: "[env('NETSUKE_SENTINEL')]"` on one target, and up to three
  generated unknown top-level keys drawn from identifiers excluding
  `netsuke_version`, `vars`, `macros`, `rules`, `targets`, `actions`, and
  `defaults`; an extra row supplies the version through a YAML anchor and merge
  key. Artefact: `tests/schema_version_gate_tests.rs`, using
  `from_str_with_env` with a counting reader that returns `Ok` and
  `from_path_with_policy` with a stage observer. Evidence: red first. For
  `"1.1.0"` today the load succeeds; for a missing or invalid version today's
  load fails with a structure error after `FinalRendering`. After EP-M3 all
  rows pass. Non-vacuity: a sibling `1.0.0` test asserts the sentinel is read
  at least once and the observer records `TemplateExpansion`, so both probes
  are live. Negative control: moving the gate call to just before
  `serde_json::from_value` must fail the property.

- Obligation OBL-3, supported manifests unchanged and non-manifests
  unchanged: every manifest declaring `1.0.0` loads, generates, and builds as
  before, and non-mapping roots keep today's structural errors. Method: the
  existing suite as a regression oracle. Artefact: existing tests and
  snapshots, including `tests/yaml_error_tests.rs` and
  `src/diagnostic_json_excerpt_tests.rs`; only the new snapshot files may
  appear. Evidence: `make test` passes; `git status --porcelain -- '*.snap'`
  lists only new files. Non-vacuity: the corpus holds more than 150 manifests
  declaring `1.0.0` (quoted and unquoted); the empty-file cases exercise the
  non-mapping path.

- Obligation OBL-4, command-line behaviour:
  `netsuke generate --output out.ninja` against a manifest declaring `1.1.0`
  exits non-zero, prints the localized diagnostic, and leaves `out.ninja`
  absent; the JSON diagnostic has top-level `code` `netsuke::manifest::parse`
  and a `causes` entry whose text contains `netsuke::manifest::schema_version`.
  Method: an `assert_cmd` subprocess test and an in-process `insta` snapshot.
  Artefact: a new case in `tests/assert_cmd_tests.rs`; a new snapshot test in
  `src/diagnostic_json_tests.rs`. Evidence: red before EP-M3 (generation
  succeeds); green after. Non-vacuity: a paired `1.0.0` run in the same test
  creates `out.ninja`.

- Obligation OBL-5, registry and reader agree: the highest admitted minor in
  the design-document registry equals `SUPPORTED.max_minor`. Method: a contract
  test that parses the registry table. Artefact:
  `tests/schema_version_registry_tests.rs`, reading the table between marker
  comments in `docs/netsuke-design.md`. Evidence: passes with the core `1.0`
  row admitted and every other row reserved. Non-vacuity: the parser is
  exercised on an in-memory table with an admitted `1.1` row, which must report
  a mismatch against a maximum of `0`.

Residual gaps: rungs 2, 2b, and 4 are not implemented here. General stdlib
vocabulary is not version-gated (Risks). Library callers that deserialize
`NetsukeManifest` directly bypass the gate (ADR-041 limitation).

## Plan of work

Stage A (EP-M0) re-sweeps ADR numbers and records baseline gates.

Stage B (EP-M1, EP-M2) drafts ADR-041 and the registry, then ADR-042 to
ADR-047, the resolution table, and the RFC and roadmap amendments. All ADRs are
`Proposed`. Each milestone ends with the full gate set and a commit.

Stage C (EP-M3) adds red tests, observes them fail, implements the pure
admission policy and its diagnostic, migrates two fixtures, updates the users'
guide, CHANGELOG, and design section 3.3, and refactors. Gate and commit. This
stage may ship as its own pull request.

Stage D (EP-M4, EP-M5) updates the developers' guide and fixes the snapshot
guide example, then, after explicit ADR acceptance, flips statuses, marks the
roadmap item done, and completes this plan.

## Milestones and plateaus

### EP-M0: baseline

- Outcome: ADR numbers confirmed; baseline gate logs recorded.
- Requirements: none; this protects later attribution.
- Acceptance evidence: sweep output and `scrutineer` log paths recorded under
  `Artefacts and notes`.
- Conformance check: no file changed except this plan's `Progress`.
- Recovery: none needed.
- Remaining gaps: all later milestones.
- Compatibility decision: none.

### EP-M1: ADR-041 and the schema-version registry

- Outcome: `docs/adr-041-manifest-schema-versions-and-admission.md`
  (`Proposed`) records D-1 to D-6, the options considered under D-5, the prior
  art listed under `Artefacts and notes`, and the limitations (library bypass,
  stdlib vocabulary, pre-gate binaries). `docs/netsuke-design.md` section 2.2
  gains `#### Schema versions and feature admission`, containing the registry
  table between `<!-- schema-registry:start -->` and
  `<!-- schema-registry:end -->` markers, with columns: feature identifier,
  owning RFC and roadmap task, reserved keys, reserved fields, reserved
  discriminators and closed vocabularies, status, schema minor, first tag. The
  core `1.0` row is `admitted`/`released`; every other row is `reserved`. The
  `netsuke_version` bullet links to the subsection.
- Requirements: RM-b, RM-c (rejection), OLD-READER.
- Acceptance evidence: the registry reserves `includes`, `bundles`,
  `host_facts`, `inputs`, `contention_classes`, `states`, `artefacts`, and
  `maturity`; the fields `contention` and `produces`; the discriminators
  `require_state`, `ensure_state`, `prepare_state`, and `clean_owned`; and the
  closed vocabularies of D-11 to D-14. Gates pass.
- Conformance check: no code or RFC changed.
- Recovery: revert the milestone commit.
- Remaining gaps: ADR-042 to ADR-047; the gate.
- Compatibility decision: none.

### EP-M2: contract ADRs, resolution table, RFC and roadmap amendments

- Outcome: all `Proposed`:
  `docs/adr-042-progressive-enhancement-contract-boundaries.md` (D-7 to D-10),
  `docs/adr-043-managed-state-schema.md` (D-14, states),
  `docs/adr-044-typed-task-input-schema.md` (D-13),
  `docs/adr-045-artefact-ownership-schema.md` (D-12),
  `docs/adr-046-contention-class-schema.md` (D-11), and
  `docs/adr-047-maturity-policy-schema.md` (D-14, maturity, and the
  rule-to-feature table). `docs/netsuke-design.md` gains
  `### 2.9 Progressive-enhancement contracts`, holding the resolution table:
  one row per outstanding item from OD-21 to OD-25, with the RFC section, the
  decision or deferral, the governing ADR, the owning task, and the D-17 gate.
  RFCs 0021 to 0025 gain `**Governing decisions:**` preamble bullets. RFC 0021
  section 10's sentence "Allocate the manifest and persisted-plan versions
  during acceptance alongside RFC 0001" and RFC 0022 section 8's "Allocate the
  manifest version with the shared schema acceptance work" are each replaced by
  a sentence stating that ADR-041 allocates schema versions at admission. In
  `docs/roadmap.md`, 12.1.1's third bullet drops "the next available manifest
  minor version" and cites ADR-041 and ADR-042's action-plan constraint;
  12.1.2's title and first bullet say it extends ADR-041's reader gate with
  rungs 2 and 2b and reserves D-8's discriminators. In
  `docs/roadmap-composition.md`, 16.1.1 and 17.1.1 each gain one sentence
  citing ADR-041 and D-6. `docs/roadmap-progressive-enhancement.md`'s ownership
  section links the seven ADRs. `docs/contents.md` lists them in ascending
  order.
- Requirements: RM-a, RM-b (persisted-plan coordination), RM-c
  (operation-union ownership, capability reporting), SE.
- Acceptance evidence: the two review checks in `Verification plan` pass;
  gates pass.
- Conformance check: RFCs 0001 to 0004, 0011, and 0029 unchanged; no RFC
  status changed; no code changed.
- Recovery: revert the milestone commit.
- Remaining gaps: the gate; acceptance.
- Compatibility decision: none.

### EP-M3: reader-side schema version gate

- Outcome: `src/manifest/schema_version.rs` holds the pure policy and typed
  rejection facts; `parse_admitted_document` in `src/manifest/mod.rs` parses
  and admits; rejections render through four new Fluent keys in all 35
  catalogues; two fixtures declare `1.0.0`; `src/manifest/tests/stages.rs`'s
  final-rendering test uses an unknown-field trigger; the users' guide gains
  `### Declare the schema version` under "Author a manifest"; the CHANGELOG's
  `Unreleased` section records a **Breaking** entry; design section 3.3 lists
  admission as the first ingestion stage.
- Requirements: OLD-READER, D-1, D-2, D-4 rung 1, D-15, D-18.
- Acceptance evidence: OBL-1 to OBL-5 discharged; the BDD scenario passes;
  `make check-fmt`, `make typecheck`, `make lint`, `make doc-coverage`,
  `make test`, `make markdownlint`, and `make nixie` pass.
- Conformance check: no public signature changed; no dependency added; no RFC
  syntax admitted; the gate reads no environment or filesystem.
- Recovery: revert the milestone commit; per D-15, ADR-041 then names 12.1.2
  as the owner of rung 1.
- Remaining gaps: rungs 2, 2b, and 4.
- Compatibility decision: the deployed state is users' manifests. Those
  declaring `1.0.0` are unaffected; others receive a diagnostic with a remedy.
  No compatibility layer is added.

### EP-M4: developers' guide and documentation clean-up

- Outcome: `docs/developers-guide.md` gains a section on reserving, admitting,
  and allocating a schema feature under ADR-041 (the D-3 lifecycle, the
  registry, the contract test, and the ladder rungs a new feature must
  implement). `docs/snapshot-testing-in-netsuke-using-insta.md` uses `"1.0.0"`.
- Requirements: `AGENTS.md` documentation obligations.
- Acceptance evidence: gates pass.
- Conformance check: no code changed.
- Recovery: revert the milestone commit.
- Remaining gaps: acceptance.
- Compatibility decision: none.

### EP-M5: acceptance and completion

- Precondition: the user has explicitly accepted ADR-041 to ADR-047 in
  review, possibly with amendments. Until then this plan stays `IN PROGRESS`
  with a sentence beneath the status naming the pending acceptance.
- Outcome: each accepted ADR reads `Accepted` with its date and a one-line
  summary; the design document's registry and resolution table reference the
  accepted ADRs; `docs/roadmap-progressive-enhancement.md` marks 20.1.1 and its
  three sub-bullets `[x]`; this plan is `COMPLETE` with its retrospective
  written.
- Requirements: RM-a ("record accepted decisions").
- Acceptance evidence: gates pass; the roadmap reads `[x]`.
- Conformance check: every amendment requested in review is reflected in the
  ADR, the design document, and this plan's `Decision log`.
- Recovery: revert the milestone commit.
- Remaining gaps: none within 20.1.1.
- Compatibility decision: none.

## Concrete steps

Run everything from the repository root (at planning time,
`/home/leynos/.lody/repos/github---leynos---netsuke/worktrees/5fe5acf9-eb2b-4b75-9130-bc62cb51da74`).
Gates are run sequentially by `scrutineer`, never in parallel, with output
captured to
`/tmp/$ACTION-netsuke-20-1-1-ratify-progressive-enhancement-contracts.out`.

EP-M0. Sweep ADR numbers on every branch:

```bash
git fetch origin
for b in $(git branch -r | grep -v HEAD); do
  git ls-tree --name-only "$b" docs/ | grep -oE 'adr-0[0-9]{2}'
done | sort -u | tail -5
```

Expected at planning time: the last three lines are `adr-038`, `adr-039`, and
`adr-040`. Then ask `scrutineer` for a baseline run of check-fmt, typecheck,
lint, doc-coverage, test, markdownlint, and nixie.

EP-M1 and EP-M2. Write each ADR from the ADR template in
`docs/documentation-style-guide.md` (Status, Date, Context and problem
statement, Decision drivers, Options considered, Decision outcome, Goals and
non-goals, Known risks and limitations, Outstanding decisions). Delegate the
mechanical preamble bullets and `docs/contents.md` entries to `scribe`. Run
`make fmt` after every Markdown edit, check that `git status` shows only the
intended files, then ask `scrutineer` for the full gate set. Run the review
check:

```bash
grep -nE 'ratif|allocate|Allocate' docs/rfcs/002[1-5]-*.md
```

and confirm every hit in an outstanding-decision or compatibility paragraph has
a resolution-table row. Commit each milestone separately.

EP-M3, red. Without production changes, add:

1. `src/manifest/schema_version_tests.rs` with the OBL-1 table, property, and
   class-coverage test, written against the intended `admit` signature. A
   compile failure is the expected red for a new unit.
2. `tests/schema_version_gate_tests.rs` with
   `unsupported_version_is_rejected_before_template_evaluation`,
   `missing_version_is_rejected_before_template_evaluation`, the OBL-2
   property, and the live-probe sibling. Run:

   ```bash
   cargo nextest run --workspace --all-features \
     -E 'test(/rejected_before_template_evaluation/)' \
     2>&1 | tee /tmp/red-netsuke-20-1-1-ratify-progressive-enhancement-contracts.out
   ```

   Expected: both named tests fail, the first because loading succeeds, the
   second because the observer records stages beyond `InitialYamlParsing`.
3. The OBL-4 subprocess test and JSON snapshot test, expected to fail because
   generation succeeds.
4. `tests/schema_version_registry_tests.rs` for OBL-5, expected to fail
   because the constant does not exist.
5. A BDD scenario in `tests/features/manifest.feature`, reusing the existing
   `the error message contains` step because the code is inside the message
   text:

   ```gherkin
   Scenario: Parsing fails for a manifest declaring an unsupported schema version
     Given the manifest file "tests/data/unsupported_schema_version.yml" is parsed
     When the parsing result is checked
     Then parsing the manifest fails
     And the error message contains "netsuke::manifest::schema_version"
   ```

   with `tests/data/unsupported_schema_version.yml` declaring
   `netsuke_version: "1.1.0"` and one hello-world target. Observe red with a
   name filter; do not commit red.

EP-M3, green:

1. Create `src/manifest/schema_version.rs` with a `//!` header, declaring the
   module in `src/manifest/mod.rs`:

   ```rust
   /// The schema versions this reader admits: one major and its highest minor.
   #[derive(Debug, Clone, Copy, PartialEq, Eq)]
   pub(crate) struct SupportedSchema {
       /// Supported major version.
       pub(crate) major: u64,
       /// Highest supported minor version within `major`.
       pub(crate) max_minor: u64,
   }

   /// Schema versions admitted by this release (ADR-041).
   pub(crate) const SUPPORTED: SupportedSchema = SupportedSchema { major: 1, max_minor: 0 };

   /// Reasons a declared schema version cannot be admitted.
   #[derive(Debug, Clone, PartialEq, Eq)]
   pub(crate) enum SchemaVersionRejection {
       /// The root mapping has no `netsuke_version` entry.
       Missing,
       /// The entry is present but is not a string.
       NotAString,
       /// The string is not valid SemVer.
       Invalid,
       /// The version is valid SemVer but outside the supported range or not
       /// in canonical `MAJOR.MINOR.0` form.
       Unsupported {
           /// Declared major version.
           major: u64,
           /// Declared minor version.
           minor: u64,
       },
   }
   ```

   plus `admit(&semver::Version) -> Result<(), SchemaVersionRejection>` (pure
   policy) and
   `admit_document(&ManifestValue) -> Result<(), SchemaVersionRejection>`,
   which returns `Ok(())` for a non-mapping root, reads the entry with
   `.get("netsuke_version")`, parses it with `semver::Version::parse`, and
   delegates to `admit`.
2. In `src/manifest/diagnostics/mod.rs`, add `SchemaVersionDiagnostic` with
   `#[diagnostic(code(netsuke::manifest::schema_version))]` and a `#[help]`
   remedy, and a mapping from each rejection variant: `Missing` to
   `MANIFEST_SCHEMA_VERSION_MISSING`; `NotAString` and `Invalid` to
   `MANIFEST_SCHEMA_VERSION_INVALID`; `Unsupported` to
   `MANIFEST_SCHEMA_VERSION_UNSUPPORTED` with `$declared` rendered as
   `major.minor` and `$supported` as `1.0`; and the help text to
   `MANIFEST_SCHEMA_VERSION_HELP`. Every variant field is read by this mapping.
3. Declare the four keys in `src/localization/keys.rs` and add messages to
   every `locales/*/messages.ftl`, following `docs/translators-guide.md`, with
   `[netsuke::manifest::schema_version]` inside each message's text and
   identifiers and numbers untranslated. The `en-US` unsupported message,
   wrapped here for width but one line in the catalogue:

   ```plaintext
   [netsuke::manifest::schema_version] { $name } declares manifest schema
   { $declared }, but this Netsuke supports schema { $supported }.
   ```

   and the help message:

   ```plaintext
   Upgrade Netsuke to use a newer schema. netsuke_version is the manifest
   schema version, not the Netsuke release version; write it as 1.0.0.
   ```

4. Replace the YAML-parse block in `evaluate_manifest` with a call to a new
   `parse_admitted_document(yaml, name)` helper that parses and admits, mapping
   a rejection into `ManifestError::Parse` with the `MANIFEST_PARSE` summary.
5. Change `tests/data/jinja_is.yml` and `tests/data/jinja_is_missing.yml` to
   `netsuke_version: "1.0.0"`. Change the trigger in
   `stage_callback_stops_after_final_rendering_failure`
   (`src/manifest/tests/stages.rs`) from a missing version to an unknown
   top-level key, and update its comment.
6. Confirm `serde-saphyr`'s enabled features in `Cargo.toml` (AXIOM-2).
7. Add the users' guide section, the CHANGELOG entry, and the design section
   3.3 stage; run `make fmt`.
8. Re-run the focused tests, review new snapshots with `cargo insta review`,
   then ask `scrutineer` for the full gate set.

Expected file set for EP-M3, excluding the 35 catalogues and new snapshots:
`src/manifest/schema_version.rs`, `src/manifest/schema_version_tests.rs`,
`src/manifest/mod.rs`, `src/manifest/diagnostics/mod.rs`,
`src/localization/keys.rs`, `src/manifest/tests/stages.rs`,
`src/diagnostic_json_tests.rs`, `tests/schema_version_gate_tests.rs`,
`tests/schema_version_registry_tests.rs`, `tests/assert_cmd_tests.rs`,
`tests/features/manifest.feature`, `tests/data/unsupported_schema_version.yml`,
`tests/data/jinja_is.yml`, `tests/data/jinja_is_missing.yml`,
`docs/users-guide.md`, `CHANGELOG.md`, and `docs/netsuke-design.md`: seventeen
files. If a new integration-test file must be registered with a Cargo
`[[test]]` entry or nextest group, that adds one file (see the repository's
integration-test wiring contract).

EP-M3, refactor: keep `src/manifest/mod.rs` and every new file under 400 lines
and every function under Clippy's 70-line ceiling; re-run the focused tests and
the full gate set, then commit.

EP-M4. Edit the guides, run `make fmt`, gate, and commit.

EP-M5. After explicit acceptance, set each ADR's status to `Accepted` with the
date and a one-line summary, apply any accepted amendments across ADRs, the
design document, and this plan, mark the roadmap entries done, set this plan's
status to `COMPLETE`, gate, and commit.

## Validation and acceptance

Quality criteria:

- Tests: `make test` passes; the tests named under OBL-1 to OBL-5 and the new
  BDD scenario fail before EP-M3's green step and pass after it.
- Verification: OBL-1 to OBL-5 discharged with the evidence listed.
- Lint and types: `make check-fmt`, `make typecheck`, `make lint`, and
  `make doc-coverage` pass after every milestone.
- Documentation: `make markdownlint` and `make nixie` pass after every
  milestone.
- Security: an unsupported manifest performs no template evaluation (OBL-2),
  and diagnostics echo no manifest text (D-18).

Behavioural acceptance after EP-M3:

```bash
mkdir -p /tmp/schema-gate-netsuke
printf 'netsuke_version: "1.1.0"\ntargets:\n  - name: hello.txt\n    command: "echo hi > hello.txt"\n' \
  > /tmp/schema-gate-netsuke/Netsukefile
cargo run --quiet -- --directory /tmp/schema-gate-netsuke generate --output /tmp/schema-gate-netsuke/out.ninja
echo "exit=$?"; test -e /tmp/schema-gate-netsuke/out.ninja && echo written || echo absent
```

Expected: a diagnostic containing `netsuke::manifest::schema_version` and the
supported schema `1.0`, a non-zero exit, and `absent`. Replacing `1.1.0` with
`1.0.0` exits zero and prints `written`.

## Idempotence and recovery

Every step is repeatable; re-running `make fmt` reapplies Markdown formatting.
Each milestone is one commit, so any milestone can be reverted with
`git revert` without disturbing earlier ones. If EP-M3 is reverted, amend
ADR-041's D-15 paragraph in the same revert series. The `/tmp` probe
directories may be deleted at any time.

## Artefacts and notes

Ninja behaviour probe (Ninja 1.11.1), supporting D-11:

```plaintext
ninja: error: build.ninja:5: unknown pool name 'nope'
[1/1] true                      # depth = 0 accepted: unlimited pool
ninja: error: build.ninja:2: invalid pool depth
ninja: fatal: ninja version (1.11.1) incompatible with build file ninja_required_version version (99.0).
```

Prior art consulted through Firecrawl, for ADR-041's options section:

- Ninja manual: pools arrived in 1.1; the `console` pool in 1.5 has depth 1;
  `ninja_required_version` makes an older Ninja refuse a newer build file.
- Nagios plugin guidelines: statuses 0 OK, 1 WARNING, 2 CRITICAL, 3 UNKNOWN;
  codes outside that range are out of range; plugins should finish within about
  ten seconds; performance data follows `|`. Supports D-14's defaults.
- Cargo: unstable syntax is opted into with `cargo-features`; `rust-version`
  is an unchecked tool floor
  (<https://doc.rust-lang.org/cargo/reference/rust-version.html>); editions
  carry breaking changes.
- Docker Compose: the top-level `version` is informative only and unknown
  fields warn
  (<https://docs.docker.com/reference/compose-file/version-and-name/>).
- Kubernetes: each object carries its own `apiVersion`, and fields are
  removed only by a version increment
  (<https://kubernetes.io/docs/reference/using-api/deprecation-policy/>).
- Terraform `required_version`: configurations declare accepted tool
  versions and older tools refuse them.

Expert-panel verdicts on the first draft: structure and contracts "revise";
failure modes "approve with revisions"; alternatives "approve with revisions";
process and viability "revise before approval". Every finding is addressed in
this revision; see `Revision note`.

## Interfaces and dependencies

No new crates. `semver` provides `Version` and `VersionReq`; `proptest`,
`rstest`, `rstest-bdd`, `insta`, `googletest`, and `pretty_assertions` provide
testing. `ortho_config` is not touched by EP-M3, because the gate has no
configuration; D-10 records how the owning tasks will add operator ceilings
through it.

At the end of EP-M3 these crate-private items exist:
`crate::manifest::schema_version::{SupportedSchema, SUPPORTED,
SchemaVersionRejection, admit, admit_document}`,
`crate::manifest::parse_admitted_document`, and a `SchemaVersionDiagnostic`
source diagnostic carried by the existing `ManifestError::Parse`.

Hexagonal boundary: `admit` is domain policy with no knowledge of YAML, Fluent,
or the CLI. `admit_document` is a narrow translation from the parsed document
shape. Rendering happens only in `src/manifest/diagnostics`. No port or adapter
is introduced, because the gate needs no infrastructure; the existing
`EnvReader` and stage-observer seams are used only by tests.

Signposts for the implementer:

- Documentation: `docs/netsuke-design.md` (sections 2.2, 2.9, and 3.3),
  `docs/documentation-style-guide.md` (ADR template, RFC preambles),
  `docs/rust-testing-with-rstest-fixtures.md`, `docs/rstest-bdd-users-guide.md`,
  `docs/reliable-testing-in-rust-via-dependency-injection.md`,
  `docs/rust-doctest-dry-guide.md`, `docs/ortho-config-users-guide.md` (D-10),
  `docs/snapshot-testing-in-netsuke-using-insta.md`,
  `docs/translators-guide.md`,
  `docs/formal-verification-methods-in-netsuke.md`, and ADR-019 with RFC 0011
  as the ratification precedent.
- Skills: `execplans` (keeping this plan current); `rust-router`, then
  `rust-errors` for the rejection type, `rust-unit-testing` and `proptest` for
  OBL-1 and OBL-2; `hexagonal-architecture` for the boundary note above;
  `arch-decision-records` for the ADRs; `en-gb-oxendict-style` for prose;
  `codegraph-mcp` for callers of `evaluate_manifest`.
- Agents: `scrutineer` runs every gate; `scribe` performs mechanical
  documentation edits; `wyvern` performs read-only lookups.

## Revision note

2026-09-27, first draft: initial plan.

2026-09-27, second draft, after an expert-panel review of four reviewers. What
changed:

- Ratification now follows the normal ADR process. ADRs are drafted as
  `Proposed` and accepted only by explicit user acceptance (new EP-M5). RFCs
  stay `Proposed` with governing-decision bullets, as RFC 0011 does.
  Contradicting RFC and roadmap sentences are amended.
- The single large ADR-042 is split: ADR-042 keeps cross-cutting contracts,
  and ADR-043 to ADR-047 hold one RFC each. The resolution table moves to the
  design document.
- D-1 now requires canonical `MAJOR.MINOR.0` and holds the supported range as
  data. D-3 gains a full release lifecycle. D-4 defines what counts as syntax
  and adds rung 2b. D-5 records the rejected alternatives. D-6 is new and
  covers composed and nested documents. D-8 registers union discriminators and
  settles direct-value operations. D-9 reduces persisted formats to
  coordination constraints with the safety clause for state records. D-10 is
  new and gives one ceiling-merge rule. D-11 gives `capacity` its own bound.
  D-12 protects only unproduced sources. D-13 no longer decides ownership
  reserved for 20.2.1. D-17 and D-18 are new.
- EP-M3 now skips non-mapping roots, pre-authorizes the
  `parse_admitted_document` extraction, lists every affected test, moves the
  users' guide and CHANGELOG into the milestone, uses the stage observer as a
  sentinel, asserts the JSON code where it actually appears, uses `--output`
  for the file-absence check, documents every private item, and never echoes
  manifest text. OBL-1 gains an explicit class-coverage test and OBL-5 binds
  the registry to the reader.

Effect on remaining work: the plan awaits user approval. Implementation follows
EP-M0 to EP-M5.
