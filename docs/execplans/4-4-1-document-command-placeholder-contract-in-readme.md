# Document the command placeholder contract in the README (roadmap 4.4.1)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: DRAFT

Revision 1. See `Revision note` at the foot of this document.

## Purpose / big picture

Netsuke compiles a YAML manifest into a Ninja build file. Somewhere in that
pipeline it decides which parts of a recipe an author wrote are Netsuke's to
rewrite, and which are opaque shell text handed to `/bin/sh`, `bash.exe`, or
Windows PowerShell untouched. That decision is a security boundary: it is the
last place Netsuke can reject a command whose shell syntax the substitution
damaged, and the only place Netsuke promises to quote a path.

Today that boundary is implemented, proved with Kani and Proptest, and
described in three internal documents — but a person evaluating Netsuke from
its README cannot find it. The README's only security prose is one paragraph
buried inside a release-status section. Roadmap item 4.4.1 exists to fix that,
and to settle three questions the design documents explicitly leave open.

After this change:

- A reader of `README.md` finds a first-class **Security and command
  interpolation** section that names every supported placeholder, states what
  Netsuke quotes and what it does not, states the exact backtick-handling
  boundary, and states the status of the `shlex::split` guard.
- The same section exists in all six translated READMEs, so the localized
  editions keep their present section-for-section parity with the English one.
- Two of the README's claims are executable: a manifest that Netsuke accepts,
  and a manifest that Netsuke rejects with a named diagnostic. Both run in the
  repository's documented-example harness, so the README cannot silently drift
  away from the implementation.
- A new Architectural Decision Record,
  `docs/adr-021-command-placeholder-contract.md`, records the three settled
  contract decisions and their rationale, and the design document points at it.
- Three documents that currently misstate the contract are corrected —
  `docs/developers-guide.md`, `docs/users-guide.md`, and
  `docs/formal-verification-methods-in-netsuke.md`. All three say, without
  qualifying it to command recipes, that literal `$in` and `$out` are left
  alone. In a `script:` recipe they are not.

You can see it working by running `NETSUKE_REQUIRE_NINJA=1 make test` and
observing the new cases
`readme_security_tests::placeholder_rewriting_differs_by_recipe_kind`,
`readme_security_tests::netsuke_owned_path_substitutions_are_quoted`, and
`readme_security_tests::documented_backtick_manifest_is_rejected` pass, and by
reading `README.md` and finding a section that agrees with what those tests
assert.

## Context and orientation

Assume no prior knowledge of this repository. The paragraphs below name every
file this plan reads or edits, with a repository-relative path.

### What Netsuke does with a recipe

A manifest (`Netsukefile`) is YAML with Jinja templating. A target or action
carries either a `command:` (a single shell command line) or a `script:` (a
multi-line shell script). Netsuke processes these in three stages:

1. **Manifest rendering.** `src/manifest/render.rs` runs MiniJinja over the
   recipe text. The manifest-author-facing markers `{{ ins }}` and `{{ outs }}`
   are rendered into two internal sentinel strings,
   `__NETSUKE_INS_PLACEHOLDER__` and `__NETSUKE_OUTS_PLACEHOLDER__`, referred
   to in code as `INS_TOKEN` and `OUTS_TOKEN`. Arbitrary other Jinja
   expressions are rendered to their values here and are thereafter
   indistinguishable from text the author typed.
2. **IR lowering.** `src/ir/from_manifest_support.rs` calls one of two entry
   points in `src/ir/cmd_interpolate/mod.rs`:
   `interpolate_command_with_bindings` for a `command:`, or
   `interpolate_script_with_bindings` for a `script:`. These replace the
   internal tokens with concrete, shell-quoted input and output paths, tracking
   the POSIX quoting context so a path lands correctly whether it appears
   unquoted, single-quoted, or double-quoted. Quoting uses the `shell-quote`
   crate (`Cargo.toml` line 137) in its `Sh` mode, not `shlex`.
3. **Ninja generation.** `src/ninja_gen_recipe_shell.rs` turns the fully
   interpolated text into the `command =` binding of a content-hashed Ninja
   rule. `src/ninja_gen_escape.rs` escapes `$` as `$$` so Ninja's own variable
   grammar does not consume a shell dollar. Netsuke never emits Ninja's native
   `$in` / `$out` rule variables; the paths are already baked into the text.

**Term of art — "marker".** In this plan, *marker* means a Netsuke-owned
placeholder that Netsuke rewrites: `{{ ins }}`, `{{ outs }}`, and (in scripts
only) `$in` and `$out`. *Internal token* means the `INS_TOKEN` /`OUTS_TOKEN`
sentinel strings that exist only between stages 1 and 2. *Shell variable* means
text such as `$PATH` or `$ins` that Netsuke deliberately leaves alone. These
three are distinct and the existing documentation conflates them; not
conflating them is a goal of this plan.

### The three code facts this plan documents

These were established by reading the implementation, not by trusting existing
prose. Cite them when writing.

**Fact A — the supported placeholder set differs by recipe kind.**
`find_substitution` (`src/ir/cmd_interpolate/mod.rs:261-266`) matches only
`INS_TOKEN` and `OUTS_TOKEN`, and is what `command:` recipes use.
`find_script_substitution` (`src/ir/cmd_interpolate/mod.rs:269-275`) also
matches `$in` and `$out` via `try_match_dollar_placeholder`
(`src/ir/cmd_interpolate/mod.rs:277-297`), which requires a non-alphanumeric,
non-underscore character on each side. So `$in` and `$out` **are** rewritten in
a `script:`, and **are not** rewritten in a `command:`. `$ins`, `$outs`,
`$input`, `$output`, and `$PATH` are never rewritten in either. Confirmed by
`src/ir/cmd_interpolate_tests.rs:42-50`
(`interpolate_command_preserves_dollar_prefixed_shell_variables`) and the
property test `dollar_prefixed_shell_variables_are_preserved`
(`src/ir/cmd_interpolate_property_tests.rs:55-66`).

**Fact B — backtick handling is two narrow checks, not a shell model.**

- During lowering, a marker found inside a backtick region or a `$( … )`
  command substitution is rejected, because substituting into a region the
  shell will re-evaluate cannot be made safe. See
  `SubstitutionTraversal::append_protected_character`
  (`src/ir/cmd_interpolate/substitution.rs:363-375`) and the script equivalent
  (`src/ir/cmd_interpolate/script_substitution.rs:221-243`).
- After substitution, and for `command:` recipes only,
  `is_valid_command_for_shell` (`src/ir/cmd_interpolate/mod.rs:225-231`)
  rejects the command when `has_unmatched_backticks`
  (`src/ir/cmd_interpolate/mod.rs:165-174`) reports an **odd total count of
  backtick characters in the whole string**. That is a parity count. It is not
  quoting-aware, so a backtick inside single quotes counts, and two balanced
  but unrelated backticks do not.
- PowerShell is exempt from both. `is_valid_command_for_shell` returns `true`
  unconditionally for `RecipeShell::PowerShell`
  (`src/ir/cmd_interpolate/mod.rs:226-228`), because PowerShell uses a backtick
  as its escape character rather than as command substitution. Confirmed by
  `power_shell_bindings_preserve_literal_backticks_in_paths`
  (`src/ir/cmd_interpolate_tests.rs:337-345`).
- Neither check sanitizes author-written shell text. A balanced backtick pair
  containing text the author typed reaches the shell and is executed as command
  substitution. The only exception is that `quote_double_quoted_path`
  (`src/ir/cmd_interpolate/mod.rs:153-163`) backslash-escapes a backtick inside
  a **Netsuke-substituted path** that lands in a double-quoted context.

**Fact C — `shlex::split` is a rejection gate on one route only.**
`is_valid_command_for_shell` calls `shlex::split(command).is_some()` on the
fully substituted text (`src/ir/cmd_interpolate/mod.rs:230`). Failure produces
`IrGenError::InvalidCommand`, surfaced through the Fluent message
`ir.invalid_command` — in `locales/en-GB/messages.ftl:179`,
`Invalid command interpolation: { $snippet }.` The gate applies to `command:`
recipes on the POSIX and Bash routes. It is **not** applied to `script:`
recipes (see the doc comment on `interpolate_script_with_bindings`,
`src/ir/cmd_interpolate/mod.rs:200-206`), and not on PowerShell. Netsuke never
uses the returned token vector to spawn anything; it only asks whether a split
was possible. A second, `debug_assert!`-only use exists in
`assert_shell_command` (`src/ninja_gen/mod.rs:283-290`); it is reached only for
`Recipe::Command`, because `script_shell_text` (`src/ninja_gen/mod.rs:343-356`)
is explicitly exempt.

The dependency is **not pinned**. `Cargo.toml:138` reads `shlex = "2.0.1"`,
which is a caret requirement admitting any `2.x`; `Cargo.lock:2638` records
today's resolution, and `README.md:110` tells users to install with
`cargo install --path .`, which ignores the lockfile. Two people building the
same Netsuke source can therefore get different acceptance sets. This matters to
`D3` and is why `AX-SHLEX` is stated as an assumption rather than a fact.

### Where the contract is currently written

- `docs/formal-verification-methods-in-netsuke.md:263-279`, section **Command
  placeholder contract**. This is the upstream requirement for 4.4.1. It asks
  the project to state three things and names the README section to add.
- `docs/formal-verification-methods-in-netsuke.md:62-83`, section **Kani for
  command interpolation**. Its footnote `[^8]`
  (`docs/formal-verification-methods-in-netsuke.md:332-333`) cites
  `src/ir/cmd_interpolate.rs`, a path that no longer exists; the module was
  split into `src/ir/cmd_interpolate/`.
- `docs/developers-guide.md:3186-3201`, section **Command interpolation
  contract**. States "Literal shell variables such as `$in`, `$out`, `$ins`, and
  `$outs` remain unchanged". True for a `command:`; false for a `script:`
  (Fact A).
- `docs/users-guide.md:1649-1718`, section **Review the safety boundary**.
  User-facing and largely accurate; the README will link to it rather than
  restate it.
- `docs/netsuke-design.md:290-298` and `docs/netsuke-design.md:2616-2644`, the
  canonical design text.
- `docs/adr-004-bound-kani-ir-harnesses-to-small-n.md:167-177`, which records
  that the Kani proofs cover only the marker recognizer, and that the backtick
  state machine and `shlex` guard are covered by Proptest rather than proof.
- `docs/adr-014-backend-text-escaping-seam.md:42-45`, the escaping seam.

### The README and its translations

`README.md` is 256 lines with twelve headings and no table of contents.
Sections are separated by an `mdtablefix`-canonical thematic break, a line of
seventy underscore characters. Six translated editions — `README.de.md`,
`README.es.md`, `README.fr.md`, `README.ja.md`, `README.pt-BR.md`,
`README.zh-CN.md` — mirror the English heading structure exactly, twelve
headings each, same order, same levels. A localization menu at the top of every
edition links to the other six.

No continuous-integration job checks translation parity. The obligation is a
convention recorded in `docs/repository-layout.md:60-65`. The translated
READMEs are excluded from the `typos` spelling gate by
`typos.local.toml:85-96`, so nothing mechanical will catch a mistranslation.
`docs/localization-glossary.md` holds the per-locale terminology tables, and
`docs/localization-styleguide.md` the tone rules.

### The documented-example harness — the most important local constraint

`tests/documentation_examples/mod.rs` parses `README.md`,
`docs/users-guide.md`, and `docs/stdlib-yaml-and-jinja-guide.md`. It enforces
that **every fenced code block in those three files is immediately preceded by
a marker comment** of the form `<!-- tested-example: some-id -->`, that the
fence declares a language, and that identifiers are unique
(`tests/documentation_examples/mod.rs:120-175`). Separately,
`tests/documentation_examples_tests.rs:18-61` holds `EXPECTED_EXAMPLE_IDS`, a
sorted list compared for **exact set equality** against the parsed identifiers.

The consequence is that adding a fenced code block to `README.md` is not a
documentation-only edit: it is a test change. This is what makes the plan's
red-green cycle real rather than ceremonial. The translated READMEs are not in
`DOCUMENT_PATHS` and already contain unmarked fences, so they need no markers
and no registry entries.

### Quality gates

Run from the repository root. All are Makefile targets.

- `make check-fmt` — `cargo fmt --check`, `ruff format --check`, and
  `scripts/check-markdown-format.sh` over every Markdown file matched by
  `MD_FILES_FIND` (`Makefile:126-134`). The Markdown half requires `mdtablefix`
  on `PATH` and enforces canonical wrapping and table markup.
- `make markdownlint` — depends on `spelling`, then runs `markdownlint-cli2`.
  `.markdownlint-cli2.jsonc` sets `MD013` line length 80 for prose, 120 inside
  code blocks, `MD004` dash bullets, `MD029` ordered-list style.
- `make typecheck` — `typecheck-python` then
  `cargo check --all-targets --all-features`.
- `make lint` — Clippy, the Whitaker Dylint suite, the Python lints, and the
  GitHub Actions lints.
- `make test` — `cargo nextest run --workspace --all-targets --all-features`
  followed by `cargo test --workspace --doc --all-features`.
- `make nixie` — validates Mermaid diagrams. This plan adds none, but the gate
  scans every Markdown file, so it must still pass.

There is **no** path filter for documentation-only pull requests
(`.github/workflows/ci.yml:4-11`); every gate runs regardless.

## Signposts: documentation and skills

Read before starting:

- `AGENTS.md` — repository conventions, especially *Documentation
  maintenance*, *Markdown guidance*, and *Testing*.
- `docs/documentation-style-guide.md` — the binding prose rules. Note
  line 39-40: first- and second-person pronouns are forbidden *except* in
  `README.md`, so the new README section may address the reader directly while
  the ADR may not. Note line 108: the contents file must be updated whenever a
  document is added.
- `docs/contents.md` — the documentation index; the ADR is added under
  `## Decision records` (line 84), newest first, matching the ADR-020 and
  ADR-019 entries at lines 146-150.
- `docs/users-guide.md` section *Review the safety boundary* (line 1649).
- `docs/developers-guide.md` section *Command interpolation contract*
  (line 3186).
- `docs/netsuke-design.md` sections at lines 290-298 and 2616-2644.
- `docs/rust-testing-with-rstest-fixtures.md` — fixture and parameterization
  idiom for the new tests.
- `docs/rstest-bdd-users-guide.md` — consulted and deliberately not used; see
  decision `D-NO-BDD`.
- `docs/localization-glossary.md` and `docs/localization-styleguide.md` — for
  milestone EP-M4.
- `docs/repository-layout.md:55-69` — the README family's stated obligations.

Skills to load:

- `execplans` — this document's format and living-section obligations.
- `rust-router` — routes to `rust-unit-testing` for the new `rstest` cases.
- `hexagonal-architecture` — used as a boundary check only. This plan adds no
  port and no adapter; see `Conformance basis` for why that is the correct
  outcome rather than an omission.
- `en-GB-oxendict` — Oxford spelling, enforced by the `typos` gate.
- `codegraph-mcp` — for navigating `src/ir/cmd_interpolate/` if the writer
  needs to re-verify a fact rather than trust this plan.

## Conformance basis

Upstream artefacts, at the revisions current on branch
`4-4-1-document-command-placeholder-contract-in-readme`, based on `origin/main`
at commit `81d44f89`:

- `docs/roadmap.md`, phase 4, item **4.4.1** (lines 530-537) and its four
  sub-items. Identifier used below: `RM-4.4.1`, with sub-items `RM-4.4.1.a`
  (add the README section), `RM-4.4.1.b` (state the supported placeholders),
  `RM-4.4.1.c` (state the backtick boundary), `RM-4.4.1.d` (state whether
  `shlex::split` is part of the semantic acceptance contract).
- `docs/formal-verification-methods-in-netsuke.md`, section **Command
  placeholder contract** (lines 263-279). Identifier `FV-CPC`. It poses three
  open questions, referred to as `FV-CPC-Q1` (are those the only supported
  placeholders), `FV-CPC-Q2` (is the POSIX backtick rejection and PowerShell
  escape handling the full contract or a temporary subset), and `FV-CPC-Q3` (is
  `shlex::split` part of the semantic acceptance contract or only a guard
  against obviously malformed commands).
- `docs/roadmap.md` item **4.2.3** (lines 490-504), the stated prerequisite.
  All six sub-items are checked. See `Risks` for the status discrepancy with
  its execplan and how it is resolved.
- `docs/adr-004-bound-kani-ir-harnesses-to-small-n.md` — constrains what may
  be claimed as *proved* versus *property-tested*.
- `docs/adr-014-backend-text-escaping-seam.md` — the escaping seam this
  contract sits above.
- No Terms of Reference document exists for this repository. Do not invent
  one; `docs/roadmap.md` and `docs/formal-verification-methods-in-netsuke.md`
  are the upstream artefacts.

Trace links:

```plaintext
RM-4.4.1.a -> EP-M3 -> README.md "Security and command interpolation"
RM-4.4.1.b / FV-CPC-Q1 -> D1 -> EP-M1 (ADR-021 §Placeholders) -> EP-M3 -> tests::readme_security::documented_safe_placeholder_manifest_builds
RM-4.4.1.c / FV-CPC-Q2 -> D2 -> EP-M1 (ADR-021 §Backticks)   -> EP-M3 -> tests::readme_security::documented_backtick_manifest_is_rejected
RM-4.4.1.d / FV-CPC-Q3 -> D3 -> EP-M1 (ADR-021 §shlex guard) -> EP-M3 -> tests::readme_security::documented_backtick_manifest_is_rejected
RM-4.4.1.b / Fact A    -> OBL-RECIPE-KIND -> tests::readme_security::placeholder_rewriting_differs_by_recipe_kind
Fact A     -> EP-M2 -> docs/developers-guide.md "Command interpolation contract"
Fact A     -> EP-M2 -> docs/users-guide.md "Review the safety boundary" (:1697, :1713)
Fact A     -> EP-M2 -> docs/formal-verification-methods-in-netsuke.md FV-CPC
D2 quoting -> OBL-QUOTING -> tests::readme_security::netsuke_owned_path_substitutions_are_quoted
RM-4.4.1.a -> EP-M4 -> six translated READMEs
RM-4.4.1   -> EP-M5 -> docs/roadmap.md item 4.4.1 marked done
```

**Hexagonal-architecture conformance.** The skill's dependency rule requires
that domain logic not depend on adapters. `src/ir/cmd_interpolate/` is domain
policy: it decides what may be rewritten and what must be rejected, and it
returns a typed `IrGenError` rather than performing input or output. The shell
and Ninja adapters (`src/ninja_gen_recipe_shell.rs`, `src/runner/process/`)
consume its output. This plan writes documentation about that boundary and does
not move it. The correct architectural outcome here is **no new port and no new
adapter**: introducing one to satisfy a pattern would be a transplant, not a
boundary. Recorded as `D-NO-PORT`.

## Constraints

Hard invariants. A conflict with any of these requires escalation, not a
workaround.

1. **No production behaviour change.** Do not modify any file under `src/`
   other than a doc comment correction explicitly authorized by EP-M2. This is
   a documentation item; the contract is documented as it is, not as it might
   ideally be. If the documented behaviour looks wrong, record it in
   `Surprises & discoveries` and escalate — do not fix it here.
2. **Documentation must match the implementation.** Every factual claim in the
   new README section must be traceable to a cited file and line, or to a test
   added by this plan. Where the implementation is narrower than a reader might
   assume, say so plainly rather than omitting it.
3. **README fence discipline.** Every fenced code block added to `README.md`
   carries a `<!-- tested-example: ... -->` marker, declares a language, and
   has a matching entry in `EXPECTED_EXAMPLE_IDS`
   (`tests/documentation_examples_tests.rs`).
4. **Structural parity of the README family.** At the end of EP-M4 all seven
   READMEs have the same heading count, order, and nesting. The plan may not
   land with English-only parity broken across a milestone boundary other than
   the explicitly declared EP-M3 → EP-M4 window.
5. **Prose rules.** en-GB Oxford spelling; prose wrapped at 80 columns; code
   fences at most 120 columns; `-` bullets; sentence-case headings; no skipped
   heading levels; a language identifier on every fence. Backtick any
   identifier whose spelling would otherwise trip the `typos` gate.
6. **No new dependency.** Nothing is added to `Cargo.toml`.
7. **No claim of proof beyond what exists.** Per
   `docs/adr-004-bound-kani-ir-harnesses-to-small-n.md:167-177`, Kani proves
   the marker recognizer only. The README and ADR must not describe the
   backtick guard or the `shlex` guard as *proved*; they are property-tested.
8. **`docs/users-guide.md` remains the detailed reference.** The README owns
   the placeholder contract and the non-guarantees; the users' guide keeps the
   shell-route mechanics, and the README does not restate them. Correcting a
   factually wrong sentence there (EP-M2) is *not* weakening it — leaving the
   README to contradict it would be. Do not delete guidance, and do not soften
   a warning.
9. **Do not edit historical documents.** No file under `docs/archive/`, no
   `docs/adr-0*.md` other than the new ADR-021, and no completed execplan. A
   merged execplan records the repository at its own moment; the stale
   `IN PROGRESS` header on the 4.2.3 plan is flagged, not fixed.

## Tolerances (exception triggers)

Stop and escalate when any threshold is reached. Do not work around them.

- **Scope.** More than eighteen files changed, or more than 1200 net added
  lines across the whole plan. The expected set is sixteen paths: `README.md`;
  the six translations; `docs/adr-021-command-placeholder-contract.md`;
  `docs/contents.md`; `docs/developers-guide.md`;
  `docs/formal-verification-methods-in-netsuke.md`; `docs/users-guide.md`;
  `docs/repository-layout.md`; `docs/roadmap.md`;
  `tests/documentation_examples_tests.rs`; and one new test file. This plan
  document itself is a seventeenth, and `src/ir/cmd_interpolate/mod.rs` a
  possible eighteenth if its doc comment needs correcting.

  The line ceiling is deliberately generous because the README section is
  written seven times. A realistic estimate is 70-100 lines of section prose
  per edition (490-700 in total), 150-250 for the ADR, 150-220 for the test
  file, and 30-50 for the index, roadmap, and correction edits: 820-1220. A
  tighter ceiling would fire on the plan's own expected path, which trains an
  implementer to ignore the tolerance and destroys its value for the cases that
  matter.
- **Production code.** Any change to `src/` beyond the doc-comment correction
  named in EP-M3. Immediate stop.
- **Contract disagreement.** If a new test shows the implementation does not
  behave as `Context and orientation` states, stop. The plan's facts are wrong,
  or the code has a defect; either way a human decides.
- **Interface.** Any change to a public API signature, or to
  `EXPECTED_EXAMPLE_IDS` beyond adding the three new identifiers.
- **Dependencies.** Any addition to `Cargo.toml`. Immediate stop.
- **Iterations.** A gate that still fails after three fix attempts.
- **Translation confidence.** If the writer cannot render a security claim in
  a target language with confidence that the meaning is preserved, stop at
  EP-M4 and escalate rather than shipping an approximate security statement. A
  wrong translation of a safety boundary is worse than an absent one.
- **Ambiguity.** Any point where `FV-CPC-Q1`, `FV-CPC-Q2`, or `FV-CPC-Q3`
  could reasonably be settled the other way and the choice changes what the
  README promises. `D1` carries a known live instance of this: see the
  `D1-LEGACY` note in `Decision log`.

## Risks

- **Risk: roadmap 4.2.3 is marked complete but its execplan header says
  `IN PROGRESS`.** `docs/roadmap.md:490-504` checks all six sub-items;
  `docs/execplans/4-2-3-kani-harnesses-for-command-interpolation.md:8` reads
  `Status: IN PROGRESS`. 4.4.1 declares 4.2.3 a prerequisite. Severity: low.
  Likelihood: certain (already observed). Mitigation: the substantive
  prerequisite is that the Kani and Proptest harnesses exist and pass, which is
  directly verifiable — `src/ir/cmd_interpolate/verification.rs` contains both
  proofs and `src/ir/cmd_interpolate_property_tests.rs` contains eleven
  properties. The prerequisite is met in substance. Treat the header as a stale
  status line, note it in `Surprises & discoveries`, and do **not** edit that
  execplan; a completed plan is a historical document. Raise it separately if
  desired.

- **Risk: the README section overstates the guarantee.** A security section
  that reads as reassurance rather than as a boundary is actively harmful,
  because a manifest author may conclude Netsuke sanitizes their shell text.
  Severity: high. Likelihood: medium. Mitigation: the section leads with what
  Netsuke does *not* do. Constraint 2 and the `logisphere-design-review` pass
  at Stage A both target this. The existing README already says "it is not a
  sandbox"; keep that framing and strengthen it.

- **Risk: the two new README examples make the test suite slower or flaky.**
  The e2e harness needs `ninja` on `PATH` and skips when absent
  (`tests/documentation_examples_e2e_tests.rs:46-50`). Severity: low.
  Likelihood: low. Mitigation: the rejection example needs no build at all — it
  fails during IR lowering, before Ninja is invoked — so it can be a plain
  integration test with no external tool. Only the accepting example touches
  Ninja, and it reuses the existing skip guard.

- **Risk: translated security prose drifts from the English original.** No
  gate checks it; the `typos` gate explicitly excludes the six files. Severity:
  medium. Likelihood: medium. Mitigation: translate the *structure*
  mechanically — same heading, same bullet count, same order, identical code
  fences and identifiers — so a reviewer can compare line-for-line without
  reading the target language. Consult `docs/localization-glossary.md` for each
  locale's established term for "placeholder", "shell", and "quote". Escalate
  per the translation tolerance rather than guessing.

- **Risk: `mdtablefix` reformats unrelated Markdown.** `make check-fmt`
  enforces canonical formatting repository-wide, and a stale file can surface
  as a spurious failure in this branch. Severity: low. Likelihood: medium.
  Mitigation: run `make fmt` early, inspect `git diff --stat`, and if files
  outside this plan's scope change, commit that reformatting as a separate,
  clearly labelled commit before the substantive work.

- **Risk: `actionlint` is not on `PATH`, so `make lint` exits 127.** A known
  environment issue, not a code defect. Severity: low. Likelihood: medium.
  Mitigation: prepend `~/go/bin` to `PATH` before running `make lint`. If it is
  genuinely absent, record that `github-actions-lint` did not run and say so in
  the evidence rather than reporting a clean gate.

- **Risk: the ADR number collides.** `docs/` already contains duplicate
  numbers at 003, 004, and 014. Severity: low. Likelihood: low. Mitigation: the
  highest existing number is 020
  (`docs/adr-020-release-admission-observability.md`). Use 021 and re-run
  `ls docs/adr-*` immediately before creating the file, in case another branch
  has landed one.

## Verification plan

This change adds no production code, so it introduces no new invariant over
program inputs, states, or transitions. Stating that and stopping would be a
vacuous discharge of this section. It is not the right answer either, because
the change *does* introduce an obligation of a different kind, and that
obligation is checkable.

The obligation is **documentation–implementation agreement**: each claim the
README makes about the placeholder contract must be true of the code at the
commit that ships it, and must become false — visibly, in a failing test — if
the code later changes. A README claim with no executable counterpart is an
assertion nobody can falsify, which is exactly the failure mode this section
exists to prevent.

### Axioms

Assumptions relied upon, not verified here:

- `AX-SHLEX`: `shlex` 2.0.1's `split` implements a POSIX-compatible word
  split and returns `None` on unterminated quoting or an unterminated escape.
  Netsuke's contract is defined *in terms of* this behaviour; the crate's
  internals are not this repository's to verify. Recorded in the ADR because it
  is precisely why `D3` refuses to commit to the accepted set.
- `AX-NINJA-SH`: on Unix, Ninja passes a rule's `command` string to `sh -c`;
  on Windows it passes the string to `CreateProcess`. This is Ninja's
  documented behaviour, not Netsuke's, and it is the reason backticks are a
  live construct rather than inert text. No Netsuke source names `sh -c`; the
  README must therefore attribute this to Ninja, not claim it as Netsuke
  behaviour.
- `AX-SHELL-QUOTE`: `shell-quote` 0.7.2 in `Sh` mode emits POSIX-quoted text
  that round-trips through a POSIX shell. This is what makes the path-quoting
  claim true.
- `AX-HARNESS`: `tests/documentation_examples/mod.rs` faithfully extracts the
  fenced text from `README.md`. It is repository-owned and already
  self-testing, so a test that runs an extracted example genuinely exercises
  the published prose rather than a copy.

### Obligations

**`OBL-PLACEHOLDERS` — the documented placeholder set is the accepted set.**

- Statement: a manifest using `{{ ins }}` and `{{ outs }}` in a `command:`,
  alongside a literal `$PATH`, compiles and builds; the `$PATH` reaches the
  shell unrewritten.
- Method: parameterized integration test over the documented example, using
  `rstest` with `googletest` matchers and `pretty_assertions`.
- Rationale: the accepted set is finite and enumerable. Property testing over
  generated templates already exists upstream
  (`src/ir/cmd_interpolate_property_tests.rs`); duplicating it here would add
  cost without adding evidence. What is *not* yet covered is that the README's
  specific published text behaves as the README says, and that is an
  exact-example obligation.
- Domain: the three fenced examples in the new README section, extracted at
  run time by `documented_example`.
- Artefact: `tests/readme_security_tests.rs`, case
  `documented_safe_placeholder_manifest_builds`.
- Evidence:
  `NETSUKE_REQUIRE_NINJA=1 cargo nextest run --test readme_security_tests`. Red
  before the README section exists, because `documented_example` returns an
  error whose context is
  `documented example 'readme-safe-placeholder-manifest' should exist`. Green
  once the section and the registry entry are added. Discharged when the test
  passes and `every_documented_fence_has_a_known_unique_identifier`
  (`tests/documentation_examples_tests.rs:143`) also passes.

  The environment variable is load-bearing. The `ninja` guard at
  `tests/documentation_examples_e2e_tests.rs:52` is a silent `return Ok(())`,
  but `test_support/src/ninja.rs:73-89` escalates to a panic when
  `NETSUKE_REQUIRE_NINJA=1`, which `.github/workflows/ci.yml:78` sets. A green
  local `make test` without it is therefore not evidence that this obligation
  was exercised.
- Non-vacuity: the test asserts on observable output, not merely on a
  non-error exit. It requires that the built artefact contains the expected
  text and that the generated Ninja command still contains a literal `$PATH`
  after Netsuke's `$` → `$$` Ninja escaping is reversed.

  The obvious negative control — change `{{ ins }}` to `{{ inputs }}` and
  expect failure — is **rejected as non-discriminating**.
  `src/manifest/mod.rs:128` sets `UndefinedBehavior::Strict`, so an undefined
  variable fails at stage 1 inside MiniJinja, before `find_substitution` is
  ever reached. That control fires identically against an implementation
  recognizing a completely different marker set, so it proves only that
  MiniJinja is strict.

  The discriminating control is to put a literal `$in` in the **`command:`**
  example and require it to survive into the generated Ninja verbatim, as
  `$$in`. That fails only if the recipe-kind asymmetry breaks, which is the
  claim actually at issue. Run it once by hand, record the transcript in
  `Artefacts and notes`, and do not commit it.

**`OBL-RECIPE-KIND` — the placeholder set really does differ by recipe kind.**

- Statement: `$in` and `$out` are rewritten to paths in a `script:` recipe and
  left as literal shell text in a `command:` recipe; `$ins`, `$outs`, `$input`,
  and `$output` are left alone in both.
- Method: parameterized `rstest` over the (placeholder, recipe kind) matrix,
  asserting on generated Ninja text.
- Rationale: this is the plan's headline discovery, the reason three documents
  are wrong today, and the most surprising claim the README will make. Shipping
  it with no executable counterpart would reproduce exactly the failure mode
  this section exists to prevent: a documentation claim nobody can falsify.
  Absent from revision 1; added at design review.
- Domain: the ten cells formed by `{{ ins }}`, `{{ outs }}`, `$in`, `$out`,
  and `$ins`, each crossed with `command:` and `script:`.
- Artefact: `tests/readme_security_tests.rs`, case
  `placeholder_rewriting_differs_by_recipe_kind`.
- Evidence: `cargo nextest run --test readme_security_tests`. Discharged when
  every cell matches the README's table.
- Non-vacuity: both arms of each row are asserted, so the test fails against an
  implementation that rewrote `$in` in both recipe kinds **and** against one
  that rewrote it in neither. A single-arm test would pass against the
  currently documented — and wrong — uniform behaviour, which is precisely how
  the existing misstatement survived. As a seeded fault, remove the two
  `try_match_dollar_placeholder` calls from `find_script_substitution`
  (`src/ir/cmd_interpolate/mod.rs:269-275`); the `script:` rows must fail.
  Revert immediately.

**`OBL-QUOTING` — Netsuke's own path substitutions are shell-quoted.**

- Statement: an input path containing a space is substituted into a `command:`
  in a form the shell reads as a single argument.
- Method: exact-example integration test asserting on generated Ninja text.
- Rationale: this is the section's only *positive* security promise; every
  other claim is a boundary or a non-guarantee. An unasserted positive security
  promise is the worst kind of documentation debt, because a reader acts on it.
  Absent from revision 1; added at design review.
- Domain: one input path containing a space, on the POSIX route.
- Artefact: `tests/readme_security_tests.rs`, case
  `netsuke_owned_path_substitutions_are_quoted`.
- Evidence: `cargo nextest run --test readme_security_tests`. Discharged when
  the generated command contains the quoted form `shell-quote` produces rather
  than the bare path.
- Non-vacuity: the assertion is on the quoted form specifically, not merely on
  the path appearing somewhere, so it fails against an implementation that
  interpolated the path unquoted. `tests/command_escaping_tests.rs` is adjacent
  but asserts through `shlex::split` word counts rather than pinning the
  README's wording; this case pins the wording.

**`OBL-BACKTICK-REJECT` — the documented backtick boundary is the enforced
boundary.**

- Statement: a manifest whose `command:` places `{{ ins }}` inside a backtick
  pair is rejected during IR lowering with the diagnostic
  `Invalid command interpolation:`, before Ninja is invoked.
- Method: parameterized integration test with a negative control, plus a
  mutation check reusing the repository's existing mutation artefact.
- Rationale: this is the single claim in the section most likely to be read as
  a stronger guarantee than it is. It needs a test that fails loudly if
  rejection is ever relaxed, and prose that a reader cannot mistake for general
  injection defence.
- Domain: the documented rejecting example, plus two handwritten controls in
  the same test file that are *not* in the README: a balanced backtick pair
  containing only author text, which must be **accepted** (proving the guard is
  narrow, exactly as documented); and an odd count of backticks with no marker
  at all, which must be **rejected** (proving the parity check is whole-string,
  exactly as documented).
- Artefact: `tests/readme_security_tests.rs`, cases
  `documented_backtick_manifest_is_rejected`,
  `balanced_author_backticks_are_accepted`,
  `odd_backtick_count_without_markers_is_rejected`.
- Evidence: `cargo nextest run --test readme_security_tests`. Red before the
  README section exists, for the same missing-identifier reason. Discharged
  when all three pass and the diagnostic text matches
  `locales/en-GB/messages.ftl:179`.
- Non-vacuity: the two control cases are the whole point. A test suite that
  only checked rejection would pass equally well against an implementation that
  rejected *every* backtick, which is not what the README will claim. The
  accepting control fails against such an implementation, so the pair pins the
  boundary from both sides. As a seeded fault, apply the existing mutation patch
  `docs/verification/mutations/ir__cmd_interpolate__property_tests__substituted_odd_backticks_are_rejected.patch`,
  which flips `has_unmatched_backticks`'s parity test from `!= 0` to `== 0`,
  and confirm `odd_backtick_count_without_markers_is_rejected` fails under it.
  Revert the patch afterwards. Record the transcript.

**`OBL-SHLEX-SCOPE` — the `shlex` gate applies to commands, not scripts.**

- Statement: text that `shlex::split` cannot split is rejected in a
  `command:` and accepted in a `script:`.
- Method: parameterized integration test over both recipe kinds with the same
  offending text.
- Rationale: this asymmetry is `RM-4.4.1.d`'s substance and is currently
  documented nowhere. A single example per branch is decisive because the two
  code paths are distinct functions with no shared guard
  (`src/ir/cmd_interpolate/mod.rs:189-212`).
- Domain: one unterminated single quote, used as a `command:` and as a
  `script:`.
- Artefact: `tests/readme_security_tests.rs`, case
  `shlex_gate_applies_to_commands_not_scripts`.
- Evidence: `cargo nextest run --test readme_security_tests`. Discharged when
  the command form errors and the script form generates successfully.
- Non-vacuity: both arms must be asserted. Asserting only the rejection would
  pass against an implementation that gated both, which would contradict the
  README. The offending text must be chosen so that `shlex::split` genuinely
  returns `None` — verify by running `cargo test --doc` on a scratch snippet,
  or by observing the diagnostic — and not merely so the manifest is invalid
  for some unrelated reason. Assert on the specific `ir.invalid_command` text,
  not on exit status alone.

**`OBL-STRUCTURAL-PARITY` — the README family stays in structural lockstep.**

- Statement: at the end of EP-M4, the seven README files have identical
  heading counts, levels, and order.
- Method: a mechanical check, run and recorded, not a committed test.
- Rationale: a committed parity test is tempting but would be a new
  repository-wide policy gate, which exceeds this item's mandate and would fire
  on unrelated future work. The convention is documented in
  `docs/repository-layout.md`; enforcing it in CI is a separate decision. If a
  reviewer asks for the gate, escalate rather than adding it silently.
- Artefact: the command in `Concrete steps` step 9.
- Evidence: the command prints `7` identical counts and a matching level
  sequence.
- Non-vacuity: revision 1 claimed the check "compares the level sequence, not
  just the count". It did not. `tr -d ' \n'` collapsed the levels into one
  undelimited run of `#` characters, so `## A / ### B` and `### A / ## B` were
  indistinguishable and the second command was a character count redundant with
  the first. The corrected command in `Concrete steps` step 8 delimits each
  heading with `paste -sd,`, so a reordering or a level change is caught. This
  correction came from the design review.

**No obligation is claimed for the correctness of `src/ir/cmd_interpolate/`
itself.** That is discharged upstream by roadmap 4.2.3's Kani proofs and
Proptest properties, and this plan must not restate them as though they were
new evidence.

## Milestones and plateaus

### EP-M1 — settled contract, recorded

- Outcome: `docs/adr-021-command-placeholder-contract.md` exists and records
  decisions `D1`, `D2`, and `D3`. `docs/contents.md` lists it under
  `## Decision records`. `docs/netsuke-design.md` references it from the
  command-lowering discussion near line 2616. No other file changes.
- Requirements advanced: `FV-CPC-Q1`, `FV-CPC-Q2`, `FV-CPC-Q3`; prerequisites
  for `RM-4.4.1.b`, `.c`, `.d`.
- Acceptance evidence: `make check-fmt` and `make markdownlint` pass. The ADR
  contains a Status, Date, and Context and Problem Statement section in that
  order, per `docs/documentation-style-guide.md:374-384`.
- Conformance check: the ADR states no guarantee the code does not provide;
  each of its three decisions cites the implementing file and line; it does not
  describe the backtick or `shlex` guards as proved (Constraint 7).
- Recovery: the ADR is a new file and two small insertions. Revert with
  `git revert` or delete the file and drop the two index lines.
- Remaining gaps: the README still says nothing.
- Compatibility decision: none. A new document has no consumers.

### EP-M2 — internal documents corrected

This milestone deliberately precedes the README section. Landing the README
first would create a plateau at which `README.md` and `docs/users-guide.md`
state opposite things about `$in` in a `script:`, and `D-SCOPE` already argues
that such a state is exactly the ambiguity 4.4.1 exists to remove. The two
milestones are independent, so ordering costs nothing and removes the
contradiction window entirely.

- Outcome: three documents state the per-recipe-kind placeholder set
  correctly.
  - `docs/developers-guide.md:3187-3190` (§*Command interpolation contract*),
    which currently says "Literal shell variables such as `$in`, `$out`,
    `$ins`, and `$outs` remain unchanged" without qualifying it to command
    recipes.
  - `docs/users-guide.md:1697-1698`, which says "`{{ ins }}` and `{{ outs }}`
    are the only Netsuke markers for input and output paths", and
    `docs/users-guide.md:1713-1715`, which tells authors to replace `$in` and
    `$out` without saying that the `script:` forms still resolve.
  - `docs/formal-verification-methods-in-netsuke.md:265-266` (§*Command
    placeholder contract*) and `:64-66` (§*Kani for command interpolation*),
    both of which say literal `$in` and `$out` "remain shell variables"
    unqualified. Also record in §*Command placeholder contract* that
    `FV-CPC-Q1` through `FV-CPC-Q3` are now answered, pointing at ADR-021, and
    correct the `[^8]` footnote path from `src/ir/cmd_interpolate.rs` to
    `src/ir/cmd_interpolate/mod.rs`.

  `docs/netsuke-design.md:290-291` already states this correctly — "standalone
  `$in` and `$out` resolve only in scripts" — and must **not** be changed. It
  is the canonical wording the other three should converge on. Where a doc
  comment in `src/ir/cmd_interpolate/mod.rs` is itself inaccurate about
  scripts, correct the comment text only; no code.

- Requirements advanced: Fact A consistency across the documentation set;
  prerequisite for `RM-4.4.1.a`, because the README will link to
  `docs/users-guide.md#review-the-safety-boundary`.
- Acceptance evidence: the scoped grep in `Concrete steps` step 5 returns no
  unqualified claim outside `docs/archive/` and the ADR set. `make check-fmt`,
  `make markdownlint`, `make lint`, and `make test` pass — `make lint` matters
  because a doc-comment edit recompiles.
- Conformance check: no behaviour changed; `git diff --stat src/` shows only
  comment lines; the three corrected statements agree with each other and with
  `docs/netsuke-design.md:290-291`; no historical document (`docs/archive/`, any
  `docs/adr-0*.md`, any completed execplan) was edited.
- Recovery: revert the commit. The edits are independent of every later
  milestone.
- Remaining gaps: the README still says nothing; six translations lack the
  section.
- Compatibility decision: none.

### EP-M3 — the README states the contract, executably

- Outcome: `README.md` carries a new `## Security and command interpolation`
  section, placed after `## What works today` and before
  `## Release and development status`, delimited by the repository's
  seventy-underscore thematic breaks. Its internal parts are **bold labels, not
  `###` headings**, so the file's heading count rises from twelve to exactly
  thirteen and the translations do not each gain six headings. It contains
  three marked, executable fenced examples.
  `tests/documentation_examples_tests.rs` registers the three new identifiers.
  `tests/readme_security_tests.rs` exists and discharges `OBL-PLACEHOLDERS`,
  `OBL-RECIPE-KIND`, `OBL-QUOTING`, `OBL-BACKTICK-REJECT`, and
  `OBL-SHLEX-SCOPE`. The existing safety paragraph in
  `## Release and development status` (`README.md:203-206`) is reduced to a
  one-line cross-reference that **retains its link** to the users' guide safety
  boundary, so the two do not contradict each other and no link is lost.
- Requirements discharged: `RM-4.4.1.a`, `.b`, `.c`, `.d`.
- Acceptance evidence: `make check-fmt`, `make markdownlint`, `make lint`, and
  `NETSUKE_REQUIRE_NINJA=1 make test` all pass. Every new test fails before the
  README section exists and passes after. All three negative controls behave as
  `Verification plan` requires.
- Conformance check: every claim traces to a cited line or a new test; the
  live hazard appears before the placeholder table, not after it; the sentence
  "a backtick pair you wrote is executed by the shell" appears above the first
  fence; `D1`-`D3` in ADR-021 and the README prose agree word-for-word on the
  three contested points; the section carries a pre-1.0 stability caveat
  because it sits above the release-status hedge.
- Recovery: revert the commit. `README.md` and the two test files are the only
  changes; nothing else depends on them.
- Remaining gaps: six translations lack the section.
- Compatibility decision: none. `EXPECTED_EXAMPLE_IDS` is a test-only,
  crate-internal constant; adding to it needs no shim.

### EP-M4 — the translated READMEs regain parity

- Outcome: all six translated READMEs carry the equivalent section in the same
  position, with the same heading level, the same bullet order, and byte-
  identical code fences. `README.md` is unchanged by this milestone.
- Requirements discharged: `RM-4.4.1.a` across the README family;
  `OBL-STRUCTURAL-PARITY`.
- Acceptance evidence: the parity command in `Concrete steps` step 9 reports
  identical heading structures across all seven files. `make check-fmt` and
  `make markdownlint` pass. The translated files remain outside the `typos`
  scope, so the spelling gate is unaffected.
- Conformance check: no translated file gained a `tested-example` marker;
  terminology matches `docs/localization-glossary.md` for each locale; no
  security claim was softened or strengthened in translation.
- Recovery: revert the commit. English parity is restored trivially because
  EP-M3 did not depend on the translations.
- Remaining gaps: the roadmap entry is still open.
- Compatibility decision: none.

### EP-M5 — roadmap closed and gates green

- Outcome: `docs/roadmap.md` item 4.4.1 and its four sub-items are marked
  `[x]`, with a short completion note in the repository's established style
  recording that the contract was settled in ADR-021 and that translations
  landed. All gates pass on the full branch.
- Requirements discharged: `RM-4.4.1`.
- Acceptance evidence: the full gate sequence in `Concrete steps` step 10,
  with each log file retained.
- Conformance check: reconcile every entry in `Surprises & discoveries`
  against the upstream artefacts before setting this plan to `COMPLETE`, per the
  `execplans` skill's reconciliation rule.
- Recovery: the roadmap edit is a four-line change; revert independently.
- Remaining gaps: none for `RM-4.4.1`. The 4.2.3 execplan status discrepancy
  is deliberately left for a separate decision.
- Compatibility decision: none.

## Plan of work

### Stage A — settle the three contract questions (no file changes)

Re-verify Facts A, B, and C against the code before writing anything. Do not
take this plan's citations on trust; open each cited line. If any fact is
wrong, stop — a `Constraints` item 2 conflict.

Then settle the three questions. These answers were revised at the
`logisphere-design-review` checkpoint; the wording below is what the README and
ADR-021 must say.

- **`D1` (answers `FV-CPC-Q1`).** `{{ ins }}` and `{{ outs }}` are the
  supported placeholders, in both `command:` and `script:` recipes. In
  `script:` recipes Netsuke *additionally* rewrites bare `$in` and `$out`, at
  identifier boundaries. Everything else — `$ins`, `$outs`, `$input`, `$output`,
  `$PATH` — is literal text for the shell, in both recipe kinds. Netsuke does
  not expose Ninja's own `$in` / `$out` rule variables, because it bakes
  resolved paths into each content-hashed rule.

  The `script:` forms are documented as **retained legacy behaviour, not a
  blessed feature**, and the README steers authors to `{{ ins }}` and
  `{{ outs }}` in new manifests. Two consequences must be stated, because both
  are foot-guns a reader would not predict: a script that legitimately writes
  `in=foo; echo $in` has its own shell variable rewritten to input paths with
  no diagnostic; and moving that text from `script:` to `command:` silently
  changes its meaning, because `$in` there degrades to an empty shell variable.
  See `D1-LEGACY` in `Decision log` for the open question this raises.

- **`D2` (answers `FV-CPC-Q2`).** Two separate mechanisms, promised at two
  different strengths. Revision 1 promised both as guarantees; the review
  showed that freezes a whole-string parity approximation alongside a genuine
  invariant, such that a future quoting-aware fix would be a breaking change.
  The revised wording:

  - **Invariant, promised.** A Netsuke placeholder is never substituted inside
    a backtick or `$( … )` region; such a recipe is rejected. This is policy
    and will hold.
  - **Conservative check, not promised.** A `command:` recipe whose
    substituted text contains an odd total number of backtick characters is
    *currently* rejected. The check is not quoting-aware — it counts backticks
    inside single quotes too — so it may reject valid shell text such as
    `echo 'a` followed by a backtick and a closing quote. A future release may
    accept more, and such widening is **not** treated as a breaking change.
  - **Not a guarantee at all.** Netsuke does not inspect backticks the author
    wrote. A balanced pair is passed to the shell and executed as command
    substitution.
  - **Route scope.** PowerShell is outside both, because its backtick is an
    escape character. The parity check is also `command:`-only; a `script:`
    recipe gets the marker invariant but no parity check. State this, because
    a reader who moves a recipe to `script:` otherwise loses a check silently.

- **`D3` (answers `FV-CPC-Q3`).** `shlex::split` **is** part of the acceptance
  contract in the observable sense: a `command:` recipe on the POSIX or Bash
  route whose substituted text cannot be split is rejected, with a stable,
  localized diagnostic, at IR-lowering time. It does not apply to `script:`
  recipes or to PowerShell, and Netsuke never executes the tokens it produces.

  It is **not** a stability commitment about the precise accepted set. That set
  is fixed by the `shlex` version resolved when the binary was built, and
  `Cargo.toml:138` is a caret requirement rather than a pin (`AX-SHLEX`). Name
  both drift directions explicitly, because they are not symmetric:

  - a future `shlex` that accepts text today's rejects means previously
    rejected manifests begin to build — not treated as a breaking change;
  - a future `shlex` that rejects text today's accepts means a working
    manifest stops building — a defect, not a policy change, and recorded in
    the changelog.

  The README must also say what passing the gate does *not* mean: `shlex`
  performs no expansion and treats a backtick as an ordinary character, so a
  command that splits cleanly is not thereby safe. Downstream tooling must not
  reuse `shlex::split(x).is_some()` as an injection check.

Go/no-go: the `logisphere-design-review` pass has run and its findings are
folded into revision 2. Before writing prose, re-read `D1`-`D3` once and
confirm that no sentence could be read as a guarantee Netsuke does not make.

### Stage B — red

Add `tests/readme_security_tests.rs` with the seven cases named in
`Verification plan`, referencing the three documented-example identifiers that
do not yet exist. Its first line must be `mod documentation_examples;` — that
declaration is load-bearing beyond the import, because
`tests/integration_test_wiring_tests.rs` enforces both that module trees are
wired to Cargo test targets (`:150`) and that Cargo discovers every top-level
integration source (`:186`). No `[[test]]` entry is needed: `Cargo.toml` sets no
`autotests = false`, so autodiscovery covers the new file.

Run the suite and observe each case failing with
`documented example '…' should exist`. This is the red stage and it must be
observed, not assumed. Do not use an expected-failure marker; Rust has no strict
`xfail`, and the missing-identifier error is already specific enough to prove
the test fails for the intended reason.

Also add the three identifiers to `EXPECTED_EXAMPLE_IDS` *before* adding the
README fences, and observe
`every_documented_fence_has_a_known_unique_identifier`
(`tests/documentation_examples_tests.rs:143`) fail with a registry-drift
message naming exactly the three missing identifiers. That is a second, sharper
red signal. Revision 1 named this test
`documented_example_registry_is_exhaustive`, which does not exist.

### Stage C — green

Write the README section (EP-M3). Its parts are **bold labels, not `###`
headings**, so the file gains exactly one heading and each translation does not
gain six. The order below was changed at design review: revision 1 put the
placeholder table second and the backtick hazard fifth, so a reader who stopped
halfway came away with a capability list and no warning. The live hazards now
come first.

1. **Framing.** A `Netsukefile` executes commands, so treat it as you would a
   `Makefile`. Netsuke narrows some quoting mistakes but is not a sandbox. This
   paragraph must contain the sentence "a backtick pair you wrote is executed
   by the shell", and it must appear above the first fence.
2. **What Netsuke does not protect you from.** Author-written backticks and
   `$( … )`; and, under its own bold label, **"Values you template in are not
   quoted"** — arbitrary Jinja output, `raw` blocks, and handwritten shell
   fragments are indistinguishable from typed text by the time Netsuke sees
   them. This is the actual injection vector, and revision 1 buried it in a
   list of things Netsuke "leaves alone" alongside the entirely benign `$PATH`,
   which reads as a capability list rather than a warning. Give it a label a
   reader cannot skim past.
3. **What Netsuke rewrites.** The `D1` contract as a compact table: rows for
   `{{ ins }}`, `{{ outs }}`, `$in`, `$out`, and the inert `$ins`/`$outs`/
   `$input`/`$output` group; columns for `command:` and `script:`. Cells are
   code identifiers and yes/no, which is what survives translation intact. Mark
   the `$in` / `$out` row as retained legacy, with the shadowing caveat from
   `D1`. The accepting example fence follows.
4. **What Netsuke quotes.** Only its own path substitutions, via
   `shell-quote`, encoded for the surrounding quote context. Nothing else. The
   quoting example fence follows.
5. **Backticks and command substitution.** The `D2` boundary at its three
   stated strengths — invariant, conservative check, no guarantee at all — with
   the rejecting example fence and the exact diagnostic text.
6. **The `shlex` guard.** The `D3` statement: the routes and recipe kinds it
   covers, both drift directions, and the explicit warning that passing the
   gate does not make a command safe.
7. **Stability and further reading.** A pre-1.0 caveat, because this section
   sits *above* `## Release and development status` and would otherwise make
   promises before the reader meets the hedge at `README.md:179-186`. Then
   pointers to `docs/users-guide.md#review-the-safety-boundary` and ADR-021.

Keep the split with the users' guide deliberate. The README owns the
placeholder contract and the non-guarantees; the users' guide keeps shell-route
mechanics such as command-list chaining, `exec`, and background jobs, and the
README does not restate them.

Then reduce the existing safety paragraph at `README.md:203-206` to a single
cross-referencing sentence that **keeps its link** to the users' guide safety
boundary.

Run the suite again and observe green.

### Stage D — correct, translate, refactor, and validate

EP-M2, then EP-M4, then EP-M5. Each is a separate commit. After each, run the
gates listed for that milestone. Re-read the whole new section once with fresh
eyes before EP-M4, because a translation multiplies any wording defect by six.

## Concrete steps

Run everything from the repository root,
`/home/leynos/.lody/repos/github---leynos---netsuke/worktrees/a52242fa-b09a-4ab9-8c9e-687ca8533644`.

1. Confirm the branch and freshness.

   ```sh
   git branch --show-current
   git fetch origin
   git log --oneline -1 origin/main
   ```

   Expect `4-4-1-document-command-placeholder-contract-in-readme` and a commit
   at or behind the branch point.

2. Re-verify the three facts.

   ```sh
   sed -n '150,235p' src/ir/cmd_interpolate/mod.rs
   sed -n '255,300p' src/ir/cmd_interpolate/mod.rs
   grep -n 'ir.invalid_command' locales/en-GB/messages.ftl
   ```

   Expect `has_unmatched_backticks` to be a `rem_euclid(2) != 0` parity test,
   `is_valid_command_for_shell` to return early for `RecipeShell::PowerShell`,
   `find_script_substitution` to call `try_match_dollar_placeholder`, and the
   Fluent line
   `ir.invalid_command = Invalid command interpolation: { $snippet }.`

3. Confirm the next free ADR number.

   ```sh
   ls docs/adr-*.md | sed 's/.*adr-0*\([0-9]*\).*/\1/' | sort -n | tail -1
   ```

   Expect `20`. If it is higher, use the next number and update every reference
   in this plan.

4. Write `docs/adr-021-command-placeholder-contract.md`, add its entry to
   `docs/contents.md` under `## Decision records` immediately above the ADR-020
   entry, and add the design-document reference. Commit as EP-M1.

   ```sh
   make check-fmt 2>&1 | tee /tmp/check-fmt-netsuke-$(git branch --show-current).out
   make markdownlint 2>&1 | tee /tmp/markdownlint-netsuke-$(git branch --show-current).out
   ```

5. Correct the three internal documents and any inaccurate doc comment, then
   confirm no unqualified claim survives outside historical documents. Commit
   as EP-M2. This precedes the README so the repository is never in a state
   where two normative documents disagree.

   ```sh
   grep -rn -e 'only Netsuke markers' -e 'remain unchanged' \
            -e 'remain shell variables' -e 'remain literal shell variables' \
            README.md docs/*.md src/ir/cmd_interpolate/ \
     | grep -v '^docs/adr-' | grep -v '^docs/archive/'
   ```

   Every surviving hit must be qualified by recipe kind, or be
   `docs/netsuke-design.md:290-291`, which is already correct and must not be
   changed. Revision 1's grep (`'literal .\$in\|\$in. and .\$out. remain'`)
   matched neither `docs/users-guide.md:1697`, whose claim wraps across a line
   break, nor the developers-guide phrasing, and it did hit
   `docs/adr-004-…:165`, which must not be edited.

6. Add `tests/readme_security_tests.rs` and the three identifiers to
   `EXPECTED_EXAMPLE_IDS`. Observe red.

   ```sh
   cargo nextest run --test readme_security_tests --test documentation_examples_tests \
     2>&1 | tee /tmp/red-netsuke-$(git branch --show-current).out
   ```

   Expect failures naming `readme-safe-placeholder-manifest`,
   `readme-quoted-path-manifest`, and `readme-backtick-rejection-manifest`,
   plus a registry-drift failure from
   `every_documented_fence_has_a_known_unique_identifier` listing exactly those
   three. Record the transcript in `Artefacts and notes`.

7. Write the README section with all three marked fences. Observe green, then
   run the full gates and commit as EP-M3.

   ```sh
   NETSUKE_REQUIRE_NINJA=1 cargo nextest run \
     --test readme_security_tests --test documentation_examples_tests \
     2>&1 | tee /tmp/green-netsuke-$(git branch --show-current).out
   ```

   `NETSUKE_REQUIRE_NINJA=1` is required. Without it the `ninja` guard at
   `tests/documentation_examples_e2e_tests.rs:52` returns `Ok(())` silently, so
   a green local run is not evidence that `OBL-PLACEHOLDERS` was exercised.
   Continuous integration sets it at `.github/workflows/ci.yml:78`.

8. **Only now** run the three negative controls, and record each transcript.
   Revision 1 placed these before the commit and restored with
   `git checkout -- README.md`, which would have deleted the entire
   newly-written section, because HEAD has no such section. Commit first;
   restore by copy, never by `git checkout`.

   ```sh
   MUT=docs/verification/mutations/ir__cmd_interpolate__property_tests__substituted_odd_backticks_are_rejected.patch

   # Control 1 (OBL-PLACEHOLDERS): a literal $in in the *command* example must
   # survive verbatim into the generated Ninja as $$in.
   cp README.md /tmp/readme-control.md
   # ...edit README.md, then:
   cargo nextest run --test readme_security_tests
   cp /tmp/readme-control.md README.md

   # Control 2 (OBL-RECIPE-KIND): remove the two try_match_dollar_placeholder
   # calls from find_script_substitution; the script rows must fail.
   cp src/ir/cmd_interpolate/mod.rs /tmp/cmd-interpolate-control.rs
   # ...edit, then:
   cargo nextest run --test readme_security_tests
   cp /tmp/cmd-interpolate-control.rs src/ir/cmd_interpolate/mod.rs

   # Control 3 (OBL-BACKTICK-REJECT): seed the stored parity fault.
   git apply "$MUT"
   cargo nextest run --test readme_security_tests
   git apply -R "$MUT"
   git status --porcelain
   ```

   Use `git status --porcelain`, not `git diff`, to confirm the tree is clean
   between controls: this repository sets `diff.external=difft`, so `git diff`
   output is a rendering rather than a machine signal. `git apply` itself is
   unaffected, being content-addressed. Two further nets catch a forgotten
   revert — the existing property test, and
   `tests/kani_mutation_evidence_tests.rs:9-10`, which re-runs
   `git apply --check` on every stored patch.

9. Translate into the six READMEs, add the recurring-obligation bullet to
   `docs/repository-layout.md`, then check parity. Commit as EP-M4.

   ```sh
   for f in README.md README.de.md README.es.md README.fr.md \
            README.ja.md README.pt-BR.md README.zh-CN.md; do
     printf '%s ' "$f"
     grep -c '^#\{1,6\} ' "$f"
   done
   for f in README.md README.de.md README.es.md README.fr.md \
            README.ja.md README.pt-BR.md README.zh-CN.md; do
     printf '%s: ' "$f"
     grep -o '^#\{1,6\} ' "$f" | tr -d ' ' | paste -sd,
   done
   ```

   Expect `13` for every file — they are 12 today — and an identical
   comma-delimited level sequence on every line. The delimiter matters:
   revision 1 used `tr -d ' \n'`, which collapsed the levels into one
   undelimited run of `#` characters and so could not distinguish
   `## A / ### B` from `### A / ## B`.

   Translate the table's column headers and the surrounding prose; leave the
   table's cells in English. Cells are code identifiers and yes/no, so a
   mistranslated cell becomes structurally impossible and a reviewer with no
   target-language reading can diff the table.

10. Mark roadmap 4.4.1 done and run the full gate sequence. Commit as EP-M5.
    Delegate this run to the `scrutineer` sub-agent rather than running it in
    the planning context.

    ```sh
    make check-fmt 2>&1 | tee /tmp/check-fmt-netsuke-$(git branch --show-current).out
    make typecheck 2>&1 | tee /tmp/typecheck-netsuke-$(git branch --show-current).out
    PATH="$HOME/go/bin:$PATH" make lint 2>&1 | tee /tmp/lint-netsuke-$(git branch --show-current).out
    make test 2>&1 | tee /tmp/test-netsuke-$(git branch --show-current).out
    make markdownlint 2>&1 | tee /tmp/markdownlint-netsuke-$(git branch --show-current).out
    make nixie 2>&1 | tee /tmp/nixie-netsuke-$(git branch --show-current).out
    ```

## Validation and acceptance

A reader can verify the outcome without reading any test:

- Open `README.md`. Between `## What works today` and
  `## Release and development status` there is a
  `## Security and command interpolation` section. It names `{{ ins }}`,
  `{{ outs }}`, and the script-only `$in` and `$out`. It says plainly that a
  backtick pair the author wrote is executed by the shell. It says which recipe
  kinds and which shells the `shlex` gate covers.
- Copy the section's rejected example into a `Netsukefile` and run `netsuke`.
  The output contains `Invalid command interpolation:` and no build runs.
- Copy the section's accepted example and run `netsuke`. It builds.
- Open any translated README. The same section is in the same place at the
  same heading level.

Quality criteria — what "done" means:

- Tests: `make test` passes. The five new cases in
  `tests/readme_security_tests.rs` pass, and each failed before the README
  section existed.
- Verification: `OBL-PLACEHOLDERS`, `OBL-BACKTICK-REJECT`, and
  `OBL-SHLEX-SCOPE` are discharged with both negative-control transcripts
  recorded. `OBL-STRUCTURAL-PARITY` is discharged by the step-10 output.
- Lint and typecheck: `make check-fmt`, `make typecheck`, `make lint`,
  `make markdownlint`, and `make nixie` all pass. If `actionlint` is missing,
  say so explicitly rather than reporting `make lint` clean.
- Performance: no threshold. The new tests add one Ninja-dependent case, which
  skips when `ninja` is absent.
- Security: the README section must not claim any protection the code does not
  provide. This is checked by the Stage A design review and re-checked at the
  EP-M3 conformance check.

Quality method: the gate sequence in `Concrete steps` step 10, delegated to
`scrutineer`, with each log retained under `/tmp` for inspection on failure.

## Idempotence and recovery

Every step is re-runnable. The gates are read-only except `make fmt`, which
rewrites Markdown formatting in place and is safe to repeat. The two negative
controls mutate the working tree and each is paired with an explicit revert;
run them one at a time and confirm `git status` is clean between them.

Each milestone is its own commit, so `git revert` restores a coherent state at
any plateau. Do not use bare `git stash` in this worktree: the stash stack is
shared with other checkouts. Set work aside with a temporary work-in-progress
commit instead.

If `make check-fmt` fails on files this plan did not touch, run `make fmt`,
inspect `git diff --stat`, and commit the unrelated reformatting separately
before continuing.

## Interfaces and dependencies

No production interface changes. No dependency changes.

New test file `tests/readme_security_tests.rs`. It must reuse the existing
harness rather than re-reading `README.md` itself:

```rust
mod documentation_examples;

use documentation_examples::{documented_example, manifest_workspace};
```

Its first line must be `mod documentation_examples;`. That declaration is
load-bearing beyond the import: `tests/integration_test_wiring_tests.rs`
enforces both that module trees are wired to Cargo test targets (`:150`) and
that Cargo discovers every top-level integration source (`:186`). No `[[test]]`
entry is required — `Cargo.toml` sets no `autotests = false`, so autodiscovery
covers the file.

The cases it must define, by name:

- `documented_safe_placeholder_manifest_builds`
- `placeholder_rewriting_differs_by_recipe_kind`
- `netsuke_owned_path_substitutions_are_quoted`
- `documented_backtick_manifest_is_rejected`
- `balanced_author_backticks_are_accepted`
- `odd_backtick_count_without_markers_is_rejected`
- `shlex_gate_applies_to_commands_not_scripts`

Decide deliberately whether the file is Unix-only. Its sibling
`tests/documentation_examples_e2e_tests.rs:3` carries `#![cfg(unix)]`, and
`.github/workflows/ci-windows.yml` does not set `NETSUKE_REQUIRE_NINJA`.

Assertions use `googletest` matchers with `rstest`, and `pretty_assertions` for
equality diffs, per `AGENTS.md`. Prefer `.expect(...)` over `.unwrap()` in test
bodies; the repository's lint exemption covers `#[test]` and `#[rstest]` bodies
only, not helpers outside them.

New identifiers added to `EXPECTED_EXAMPLE_IDS` in
`tests/documentation_examples_tests.rs`, in sorted position among the other
`readme-` entries:

- `readme-backtick-rejection-manifest`
- `readme-quoted-path-manifest`
- `readme-safe-placeholder-manifest`

Also add one bullet to `docs/repository-layout.md` near lines 60-65 recording
that a change to any README section must be mirrored in the six translations.
Nothing records this obligation today.

New document `docs/adr-021-command-placeholder-contract.md`. Its required
sections, in the order `docs/documentation-style-guide.md:374-384` mandates,
are **Status**, **Date**, and **Context and Problem Statement**. Add the
conditional sections **Decision Drivers**, **Options Considered**, and
**Decision Outcome** (lines 386-398), because three contested questions with
two or more defensible answers each is exactly the complexity those sections
exist for. Status is `Accepted` with today's date once the plan is approved. The
`arch-decision-records` skill's Y-Statement phrasing is a useful way to
compress each decision's rationale into a sentence, but it is not required by
this repository's style guide; use it inside **Decision Outcome** only if it
reads naturally.

## Progress

- [ ] EP-M1 — ADR-021 written, indexed, and referenced from the design
      document.
- [ ] EP-M2 — `docs/developers-guide.md`, `docs/users-guide.md`,
      `docs/formal-verification-methods-in-netsuke.md`, and any inaccurate doc
      comment corrected. Precedes the README deliberately.
- [ ] EP-M3 — README section and three executable examples; red observed
      before green; three negative controls recorded after the commit.
- [ ] EP-M4 — six translated READMEs regain structural parity;
      `docs/repository-layout.md` records the recurring obligation.
- [ ] EP-M5 — roadmap 4.4.1 marked done; full gate sequence green.

## Surprises & discoveries

- Observation: `$in` and `$out` **are** substituted in `script:` recipes,
  contradicting a plain reading of both `docs/developers-guide.md:3186-3190` and
  `docs/formal-verification-methods-in-netsuke.md:265-266`. Evidence:
  `find_script_substitution` and `try_match_dollar_placeholder`
  (`src/ir/cmd_interpolate/mod.rs:269-297`); `substitute_script` is reached from
  `src/ir/from_manifest_support.rs:114`. Impact: EP-M2 exists because of this.
  The README must state the placeholder set per recipe kind rather than as a
  single list.

- Observation: the backtick guard is a whole-string parity count, not a
  quoting-aware model, so a backtick inside single quotes still counts toward
  the total. Evidence: `has_unmatched_backticks`
  (`src/ir/cmd_interpolate/mod.rs:172-174`) is a single expression:

  ```rust
  s.chars().filter(|&c| c == '`').count().rem_euclid(2) != 0
  ```

  Impact: `D2` must describe this as a narrow structural guard. Describing it
  as "unmatched backticks are rejected" without qualification would imply a
  parser that does not exist.

- Observation: adding a fenced code block to `README.md` is a test change, not
  a prose change, because of the `tested-example` marker enforcement. Evidence:
  `tests/documentation_examples/mod.rs:120-175` and the exact-set registry
  check at `tests/documentation_examples_tests.rs:145-158`. Impact: this is
  what makes a genuine red-green cycle available for a documentation item, and
  it is why `Verification plan` has real obligations rather than a "no
  invariants introduced" disclaimer.

- Observation: `docs/roadmap.md` marks 4.2.3 complete while
  `docs/execplans/4-2-3-kani-harnesses-for-command-interpolation.md:8` still
  reads `Status: IN PROGRESS`. Evidence: both files, and the merge commit
  `6c646f1c` that landed the work. Impact: the prerequisite is met in substance
  — both Kani proofs and eleven Proptest properties exist and run. Flagged, not
  fixed; see `Risks`.

- Observation: footnote `[^8]` of
  `docs/formal-verification-methods-in-netsuke.md:332-333` cites
  `src/ir/cmd_interpolate.rs`, which no longer exists. Evidence: the module is
  now the directory `src/ir/cmd_interpolate/`. Impact: corrected in EP-M2.

- Observation: four factual errors in revision 1 of this plan, all found at
  design review. Evidence and correction: `assert_shell_command` is at
  `src/ninja_gen/mod.rs:283-290`, not `src/ninja_gen_recipe_shell.rs:282-290`;
  `shlex = "2.0.1"` (`Cargo.toml:138`) is a caret requirement, not a pin, and
  `README.md:110` installs without `--locked`; the registry test is
  `every_documented_fence_has_a_known_unique_identifier`
  (`tests/documentation_examples_tests.rs:143`), not
  `documented_example_registry_is_exhaustive`, which does not exist; and
  `docs/netsuke-design.md:290-291` already states Fact A correctly, so EP-M2
  must not touch it. Impact: an implementer told by Stage A to "open each cited
  line" would have found test code at the wrong `assert_shell_command` citation
  and could have triggered the contract-disagreement tolerance spuriously.

- Observation: `docs/users-guide.md:1697-1698` and `:1713-1715` also
  contradict Fact A, and revision 1 neither listed them for correction nor
  could detect them. Evidence: line 1697 reads "`{{ ins }}` and `{{ outs }}`
  are the only Netsuke markers for input and output paths"; revision 1's grep
  (`'literal .\$in\|\$in. and .\$out. remain'`) matches neither, because the
  claim wraps across a line break. Impact: the README's closing pointer sends
  readers to precisely that page. Three reviewers independently flagged this as
  blocking. EP-M2 now covers it and the grep in `Concrete steps` step 5 is
  rewritten.

- Observation: `UndefinedBehavior::Strict` (`src/manifest/mod.rs:128`) makes
  revision 1's negative control non-discriminating. Evidence: `ins` and `outs`
  are ordinary MiniJinja context entries (`src/manifest/render.rs:305-306`), so
  `{{ inputs }}` fails during rendering, before `find_substitution` is reached.
  Impact: the control proved only that MiniJinja is strict, and would have
  passed against an implementation recognizing a completely different marker
  set. Replaced with a control that discriminates.

- Observation: continuous integration sets `NETSUKE_REQUIRE_NINJA=1`
  (`.github/workflows/ci.yml:78`), which turns the `ninja` skip guard into a
  panic (`test_support/src/ninja.rs:73-89`). Impact: `OBL-PLACEHOLDERS` *is*
  discharged in CI, contrary to a worry raised at review — but a green local
  `make test` without the variable is not evidence, and revision 1 stated
  flatly that the case "skips when `ninja` is absent". The test commands now
  set it.

- Observation: the repository has no `SECURITY.md` in any conventional
  location. Evidence: checked at the repository root, `.github/`, and `docs/`.
  Impact: out of scope for 4.4.1, which asks specifically for a README section,
  but for a tool whose function is executing shell strings this is a genuine
  gap worth its own roadmap item. Recorded here so it is not lost.

## Decision log

- Decision `D1`: the supported placeholder set is `{{ ins }}` and `{{ outs }}`
  in both recipe kinds, plus `$in` and `$out` in `script:` recipes only.
  Rationale: this is what the code does (Fact A). The alternative — documenting
  the simpler, uniform set the existing prose implies — would be documenting a
  contract Netsuke does not honour, which is worse than documenting an
  irregular one. Answers `FV-CPC-Q1`. Date/Author: 2026-09-09, planning agent.
  Awaiting approval.

- Decision `D2`: the backtick handling is a deliberate, narrow structural
  guard over Netsuke-owned lowering, not a temporary subset of a
  command-substitution model. Rationale: calling it a temporary subset would
  imply an intent to widen it, and nothing in the roadmap commits to that.
  Calling it the full contract would imply it defends against author-written
  command substitution, which it does not. The honest third option is to scope
  the guarantee precisely to what it covers — markers — and to say explicitly
  what it does not cover. Answers `FV-CPC-Q2`. Date/Author: 2026-09-09,
  planning agent. Awaiting approval.

- Decision `D3`: `shlex::split` is part of the observable acceptance contract
  for `command:` recipes on POSIX and Bash routes, but the precise accepted set
  is not a stability commitment. Rationale: users can and do rely on the
  rejection, which has a stable, localized diagnostic, so denying that it is
  part of the contract would be false. But the accepted set is `shlex` 2.0.1's
  approximation of POSIX (`AX-SHLEX`), and pinning the project to it across
  future crate versions would be a commitment nobody has agreed to. Splitting
  the answer along the rejection/acceptance axis is more precise than either of
  the two options `FV-CPC-Q3` offers. Answers `FV-CPC-Q3`. Date/Author:
  2026-09-09, planning agent. Awaiting approval.

- Decision `D-SCOPE`: the plan corrects the two inaccurate upstream documents
  and records `D1`–`D3` in a new ADR, rather than writing the README section
  alone. Rationale: the user selected this scope when asked. It is also
  required by `AGENTS.md`, which mandates an ADR for a substantive decision and
  proactive correction of documentation that a change makes inaccurate. Leaving
  `docs/developers-guide.md` contradicting the new README would create the
  exact ambiguity 4.4.1 exists to remove. Date/Author: 2026-09-09, user and
  planning agent.

- Decision `D-TRANSLATE`: all six translated READMEs receive the section
  within this plan, as milestone EP-M4. Rationale: the user selected this. The
  seven READMEs currently have exact structural parity, and
  `docs/repository-layout.md:60-65` treats the translations as first-class
  editions rather than as a courtesy. A security-relevant section present only
  in English would be a meaningful gap for a non-English reader. Date/Author:
  2026-09-09, user and planning agent.

- Decision `D-NO-BDD`: the new coverage is `rstest` integration tests, not
  `rstest-bdd` scenarios. Rationale: the observable behaviour is "this exact
  published manifest is accepted / rejected with this diagnostic". A Gherkin
  scenario would restate the test name in prose without adding a
  stakeholder-legible workflow, and the repository already routes
  documented-example coverage through plain integration tests
  (`tests/documentation_examples_tests.rs`). Revisit if a reviewer identifies a
  workflow, rather than a fact, worth specifying. Date/Author: 2026-09-09,
  planning agent.

- Decision `D-NO-PROPTEST`: no new property test or Kani harness is added.
  Rationale: roadmap 4.2.3 already covers the invariant space with two Kani
  proofs and eleven Proptest properties
  (`src/ir/cmd_interpolate_property_tests.rs`). This item introduces no new
  invariant over generated inputs; it introduces a documentation–implementation
  agreement obligation, which exact examples plus negative controls discharge
  better than generated ones. Adding a redundant property test would be
  verification theatre. Recorded here because omitting property testing needs a
  reason, not silence. Date/Author: 2026-09-09, planning agent.

- Decision `D-NO-PORT`: no port or adapter is introduced.
  Rationale: `hexagonal-architecture` was consulted. `src/ir/cmd_interpolate/`
  is already domain policy returning a typed error, with the shell and Ninja
  adapters downstream; a review sweep confirmed it imports only `localization`,
  `camino`, `shell_quote`, `std::cell`, `std::collections`,
  `super::IrGenError`, and `crate::recipe_shell`, and performs no input or
  output. The boundary is correct; the plan documents it.

  One caveat the ADR must not gloss: `invalid_command_error`
  (`src/ir/cmd_interpolate/mod.rs:214-222`) calls `localization::message(...)`
  and stores a pre-rendered, locale-resolved human string inside the domain
  error. That is presentation resolved within domain policy against ambient
  state. It does not justify a port here, but ADR-021 must not cite this module
  as an exemplar of a clean domain boundary. Date/Author: 2026-09-09, planning
  agent; caveat added at design review.

- Decision `D-NO-PARITY-GATE`: structural parity of the README family is
  checked by `scripts/check-readme-parity.sh`, committed but wired into **no**
  gate. Rationale: revision 1 argued a committed test "would be a new
  repository-wide policy gate", which the review rightly called weak — the
  policy already exists at `docs/repository-layout.md:60-65`, and a test would
  only mechanize it. The real argument is the recurring tax: gating parity
  makes every future README edit a seven-file edit to stay green, which is a
  separate decision with a separate owner. A committed script that no gate runs
  gives any reviewer a reproducible check for roughly fifteen lines and changes
  no policy. Escalate if a reviewer wants it gated. Date/Author: 2026-09-09,
  planning agent; revised at design review.

- Decision `D-TABLE`: `D1` is presented as a table whose cells stay in English
  while its column headers and surrounding prose are translated. Rationale: the
  placeholder contract is two-dimensional (placeholder × recipe kind), so a
  table is shorter and clearer than the two prose blocks revision 1 planned.
  Cells that are code identifiers and yes/no make a mistranslation structurally
  impossible and let a reviewer with no target-language reading diff all seven
  editions. The countervailing risk — that a table reads as reassurance — is
  handled by placing the non-guarantees above it. Date/Author: 2026-09-09,
  design review.

- Decision `D-M2-FIRST`: EP-M2 (document corrections) precedes EP-M3 (the
  README). Rationale: revision 1 had the README land first, creating a plateau
  at which `README.md` and `docs/users-guide.md` state opposite things about
  `$in` in a `script:` — the exact ambiguity `D-SCOPE` argues 4.4.1 exists to
  remove. The milestones are independent, so the reorder costs nothing.
  Date/Author: 2026-09-09, design review.

- Decision `D-CONTROLS-AFTER-COMMIT`: the negative controls run after EP-M3 is
  committed, and restore by file copy rather than `git checkout`. Rationale:
  revision 1 ran them before the commit and restored with
  `git checkout -- README.md`, which would have deleted the entire
  newly-written section because HEAD has no such section. The tests would have
  failed loudly afterwards, so it was detectable — but the most expensive
  artefact in the plan would already have been lost. Date/Author: 2026-09-09,
  design review.

- Decision `D1-LEGACY`: the `script:`-only `$in` / `$out` forms are documented
  as retained legacy behaviour, with an explicit shadowing warning and a steer
  towards `{{ ins }}` / `{{ outs }}`, rather than as a blessed feature.
  Rationale: the evidence points both ways and the plan must not silently pick
  one. *Intended:* three `#[cfg(unix)]` end-to-end cases exercise the quoted
  contexts (`tests/ninja_dollar_escaping_tests.rs:361-393`). *Vestigial:* the
  users' guide already tells authors to migrate away
  (`docs/users-guide.md:1713-1715`); the Ninja-escaping case covering the
  command-recipe pass-through is named `legacy_marker_aliases`
  (`tests/ninja_dollar_escaping_tests.rs:207`); and the Kani harness doc
  comment asserts "Literal `$in` and `$out` must remain shell text"
  (`src/ir/cmd_interpolate/verification.rs:4`). Documenting current behaviour
  plus a direction of travel is honest under either reading and commits the
  project to neither.

  **This decision needs the owner's confirmation.** If the intent is that these
  forms be withdrawn before 1.0, the README should say so. If they are
  supported indefinitely, the shadowing hazard deserves a diagnostic rather
  than a footnote — and that would be a code change outside this item.
  Date/Author: 2026-09-09, design review. Awaiting approval.

- Decision `D-4-2-3-STATUS`: the 4.2.3 execplan's stale `IN PROGRESS` header is
  flagged and left unedited. Rationale: a merged execplan is a historical
  document, and the substantive prerequisite is verifiably met. Editing another
  item's completion record from this branch would obscure history. Date/Author:
  2026-09-09, planning agent.

## Outcomes & retrospective

Not yet started. To be completed at EP-M5.

Before setting this plan to `COMPLETE`, reconcile each entry in
`Surprises & discoveries` against the artefacts in `Conformance basis`: confirm
that EP-M2 corrected the Fact A misstatements in both documents and the `[^8]`
footnote, that `docs/formal-verification-methods-in-netsuke.md` records
`FV-CPC-Q1` through `FV-CPC-Q3` as answered with a pointer to ADR-021, and that
the 4.2.3 status discrepancy is either resolved elsewhere or recorded as
knowingly deferred.

## Artefacts and notes

To be populated during implementation. At minimum, retain:

- the red transcript from `Concrete steps` step 5, showing both
  missing-identifier failures and the registry-drift failure;
- the green transcript from step 6;
- both negative-control transcripts from step 7;
- the parity output from step 10;
- the gate summary from step 11, naming any gate that did not run.

Reference material gathered during planning, for the writer's use:

- Ninja's manual, *Interpretation of the `command` variable*: on Unix the
  `command` string is passed to `sh -c`; on Windows it is passed to
  `CreateProcess`. This is the source of `AX-NINJA-SH` and the reason the
  README must attribute shell interpretation to Ninja rather than to Netsuke.
- `shlex`'s crate documentation, *Compatibility*: the crate targets any
  POSIX-compatible shell and also aims to match Python's `shlex` and C's
  `wordexp`. Its `Shlex` iterator splits words; it performs no expansion, so a
  backtick is an ordinary character to it. This is why `shlex::split`
  succeeding says nothing about whether a shell would run a command
  substitution in the same text — a point the README should make, because it is
  the most likely misreading of `D3`.
- `RUSTSEC-2024-0006` and the `shlex::quoting_warning` module: the crate's
  `quote` family cannot portably escape control characters, and versions before
  1.3.0 failed to quote `{` and `\xa0`. Netsuke quotes with `shell-quote`, not
  `shlex`, so the advisory does not apply to Netsuke's quoting path — but it is
  worth citing in ADR-021 as evidence for why `D3` declines to make the
  accepted set a stability commitment.

## Revision note

Revision 2 (2026-09-09). Incorporates a six-lens `logisphere-design-review`
pass. Blocking changes: EP-M2 (document corrections) now precedes the README
milestone and covers `docs/users-guide.md:1697,1713`, removing a plateau at
which two normative documents would contradict each other; the negative
controls now run after the EP-M3 commit and restore by copy, because revision
1's `git checkout -- README.md` would have deleted the section it had just
written; `OBL-PLACEHOLDERS`' control was replaced because
`UndefinedBehavior::Strict` made the original non-discriminating; two
obligations were added, `OBL-RECIPE-KIND` for the plan's own headline finding
and `OBL-QUOTING` for the section's only positive security promise, neither of
which had any test in revision 1; and four factual errors were corrected (see
`Surprises & discoveries`).

Substantive rewording: `D2` now promises the marker invariant and the parity
check at different strengths, so a future quoting-aware fix is not a breaking
change; `D3` names both `shlex` drift directions and states that passing the
gate does not make a command safe; `D1` frames the `script:`-only `$in`/`$out`
forms as retained legacy with a shadowing warning, and raises `D1-LEGACY` for
the owner. New decisions: `D-TABLE`, `D-M2-FIRST`, `D-CONTROLS-AFTER-COMMIT`,
`D1-LEGACY`; `D-NO-PARITY-GATE` keeps its outcome but replaces its rationale
and now commits an ungated script. The README section order was inverted to put
the live hazards before the placeholder table, and the templated-value
injection vector was promoted out of a bullet list into its own labelled part.
Scope tolerance was raised from 14 files / 700 lines to 18 / 1200, because the
review showed the original would fire on the plan's own expected path.

Revision 1 (2026-09-09). Initial draft. Established the three contract
decisions from a direct reading of `src/ir/cmd_interpolate/` and scoped the
work to five milestones.

Remaining work: all of it; the plan awaits approval before any implementation
begins. `D1-LEGACY` additionally awaits the owner's confirmation, though it is
written so that either answer leaves the documentation honest.
