# Netsuke roadmap

This roadmap tracks unfinished and future Netsuke work. Completed historical
foundations live in
[`docs/archive/roadmap-completed-foundations.md`](archive/roadmap-completed-foundations.md)
so the active roadmap can focus on remaining hypotheses without erasing prior
implementation detail.

Task identifiers are globally unique across the active roadmap and the archive.
When a completed task moves to the archive, it keeps its original number and is
not repeated here. When a historical task is renamed under the command-line
interface (CLI) redesign, the active task states the mapping explicitly.

## How to read this roadmap

Each phase validates a product hypothesis:

- Phase 3 validates that Netsuke can stay friendly for local human workflows
  while becoming predictable for automation and agents.
- Phase 4 validates that the build compiler and its cross-platform behaviour
  can be specified and checked rigorously.
- Phase 5 validates that repeated human, Continuous Integration (CI), editor,
  and agent usage improves through introspection, profiles, run history,
  delivery, and feedback.
- Phase 6 validates that absorbing common manifest operations into the
  template standard library makes declarative build manifests markedly easier
  to write without weakening determinism or the capability boundary.

Each phase carries one hypothesis, and Phase 6 is the capability track for
template standard-library work. Phases 3 to 5 predate that separation: each
mixes capability delivery with verification and consistency work under a
single hypothesis, and they are not being re-partitioned. New template
standard-library work belongs in Phase 6 rather than being appended to
whichever phase happens to be open.

The roadmap keeps user-facing product grammar separate from implementation
detail. Public tasks name Netsuke capabilities first. Implementation adapters,
including OrthoConfig, appear only when they define ownership, dependency, or
validation boundaries.

## External dependencies

Netsuke depends on OrthoConfig for generic command/configuration/schema
machinery. Netsuke tasks cover integration, product policy, validation, and
local build-tool adaptation. They must not duplicate the shared infrastructure
owned by OrthoConfig.

Relevant OrthoConfig roadmap dependencies:

- OrthoConfig `5.2.3`: consumer dependency boundaries for Netsuke and Weaver.
- OrthoConfig `6.1.1` and `6.1.2`: recursive command metadata extraction.
- OrthoConfig `6.2.1` to `6.2.3`: `<tool> context --json` schema, emission,
  and downstream command naming.
- OrthoConfig `6.3.1` and `6.3.2`: skill manifest metadata and validation.
- OrthoConfig `7.1.1` to `7.1.3`: vocabulary policy and canonical global
  option glossary.
- OrthoConfig `7.2.1` to `7.2.7`: non-interactive metadata, mutation metadata,
  dual-renderer output metadata, structured result classes, stream contracts,
  bounded-list metadata, and capability provenance.
- OrthoConfig `7.3.1`: shared exit-code and error-remediation metadata.
- OrthoConfig `8.1.1` and `8.1.2`: reference CLI structured result and
  enumerable-error behaviour.
- OrthoConfig `9.1.1` to `9.1.3`: profile metadata, redaction, and profile
  store helpers.
- OrthoConfig `9.2.1` and `9.2.2`: delivery-target parsing and feedback
  storage helpers.
- OrthoConfig `9.3.1` to `9.3.3`: execution-ledger metadata, run-ledger nouns,
  and run-ledger helper APIs.

## Canonical public vocabulary

These command and flag spellings are the public grammar assumed by this
roadmap. Examples must use this list unless a task explicitly extends it.

- Top-level commands: `build`, `check`, `clean`, `generate`, `graph`,
  `context`, `skill-path`, `runs`, `profile`, and `feedback`.
- Resource verbs: `list`, `get`, `save`, `delete`, `add`, `send`, and `prune`.
- Structured output: `--json`.
- Non-interactive execution: `--no-input`.
- Destructive confirmation: `--force`.
- Mutation preview: `--dry-run`.
- Pagination: `--limit` and `--cursor`.
- Output and delivery: `--output` and `--deliver`.
- Display policy flags: `--color auto|always|never`,
  `--emoji auto|always|never`, `--progress auto|always|never`, and
  `--accessibility auto|on|off`.

## Historical task traceability

The following previous roadmap tasks were assessed during the CLI-roadmap
rewrite:

- `1.1.1` to `1.3.3`, `2.1.1` to `2.3.2`, and completed `3.x` foundation
  tasks through `3.13.2` moved to the archive as completed foundations.
- `3.4.5`, `3.4.6`, `3.8.3`, `3.11.4`, `3.12.3`, `3.13.3`, `3.14.1`, and
  `3.14.3` to `3.14.11` remain active under their existing numbers.
- `3.14.2` was completed after restoring coverage for top-level action
  expansion and complementary `command_available(...)` branches.
- `3.14.4` is complete; `command_available(...)` now has the documented
  non-throwing executable-probe contract and typed resolver boundary.
- Issue #83 is complete; every non-empty UTF-8 `NETSUKE_NINJA` value is selected
  even if spawning later fails, while unset, empty, and non-UTF-8 values fall
  back to `ninja` with documented verbose and structured tracing.
- Phase 4 remains active because none of its formal-verification work has been
  delivered yet.
- New CLI-redesign work starts at `3.15` and Phase 5 so historical numbers are
  not reused.
- Phase 6 is new and introduces the template standard-library expansion
  specified in
  [RFC 0006](rfcs/0006-ansible-inspired-template-standard-library.md). It
  reuses no historical numbers.

## 3. Friendly polish and agent-consistent CLI foundations

Hypothesis: Netsuke can keep a pleasant, accessible local command-line
experience while making every command predictable for CI, editor integrations,
and agents.

### 3.4. Graph and explanation work

- [x] 3.4.5. Extend the graph subcommand with an optional `--html` renderer.
  - [x] Keep raw graph data available for automation.
  - [x] Add `--output <FILE>` for file-based graph artefacts.
  - [x] Document how `graph --html --output graph.html` differs from
    structured graph inspection.

- [ ] 3.4.6. Evaluate whether `netsuke explain <code>` should exist.
  - [ ] Compare `explain` with richer diagnostics and documentation links.
  - [ ] Avoid adding the command unless it has a clear user-facing workflow.
  - [ ] If accepted, add `explain` to the canonical vocabulary before examples
    use it.

### 3.8. Accessibility verification

- [ ] 3.8.3. Verify accessible output with assistive technology.
  - [ ] Test screen-reader behaviour for diagnostics, progress, and summaries.
  - [ ] Validate reduced-motion and no-colour modes.
  - [ ] Record findings in the accessibility documentation.

### 3.11. Configuration precedence verification

- [ ] 3.11.4. Add OrthoConfig precedence-ladder regression tests.
  - [x] Explicit config-path selector precedence (`--config` >
    `NETSUKE_CONFIG`) verified by exhaustive rstest cases and a proptest
    property test (PR `#327`, closes `#291`).
  - [ ] Depend on OrthoConfig `5.2.3` for consumer boundary guidance.
    OrthoConfig `5.2.3` is an upstream OrthoConfig roadmap identifier, not a
    crate version; Netsuke stays pinned at `ortho_config = "0.9.0"` until
    that guidance ships. Blocked on upstream; no `Cargo.toml` change.
  - [x] Preserve Netsuke-specific precedence expectations for manifest path,
    display policies, and locale across the two-selector ladder
    (`--config` > `NETSUKE_CONFIG` > automatic discovery), where the
    discovered rung is a single exclusive winner among system scope, user
    scope, and defaults. When a user-scope configuration wins over the system
    scope, system-only fields are not merged through and fall back to their
    defaults (regression tests added in issue `#385`).
  - [ ] Preserve Netsuke-specific precedence expectations for profile
    selection (deferred to 5.3.1, when the `--profile` flag lands).
  - [x] Verify that CLI flags override environment, project, user, system,
    and default configuration layers for scalar fields (manifest path,
    display policies, locale, jobs) in issue `#385`.
  - [ ] Verify that CLI flags override the profile configuration layer
    (deferred to 5.3.1, when the `--profile` flag lands).

  - Note: OrthoConfig automatic discovery is exclusive: one discovered file
    wins, so system-only fields are absent when a user-scope file is
    selected (user-over-system and system-only merge-through cases are
    covered by the regression tests added in issue #385).

### 3.12. Terminal rendering verification

- [ ] 3.12.3. Add terminal rendering regression tests.
  - [ ] Verify `--color auto|always|never` policy behaviour.
  - [ ] Verify `--emoji auto|always|never` policy behaviour.
  - [ ] Verify `--progress auto|always|never` policy behaviour.
  - [ ] Verify `--accessibility auto|on|off` behaviour.

### 3.13. CI guidance

- [ ] 3.13.3. Revise CI-focused guidance for the canonical CLI.
  - [ ] Replace legacy diagnostics-only examples with `--json --no-input`.
  - [ ] Include `check --json --no-input` and `build --json --no-input`.
  - [ ] Keep examples friendly for humans who maintain CI scripts.

### 3.14. Conditional action planning

- [x] 3.14.1. Record manifest-time condition semantics for actions and targets.
  See [netsuke-design.md §2.5](netsuke-design.md).
  - [x] State that `foreach` and `when` are evaluated before typed Abstract
    Syntax Tree (AST) deserialization, intermediate representation (IR)
    generation, and Ninja execution.
  - [x] Document that build-time branching belongs in recipes unless a future
    runtime-condition feature is designed.
- [x] 3.14.2. Apply `foreach` and `when` expansion to top-level `actions`.
  Depends on archived task `2.2.3`. See
  [netsuke-design.md §2.5](netsuke-design.md).
  - [x] Preserve the existing implicit `phony: true` action behaviour after
    expansion.
  - [x] Support complementary branches such as `when: command_available(...)`
    and `when: not command_available(...)`.
- [x] 3.14.3. Lower target and action `deps` into implicit IR and Ninja
  dependency edges. Requires archived tasks `1.2.2` and `1.3.2`. See
  [netsuke-design.md §§2.4 and 5.3](netsuke-design.md).
  - [x] Keep `sources` in the explicit recipe-input class used for `ins` and
    `$in`.
  - [x] Add a separate implicit dependency class for `deps` so they affect
    ordering and rebuild decisions without appearing in recipe arguments.
  - [x] Align cycle detection, generated Ninja output, and user-facing
    dependency documentation.
  - [x] Add `dependency_order: serial` for direct action and target `deps`.
    Staged Ninja dyndep lowering preserves declaration order, failure
    short-circuiting, shared-work reuse, and unrelated-branch concurrency;
    [ADR-011](adr-011-use-ninja-dyndep-for-serial-dependency-ordering.md)
    records the path-scoped guarantee and generated-state contract.
- [x] 3.14.4. Add `command_available(name, **kwargs)` as a non-throwing
  executable probe. Depends on archived task `3.5.1`. See
  [executable discovery](netsuke-design.md#executable-discovery-filter-which).
  - [x] Reuse the `which` resolver and cache.
  - [x] Return `false` for absent commands instead of raising
    `netsuke::jinja::which::not_found`.
  - [x] Preserve argument validation diagnostics for invalid options.
- [ ] 3.14.5. Add regression coverage for conditional action dependency
  manifests.
  - [ ] Test action-level `when` and action-level `foreach`.
  - [ ] Test complementary nextest and legacy branches select exactly one
    action.
  - [ ] Test absent-command fallback without invoking `shell()`.
  - [ ] Test `deps` lowering in the IR and emitted Ninja build statements.
  - Note: the manifest-expansion building blocks already exist in
    `src/manifest/expand_test_cases/action_condition_cases.rs` and the BDD
    scenarios in `tests/bdd/steps/conditional_manifest.rs`. The outstanding gap
    is end-to-end coverage that traces a conditionally selected action through
    `deps` lowering into emitted Ninja, plus the named nextest-versus-legacy
    scenario (existing tests use a generic `preferred-tool`).
- [ ] 3.14.6. Add rule-level `deps_from` for compiler dependency imports.
  Requires 3.14.3. See
  [netsuke-design.md §2.3](netsuke-design.md#planned-compiler-dependency-import).
  - [ ] Parse `deps_from.format` and `deps_from.depfile` without accepting
    rule-level `deps` as an alias.
  - [ ] Validate the initial `gcc` and `msvc` dependency formats.
  - [ ] Lower `deps_from` into the IR action `depfile` and Ninja `deps`
    attributes.
  - [ ] Add parser, IR, Ninja output, and user-guide coverage once the feature
    is implemented.
  - Note: the IR sink already exists but is unwired. `Action.depfile` and
    `Action.deps_format` are defined in `src/ir/graph.rs` and emitted by
    `src/ninja_gen.rs`; only manifest parsing and population from `deps_from`
    remain.
- [ ] 3.14.7. Escape backend dollar syntax after Netsuke placeholder lowering.
  Depends on archived task `1.3.2`. See
  [netsuke-design.md §§2.6 and 5.4](netsuke-design.md).
  - [ ] Preserve shell variables such as `$PATH`, `${CARGO:-cargo}`, and
    `$RUSTFLAGS` in generated Ninja by emitting literal dollars as `$$`.
  - [ ] Keep the IR free of Ninja-specific dollar escaping.
  - [ ] Add command and script regression tests covering shell variables,
    `$in` / `$out`, and unrelated identifiers such as `$input`.
- [ ] 3.14.8. Make Jinja command helpers match the documented ergonomics.
  Depends on archived task `2.2.4` and 3.14.4. See
  [netsuke-design.md §§4.4 and 4.5](netsuke-design.md).
  - [ ] Add `env(name, default=...)` without changing the existing missing and
    invalid UTF-8 diagnostics.
  - [ ] Implement or remove the documented `shell_escape` helper so the user
    guide and code agree.
  - [ ] Add `shell_join` and `compact` helpers for deliberate shell recipes.
  - [ ] Add documentation and tests showing optional `RUSTFLAGS` construction
    without shell parameter expansion.
  - Note: `env(name)` already exists in `src/manifest/mod.rs` but without the
    `default=` kwarg (it raises on missing and non-UTF-8 values). The
    `shell_escape`, `shell_join`, and `compact` helpers are not yet
    implemented.
- [ ] 3.14.9. Add structured recipe environment mappings.
  Requires 3.14.7 and 3.14.8. See
  [netsuke-design.md §2.6](netsuke-design.md#26-planned-recipe-ergonomics-and-execution-feedback).
  - [ ] Parse rule, target, and action `env` mappings with `value`, `default`,
    `prepend`, `append`, and `unset` operations.
  - [ ] Merge rule-level and target/action-level environment bindings during
    IR generation.
  - [ ] Emit backend-specific environment setup without exposing Ninja variable
    syntax in the manifest contract.
  - [ ] Test platform path-list separators for `prepend` and `append`.
- [ ] 3.14.10. Add structured `exec` recipes for argv-safe commands.
  Requires 3.14.8 and 3.14.9. See
  [netsuke-design.md §2.6](netsuke-design.md#26-planned-recipe-ergonomics-and-execution-feedback).
  - [ ] Extend the recipe union with `exec.program` and `exec.args`.
  - [ ] Reject manifests that combine `exec` with `rule`, `command`, or
    `script`.
  - [ ] Preserve list-valued argument expressions without accidental shell word
    splitting.
  - [ ] Add Ninja output and execution tests for arguments containing spaces,
    shell metacharacters, and empty optional values.
- [ ] 3.14.11. Surface selected conditional actions without recipe `echo`.
  Requires 3.14.2 and 3.14.4. See
  [netsuke-design.md §2.6](netsuke-design.md#26-planned-recipe-ergonomics-and-execution-feedback).
  - [x] Add target/action `description` as discovery metadata and list it with
    `netsuke help targets`.
  - [x] Keep target/action descriptions as discovery metadata; they do not
    override the referenced rule description for Ninja progress.
  - [x] Keep normal Ninja progress sourced from the referenced rule description;
    do not report target/action descriptions there.
  - [ ] In verbose mode, report why manifest-time `when` branches were included
    or skipped.
  - [ ] Do not add generic `debug`, `info`, or `warn` manifest keys unless a
    later diagnostics design defines severity semantics.
  - Note: target/action `description` is discovery metadata rendered by
    `netsuke help targets`; Ninja progress remains sourced from the referenced
    rule's `description`. Target/action environment mappings remain future work
    under 3.14.9.

### 3.15. Canonical CLI redesign

- [ ] 3.15.1. Replace the pre-0.1.0 command surface with canonical names.
  - [x] Rename `manifest` to `generate`.
  - [x] Remove `build --emit`; use `generate --output`.
  - [ ] Add `check`, `context`, `skill-path`, `runs`, `profile`, and
    `feedback`.
  - [ ] Rename `--file` to `--manifest`, keeping `-f` as an intentional
    shorthand.
  - [ ] Depend on OrthoConfig `7.1.1` to `7.1.3` for shared vocabulary policy
    and global option glossary.

- [ ] 3.15.2. Add non-interactive and mutation-safety guarantees.
  - [x] Add root `--no-input`.
  - [x] Make prompts impossible unless a future explicit interactive mode is
    added.
  - [ ] Require `--force` for destructive operations.
  - [ ] Require or support `--dry-run` for consequential operations.
  - [ ] Make bare `clean` fail fast with a corrective hint.
  - [ ] Depend on OrthoConfig `7.2.1`, `7.2.2`, and `8.1.1` for shared
    non-interactive and mutation metadata.

- [ ] 3.15.3. Replace diagnostics-only JSON with canonical structured output.
  - [x] Remove `--diag-json` and `--output-format`.
  - [x] Add root `--json`.
  - [x] Emit exactly one JSON result document on successful JSON-mode commands.
  - [x] Emit exactly one JSON diagnostic document on failing JSON-mode commands.
  - [x] Suppress progress, colour, emoji, tracing, and timing text in JSON mode.
  - [ ] Snapshot every v1 JSON schema.
  - [ ] Depend on OrthoConfig `7.2.3` to `7.2.5`, `7.3.1`, `8.1.1`, and
    `8.1.2` for shared result, stream, exit-code, and enumerable-error
    metadata.

- [ ] 3.15.4. Replace legacy output preferences with canonical policy flags.
  - [x] Replace `--colour-policy` with `--color auto|always|never`.
  - [x] Replace `--spinner-mode` and boolean `--progress` with
    `--progress auto|always|never`.
  - [x] Replace `--no-emoji` with `--emoji auto|always|never`.
  - [x] Add `--accessibility auto|on|off`.
  - [x] Update OrthoConfig field integration, environment names, config
    examples, localization keys, and tests.
  - [ ] Depend on OrthoConfig `7.1.2`, `7.1.3`, and `7.2.3` for shared flag
    vocabulary and dual-renderer metadata.

- [ ] 3.15.5. Add stable exit codes and enumerable errors.
  - [ ] Define the Netsuke exit-code taxonomy in the design docs.
  - [ ] Ensure every enum-like failure lists valid values.
  - [ ] Add tests for CLI enums, config enums, manifest enums, stdlib options,
    delivery schemes, profile names, and run states.
  - [ ] Depend on OrthoConfig `7.3.1` and `8.1.2` for shared exit-code and
    enumerable-error metadata.

- [ ] 3.15.6. Bound every large response.
  - [ ] Add `--limit` and `--cursor` where lists can grow.
  - [ ] Add `--target` and `--depth` to graph inspection.
  - [ ] Add truncation hints to JSON and human output.
  - [ ] Bound build-log previews in JSON mode and reference log files.
  - [ ] Depend on OrthoConfig `7.2.6` for bounded-list metadata.

- [ ] 3.15.7. Add CLI vocabulary linting.
  - [ ] Generate a command inventory from the real command surface.
  - [ ] Fail CI on banned verbs and flags.
  - [ ] Snapshot the canonical command surface.
  - [ ] Keep the lint aligned with OrthoConfig `7.1.1` to `7.1.3`.

## 4. Formal verification and property testing

Hypothesis: Netsuke can state and check its core compiler invariants strongly
enough that future features do not erode deterministic build behaviour. This
phase preserves the detailed formal-verification workload from the previous
roadmap rather than compressing it into broad strategy items.

Objective: To add bounded formal verification and generated testing where the
repository's semantic risk is highest, while keeping the existing build, lint,
and test workflow intact. See
[formal-verification-methods-in-netsuke.md](formal-verification-methods-in-netsuke.md).

### 4.1. Verification tooling and gating

- [x] 4.1.1. Add Kani tooling and local smoke targets. See
  [formal-verification-methods-in-netsuke.md §Repository integration plan](formal-verification-methods-in-netsuke.md#repository-integration-plan).
  - [x] Pin the supported Kani version under `tools/kani/`.
  - [x] Add `rust-prover-tools` backed Kani installation.
  - [x] Add `make kani-check`, `make kani-full`, and `make formal-pr`.
- [x] 4.1.2. Add a dedicated `kani-smoke` continuous integration (CI) job.
  Requires 4.1.1. See
  [formal-verification-methods-in-netsuke.md §Continuous integration (CI)](formal-verification-methods-in-netsuke.md#continuous-integration-ci).
  - [x] Keep the existing `build-test` job unchanged.
  - [x] Run the bounded smoke path on pull requests. The `kani-smoke` job in
    `.github/workflows/ci.yml` runs `make install-kani`, `make kani-check` (a
    Kani version check), and `make kani-ir` (the bounded harnesses landed by
    `4.2.*`). The harness wiring resolved the follow-up previously tracked as
    [issue #445](https://github.com/leynos/netsuke/issues/445).
  - [x] Cache Kani tool downloads separately from ordinary Cargo artefacts.
- [x] 4.1.3. Record the phase-1 scope boundary for Verus and Stateright. See
  [formal-verification-methods-in-netsuke.md §Optional Verus proof kernel](formal-verification-methods-in-netsuke.md#optional-verus-proof-kernel)
  and
  [formal-verification-methods-in-netsuke.md §Stateright remains deferred](formal-verification-methods-in-netsuke.md#stateright-remains-deferred).
  - [x] Document Verus as optional and proof-kernel-only.
  - [x] Document Stateright as deferred until Netsuke gains a stateful
    concurrent subsystem.

### 4.2. Intermediate representation verification

- [x] 4.2.1. Add Kani harnesses for manifest-to-IR safety checks. Requires
  4.1.1. See
  [formal-verification-methods-in-netsuke.md §Kani for the IR core](formal-verification-methods-in-netsuke.md#kani-for-the-ir-core).
  See also
  [execplan 4.2.1](execplans/4-2-1-kani-harnesses-for-manifest-to-ir-safety-checks.md)
  and [ADR-004](adr-004-bound-kani-ir-harnesses-to-small-n.md).
  - [x] Prove duplicate-output rejection on bounded manifests. Kani covers the
    accepted small-N proof boundary; 4.3.1 closes the larger-N Proptest
    coverage.
  - [x] Prove empty-rule, multiple-rule, and missing-rule error selection.
  - [x] Prove self-edge and small bounded multi-node cycle rejection. Kani
    covers the accepted small-N proof boundary; 4.3.1 closes the larger-N
    Proptest coverage.
  - [x] Prove missing dependencies do not create false cycles.
  - [x] Record the implementation decision to keep the public `netsuke::ir`
    API unchanged and place Kani-only verification support behind `cfg(kani)`.
  - [x] Record the implementation decision to use a private `IrHashMap`
    compatibility layer for proof builds rather than a public verification
    collection port.
  - [x] Verify the final harness inventory: nine IR harnesses covering
    duplicate-output discovery, rule-shape error selection, self-cycle and
    two-node-cycle detection, and missing-dependency false-cycle rejection.
  - [x] Validate the final branch with `make check-fmt`, `make lint`,
    `make test`, `make markdownlint`, `make nixie`, and `make kani-ir`.
    `make kani-ir` reported nine successfully verified harnesses and zero
    failures.
  - [x] Record the review observation that repeated
    `coderabbit review --agent` attempts reached `preparing_sandbox` and
    emitted no findings or rate-limit notice.
- [x] 4.2.2. Add Kani harnesses for cycle canonicalization. Requires 4.2.1.
  See
  [formal-verification-methods-in-netsuke.md §Optional Verus proof kernel](formal-verification-methods-in-netsuke.md#optional-verus-proof-kernel).
  Kani proves the private production `canonicalize_cycle_by` kernel over
  distinct `u8` cycles for N=2, N=3, and N=4. A direct adapter harness checks
  the `Utf8PathBuf` wrapper connection for two-node path cycles, and the
  existing Proptest suite continues to cover larger path-bearing cycles. Direct
  `Utf8PathBuf` property harnesses for N=2 through N=4 were measured and
  rejected under the 8 GiB cap.
  - [x] Prove preserved length and closed-cycle output.
  - [x] Prove the interior node multiset is preserved.
  - [x] Prove the selected start node is stable under the current ordering
    rule.
  - [x] Record the implementation decision to prove a private production-owned
    `canonicalize_cycle_by` kernel rather than a direct path-bearing Kani proof
    or a duplicated Kani-only model.
  - [x] Validate the final branch with `make check-fmt`, `make lint`,
    `make test`, `make markdownlint`, `make nixie`, and capped `make kani-ir`.
    `make kani-ir` reported thirteen successfully verified harnesses and zero
    failures.
  - [x] Record the mutation and review evidence: the three canonicalization
    mutation patches fail their matching harnesses, and
    `coderabbit review --agent` returned zero findings.
- [ ] 4.2.3. Add Kani harnesses for command interpolation. Requires 4.1.1. See
  [formal-verification-methods-in-netsuke.md §Kani for command interpolation](formal-verification-methods-in-netsuke.md#kani-for-command-interpolation).
  - [ ] Prove `$in` and `$out` rewrite only at valid token boundaries (bounded
    to 256-character commands with at most 8 placeholders).
  - [ ] Prove backtick-delimited regions are preserved.
  - [ ] Prove unmatched backticks are rejected.
  - [ ] Prove successful results satisfy the current `shlex` guard.

### 4.3. Determinism and manifest property testing

- [ ] 4.3.1. Add Proptest coverage for deterministic Ninja emission. Requires
  4.1.1. See the
  [Proptest section](formal-verification-methods-in-netsuke.md#proptest-for-determinism-and-manifest-semantics).
  - [ ] Prove Ninja output is stable across equivalent insertion orders
    (generated graphs bounded to 50 actions and 100 edges).
  - [ ] Prove `default` target ordering is stable.
  - [ ] Prove `path_key` is invariant for equivalent output sets.
- [ ] 4.3.2. Add Proptest coverage for manifest expansion invariants. Requires
  4.1.1. See the
  [Proptest section](formal-verification-methods-in-netsuke.md#proptest-for-determinism-and-manifest-semantics).
  - [ ] Prove `foreach` preserves non-control fields.
  - [ ] Prove `when` is removed after evaluation.
  - [ ] Prove `item` and `index` are injected correctly for each expansion.
  - [ ] Prove static targets still honour `when`.
- [ ] 4.3.3. Add Proptest coverage for render stability. Requires 4.3.2. See
  the
  [Proptest section](formal-verification-methods-in-netsuke.md#proptest-for-determinism-and-manifest-semantics).
  - [ ] Prove rendering is idempotent after Jinja syntax is exhausted.
  - [ ] Prove variable rendering uses the intended snapshot semantics.

### 4.4. Contract documentation and optional proof kernels

- [ ] 4.4.1. Document the command placeholder contract in the README. Requires
  4.2.3. See
  [formal-verification-methods-in-netsuke.md §Command placeholder contract](formal-verification-methods-in-netsuke.md#command-placeholder-contract).
  - [ ] Add a "Security and command interpolation" section to the README.
  - [ ] State the supported placeholders explicitly.
  - [ ] State the current backtick-handling boundary explicitly.
  - [ ] State whether `shlex::split` is part of the semantic acceptance
    contract.
- [ ] 4.4.2. Document which dependency kinds participate in cycle detection in
  the user guide. Requires 4.2.1. See
  [formal-verification-methods-in-netsuke.md §Cycle-participation contract](formal-verification-methods-in-netsuke.md#cycle-participation-contract).
  - [ ] Decide whether order-only dependencies participate.
  - [ ] Decide whether implicit outputs participate.
  - [ ] Document the chosen rule in the user guide's dependency and build-graph
    semantics chapter.
  - [ ] Align implementation, tests, and documentation with the chosen rule.
- [ ] 4.4.3. Evaluate a minimal Verus proof kernel for cycle canonicalization.
  Requires 4.2.2 and 4.1.3. See
  [formal-verification-methods-in-netsuke.md §Optional Verus proof kernel](formal-verification-methods-in-netsuke.md#optional-verus-proof-kernel).
  - [ ] Keep the proof outside Cargo.
  - [ ] Use proof-specific model types rather than production `HashMap`
    structures.
  - [ ] Accept the proof only if it remains narrower and cheaper than the Kani
    equivalent.

**Success criterion:** Netsuke ships bounded Kani smoke checks for the IR core,
generated property tests for deterministic emission and manifest semantics, and
documented verification contracts that keep optional Verus work narrow and
defer Stateright until the architecture justifies model checking.

## 5. Agent-consistent compounding features

Hypothesis: Netsuke becomes more valuable across repeated invocations when
humans, CI systems, editors, and agents can discover its surface, reuse local
configuration, inspect run history, route artefacts, and report friction.

### 5.1. Context and schema generation

- [ ] 5.1.1. Implement `netsuke context --json`.
  - [ ] Emit compact versioned JSON by default.
  - [ ] Include commands, flags, enums, exit codes, result schemas,
    diagnostics schema, config schema, manifest schema, and stdlib metadata.
  - [ ] Add `--detail` for expanded descriptions.
  - [ ] Depend on OrthoConfig `5.2.3`, `6.1.1`, `6.1.2`, `6.2.1`,
    `6.2.2`, `6.2.3`, and `7.2.7`.

- [ ] 5.1.2. Add Netsuke-specific manifest and build-plan context.
  - [ ] Include bounded target, default-target, graph, and stdlib previews.
  - [ ] Include truncation hints for omitted manifest-derived detail.
  - [ ] Keep implementation-adapter names out of public command examples.

- [ ] 5.1.3. Implement `netsuke skill-path`.
  - [ ] Add `docs/skills/netsuke/SKILL.md`.
  - [ ] Validate the skill manifest against `netsuke context --json`.
  - [ ] Depend on OrthoConfig `6.3.1` and `6.3.2`.

- [ ] 5.1.4. Add schema and description-budget validation.
  - [ ] Snapshot compact and detailed context output.
  - [ ] Enforce description-size budgets in CI.
  - [ ] Fail validation when the command surface and context drift.

### 5.2. Run ledger

- [ ] 5.2.1. Define the Netsuke run record model.
  - [ ] Record run ID, command, targets, manifest fingerprint, status,
    exit code, timings, artefacts, and log paths.
  - [ ] Keep `runs` as the public noun to avoid collision with build-job
    parallelism.
  - [ ] Depend on OrthoConfig `9.3.1` and `9.3.2`.

- [ ] 5.2.2. Persist Netsuke run records.
  - [ ] Store project-local records under `.netsuke/runs/`.
  - [ ] Recover cleanly from interrupted runs.
  - [ ] Treat run persistence as product state, not generic configuration.
  - [ ] Depend on OrthoConfig `9.3.3` where its helper APIs are available.

- [ ] 5.2.3. Implement `runs list`, `runs get`, and `runs prune`. Requires:
  5.2.1, 5.2.2.
  - [ ] Support `--json` on all run commands.
  - [ ] Bound list output with `--limit` and `--cursor`.
  - [ ] Require `--force` for pruning.
  - [ ] Include recovery hints for interrupted builds.

- [ ] 5.2.4. Add run-ledger validation and documentation. Requires: 5.2.3.
  - [ ] Test interrupted writes and corrupted record recovery.
  - [ ] Test human and JSON rendering.
  - [ ] Document run history for local users, CI, and agents.

### 5.3. Profiles

- [ ] 5.3.1. Integrate named profiles with Netsuke configuration.
  - [ ] Add root `--profile <name>`.
  - [ ] Apply precedence:
    defaults < system config < user config < project config < profile <
    environment < CLI.
  - [ ] Surface available profiles in `context --json`.
  - [ ] Depend on OrthoConfig `9.1.1`.

- [ ] 5.3.2. Define profile redaction and secret handling.
  - [ ] Avoid storing secrets by default.
  - [ ] Redact sensitive values from human output and `context --json`.
  - [ ] Depend on OrthoConfig `9.1.2`.

- [ ] 5.3.3. Implement profile commands.
  - [ ] Add `profile save`, `profile list`, `profile get`, and
    `profile delete`.
  - [ ] Require `--force` for destructive profile deletion.
  - [ ] Depend on OrthoConfig `9.1.3`; if unavailable, implement only the
    Netsuke-local adapter and mark the helper dependency as outstanding.

- [ ] 5.3.4. Add profile validation and documentation.
  - [ ] Test every precedence boundary.
  - [ ] Test missing, invalid, and redacted profile values.
  - [ ] Document local and CI profile workflows.

### 5.4. Delivery and feedback

- [ ] 5.4.1. Add structured delivery for Netsuke-owned artefacts.
  - [ ] Support `--deliver=stdout`, `--deliver=file:<path>`, and
    `--deliver=webhook:<url>` where applicable.
  - [ ] Write file deliveries atomically.
  - [ ] Surface webhook HTTP status in JSON results.
  - [ ] Require explicit authenticated endpoint configuration for
    `deliver:webhook`, including supported authentication schemes and required
    configuration fields.
  - [ ] Bound webhook timeouts and retry behaviour with documented maximum
    retry counts, backoff strategy, and backoff limits.
  - [ ] Enforce strict TLS and certificate authority validation by default,
    document any override options, and specify certificate pinning behaviour.
  - [ ] Redact webhook secrets from logs and JSON diagnostics, including
    headers, tokens, credentials, and query parameters.
  - [ ] Link implementation acceptance to
    [`security-network-command-audit.md`](security-network-command-audit.md)
    so `deliver:webhook` code paths cannot ship before meeting these
    requirements.
  - [ ] Depend on OrthoConfig `9.2.1` for generic delivery-target parsing.

- [ ] 5.4.2. Keep delivery scoped to product-owned artefacts.
  - [ ] Support generated manifests, graph output, reports, and JSON result
    envelopes.
  - [ ] Do not promise arbitrary build-output delivery until manifest artefact
    ownership is modelled.
  - [ ] Enumerate valid delivery schemes on error.

- [ ] 5.4.3. Implement local-first feedback.
  - [ ] Add `feedback add`, `feedback list`, and `feedback send`.
  - [ ] Store feedback as JSON Lines locally by default.
  - [ ] Require explicit upstream configuration and `feedback send --force`
    for network submission.
  - [ ] Depend on OrthoConfig `9.2.2` for generic feedback storage helpers.

- [ ] 5.4.4. Add delivery and feedback validation.
  - [ ] Test atomic file writes, webhook status reporting, and invalid schemes.
  - [ ] Test local feedback storage and upstream-disabled behaviour.
  - [ ] Surface delivery and feedback capabilities in `context --json`.

### 5.5. Agent-facing validation and documentation

- [ ] 5.5.1. Integrate the CLI vocabulary lint.
  - [ ] Fail CI on banned verbs and flags.
  - [ ] Check examples in docs as well as the command inventory.
  - [ ] Depend on OrthoConfig `7.1.1` to `7.1.3`.

- [ ] 5.5.2. Add non-interactive and stream-purity tests.
  - [ ] Verify commands do not wait for stdin.
  - [ ] Verify successful JSON mode writes exactly one stdout document and
    empty stderr.
  - [ ] Verify failing JSON mode writes empty stdout and exactly one stderr
    diagnostic document.
  - [ ] Depend on OrthoConfig `7.2.1`, `7.2.5`, and `8.1.1`.

- [ ] 5.5.3. Add error-remediation and exit-code tests.
  - [ ] Verify enum-like failures enumerate valid values.
  - [ ] Verify stable exit classes for usage, manifest, not-found, external
    tool, delivery, and interruption failures.
  - [ ] Depend on OrthoConfig `7.3.1` and `8.1.2`.

- [ ] 5.5.4. Update user and contributor documentation.
  - [ ] Add automation examples that use only canonical vocabulary.
  - [ ] Keep human-first local examples beside automation examples.
  - [ ] Cross-link the archive so reviewers can trace where historical work
    moved.

## 6. Template standard-library expansion

Hypothesis: Netsuke manifests currently fall back to `shell()` whenever they
need structured data, mapping composition, pattern matching, or version
comparison; if the template standard library absorbs those operations as pure,
bounded, capability-classified helpers, manifests become markedly easier to
write, the generated graph stays byte-for-byte reproducible, and
`netsuke help targets` can answer more questions without executing anything.

Kind: capability. Value is measured by the manifest operations that no longer
require a subprocess, not by name parity with Ansible.

Scope: the accepted set in
[RFC 0006](rfcs/0006-ansible-inspired-template-standard-library.md). Every
task in this phase targets a release after v0.1.0 final and must not widen the
hardening release defined by issue `#594`. Candidates that RFC 0006 §10
rejects must not be reintroduced by a later task, and candidates it defers are
gated behind step 6.10.

### 6.1. Settle the standard-library contract before the volume arrives

This step answers whether one shared contract, covering canonical equality,
checked bounds, typed localized diagnostics, and an enumerated manifest-query
boundary, can carry every later helper, or whether each capability group needs
its own. Its outcome decides whether steps 6.2 to 6.9 can be reviewed as
ordinary additions or need individual design passes. See RFC 0006 §§6 and
14.1.

- [ ] 6.1.1. Split the RFC 0006 accepted set into focused child issues.
  - See RFC 0006 §14.
  - Give each issue the full cross-cutting contract from RFC 0006 §6 rather
    than a reference to Ansible.
  - Record a release target of v0.1.x or later for each issue.
  - Success: every accepted capability in RFC 0006 §7 is covered by exactly
    one open child issue, and every deferred or rejected candidate is covered
    by none.
- [ ] 6.1.2. Implement the canonical value key and equality relation.
  - See RFC 0006 §6.7.
  - Derive the key with the existing `serde_json_canonicalizer` dependency and
    reject value kinds that have no canonical form.
  - Provide an order-preserving deduplication helper keyed on it.
  - Success: property tests show the relation is reflexive, symmetric, and
    transitive; that deduplication preserves first appearance; and that no
    public helper exposes a hash-map iteration order.
- [ ] 6.1.3. Implement the shared bounded-materialization helper. Requires
  6.1.2.
  - See RFC 0006 §6.8 and table 3.
  - Cover input length, nesting depth, checked combinatorial cardinality, and
    match counts.
  - Success: cardinality is rejected before allocation, and the diagnostic
    names both the computed cardinality and the ceiling.
- [ ] 6.1.4. Establish the stdlib domain-error and diagnostic-code scaffolding.
  - See RFC 0006 §6.9 and the stdlib resolver-boundary conventions in
    [developers-guide.md](developers-guide.md).
  - Follow the existing `ResolveError` boundary: one domain error enum per
    capability group with a single `From` conversion, not ad hoc `Error::new`
    calls in leaf helpers.
  - Register `netsuke::jinja::<module>::<reason>` codes with Fluent keys under
    `stdlib.<module>.<condition>`.
- [ ] 6.1.5. Close the manifest-query disclosure gaps.
  - See RFC 0006 §§3.3 and 6.2.
  - Localize `manifest_query_operation_error` through a Fluent key instead of
    building it with `format!`.
  - Add explicit failing stubs for every full-environment helper currently
    absent from the manifest-query environment, all of which vanish rather
    than explaining the restriction: the filters `realpath`, `expanduser`,
    `size`, `linecount`, `hash`, and `digest`; `which`, as both a filter and a
    function; the functions `command_available` and `now`; and the file tests
    `dir`, `file`, `symlink`, `pipe`, `block_device`, `char_device`, and
    `device`.
  - Success: no helper registered in the full environment is silently absent
    from the manifest-query environment.
- [ ] 6.1.6. Add the manifest-query registration contract test. Requires
  6.1.5.
  - See RFC 0006 §6.2.
  - Assert dispositions by exercising each registration rather than by
    differencing name sets. Task 6.1.5 keeps every non-pure helper registered
    as a stub, so both environments hold the same names and the difference is
    always empty.
  - For every helper in the inventory, check that the name resolves in both
    environments, that a pure helper evaluates normally under the
    manifest-query registration, and that a non-pure helper raises the
    restriction diagnostic there.
  - Success: a helper absent from the inventory, or one whose manifest-query
    registration neither evaluates nor raises the restriction diagnostic,
    fails the suite, so a non-pure helper cannot be added without deciding its
    query disposition.
- [ ] 6.1.7. Publish the maintained standard-library inventory.
  - See RFC 0006 §§11 and 14.1.
  - Distinguish MiniJinja built-ins, existing Netsuke extensions, adopted
    Ansible-inspired helpers, and deliberately unsupported Ansible helpers
    with the Netsuke spelling to use instead.
  - Record each collision resolution at both colliding entries, covering
    `hash` against `text_hash`, `groupby` against `group_by`, `unique` against
    `uniq`, `items` against `dict2items`, `in` against `contains`, and the
    `abs` filter against the `abs` test.
  - Success: a test asserts that every registered name appears in the
    inventory, so the table cannot drift.

### 6.2. Read structured data without a subprocess

This step answers whether a manifest can consume compiler metadata, package
manifests, and generated configuration fragments directly, or whether `jq`,
`yq`, and a scripting runtime remain unavoidable host assumptions. Its outcome
determines how much surrounding toolchain a `Netsukefile` still has to assume.
See RFC 0006 §8.1 and
[adr-001-replace-serde-yml-with-serde-saphyr.md](adr-001-replace-serde-yml-with-serde-saphyr.md).

- [ ] 6.2.1. Add `from_json` with duplicate-key rejection and source offsets.
  Requires 6.1.3 and 6.1.4.
  - See RFC 0006 §8.1.
  - Preserve object order and report line, column, and byte offset on failure.
  - Success: a document with a repeated object key fails naming the key and
    the offset of its second occurrence, rather than silently keeping the last
    value.
- [ ] 6.2.2. Add `from_yaml` and `from_yaml_all` over the existing safe YAML
  stack. Requires 6.2.1.
  - See RFC 0006 §8.1.
  - Reject non-standard tags, duplicate keys, and merge keys, and materialize
    the multi-document result rather than exposing a lazy iterator.
  - Establish whether `serde-saphyr` can bound alias expansion; if it cannot,
    reject aliases outright and record that in the standard-library guide.
  - Success: an alias-expansion bomb fails with a bounded-resource diagnostic
    instead of exhausting memory.
- [ ] 6.2.3. Add the deterministic `to_yaml` and `to_nice_json` serializers.
  Requires 6.2.2.
  - See RFC 0006 §§6.3 and 8.1.
  - Pin key ordering, indentation, scalar quoting, line endings, and
    trailing-newline behaviour, quoting every scalar that could be read back
    as a boolean, null, number, or timestamp.
  - Resolve RFC 0006 §16 question 1 before registering: either reject
    `to_nice_yaml` outright or register it solely to raise a diagnostic naming
    `to_yaml(indent=4)`.
  - Success: property tests show that `to_yaml` with `from_yaml` and
    `to_nice_json` with `from_json` round-trip under canonical equality, and
    that the YAML 1.1 `yes`, `no`, `on`, and `off` spellings cannot reach a
    generated file unquoted.

### 6.3. Make configuration layering expressible

This step answers whether the mapping transforms remove the merge and
re-index loops that `vars`, `foreach`, and per-entry overrides currently force
manifest authors to write by hand. Its outcome informs how much of the
platform and toolchain configuration problem the template layer can own. See
RFC 0006 §8.2.

- [ ] 6.3.1. Add `combine` with explicit recursion and list policies. Requires
  6.1.2 and 6.1.4.
  - See RFC 0006 §8.2.
  - Support `recursive` and the `replace`, `keep`, `append`, and `prepend`
    list policies, enumerating the valid values on an unknown one.
  - Preserve first-appearance key order, updating an overridden key in place.
  - Success: property tests show that an empty mapping is the identity under
    every policy, that associativity and self-merge idempotence hold under
    `replace` and `keep`, and that the result is independent of hash iteration
    order. `append` and `prepend` accumulate and are exempt from the latter
    two laws.
- [ ] 6.3.2. Add `dict2items` and `items2dict` with an explicit duplicate
  policy. Requires 6.3.1.
  - See RFC 0006 §8.2.
  - Reject equal `key_name` and `value_name`, and reject missing fields naming
    the element index.
  - Success: `dict2items` followed by `items2dict` is the identity, and a
    duplicate derived key fails by default rather than silently collapsing.
- [ ] 6.3.3. Add `extract` with explicit missing-value behaviour. Requires
  6.3.2.
  - See RFC 0006 §8.2.
  - Keep the key as the filter subject so the filter composes with `map`.
  - Reject negative sequence indices, and treat traversal into a
    non-container as an error even when `default` is supplied.
  - Success: a missing key errors naming the failing step of the path unless
    `default` is given; the filter never yields undefined.
- [ ] 6.3.4. Add `subelements` and `rekey_on_member`. Requires 6.3.3.
  - See RFC 0006 §8.2.
  - Keep `skip_missing` scoped to absence only; a value present at the path
    but not a sequence is always an error.
  - Accept sequences only for `rekey_on_member`, rejecting the
    mapping-of-mappings form that silently discards keys.
- [ ] 6.3.5. Add an end-to-end layered-configuration manifest example.
  Requires 6.3.4.
  - Cover defaults, a platform overlay, and a per-target overlay composed with
    `combine`, `dict2items`, and `extract`.
  - Add the example to
    [stdlib-yaml-and-jinja-guide.md](stdlib-yaml-and-jinja-guide.md) with a
    `tested-example` marker so the documentation harness executes it.
  - Success: the example builds in an isolated workspace and its generated
    Ninja is byte-identical across two runs.

### 6.4. Make build matrices expressible with stable ordering

This step answers whether ordered collection algebra can replace both
hand-expanded target matrices and Ansible's set-backed filters without
introducing a single unstable ordering. Its outcome is the strongest test of
the determinism claim in RFC 0006 §6.3. See RFC 0006 §§8.3 and 8.8.

- [ ] 6.4.1. Add the ordered set algebra. Requires 6.1.2.
  - See RFC 0006 §8.3.
  - Implement `union`, `intersect`, `difference`, and `symmetric_difference`
    with first-appearance ordering and canonical-key deduplication.
  - Success: property tests show idempotence, the documented ordering, and
    that reordering an input's duplicate positions does not change the result.
- [ ] 6.4.2. Add the bounded combinatorial filters. Requires 6.1.3.
  - See RFC 0006 §§6.8 and 8.3.
  - Implement `product`, `combinations`, and `permutations` with checked
    cardinality and the lower ceiling for `permutations`.
  - Success: an over-large request fails naming the computed cardinality and
    the ceiling, without allocating the result.
- [ ] 6.4.3. Add `zip_longest` with a required fill value. Requires 6.4.1.
  - See RFC 0006 §8.3.
  - Omitting `fill_value` is an error, so a silent `none` cannot enter a build
    graph.
- [ ] 6.4.4. Add the collection and truth predicates. Requires 6.1.2.
  - See RFC 0006 §8.8.
  - Implement `any`, `all`, `subset`, `superset`, `contains`, `truthy`, and
    `falsy`.
  - Restrict `convert_bool` to the closed eight-spelling vocabulary, erroring
    on anything else rather than reproducing Ansible's permissive fallback.
  - Success: `truthy` and `falsy` are exact complements wherever both succeed,
    and `contains` is documented against MiniJinja's `in` at both entries.
- [ ] 6.4.5. Add the matrix-determinism end-to-end suite. Requires 6.4.2 and
  6.4.4.
  - Compile a representative target matrix built from `product`, the set
    algebra, and `selectattr` with `contains`, twice from the same inputs.
  - Add a property test that holds the logical input order fixed while varying
    the internal hash state of the mappings the helpers consume, for example
    by building equal mappings through different insertion and reservation
    histories.
  - Do not permute logical input order. RFC 0006 §8.3 makes first-appearance
    order observable, so a different input order may legitimately produce
    different output, and requiring otherwise would contradict the contract.
  - Success: both runs emit byte-identical Ninja, and the property test fails
    if any helper is reimplemented over a hash set.

### 6.5. Make text and version conditions expressible

This step answers whether a coherent, bounded Netsuke regular-expression
dialect and a strict version predicate can replace the `shell()` calls that
manifests currently use to inspect `--version` output and filter path lists.
Its outcome determines whether conditional flag selection can be expressed at
manifest time. See RFC 0006 §§8.4 and 8.5.

- [ ] 6.5.1. Establish the `netsuke-regex-v1` dialect and the bounded pattern
  cache. Requires 6.1.3 and 6.1.4.
  - See RFC 0006 §8.4.
  - Add the `regex` dependency, name the supported and unsupported constructs
    in the standard-library guide, and enforce the compiled-pattern size limit
    and least-recently-used cache from RFC 0006 table 3.
  - Success: an unsupported construct such as a look-ahead produces a typed
    localized diagnostic naming the construct and offset, not a generic parse
    failure.
- [ ] 6.5.2. Add `regex_replace` with dollar-form replacements. Requires
  6.5.1.
  - See RFC 0006 §8.4.
  - Support `count` and `mandatory_count`, and reject a Python-style `\1` or
    `\g<name>` replacement with a diagnostic pointing at the `$1` form.
  - Success: a replacement pasted from an Ansible playbook fails loudly rather
    than emitting the literal text `\1`.
- [ ] 6.5.3. Add `regex_search`, `regex_findall`, and `regex_escape`. Requires
  6.5.2.
  - See RFC 0006 §8.4.
  - Return `none` for a non-match, keep the `regex_findall` return shape
    dependent only on the arguments, and accept only the `netsuke` escape
    dialect.
  - Success: `regex_findall` returns a sequence of strings whether the pattern
    has zero, one, or several capture groups.
- [ ] 6.5.4. Add the `match`, `search`, and `regex` tests. Requires 6.5.1.
  - See RFC 0006 §8.4.
  - Support `match_type` values `search`, `match`, and `fullmatch`,
    enumerating them on an unknown value.
- [ ] 6.5.5. Add the `version` test over the existing `semver` dependency.
  Requires 6.1.4.
  - See RFC 0006 §8.5.
  - Make `operator` required, accept the six symbolic and six mnemonic forms,
    and accept only `semver` for `scheme`.
  - Resolve RFC 0006 §16 question 3 on whether a `v` prefix is tolerated
    before registering the test.
  - Success: parse failure names which operand failed and the offending text,
    and the guide states that build metadata is ignored for comparison.

### 6.6. Compose path text for a platform other than the host

This step answers whether one uniform `dialect` mechanism can serve
cross-compilation better than a family of Windows-specific filter names, and
whether lexical normalization can be offered without weakening the capability
boundary. Its outcome informs how Netsuke describes any future
cross-platform surface. See RFC 0006 §8.6.

- [ ] 6.6.1. Add the `dialect` argument and its `host`, `posix`, and `windows`
  path parsers. Requires 6.1.4.
  - See RFC 0006 §8.6.
  - Extend the existing `basename` and `dirname` filters additively, so
    omitting `dialect` preserves current behaviour.
  - Success: a Unix host parses a Windows path identically to a Windows host,
    with no host-native fallback.
- [ ] 6.6.2. Add `path_join`, `normpath`, and `splitext`. Requires 6.6.1.
  - See RFC 0006 §8.6.
  - Reject an absolute component after the first position in `path_join`, and
    reject empty components.
  - Document the single-suffix `splitext` rule against the existing
    `with_suffix` filter.
  - Success: `['/safe/root', '/etc/passwd'] | path_join` fails naming the
    index rather than yielding `/etc/passwd`.
- [ ] 6.6.3. Add `commonpath`, `relpath`, and `splitdrive`. Requires 6.6.2.
  - See RFC 0006 §8.6.
  - Compare component-wise, reject mixed absolute and relative inputs, and
    reject differing drives or UNC roots under the `windows` dialect.
  - Contrast `relpath` with the stricter existing `relative_to` in the guide.
- [ ] 6.6.4. Add the `abs` test as a pure lexical predicate. Requires 6.6.1.
  - See RFC 0006 §§8.7 and 11.4.
  - Resolve RFC 0006 §16 question 2 on the name before registering.
  - Success: `abs` is registered in the read-only manifest-query environment,
    unlike the filesystem predicates in step 6.7.
- [ ] 6.6.5. Add the combinatorial path-dialect suite. Requires 6.6.3 and
  6.6.4.
  - Cross every lexical path helper with all three dialects and both host
    platforms, including drive-relative paths, UNC roots, trailing
    separators, and leading `..` components.
  - Success: the suite fails if any helper reaches for host-native parsing
    when an explicit dialect was supplied.

### 6.7. Probe host state through injected, query-gated seams

This step answers whether existence probing and environment expansion can be
added without a second ambient-authority path and without any helper silently
disappearing from a manifest query. Its outcome is the practical test of the
capability contract in RFC 0006 §6.4. See RFC 0006 §§8.6 and 8.7.

- [ ] 6.7.1. Add the `exists` and `link_exists` tests. Requires 6.1.6.
  - See RFC 0006 §8.7.
  - Route both through the injected `cap_std` workspace handle, and treat a
    path outside the capability boundary as an error rather than `false`.
  - Success: a dangling symbolic link is `false` for `exists` and `true` for
    `link_exists`.
- [ ] 6.7.2. Add the `same_file` and `mount` tests. Requires 6.7.1.
  - See RFC 0006 §§6.5 and 8.7.
  - Compare file identity rather than path spelling, and error on a missing
    operand rather than reporting `false`.
  - Qualify `mount` explicitly per platform; an unsupported platform errors
    rather than returning a plausible `false`.
  - Success: the platform contract for both tests is stated in the guide and
    exercised on Unix and Windows continuous integration.
- [ ] 6.7.3. Add the `files_only` option to the existing `glob` function.
  Requires 6.7.1.
  - See RFC 0006 §8.7 and
    [adr-010-scope-glob-capability-to-literal-prefix.md](adr-010-scope-glob-capability-to-literal-prefix.md).
  - Leave the existing capability scoping, ordering, and observability
    contracts unchanged, and do not introduce a second glob implementation.
- [ ] 6.7.4. Add `expandvars` through an injected environment reader. Requires
  6.1.5 and 6.6.1.
  - See RFC 0006 §8.6 and
    [adr-008-environment-seam-taxonomy.md](adr-008-environment-seam-taxonomy.md).
  - Support `missing` values `error`, `empty`, and `preserve`, defaulting to
    `error`, and reject malformed references rather than passing them through.
  - Exclude it from the read-only manifest-query registration exactly as `env`
    is excluded, so every read-only generation caller is covered, not just
    `netsuke help targets`.
  - Success: no leaf helper reads the environment ambiently, and the
    manifest-query stub explains the restriction.

### 6.8. Generate file text and stable identities

This step answers whether Netsuke can produce comment banners, encoded
payloads, quoted shell words, human-readable sizes, and content-derived
identifiers without either a subprocess or a second quoting implementation.
See RFC 0006 §8.9.

- [ ] 6.8.1. Add `b64encode`, `b64decode`, and `urldecode`. Requires 6.1.3 and
  6.1.4.
  - See RFC 0006 §8.9.
  - Add the Base64 dependency, support both alphabets and configurable
    padding, and default `urldecode` to `plus=false` so it round-trips with
    MiniJinja's `urlencode`.
  - Success: property tests show both Base64 alphabets and the URL codec
    round-trip, and invalid input errors naming the offset.
- [ ] 6.8.2. Add `to_uuid` over a documented Netsuke namespace. Requires
  6.1.4.
  - See RFC 0006 §8.9.
  - Use the frozen namespace recorded in the RFC, accept an explicit
    `namespace`, and record why UUID version 5 does not fall under the
    `legacy-digests` policy.
  - Success: the default namespace is Netsuke's, not Ansible's, and the guide
    records both its derivation and its literal value.
- [ ] 6.8.3. Add `shell_quote` over the existing quoting machinery. Requires
  3.14.8 and 6.1.4.
  - See RFC 0006 §§8.9 and 13.3.
  - Adopt `shell_quote` as the canonical name that resolves the documented but
    unimplemented `shell_escape` helper, and add the `dialect` argument with
    an enumerated value set.
  - Success: the user guide and the registered surface agree, and no second
    quoting implementation is introduced.
- [ ] 6.8.4. Add `comment` with a closing-marker guard. Requires 6.1.4.
  - See RFC 0006 §8.9.
  - Support the three line styles, the two block styles, and an explicit
    `prefix`, emitting no trailing whitespace.
  - Success: a block style whose input already contains the closing marker
    fails, so comment text cannot escape into a generated file as live syntax.
- [ ] 6.8.5. Add `human_readable` and `human_to_bytes`. Requires 6.1.3.
  - See RFC 0006 §8.9.
  - Pin locale-independent output, parse case-insensitively, select bits by
    keyword rather than by letter case, and use checked integer arithmetic.
  - Success: overflow and non-integral results error rather than truncating,
    and unknown units are rejected with the valid set enumerated.
- [ ] 6.8.6. Add `text_hash` without disturbing the existing `hash` contract.
  Requires 6.1.4.
  - See RFC 0006 §§8.9 and 11.1.
  - Reuse the existing `legacy-digests` gating for `sha1` and `md5`, and
    register neither `hash_text` nor `checksum`.
  - Success: `hash` continues to hash the file at the supplied path, and the
    inventory states the distinction at both entries.

### 6.9. Convert explicit timestamps without reading the clock

This step answers whether timestamp parsing and formatting can be added as
pure helpers over the existing `now()` value, leaving clock access as the only
host-observing time operation. See RFC 0006 §8.10.

- [ ] 6.9.1. Add the shared conversion-specifier set and `strftime`. Requires
  6.1.4.
  - See RFC 0006 §8.10 and table 12.
  - Pin the invariant C locale for the name-producing specifiers, and reject
    every specifier outside the accepted set with the supported set
    enumerated.
  - Accept the `now()` timestamp value and integer epoch seconds; reject
    floats.
  - Success: identical manifests produce identical text on machines with
    different locales.
- [ ] 6.9.2. Add `to_datetime`. Requires 6.9.1.
  - See RFC 0006 §8.10.
  - Accept `UTC` and fixed offsets for `timezone`, rejecting IANA zone names
    so no time-zone database is required.
  - Success: a property test shows that `to_datetime` followed by `strftime`
    round-trips for every lossless format.

### 6.10. Decide the deferred candidates on evidence rather than parity

This step answers whether the capabilities RFC 0006 deliberately withheld are
actually wanted once the accepted set is in use. Its outcome either closes
them permanently or produces a specification with a named consumer, so neither
is adopted for name parity alone. See RFC 0006 §9.

- [ ] 6.10.1. Evaluate seeded `shuffle` and `random`. Requires steps 6.1 to
  6.9.
  - See RFC 0006 §9.1.
  - Adopt only with a named consumer, a required seed, and a pinned
    seed-to-stream mapping recorded as a compatibility contract.
  - Unseeded forms remain permanently rejected.
- [ ] 6.10.2. Evaluate `log`, `pow`, `root`, and checked integer
  exponentiation. Requires 6.10.1.
  - See RFC 0006 §9.2.
  - Adopt only against a real `Netsukefile` requirement, and specify the
    floating-point domain, signed-zero, and rounding behaviour before
    registering anything.
- [ ] 6.10.3. Evaluate `type_debug`. Requires 6.10.1.
  - See RFC 0006 §9.3.
  - Adopt only if a recorded diagnostic session shows MiniJinja's `debug`,
    `pprint`, and type tests do not expose the relevant value kind.

**Success criterion:** Netsuke's template standard library covers the accepted
RFC 0006 set behind one shared contract; every helper carries a purity label,
an enumerated manifest-query disposition, checked resource bounds, and a typed
localized diagnostic; the maintained inventory names every registered helper
and every deliberately unsupported Ansible spelling; and a representative
matrix manifest compiles twice to byte-identical Ninja.
