# Add trusted configured shell definitions (11.2.2)

This ExecPlan is a living document. Keep `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` current at each milestone and
whenever evidence changes the design.

Status: DRAFT

Implementation requires explicit maintainer approval. Publishing this plan as a
draft pull request does not authorize implementation or complete roadmap 11.2.2.

## Purpose and observable outcome

Operators can declare additional structured-command shells in trusted system
and user configuration. Loading configuration accepts structurally valid
entries, appends entries in layer order, and rejects reserved or repeated
names. Project files and explicit configuration selectors cannot grant this
interpreter authority. Errors identify the relevant configuration key before
manifest compilation starts.

This task delivers configuration primitives only. A configured shell need not
exist on the current machine to pass configuration validation. Executable
resolution belongs to 11.2.3; structured-command integration waits for 12.3.1.
No new command execution, shell selector flag, manifest field, or project trust
switch is delivered here.

## Constraints

- Preserve ADR-019, including its corrective addenda, and RFC 0011 section 5.
  Reuse `ShellName` and `BuiltInShell`; do not create another name grammar or
  built-in registry.
- Keep configuration, source provenance, filesystem discovery, and environment
  reading outside execution-domain types. A future resolver consumes validated
  definitions; runners must never consult `CliConfig`.
- Accept definitions only from system/user operator discovery roots and their
  inherited occurrences. An arbitrary explicit file, `--config`, or
  `NETSUKE_CONFIG` does not establish this authority. A project occurrence
  never becomes trusted through path equality with another occurrence.
- Reject unauthorized `shells` keys before generic merging, including empty or
  malformed values. Do not silently discard a request and report success.
  Existing fetch-policy trust flags grant no shell authority.
- Definitions have exactly `name`, `executable`, and `args`. Field validation
  happens during deserialization; merged collisions happen in `PostMergeHook`.
- Do not add an aggregate registry-count or registry-byte cap. ADR-019's first
  correction removed that requirement; the argument-size bound is per entry.
- Never mutate the test harness environment. Inject discovery inputs and use
  child-process environment configuration for end-to-end cases.
- Preserve the pinned Polonius Alpha and next-generation trait solver posture;
  add no compiler-mode switches or defensive borrow-checker workarounds.
- Run gates sequentially with shared Cargo caches and compiler admission.
  Stop on full storage; report unhealthy admission without bypassing it.

## Tolerances (exception triggers)

Changes may touch the configuration/discovery/merge modules, their build-time
source slice, shell-name imports, tests, proof files, and the documentation
listed below. No runtime execution or structured-command AST work is allowed.
No new production dependency, public API compatibility layer, global discovery
precedence change, or project-definition trust mechanism is authorized.

Pause and record a proposed deviation if source authority cannot be retained
through the pinned OrthoConfig interface without changing unrelated policy, if
the executable grammar needs a new platform policy, or if the proof cannot be
connected to the production contract. Seek approval for that deviation; do not
weaken the trust boundary, omit evidence, or invent an admission flag. Each
code file remains at most 400 lines. Split by feature responsibility when
needed, documenting ownership and permitted callers before extraction.

## Risks

The existing `FileScope::Operator` means ordinary configuration precedence, not
independently established shell authority. Treating it as sufficient would
trust arbitrary explicit files. Discovery must retain the kind of root that
loaded each occurrence before flattening inheritance.

The same file can occur under both an operator root and a project root. Global
path deduplication can erase the distinction. Preserve occurrence-level labels
and test both orderings. A project occurrence containing definitions fails even
if the same file was also loaded through a trusted root.

OrthoConfig merge deserializes values after layering. A typed field alone may
not detect an invalid entry hidden by later replacement. Validate every trusted
occurrence's entries before merging, using the same deserializer, then validate
merged collisions through the hook. Keep raw layer values until error
collection and JSON preference resolution are complete.

Host path syntax differs. A relative executable must not slip through as a bare
name because a separator has different meaning on the validating host. Define
and test the lexical policy before adding filesystem probes; none belong here.

A proof of a model does not prove Serde, OrthoConfig, filesystem provenance, or
the executable's source-evaluation behaviour. Keep these residual boundaries
explicit and exercise the actual adapters in tests.

## Progress

- [x] 2026-10-20: Rename the working branch and inspect the clean baseline.
- [x] 2026-10-20: Load requested skills and gather two Wyvern reconnaissance
  reports through context pack `pk_3mn3mkwi`.
- [x] 2026-10-20: Draft contracts, proof sketch, milestones, and test strategy.
- [x] 2026-10-20: Complete six-perspective community-of-experts review and
  reconcile architecture, contract, and operations findings.
- [x] 2026-10-20: Pass plan-only formatting, format checks, Markdown/spelling,
  diagram validation, and whitespace checks before publication.
- [ ] Obtain explicit approval before any implementation.
- [ ] M1: Establish definition authority and reject unauthorized sources.
- [ ] M2: Add validated definitions, append merging, and collision proof.
- [ ] M3: Complete behavioural evidence, documentation, and delivery.

## Surprises & discoveries

The baseline is `37fc364a3caf6845dc4550666dea779d8b9328e2`; roadmap 11.2.1 is
checked complete and the selection vocabulary is present. Its older ExecPlan
header still describes unfinished PR delivery; use current source and roadmap
as prerequisite evidence rather than editing that unrelated history here.

`src/cli/discovery/project_policy.rs` already preserves project occurrences, but
`scope_selected_primary_layer` labels an arbitrary non-project explicit chain
as `Operator`. That label cannot implement this feature's authority rule.
CodeGraph was unavailable in the planning session; targeted source inspection
and shared context-pack references supplied the repository evidence instead.

The repository already declares `googletest`, `pretty_assertions`, `rstest`,
`insta`, and property-test tooling. Reuse those dependencies. The Verus runner
already supports a selected proof file through `VERUS_FLAGS`.

## Decision log

- 2026-10-20: Keep the plan approval-gated and the roadmap entry unchecked.
- 2026-10-20: Introduce shell-specific authority at discovery roots rather than
  reinterpret `FileScope::Operator` and alter existing policy semantics.
- 2026-10-20: Fail unauthorized definitions with a configuration-key error.
  Silent quarantine would hide an operator's mistaken assumption of trust.
- 2026-10-20: Use Verus selectively for the unbounded collision-validation
  kernel, with an independent specification and executable oracle tests. Do not
  formalize diagnostics, deserialization plumbing, or filesystem trust. The
  Wyvern reports favoured tests alone for provenance; this plan agrees and
  limits deductive proof to the sequence contract rather than that adapter.
- 2026-10-20: Pandalump/Wafflecat identified first-file discovery and lost
  candidate provenance; preserve discovery precedence, use explicit inheritance
  in layered examples, and deny ambiguous project/operator roots.
- 2026-10-20: Telefono/Dinolump clarified host path syntax, overlap cases, and
  focused gate commands. Buzzy Bee/Doggylump added normalized environment
  ingress, bounded escaped diagnostics, late JSON preference evidence, and
  set-based production validation. These review findings are incorporated.

## Outcomes & retrospective

The draft has completed Wyvern reconnaissance and community review. No feature
implementation, proof, or feature-test result is claimed. Approval,
implementation, and roadmap closure remain open.

## Conformance basis

The repository-relative documents below are read at baseline commit
`37fc364a3caf6845dc4550666dea779d8b9328e2`; recheck changes before execution.
The terms of reference identify themselves as draft v0.2, last revised
2026-09-25. The design has no independent version identifier, so the baseline
commit identifies its revision.

- [Terms of reference](../terms-of-reference.md), goals and safe-failure
  constraints, and [Netsuke design](../netsuke-design.md), configuration and
  execution boundaries: M1–M3 preserve finite manifest authority.
- [Roadmap 11.2.2](../roadmap.md#112-resolve-trusted-shells-without-expanding-manifest-authority):
  M1 establishes source authority, M2 establishes validation and merge rules,
  and M3 supplies user evidence and documentation. 11.2.3 and 11.3 remain out
  of scope; the wider issue is #593, which this task does not close.
- [ADR-019](../adr-019-structured-command-shell-selection.md), including all
  addenda, and
  [RFC 0011](../rfcs/0011-allow-listed-structured-command-shells.md) sections 5
  and 11: contracts C1–C5 below.
- [RFC 0001](../rfcs/0001-structured-command-blocks.md), shell mode and
  execution boundaries: preserve existing command behaviour throughout.
- [ADR-008](../adr-008-environment-seam-taxonomy.md): injected environment and
  child-process isolation for M1 and M3.

Read [documentation contents](../contents.md) and
[repository layout](../repository-layout.md) first. Use the `codegraph-mcp`,
`hexagonal-architecture`, and `rust-router` skills; use `leta` when semantic
navigation is available, `rust-verification` and `verus` for M2, and
`firecrawl-mcp` for unresolved external-interface questions. The `execplans`
skill governs approval, progress, and deviation recording.

Testing references:

- [rstest fixtures](../rust-testing-with-rstest-fixtures.md).
- [Doctest guidance](../rust-doctest-dry-guide.md).
- [Dependency injection](../reliable-testing-in-rust-via-dependency-injection.md).
- [rstest-bdd](../rstest-bdd-users-guide.md).
- [Formal verification](../formal-verification-methods-in-netsuke.md).

Use the actual repository toolchain and test macros when an embedded upstream
guide describes an older baseline.

## Context and intended boundaries

`src/shell_selection/` owns `ShellName`, `BuiltInShell`, `HostFamily`, and
`ShellSelection`. The `name` module is also consumed by a restricted build-time
source slice. Preserve that import boundary when exposing the existing name
validator to configuration; inspect `build.rs` and the module-slice tests.

`src/cli/config/mod.rs` owns `CliConfig`, its defaults, append annotations, and
`PostMergeHook`. Add a feature-owned `src/cli/config/shells/` module for the
configuration value, local validation, and collision validation. Keep its API
limited to configuration loading and the future shell-registry composition
root. No general interpreter service or execution-domain port is needed.

`src/cli/discovery/layers/mod.rs` discovers root chains;
`src/cli/discovery/project_policy.rs` preserves authority before merging and
retains errors while resolving output preferences. `src/cli/merge/mod.rs` and
`src/cli/merge/input.rs` assemble file, environment, and CLI layers. Audit each
route, including explicit selection and cached discovery replay. Existing
policy helpers are reuse precedents, not interchangeable trust grants.

Before adding any helper, search these modules for equivalent functionality.
Record ownership, permitted callers, and composition rules in
`docs/netsuke-design.md` and `docs/repository-layout.md`. No adapter calls a
future execution adapter; configuration yields validated data only.

## Behavioural contracts and abstract specification

C1, source authority: each file occurrence has a root category, an inheritance
chain, a value, and a diagnostic location. Only roots positively identified as
system or user operator sources grant definition authority to their inherited
occurrences. Project and arbitrary explicit roots do not. Unknown provenance
fails closed. `shells` is configuration-file-only; ambient `NETSUKE_SHELLS` and
generated CLI arguments must not inject it. Reject environment ingress by the
normalized top-level key, covering case/whitespace normalization and `__`
nested components, not merely the exact raw spelling. Existing selector flags
merely choose files. No independent trust mechanism is introduced.

C2, local validity: deserialize a definition with exactly three required
fields. Reuse `ShellName`'s ASCII grammar `[a-z][a-z0-9_-]{0,62}` and its
rejection of `true`, `false`, `yes`, `no`, `y`, `n`, `on`, and `off`.
`executable` is a non-empty UTF-8 string without NUL, either a bare executable
name or an absolute path. Reject relative paths containing `/` or `\`, and
reject `.` and `..` as executable names. On Windows, reject drive-relative and
rooted-without-drive paths; test complete drive and UNC paths. Apply the host's
absolute-path semantics without checking existence. A Windows drive path
containing backslashes is rejected on Unix rather than reinterpreted as a bare
name. `C:tool` is a bare name on Unix, but a rejected drive-relative path on
Windows. `/opt/tool` is absolute on Unix and is rejected as
rooted-without-drive on Windows; `C:\tools\shell.exe` and
`\\server\share\shell.exe` are absolute on Windows. A relative `bin/tool` or
`bin\tool` is rejected on either host. Preserve accepted spelling without
expansion, normalization, trimming, or executable probing.

`args` has 1–16 elements, no NUL in any element, and a sum of UTF-8 byte
lengths at most 4096. Empty individual arguments are permitted. The sum
excludes serialization syntax and process terminators. Use checked accumulation
or remaining-budget subtraction. Reject missing fields, wrong field types, and
unknown fields. Do not infer a source switch or reject an arbitrary
structurally valid prefix merely because it differs from a known shell.

C3, composition: let D be the concatenation, in merge order, of the definition
sequences from authorized occurrences. Empty layers contribute the empty
sequence. This means loaded occurrences, not all candidate files: preserve
first-available-file discovery and use `extends` for layered operator input.
Successful post-merge validation returns D unchanged exactly when no name is
built in and no two positions have equal names. Built-ins are `sh`, `bash`,
`pwsh`, and `powershell`. Identical repeated definitions still collide; there
is no replacement, keyed merge, or deduplication. An empty higher layer does
not clear a lower layer. Invalid input never exposes a partial registry.

C4, operator policy: for executable E and fixed prefix A, the future invocation
is E with A followed by exactly one rendered source argument S. The operator
must choose A such that this invokes E's source evaluator. Structural checking
cannot establish that semantic precondition for arbitrary E. Document a
violation as a trusted operator policy error; do not claim this phase detects
it, or execute E to inspect it. Named selection does not sanitize shell source.

C5, diagnostics and timing: malformed trusted fields, unauthorized sources,
built-in collisions, and duplicate names fail configuration loading before
manifest compilation. Errors identify `shells[index].name`,
`shells[index].executable`, or `shells[index].args` as appropriate, plus a safe
name/source preview when available. Bound and escape control characters in raw
invalid names, executable strings, and source paths before rendering; reuse
`BoundedConfigPath` conventions rather than relying on successful name parsing.
Unauthorized-key errors identify `shells` and the source. Do not print the full
argument vector or environment values. Preserve text/JSON output preference
even when a later layer selects JSON.

## Verification plan and proof sketch

Define the specification before choosing the collision algorithm. Treat a name
as a finite byte sequence, reserved names as a fixed set B, and D as an
arbitrary finite sequence of names. Define Accept(D) independently by the
pairwise condition: every name is outside B, and for all indices i < j in D,
D[i] differs from D[j]. The expected successful output is D itself. There are
no uniqueness or non-reservation preconditions; those are conclusions on
success, not premises.

The proposed executable validator walks left to right, rejects membership in B
or in the previously seen names, and otherwise records the current name. Use
set membership in production, with expected O(n) or O(n log n) work and O(n)
auxiliary storage; the quadratic oracle is test-only. Do not introduce an
aggregate count cap or a benchmark project for this startup validation. The
important representation relationship is that the seen set equals the names in
the already accepted prefix. Proposed semantic lemmas are: extending a valid
prefix succeeds exactly for a non-reserved fresh name; failure has a
reserved-name or earlier-equal-name witness; the complete walk accepts exactly
Accept(D); and success preserves sequence order and payloads. These statements
are hypotheses about proof structure, not a prescribed list of solver helpers.

M2 adds `verus/configured_shells/main.rs` proving the walk against the pairwise
specification over arbitrary sequence lengths. Use byte-sequence names or a
proved faithful equality abstraction, not an unexplained finite integer hash.
Connect the model to the production function by documented branch/state
correspondence and an independent quadratic Rust oracle comparing production
results on actual `ShellName` values. State explicitly that this is a model
proof plus tested correspondence, unless the implementation establishes a
stronger verified executable bridge. Do not describe it as proof of all Rust
configuration loading. Use no `assume`, `admit`, external proof body, or axiom
introduced to obtain the result. Defer triggers, collection algebra, and
auxiliary induction details until the real verification conditions exist.

The non-trivial external assumptions are correct byte/string equality, standard
collection membership, the pinned OrthoConfig append contract, Serde's typed
input contract, and the discovery adapter's source labels. Verus relies on its
standard library and solver soundness. Filesystem ownership and arbitrary
executable semantics are outside the proof. Test the repository's use of each
interface rather than attempting to verify third-party internals.

C1 uses `rstest` source matrices and discovery integration tests, with
`proptest` sequences of root-labelled occurrences, including shared parents and
reversed root order. Include overlapping HOME/XDG/project roots, candidate
deduplication, normalized aliases, and path-normalization failure; ambiguity
must never authorize a definition. Verify unauthorized presence always fails,
no file path alone elevates authority, and unrelated configuration retains its
existing precedence. The proof does not certify these I/O decisions.

C2 uses parameterized boundary tests: name lengths 0/1/63/64, invalid ASCII and
Unicode, all reserved Boolean words, empty/NUL executables, bare/absolute and
relative path families, 0/1/16/17 arguments, NUL arguments, empty elements, and
4095/4096/4097 UTF-8 bytes including multibyte characters. Test every supported
configuration format through the real loader where practical. Missing and extra
fields must have explicit cases. Conventional tests suffice here.

C3 combines direct post-merge tests, real OrthoConfig layer tests, the
unbounded Verus model, and generated oracle comparisons. Generate zero to eight
layers and zero to sixteen definitions per layer, deliberately including empty,
unique, reserved, identical-duplicate, and cross-layer-duplicate classes; these
are test bounds, not production limits. Acceptance under regrouping of
unchanged ordered layers must match; order changes may change the first error
but never validity. Record class coverage and avoid filtering away collisions.

Non-vacuity evidence includes an empty successful registry, two distinct
configured names succeeding, a built-in rejection, and a duplicate introduced
only in the last layer. Temporarily remove duplicate rejection and separately
reserved-name rejection: the focused tests must fail and the corresponding
model mutation must fail verification. Restore mutations before any commit. No
incorrect program or intentionally false theorem remains in the tree.

C4 is an explicit operator assumption, demonstrated by documented `dash`
`args = ["-c"]` examples; no proof or test claims arbitrary evaluator-prefix
correctness. C5 uses text/JSON diagnostic snapshots with stable temporary-path
redaction and child-process tests demonstrating failure before a manifest or
Ninja side effect. `insta` is justified for these two observable formats. Use
`googletest` matchers for typed outcomes and `pretty_assertions` for exact
collections; preserve meaningful failure messages.

## Implementation milestones

### M1: Preserve shell-definition source authority

After approval, first add failing tests for a project `shells` key, an
arbitrary explicit file, a project parent loaded through `extends`, and an
environment injection attempt. Confirm failure reflects missing enforcement,
not a broken fixture. Introduce the narrow root authority metadata and enforce
C1 before flattening into merge layers. Preserve all existing fetch,
environment-grant, budget, discovery-cache, and JSON preference behaviour.

With pinned OrthoConfig 0.9.0, use public `ConfigDiscovery::candidates()` and
`load_config_file_as_chain()` for a narrow Netsuke-owned automatic candidate
walk: the automatic path must have neither an explicit selector nor injected
explicit paths. Classify each candidate root before loading its inherited
chain. Match project roots using the existing normalized comparison; only the
remaining system/XDG/home candidates from this restricted builder receive
operator authority. If project and operator roots overlap, classify the
ambiguous occurrence as untrusted. Preserve the first usable primary chain,
optional-error handling, candidate order, and Netsuke's existing separate
project pass. Differential tests must compare old/new discovery outcomes for
missing, malformed, inherited, and successful ordinary configurations. Do not
reimplement candidate-path generation. `clear_project_roots()` alone is not an
operator-only discovery mechanism: the dependency restores a default current
directory when the builder has no project roots. Keep explicit selection on its
separate untrusted path. Cache the tagged result rather than reconstruct trust
from flattened paths during replay.

A coherent M1 plateau rejects unsupported unauthorized declarations while no
configured-shell schema is exposed yet. Add no permissive intermediate path.
Use existing injected discovery seams. Record the exact root classifier and
handling of inherited occurrences in the design document. Regression tests for
ordinary config discovery and existing trust policies must still pass. Run the
milestone gates below and commit the complete authority change. Remaining gap:
trusted definition schema and collision validation are M2.

### M2: Validate and compose trusted definitions

Add failing field-deserialization and merged-collision tests, then implement
C2/C3 in `src/cli/config/shells/`. Add `shells` with empty default and append
merge to `CliConfig`; suppress CLI and environment ingress through the pinned
derive's supported controls or the existing explicit layer allow-list. Check
the generated help/completion/config metadata and restricted build slice. Do
not add a field to runtime `Cli` merely to carry unused execution state.

Validate each trusted occurrence with the same field deserializer before
composition. Invoke the merged validator from `PostMergeHook`; retain useful
field/index errors through OrthoConfig conversion. Make it impossible for the
normal startup path to reach manifest loading after any definition error.
Perform the proof-sketch review before settling the validator's state shape;
then implement and verify the proof and oracle correspondence described above.

Acceptance requires all boundary and real append-merge tests passing, the
mutation controls detecting the intended faults, no definitions accepted from
unauthorized ingress, and the proof command reporting verified obligations
without failures. The coherent plateau loads validated data without resolving
or executing shells. Commit only after all milestone gates and the proof pass.
Remaining gap: full user-workflow evidence and documentation closure are M3.

### M3: Demonstrate and document the configuration contract

Add `tests/features/configured_shells.feature` and bind it using the existing
`rstest-bdd` test layout. Include these acceptance scenarios; extend with the
invalid-field and collision matrix without duplicating unit-test mechanics:

```gherkin
Feature: Trusted configured shell definitions
  Scenario: Operator layers append different definitions
    Given a system configuration defining shell "dash"
    And the selected user configuration extends that system configuration
    And that user configuration defines shell "custom"
    When Netsuke loads its configuration
    Then both definitions retain their fixed arguments in layer order

  Scenario: A project cannot define an interpreter
    Given a project configuration containing a shell definition
    When Netsuke loads its configuration
    Then loading fails at configuration key "shells"
    And no manifest is compiled

  Scenario: Explicit selection does not grant authority
    Given an otherwise untrusted file defining shell "custom"
    When that file is selected with --config
    Then loading fails at configuration key "shells"

  Scenario: Layered names must be unique
    Given two trusted layers defining shell "dash"
    When Netsuke loads its configuration
    Then loading fails at the duplicate shell name
```

Add subprocess startup tests using isolated config roots and child-only
`Command::env`/`env_clear`, including `NETSUKE_CONFIG`, `--config`, inherited
project definitions, malformed data, duplicate trusted definitions, and both
text and JSON errors. Include an earlier malformed or unauthorized `shells`
value followed by a later `json = true`: assert parseable JSON, the relevant
key, nonzero exit, and an untouched manifest/Ninja sentinel. Cover normalized
environment aliases and malformed nested environment values as denied ingress.
Assert oversized/control-character inputs yield bounded escaped diagnostics
without argument contents or ambient values. Use a valid harmless manifest and
controlled Ninja stub for success; use an observable sentinel to show invalid
config prevents later work. Do not depend on an installed `dash`, `bash`, or
PowerShell.

Add a commented `[[shells]]` example to `docs/sample-netsuke.toml`, including
the complete-prefix operator obligation, limits, trust scope, and absent
executable acceptance. Update `docs/ortho-config-users-guide.md` with append
order, empty-array behaviour, collision rejection, and post-merge hooks. Update
`docs/users-guide.md` to explain authorized sources and actionable failures
while making clear that named-command execution is a later task. Update
`docs/netsuke-design.md`, `docs/repository-layout.md`, and
`docs/developers-guide.md` with ownership, source-label handling, proof scope,
proof/runtime correspondence, and test commands. Record substantive deviations
in an ADR and reference it from the design only after approval.

Any new fenced example in the users' guide needs a stable `tested-example`
marker, `EXPECTED_EXAMPLE_IDS` registration, and semantic validation. Apply the
same existing documentation-contract discipline to edited registered examples.
Run the final gates, inspect the diff for unrelated formatter output, and
commit. Only after feature completion and evidence reconciliation mark roadmap
11.2.2 `[x]` (done). Never mark the whole phase or issue #593 complete. Update
this plan's progress and outcomes; reserve COMPLETE for delivered and reviewed
work.

## Commands and acceptance evidence

Run from the repository root. Before each milestone, verify the branch, clean
state, and current dependency baseline. Use the `scrutineer` agent as the sole
full-gate runner for this worktree. Run the focused newly added test before and
after production changes, recording the actual nextest test identifier in
Progress. The current Makefile has no test-name filter variable. For focused
Linux runs, use its underlying command with the same flags (drop only the mold
flag on other hosts), replacing the filter with the recorded test name:

```bash
RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }-D warnings -Zthreads=8 -Clink-arg=-fuse-ld=mold" \
  cargo nextest run --workspace --all-targets --all-features --no-tests fail -E 'test(configured_shell)'
```

Preserve compiler wrappers and the shared cache. A red run must fail for the
intended missing rule.

After each major implementation milestone, run these commands sequentially,
with Bash `set -o pipefail` and a separate `/tmp` tee log per command:

```bash
make fmt
make check-fmt
make lint
make doc-coverage
make test
make markdownlint
make nixie
```

For M2 and subsequent changes affecting the collision contract, additionally
run the selected proof explicitly; do not rely on the runner's default file:

```bash
make verus VERUS_FLAGS='--proof-file verus/configured_shells/main.rs'
```

If the pinned verifier is absent, install using `make install-verus`,
preserving its pinned version and checksum. A skipped proof is not acceptance.
Review assumptions and correspondence evidence alongside the verifier's success
report.

For M3 documentation examples, additionally run:

```bash
make test-documentation-contracts
make test-workflow-contracts
```

If workflow contracts or coverage validators are changed incidentally, apply
AGENTS.md's corresponding extra gates; do not broaden scope merely to obtain a
green run. For the plan-only PR, run `make fmt`, `make check-fmt`,
`make markdownlint`, and `make nixie`, plus `git diff --check`; no Rust
behaviour or proof is changed by publishing this document. Record actual
outcomes, not assumed success. Full code gates apply to every implementation
commit.

At each plateau check C1–C5 against the assigned evidence, confirm no
unapproved surface or dependency was added, and refresh traceability and risks.
Commit only passing changes. Diagnose a failed gate from its existing log;
rerun after a fix, not while an earlier gate is queued. Never place build
targets in `/tmp`.

## Recovery and delivery

All steps are repeatable with isolated fixtures and unchanged shared Cargo
state. If a milestone fails, keep the preceding passing commit as rollback
point and correct the uncommitted milestone. Do not reset other work, change
trust semantics to bypass a test, or force-push divergent history.

For plan delivery, push the requested branch over SSH after verifying the
`leynos` identity, refresh
`origin/11-2-2-trusted-configured-shell-definitions`, and set it as this
branch's upstream. Create a draft PR titled
`Plan: Trusted configured shell definitions (11.2.2)`. Link this file with a
branch-based GitHub URL and end the description with `## References` containing
<https://lody.ai/leynos/sessions/4d87d518-d8e7-424d-a9e7-9031fd2c62d8>. Read
the published body back and verify draft status, branch, and commit parity.
Then request plan approval and stop before implementation.

## External-interface evidence

Firecrawl checked the versioned
[OrthoConfig 0.9 PostMergeHook documentation](https://docs.rs/ortho_config/0.9.0/ortho_config/trait.PostMergeHook.html)
on 2026-10-20: the hook runs after declarative merging and returns
`OrthoResult<()>`. It does not supply a substitute for root authority metadata.
The checked
[rust-prover-tools interface](https://github.com/leynos/rust-prover-tools)
accepts `verus run --proof-file`; the repository's pinned runner and Makefile
remain authoritative during execution. No external claim replaces an
integration test against the pinned dependency.

## Revision note

2026-10-20: Initial approval-gated draft, based on two Wyvern reconnaissance
reports, current source contracts, and external-interface checks. Reconciled
all six expert perspectives: source-root ambiguity, unchanged discovery order,
host path cases, normalized environment ingress, failure diagnostics, and
proof/test correspondence. Plan-only formatting, Markdown/spelling, diagram,
and whitespace checks passed; feature implementation still awaits approval.
