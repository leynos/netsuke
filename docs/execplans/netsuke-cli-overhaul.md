# Plan the Netsuke command-line interface (CLI) and documentation overhaul

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: IN PROGRESS (roadmap fidelity review)

## Purpose / big picture

Netsuke is pre-0.1.0, so the project can replace inconsistent command names,
flags, documentation framing, and roadmap commitments without preserving
backwards compatibility. The goal of this work is to overhaul the design
documents and future roadmap so they describe one clear product direction:
Netsuke remains friendly for humans, but every interface is designed so agents,
Continuous Integration (CI) systems, editor integrations, and shell scripts can
discover it, invoke it without prompts, parse it, retry safely, and recover
from failure.

After this plan is executed, a reader should be able to open the repository and
see the same doctrine reflected across the architecture design, CLI design,
roadmap, user guide, README, and execution plans. The planned product shape is
not "agent-first at the expense of people". It is human-first in presentation
and agent-consistent in structure. Friendly copy, localization, accessibility,
and progress rendering stay first-class, but they sit on top of a stable,
bounded, machine-readable command contract.

Observable success means:

1. `docs/netsuke-design.md` defines an agent-consistent human-first CLI
   contract with canonical vocabulary, stream purity, JSON result schemas,
   mutation boundaries, bounded responses, introspection, profiles, run
   history, delivery, feedback, and mechanical drift checks.
2. `docs/netsuke-cli-design-document.md` is rewritten around the same contract
   instead of treating automation as a diagnostics-only add-on.
3. `docs/roadmap.md` tracks unfinished and future work only, while
   `docs/archive/roadmap-completed-foundations.md` preserves completed
   historical work with relevance classifications and traceability notes.
4. `docs/users-guide.md`, `README.md`, `docs/contents.md`, and any affected
   configuration documentation stop presenting old spellings as the desired
   future interface.
5. Follow-on ExecPlans exist for implementation-sized slices, so future agents
   do not try to land the entire overhaul as one unreviewable change.

## Constraints

- This plan remains the governing design-overhaul plan. Do not implement Rust
  source changes from the overhaul until the user explicitly approves that
  implementation work. Documentation and roadmap revisions may proceed when the
  user explicitly requests them.
- Preserve the product decision from the source conversation: Netsuke is
  pre-0.1.0 and has no backwards-compatibility obligation for old CLI spellings
  or configuration names.
- Preserve the human-first interface goal. The revised documents must not
  claim that agents are the only or primary audience. The target is friendly
  human UX with agent-consistent structure at every boundary.
- Use British English with Oxford spelling in prose. Preserve conventional US
  spelling in command names, flags, environment variables, schema fields, and
  external APIs, for example `--color`.
- Keep documentation truthful. Shipped behaviour, planned behaviour, and design
  requirements must be clearly separated.
- Treat `docs/roadmap.md` as the forward planning spine. Completed checkboxes
  may be reopened or replaced where they describe a pre-0.1.0 surface that is
  no longer the desired product shape.
- Do not reuse roadmap step number `3.14`; it already contains conditional
  action planning. Add new work under later available numbers or new phases.
- Do not duplicate reusable command-contract work that is planned in
  OrthoConfig. Netsuke should depend on OrthoConfig roadmap tasks for generic
  configuration, generated command metadata, policy linting, shared agent
  context schemas, profile metadata, delivery/feedback parsers, and execution
  ledger metadata. Netsuke should own only Netsuke-specific build, package, and
  manifest semantics.
- State explicit dependencies on OrthoConfig roadmap tasks in every planned
  Netsuke roadmap item that relies on shared command-contract machinery. If an
  OrthoConfig task is not available in time, Netsuke may plan a temporary local
  adapter only when the task says so, and the local adapter must be labelled as
  temporary.
- Preserve existing planned work and historical completed work on the Netsuke
  roadmap. Do not delete a completed or planned Netsuke roadmap item merely
  because the CLI surface is being redesigned. Mark items as "superseded",
  "renamed", "moved", or "still relevant" unless the UI redesign makes the work
  genuinely irrelevant to the future product.
- Prefer Makefile targets for validation. For documentation-only changes,
  run `make fmt`, `make markdownlint`, and `make nixie`; run `make check-fmt`
  if formatting touches Rust or if the implementation phase includes code.
- Use `tee` to log long validation outputs when executing the implementation
  phase. A suitable naming pattern is
  `/tmp/$ACTION-netsuke-feat-netsuke-cli-overhaul.out`.
- Commit each approved, gated change as a small atomic commit. Do not commit a
  draft that has not passed its relevant gates.
- Do not use Firecrawl or a wyvern agent unless a specific source gap appears.
  The user supplied the controlling blog-post content and final decisions in
  the prompt, so external retrieval is not required for this draft.

## Tolerances

- Scope: if the implementation of the documentation overhaul requires changing
  more than 14 documentation files in one commit, split the work into smaller
  approved commits.
- Roadmap churn: if preserving useful existing roadmap structure would require
  renumbering more than one existing phase, stop and propose alternatives
  before editing.
- Design ambiguity: if a planned command name has two plausible canonical forms
  and the choice affects future implementation, record both options and ask for
  direction instead of burying the choice in prose.
- Truthfulness: if current implementation behaviour cannot be described without
  making the docs sound contradictory, mark it as existing behaviour scheduled
  for removal and add an explicit roadmap task.
- Validation: if Markdown formatting or linting fails twice after focused
  fixes, stop and record the failing rule, file, and options in the Decision
  Log.
- File size: if any edited Markdown file grows by more than roughly 400 lines,
  split material into a dedicated design or ADR document and link to it.
- Diagrams: if Mermaid diagrams are added and `make nixie` fails because of the
  diagram syntax, fix the diagram or remove it rather than leaving validation
  optional.

## Risks

- Risk: the current implementation has already completed roadmap items for
  `--diag-json`, `--output-format`, `--colour-policy`, `--spinner-mode`,
  `manifest`, and `build --emit`. Severity: high. Likelihood: high. Mitigation:
  because Netsuke is pre-0.1.0, document these as legacy implementation state
  to delete rather than compatibility aliases to preserve.

- Risk: "human-first" can be misunderstood as permission to keep charming but
  inconsistent vocabulary. Severity: medium. Likelihood: medium. Mitigation:
  state the doctrine repeatedly: friendly presentation, canonical command
  structure. Human prose may be warm; command names, flags, schemas, and stream
  contracts must remain predictable.

- Risk: JSON result mode for `build` conflicts with arbitrary subprocess
  stdout. Severity: high. Likelihood: high. Mitigation: require JSON mode to
  capture bounded subprocess output and write overflow logs to referenced
  files. JSON mode must never stream compiler text into `stdout` beside the
  result document.

- Risk: the term `jobs` conflicts with build parallelism through `-j/--jobs`.
  Severity: medium. Likelihood: high. Mitigation: use `runs list|get|prune` for
  historical Netsuke invocations, while reserving `--jobs` for parallelism.

- Risk: introducing `context`, profiles, run ledgers, delivery, and feedback in
  one implementation slice would be too large to review. Severity: high.
  Likelihood: high. Mitigation: the roadmap and follow-on ExecPlans split the
  work into table stakes first, then compounding features.

- Risk: Netsuke could accidentally reimplement generic OrthoConfig machinery
  while planning its own UI redesign. Severity: high. Likelihood: medium.
  Mitigation: cite OrthoConfig roadmap tasks directly and distinguish hard
  dependencies, soft dependencies, and Netsuke-owned application semantics.

- Risk: a roadmap rewrite could lose useful Netsuke history or planned work
  that still matters after the UI redesign. Severity: high. Likelihood:
  medium. Mitigation: preserve completed work in an archive, keep still-valid
  planned work live, and record relevance decisions rather than deleting by
  default.

- Risk: updating user-facing docs before implementation could make planned
  commands appear shipped. Severity: medium. Likelihood: high. Mitigation: use
  explicit "planned surface" wording until the relevant implementation tasks
  land.

## Progress

- [x] 2026-05-09: Read `AGENTS.md`, loaded the `execplans`, `roadmap-doc`,
      `en-gb-oxendict-style`, and `leta` skill guidance relevant to this
      planning task.
- [x] 2026-05-09: Confirmed the current branch is
      `feat/netsuke-cli-overhaul`, making this plan path
      `docs/execplans/netsuke-cli-overhaul.md`.
- [x] 2026-05-09: Reviewed the existing roadmap, core design document, CLI
      design document, documentation style guide, contents index, README, user
      guide, and neighbouring ExecPlans.
- [x] 2026-05-09: Registered the workspace with `leta` and inspected the source
      tree. Focused symbol search dropped its connection, so direct reads of
      the small CLI source files were used for the current command and flag
      inventory.
- [x] 2026-05-09: Identified the current implementation surfaces that the
      overhaul must delete or replace: `--file`, `--diag-json`,
      `--output-format`, `--colour-policy`, `--spinner-mode`, boolean
      `--progress`, `--no-emoji`, `--accessible`, `build --emit`, `manifest`,
      and `NETSUKE_CONFIG_PATH`.
- [x] 2026-05-09: Drafted this ExecPlan.
- [x] 2026-05-09: Ran `make fmt`; it formatted this plan but failed on
      pre-existing Markdown lint issues in unrelated documents.
- [x] 2026-05-09: Reverted formatter-only churn in unrelated existing docs and
      kept the new plan file as the only worktree change.
- [x] 2026-05-09: Ran targeted Markdown lint on this plan with
      `markdownlint-cli2 docs/execplans/netsuke-cli-overhaul.md`; it passed
      with zero errors.
- [x] 2026-05-09: Ran `make nixie`; all Mermaid diagrams in the repository
      validated successfully.
- [x] 2026-05-11: Fetched and reviewed the active OrthoConfig roadmap from
      `https://raw.githubusercontent.com/leynos/ortho-config/refs/heads/main/docs/roadmap.md`.
- [x] 2026-05-11: Updated this plan to require explicit OrthoConfig roadmap
      dependencies, prevent duplicated shared configuration work, and preserve
      existing Netsuke roadmap history unless an item is no longer relevant
      after the UI redesign.
- [x] 2026-05-14: Reviewed the active Netsuke roadmap, contents index,
      design-doc anchors, ADRs, and this ExecPlan for the roadmap-fidelity
      guidance.
- [x] 2026-05-14: Confirmed there was no existing roadmap archive and created
      `docs/archive/roadmap-completed-foundations.md` for completed historical
      foundations.
- [x] 2026-05-14: Rewrote `docs/roadmap.md` so completed work is archived,
      partial and planned work remains live, OrthoConfig dependencies are
      explicit, and new CLI work starts at `3.15` and Phase 5.
- [x] 2026-05-14: Updated `docs/contents.md` to expose the roadmap archive.
- [x] 2026-05-14: Restored detailed Phase 4 formal-verification tasks after
      noticing the first rewrite compressed the planned verification workload
      too aggressively.
- [x] 2026-05-14: Restored detailed `3.14` conditional-action tasks after
      noticing the first rewrite compressed manifest dependency, executable
      probing, recipe environment, and structured `exec` work into broader
      conditional-semantics tasks.
- [x] 2026-05-14: Replaced the archive paraphrase with the exact completed
      task text from the previous roadmap so historical implementation
      obligations remain reviewable.
- [x] 2026-05-14: Validated the roadmap-fidelity revision with targeted
      Markdown lint, full `make markdownlint`, `make nixie`, `make check-fmt`,
      and `git diff --check`.
- [x] 2026-05-14: Re-ran `make fmt`; it still fails inside `mdformat-all`
      because that helper invokes a different `markdownlint` binary that
      reports pre-existing repository-wide line-length and table findings.
      Restored unrelated formatter churn and kept the passing project gates
      recorded above.
- [x] Commit the roadmap-fidelity revision.
- [x] Push the roadmap-fidelity revision.
- [ ] Stage A: create the governing design record.
- [ ] Stage B: update the core design document.
- [ ] Stage C: rewrite the CLI design document around the contract.
- [x] Stage D: rewrite the roadmap.
- [ ] Stage E: update user-facing documentation.
- [ ] Stage F: add follow-on ExecPlans for implementation slices.

## Surprises & discoveries

- Observation: roadmap number `3.14` is already used for conditional action
  planning, including dependency semantics, `command_available`, recipe
  ergonomics, and structured `exec` recipes. Evidence: `docs/roadmap.md`
  contains `### 3.14. Conditional action planning`. Impact: the CLI overhaul
  must use `3.15` or a new phase rather than the earlier conversation's
  suggested `3.14`.

- Observation: the current CLI is small but already has multiple legacy-shaped
  automation knobs. Evidence: `src/cli/mod.rs` defines `--diag-json`,
  `--output-format`, `--colour-policy`, `--spinner-mode`, `--progress`,
  `--no-emoji`, `--accessible`, `build --emit`, and `manifest`. Impact: the
  documentation overhaul must be explicit that these names are not
  compatibility surfaces; they are pre-0.1.0 scaffolding scheduled for removal.

- Observation: existing docs already contain strong accessibility,
  localization, diagnostics, and stream-separation material. Evidence:
  `docs/netsuke-cli-design-document.md` has extensive sections on Fluent,
  Section 508 accessibility, progress, stream separation, and miette
  diagnostics. Impact: the overhaul should not delete that work. It should
  reorganize it beneath the command contract so accessible human output and
  machine-stable structure reinforce each other.

- Observation: the README still documents `netsuke manifest FILE` and
  `--file`. Evidence: the README CLI block lists `netsuke manifest FILE` and
  the `--file` option. Impact: user-facing docs need a planned-surface update
  after the design and roadmap are aligned.

- Observation: repo-wide Markdown formatting currently fails on existing
  documents unrelated to this plan. Evidence: `make fmt` reached
  `markdownlint --fix` and reported many MD013 line-length violations plus
  existing table issues in files such as
  `docs/adr-002-replace-cucumber-with-rstest-bdd.md`,
  `docs/behavioural-testing-in-rust-with-cucumber.md`,
  `docs/developers-guide.md`, and `docs/netsuke-design.md`. Impact: this draft
  can be checked with targeted Markdown lint and `make nixie`, but a full gated
  commit requires either fixing those pre-existing documentation lint issues or
  agreeing that the gate is out of scope for this plan-only change.

- Observation: the active OrthoConfig roadmap already plans reusable
  agent-native contracts for downstream consumers including Netsuke. Evidence:
  OrthoConfig roadmap tasks `5.2.3`, `6.1`, `6.2`, `6.3`, `7.1`, `7.2`, `8.1`,
  `9.1`, `9.2`, and `9.3` cover consumer boundaries, recursive command
  metadata, agent-context output, skill manifest validation, vocabulary policy,
  behavioural semantics, JSON command results, profile contracts, delivery and
  feedback contracts, and execution ledger contracts. Impact: Netsuke's revised
  roadmap must depend on those tasks instead of planning independent generic
  implementations.

- Observation: the repository did not have an archived roadmap file before the
  fidelity review. Evidence: searching `docs/` found the active roadmap but no
  `docs/archive/` roadmap material. Impact: completed historical work needed a
  new archive rather than being compressed or deleted.

- Observation: roadmap item `3.14.2` was marked complete while one of its
  subtasks was still unchecked. Evidence: the previous `docs/roadmap.md` marked
  the heading as `[x]` and still included an unchecked complementary branch
  subtask. Impact: `3.14.2` stays in the active roadmap and is marked
  incomplete until every subtask is done.

- Observation: the first archive draft preserved task identities but softened
  implementation-level wording. Evidence: completed tasks were summarized as
  broad foundations rather than copied from the previous roadmap. Impact: the
  archive now keeps exact completed task text and uses phase-level relevance
  assessments to avoid erasing historical implementation risk.

## Decision log

- Decision: use `context` rather than `agent-context` as the planned command
  name. Rationale: `context` is friendlier for humans, still discoverable for
  agents, and avoids creating a separate "robot annex" in the command surface.
  The JSON payload can still identify itself as `netsuke.agent_context`.
  Date/Author: 2026-05-09 / planning agent.

- Decision: use `runs list|get|prune` rather than `jobs list|get|prune`.
  Rationale: Netsuke already needs `-j/--jobs` for parallel build jobs. `runs`
  accurately names historical Netsuke invocations and avoids a noun collision.
  Date/Author: 2026-05-09 / planning agent.

- Decision: remove legacy spellings instead of documenting aliases.
  Rationale: the user explicitly stated that Netsuke is pre-0.1.0 and has zero
  backwards-compatibility expectation. Carrying aliases would preserve the
  inconsistency the overhaul is meant to eliminate. Date/Author: 2026-05-09 /
  planning agent.

- Decision: keep the bare `netsuke` default as `build`.
  Rationale: this is a friendly human-first affordance and does not harm agent
  usability because agents can still call `netsuke build` explicitly.
  Date/Author: 2026-05-09 / planning agent.

- Decision: treat `--json` as full result mode, not diagnostics-only mode.
  Rationale: diagnostics-only JSON helps failures, but agents also need stable
  success documents. The JSON-mode stream contract must therefore cover both
  success and failure. Date/Author: 2026-05-09 / planning agent.

- Decision: commit and open a draft PR for this pre-implementation plan after
  the user explicitly requested PR creation. Rationale: the PR carries the
  draft for review, not permission to implement it. This plan passes targeted
  Markdown lint and Mermaid validation, but `make fmt` currently fails on
  unrelated existing Markdown issues, so the PR must disclose that gate
  limitation. Date/Author: 2026-05-09 / planning agent.

- Decision: make OrthoConfig the owner of reusable command-contract machinery
  and make Netsuke the owner of build-domain semantics. Rationale: the
  OrthoConfig roadmap explicitly says it should absorb reusable contracts
  before Weaver and Netsuke fossilize divergent local schemas. Netsuke should
  depend on that work for generic metadata, policy, and parser contracts while
  still owning manifest, build graph, Ninja, run-record, and artefact
  behaviour. Date/Author: 2026-05-11 / planning agent.

- Decision: move only fully completed historical roadmap tasks to
  `docs/archive/roadmap-completed-foundations.md`. Rationale: this preserves
  implementation fidelity while keeping the active roadmap focused on
  unfinished hypotheses. Partial work, including `3.14.2`, remains live under
  its original number. Date/Author: 2026-05-14 / planning agent.

- Decision: keep archived task numbers unchanged and avoid repeating them in
  the active roadmap. Rationale: globally unique task identifiers across
  current and archived roadmap files let reviewers trace historical work
  without ambiguity. Date/Author: 2026-05-14 / planning agent.

## Context and orientation

The repository is a Rust build-system compiler. Users write a YAML
`Netsukefile` with Jinja expressions; Netsuke compiles it into a static Ninja
build file and invokes Ninja. The primary design document is
`docs/netsuke-design.md`. The CLI companion document is
`docs/netsuke-cli-design-document.md`. The forward work tracker is
`docs/roadmap.md`. The user-facing references that will need follow-up are
`README.md`, `docs/users-guide.md`, `docs/quickstart.md`, `docs/contents.md`,
and `docs/ortho-config-users-guide.md`.

Current code defines the CLI in `src/cli/mod.rs`. The top-level commands are
`build`, `clean`, `graph`, and `manifest`; absent a subcommand, `Cli` defaults
to `build`. `BuildArgs` contains `emit`, which writes a generated Ninja
manifest as part of `build`. Configuration enums in `src/cli/config.rs` include
`ColourPolicy`, `SpinnerMode`, and `OutputFormat`. Output preference and
accessibility behaviour lives in `src/output_prefs.rs`, `src/output_mode.rs`,
`src/theme.rs`, and `src/status.rs`.

The controlling product conversation adopts a replacement doctrine for the "10
Principles for Agent-Native CLIs" blog post:

- no prompts or hidden interaction by default,
- one canonical structured mode, `--json`,
- errors that enumerate valid values,
- explicit mutation boundaries through `--dry-run` and `--force`,
- bounded output and truncation hints,
- community-consistent vocabulary,
- three-layer introspection through help, `context`, and a skill manifest,
- durable run history rather than ad hoc polling state,
- named profiles,
- structured delivery and feedback.

The active OrthoConfig roadmap at
`https://raw.githubusercontent.com/leynos/ortho-config/refs/heads/main/docs/roadmap.md`
 is an additional planning input. It states that Weaver and Netsuke are the
first downstream consumers for the expanded agent-native contract, and that
OrthoConfig should absorb reusable contracts before downstream applications
fossilize divergent local schemas. That roadmap makes the following dependency
boundaries explicit:

- OrthoConfig task `5.2.3` records consumer boundaries. Netsuke depends on it
  for the split between reusable command-contract machinery and application
  build execution.
- OrthoConfig tasks `6.1.1` and `6.1.2` own recursive command metadata.
  Netsuke should not build an independent generic command-tree metadata model.
- OrthoConfig tasks `6.2.1`, `6.2.2`, and `6.2.3` own agent-context generation,
  schema versioning, and downstream `<tool> context --json` naming. Netsuke's
  `context` command should consume or conform to that contract.
- OrthoConfig tasks `6.3.1` and `6.3.2` own reusable skill-manifest metadata
  and validation against real commands. Netsuke owns the build-workflow skill
  prose.
- OrthoConfig tasks `7.1.1`, `7.1.2`, and `7.1.3` own opt-in agent-native
  vocabulary policy and canonical global option glossary. Netsuke should
  configure and consume those checks rather than creating a separate policy
  engine.
- OrthoConfig tasks `7.2.1` through `7.2.7` own reusable behavioural metadata,
  including non-interactive execution, mutation boundaries, dual renderers,
  structured output, JSON stream contracts, exit-code classes, bounded list
  output, and capability provenance.
- OrthoConfig tasks `8.1.1` and `8.1.2` make `cargo-orthohelp` the reference
  CLI for JSON command results and enumerable choice errors. Netsuke should
  cite that reference contract when designing its own JSON result mode.
- OrthoConfig tasks `9.1.1` through `9.1.3` own reusable profile metadata,
  redaction rules, and the decision about a shared profile store helper.
  Netsuke owns build-specific profile values.
- OrthoConfig tasks `9.2.1` and `9.2.2` own reusable delivery target parsing
  and feedback storage contracts. Netsuke owns build artefact payload semantics.
- OrthoConfig tasks `9.3.1` through `9.3.3` own execution ledger metadata,
  configurable ledger nouns, and the decision about a reusable ledger helper.
  Netsuke may expose the noun `runs`, but it should share the metadata model.

Because Netsuke is a build tool rather than a hosted media API, the source
principles must be translated into build-tool concepts. `runs` replaces `jobs`;
local `build` still waits by default; `--wait` is reserved for future remote or
asynchronous backends; and `--deliver` initially applies only to Netsuke-owned
artefacts such as generated manifests, graph exports, JSON summaries, and
reports.

The revised Netsuke design and roadmap must therefore avoid duplicated
OrthoConfig scope. The correct shape is dependency-first: document the shared
contract dependency, use temporary Netsuke-local adapters only when explicitly
permitted, and keep every adapter easy to remove when the upstream OrthoConfig
task lands.

## Target product contract

The documentation should converge on this command surface:

```plaintext
netsuke [OPTIONS] [COMMAND]

Commands:
  build              Build targets
  check              Validate the manifest and generated build plan
  clean              Remove build artefacts with explicit mutation flags
  generate           Generate the Ninja manifest without building
  graph              Inspect the build graph
  context            Emit machine-readable CLI, manifest, and workspace context
  skill-path         Print the path to Netsuke's agent skill manifest
  runs               Inspect previous Netsuke invocations
  profile            Manage named configuration profiles
  feedback           Record local or upstream feedback for maintainers
```

Grouped commands should use resource verbs consistently:

```plaintext
netsuke runs list
netsuke runs get <run-id>
netsuke runs prune --force

netsuke profile save <name>
netsuke profile list
netsuke profile get <name>
netsuke profile delete <name> --force

netsuke feedback add <text>
netsuke feedback list
netsuke feedback send --force
```

The canonical global option set should be:

```plaintext
-f, --manifest <FILE>              Manifest path, default: Netsukefile
-C, --directory <DIR>              Run as if started in this directory
    --config <FILE>                Configuration file
-j, --jobs <N>                     Parallel build jobs
    --json                         Emit structured JSON result output
    --no-input                     Never prompt; fail fast instead
-q, --quiet                        Suppress non-essential human status output
-v, --verbose                      Emit detailed human diagnostics and timings
    --color auto|always|never      Colour policy
    --emoji auto|always|never      Emoji policy
    --progress auto|always|never   Progress rendering policy
    --accessibility auto|on|off    Accessible output mode
    --locale <LOCALE>              Localized CLI copy
    --profile <NAME>               Apply a named profile
```

The overhaul should delete and replace these current or planned names:

```plaintext
--file                 -> --manifest, keeping -f as an intentional shorthand
--diag-json            -> --json
--output-format        -> --json, with human output as the default
--colour-policy        -> --color auto|always|never
--spinner-mode         -> --progress auto|always|never
--progress true|false  -> --progress auto|always|never
--no-emoji             -> --emoji auto|always|never
--accessible true|false -> --accessibility auto|on|off
manifest <FILE>        -> generate --output <FILE>
build --emit <FILE>    -> generate --output <FILE>
NETSUKE_CONFIG_PATH    -> remove; keep NETSUKE_CONFIG
```

The JSON stream contract should be:

```plaintext
Human mode:
  stdout: primary artefacts or subprocess output
  stderr: Netsuke status, progress, warnings, and diagnostics

JSON mode success:
  stdout: exactly one JSON result document
  stderr: empty

JSON mode failure:
  stdout: empty unless an explicit artefact was already delivered
  stderr: exactly one JSON diagnostic document
```

For `build --json`, subprocess output must not be streamed through `stdout`.
Netsuke should capture a bounded preview in the result document and write
overflow logs under the run directory, for example
`.netsuke/runs/<run-id>/logs/stdout.log`.

## Plan of work

### Stage A: create the governing design record

Add `docs/adr-003-agent-consistent-human-first-cli.md`. The ADR should follow
the required sections in `docs/documentation-style-guide.md`: status, date,
context, decision drivers, options considered, outcome, risks, and related
documents. It should record that:

- Netsuke is pre-0.1.0 and will remove inconsistent legacy spellings rather
  than support aliases.
- The CLI is human-first in presentation and agent-consistent in structure.
- `--json` is the only structured result mode.
- JSON mode owns both success results and failure diagnostics.
- JSON mode forbids mixed subprocess output on `stdout`.
- Destructive operations require `--force`.
- Consequential operations provide `--dry-run`.
- `context`, `skill-path`, `runs`, `profile`, delivery, and feedback are
  planned product surfaces.
- Vocabulary consistency is enforced mechanically in CI.
- Shared command-contract features depend on the active OrthoConfig roadmap,
  especially tasks `5.2.3`, `6.1`, `6.2`, `6.3`, `7.1`, `7.2`, `8.1`, `9.1`,
  `9.2`, and `9.3`.
- Netsuke does not duplicate OrthoConfig-owned generic machinery. It records
  hard dependencies where work must wait, soft dependencies where temporary
  local adaptation is acceptable, and Netsuke-owned build semantics where
  OrthoConfig should not reach.
- Existing Netsuke roadmap tasks are preserved unless a relevance audit shows
  that the redesigned UI makes them obsolete.

Then update `docs/contents.md` to list the ADR after `adr-002`.

Stage A validation is `make fmt`, `make markdownlint`, and `make nixie`.

### Stage B: update the core design document

In `docs/netsuke-design.md`, add or replace a top-level section named
`Agent-consistent human-first CLI contract`. This section should become the
architectural source of truth for command behaviour, not an appendix. It should
define:

- the human-first, agent-consistent doctrine,
- the canonical command surface,
- the canonical global options,
- the banned vocabulary table,
- the JSON stream contract,
- the exit-code taxonomy,
- enumerable remediation rules for invalid values,
- mutation boundaries for `build`, `clean`, `generate`, `runs prune`,
  `profile delete`, and `feedback send`,
- bounded response requirements for `graph`, `context`, lists, logs, and
  diagnostics,
- the `context` schema shape,
- the explicit OrthoConfig dependency map for generic command metadata and
  policy work,
- the `runs` ledger purpose and storage outline,
- profile precedence,
- delivery schemes,
- feedback storage and send behaviour,
- CI linting and snapshot requirements.

Use this exit-code taxonomy unless implementation findings force a revision:

```plaintext
0 success
1 general runtime failure
2 CLI usage or configuration error
3 manifest parse, render, validation, or graph error
4 requested resource not found
5 external tool failure, including Ninja
6 network or delivery failure
7 interrupted or cancelled operation
```

Use this compact `context` envelope as the documented target:

```json
{
  "schema_version": 1,
  "kind": "netsuke.agent_context",
  "generator": {
    "name": "netsuke",
    "version": "0.1.0"
  },
  "commands": {},
  "global_flags": {},
  "exit_codes": {},
  "schemas": {
    "results": {},
    "diagnostics": {},
    "manifest": {},
    "configuration": {}
  },
  "stdlib": {
    "functions": {},
    "filters": {},
    "tests": {}
  },
  "available_profiles": [],
  "feedback": {
    "local": true,
    "upstream_configured": false
  }
}
```

Stage B validation is `make fmt`, `make markdownlint`, and `make nixie`.

### Stage C: rewrite the CLI design document around the contract

In `docs/netsuke-cli-design-document.md`, rewrite the introduction so it says
Netsuke presents a friendly human interface while treating every command as
potentially invoked by an agent, CI runner, editor integration, or shell
script. Then reorganize the document around these sections:

```plaintext
Human defaults
Non-interactive execution
Canonical command vocabulary
Output streams
JSON result mode
Diagnostics and exit codes
Mutation boundaries
Bounded output
Context and introspection
Profiles
Run ledger
Delivery
Feedback
Accessibility and localization
Progress and status display
Configuration and preference resolution
Validation and CI enforcement
```

Keep the existing accessibility, Fluent localization, progress, miette
diagnostics, and stream-separation material where still valid, but rewrite
examples to use canonical names:

- `--manifest`, not `--file`,
- `--json`, not `--diag-json` or `--output-format json`,
- `--color`, not `--colour-policy`,
- `--progress auto|always|never`, not `--spinner-mode` or boolean
  `--progress`,
- `--emoji auto|always|never`, not `--no-emoji`,
- `--accessibility auto|on|off`, not boolean `--accessible`,
- `generate --output`, not `manifest` or `build --emit`.

The `clean` section should state that bare `netsuke clean` fails fast with a
corrective hint unless `--dry-run` or `--force` is supplied.

Stage C validation is `make fmt`, `make markdownlint`, and `make nixie`.

### Stage D: rewrite the roadmap

This stage is complete for the current roadmap-fidelity revision. It changed
the planning model without deleting historical obligations:

- `docs/archive/roadmap-completed-foundations.md` now preserves completed
  roadmap items under their original numbers with relevance classifications.
- `docs/roadmap.md` now contains only unfinished and future work.
- Partial and planned tasks remain live under their original numbers, including
  `3.4.5`, `3.4.6`, `3.8.3`, `3.11.4`, `3.12.3`, `3.13.3`, `3.14.1`, and
  `3.14.2` through `3.14.11`.
- New CLI-foundation work starts at `3.15`; compounding features start at
  Phase 5.
- Every roadmap item that relies on reusable command/configuration/schema
  machinery cites the relevant OrthoConfig dependency.
- The active roadmap includes a canonical public vocabulary section so planned
  examples cannot drift away from the future CLI grammar.

Stage D validation is `make fmt`, targeted or full Markdown lint, `make nixie`,
and `git diff --check`.

### Stage E: update user-facing documentation

Update `README.md` after the design and roadmap agree. Replace the CLI block
with the planned command surface, clearly labelled as the pre-0.1.0 target if
the implementation has not landed. Remove examples that present `manifest`,
`--file`, or diagnostics-only JSON as the future interface.

Update `docs/users-guide.md` with an "Automation, agents, and CI" chapter or
section. It should include examples such as:

```sh
netsuke context
netsuke check --json --no-input
netsuke build --json --no-input
netsuke clean --dry-run --json --no-input
netsuke graph --json --target app --depth 2 --limit 50
netsuke --profile ci build --json --no-input
netsuke runs get run_01hv7x4q9n --json
```

Update `docs/ortho-config-users-guide.md` and `docs/sample-netsuke.toml` only
after deciding the exact configuration key names. The planned names should
match the command flags:

```toml
color = "auto"
emoji = "auto"
progress = "auto"
accessibility = "auto"
json = false
default_targets = ["app"]
```

Update `docs/quickstart.md` only for command names and examples that would
otherwise contradict the new surface.

Stage E validation is `make fmt`, `make markdownlint`, and `make nixie`.

### Stage F: add follow-on ExecPlans

Create follow-on implementation plans so the roadmap can be delivered in
reviewable slices. These plans should be draft-only until approved:

```plaintext
docs/execplans/3-15-1-canonical-command-surface.md
docs/execplans/3-15-2-json-result-mode.md
docs/execplans/3-15-3-output-policy-flags.md
docs/execplans/3-15-4-non-interactive-mutation-safety.md
docs/execplans/3-15-7-cli-vocabulary-lint.md
docs/execplans/5-1-1-context-schema-generation.md
docs/execplans/5-2-1-run-ledger.md
docs/execplans/5-3-1-named-profiles.md
docs/execplans/5-4-1-delivery-and-feedback.md
```

The `3-15-2-json-result-mode.md` plan must include a dedicated risk section for
subprocess stdout and stderr capture. That is the highest-risk implementation
detail in the command contract.

Stage F validation is `make fmt`, `make markdownlint`, and `make nixie`.

## Concrete steps

Run these commands from the repository root:

```sh
git branch --show-current
```

Expected output:

```plaintext
feat/netsuke-cli-overhaul
```

Inspect the current documentation and CLI surface:

```sh
rg --files docs
sed -n '1,260p' docs/roadmap.md
sed -n '1,260p' docs/netsuke-design.md
sed -n '1,260p' docs/netsuke-cli-design-document.md
sed -n '1,260p' README.md
sed -n '1,260p' docs/users-guide.md
sed -n '1,260p' src/cli/mod.rs
sed -n '1,220p' src/cli/config.rs
```

Inspect the active OrthoConfig roadmap before drafting revised Netsuke roadmap
tasks:

```sh
gh api -H Accept:application/vnd.github.raw \
  repos/leynos/ortho-config/contents/docs/roadmap.md
```

Expected output starts with:

```plaintext
# OrthoConfig roadmap
```

When implementation is approved, edit in this order:

1. Add the ADR and contents entry.
2. Update `docs/netsuke-design.md`.
3. Update `docs/netsuke-cli-design-document.md`.
4. Update `docs/roadmap.md`.
5. Update README and user-facing guides.
6. Add follow-on ExecPlans.

After each coherent commit-sized stage, run:

```sh
make fmt 2>&1 | tee /tmp/fmt-netsuke-feat-netsuke-cli-overhaul.out
make markdownlint 2>&1 | tee /tmp/markdownlint-netsuke-feat-netsuke-cli-overhaul.out
make nixie 2>&1 | tee /tmp/nixie-netsuke-feat-netsuke-cli-overhaul.out
```

If a stage touches Rust source or generated Rust-facing help snapshots, also
run:

```sh
make check-fmt 2>&1 | tee /tmp/check-fmt-netsuke-feat-netsuke-cli-overhaul.out
make lint 2>&1 | tee /tmp/lint-netsuke-feat-netsuke-cli-overhaul.out
make test 2>&1 | tee /tmp/test-netsuke-feat-netsuke-cli-overhaul.out
```

Commit using the repository's commit-message rules after validation passes. Use
a file-based commit message, not `git commit -m`.

## Validation and acceptance

The documentation overhaul is accepted when:

- `docs/adr-003-agent-consistent-human-first-cli.md` exists and records the
  pre-0.1.0 no-compatibility decision.
- `docs/netsuke-design.md` and `docs/netsuke-cli-design-document.md` use the
  same canonical command names, flags, stream contract, and JSON doctrine.
- `docs/roadmap.md` adds `3.15` for table-stakes foundations and Phase 5 for
  compounding features without colliding with the existing `3.14`.
- `docs/archive/roadmap-completed-foundations.md` preserves completed
  historical work under original numbers with relevance classifications.
- Every revised Netsuke roadmap item that relies on reusable command-contract
  machinery cites the relevant OrthoConfig roadmap dependency.
- The revised design states that OrthoConfig owns generic command metadata,
  vocabulary policy, agent-context schema machinery, profile redaction,
  delivery/feedback parsers, and ledger metadata, while Netsuke owns
  build-domain semantics.
- Existing Netsuke roadmap work is preserved unless a recorded relevance audit
  shows that the redesigned UI makes the item obsolete.
- Existing completed roadmap items that conflict with the new doctrine are
  archived as foundations and the replacement work is expressed as new live
  roadmap tasks.
- User-facing docs distinguish planned behaviour from shipped behaviour.
- Follow-on ExecPlans exist for the major implementation slices.
- `make fmt`, `make markdownlint`, and `make nixie` pass for documentation-only
  edits.
- If implementation source changes are included in a later approved stage,
  `make check-fmt`, `make lint`, and `make test` also pass.

Expected successful validation transcript shape:

```plaintext
$ make markdownlint
markdownlint-cli2 ...

$ make nixie
...
```

The exact validator output may vary, but each command must exit with status 0.

## Idempotence and recovery

Documentation edits are safe to repeat if each stage is kept focused. If a
stage fails formatting, run `make fmt` again, inspect `git diff`, and rerun the
validators. If a roadmap edit becomes too large, split it into a
design-doctrine commit and a roadmap-structure commit. If user-facing docs
accidentally imply a planned command is already shipped, correct the wording
before committing.

If a future implementation step starts changing Rust source while this plan is
still documentation-only, stop and ask for approval. This plan intentionally
separates planning from execution.

## Interfaces and dependencies

The documentation plan should constrain future implementation to these stable
interfaces:

- CLI parser: `src/cli/mod.rs` remains the authority for Clap command and flag
  definitions.
- Configuration projection: `src/cli/config.rs` should eventually replace
  `ColourPolicy`, `SpinnerMode`, and `OutputFormat` with canonical policy types
  aligned to the new flags.
- Output mode and theme resolution: `src/output_mode.rs`,
  `src/output_prefs.rs`, and `src/theme.rs` should eventually resolve
  `--color`, `--emoji`, `--progress`, and `--accessibility`.
- JSON diagnostics: `src/diagnostic_json.rs` should become part of a broader
  JSON result/diagnostic contract rather than a diagnostics-only surface.
- Runner integration: `src/runner/mod.rs` and `src/runner/process/` will need
  explicit capture and log-reference behaviour for `build --json`.
- Roadmap and documentation: `docs/roadmap.md`, `docs/netsuke-design.md`,
  `docs/netsuke-cli-design-document.md`, `docs/users-guide.md`, `README.md`,
  and `docs/contents.md` are the main documentation interfaces.

No new external dependency is required for the documentation overhaul. Future
implementation may require HTTP client decisions for webhook delivery and
feedback sending; those decisions belong in the relevant Phase 5 ExecPlan and
ADR if needed.

## Outcomes & retrospective

This section will be completed after the approved documentation overhaul is
implemented and validated.

## Revision note

- 2026-05-09: Initial draft created from the supplied agent-native CLI
  conversation, current repository documentation, and current CLI/configuration
  source inventory. This draft defines the target doctrine and staged
  documentation-overhaul work, but does not execute the overhaul.
- 2026-05-09: Added validation evidence after running the formatter, targeted
  Markdown lint, and Mermaid validation. The plan records that full repo
  formatting is blocked by unrelated existing Markdown lint issues.
- 2026-05-11: Added the active OrthoConfig roadmap as a planning input and
  made the Netsuke design/roadmap overhaul dependency-first. The revision
  requires explicit OrthoConfig task dependencies, avoids duplicated shared
  command-contract work, and preserves existing Netsuke roadmap items unless a
  relevance audit proves they no longer matter after the UI redesign.
- 2026-05-14: Added the completed-foundations roadmap archive, rewrote the
  active roadmap around unfinished work and future hypotheses, retained partial
  work under its original numbers, and made OrthoConfig dependencies explicit
  in the live roadmap.
