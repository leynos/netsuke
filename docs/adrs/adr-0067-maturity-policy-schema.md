# Architectural decision record (ADR) 0067: Maturity policy schema

## Status

Proposed. This decision remains pending review and explicit acceptance.

## Date

2026-10-24

## Context and problem statement

RFC 0025 proposes opt-in declaration-coverage policies that strengthen a
project without making advanced features compulsory. A policy must select
explicit graph nodes, produce useful findings, and compose with trusted
operator requirements. It must also reuse the semantic linter's resolved
inventory instead of adding another parser or guessing from command text.

## Decision Drivers

- Preserve a silent shallow end with no default annotation warnings.
- Make policy scope explicit and non-contagious.
- Keep policy rule identifiers distinct from feature identifiers.
- Allow project policy to strengthen, but never weaken, trusted operator
  requirements.
- Reuse existing lint, localization, metadata, and reporting surfaces.

## Requirements

### Functional requirements

- Add optional root `maturity.rules`, empty by default.
- Require each rule to name an initial registered rule ID, severity, and
  explicit selector list.
- Support optional rule-specific `subjects` only where the rule contract needs
  them.
- Fail on unknown, duplicate, or unsupported rules instead of silently
  skipping them.

### Technical requirements

- Use a closed initial severity vocabulary `off`, `warn`, and `error`.
- Evaluate resolved declarations and provenance through the shared semantic
  inventory.
- Merge policy severity monotonically with the trusted operator floor.
- Do not infer scope from glob patterns or command-text regular expressions.

## Options considered

### Option A: Explicit rules over the semantic linter inventory

Authors select exact actions or targets and request narrow checks over resolved
declarations. The policy remains opt-in, and reporting uses the existing
`netsuke check` surfaces.

### Option B: Require maturity levels globally

Mandatory levels would make unrelated features contagious and increase the
minimum vocabulary needed for a simple build.

### Option C: Add a global strict mode with an evolving implicit rule list

An implicit list could make unchanged manifests fail after an upgrade and would
hide which contract a project chose to enforce.

## Decision outcome / proposed direction

The root `maturity` key contains a `rules` list. Each rule has `id`,
`severity`, and an explicit `select` list of action/target identities.
Rule-specific `subjects` refine the check only where required. The default is
an empty list; absent maturity policy produces no message at any verbosity.
Patterns, wildcard selectors, and implicit scope expansion are not part of the
initial schema.

Feature identifiers and rule identifiers are separate namespaces. The initial
rule-to-feature table is:

| Rule identifier       | Required feature      |
| --------------------- | --------------------- |
| `structured-commands` | `structured-commands` |
| `typed-inputs`        | `inputs`              |
| `owned-cleanup`       | `artefacts`           |
| `verified-states`     | `states`              |
| `contention-declared` | `contention`          |

_Table 1: Feature capability required by each initial maturity rule._

Severity is ordered `off < warn < error`. Effective severity combines
monotonically: project policy may strengthen a trusted floor but cannot weaken
it. Unknown rules, duplicate rule/subject combinations, unknown selectors,
unsupported severities, or rules requiring unavailable features fail with an
actionable version or capability remedy.

The initial rule meanings are deliberately narrow. `structured-commands`
requires structured recipe units; `typed-inputs` applies only to explicitly
named input subjects; `owned-cleanup` requires `clean_owned` for executable
deletion units; `verified-states` checks declared state operations before the
first non-state unit; and `contention-declared` requires a valid class on each
selected executable edge. These checks establish declaration coverage, not
hermeticity, reproducibility, correct ownership, or safe execution of untrusted
code.

Project policy lives at root `maturity.rules`. The operator-floor configuration
shape belongs to roadmap item 25.2.1. Evaluate policy through the typed
semantic inventory owned by ADR-0056 and roadmap item 25.1.1. Report findings
through `netsuke check`; selected-closure inspection metadata belongs to
25.2.2. Inspection and dry-run check declaration evidence and do not execute
state probes to satisfy a rule.

## Goals and non-goals

- Goals:
  - Let a project enforce one useful contract at a time.
  - Keep policy scope and remedies visible to human and JSON consumers.
  - Preserve ordinary validation independently of rule severity.
- Non-goals:
  - Certify safety, hermeticity, or correctness from declaration presence.
  - Add mandatory maturity levels, a global strict mode, or implicit rules.
  - Add exceptions, presets, patterns, or context-coverage rules to the initial
    vocabulary.

## Migration plan

1. Roadmap item 25.1.1 establishes the reusable semantic inventory boundary
   with the linter owner.
2. Item 25.1.2 adds each initial rule only after its required feature exists.
3. Item 25.2.1 defines the trusted operator floor and 25.2.2 provides selected
   closure inspection metadata.
4. Keep report-only adoption ahead of error enforcement and measure onboarding
   separately from feature migration.

## Known risks and limitations

- A narrow rule checks declared evidence, not arbitrary behaviour inside a
  shell command or external program.
- Policy may be syntactically valid while an installed reader lacks its
  required feature; this must fail with a remedy.
- An over-broad selected closure could make unrelated actions strict, so the
  implementation must preserve exact selectors and invocation scope.

## Outstanding decisions

- Roadmap item 25.2.1 owns operator policy placement and floor configuration;
  merge an ADR amendment before its code merges if those rules change.
- Roadmap item 25.2.2 owns selected-closure inspection metadata; merge an ADR
  amendment before its code merges if scope or provenance changes.
- Roadmap item 25.1.1 owns reuse of the typed inventory; merge an ADR amendment
  before its code merges if that ownership boundary changes.
- Rule-specific exceptions, presets, patterns, and context coverage remain
  outside this feature and need a separate accepted proposal before
  implementation.
