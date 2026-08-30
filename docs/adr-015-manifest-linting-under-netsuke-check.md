# Architectural decision record (ADR) 015: Host manifest linting under `netsuke check`

## Status

Accepted. `netsuke check` is the manifest linter: findings are the command's
result rather than its failure mode, rule identifiers are stable kebab-case
names owned by a static registry, and rule text stays in that registry instead
of the Fluent catalogues.

## Date

2026-08-30

## Context and problem statement

Netsuke rejects manifests it can prove wrong and accepts everything else. The
band between "parses and lowers" and "provably wrong" contains the mistakes
that cost the most: recipes that defeat change detection, dependency shapes
that race under parallel builds, shell constructs that fail on a host whose
`/bin/sh` is not `bash`, and stale workarounds such as the pre-v0.1.0 `$$PATH`
spelling that [ADR-014](adr-014-backend-text-escaping-seam.md) turned from a
fix into a bug.

Adding an analysis for that band forces four decisions that outlive the
implementation, and that a reader of the code alone would have to reconstruct:

1. Where the analysis lives in a command vocabulary that deliberately refuses
   compatibility aliases and unjustified nouns.
2. Whether a lint finding is a command result or a command failure, given that
   the JSON envelope pairs "success" with a result document on stdout and
   "failure" with a diagnostic document on stderr.
3. What identifies a rule, given that the identifier is simultaneously a
   configuration key, a suppression token, a documentation anchor, and a field
   in machine output, and therefore cannot change.
4. Whether rule text is localized, given that a build-script audit fails the
   build when any declared Fluent key is missing from any of the 35 shipped
   catalogues.

## Decision drivers

- [ADR-003](adr-003-agent-consistent-human-first-cli.md) removes legacy
  spellings rather than aliasing them, makes `--json` the only structured
  result mode, and requires that agents and humans share one vocabulary.
- Roadmap task 3.15.1 already lists `check` in the canonical command vocabulary
  as unbuilt work, so the noun exists in the grammar but not in the binary.
- The `--json` envelope invariant — one document per invocation, result on
  stdout for success, diagnostics on stderr for failure — is documented in the
  users' guide and relied on by CI consumers.
- The localization audit in `build_l10n_audit` is a hard build failure, so every
  localized string is a standing 35-way obligation.
- A rule set is expected to grow. Whatever is per-rule work must stay small.

## Options considered

### Command placement

#### Option A: a new `netsuke lint` command

A dedicated noun, matching what most linters call themselves.

#### Option B: `netsuke check`

The noun the roadmap already reserved for validation without side effects.

#### Option C: a flag on `generate`, such as `netsuke generate --lint`

No new noun at all; linting rides the command that already builds the graph.

Table: command placement options

| Dimension                        | A: `lint`                          | B: `check`                                  | C: `generate --lint`                                           |
| -------------------------------- | ---------------------------------- | ------------------------------------------- | -------------------------------------------------------------- |
| New top-level noun               | Yes, unbudgeted                    | No, already in the vocabulary               | No                                                             |
| Discoverability                  | High                               | High                                        | Low; hidden behind an unrelated verb                           |
| Room to grow                     | Linting only                       | Linting plus future non-mutating validation | Constrained by `generate`'s contract                           |
| Output contract                  | Free                               | Free                                        | Must share `generate`'s result shape, which carries Ninja text |
| Conflicts with vocabulary policy | Adds a synonym for a reserved noun | None                                        | None                                                           |

_Table 1: Trade-offs between command placements for the manifest linter._

Option A was rejected because `check` and `lint` would be synonyms in the same
vocabulary, which is exactly the inconsistency ADR-003 exists to prevent.
Option C was rejected because `generate`'s result document carries the
generated Ninja text, so lint findings would have to be smuggled alongside an
artefact, and because a user who wants validation should not have to ask for
code generation.

### Findings as result or as failure

#### Option A: findings are always failures

Every finding becomes a diagnostic; the command fails whenever anything is
reported.

#### Option B: findings are always results

The command always succeeds when it could analyse the manifest; CI compares
counts itself.

#### Option C: findings are data, and a threshold decides the branch

Findings are carried in whichever branch the threshold selects, using one
per-finding representation in both.

Option A discards severity: an advisory becomes indistinguishable from a
defect, and no manifest with an advisory could ever pass CI. Option B makes the
common case — "fail my build on lint errors" — require every consumer to
reimplement threshold logic, and puts a non-zero exit out of reach of `make`.
Option C was chosen.

### Rule identity

#### Option A: numeric codes, as `hadolint` and ShellCheck use

Compact and stable, but opaque: a reader must look up `NSK014` to learn
anything.

#### Option B: category-prefixed codes, as Ruff uses

Self-grouping, but the category becomes part of the identifier, so
recategorizing a rule breaks every configuration file and suppression comment
that named it.

#### Option C: flat kebab-case names, as `buildifier` and Clippy use

Self-describing and greppable, with category carried as separate metadata.

## Decision outcome

**Placement.** Manifest linting is `netsuke check`. It adds no top-level noun
and four flags: `--rule`, `--fail-on`, `--limit`, and `--explain`. Rule
documentation is `--explain`, a mode of `check`, rather than the `explain`
command the roadmap defers.

**Findings are data.** A finding is a typed record with a severity resolved
from policy. `--fail-on` sets the threshold at which findings fail the command,
defaulting to `error`. Below the threshold the command succeeds and writes a
result document to stdout whose `result.findings` array holds every finding. At
or above it the command fails and writes a diagnostic document to stderr whose
single top-level diagnostic is the threshold summary and whose `related` array
holds the same findings in the same order. Both arrays use the existing
diagnostic-entry shape, so a consumer parses one finding representation and
selects the array by presence. The envelope invariant is preserved unchanged.

**Identity.** A rule is identified by a stable, unique, kebab-case name, for
example `directory-dep-not-order-only`. Category is metadata, never part of the
name. Names are permanent: a retired rule keeps its name reserved, and a rule
whose meaning changes materially takes a new name. Each finding additionally
carries the diagnostic code `netsuke::lint::<name_in_snake_case>`, matching the
existing code convention, and a `url` into the rule reference.

**Rule text is not localized.** A rule's summary, rationale, and remediation
are English technical documentation owned by the static rule registry,
alongside the rule's name and category. The command's framing text — subcommand
and flag help, the threshold message, the summary line, and policy errors — is
localized as usual. This mirrors Clippy, ShellCheck, `hadolint`, `buildifier`,
and Ruff, none of which localizes lint text.

Three reasons carried this. First, the registry is the source of truth for the
rule reference document, which a contract test checks; splitting the same prose
across 35 catalogues would let the emitted text and the documentation drift
with no gate able to notice. Second, remediation text is dense with
untranslatable manifest identifiers — `order_only_deps`, `{{ outs }}`,
`dependency_order` — which the translators' guide already instructs translators
to leave verbatim, so the translatable residue is small. Third, and decisively,
localization would put a 35-way obligation on the critical path of every new
rule, and the property that keeps a rule set healthy is that adding a rule is
cheap.

## Known risks and limitations

- **Rule text is English-only in an otherwise localized CLI.** This is a real
  inconsistency, accepted deliberately. The reversal path is additive and
  mechanical: add `lint.rule.<name>.summary` and `.remediation` keys, have the
  registry look them up with the current text as the fallback, and translate
  incrementally. No identifier, schema, or suppression comment changes.
- **A threshold change moves findings between JSON branches.** A consumer that
  reads only `result.findings` sees nothing when the threshold is met. This is
  mitigated by both branches carrying the same per-finding shape, and
  documented in the users' guide as reading `result.findings` when present and
  `diagnostics[0].related` otherwise.
- **`check` may later host validation beyond linting.** The result document
  therefore names the command `check` and reports lint findings under
  `result.findings` rather than claiming the whole result shape for linting.
- **The exit code does not distinguish "findings met the threshold" from "the
  manifest could not be analysed".** Both exit `1`, because Netsuke has no
  exit-code taxonomy yet. Roadmap task 3.15.5 owns that separation; this ADR
  does not pre-empt it, and the `code` field already distinguishes the cases
  for machine consumers.

## Architectural rationale

The decision keeps three existing boundaries intact rather than widening any of
them. The command vocabulary gains no synonym. The JSON envelope gains no third
branch and no second finding shape. The localization audit gains no obligation
that scales with the rule set. What it does add — a stable identifier namespace
and a per-finding schema — is the minimum needed for configuration,
suppression, documentation, and editor integration to refer to the same thing,
which is the property that lets all four evolve independently.

The full rule model, stage hooks, suppression grammar, and output schemas are
specified in the [manifest linter design](netsuke-linter-design.md).
