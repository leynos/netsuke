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
- Two internal documents that currently misstate the contract are corrected.

You can see it working by running `make test` and observing the new cases
`documentation_examples_tests::documented_example_registry_is_exhaustive`,
`readme_security_tests::documented_safe_placeholder_manifest_builds`, and
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
`assert_shell_command` (`src/ninja_gen_recipe_shell.rs:282-290`). The
dependency is pinned at `shlex = "2.0.1"` (`Cargo.toml:138`).

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
RM-4.4.1.a -> EP-M2 -> README.md "Security and command interpolation"
RM-4.4.1.b / FV-CPC-Q1 -> D1 -> EP-M1 (ADR-021 §Placeholders) -> EP-M2 -> tests::readme_security::documented_safe_placeholder_manifest_builds
RM-4.4.1.c / FV-CPC-Q2 -> D2 -> EP-M1 (ADR-021 §Backticks)   -> EP-M2 -> tests::readme_security::documented_backtick_manifest_is_rejected
RM-4.4.1.d / FV-CPC-Q3 -> D3 -> EP-M1 (ADR-021 §shlex guard) -> EP-M2 -> tests::readme_security::documented_backtick_manifest_is_rejected
Fact A     -> EP-M3 -> docs/developers-guide.md "Command interpolation contract"
Fact A     -> EP-M3 -> docs/formal-verification-methods-in-netsuke.md FV-CPC
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
   other than a doc comment correction explicitly authorized by EP-M3. This is
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
   the explicitly declared EP-M2 → EP-M4 window.
5. **Prose rules.** en-GB Oxford spelling; prose wrapped at 80 columns; code
   fences at most 120 columns; `-` bullets; sentence-case headings; no skipped
   heading levels; a language identifier on every fence. Backtick any
   identifier whose spelling would otherwise trip the `typos` gate.
6. **No new dependency.** Nothing is added to `Cargo.toml`.
7. **No claim of proof beyond what exists.** Per
   `docs/adr-004-bound-kani-ir-harnesses-to-small-n.md:167-177`, Kani proves
   the marker recognizer only. The README and ADR must not describe the
   backtick guard or the `shlex` guard as *proved*; they are property-tested.
8. **Do not weaken `docs/users-guide.md`.** The README summarizes and links;
   the users' guide remains the detailed reference.

## Tolerances (exception triggers)

Stop and escalate when any threshold is reached. Do not work around them.

- **Scope.** More than fourteen files changed, or more than 700 net added
  lines across the whole plan. Expected: eleven files (`README.md`, six
  translations, `docs/adr-021-…`, `docs/contents.md`,
  `docs/developers-guide.md`, `docs/formal-verification-methods-in-netsuke.md`,
  `docs/netsuke-design.md`, `docs/roadmap.md`,
  `tests/documentation_examples_tests.rs`, and one new test file) — thirteen
  paths.
- **Production code.** Any change to `src/` beyond the two doc-comment
  corrections named in EP-M3. Immediate stop.
- **Contract disagreement.** If a new test shows the implementation does not
  behave as `Context and orientation` states, stop. The plan's facts are wrong,
  or the code has a defect; either way a human decides.
- **Interface.** Any change to a public API signature, or to
  `EXPECTED_EXAMPLE_IDS` beyond adding the two new identifiers.
- **Dependencies.** Any addition to `Cargo.toml`. Immediate stop.
- **Iterations.** A gate that still fails after three fix attempts.
- **Translation confidence.** If the writer cannot render a security claim in
  a target language with confidence that the meaning is preserved, stop at
  EP-M4 and escalate rather than shipping an approximate security statement. A
  wrong translation of a safety boundary is worse than an absent one.
- **Ambiguity.** Any point where `FV-CPC-Q2` or `FV-CPC-Q3` could reasonably
  be settled the other way and the choice changes what the README promises.

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
- Domain: the two fenced examples in the new README section, extracted at run
  time by `documented_example`.
- Artefact: `tests/readme_security_tests.rs`, case
  `documented_safe_placeholder_manifest_builds`.
- Evidence: `cargo nextest run --test readme_security_tests`. Red before the
  README section exists, because
  `documented_example("readme-safe-placeholder- manifest")` returns an error
  whose context is
  `documented example 'readme-safe-placeholder-manifest' should exist`. Green
  once the section and the registry entry are added. Discharged when the test
  passes and `documented_example_registry_is_exhaustive` also passes.
- Non-vacuity: the test asserts on observable output, not merely on a
  non-error exit. It requires that the built artefact contains the expected
  text and that the generated Ninja command still contains a literal `$PATH`
  after Netsuke's `$` → `$$` Ninja escaping is reversed. A negative control:
  change the example's `{{ ins }}` to `{{ inputs }}` and the build must fail
  because the marker is not recognized. Run that control once by hand and
  record the transcript in `Artefacts and notes`; do not commit it.

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
- Domain: the documented rejecting example, plus two hand-written controls in
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
- Artefact: the command in `Concrete steps` step 10.
- Evidence: the command prints `7` identical counts and a matching level
  sequence.
- Non-vacuity: the check compares the level sequence, not just the count, so
  swapping an `##` for a `###` is caught.

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

### EP-M2 — the README states the contract, executably

- Outcome: `README.md` carries a new `## Security and command interpolation`
  section, placed after `## What works today` and before
  `## Release and development status`, delimited by the repository's
  seventy-underscore thematic breaks. It contains two marked, executable fenced
  examples. `tests/documentation_examples_tests.rs` registers the two new
  identifiers. `tests/readme_security_tests.rs` exists and discharges
  `OBL-PLACEHOLDERS`, `OBL-BACKTICK-REJECT`, and `OBL-SHLEX-SCOPE`. The
  existing safety paragraph in `## Release and development status` is reduced
  to a one-line cross-reference so the two do not contradict each other.
- Requirements discharged: `RM-4.4.1.a`, `.b`, `.c`, `.d`.
- Acceptance evidence: `make check-fmt`, `make markdownlint`, `make lint`,
  `make test` all pass. The three new tests fail before the README section
  exists and pass after. Both negative controls behave as `Verification plan`
  requires.
- Conformance check: every claim traces to a cited line or a new test; the
  section leads with the boundary rather than the reassurance; `D1`–`D3` in the
  ADR and the README prose agree word-for-word on the three contested points.
- Recovery: revert the commit. `README.md` and the two test files are the only
  changes; nothing else depends on them.
- Remaining gaps: two internal documents still misstate Fact A; six
  translations lack the section.
- Compatibility decision: none. `EXPECTED_EXAMPLE_IDS` is a test-only,
  crate-internal constant; adding to it needs no shim.

### EP-M3 — internal documents corrected

- Outcome: `docs/developers-guide.md` §*Command interpolation contract* states
  the per-recipe-kind placeholder set correctly.
  `docs/formal-verification-methods-in-netsuke.md` §*Command placeholder
  contract* records that its three questions are now answered and points at
  ADR-021; §*Kani for command interpolation* is corrected the same way; the
  `[^8]` footnote path is corrected to `src/ir/cmd_interpolate/mod.rs`.
  `docs/netsuke-design.md` is checked and corrected only if it also misstates
  Fact A. Where a doc comment in `src/ir/cmd_interpolate/mod.rs` is itself
  inaccurate about scripts, correct the comment text only — no code.
- Requirements advanced: Fact A consistency; `RM-4.4.1` completeness.
- Acceptance evidence: `grep -n '\$in' docs/*.md src/ir/cmd_interpolate/mod.rs`
  shows no remaining statement that `$in` is universally left unchanged.
  `make check-fmt`, `make markdownlint`, `make lint`, `make test` pass —
  `make lint` matters here because a doc-comment edit recompiles.
- Conformance check: no behaviour changed; `git diff --stat src/` shows only
  comment lines; the corrected statements agree with EP-M2's README text.
- Recovery: revert the commit; the edits are independent of EP-M2.
- Remaining gaps: translations.
- Compatibility decision: none.

### EP-M4 — the translated READMEs regain parity

- Outcome: all six translated READMEs carry the equivalent section in the same
  position, with the same heading level, the same bullet order, and byte-
  identical code fences. `README.md` is unchanged by this milestone.
- Requirements discharged: `RM-4.4.1.a` across the README family;
  `OBL-STRUCTURAL-PARITY`.
- Acceptance evidence: the parity command in `Concrete steps` step 10 reports
  identical heading structures across all seven files. `make check-fmt` and
  `make markdownlint` pass. The translated files remain outside the `typos`
  scope, so the spelling gate is unaffected.
- Conformance check: no translated file gained a `tested-example` marker;
  terminology matches `docs/localization-glossary.md` for each locale; no
  security claim was softened or strengthened in translation.
- Recovery: revert the commit. English parity is restored trivially because
  EP-M2 did not depend on the translations.
- Remaining gaps: the roadmap entry is still open.
- Compatibility decision: none.

### EP-M5 — roadmap closed and gates green

- Outcome: `docs/roadmap.md` item 4.4.1 and its four sub-items are marked
  `[x]`, with a short completion note in the repository's established style
  recording that the contract was settled in ADR-021 and that translations
  landed. All gates pass on the full branch.
- Requirements discharged: `RM-4.4.1`.
- Acceptance evidence: the full gate sequence in `Concrete steps` step 11, with
  each log file retained.
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

Then settle the three questions. The recommended answers, to be confirmed at the
`logisphere-design-review` checkpoint below:

- **`D1` (answers `FV-CPC-Q1`).** These are the only supported placeholders:
  `{{ ins }}` and `{{ outs }}` in both `command:` and `script:` recipes, and
  `$in` and `$out` in `script:` recipes only, matched at identifier boundaries.
  Everything else is literal text for the shell. Netsuke does not expose
  Ninja's own `$in` / `$out` rule variables, because it bakes resolved paths
  into each content-hashed rule.
- **`D2` (answers `FV-CPC-Q2`).** The backtick handling is a **deliberate,
  narrow structural guard over Netsuke-owned lowering**, not a model of shell
  command substitution and not a subset that a future release is committed to
  widening. Netsuke guarantees exactly two things: a Netsuke marker never
  survives into a backtick or `$( … )` region, and a `command:` recipe whose
  substituted text has an odd total backtick count is rejected. Netsuke
  guarantees nothing about author-written backticks that contain no marker;
  those reach the shell and execute. PowerShell is outside both checks because
  its backtick is an escape character.
- **`D3` (answers `FV-CPC-Q3`).** `shlex::split` **is** part of the semantic
  acceptance contract in the observable sense: a `command:` recipe on the POSIX
  or Bash route whose substituted text cannot be split is rejected, with a
  stable, localized diagnostic, at IR-lowering time. It is **not** a stability
  commitment about the precise accepted set, because that set is a third-party
  approximation of POSIX (`AX-SHLEX`) and may shift with the crate. It does not
  apply to `script:` recipes or to PowerShell, and Netsuke never executes the
  tokens it produces.

Go/no-go: run the `logisphere-design-review` skill over `D1`–`D3` and the draft
section outline. Proceed only if the review does not find that the wording
could be read as a guarantee Netsuke does not make.

### Stage B — red

Add `tests/readme_security_tests.rs` with the five cases named in
`Verification plan`, referencing the two documented-example identifiers that do
not yet exist. Run the suite and observe each case failing with
`documented example '…' should exist`. This is the red stage and it must be
observed, not assumed. Do not use an expected-failure marker; Rust has no strict
`xfail` and the missing-identifier error is already specific enough to prove
the test fails for the intended reason.

Also add the two identifiers to `EXPECTED_EXAMPLE_IDS` *before* adding the
README fences, and observe `documented_example_registry_is_exhaustive` fail
with a registry-drift message naming exactly the two missing identifiers. That
is a second, sharper red signal.

### Stage C — green

Write the README section (EP-M2). The section's shape, in order:

1. A one-paragraph framing: a `Netsukefile` executes commands, so treat it as
   you would a `Makefile`; Netsuke narrows some mistakes but is not a sandbox
   and does not sanitize shell text you write.
2. **What Netsuke rewrites** — the `D1` placeholder table or bullet list, split
   by recipe kind, with the accepted example fence.
3. **What Netsuke leaves alone** — `$PATH`, `$ins`, `$outs`, `$input`,
   `$output`, arbitrary Jinja values, and handwritten shell fragments.
4. **What Netsuke quotes** — only its own path substitutions, via
   `shell-quote`, encoded for the surrounding quote context.
5. **Backticks and command substitution** — the `D2` boundary, with the
   rejected example fence and the exact diagnostic text.
6. **The `shlex` guard** — the `D3` statement, naming the routes it covers and
   the routes it does not.
7. A closing pointer to `docs/users-guide.md#review-the-safety-boundary` and to
   ADR-021.

Then reduce the existing safety paragraph in
`## Release and development status` to a single cross-referencing sentence.

Run the suite again and observe green.

### Stage D — correct, translate, refactor, and validate

EP-M3, then EP-M4, then EP-M5. Each is a separate commit. After each, run the
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

5. Add `tests/readme_security_tests.rs` and the two identifiers to
   `EXPECTED_EXAMPLE_IDS`. Observe red.

   ```sh
   cargo nextest run --test readme_security_tests --test documentation_examples_tests \
     2>&1 | tee /tmp/red-netsuke-$(git branch --show-current).out
   ```

   Expect failures naming `readme-safe-placeholder-manifest` and
   `readme-backtick-rejection-manifest`, and a registry-drift failure listing
   exactly those two identifiers. Record the transcript in
   `Artefacts and notes`.

6. Write the README section with both marked fences. Observe green.

   ```sh
   cargo nextest run --test readme_security_tests --test documentation_examples_tests \
     2>&1 | tee /tmp/green-netsuke-$(git branch --show-current).out
   ```

7. Run the two negative controls by hand, record both transcripts, and revert
   each afterwards.

   ```sh
   # Control 1: break the accepted example's marker and expect a build failure.
   # Edit README.md, change {{ ins }} to {{ inputs }} in the safe example, then:
   cargo nextest run --test readme_security_tests
   git checkout -- README.md

   # Control 2: seed the parity fault and expect the odd-backtick case to fail.
   git apply docs/verification/mutations/ir__cmd_interpolate__property_tests__substituted_odd_backticks_are_rejected.patch
   cargo nextest run --test readme_security_tests
   git apply -R docs/verification/mutations/ir__cmd_interpolate__property_tests__substituted_odd_backticks_are_rejected.patch
   ```

8. Reduce the duplicated safety paragraph in
   `## Release and development status` to a cross-reference. Run the full gates
   and commit as EP-M2.

9. Correct the internal documents and any inaccurate doc comment. Commit as
   EP-M3.

   ```sh
   grep -rn 'literal .\$in\|\$in. and .\$out. remain' docs/ src/
   ```

   Expect no remaining unqualified claim that `$in` is always left unchanged.

10. Translate into the six READMEs, then check parity. Commit as EP-M4.

    ```sh
    for f in README.md README.de.md README.es.md README.fr.md \
             README.ja.md README.pt-BR.md README.zh-CN.md; do
      printf '%s ' "$f"
      grep -c '^#\{1,3\} ' "$f"
    done
    for f in README.md README.de.md README.es.md README.fr.md \
             README.ja.md README.pt-BR.md README.zh-CN.md; do
      printf '%s: ' "$f"
      grep -o '^#\{1,3\} ' "$f" | tr -d ' \n'
      echo
    done
    ```

    Expect `13` for every file, and an identical `#`-level sequence on every
    line.

11. Mark roadmap 4.4.1 done and run the full gate sequence. Commit as EP-M5.
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
  EP-M2 conformance check.

Quality method: the gate sequence in `Concrete steps` step 11, delegated to
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

The cases it must define, by name:

- `documented_safe_placeholder_manifest_builds`
- `documented_backtick_manifest_is_rejected`
- `balanced_author_backticks_are_accepted`
- `odd_backtick_count_without_markers_is_rejected`
- `shlex_gate_applies_to_commands_not_scripts`

Assertions use `googletest` matchers with `rstest`, and `pretty_assertions` for
equality diffs, per `AGENTS.md`. Prefer `.expect(...)` over `.unwrap()` in test
bodies; the repository's lint exemption covers `#[test]` and `#[rstest]` bodies
only, not helpers outside them.

New identifiers added to `EXPECTED_EXAMPLE_IDS` in
`tests/documentation_examples_tests.rs`, in sorted position among the other
`readme-` entries:

- `readme-backtick-rejection-manifest`
- `readme-safe-placeholder-manifest`

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
- [ ] EP-M2 — README section and executable examples; red observed before
      green.
- [ ] EP-M3 — `docs/developers-guide.md`,
      `docs/formal-verification-methods-in-netsuke.md`, and any inaccurate doc
      comment corrected.
- [ ] EP-M4 — six translated READMEs regain structural parity.
- [ ] EP-M5 — roadmap 4.4.1 marked done; full gate sequence green.

## Surprises & discoveries

- Observation: `$in` and `$out` **are** substituted in `script:` recipes,
  contradicting a plain reading of both `docs/developers-guide.md:3186-3190` and
  `docs/formal-verification-methods-in-netsuke.md:265-266`. Evidence:
  `find_script_substitution` and `try_match_dollar_placeholder`
  (`src/ir/cmd_interpolate/mod.rs:269-297`); `substitute_script` is reached from
  `src/ir/from_manifest_support.rs:114`. Impact: EP-M3 exists because of this.
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
  now the directory `src/ir/cmd_interpolate/`. Impact: corrected in EP-M3.

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
  adapters downstream. The boundary is correct; the plan documents it. See
  `Conformance basis`. Date/Author: 2026-09-09, planning agent.

- Decision `D-NO-PARITY-GATE`: structural parity of the README family is
  verified by a recorded command, not by a committed test. Rationale: a
  committed parity test would be a new repository-wide policy gate firing on
  unrelated future work, which exceeds 4.4.1's mandate. Escalate if a reviewer
  wants the gate. Date/Author: 2026-09-09, planning agent.

- Decision `D-4-2-3-STATUS`: the 4.2.3 execplan's stale `IN PROGRESS` header is
  flagged and left unedited. Rationale: a merged execplan is a historical
  document, and the substantive prerequisite is verifiably met. Editing another
  item's completion record from this branch would obscure history. Date/Author:
  2026-09-09, planning agent.

## Outcomes & retrospective

Not yet started. To be completed at EP-M5.

Before setting this plan to `COMPLETE`, reconcile each entry in
`Surprises & discoveries` against the artefacts in `Conformance basis`: confirm
that EP-M3 corrected the Fact A misstatements in both documents and the `[^8]`
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

Revision 1 (2026-09-09). Initial draft. Establishes the three contract decisions
`D1`–`D3` from a direct reading of `src/ir/cmd_interpolate/`, scopes the work
to five milestones covering the ADR, the README section with executable
examples, correction of two inaccurate internal documents, six translations,
and the roadmap closure. Remaining work: all of it; the plan awaits approval
before any implementation begins.
