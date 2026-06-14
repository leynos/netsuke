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
- Phase 4 remains active because none of its formal-verification work has been
  delivered yet.
- New CLI-redesign work starts at `3.15` and Phase 5 so historical numbers are
  not reused.

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
    `NETSUKE_CONFIG` > `NETSUKE_CONFIG_PATH`) verified by exhaustive rstest
    cases and a proptest property test (PR `#327`, closes `#291`).
  - [ ] Depend on OrthoConfig `5.2.3` for consumer boundary guidance.
  - [ ] Preserve Netsuke-specific precedence expectations for manifest path,
    display policies, locale, and profile selection.
  - [ ] Verify that CLI flags override environment, profile, project, user,
    system, and default configuration layers.

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
  Requires archived task `2.2.3`. See
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
- [x] 3.14.4. Add `command_available(name, **kwargs)` as a non-throwing
  executable probe. Requires archived task `3.5.1`. See
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
- [ ] 3.14.7. Escape backend dollar syntax after Netsuke placeholder lowering.
  Requires archived task `1.3.2`. See
  [netsuke-design.md §§2.6 and 5.4](netsuke-design.md).
  - [ ] Preserve shell variables such as `$PATH`, `${CARGO:-cargo}`, and
    `$RUSTFLAGS` in generated Ninja by emitting literal dollars as `$$`.
  - [ ] Keep the IR free of Ninja-specific dollar escaping.
  - [ ] Add command and script regression tests covering shell variables,
    `$in` / `$out`, and unrelated identifiers such as `$input`.
- [ ] 3.14.8. Make Jinja command helpers match the documented ergonomics.
  Requires archived task `2.2.4` and 3.14.4. See
  [netsuke-design.md §§4.4 and 4.5](netsuke-design.md).
  - [ ] Add `env(name, default=...)` without changing the existing missing and
    invalid UTF-8 diagnostics.
  - [ ] Implement or remove the documented `shell_escape` helper so the user
    guide and code agree.
  - [ ] Add `shell_join` and `compact` helpers for deliberate shell recipes.
  - [ ] Add documentation and tests showing optional `RUSTFLAGS` construction
    without shell parameter expansion.
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
  - [ ] Add target/action `description` support and let it override referenced
    rule descriptions for the concrete edge.
  - [ ] Report selected action descriptions in normal Ninja progress output.
  - [ ] In verbose mode, report why manifest-time `when` branches were included
    or skipped.
  - [ ] Do not add generic `debug`, `info`, or `warn` manifest keys unless a
    later diagnostics design defines severity semantics.

### 3.15. Canonical CLI redesign

- [ ] 3.15.1. Replace the pre-0.1.0 command surface with canonical names.
  - [ ] Rename `manifest` to `generate`.
  - [ ] Remove `build --emit`; use `generate --output`.
  - [ ] Add `check`, `context`, `skill-path`, `runs`, `profile`, and
    `feedback`.
  - [ ] Rename `--file` to `--manifest`, keeping `-f` as an intentional
    shorthand.
  - [ ] Depend on OrthoConfig `7.1.1` to `7.1.3` for shared vocabulary policy
    and global option glossary.

- [ ] 3.15.2. Add non-interactive and mutation-safety guarantees.
  - [ ] Add root `--no-input`.
  - [ ] Make prompts impossible unless a future explicit interactive mode is
    added.
  - [ ] Require `--force` for destructive operations.
  - [ ] Require or support `--dry-run` for consequential operations.
  - [ ] Make bare `clean` fail fast with a corrective hint.
  - [ ] Depend on OrthoConfig `7.2.1`, `7.2.2`, and `8.1.1` for shared
    non-interactive and mutation metadata.

- [ ] 3.15.3. Replace diagnostics-only JSON with canonical structured output.
  - [ ] Remove `--diag-json` and `--output-format`.
  - [ ] Add root `--json`.
  - [ ] Emit exactly one JSON result document on successful JSON-mode commands.
  - [ ] Emit exactly one JSON diagnostic document on failing JSON-mode commands.
  - [ ] Suppress progress, colour, emoji, tracing, and timing text in JSON mode.
  - [ ] Snapshot every v1 JSON schema.
  - [ ] Depend on OrthoConfig `7.2.3` to `7.2.5`, `7.3.1`, `8.1.1`, and
    `8.1.2` for shared result, stream, exit-code, and enumerable-error
    metadata.

- [ ] 3.15.4. Replace legacy output preferences with canonical policy flags.
  - [ ] Replace `--colour-policy` with `--color auto|always|never`.
  - [ ] Replace `--spinner-mode` and boolean `--progress` with
    `--progress auto|always|never`.
  - [ ] Replace `--no-emoji` with `--emoji auto|always|never`.
  - [ ] Add `--accessibility auto|on|off`.
  - [ ] Update OrthoConfig field integration, environment names, config
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
  - [x] Run only the bounded smoke harness set on pull requests.
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
