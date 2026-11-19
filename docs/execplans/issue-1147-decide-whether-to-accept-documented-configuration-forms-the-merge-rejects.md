# Decide whether to accept documented configuration forms the merge rejects

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: COMPLETE

Every milestone is delivered and validated; the only remaining step is the pull
request merge.

Branch:
`issue-1147-decide-whether-to-accept-documented-configuration-forms-the-merge-rejects`

Roadmap item: none; tracked by GitHub issue #1147.

## Purpose / big picture

The users' guide documented three configuration forms that the configuration
merge rejects. This plan decides, one form at a time, whether to accept the
documented form or to keep rejecting it, and it makes the implementation, the
tests, and the guide agree.

The user-visible outcomes are:

1. `NETSUKE_JSON=1` and `NETSUKE_VERBOSE=1` work, as do `0`, `false`, and
   `true`. The early diagnostic-mode resolver and the full merge accept exactly
   the same `NETSUKE_JSON` spellings.
2. Policy values such as `color = "AlWaYs"`, `NETSUKE_COLOR=ALWAYS`, and
   `accessibility = "ON"` are accepted from configuration files and the
   environment, matching the case-insensitive `--color` command-line behaviour.
3. Indexed list variables such as `NETSUKE_DEFAULT_TARGETS__0` remain rejected.
   The guide instead documents the scalar form and the serialized JSON-array
   form.

A configuration file keeps strict Boolean parsing: `json = 1` remains invalid.
Policy values are matched case-insensitively but are not trimmed, so
`color = " always "` remains invalid. Canonical lowercase spellings are still
written on serialization.

The decision for all three forms is recorded in a new Architectural Decision
Record (ADR) and referenced from the CLI design document. A migration-guide
entry records the broadened acceptance and the previously unsupported indexed
list form.

## Constraints

- Accept `1`/`0` only on the five existing Boolean configuration paths:
  `verbose`, `json`, `fetch_default_deny`, `trust_project_fetch_policy`, and
  `no_input`. Do not broaden Boolean parsing in configuration files.
- Keep `FromStr` for the four display policies public with
  `Err = String` and unchanged error strings. Serialization stays canonical
  lowercase.
- Do not trim policy strings and do not case-fold non-ASCII policy input.
- Keep the existing `NETSUKE_JSON` validation error text, non-Unicode
  rejection, discovery-first precedence, and deferral of quarantined-policy
  errors.
- Do not modify completed ExecPlans under `docs/execplans/`; this new plan is
  exempt from its own size budgets.
- No new dependencies and no public API changes beyond case-insensitive
  `Deserialize` for the display policies.
- Tests must not mutate the harness process environment. Use the existing
  `isolated_environment` and `merge_in_child` child-process seam.
- Keep every code file within the repository's 400-line limit.

## Tolerances (exception triggers)

- Files: stop and escalate if more than 20 non-ExecPlan files change.
- Code: stop and escalate if production, test, or build logic exceeds 900 net
  lines (additions minus deletions).
- Configuration: stop and escalate if declarative configuration exceeds 50 net
  lines; none is expected.
- Documentation: stop and escalate if prose outside ExecPlans exceeds 500 net
  lines.
- Interface: stop and escalate if a public signature other than the policy
  `Deserialize` implementations must change.
- Dependencies: stop and escalate if a new dependency is required.
- Iterations: stop and escalate if a focused test still fails after three
  distinct fix attempts.
- Ambiguity: stop and escalate if a documented decision cannot be implemented
  without contradicting another regression test.

## Risks

- Risk: accepting `1`/`0` in the environment layer could accidentally accept it
  in configuration files. Severity: medium Likelihood: low Mitigation:
  normalize only in `src/cli/environment/mod.rs::parse_entry`, and pin
  `json = 1` rejection in a file-layer test.
- Risk: a lower invalid layer may or may not be masked by a higher valid layer,
  which changes what the early resolver must do. Severity: medium Likelihood:
  low Mitigation: characterize masking first with direct `MergeComposer` tests,
  then keep the `--json` short circuit only if the characterization proves
  masking.
- Risk: replacing derived `Deserialize` on the policy enums could change
  accepted spellings or error text. Severity: medium Likelihood: medium
  Mitigation: delegate to the existing `FromStr`, keep
  `rename_all = "kebab-case"` on `Serialize`, and test lowercase, uppercase,
  and mixed-case input through both `toml` and `serde_json`.
- Risk: the ADR number may collide with unpublished work.
  Severity: low Likelihood: low Mitigation: confirm the highest allocated
  number in `docs/adrs/` immediately before creating the file.

## Progress

- [x] (2026-11-18) Reconnaissance of the environment layer, diagnostic
  resolver, policy enums, merge pipeline, tests, and user guide.
- [x] (2026-11-18) Plan recorded.
- [x] (2026-11-18) `EP-M1` strict shared Boolean parser, normalized environment
  layer, and masking characterization.
- [x] (2026-11-18) `EP-M2` case-insensitive policy deserialization.
- [x] (2026-11-18) `EP-M3` full-merge, cached-merge, and binary regression tests
  written. The focused suite passes (`150` tests), including the masking
  characterization (`3` tests).
- [x] (2026-11-18) `EP-M4` ADR-0083, design-document reference, contents index,
  user guide, migration guide, and changelog entry drafted.
- [x] (2026-11-18) Module-layout and lint corrections: both test modules
  that gained submodules moved to `mod.rs`, and the policy enums moved to
  `src/cli/config/policies.rs` so `src/cli/config/mod.rs` stays under the
  400-line cap.
- [x] (2026-11-18) All code quality gates pass; draft pull request #1164
  opened. Evidence: `make check-fmt`, `make lint` (including all four Whitaker
  passes, Python lint, and `github-actions-lint`), `make typecheck`,
  `make doc-coverage` (98.29% against the 80% bar), `make test` (6125 nextest
  tests and the doctest pass), `make markdownlint`, `make nixie`, and
  `make test-documentation-contracts` (15 passed).
- [x] (2026-11-18) Advisory CodeScene follow-up: the full-merge helpers gained
  a declarative `MergeCase`, and the four per-key Boolean merges and the two
  mixed-case policy merges became `rstest` tables. CodeScene reported code
  duplication in `env_value_forms/mod.rs`, `boolean_values.rs`, and
  `policy_values.rs` (scores 9.10 and 9.39); `cs check` now scores all four
  files 10.00 with no warnings. That check is not in the required set, so the
  refactor is a separate commit after the functional one.
- [x] (2026-11-18) CodeRabbit agent review re-run on the rebased head after
  the refactor landed. The first attempt had ended with
  `Connection failed: WebSocket closed`, which the tool reports as recoverable;
  the retry completed and returned one minor finding: the changelog, ADR-0083,
  and users' guide present `0` as accepted for `NETSUKE_NO_INPUT` without
  saying that configuration then rejects `false`.
- [x] (2026-11-18) CI `build-test` was red on `9b36dca8d`: the Kani scope
  contract reported that the harnesses reach `src/cli/config/policies.rs`, which
  `tools/kani/proof-scope.toml` did not cover. The gate brief for that batch
  had omitted `make test-workflow-contracts`, so the move in `D7` was never
  checked against the manifest. Reproduced locally (`1 failed`, `2439 passed`)
  and fixed by adding the module in sorted position; the contract suite then
  passed. Commit `5dba44571`.
- [x] (2026-11-18) Answered the CodeRabbit finding in all three documents:
  `NETSUKE_NO_INPUT=0` parses to `false` and is then rejected by the no-input
  policy, so only `true` and `1` are usable. Commit `338687099`.
- [x] (2026-11-18) Five-gate delta run on `76b98be0d` green: `make check-fmt`
  (5s), `make markdownlint` (48s, 238 files, 0 issues), `make nixie` (2s, 239
  files), `make test-documentation-contracts` (15 passed), and
  `make test-workflow-contracts` (2440 passed, 3 skipped). The last is the
  decisive check for the proof-scope entry.
- [x] (2026-11-18) CodeRabbit's second pass raised the same
  `NETSUKE_NO_INPUT=0` omission in the migration guide and the CLI design
  document; both now state the exception beside the accepted spellings.
- [x] (2026-11-18) Second batch on `debbf370a`, the Markdown-only delta that
  carries the two guide fixes and these records: `make check-fmt` (4s),
  `make markdownlint` (35s, 238 files, 0 issues), and `make nixie` (1s, 239
  files), all green.
- [x] (2026-11-18) CodeRabbit's final pass on `cbcf89586` returned no verified
  findings and one unverified one: the `NETSUKE_JSON` binary tests inherited
  the test process environment. Both runs now use
  `test_support::netsuke::run_netsuke_in_with_env`, which clears the child
  environment and supplies an isolated `PATH`, `HOME`, and `XDG_CONFIG_HOME`;
  the focused suite passes (`2` tests).
- [x] (2026-11-18) Rebased onto `origin/main` at `8daa9143`, which adds
  `338ff4ac4` (public-consumer release smoke) and `8daa91431` (stdlib
  file-reading filters within the workspace capability). No conflicts: all ten
  commits replayed with identical patches (`range-diff` reports `=` for each),
  and main's 130 changed files overlap this branch only in `CHANGELOG.md`,
  `docs/developers-guide.md`, `docs/users-guide.md`, and
  `docs/v0-1-0-migration-guide.md`, where both sides' entries survive. Main
  changed no source, test, or manifest file this branch touches, so no branch
  work needed to adopt a new pattern or helper.
- [x] (2026-11-19) Review assessment: the three Whitaker
  `no_unwrap_or_else_panic` sites it named at `0c9aca0` were already replaced by
  `bd7de32e2`; neither file contains `unwrap_or_else` at the current head. The
  second finding was real: the early JSON resolver read only `NETSUKE_JSON`, so
  `netsuke_json=1` reached the merge but not the mode chosen before it.
- [x] (2026-11-19) `EP-M5` shared key normalization. The merge's
  normalization became `normalized_components` in `src/cli/environment/mod.rs`,
  and the resolver in `src/cli/diag/mod.rs` calls it instead of repeating the
  rule. `src/cli/diag/alias_tests.rs` compares the resolver with
  `merge_with_cached_file_layers` for the canonical, lower-case, mixed-case,
  and whitespace-trimmed spellings, for the same spellings when the merge then
  fails, and for three keys that name other paths. ADR-0083 and the users'
  guide record the behaviour. Red probe: with the snapshot scan disabled, four
  of the nine alias cases fail and the others still pass, so the suite is not
  vacuous.
- [x] (2026-11-19) Rebased onto `origin/main` at `59637f56f`, which lands the
  release write-authorization work. One conflict: both sides appended an
  ADR-0082 entry to `docs/contents.md`. Resolved by keeping both entries and
  renumbering this branch's record to ADR-0083, along with its references in
  the changelog, contents index, CLI design document, migration guide, and this
  plan.
- [x] (2026-11-19) Review documentation corrections. The review found the
  orientation section still describing the pre-change resolver, and the users'
  guide's list-variable rule not excluding `shells`. Both are corrected here,
  and the orientation section now names `normalized_components`,
  `parse_environment_bool`, `BOOLEAN_PATHS`, and the
  `src/cli/config/policies.rs` location.
- [x] (2026-11-20) Review round three: the Figment description. The
  orientation section listed Booleans, numbers, arrays, and strings, which read
  as an exhaustive set. Figment's `Value::from_str` also parses floats and
  `{key=value}` dictionaries, so both the plan and the developers' guide now
  describe the recognized forms as non-exhaustive and name the string fallback.
  The ADR and decision D6 say "value parsing" where they had said "scalar
  parsing", because arrays and dictionaries are values too.

## Surprises & discoveries

- Observation: the current guide still documents all three forms, so the audit
  described by issue #1147 has not yet removed them from this branch. Evidence:
  `docs/users-guide.md` still shows `NETSUKE_JSON=1`, `color = "AlWaYs"`, and
  `NETSUKE_DEFAULT_TARGETS__0`. Impact: the guide must be corrected as part of
  this work for the rejected form, and updated for the two newly accepted forms.
- Observation: `src/cli/l10n/mod.rs::parse_bool_hint` is a third, lenient
  `NETSUKE_JSON` reader used before full parsing; it accepts `yes`, `on`, `no`,
  and `off` and falls back to `false`. Evidence:
  `src/locale/resolution.rs::resolve_startup_json` calls it. Impact: it is a
  pre-parse display hint, not an acceptance path; it is left unchanged and its
  difference from the strict resolver is recorded in the ADR.
- Observation: the environment layer keeps values untrimmed, but Figment's
  text parser trims an unquoted scalar before storing it. A policy value
  therefore behaves asymmetrically: `NETSUKE_COLOR=' always '` is accepted,
  while a quoted file value `color = " always "` is rejected. Evidence:
  `figment-0.10.19/src/value/parse.rs` trims the non-quoted branch of
  `value()`, and the focused run showed `" AlWaYs "` merging as `always`; the
  new `non_boolean_string_values_are_trimmed` unit test pins it. Impact: the
  guide, migration guide, and ADR state the trim as environment parser
  behaviour rather than claiming whitespace is rejected everywhere.
- Observation: the workspace denies `clippy::self-named-module-files`, so a
  module that gains submodules must use the directory form. Adding
  `src/cli/environment/tests/boolean_values.rs` and
  `tests/cli_tests/merge/env_value_forms/*` made the existing sibling files
  `tests.rs` and `env_value_forms.rs` lint errors; both became `mod.rs` files.
  Impact: the ExecPlan's artefact paths follow the new layout, and a future
  test file added beside one of these modules belongs in the directory.

## Decision log

- D1. Accept `1`/`0` for the five Boolean environment variables only.
  Rationale: `1` is the conventional continuous-integration spelling, while
  configuration files remain strongly typed and unchanged. The alternative,
  keeping the rejection and correcting the guide, was rejected because it makes
  the early and later `NETSUKE_JSON` resolvers disagree. Date/Author:
  2026-11-18, agent.
- D2. Accept mixed-case policy values from every layer.
  Rationale: `FromStr` is already case-insensitive and drives the CLI; making
  `Deserialize` delegate to it removes a surprising difference between sources.
  The alternative, a case-sensitive guide correction, was rejected because the
  issue records case-insensitivity as the less surprising behaviour.
  Date/Author: 2026-11-18, agent.
- D3. Keep rejecting indexed `__N` list variables.
  Rationale: the scalar and JSON-array forms already cover the use case, and
  the nested-map semantics are well defined. The guide will document the
  supported forms and the rejection. Date/Author: 2026-11-18, agent.
- D4. Keep the explicit `--json` short circuit.
  Evidence: `tests/cli_tests/merge/layer_masking.rs` proves a valid higher
  layer masks an invalid lower layer before the single deserialization, so
  skipping the lower-priority malformed environment value is consistent with
  the merge contract rather than a special case. Date/Author: 2026-11-18, agent.
- D5. Extend `Deserialize` tests to cover TOML as well as JSON.
  Rationale: the merge boundary receives both file-derived and environment
  values, so proving only `serde_json` would leave the file layer untested.
  Date/Author: 2026-11-18, agent.
- D6. Document environment-side trimming instead of forcing the environment
  layer to preserve policy whitespace. Rationale: the trim is Figment's
  ordinary value parsing and already applies to every string-valued environment
  variable; removing it for four policy keys would add a second special-case
  list and make `NETSUKE_LOCALE=" en-GB "` behave unlike
  `NETSUKE_COLOR=" always "`. Accepting the trim keeps one rule for all
  environment strings, and the file layer still rejects whitespace.
  Date/Author: 2026-11-18, agent.
- D7. Move the four policy enums into `src/cli/config/policies.rs`.
  Rationale: the manual `Deserialize` implementations pushed
  `src/cli/config/mod.rs` from 352 to 413 lines, past the 400-line file cap.
  The enums and their shared deserializer form one coherent unit, and `mod.rs`
  re-exports the types so every existing path, including
  `src/cli/policy/values.rs`, keeps working. Date/Author: 2026-11-18, agent.
- D8. Replace the repeated merge-test bodies with declarative cases.
  Rationale: `AGENTS.md` asks for duplicated tests to become parameterized
  cases, and CodeScene flags the four near-identical Boolean merges and the two
  mixed-case policy merges. A `MergeCase` value names the file, the environment
  above it, and the command line, so the success and failure adapters share one
  sandbox preparation and each test states only what makes it distinct. The
  masking case stays a named `rstest` row rather than a merged body, so the
  layering contract remains visible. Date/Author: 2026-11-18, agent.
- D9. Share the merge's key normalization with the early resolver rather than
  documenting the alias difference as a limitation. Rationale: the two passes
  make the same decision about whether to print human or JSON diagnostics, so a
  spelling the merge accepts and the resolver ignores makes a merge failure
  print human output for a user who asked for JSON. The canonical spelling is
  still consulted first, which leaves a case-insensitive platform resolving the
  same entry as before. Duplicate spellings of one key are a merge error, and
  the resolver answers from the canonical entry until that error is reported in
  the selected mode. Date/Author: 2026-11-19, agent.
- D10. Renumber this branch's ADR to 0083 rather than reusing 0082. Rationale:
  main allocated ADR-0082 to the release write-authorization decision after
  this branch opened, and two records sharing a serial would make the contents
  index ambiguous. The next free number keeps both records addressable and
  costs only reference updates. Date/Author: 2026-11-19, agent.

## Outcomes & retrospective

Delivered the three decisions issue #1147 asked for. `NETSUKE_JSON=1` and
`NETSUKE_VERBOSE=1` now merge as Booleans, and the early diagnostic resolver
and the full merge share one strict parser, so both accept exactly `true`,
`false`, `1`, and `0`. Every policy spelling is accepted in any ASCII casing
from files, the environment, and the command line, and a rejection lists the
accepted spellings. Indexed `__N` list variables remain rejected, and the
users' guide now documents the scalar and serialized JSON-array forms instead.

ADR-0083 records the decision, the rejected alternatives, and the known risk
that `BOOLEAN_PATHS` mirrors the `CliConfig` Boolean fields by hand. The
upstream capability that would remove that workaround is requested in
leynos/ortho-config#714, and removing it is tracked in leynos/netsuke#1165. The
users' guide, migration guide, CLI design document, developers' guide, and
changelog are updated.

Lessons:

- Layer masking decides what the merge can accept: a valid higher-layer value
  hides an invalid lower-layer one before the single deserialization, which is
  what makes the early `--json` short circuit consistent with the merge.
- Two lint oracles constrain test error handling at once. Whitaker's
  `no_unwrap_or_else_panic` and `clippy::expect_fun_call` between them rule out
  both idiomatic forms, so the helpers use an explicit `match` arm.
- The 400-line cap applies as work lands, not only when a file is created; the
  policy enums moved to `src/cli/config/policies.rs` once the new
  implementations pushed `mod.rs` over the limit.
- A module move can change more than the Rust build. The workflow-contract
  suite is outside the eight-gate set, and it owns the Kani scope contract that
  recomputes the harness closure from the source. It must therefore run
  whenever a module the proofs already reach is split, or a new one is created.

## Context and orientation

Netsuke is a Rust build tool whose layered configuration is composed through
`ortho_config`. `src/cli/config/mod.rs` defines `CliConfig`, the typed schema.
`src/cli/merge/mod.rs` composes four layers: defaults, discovered files,
environment, and explicit command-line overrides. `ortho_config` merges the
JSON objects and deserializes the result once, so a higher layer's value masks
a lower layer's value.

The environment layer is `src/cli/environment/mod.rs::EnvironmentLayer`.
`normalized_components` in that module trims surrounding whitespace, strips the
`NETSUKE_` prefix without regard to ASCII case, splits the remainder on `__`,
and lowercases each component. The five single-component paths in
`BOOLEAN_PATHS` are parsed by `parse_environment_bool`, which accepts exactly
`true`, `false`, `1`, and `0` with no trimming and no case folding; every other
path keeps Figment's own value parsing, which recognizes values including
Booleans, numbers, arrays, dictionaries, and quoted strings; anything it cannot
parse becomes a string.

The early diagnostic-mode resolver is `src/cli/diag/mod.rs`. It reads the key
naming the `json` path through an injected `EnvProvider`, trying the canonical
`NETSUKE_JSON` entry first and then scanning the snapshot with the same
`normalized_components` function the merge uses. It parses the value with the
shared `parse_environment_bool` and otherwise reports an
`OrthoError::Validation`. It runs before the full merge so failures can be
reported in the requested output mode, and sharing both functions keeps the
mode it selects identical to the mode the merge would select.

The four display policies live in `src/cli/config/policies.rs` with canonical
spellings in `src/cli/config/policy_definitions.rs`. Their `FromStr`
implementations use `parse_policy`, which compares with `eq_ignore_ascii_case`.
Their `Deserialize` implementations delegate to that `FromStr`, so a
configuration file accepts the same case-insensitive spellings as the command
line, while `rename_all = "kebab-case"` keeps serialized output canonical and
lowercase. The same enums are serialized into the CLI override layer, so
canonical lowercase output matters.

Relevant tests are `src/cli/environment/tests/mod.rs`,
`src/cli/config/tests.rs`, the diagnostic-side modules
`src/cli/diag/alias_tests.rs` and `src/cli/diag/quarantined_policy_tests.rs`
plus the inline test module in `src/cli/diag/mod.rs`,
`tests/cli_tests/merge/mod.rs`, `tests/cli_tests/merge_diag.rs`,
`tests/cli_tests/merge_diag_proptests.rs`, and the binary-level
`tests/logging_stderr/json.rs`.

Existing test infrastructure: `tests/cli_tests/merge_probe.rs` runs the real
merge in a child process with an injected environment via
`merge_in_child(args, current_dir, environment)`; `isolated_environment` builds
a sandboxed `HOME`/`XDG_CONFIG_DIRS`. `tests/cli_tests/merge/mod.rs` also
contains `with_config_file` for file-layer cases. `rstest`, `proptest`,
`insta`, and `assert_cmd` are already configured.

Gate commands live in the `Makefile`. The required sequence for this change is
`make check-fmt`, `make lint`, `make doc-coverage`, `make test`,
`make markdownlint`, and `make test-documentation-contracts`. The repository
runs these through the `scrutineer` subagent.

## Conformance basis

- Issue #1147, "Decide whether to accept documented configuration forms the
  merge rejects", inspected 2026-11-18.
- `docs/netsuke-cli-design-document.md`, "Configuration and Customization
  (Ortho-Config Integration)", inspected 2026-11-18.
- `docs/documentation-style-guide.md`, ADR and migration-guide requirements,
  inspected 2026-11-18.
- `docs/adrs/adr-0021-keep-project-fetch-policy-below-the-operator-ceiling.md`,
  for the `NETSUKE_TRUST_PROJECT_FETCH_POLICY` opt-in, inspected 2026-11-18.
- `docs/adrs/adr-0072-display-policy-contract-and-terminal-verification.md`, for
  the display-policy contract, inspected 2026-11-18.

Named requirements:

- `REQ-bool-env`: the environment layer accepts `true`, `false`, `1`, and `0`
  on the five Boolean paths only.
- `REQ-bool-agree`: the early `NETSUKE_JSON` resolver and the full merge accept
  the same `NETSUKE_JSON` values.
- `REQ-policy-case`: all four display policies deserialize
  case-insensitively from files and the environment.
- `REQ-indexed-reject`: indexed `__N` list variables remain rejected and are
  documented as unsupported.
- `REQ-tests`: every outcome is exercised through the full merge, not only
  through `str::parse` or the early resolver.

Trace chain:

```plaintext
REQ-bool-env -> EP-M1 -> tests/cli_tests/merge/env_value_forms/mod.rs
REQ-bool-agree -> EP-M1/EP-M3 -> tests/cli_tests/merge_diag.rs
REQ-json-key-agree -> EP-M5 -> src/cli/diag/alias_tests.rs
REQ-policy-case -> EP-M2/EP-M3 -> tests/cli_tests/merge/env_value_forms/mod.rs
REQ-indexed-reject -> EP-M3 -> tests/cli_tests/merge/env_value_forms/mod.rs
REQ-tests -> EP-M3 -> tests/logging_stderr/json.rs
```

## Verification plan

### Abstract model

For each configuration key, a value is selected from the highest-priority layer
that supplies it. The environment layer maps an accepted Boolean spelling to a
Figment Boolean and every other spelling on a Boolean path to a string. The
merge deserializes only the selected value. A display policy value is accepted
when its ASCII case-insensitive form equals one of the canonical spellings, and
rejected otherwise.

### Invariants and lemmas

- `INV-bool-set`: for each Boolean path, the environment layer emits a Boolean
  for exactly `{true, false, 1, 0}` and a string for anything else.
- `INV-bool-agree`: for any raw `NETSUKE_JSON` value, the early resolver and the
  full merge either both succeed or both fail.
- `INV-key-agree` (added 2026-11-19): for any environment key whose
  normalization yields the single `json` path, the early resolver reads the
  same value the merge reads, so both select the same diagnostic mode,
  including when the merge then fails. Keys that normalize to any other path
  leave the mode to the configuration files.
- `INV-policy-set`: a policy value is accepted exactly when
  `parse_policy(value, definitions)` is `Some`.
- `INV-policy-canonical`: serializing any policy writes its canonical lowercase
  spelling.
- `INV-indexed`: a key with more than one component that targets a list field
  is not converted into an array.

### Axioms (trusted, not verified here)

- `AXIOM-figment`: Figment's `Value::from_str` recognizes `true` and `false` as
  Booleans and integer text as numbers.
- `AXIOM-ortho-merge`: `ortho_config` deserializes the merged object once, so a
  higher layer masks a lower layer's value.
- `AXIOM-toml`: the `toml` crate's `Deserializer` hands strings to
  `Deserialize` unchanged.

### Methods per obligation

- Obligation: `INV-bool-set`.
  Method: parameterized `rstest` cases over all five paths and all accepted and
  rejected spellings. Rationale: the input domain is finite and normative.
  Domain: `true`, `false`, `1`, `0`, `True`, `yes`, `2`, empty, and whitespace.
  Artefact: `src/cli/environment/tests/mod.rs`. Evidence:
  `cargo test -p netsuke cli::environment` passes; a test fails if the
  normalization is removed. Non-vacuity: both accepted and rejected values are
  asserted, and a non-Boolean path keeps its numeric value.
- Obligation: `INV-bool-agree`.
  Method: fixed and property tests through the real resolver and merge.
  Rationale: the two code paths are independently implemented. Domain: raw
  strings including `1`, `0`, `true`, `false`, `yes`, `2`, and the empty
  string, with and without `--json`. Artefact: `tests/cli_tests/merge_diag.rs`
  and `tests/cli_tests/merge_diag_proptests.rs`. Evidence: both paths return
  the same success or rejection verdict. Non-vacuity: the generated set
  includes at least one accepted and one rejected spelling, and the
  `--json`-masked case is exercised separately.
- Obligation: `INV-key-agree`.
  Method: parameterized `rstest` cases that run the real resolver and the real
  cached merge over one injected environment. Rationale: the two passes are
  separate code, and only running both binds them together. Domain: the
  canonical, lower-case, mixed-case, and whitespace-trimmed spellings; the same
  spellings with a merge rejection beside them; and `NETSUKE_JSONX`,
  `NETSUKE_JSON__NESTED`, and `NETSUKE_VERBOSE` as controls. Artefact:
  `src/cli/diag/alias_tests.rs`. Evidence:
  `cargo nextest run -p netsuke-build --lib -E 'test(cli::diag)'` passes all 22
  cases. Non-vacuity: disabling the snapshot scan fails four alias cases while
  the three controls still pass, so the controls do not stand in for the alias
  cases and the scan is what the alias cases exercise.
- Obligation: `INV-policy-set`.
  Method: parameterized and property tests over all policy definitions.
  Rationale: the spellings are a finite set and `FromStr` already defines the
  oracle. Domain: lowercase, uppercase, mixed case, invalid strings, and
  non-string inputs. Artefact: `src/cli/config/tests.rs`. Evidence: TOML and
  `serde_json` deserialization agree with `FromStr`. Non-vacuity: the invalid
  case must fail, and the accepted cases must cross both formats.
- Obligation: `INV-policy-canonical`.
  Method: parameterized serialization test. Domain: every variant. Artefact:
  `src/cli/config/tests.rs`. Evidence: serialized values are the canonical
  lowercase spellings.
- Obligation: `INV-indexed`.
  Method: fixed integration test in the child merge process. Domain:
  `NETSUKE_DEFAULT_TARGETS__0=hello.txt` and the scalar form. Artefact:
  `tests/cli_tests/merge/env_value_forms/mod.rs`. Evidence: the indexed form
  fails with `invalid type: map`, the scalar form succeeds. Non-vacuity: the
  scalar control succeeds.

### Formal verification decision

No bounded model check or formal proof is warranted: the behaviour is finite
input normalization and deserialization delegation, and the existing
parameterized and property tests provide the appropriate rigour. Kani and Verus
are not used.

### Suggested BDD features

Not adopted for this change. The repository's CLI integration tests already
exercise the observable behaviour through the real merge boundary.

## Plan of work

### `EP-M1` Boolean environment values

In `src/cli/environment/mod.rs`, add a `pub(crate)` strict parser that accepts
exactly `true`, `false`, `1`, and `0`. Add a named constant listing the five
single-component Boolean paths. In `parse_entry`, after UTF-8 validation, emit
a Figment Boolean for an accepted spelling on those paths and a raw string
otherwise. In `src/cli/diag/mod.rs`, call the shared parser from
`json_from_env` and keep the existing error text. Characterize layer masking
first with direct `MergeComposer` tests in `tests/cli_tests/merge/mod.rs`, then
decide whether the explicit `--json` short circuit stays.

### `EP-M2` Case-insensitive policy deserialization

In `src/cli/config/policies.rs` (the enums moved there from `mod.rs` to keep
that file under the 400-line cap; see `D7`), replace the derived `Deserialize`
on `ColourPolicy`, `EmojiPolicy`, `ProgressPolicy`, and `AccessibilityPolicy`
with a manual implementation delegating to `FromStr`, retaining derived
`Serialize` and `rename_all = "kebab-case"`. Add a private generic helper that
deserializes a string and maps `FromStr` failure with
`serde::de::Error::custom`, including the canonical expected values in the
message. Extend `src/cli/config/tests.rs` with TOML and `serde_json` cases and
a serialization test.

### `EP-M3` End-to-end regression tests

Add `tests/cli_tests/merge/env_value_forms/mod.rs` with per-form submodules,
and register it in `tests/cli_tests/merge/mod.rs`. Cover Boolean spellings,
policy casing and precedence, file-layer strictness, and the indexed-list
rejection. Extend `tests/cli_tests/merge_diag.rs` and `merge_diag_proptests.rs`
with a coherent map-backed provider whose `get` and `entries` share a snapshot.
Add `tests/logging_stderr/json_env.rs` — registered in
`tests/logging_stderr_tests.rs` so the existing file stays under the size cap —
with `NETSUKE_JSON=1` and `NETSUKE_JSON=0` child runs.

### `EP-M4` Documentation and decision record

Create the next ADR under `docs/adrs/`, reference it from
`docs/netsuke-cli-design-document.md`, update "Configure Netsuke", "Policy
values and parsing", and "JSON output" in `docs/users-guide.md`, and add the
unreleased migration-guide entry. Run the documentation gates.

## Concrete steps

```plaintext
Working directory: repository root
Red:     cargo test -p netsuke cli::environment::tests  -> new cases fail
Green:   the same command -> passes
Gates:   make check-fmt; make lint; make doc-coverage; make test;
         make markdownlint; make test-documentation-contracts
```

Because a Rust build may wait for a shared compiler slot, allow generous
timeouts and do not retry a waiting build.

## Validation and acceptance

- `NETSUKE_JSON=1 netsuke generate` merges successfully and selects JSON mode.
- `NETSUKE_JSON=0` selects human-readable output.
- `NETSUKE_VERBOSE=1` merges successfully and sets verbose mode.
- `color = "AlWaYs"` in a TOML file merges to `ColourPolicy::Always`.
- `NETSUKE_COLOR=ALWAYS` merges to `ColourPolicy::Always`.
- `json = 1` in a TOML file is still rejected.
- `NETSUKE_DEFAULT_TARGETS=hello.txt` and
  `NETSUKE_DEFAULT_TARGETS='["hello.txt"]'` both work.
- `NETSUKE_DEFAULT_TARGETS__0=hello.txt` still fails with `invalid type: map`.
- The early resolver and the full merge agree for every tested
  `NETSUKE_JSON` value.

## Idempotence and recovery

All edits are additive or local replacements. Re-running a gate is safe. If a
gate fails, read its captured log and fix the cited file before re-running; do
not amend a commit while a gate batch is running.

## Artefacts and notes

Reconnaissance confirmed the current rejection text for `NETSUKE_JSON`, the
Figment value parsing behaviour, and the existing child-process merge adapter.
Implementation evidence will be recorded after each milestone.

The per-field Boolean rule is a workaround for an ortho-config capability gap.
The upstream request is
[ortho-config#714](https://github.com/leynos/ortho-config/issues/714) and
removing the workaround once it lands is tracked in
[#1165](https://github.com/leynos/netsuke/issues/1165); ADR-0083 records both.

## Interfaces and dependencies

- `pub(crate) fn parse_environment_bool(value: &str) -> Option<bool>` in
  `src/cli/environment/mod.rs`, re-used by `src/cli/diag/mod.rs`.
- A private `const BOOLEAN_PATHS: [&str; 5]` naming `verbose`, `json`,
  `fetch_default_deny`, `trust_project_fetch_policy`, and `no_input`.
- Manual `impl<'de> Deserialize<'de>` for each policy enum, delegating to the
  existing `FromStr`.
- No new dependencies.

## Revision note

- 2026-11-18: Initial plan drafted from issue #1147 and reconnaissance of the
  current branch.
- 2026-11-18: Recorded the rebase onto `origin/main` (`8daa9143`) and the
  post-review test refactor. The replay was conflict-free and every replayed
  patch is identical to its pre-rebase form.
- 2026-11-18: Recorded the CI `build-test` failure and the Kani scope fix, and
  the CodeRabbit finding about `NETSUKE_NO_INPUT=0` with the documentation
  change that answers it.
- 2026-11-18: Recorded the second CodeRabbit pass, the migration-guide and
  CLI-design fixes it asked for, and the gate batches that validated each delta.
- 2026-11-18: Recorded the third review pass and the child-environment
  isolation it asked for in the `NETSUKE_JSON` binary tests.
