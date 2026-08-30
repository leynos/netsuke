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
  to write without weakening determinism or the capability boundary, including
  proportionate quality-gate selection from a deterministic Git changeset.
- Phase 7 validates that Netsukefile authors adopt manifest-time testing when
  it is deterministic, mock-friendly, and runs through the same compiler as the
  build.
- Phase 8 validates that one executable repository policy and bounded hostile-
  input fuzzing can close workflow drift and malformed-input failures.
- Phase 9 validates that exact-commit evidence can make release publication
  fail closed without changing Netsuke's public or archive contracts.

Each phase carries one hypothesis, and Phase 6 is the capability track for
template standard-library work. Phases 3 to 5 predate that separation: each
mixes capability delivery with verification and consistency work under a single
hypothesis, and they are not being re-partitioned. New template
standard-library work belongs in Phase 6, while repository and release
hardening belong in Phases 8 and 9, rather than being appended to whichever
phase happens to be open.

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
  `context`, `skill-path`, `runs`, `profile`, `feedback`, and `test`.
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
- Git change-detection helper work follows the shared Phase 6 contract in
  [the helper design](git-change-detection-helpers-design.md).

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

- [x] 3.11.5. Retire `EnvLock` rather than harden its synchronization tests.
  - [x] Ensure production environment-variable callers accept injected
    `mockable::Env` seams.
  - [x] Ensure tests use `mockable::MockEnv` or isolated child processes.
  - [x] Ensure CWD-only callers use the existing working-directory seam,
    absolute paths, or `-C/--directory`.
  - [x] Migrate the remaining callers under issue #491.
  - [x] Migrate the remaining callers under issue #492.
  - [x] Migrate the remaining callers under issue #493.
  - [x] Remove `EnvLock` under issue #494 after every remaining `EnvLock`
    caller has migrated.
  - See [ADR-008](adr-008-environment-seam-taxonomy.md).

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
- [x] 3.14.5. Add regression coverage for conditional action dependency
  manifests. Depends on 3.14.2, 3.14.3, and 3.14.4.
  - [x] Test action-level `when` and action-level `foreach`.
  - [x] Test complementary nextest and legacy branches select exactly one
    action.
  - [x] Test absent-command fallback without invoking `shell()`.
  - [x] Test `deps` lowering in the IR and emitted Ninja build statements.
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
- [x] 3.14.7. Escape backend dollar syntax after Netsuke placeholder lowering.
  This task depends on archived task `1.3.2`. See
  [netsuke-design.md §§2.6 and 5.4](netsuke-design.md).
  - [x] Preserve shell variables such as `$PATH`, `${CARGO:-cargo}`, and
    `$RUSTFLAGS` in generated Ninja by emitting literal dollars as `$$`.
  - [x] Keep the IR free of Ninja-specific dollar escaping.
  - [x] Add command and script regression tests covering shell variables,
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
- [x] 4.2.3. Add Kani harnesses for command interpolation. Requires 4.1.1. See
  [formal-verification-methods-in-netsuke.md §Kani for command interpolation](formal-verification-methods-in-netsuke.md#kani-for-command-interpolation).
  - [x] Prove `$in` and `$out` rewrite only at valid token boundaries. An
    eight-character Kani window is complete for the sigil matcher; Proptest
    covers templates up to 256 characters with at most 8 placeholders.
  - [x] Prove POSIX placeholders in backtick-delimited regions are rejected
    through independent scanner-specification Proptest coverage.
  - [x] Prove unmatched backticks are rejected after substitution through
    adversarial Proptest coverage.
  - [x] Prove successful results satisfy the current `shlex` guard by checking
    its outcome and returned command against the substituted command.
  - [x] Kani verifies the sigil and marker kernels; capped `make kani-ir`
    completed 15 harnesses with zero failures.
  - [x] Mutation evidence rejects all five new Kani and Proptest mutations, and
    `coderabbit review --agent` returned zero findings at each milestone.

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
    diagnostic document. For `netsuke test` this means a command failure,
    not a completed run reporting failed cases; see invariant I8 and
    `6.6.2`.
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
[RFC 0006](rfcs/0006-ansible-inspired-template-standard-library.md). Every task
in this phase targets a release after v0.1.0 final and must not widen the
hardening release defined by issue `#594`. Candidates that RFC 0006 §10 rejects
must not be reintroduced by a later task, and candidates it defers are gated
behind step 6.10. The bounded Git change-detection capability in step 6.11 is
outside RFC 0006's Ansible-derived candidate set, but follows the same shared
contract and capability boundary.

### 6.1. Settle the standard-library contract before the volume arrives

This step answers whether one shared contract, covering canonical equality,
checked bounds, typed localized diagnostics, and an enumerated manifest-query
boundary, can carry every later helper, or whether each capability group needs
its own. Its outcome decides whether steps 6.2 to 6.9 can be reviewed as
ordinary additions or need individual design passes. See RFC 0006 §§6 and 14.1.

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

This step answers whether the mapping transforms remove the merge and re-index
loops that `vars`, `foreach`, and per-entry overrides currently force manifest
authors to write by hand. Its outcome informs how much of the platform and
toolchain configuration problem the template layer can own. See RFC 0006 §8.2.

- [ ] 6.3.1. Add `combine` with explicit recursion and list policies. Requires
  6.1.2 and 6.1.4.
  - See RFC 0006 §8.2.
  - Support `recursive` and the `replace`, `keep`, `append`, and `prepend`
    list policies, enumerating the valid values on an unknown one.
  - Preserve first-appearance key order, updating an overridden key in place.
  - Add a regression test for the recursive non-associativity counterexample
    in RFC 0006 §8.2: with `a = {'x': {'a': 1}}`, `b = {'x': 0}`, and
    `c = {'x': {'b': 2}}`, grouping to the left yields `{'x': {'b': 2}}` and
    grouping to the right yields `{'x': {'a': 1, 'b': 2}}`.
  - Success: property tests show that an empty mapping is the identity under
    every policy, that self-merge idempotence holds under `replace` and
    `keep` with or without `recursive`, that associativity holds when
    `recursive=false` under `replace` and `keep`, and that the result is
    independent of hash iteration order. Associativity is not asserted for
    `recursive=true`, and neither law is asserted for the accumulating
    `append` and `prepend` policies.
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
boundary. Its outcome informs how Netsuke describes any future cross-platform
surface. See RFC 0006 §8.6.

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
identifiers without either a subprocess or a second quoting implementation. See
RFC 0006 §8.9.

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

This step answers whether timestamp parsing and formatting can be added as pure
helpers over the existing `now()` value, leaving clock access as the only
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
actually wanted once the accepted set is in use. Its outcome either closes them
permanently or produces a specification with a named consumer, so neither is
adopted for name parity alone. See RFC 0006 §9.

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

### 6.11. Select proportionate quality gates from Git changes

This step asks whether Netsuke can expose deterministic commit-range paths as
typed template data and compose them with a pure glob predicate, so manifests
can omit irrelevant expensive gates without inheriting shell quoting, path
parsing, or platform differences. It does not add working-tree inspection,
untracked-file discovery, or per-linter file selection.

- [ ] 6.11.1. Implement the strict commit-range parser and feature-private Git
  repository port.
  - Accept exactly non-empty `A..B` and `A...B` forms, resolve both endpoints
    to commits, and require a unique merge base for three-dot ranges.
  - Keep the port private to `stdlib::change_detection`, inject scripted
    responses in tests, and prohibit caller-selected Git flags or pathspecs.
  - See
    [git-change-detection-helpers-design.md §§4 and 7](git-change-detection-helpers-design.md#4-commit-range-semantics).
  - Success: generated and example-backed cases reject malformed ranges and
    option-like endpoints before Git starts, while valid two-dot and three-dot
    ranges select the designed commits and assert the complete operation and
    resolved-ID sequence. Fixed `rev-parse` and `merge-base` vectors must use
    Git's top-level `--no-lazy-fetch` option.
- [ ] 6.11.2. Register `git_changed_files()` with bounded path decoding and
  impurity tracking. Requires 6.11.1.
  - Run fixed `rev-parse`, `merge-base`, and `diff` argument vectors in the
    configured workspace, honouring the injected command `PATH` and requiring
    Git's top-level `--no-lazy-fetch` option on each vector.
  - Parse NUL-delimited output, reject non-UTF-8 names, normalize separators,
    sort and de-duplicate paths, disable rename and external-diff processing,
    and apply the configured capture limit.
  - Install the named manifest-query rejection and mark ordinary rendering
    impure immediately before Git execution.
  - Reserve `git_changed_files` against manifest-variable shadowing.
  - Add localized, bounded diagnostics and low-cardinality operation telemetry.
  - Add a partial-clone integration case with an instrumented remote proving a
    missing promisor object fails locally without remote contact.
  - See
    [git-change-detection-helpers-design.md §§5, 8, and 9](git-change-detection-helpers-design.md#5-changed-path-result-contract).
  - Follow
    [ADR-015](adr-015-use-bounded-git-cli-for-change-detection.md).
  - Success: temporary-repository cases preserve newline-bearing paths,
    represent renames as old plus new paths, reject oversized and malformed
    output, and demonstrate the specified purity transitions.
- [ ] 6.11.3. Implement the pure variadic `matches_glob()` collection filter.
  Requires 6.11.2.
  - Require a sequence of string paths and at least one string pattern; reject
    invalid patterns and validate every path member before returning a Boolean,
    even when an earlier path would match.
  - Enforce fixed v1 preflight limits of 64 supplied patterns and 65,536
    aggregate UTF-8 pattern bytes before compiling or allocating compiled
    patterns; duplicate patterns count towards both limits.
  - Reuse or extract `GlobPattern::new` preprocessing before compiling each
    distinct pattern once, then return true when any path matches any pattern
    using case-sensitive, literal-separator, leading-dot, and recursive `**`
    semantics aligned with `glob()`.
  - Test escaped metacharacters, separator normalization, invalid braces,
    exact-boundary acceptance, count and byte overages, duplicate-pattern
    accounting, and rejection before compiling or allocating compiled patterns.
  - Register the filter in ordinary and manifest-query environments and add it
    to the standard-library filter inventory.
  - See
    [git-change-detection-helpers-design.md §§3 and 6](git-change-detection-helpers-design.md#3-public-template-contract).
  - Success: property-generated path and pattern collections agree with a
    simple any-to-any reference predicate, invalid input fails closed, and
    over-limit pattern sets report observed values with their applicable fixed
    ceiling and are rejected before compilation or allocation.
- [ ] 6.11.4. Validate and document change-aware manifest gates. Requires
  6.11.3.
  - Add an end-to-end manifest matrix spanning two-dot and three-dot ranges,
    additions, modifications, deletions, renames, submodule changes, matches,
    and misses.
  - Assert operation and resolved-ID sequences for both range forms, including
    unique, multiple, and absent three-dot merge bases.
  - Add a runnable Rust-quality-gate example to the standard-library guide and
    align the core design, developer guide, localized diagnostics, and stdlib
    metadata with the public contract.
  - Verify Linux and Windows separator behaviour without mutating the process
    environment or relying on developer-installed commands.
  - See
    [git-change-detection-helpers-design.md §§10 and 12](git-change-detection-helpers-design.md#10-correctness-and-verification).
  - Success: the documented example omits its Rust target for a Python-only
    changeset, includes it for every Rust path class, and passes the repository
    documentation and behavioural gates.

## 7. Netsukefile testing framework

Hypothesis: Netsukefile authors adopt manifest-time testing when it is
deterministic, mock-friendly, and runs through the same compiler pipeline as
the build.

Objective: deliver the `netsuke test` command and YAML test dialect specified in
[RFC 0007](rfcs/0007-netsukefile-testing-framework.md), the
[UX and semantic design](netsuke-test-framework-ux-design.md), and the
[technical design](netsuke-test-framework-technical-design.md).

### 7.1. Seams and loader options

- [ ] 7.1.1. Add the clock provider seam to the stdlib time module. See
  [technical design §5.2](netsuke-test-framework-technical-design.md).
  - [ ] Register `now()` through an injected `ClockProvider` closure held in
    `StdlibConfig`.
  - [ ] Preserve current behaviour when no provider is supplied.
  - [ ] Test an injected provider value, repeated `now()` calls returning
    it, and the ambient fallback when no provider is configured, all
    registered through `StdlibConfig`.
  - [ ] Record the seam classification per
    [ADR-008](adr-008-environment-seam-taxonomy.md).

- [ ] 7.1.2. Introduce the options-carrying manifest loader entry point. See
  [technical design §4.3](netsuke-test-framework-technical-design.md).
  - [ ] Add `ManifestLoadOptions` and `TemplateOverlays`, with existing
    entry points as thin wrappers.
  - [ ] Extend `StdlibRegistration` with a `Test` mode beside `Full` and
    `ManifestQuery` rather than adding a parallel boundary mechanism.
  - [ ] Reuse the `manifest_query_operation_error` diagnostic shape and
    `disabled_env_reader` established by `netsuke help targets`.
  - [ ] Register overlays after stdlib and manifest macros and before
    `foreach` expansion.
  - [ ] Add differential tests showing the build path is unchanged
    (invariants I4 and I7).

- [ ] 7.1.3. Spike MiniJinja overlay shadowing for macro substitution.
  Requires: 7.1.2. See
  [technical design §5.4](netsuke-test-framework-technical-design.md).
  - [ ] Pin `add_function` replacement semantics with a test.
  - [ ] Rewrite the `MACRO_IMPORTS_GLOBAL` prelude for substituted names;
    `add_function` alone is shadowed by the generated
    `{% from ... import %}` statement at render time.
  - [ ] Pin runner-side handle capture for spy passthrough.
  - [ ] Fall back to filtered macro registration if shadowing fails.

- [ ] 7.1.4. Dogfood the seams before dialect work begins. Requires:
  7.1.1, 7.1.2, 7.1.3.
  - [ ] Run the differential fidelity suite over the repository's example
    manifests.
  - [ ] Record the evidence in the RFC before starting 7.2.

### 7.2. Test dialect parsing and discovery

- [ ] 7.2.1. Add the optional `tests` block to the manifest schema. See
  [UX design §3](netsuke-test-framework-ux-design.md).
  - [ ] Keep `deny_unknown_fields` semantics for the block itself.
  - [ ] Verify build-path neutrality with differential snapshots.
  - [ ] Document the minimum-version consequence for older parsers.

- [ ] 7.2.2. Implement the test-suite AST and parser. See
  [technical design §6](netsuke-test-framework-technical-design.md).
  - [ ] Partition known keys from dynamic `test_*` keys.
  - [ ] Enforce the closed-schema and nearest-known-key diagnostics.
  - [ ] Enforce the expression/template field split at parse time.
  - [ ] Enforce the `netsuke_test_version` contract from RFC 0007: accept
    the supported major and a minor at most the supported minor, with
    tests for missing, malformed, unsupported-major, and newer-minor
    values.

- [ ] 7.2.3. Implement discovery and imports. Requires: 7.2.1, 7.2.2.
  - [ ] Resolve `tests.root`, include and exclude patterns, and support
    files.
  - [ ] Confine imports to the test tree.
  - [ ] Fail empty selections without `--allow-empty`.

### 7.3. Mock engine

- [ ] 7.3.1. Implement doubles, matchers, and the journal. Requires: 7.1.2.
  See [UX design §8](netsuke-test-framework-ux-design.md) and
  [technical design §7](netsuke-test-framework-technical-design.md).
  - [ ] Implement stub, mock, and spy kinds with first-match-wins entries.
  - [ ] Compile the closed matcher vocabulary at parse time.
  - [ ] Journal every call with per-case isolation (invariants I1 and I3).

- [ ] 7.3.2. Implement verification and reporting hooks. Requires: 7.3.1.
  - [ ] Fail unmet mock expectations at end of case.
  - [ ] Warn on unused doubles with the `lenient` opt-out.
  - [ ] Render unmatched-call reports with suggested YAML entries.

- [ ] 7.3.3. Implement macro substitution doubles. Requires: 7.1.3, 7.3.1.
  - [ ] Register journalling wrappers over compiled stand-in macros.
  - [ ] Journal calls under `substitutes.<name>`.

### 7.4. Fixture engine

- [ ] 7.4.1. Add sandbox-rooted `glob()` and file-test adapters for the
  test registration. Requires: 7.1.2. See
  [technical design §5.5](netsuke-test-framework-technical-design.md).
  - [ ] Resolve relative glob patterns against the case sandbox rather
    than the process working directory.
  - [ ] Resolve file-test paths through the sandbox handle instead of
    `open_ambient_dir`, rejecting escapes.
  - [ ] Leave the build path's ADR-010 behaviour unchanged.

- [ ] 7.4.2. Implement the fixture lifecycle. See
  [UX design §9](netsuke-test-framework-ux-design.md) and
  [technical design §8](netsuke-test-framework-technical-design.md).
  - [ ] Resolve `uses` dependencies with a topological sort.
  - [ ] Run structured filesystem actions inside a `cap-std` sandbox.
  - [ ] Guarantee reverse-order teardown on every exit path
    (invariant I2, property-tested).

- [ ] 7.4.3. Implement sandbox retention. Requires: 7.4.2.
  - [ ] Support `--keep` for failing cases and print retained paths.

### 7.5. Actions, assertions, and result views

- [ ] 7.5.1. Implement pipeline actions. Requires: 7.1.2. See
  [technical design §9](netsuke-test-framework-technical-design.md).
  - [ ] Compose `load_manifest`, `build_graph`, and `generate_ninja` from
    public library functions.
  - [ ] Accumulate `results` across the case in execution order so
    assertions can compare stages.
  - [ ] Deny network, commands, and ambient environment under test
    (invariant I5).

- [ ] 7.5.2. Implement the case supervisor and frame protocol. Requires:
  7.4.3, 7.5.1. See
  [technical design §9.1](netsuke-test-framework-technical-design.md).
  - [ ] Run each case in a killable child process, keeping discovery,
    scheduling, and reporting in the parent.
  - [ ] Enforce the deadline with `wait_timeout`, then kill and reap,
    reusing the pattern in `src/stdlib/command/execution.rs`.
  - [ ] Carry `CaseResult` over length-prefixed `serde_json` frames
    versioned like `src/json_envelope.rs`; add no new dependency.
  - [ ] Synthesize an errored result with a timeout diagnostic when no
    complete frame arrived, preserving any partial journal.
  - [ ] Assign teardown ownership per the design's table, retain
    timed-out sandboxes, and reap every child including on interrupt.
  - [ ] Test a deliberately non-cooperative template expression,
    termination, timeout reporting, fixture cleanup, child reaping, and
    single-document `--json` output (invariant I10).

- [ ] 7.5.3. Confine subject-manifest paths. Requires: 7.5.1. See
  [UX design §10](netsuke-test-framework-ux-design.md) and
  [technical design §8](netsuke-test-framework-technical-design.md).
  - [ ] Resolve and validate the action `manifest` argument,
    `given.subject`, and case-level `subject` after template evaluation
    and before `open_manifest_workspace`.
  - [ ] Reject absolute paths and sandbox escapes, including through
    existing symlinked components.
  - [ ] Admit the enclosing project's Netsukefile read-only, without
    granting write access to the project root.
  - [ ] Keep valid relative fixture paths working.
  - [ ] Test every path source — the action `manifest` argument,
    `given.subject`, and case-level `subject` — against an absolute path
    and a traversal path such as `../../outside/Netsukefile`.
  - [ ] Test a symlink escape, gated on the platform supporting symbolic
    links.
  - [ ] Test that a valid relative fixture manifest still resolves, and
    that the enclosing-project Netsukefile is admitted read-only.

- [ ] 7.5.4. Implement result views. Requires: 7.5.1.
  - [ ] Expose manifest, graph, and Ninja views with the documented helper
    surface.
  - [ ] Keep views stable across internal IR changes.

- [ ] 7.5.5. Implement assertion evaluation. Requires: 7.5.4.
  - [ ] Normalize scalar and structured assertions.
  - [ ] Distinguish failures from errors end to end.
  - [ ] Implement `expect_failure` with named diagnostics.
  - [ ] Render failing expressions with substituted actual values.

### 7.6. Command, localization, and reporting

- [ ] 7.6.1. Wire the `test` subcommand.
  Requires: 7.2.3, 7.4.3, 7.5.2, 7.5.3, 7.5.5. See
  [UX design §12](netsuke-test-framework-ux-design.md).
  - [ ] Add filters, tags, `--list`, `--fail-fast`, `--timeout`, `--keep`,
    and `--allow-empty`; consume the global `--json` and `--jobs`.
  - [ ] Map exit codes 0 to 3 and 130 as specified.
  - [ ] Implement per-case timeouts, interrupt handling, and
    case-conservation reporting (invariant I9).

- [ ] 7.6.2. Localize and report. Requires: 7.6.1.
  - [ ] Add Fluent keys for report lines, diagnostics, and warnings.
  - [ ] Emit one JSON document per run under the stream-purity contract
    (invariant I8).

- [ ] 7.6.3. Document the framework. Requires: 7.6.1.
  - [ ] Add a users' guide chapter for authoring and running tests.
  - [ ] Document the `--accessibility` output contract for `test` in the
    users' guide, covering how case results, failure diagnostics, and the
    run summary render under `--accessibility on`.
  - [ ] Record the accessibility findings for `test` output in the
    accessibility documentation, cross-referencing `3.8.3`.
  - [ ] Update `contents.md`, the quickstart, and `context --json` follow-on
    notes.
  - [ ] Validate the documentation updates: check the users' guide and
    accessibility entries against the shipped output, and keep the
    documented contract in step with the display-policy behaviour.

**Success criterion:** a Netsukefile author can write the worked example from
[UX design §14.2](netsuke-test-framework-ux-design.md) and run it to a green
result on a machine with no compiler, no network, and a fixed clock.

## 8. Executable repository policy and hostile-input hardening

Idea (hardening): if Netsuke expresses repository policy through one
deterministic inventory and exercises hostile inputs at pure library
boundaries, then workflow drift, stale exceptions, and malformed-input failures
can be closed without displacing the fast checks required for pull-request
review.

This phase implements [RFC 0008](rfcs/0008-code-health.md). It establishes the
deterministic policy spine before adding scheduled coverage-guided depth, so
the repository can distinguish blocking contract failures from longer-running
health signals.

### 8.1. Establish one inventory and health-policy authority

This step tests whether one typed inventory and one health registry can
describe the repository's workflows, references, tiers, and exceptions without
duplicating the gates they govern. Its outcome defines the stable input
boundary for the blocking validator and later scheduled jobs.

- [ ] 8.1.1. Ratify the health-tier and exception-registry contracts.
  - Decide the registry format, schema version, tier vocabulary, ownership
    fields, event or schedule fields, failure meanings, and escalation rules.
  - Require each exception and allowlist entry to name one rule-specific
    subject, one narrow scope, one owner, one rationale, one issue or pull
    request, and creation and expiry dates. For external references, the
    subject identifies the approved owner, repository, exact action or
    workflow reference, and pinned SHA where applicable.
  - Protect the checked-in tier, exception, and external-source allowlist
    registries with CODEOWNERS and branch rules, or require independent
    protected approval before a registry change can affect blocking results.
  - Record the chosen boundary and reuse policy in an Architectural Decision
    Record (ADR) and the developer documentation before implementation.
  - See [RFC 0008 §Health tiers and exception/allowlist
    registry](rfcs/0008-code-health.md#health-tiers-and-exceptionallowlist-registry)
    and [§Open questions](rfcs/0008-code-health.md#open-questions).
  - Success: the registry has one documented schema and cannot represent an
    ownerless, unscoped, non-expiring, or subjectless exception, nor accept an
    unprotected policy change.
- [ ] 8.1.2. Implement the repository reference inventory.
  - Keep rule evaluation pure over a typed inventory; isolate YAML parsing and
    capability-scoped repository reads behind narrow adapters.
  - Inventory tracked workflows, jobs, `needs` edges, permissions, triggers,
    action and reusable-workflow references, Make targets, nextest profiles,
    scripts, configuration paths, and health producers.
  - Return stable source locations and identifiers so each diagnostic names a
    file, YAML path, rule, and remediation.
  - See [RFC 0008 §Repository-wide workflow-policy
    validator](rfcs/0008-code-health.md#repository-wide-workflow-policy-validator)
    and [§Gate self-consistency
    contracts](rfcs/0008-code-health.md#gate-self-consistency-contracts).
  - Success: a clean checkout produces a deterministic inventory without
    GitHub credentials, network access, or process-global environment mutation.
- [ ] 8.1.3. Run the inventory and registry in report-only mode.
  - Requires 8.1.1 and 8.1.2.
  - Classify every finding as a policy defect, repository defect, or explicit
    expiring exception; do not create a permanent baseline file.
  - Correct the known formal-verification documentation drift while preserving
    the existing Proptest, Kani, coverage, mutation, and no-blanket-retry
    contracts.
  - See [RFC 0008 §Phase 1: Inventory and
    report](rfcs/0008-code-health.md#phase-1-inventory-and-report).
  - Success: every reported finding has an owner and disposition, and the clean
    checkout has no unexplained inventory or tier mismatch.
- [ ] 8.1.4. Validate protected policy-input changes.
  - Requires 8.1.1 and 8.1.2.
  - Treat tier, exception, and external-source registry edits as policy
    changes; require CODEOWNERS and branch protection, or independent
    protected approval, before their contents affect blocking results.
  - Test acceptable protected changes and reject direct, unprotected
    modifications before exception evaluation.
  - See [RFC 0008 §Health tiers and exception/allowlist
    registry](rfcs/0008-code-health.md#health-tiers-and-exceptionallowlist-registry).
  - Success: a registry change cannot relax a blocking rule without the
    required protected approval, and validation identifies the protected input.

### 8.2. Make deterministic policy violations block pull requests

This step tests whether complete workflow and reference policy can remain fast,
actionable, and fixture-driven enough to block ordinary review. Its results
decide which checks belong in the per-pull-request tier rather than on a
schedule.

- [ ] 8.2.1. Enforce repository-wide workflow security rules.
  - Requires 8.1.2 and 8.1.3.
  - Reject mutable external references, sources outside the approved
    owner/repository allowlist, fork-only SHAs, implicit or excessive
    permissions, unsafe `pull_request_target` or privileged `workflow_run`
    execution, contradictory concurrency policy, and health checks masked by
    job- or step-level `continue-on-error` without a valid exception. For
    `workflow_run`, reject untrusted checkout content, unverified artefacts,
    and untrusted pull-request data reaching shell commands unless the required
    controls are enforced.
  - Preserve full lower-case commit-SHA pins and test each rule with valid and
    invalid YAML fixtures, including quoted and multiline values, allowlist and
    fork-provenance cases, both `continue-on-error` scopes, both privileged
    triggers, untrusted checkout and artefact-provenance cases, and
    pull-request data flowing to shell commands.
  - See [RFC 0008 §Repository-wide workflow-policy
    validator](rfcs/0008-code-health.md#repository-wide-workflow-policy-validator).
  - Success: every security-policy class has a stable failing fixture and an
    actionable rule identifier, including allowed-source and declared-repository
    SHA provenance, both `continue-on-error` scopes, `pull_request_target`, and
    privileged `workflow_run` coverage for untrusted checkout content, verified
    artefact provenance, and pull-request data reaching shell commands, while
    all tracked workflows pass.
- [ ] 8.2.2. Enforce gate, reference, and documentation consistency.
  - Requires 8.1.2 and 8.1.3.
  - Resolve local workflows and actions, Make targets, effective nextest
    profiles, scripts, configuration and artefact paths, job dependencies, and
    health producers against their owning contracts. Accept nextest's default
    and reserved built-in profiles through its own resolution, require literal
    profile sections only for repository-defined profiles, and reject missing
    or empty referenced tool-version files.
  - Check documentation links and current claims about gates and tools without
    rewriting prose automatically. Keep the blocking validator repository-only
    and network-free: it validates external URL syntax, not reachability.
  - Make focused workflow-contract tests consume the shared inventory where it
    removes duplicate parsing, while preserving their domain-specific
    assertions.
  - See [RFC 0008 §Gate self-consistency
    contracts](rfcs/0008-code-health.md#gate-self-consistency-contracts)
    and [§Documentation
    consistency](rfcs/0008-code-health.md#documentation-consistency).
  - Success: every tracked workflow and indexed documentation reference
    resolves, or is explicitly marked as historical under the documented rule;
    the blocking validator requires no network access.
- [ ] 8.2.3. Enforce tier and exception hygiene through the canonical gate.
  - Requires 8.1.1, 8.1.4, 8.2.1, and 8.2.2.
  - Reject missing producers, contradictory blocking status, expired or
    duplicate entries, unknown rules, missing or malformed rule-specific
    subjects, empty or unbounded scopes, unsupported globs, invalid owners,
    references, or calendar dates, and inline bypasses without a registry
    entry.
  - Add the validator to a canonical Make target and the per-pull-request CI
    path without weakening existing gates or adding blanket retries.
  - See [RFC 0008 §Health tiers and exception/allowlist
    registry](rfcs/0008-code-health.md#health-tiers-and-exceptionallowlist-registry)
    and [§Phase 2: Block deterministic
    contracts](rfcs/0008-code-health.md#phase-2-block-deterministic-contracts).
  - Success: twenty measured runs on the standard CI runner complete in under
    two minutes each and produce the same classifications for the same tree;
    every accepted exception has one valid subject, owner, reference, rationale,
    creation date, and later expiry date.
- [ ] 8.2.4. Add an end-to-end workflow-policy regression corpus.
  - Requires 8.2.3.
  - Cover mutable and unallowlisted references, fork-only SHAs, permission
    escalation, unsafe `pull_request_target` and `workflow_run`, both
    `continue-on-error` scopes, missing local references, broken tiers, invalid
    exceptions, shell quoting, comments, and multiline commands in combination.
  - Exercise every tracked workflow through the same command that CI invokes.
  - See [RFC 0008 §Acceptance
    criteria](rfcs/0008-code-health.md#acceptance-criteria).
  - Success: each policy class fails independently and in representative
    combinations, while the repository corpus passes without allowlist drift.
- [ ] 8.2.5. Report external-link reachability outside blocking validation.
  - Requires 8.2.2.
  - Add a scheduled, non-blocking external-link check with a 10-second
    per-URL timeout and a 10-minute job limit. Classify DNS, connection, TLS,
    redirect, HTTP-status, and timeout failures without changing PR results.
  - See [RFC 0008 §Documentation
    consistency](rfcs/0008-code-health.md#documentation-consistency).
  - Success: a temporarily unavailable external URL is reported with its
    classified failure while the repository-only blocking validator still
    completes deterministically.

### 8.3. Add bounded coverage-guided fuzzing at compiler boundaries

This step tests whether hostile byte streams can exercise the manifest-to-Ninja
pipeline through deterministic, offline library seams. The results establish
which short corpora are suitable for pull requests and which campaigns must
remain scheduled.

- [ ] 8.3.1. Establish the fuzz workspace and bounded harness contract.
  - Pin the supported `cargo-fuzz` and toolchain inputs, decide corpus storage,
    and record the fixed per-target budget: 1 MiB input, 4 MiB output, 64
    recursion frames, 64 MiB temporary storage, 256 MiB corpus, a one-second
    watchdog per input, and a 10-minute watchdog per job.
  - Forbid shell or Ninja execution, network access, unbounded recursion and
    output, and writes outside a capability-scoped temporary directory.
  - Document the harness ownership and composition rules before extracting any
    new shared boundary.
  - See [RFC 0008 §Scheduled cargo-fuzz
    targets](rfcs/0008-code-health.md#scheduled-cargo-fuzz-targets)
    and [§Open questions](rfcs/0008-code-health.md#open-questions).
  - Success: a fixed input always yields the same bounded value or typed error,
    the registry publishes every fixed budget, watchdog expiry is an explicit
    timeout result, and a harness defect cannot invoke user commands or escape
    its test root.
- [ ] 8.3.2. Implement the `manifest_yaml` fuzz boundary.
  - Requires 8.3.1.
  - Exercise arbitrary bytes, invalid UTF-8, malformed YAML, nesting pressure,
    valid minimal manifests, and current parser limits.
  - Check in minimized valid, malformed, and boundary smoke inputs.
  - See [RFC 0008 §Scheduled cargo-fuzz
    targets](rfcs/0008-code-health.md#scheduled-cargo-fuzz-targets).
  - Success: bounded parsing returns a typed result without panic, timeout, or
    uncontrolled filesystem access.
- [ ] 8.3.3. Implement the `jinja_expansion` fuzz boundary.
  - Requires 8.3.1.
  - Exercise undefined values, malformed templates, nested control flow,
    expansion-size pressure, and the current control-key and binding rules.
  - Check in minimized valid, malformed, and boundary smoke inputs.
  - See [RFC 0008 §Scheduled cargo-fuzz
    targets](rfcs/0008-code-health.md#scheduled-cargo-fuzz-targets).
  - Success: expansion is bounded and deterministic and preserves current
    semantics without panic.
- [ ] 8.3.4. Implement the `command_interpolation` fuzz boundary.
  - Requires 8.3.1.
  - Exercise placeholder boundaries, quoting, backticks, repeated
    substitutions, unmatched delimiters, and malformed UTF-8.
  - Preserve the current whole-placeholder rewriting contract.
  - Check in minimized valid, malformed, and boundary smoke inputs.
  - See [RFC 0008 §Scheduled cargo-fuzz
    targets](rfcs/0008-code-health.md#scheduled-cargo-fuzz-targets).
  - Success: the target produces a deterministic typed result without panic,
    timeout, or command execution.
- [ ] 8.3.5. Implement the `path_processing` fuzz boundary.
  - Requires 8.3.1.
  - Exercise malformed UTF-8, absolute and parent paths, separators, NUL, and
    deep paths within the current capability boundary.
  - Preserve capability-root confinement and current path classifications.
  - Check in minimized valid, malformed, and boundary smoke inputs.
  - See [RFC 0008 §Scheduled cargo-fuzz
    targets](rfcs/0008-code-health.md#scheduled-cargo-fuzz-targets).
  - Success: the target produces a deterministic typed result without panic,
    timeout, uncontrolled filesystem access, or path escape.
- [ ] 8.3.6. Implement the `ninja_emission` fuzz boundary.
  - Requires 8.3.1.
  - Exercise valid and rejected intermediate representation values, hostile
    names and commands, ordering variation, and output-size limits.
  - Check in minimized valid, malformed, and boundary smoke inputs.
  - See [RFC 0008 §Scheduled cargo-fuzz
    targets](rfcs/0008-code-health.md#scheduled-cargo-fuzz-targets).
  - Success: equivalent input produces deterministic output accepted by the
    renderer contract, or a typed error, without running Ninja.
- [ ] 8.3.7. Integrate fuzz smoke checks, scheduled campaigns, and crash
  promotion.
  - Requires 8.2.3, 8.3.2, 8.3.3, 8.3.4, 8.3.5, 8.3.6, and bounded Proptest
    suites for input, output, recursion, temporary-storage, corpus, fuel, and
    operation limits; workflow references and job graphs; exception subjects,
    scopes, globs, owners, references, and dates; limit-edge cases; the
    256-case bound; and retained regressions. Bound those suites to 10,000 fuel
    units per input, 1,000,000 per job, 32 jobs, 64 graph edges, 40-character
    SHAs, 32 exception entries, eight scopes per entry, and 128-character
    fields.
  - Run only measured, deterministic smoke corpora on pull requests; run longer
    budgeted campaigns on a scheduled workflow.
  - Promote a fuzz target only after deterministic fuel and operation
    properties cover those invariants and retained regression inputs; use
    watchdog elapsed time only to report an explicit harness timeout.
  - Publish target, corpus revision, execution budget, and crash result, retain
    failure artefacts, and minimize each crasher.
  - Require a deterministic regression test before deleting or quarantining a
    crasher, and do not use blanket retries.
  - Open or update an owned investigation for each scheduled failure. Treat a
    timeout, out-of-memory result, or uncontrolled filesystem access as a
    harness defect that blocks promotion; correct validator false positives
    with a fixture and rule change rather than a broad allowlist.
  - See [RFC 0008 §Phase 3: Add scheduled
    fuzzing](rfcs/0008-code-health.md#phase-3-add-scheduled-fuzzing).
  - Success: every target has a passing smoke corpus and bounded properties
    cover the range invariants, resource and operation limits, validator graph
    and exception-schema boundaries, limit edges, the 256-case bound, and
    retained regressions before promotion; any scheduled failure remains
    visible until its minimized regression test passes.

### 8.4. Ratchet health signals from measured evidence

This step tests whether the new signals are stable, affordable, and owned after
real operation. Its outcome determines promotion to blocking status and removes
temporary exceptions instead of normalizing them.

- [ ] 8.4.1. Measure policy and fuzz evidence.
  - Requires 8.2.4 and 8.3.7.
  - Record at least twenty consecutive policy runs and twenty consecutive
    scheduled fuzz runs with revisions, budgets, outcomes, crash regressions,
    coverage trends, mutation survivors, and exception age.
  - Distinguish tool or runner outages from product and policy failures.
  - See [RFC 0008 §Phase 4: Ratchet and
    review](rfcs/0008-code-health.md#phase-4-ratchet-and-review).
  - Success: the evidence has no unexplained missing run, hidden retry, stale
    crasher, or ownerless exception.
- [ ] 8.4.2. Promote only stable signals and reconcile documentation.
  - Requires 8.4.1.
  - Promote a scheduled signal only when its measured cost and stability fit
    the per-pull-request budget; otherwise keep its scheduled failure response
    explicit.
  - Remove resolved exceptions, review unmaintained entries, and align the
    tier registry, workflows, Make targets, developer guide, and formal
    verification guide.
  - See [RFC 0008 §Compatibility and
    migration](rfcs/0008-code-health.md#compatibility-and-migration).
  - Success: the registry and documentation agree with every producing job,
    and each surviving exception remains narrow, owned, justified, and current.

## 9. Exact-commit release integrity and admission

Idea (hardening): if Netsuke binds release-mode invariants, security and
dependency policy, and archive checks to deterministic evidence for the exact
tag commit, then publication can fail closed before an invalid or
under-evidenced artefact becomes a release.

This phase implements [RFC 0005](rfcs/0005-release-hardening.md). It keeps
measurement separate from enforcement, then makes the existing publication job
consume one read-only admission decision without changing public command or
archive naming contracts.

### 9.1. Settle the release evidence and operating contracts

This step tests whether the release candidate, its policy inputs, and its
artefacts can be described by one deterministic evidence contract. The outcome
fixes the decisions that profile measurement, security scans, and publication
must share.

- [ ] 9.1.1. Ratify release measurement, waiver, and retention policy.
  - Inventory production `debug_assert` sites, supported targets, release
    command paths, runner classes, archive and sidecar names, publication
    permissions, and current gate producers.
  - Decide representative workloads, history-scan freshness, waiver approval
    authority and maximum duration, notice retention or embedding, and whether
    first-version evidence needs external attestation.
  - Record substantive decisions as ADRs and update release architecture and
    developer documentation.
  - See [RFC 0005 §Open
    questions](rfcs/0005-release-hardening.md#open-questions) and
    [§Phase 1: Measure and
    inventory](rfcs/0005-release-hardening.md#phase-1-measure-and-inventory).
  - Success: every threshold, freshness rule, waiver, and retained artefact has
    one documented owner and source of truth.
- [ ] 9.1.2. Define and validate the release-evidence manifest schema.
  - Requires 9.1.1.
  - Bind the tag, exact commit, repository, pinned Rust toolchain, policy-tool
    versions and data state, approved workflow run, immutable producer job or
    artefact identities, every producer's checked-out release SHA, gate result
    and log identities, profile measurements, scan freshness, policy results,
    and every archive's target, byte size, and SHA-256 digest.
  - Reject unknown, missing, stale, malformed, duplicated, or contradictory
    evidence and require exactly one sidecar per archive.
  - See [RFC 0005 §Release-admission
    evidence](rfcs/0005-release-hardening.md#release-admission-evidence).
  - Success: schema fixtures prove every required field, exact-commit relation,
    producer provenance binding, and malformed or mismatched identity case
    without granting publish permissions.
- [ ] 9.1.3. Capture reproducible release-profile baselines.
  - Requires 9.1.1.
  - Measure the current profile on the pinned toolchain, supported targets,
    runner classes, build flags, and representative workloads selected by
    9.1.1.
  - Record stripped and unstripped binary sizes and representative-workload
    timings with the evidence inputs needed to reproduce them.
  - See [RFC 0005 §Release-mode
    invariants](rfcs/0005-release-hardening.md#release-mode-invariants).
  - Success: five repeated baseline runs per supported target are retained and
    identify their exact commit and build inputs.

### 9.2. Enable and exercise release-mode invariants

This step tests whether overflow checks and debug assertions can remain enabled
for all supported release targets without violating valid command behaviour or
the accepted size and performance budgets.

- [ ] 9.2.1. Enable explicit release overflow checks and debug assertions.
  - Requires 9.1.3.
  - Add one workspace release profile with `overflow-checks = true` and
    `debug-assertions = true`, and prove packaging jobs do not override it.
  - Preserve the pinned nightly toolchain, Polonius, Kani, and existing
    all-target and all-feature gates.
  - See [RFC 0005 §Release-mode
    invariants](rfcs/0005-release-hardening.md#release-mode-invariants).
  - Success: every supported target builds with both invariants enabled and no
    release path silently restores Cargo defaults.
- [ ] 9.2.2. Add end-to-end release-path invariant coverage.
  - Requires 9.2.1 and bounded Proptest suites for release arithmetic, digest
    and commit binding, freshness, archive cardinality, size and performance
    limits, limit-edge cases, the 256-case bound, and retained regressions.
  - Generate arithmetic values immediately below, at, and above each boundary;
    measurement pairs from zero through twice the baseline; archive sets from
    zero through one above the permitted cardinality; and timestamps around the
    freshness boundary. Bound command inputs to 64 KiB, staged archive bytes to
    64 MiB, and every property run to 256 cases; retain each counterexample as
    a checked-in regression that the blocking suite replays.
  - Exercise every supported command path that can reach the seven production
    `debug_assert*` sites in `src/ir/cycle_support.rs`,
    `src/ir/cycle_detector.rs`, `src/ir/cmd_interpolate/mod.rs`,
    `src/stdlib/time/format.rs`, `src/stdlib/command/quote.rs`,
    `src/ninja_gen/mod.rs`, and `src/cli/discovery_layers.rs`, plus selected
    arithmetic boundaries, valid inputs, and intentionally rejected inputs.
  - Cover `NamedAction::write_into` for `Recipe::Rule`: assert the
    debug-assertion panic in its applicable profile and
    `NinjaGenError::UnsafeNinjaValue` without debug assertions.
  - Run the bounded property cases before release admission, including the
    256-case bound and regression inputs retained from prior failures.
  - Require supported failures to remain typed, documented errors rather than
    assertion failures or arithmetic panics.
  - See [RFC 0005 §Measurable acceptance
    criteria](rfcs/0005-release-hardening.md#measurable-acceptance-criteria).
  - Success: every named assertion site, the `Recipe::Rule` profile-dependent
    rejection, and selected overflow boundaries are covered across supported
    targets without weakening an invariant. Bounded properties cover release
    arithmetic, digest and commit binding, freshness, archive cardinality, size
    and performance limits, limit edges, the 256-case bound, and retained
    regressions before admission.
- [ ] 9.2.3. Enforce release-profile size and performance budgets.
  - Requires 9.1.2, 9.1.3, and 9.2.2.
  - Run five candidate measurements with the same inputs as the baseline and
    retain both result sets in release evidence.
  - Fail when any target's median representative workload is more than 5%
    slower or its stripped binary is more than 10% larger; retain unstripped
    size as a diagnostic.
  - See [RFC 0005 §Measurable acceptance
    criteria](rfcs/0005-release-hardening.md#measurable-acceptance-criteria).
  - Success: repeated measurements are reproducible, exact-commit-bound, and
    fail closed when either budget is exceeded.

### 9.3. Enforce secret, dependency, and licence policy

This step tests whether pinned, versioned security and supply-chain checks can
block release candidates without exposing secret material or converting
temporary waivers into permanent trust.

- [ ] 9.3.1. Add pinned working-tree secret scanning.
  - Requires 8.2.3 and 9.1.1.
  - Scan tracked, staged, and developer-local untracked candidate files with a
    local hook or command before submission.
  - Run a blocking CI scan over checkout-tracked files and untracked files
    generated by the runner on pull requests, pushes, and release candidates,
    using a SHA-pinned action or verified pinned executable.
  - Keep every checkout and scanning action, and every newly introduced
    release-hardening download, pinned to an immutable commit SHA or verified
    pinned executable; reject an unpinned action tag or download URL.
  - Redact findings and require false-positive suppressions to identify the
    pattern, path, reviewer, rationale, and expiry without disabling the
    repository scan.
  - See [RFC 0005 §Secret
    scanning](rfcs/0005-release-hardening.md#secret-scanning).
  - Success: a synthetic secret fails each candidate event without printing
    secret material, while an unpinned scanner or broad suppression fails
    policy validation.
- [ ] 9.3.2. Add scheduled reachable-history secret scanning.
  - Requires 8.2.3, 9.1.2, and 9.3.1.
  - Use a tag-triggered or tag-ref full-history producer for each release,
    record the release tag or ref and its resolved commit SHA, and publish
    commit-bound scan evidence. Keep the recurring scan schedule and freshness
    window machine-readable.
  - Require the admission manifest to consume a successful, freshness-valid
    result whose resolved SHA matches the exact release-tag commit.
  - Treat a late, missing, unavailable, or newly failing scan as unknown or
    failed, never passed.
  - See [RFC 0005 §Secret
    scanning](rfcs/0005-release-hardening.md#secret-scanning).
  - Success: release evidence rejects stale history scans and identifies a
    finding without disclosing its content.
- [ ] 9.3.3. Add versioned advisory and dependency policy.
  - Requires 8.2.3, 9.1.1, and 9.1.2.
  - Pin and run `cargo-audit` and `cargo-deny` against the committed lockfile
    and versioned policy files.
  - Fail on unwaived vulnerabilities or unsoundness, banned crates or sources,
    and denied or unknown licences.
  - Require waivers to name the advisory, dependency, reason, owner, approval,
    and expiry; generated output cannot waive a failed policy.
  - See [RFC 0005 §Dependency and licence
    policy](rfcs/0005-release-hardening.md#dependency-and-licence-policy).
  - Success: clean policy output is reproducible from the committed lockfile,
    tool versions, registry state, and policy files, and every advisory,
    source, ban, and licence negative fixture fails closed.
- [ ] 9.3.4. Add versioned notice and unmaintained-dependency policy.
  - Requires 8.2.3, 9.1.1, 9.1.2, and 9.3.3.
  - Pin and run `cargo-about` and the selected `cargo-unmaintained` advisory
    against the committed lockfile and versioned policy files.
  - Fail on incomplete or unrenderable notices and newly unmaintained direct
    dependencies; report transitive findings for owned review.
  - Require each exception to name the dependency, reason, owner, approval,
    and expiry; generated output cannot waive a failed policy.
  - See [RFC 0005 §Dependency and licence
    policy](rfcs/0005-release-hardening.md#dependency-and-licence-policy).
  - Success: the notice inventory is complete and reproducible, and each
    unmaintained finding has the blocking or review outcome defined by policy.

### 9.4. Make exact-commit evidence a publication prerequisite

This step tests whether publication can consume one read-only admission result
and remain impossible for stale, incomplete, or mismatched evidence. Its dry
runs prove the failure boundary before any job receives release permissions.

- [ ] 9.4.1. Produce exact-commit evidence for every required gate.
  - Requires 9.2.3, 9.3.1, 9.3.2, 9.3.3, and 9.3.4.
  - Collect formatting, lint, tests, rustdoc, Whitaker, Kani, profile
    measurements, secret scans, dependency and licence policy, notices, and
    archive checksum results under the schema from 9.1.2.
  - Bind every producer to the tag commit, approved workflow run, and exact
    checked-out release SHA; record tool versions, policy inputs, and immutable
    producer, log, and artefact identities or digests.
  - See [RFC 0005 §Release-admission
    evidence](rfcs/0005-release-hardening.md#release-admission-evidence).
  - Success: a candidate has exactly one successful, reproducible,
    provenance-bound result for every required gate, target, archive, and
    checksum sidecar.
- [ ] 9.4.2. Add the read-only release-admission job.
  - Requires 8.2.3, 9.1.2, and 9.4.1.
  - Give admission no publication permission, require it to validate the exact
    tag-to-commit binding, approved workflow run, producer checkout SHA, and
    all evidence, and make the publication job depend on its success.
  - Recompute every staged archive SHA-256 from its bytes, then compare it with
    its matching `.sha256` sidecar and evidence-manifest digest; reject any
    filename, cardinality, sidecar, commit, result, provenance, or digest
    mismatch.
  - Preserve existing archive names, target coverage, checksum sidecars,
    `cargo-binstall` resolution, action SHA pins, and release staging policy.
  - See [RFC 0005 §Phase 4: Turn on
    admission](rfcs/0005-release-hardening.md#phase-4-turn-on-admission).
  - Success: no job with publication permission can run before successful
    exact-commit admission, and no archive is admitted without three-way
    byte-derived digest agreement.
- [ ] 9.4.3. Add end-to-end release-admission dry-run coverage.
  - Requires 9.4.2.
  - Exercise clean evidence and missing, stale, malformed, failed,
    contradictory, wrong-commit, wrong-workflow, wrong-producer-SHA,
    provenance-identity, duplicate-sidecar, missing-sidecar, and
    checksum-mismatch combinations without publishing assets.
  - Prove tool outages and expired waivers remain unknown or failed, and prove
    a dry run cannot bypass admission for a real publication event.
  - See [RFC 0005 §Failure modes and
    mitigations](rfcs/0005-release-hardening.md#failure-modes-and-mitigations)
    and [§Measurable acceptance
    criteria](rfcs/0005-release-hardening.md#measurable-acceptance-criteria).
  - Success: every invalid matrix case blocks before upload, while clean
    evidence preserves the established archive and sidecar contract.
- [ ] 9.4.4. Enable publication admission and document rollback.
  - Requires 9.4.3.
  - Enable the dependency only after all supported-target dry runs pass, retain
    the evidence manifest with the workflow run, and update release operator
    guidance.
  - Limit rollback to disabling publication while retaining checks and
    evidence; do not remove release invariants or widen thresholds to clear a
    failed release.
  - See [RFC 0005 §Compatibility and
    migration](rfcs/0005-release-hardening.md#compatibility-and-migration).
  - Success: a real release is publishable only from the admitted exact tag
    commit, and its retained evidence reproduces the admission decision.
