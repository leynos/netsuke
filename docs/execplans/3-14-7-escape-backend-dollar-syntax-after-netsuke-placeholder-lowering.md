# Escape backend dollar syntax after Netsuke placeholder lowering (roadmap 3.14.7)

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`,
`Decision log`, `Outcomes & retrospective`, `Conformance basis`, and
`Verification plan` must be kept up to date as work proceeds.

Status: IN PROGRESS

There is no `PLANS.md` in this repository; `docs/execplans/` is the plan
directory and `docs/roadmap.md` is the work index.

## Purpose / big picture

Netsuke compiles a YAML manifest into a `build.ninja` file and then runs
Ninja. Ninja's file format uses `$` as its own escape character. A literal
dollar must be written `$$`; a dollar followed by a space means a literal
space; `$:` means a literal
colon; `$` followed by an identifier is a variable reference; and `$` followed
by anything else is a syntax error. Netsuke does not currently perform that
escaping, so a manifest author cannot write ordinary shell text.

Two distinct failures happen today, and they are not the same failure.

The first is silent erasure. A recipe `echo PATH is $PATH` reaches the
generated file verbatim. Ninja lexes `$PATH` as a reference to a Ninja variable
named `PATH`, finds none, substitutes the empty string, and the shell never
sees the variable at all.

The second is a hard parse failure. A recipe `echo ${CARGO:-cargo}` produces a
`build.ninja` that Ninja refuses to read, because `${CARGO:-cargo}` is not a
valid Ninja variable reference. The build stops with
`bad $-escape (literal $ must be written as $$)`.

Both were reproduced against `ninja` 1.11.1 on this machine; transcripts are in
`Artefacts and notes`.

After this change, a manifest author writes ordinary shell. `$PATH`,
`${CARGO:-cargo}`, `$RUSTFLAGS`, and `$$` mean what they mean in a shell,
because the Ninja backend escapes residual literal dollars as `$$` at the
moment it writes the file. The intermediate representation (IR) — Netsuke's
backend-agnostic build graph — keeps plain shell text and gains no
Ninja-specific escaping, so a future non-Ninja backend is unaffected.

You can see it working like this. Given a manifest whose recipe is
`echo "${NETSUKE_DEMO:-fallback}" > out.txt`, running `netsuke` with
`NETSUKE_DEMO` unset writes `fallback` into `out.txt`; running it with
`NETSUKE_DEMO=hello` writes `hello`. Today the first case fails to build at
all.

## Constraints

These are hard invariants. If satisfying the objective requires violating one,
stop, record the conflict in `Decision log`, and escalate.

1. The IR must remain free of Ninja-specific escaping. `src/ir/graph.rs` and
   everything it holds continue to carry plain shell text. This is mandated by
   `docs/netsuke-design.md` §2.6 ("This is a backend concern, not an IR
   concern") and restated in `docs/developers-guide.md` under
   "Command and recipe lowering".
2. Escaping must be applied exactly once, to the fully assembled backend
   command line, strictly after all Netsuke placeholder lowering. Applying it
   before lowering, or twice, is a defect the type system must prevent rather
   than a review finding.
3. `docs/netsuke-design.md` §§2.6 and 5.4 are the approved design for this
   work. Do not silently diverge. A divergence requires an entry in
   `Decision log`, a design-document update, and — if architectural — an
   Architectural Decision Record (ADR).
4. No new external crate dependencies. Everything needed is already present:
   `shlex`, `shell_quote`, `rstest`, `rstest-bdd`, `googletest`,
   `pretty_assertions`, `insta`, `proptest`.
5. No source file may exceed 400 lines (`AGENTS.md`, "Keep file size
   manageable"). The existing `src/ninja_gen.rs` is close to that limit, so new
   code goes in a new sibling module.
6. All prose is en-GB with Oxford spelling. All documentation follows
   `docs/documentation-style-guide.md`.
7. Every milestone ends with `make check-fmt`, `make typecheck`, `make lint`,
   and `make test` passing, plus `make markdownlint` when Markdown changed.
8. Roadmap item 3.14.7 is ticked only after the final gate run passes.

## Tolerances (exception triggers)

Stop and escalate rather than improvising when any of these is reached.

1. Scope: more than 25 files touched, or more than 900 net added lines of
   non-test code.
2. Interface: any change to a `pub` item outside the crate, or any change to
   the `ir::Action` or `ir::BuildEdge` field set. Neither should be needed.
3. Dependencies: any new entry in `Cargo.toml`.
4. Iterations: a milestone's gate fails three times in a row without the
   failure mode changing.
5. Ambiguity: the two decisions marked `NEEDS APPROVAL` in `Decision log`
   (backtick regions, and the description/metadata scope boundary) must be
   settled before the milestone that depends on them starts.
6. Behaviour: if any change would alter generated Ninja for a manifest that
   contains no `$` anywhere, stop. Dollar-free manifests must produce
   byte-identical output.

## Risks

1. Risk: extending `$in`/`$out` lowering to `script:` recipes routes script
   text through `interpolate_command_with_bindings`
   (`src/ir/cmd_interpolate.rs:101-117`), which rejects any text that
   `shlex::split` cannot parse. Multi-line scripts containing heredocs,
   apostrophes in comments, or `case` statements would start failing to build.
   Severity: high. Likelihood: high.
   Mitigation: scripts use substitution only. Add a distinct entry point that
   calls `substitute` without the command-shaped validation, and add explicit
   regression cases (B5 in the test matrix) covering a heredoc and an
   apostrophe.

2. Risk: `substitute` (`src/ir/cmd_interpolate.rs:235-263`) deliberately
   preserves backtick-delimited regions, so `$in`/`$out` inside backticks is
   not lowered. Today Ninja expands it by accident; after escaping it becomes a
   literal `$in` reaching the shell, silently producing an empty result rather
   than an error.
   Severity: high. Likelihood: medium.
   Mitigation: this is decision `D-BACKTICK` below and must be settled before
   milestone EP-M1. The recommended resolution is a typed diagnostic rather
   than silent divergence.

3. Risk: escaping command text while leaving path emission unescaped converts
   a consistently-wrong system into an inconsistently-wrong one. `join`
   (`src/ninja_gen.rs:166-168`) writes paths raw, while `quote_paths`
   (`src/ir/cmd_interpolate.rs:63-78`) shell-quotes the same path into the
   command. For a source named `input$1`, Ninja would then believe the
   dependency is `input` while the command reads `input$1`.
   Severity: high. Likelihood: medium — `tests/command_escaping_tests.rs:55`
   already uses `input$1` as a fixture path.
   Mitigation: milestone EP-M3 makes path emission fallible and rejects
   `$`, space, colon, and control characters with a typed error, so the
   inconsistency becomes a clear diagnostic instead of a corrupt build file.

4. Risk: a scalar `command:` containing a newline currently writes raw Ninja
   syntax into the generated file. `shlex::split` treats `\n` as whitespace and
   returns `Some`, so neither
   `interpolate_command_with_bindings` nor the `assert_shell_command`
   debug guard (`src/ninja_gen.rs:259-266`) rejects it. Command *lists* are
   guarded by `has_ninja_control_character`
   (`src/ninja_gen_command_list.rs:352`); scalar commands and descriptions are
   not.
   Severity: high (it is a build-file injection). Likelihood: low in practice.
   Mitigation: the new escaping constructor is fallible and rejects control
   characters, closing the hole at the same seam. Shipping "dollars are safe
   now" while newlines inject raw Ninja would read as complete when it is not.

5. Risk: lowering `$in`/`$out` in scripts embeds per-target paths into script
   text, so script actions stop deduplicating. Today `N` targets sharing one
   script rule collapse to one action via the `contains_key` guard at
   `src/ir/from_manifest_support.rs:76-78`; afterwards they become `N` actions
   and `N` rules.
   Severity: medium. Likelihood: certain.
   Mitigation: accept and document. Command recipes already behave this way
   (`src/ir/from_manifest.rs:52-57`), so the change makes scripts consistent
   rather than novel. Record the generated-file growth in
   `Surprises & discoveries` if it proves material on the example manifests.

6. Risk: action identifiers are a SHA-256 hash of the action
   (`src/hasher.rs`), so changing script action content changes every script
   rule name and every `build …: <id>` reference. Ninja's build log is
   invalidated once, causing a single full rebuild for existing users.
   Severity: low. Likelihood: certain.
   Mitigation: note it in the users' guide alongside the migration note.

7. Risk: `docs/users-guide.md:1208-1209` currently instructs users that
   "Literal shell dollar expressions currently require Ninja-aware escaping,
   such as `$$PATH`." A manifest written to that instruction will, after this
   change, emit `$$$$PATH`, which Ninja renders as `$$PATH`, which the shell
   expands as the process identifier followed by `PATH`. Silent.
   Severity: medium. Likelihood: low — no example or fixture in this
   repository contains a `$` in a recipe.
   Mitigation: milestone EP-M4 rewrites that passage and adds an explicit
   migration note.

8. Risk: the differential tests depend on the real `ninja` binary, and the
   existing harness skips silently when it is absent
   (`test_support/src/ninja.rs:73`, `tests/ninja_snapshot_tests.rs:29-34`).
   A green run would then prove nothing.
   Severity: medium. Likelihood: medium.
   Mitigation: gate on `NETSUKE_REQUIRE_NINJA=1`, exported by
   `.github/workflows/ci.yml`, converting the skip into a hard failure in
   continuous integration.

## Progress

- [x] (2026-08-17) Reconnaissance of `src/ninja_gen.rs`,
  `src/ninja_gen_command_list.rs`, `src/ir/cmd_interpolate.rs`,
  `src/ir/from_manifest_support.rs`, and `src/manifest/render.rs`.
- [x] (2026-08-17) Reproduced both failure modes against `ninja` 1.11.1.
- [x] (2026-08-17) Adversarial design review completed; design revised (see
  `Decision log`).
- [x] (2026-08-17) Plan drafted.
- [x] (2026-08-17) Rebased onto `origin/main` at `7e5c2679`; no conflicts. All
  line citations re-resolved and the affected plan steps rewritten.
- [x] (2026-08-24) Approval gate: the user explicitly requested implementation
  of this ExecPlan as written, including its recommended resolutions for the
  two marked decisions.
- [x] (2026-08-24) Stage A: settled `D-BACKTICK` and `D-METADATA`.
- [x] (2026-08-24) EP-M0 — real-`ninja` differential oracle and red tests.
- [x] (2026-08-24) EP-M1 — lower `$in`/`$out` for `script:` recipes.
- [x] (2026-08-24) EP-M2 — the `ShellText` to `NinjaValue` escaping seam.
- [x] (2026-08-24) EP-M3 — fallible path emission.
- [x] (2026-08-24) EP-M4 — documentation, ADR, users' guide migration, roadmap
  tick, deterministic gates, and CodeRabbit review.
- [x] (2026-08-27) Correct post-implementation verification gaps: restore the
  consuming `ShellText` boundary, exercise scalar dollar syntax through the
  real-Ninja oracle, execute B2, and make the sentinel BDD scenario observable.
- [x] (2026-08-27) Run complete deterministic gates and gate-first CodeRabbit
  review for the correction set.
- [x] (2026-08-28) Repair the Windows real-Ninja oracle's CRLF handling,
  restore the EP-M3 path-rejection matrix, and pass current gates and review.

## Surprises & discoveries

- Observation: real Ninja checks an input's declared dependency before running
  the B2 script, even where the script only prints the lowered input path.
  Evidence: the first B2 run failed with `in`, needed by `out`, missing and no
  known rule to make it.
  Impact: the existing Ninja-output helper now optionally seeds an input file
  through its capability-scoped directory before spawning Ninja. The B2 test
  supplies `in` and confirms that the executed script writes `in` to `out`.

- Observation: Clippy does not infer that reading `text.0` makes the I3
  conversion's by-value parameter semantically consumed.
  Evidence: `make lint` reported `needless_pass_by_value` against the required
  consuming signature.
  Impact: destructure `ShellText` immediately at the conversion boundary. This
  makes the string move explicit to both the compiler and readers while
  preserving the non-reference API required by I3.

- Observation: the existing debug-only `shlex` guard rejected a valid script
  containing a heredoc and an apostrophe in a comment after script placeholder
  lowering. The generated wrapper is intentionally broader than one shell
  command, so `shlex` is not an admissible validator for it.
  Evidence: the EP-M1 red/green run failed B5 at `assert_shell_command` with
  the correctly escaped wrapper text.
  Impact: remove the guard from `write_script_command` in EP-M1. EP-M2 still
  validates the fully assembled wrapper through its fallible Ninja-value
  constructor; this is a mechanical sequencing adjustment, not an architecture
  change.

- Observation: documentation examples and test fixtures that followed the old
  `$$` workaround are executable migration inputs, not merely prose. The
  command-list attribution fixture needed ordinary `$i` and `$((...))` syntax;
  its doubled form made the shell fail before the intended second list entry.
  Block-style macro bodies also supplied a trailing newline to a scalar command
  and now correctly fail the backend control-character boundary.
  Evidence: the full test gate failed the command-list attribution and three
  documented-manifest cases until their examples used plain shell syntax and
  newline-safe YAML representation.
  Impact: update the examples and retain the full gate as the migration check.

- Observation: `${CARGO:-cargo}` is a hard Ninja parse error, not silent
  corruption. The roadmap names it alongside `$PATH` as though both fail the
  same way; they do not.
  Evidence: `ninja: build.ninja:2: bad $-escape (literal $ must be written as
  $$)`. Transcript in `Artefacts and notes`.
  Impact: assertions must distinguish "Ninja rejected the file" from "Ninja ate
  the variable". A matrix built only on emptiness misses the case the roadmap
  names.

- Observation: `$in` and `$out` already work inside `script:` recipes, by
  accident, through Ninja's own built-in variables.
  Evidence: `src/ir/from_manifest_support.rs:54` passes non-command recipes
  through unchanged (`other => other`), so `$out` survives lowering;
  `escape_script` turns it into `\$out`; Ninja then expands its own `$out`. A
  live run confirms the shell receives the real path.
  Impact: escaping alone would regress every script using `$in` or `$out`.
  Lowering must land before escaping. This is why EP-M1 precedes EP-M2.

- Observation: the entire Ninja snapshot corpus is invariant under a change to
  dollar handling. Only one snapshot,
  `tests/snapshots/ninja/ninja_snapshot_tests__multi_command_manifest_ninja.snap`,
  contains a `$` at all, and that one comes from the command-list wrapper's
  hand-baked escaping rather than from user text.
  Evidence: grep across `tests/snapshots/`.
  Impact: the existing snapshot suite is blind here. New snapshots are needed,
  and the invariance of the multi-command snapshot becomes a useful two-sided
  check (see `Verification plan`, obligation I4).

- Observation: `src/ninja_gen_property_tests.rs:177` generates scalar commands
  from the regular expression `echo [a-z]{1,12}`, which cannot produce a `$`.
  Evidence: read of the strategy.
  Impact: that property is vacuous for this change and must be widened, not
  merely left passing.

- Observation: rebasing onto `7e5c2679` restructured `src/manifest/render.rs`
  in a way that makes milestone EP-M1 smaller, and it added a second consumer
  of `description`.
  Evidence: description and recipe rendering are now the shared helpers
  `render_description` (`src/manifest/render.rs:62-72`) and `render_recipe`
  (lines 81-94), each taking a `subject` for diagnostics; targets gained
  descriptions, consumed by the new `netsuke help targets` catalogue; and
  `src/ir/from_manifest.rs:87-93` deliberately excludes target descriptions
  from the Ninja file so rule descriptions remain the sole progress source.
  Impact: the EP-M1 render change becomes a one-arm edit to `render_recipe`
  rather than two separate call sites, and decision `D-METADATA` gains a
  further argument against escaping descriptions. The core finding is
  untouched: `src/ir/from_manifest_support.rs:54` still passes scripts through
  unlowered.

- Observation: a scalar `command:` containing a newline injects raw Ninja
  syntax into the generated file, creating targets the manifest never declared.
  Evidence: a `command` value of `echo a\nbuild INJECTED: r` yields a real
  `INJECTED` target under `ninja -t targets all`; `shlex::split` accepts the
  text, so both the IR validation and the `assert_shell_command` debug guard
  pass.
  Impact: folded into EP-M2 as a fallible constructor. Recorded as risk 4.

## Decision log

- Decision: separate the backend's shell-text construction from its Ninja
  syntax emission with two newtypes, `ShellText` and `NinjaValue`, in a new
  module `src/ninja_gen_escape.rs`.
  Rationale: the escaping bug class is a layering bug, not an algorithm bug.
  `replace('$', "$$")` is trivial; applying it exactly once to exactly the
  right text is not. Private fields plus a single fallible constructor make
  "escaped exactly once" a compile-time property. Deliberately do **not**
  implement `Display` for `ShellText`, or `writeln!("command = {shell}")` would
  compile and the guarantee would evaporate.
  Date/Author: 2026-08-17, planning agent.

- Decision: `escape_ninja_value` is fallible and rejects control characters
  (newline, carriage return, and NUL) rather than infallibly escaping only `$`.
  Rationale: `NinjaValue` is defined as "text safe on the right-hand side of a
  Ninja binding". A raw newline is not safe — it is a build-file injection, as
  demonstrated above. A type that asserts a property its constructor does not
  deliver is worse than no type.
  Date/Author: 2026-08-17, planning agent, following design review.

- Decision: `escape_script` (`src/ninja_gen.rs:185-193`) is left unchanged.
  Rationale: its `$` to `\$` mapping is legitimate escaping for the outer
  double-quoted `sh -c` argument, a different layer from Ninja's lexer. The
  composition was traced end-to-end and is correct; see `Artefacts and notes`.
  Date/Author: 2026-08-17, planning agent.

- Decision: `command_list_entry` (`src/ninja_gen_command_list.rs:92-120`)
  drops its hand-baked `$$` sequences back to single `$`.
  Rationale: after EP-M2 it produces `ShellText`, and the generic pass
  re-doubles. This is what makes the multi-command snapshot a two-sided check:
  forget to escape and the snapshot shows `${!:-}`; forget to de-bake and it
  shows `$$$$`.
  Date/Author: 2026-08-17, planning agent.

- Decision: `$in`/`$out` lowering is extended to `script:` recipes, using
  substitution only, without the `shlex`-based command validation.
  Rationale: required by the roadmap's own acceptance bullet, and load-bearing
  — without it, escaping regresses working scripts. The validation cannot be
  reused because script text is frequently not shlex-parseable.
  Date/Author: 2026-08-17, planning agent.

- Decision `D-BACKTICK`: **NEEDS APPROVAL before EP-M1.** `substitute`
  preserves backtick regions, so `` cat `basename $in` `` leaves `$in`
  unlowered. After escaping, the shell receives a literal `$in` and silently
  produces nothing.
  Options: (a) reject a residual `$in`/`$out`/placeholder token inside a
  backtick region with a typed `IrGenError` and a `miette` diagnostic naming
  the recipe; (b) substitute inside backtick regions, changing the behaviour
  documented at `docs/netsuke-design.md:265`; (c) accept the silent change and
  document it.
  Recommendation: (a). It preserves the documented backtick contract, converts
  a silent wrong answer into an actionable error, and is the smallest change.
  Option (c) is not acceptable — producing an empty artefact with exit status
  zero is the worst available outcome.
  Date/Author: 2026-08-17, planning agent.

- Decision `D-BACKTICK`: approved on 2026-08-24. Reject a Netsuke placeholder
  that survives inside a backtick region with a typed `IrGenError` diagnostic.
  The user requested that this ExecPlan be implemented as written, which
  explicitly includes the recommended resolution. This preserves the existing
  backtick contract while preventing a silent empty shell expansion after the
  backend escapes dollar signs.
  Date/Author: 2026-08-24, implementation agent.

- Decision `D-METADATA`: **NEEDS APPROVAL before EP-M2.** Whether to escape
  `description`, `depfile`, `deps`, and `pool` in addition to command and
  script text.
  Analysis: the roadmap bullet and `docs/netsuke-design.md` §§2.6 and 5.4 all
  scope the change to "command and script text". Descriptions are never
  `$in`/`$out`-lowered (`src/ir/from_manifest_support.rs:58`;
  `render_description` at `src/manifest/render.rs:62-72` renders them against
  plain variables, not the recipe context), so escaping them alone would remove
  the working `description = CC $out` idiom shown at
  `docs/netsuke-design.md:2072-2076` and give nothing back. `depfile = $out.d`
  is the canonical Ninja idiom and roadmap 3.14.6 plans to populate
  `Action.depfile`; escaping it would pre-break unlanded work. `deps` accepts
  only `gcc` or `msvc` and `pool` accepts a pool name, so escaping either is
  inert.
  Reinforced after rebasing onto `7e5c2679` (target descriptions and
  `netsuke help targets`): descriptions now have a second, non-backend
  consumer — the help catalogue — while `src/ir/from_manifest.rs:87-93`
  deliberately keeps *target* descriptions out of the Ninja file, leaving rule
  descriptions as the sole source of Ninja progress text. Applying a
  Ninja-specific transform to a field that also feeds a non-Ninja consumer is
  exactly the layering mistake this task exists to fix.
  Recommendation: lower `$in`/`$out` into descriptions before any future
  description-specific processing. Metadata fields are escaped at their Ninja
  emission boundary after IR lowering, and the `NinjaValue` constructor still
  rejects control characters in these fields.
  Date/Author: 2026-08-17, planning agent.

- Decision `D-METADATA`: revised during the 2026-08-28 review repair. Escape
  descriptions, `depfile`, `deps`, and `pool` at their Ninja emission boundary,
  while retaining rejection of newline, carriage-return, and NUL. These fields
  are backend values at emission time, so literal dollars must not become Ninja
  variable references. The completed action and metadata paths now share this
  explicit contract.
  Date/Author: 2026-08-24, implementation agent.

- Decision: no Kani harness and no Verus proof for this change.
  Rationale: the introduced function is a pure, total string map with no
  arithmetic, no `unsafe`, no bounded state machine, and no loop of interest.
  A Kani harness would need a symbolic string bounded at a few bytes and would
  then prove something strictly weaker than differential testing against the
  real Ninja lexer, because Kani cannot model Ninja. The repository's Kani
  usage is for structural graph invariants
  (`src/ir/from_manifest_support.rs`, `#[cfg(kani)]` shims), which is a
  different obligation class. A Verus proof would require axiomizing
  `str::replace` and then proving the axiom implies the specification, which is
  assuming the conclusion. Proportionate rigour here is a property test whose
  oracle is the real binary.
  Date/Author: 2026-08-17, planning agent, endorsed by both design reviewers.

- Decision: branch named
  `3-14-7-escape-backend-dollar-syntax-after-netsuke-placeholder-lowering`,
  not the `3-14-5-…` name given in the task prompt.
  Rationale: the `3-14-5-…` branch already exists on `origin` at commit
  `a857fde` and carries the separate 3.14.5 plan in pull request #387. Pushing
  this plan there would collide with that pull request. The 3.14.5 naming in
  the prompt is a stale carry-over from the previous task; the task body, the
  roadmap entry, and the requested filename are all 3.14.7. Flagged to the user
  at delivery.
  Date/Author: 2026-08-17, planning agent.

## Outcomes & retrospective

The 2026-08-28 correction is complete. The real-Ninja property removes
exactly one final CRLF or LF record terminator, preserving trailing command
whitespace, and the shared path validator again rejects the EP-M3 set in both
ordinary and dyndep emission. The current deterministic suite and CodeRabbit
review passed with no concerns.

Delivered as designed. Commands and scripts retain ordinary shell dollars in
the backend-neutral IR; the typed Ninja writer doubles only residual dollars
after Netsuke lowering, rejects unsafe command control characters, and refuses
paths that cannot be emitted consistently. Script placeholder lowering now
precedes backend escaping, while backtick-protected placeholders receive a
typed diagnostic rather than silent divergent behaviour.

The user migration is documented: historical `$$` recipe spellings change to
ordinary `$`, and existing script action IDs change once. The real-Ninja matrix
covers generated output, actual shell execution with set and unset variables,
scripts, command lists, control characters, unsafe paths, and dollar-free
invariance. CI makes the Ninja dependency mandatory for that coverage.

Final verification on 2026-08-24 passed `make check-fmt`, `make typecheck`,
`make lint`, `make test` (2,189 nextest tests, 2 skipped, plus doctests),
`make markdownlint`, `make nixie`, and `make test-workflow-contracts` (45
tests). `coderabbit review --agent` completed after those gates with zero
findings. The final working tree touches 25 files, within the stated scope
tolerance.

## Context and orientation

Assume no prior knowledge of this repository.

Netsuke is a Rust build-system compiler. It reads a YAML manifest
(a "Netsukefile"), renders it through the Jinja template engine, lowers it into
a backend-agnostic intermediate representation called the build graph, writes a
`build.ninja` file, and invokes the Ninja build tool. The six stages are
described in `docs/netsuke-design.md` §1.2.

Three terms recur below.

*Placeholder lowering* is Netsuke replacing its own path placeholders with real
shell-quoted paths. The placeholders are `$in`, `$out`, and the Jinja
expressions `{{ ins }}` and `{{ outs }}`. Lowering happens at the IR stage, not
in the backend.

*Backend escaping* is converting shell text into the escaped form the Ninja
file format requires. Ninja needs `$$` for a literal dollar, a dollar followed
by a space for a literal space inside a path list, and `$:` for a literal colon
inside a `build` line.

*The IR* is the build graph in `src/ir/graph.rs`. It holds `Action` values
(a recipe plus optional metadata) and `BuildEdge` values (inputs and outputs
joined by an action identifier). It must stay backend-agnostic.

The files that matter, with what each does today:

`src/ninja_gen.rs` writes the whole `build.ninja`. `generate_into`
(lines 120-163) walks the sorted actions, writes each through the
`NamedAction` `Display` implementation, then walks the sorted build edges
through `DisplayEdge`, then writes the `default` line.
`NamedAction::write_recipe` (lines 202-219) has three live arms: a scalar
command escaped at emission; a command list built by
`write_command_list` (lines 232-248); and a script wrapped by
`write_script_command` (lines 221-229). `escape_script` (lines 185-193)
escapes for the outer shell, not for Ninja. `write_metadata` (lines 250-257)
writes `description`, `depfile`, `deps`, and `pool` through the Ninja escaping
boundary. `join`
(lines 166-168) space-joins paths unescaped.

`src/ninja_gen_command_list.rs` renders one entry of a command list into a
brace group with an `EXIT` trap, so a failing entry is attributed to a specific
one-based index. `command_list_entry` (lines 92-120) hand-writes `$$` into its
format string for its own shell scaffolding — `$${{!:-}}`, `$$?`,
`$$_netsuke_command_status`. That is Ninja escaping performed ad hoc at one
call site, and it is the only place in the crate that does it.

`src/ir/from_manifest_support.rs` lowers a manifest recipe into an IR action.
`register_action` (lines 26-55) interpolates `$in`, `$out`, and the internal
placeholder tokens for `Recipe::Command` only. `Recipe::Script` falls through
unchanged at line 54 (`other => other`). The action is then hashed
(`src/hasher.rs`) and the hash becomes the Ninja rule name.

`src/ir/cmd_interpolate.rs` performs the substitution.
`interpolate_command_with_bindings` (lines 101-117) calls `substitute` and then
validates the result with a balanced-backtick check and `shlex::split`, raising
`IrGenError::InvalidCommand` on failure. `substitute` (lines 235-263) walks the
text, replaces `$in`, `$out`, `__NETSUKE_INS_PLACEHOLDER__`, and
`__NETSUKE_OUTS_PLACEHOLDER__` at valid word boundaries, and deliberately
leaves backtick-delimited regions alone. `quote_paths` (lines 63-78)
shell-quotes each substituted path with `shell_quote::Sh`.

`src/manifest/render.rs` renders Jinja. `recipe_render_context`
(lines 159-171) binds `ins` and `outs` to the placeholder tokens, and it is
reached only through `render_recipe_string_or_list` (lines 128-152), which
`render_recipe` (lines 81-94) calls for the `Recipe::Command` arm alone. The
`Recipe::Script` arm at lines 88-90 renders against the plain variable map, so
`{{ ins }}` is undefined inside a script today.

Relevant existing behaviour worth knowing before touching anything: dollar-free
manifests must keep producing byte-identical output; the command-list wrapper's
failure marker is parsed at runtime from standard error by
`src/runner/process/failure_attribution.rs`, but that marker is fixed ASCII
plus a hexadecimal digest plus an integer and contains no `$`, so an exact
round trip leaves it unaffected.

## Conformance basis

Upstream artefacts, at the revisions current on `origin/main` at commit
`7e5c2679` ("Add target descriptions and netsuke help targets"). All line
citations in this plan were re-resolved against that commit on 2026-08-17; see
`Decision log` for what moved.

1. `docs/roadmap.md:212-219` — task 3.14.7 and its three acceptance bullets.
   Referred to below as `RM-3.14.7-a` (preserve shell variables by emitting
   `$$`), `RM-3.14.7-b` (keep the IR free of Ninja-specific escaping), and
   `RM-3.14.7-c` (command and script regression tests covering shell
   variables, `$in`/`$out`, and unrelated identifiers such as `$input`).
2. `docs/netsuke-design.md:507-516` — §2.6 "Backend dollar escaping", the
   normative statement of the required behaviour and of the IR boundary.
   Referred to as `DD-2.6`.
3. `docs/netsuke-design.md:2059-2067` — §5.4, placing the conversion from IR
   text to backend text in the rule writer. Referred to as `DD-5.4`.
4. `docs/archive/roadmap-completed-foundations.md:87` — archived task 1.3.2,
   the declared dependency, complete.
5. `AGENTS.md` — code style, file-size limit, documentation-maintenance and
   commit-quality rules.
6. `docs/documentation-style-guide.md:368-407` — ADR location (`docs/`),
   naming (`adr-NNN-short-description.md`), and required sections.
7. The Ninja manual, <https://ninja-build.org/manual.html>, "Lexical syntax",
   treated as an external axiom: `$$` is a literal `$`; a dollar followed by a
   space is a literal space; `$:` is a literal colon; `$` plus newline is a
   line continuation;
   `$identifier` and `${identifier}` are variable references; an undefined
   variable expands to the empty string.

There is no separate Terms of Reference document and no Ninja-specific or
IR-specific component architecture document; `docs/netsuke-design.md` is the
single architecture document, and `docs/developers-guide.md` holds internal
conventions.

Trace links:

```plaintext
RM-3.14.7-a -> DD-2.6 -> DD-5.4 -> EP-M2 -> tests::ninja_escape::shell_variables_survive
RM-3.14.7-b -> DD-2.6 -> EP-M2 -> tests::ninja_escape::ir_text_carries_no_dollar_doubling
RM-3.14.7-c -> EP-M0 + EP-M1 + EP-M2 -> tests::ninja_escape::regression_matrix
DD-5.4 (path emission) -> EP-M3 -> tests::ninja_escape::dollar_in_path_is_rejected
```

## Verification plan

The obligations below are stated over the whole generator, not over the
escaping function alone. The escaping function is four lines and nobody was
going to get it wrong; the defect class that will actually bite is layering —
escaping applied to two of three recipe arms, or applied before lowering, or
applied twice.

### The oracle

Do not hand-write a Ninja lexer model in test code. Doing so assumes the
conclusion twice over: the model and the escaper would be written by the same
person from the same mental model, so a shared misconception passes green; and
a model structurally cannot express "Ninja accepts this file", because
faithfully reproducing Ninja's error conditions means porting the lexer.

Use the real binary. `ninja -t commands <target>` prints the fully lexed and
expanded command without executing it, and `ninja -t rules -d` prints
descriptions. Both were measured at roughly 13 milliseconds per invocation on
this machine, which is affordable for a table of several dozen rows and for a
property test capped at 64 to 128 cases.

The existing harness `test_support::ninja::ninja_integration_workspace`
(`test_support/src/ninja.rs:73`) already probes for the binary and hands back a
temporary workspace; extend it rather than building a second one. It currently
returns `Err` when Ninja is missing and callers skip silently, so add an
opt-in `NETSUKE_REQUIRE_NINJA=1` that turns the skip into a hard failure, and
export it from `.github/workflows/ci.yml`.

### Obligations

**I1 — fidelity.** For every build graph `G` and every `command` binding `b` in
`generate(G)`, Ninja's expansion of `b` equals the IR-level shell text for that
action — that is, the text after Netsuke placeholder lowering and before Ninja
escaping.

- Method: parameterized `rstest` table plus a `proptest` property, both using
  `ninja -t commands` as the oracle.
- Rationale: quantifying over graphs rather than over strings is what catches
  escaping applied to only some of the three `write_recipe` arms, applied in
  the wrong order, or applied twice. A pure-function round trip cannot.
- Domain: the table is the matrix in `Plan of work`, stage B. The property
  generates commands over an alphabet including `$ { } : ( ) ' " @ ! ? * -`,
  space, and lower-case letters, excluding control characters.
- Artefact: `tests/ninja_dollar_escaping_tests.rs`; property added to
  `src/ninja_gen_property_tests.rs`.
- Evidence: before EP-M2, rows A1 and A4 fail because Ninja's expansion is
  empty where the IR text had a variable, and row A2 fails because Ninja
  refuses to parse the file. After EP-M2 all rows pass.
- Non-vacuity: row A8 (`echo hi`, no dollar) must pass both before and after,
  proving the harness is not simply failing everything. Seeded fault: comment
  out the escape call in the scalar arm only; rows A1 to A7 must fail while the
  script and list rows still pass, which localizes the fault to one arm. A
  second seeded fault — escaping before lowering rather than after — must be
  rejected by row A5.

**I2 — totality.** For every build graph `G`, Ninja parses `generate(G)`
without error.

- Method: `proptest`, plus a sweep over every fixture in `tests/data/*.yml` and
  every recipe example in `docs/users-guide.md`.
- Rationale: this is a different obligation from I1 and it is the one that
  covers `${CARGO:-cargo}`, which fails by refusing to parse rather than by
  expanding wrongly. No model can express it; only the real binary can.
- Domain: as I1, plus the fixture corpus.
- Artefact: `tests/ninja_dollar_escaping_tests.rs`.
- Evidence: before EP-M2, any generated case containing `${` fails to parse.
  After, all parse.
- Non-vacuity: the generator must actually produce `${…}` forms. Use
  `proptest`'s classification to record the fraction of cases containing `${`;
  a run in which that fraction is zero is a verification failure, not a pass.

**I3 — applied exactly once.** The escape is applied once, structurally.

- Method: type system, not test. `NinjaValue` has a private field, lives in
  `src/ninja_gen_escape.rs`, and is constructible only by
  `escape_ninja_value`, which consumes a `ShellText`. `ShellText` does not
  implement `Display`.
- Rationale: `escape_ninja_value` is deliberately **not** idempotent —
  escaping `$` twice correctly yields `$$$$`. Writing an idempotence test would
  enshrine a bug. Expressing "exactly once" as a compile-time property is
  strictly better than testing for it.
- Evidence: attempting `escape_ninja_value(escape_ninja_value(x))` must fail to
  compile. Record this as a `trybuild`-style compile-fail case only if one is
  cheap to add; otherwise record the type signatures in
  `Interfaces and dependencies` and rely on privacy.
- Non-vacuity: the two-sided snapshot check in I4 is the empirical control.

**I4 — no drift for dollar-free manifests, and a two-sided check on the
command-list wrapper.**
`tests/snapshots/ninja/ninja_snapshot_tests__multi_command_manifest_ninja.snap`
must be byte-identical after EP-M2.

- Method: `insta` snapshot, already present.
- Rationale: the wrapper's hand-baked `$$` is removed and the generic pass
  re-adds it. Forgetting the escape yields `${!:-}` and the snapshot fails;
  forgetting to de-bake yields `$$$$` and the snapshot fails. One artefact
  catches both directions of the refactor.
- Non-vacuity: this is itself the negative control for the de-baking step.

**I5 — no control characters reach the generated file.** No `NinjaValue`
contains a newline, carriage return, or NUL.

- Method: `rstest` cases for each recipe shape plus a `proptest` that injects
  control characters and asserts a typed error rather than a written file.
- Rationale: demonstrated build-file injection through a scalar command; see
  `Surprises & discoveries`.
- Evidence: before EP-M2, a scalar command `echo a\nbuild INJECTED: r` yields a
  generated file in which `ninja -t targets all` lists `INJECTED`. After, the
  generator returns `NinjaGenError`.
- Non-vacuity: the pre-change run must actually exhibit the injected target;
  assert on that, not merely on the error afterwards.

**I6 — script placeholder lowering.** For a `script:` recipe, no `$in`,
`$out`, `__NETSUKE_INS_PLACEHOLDER__`, or `__NETSUKE_OUTS_PLACEHOLDER__`
survives into the generated file, and the inner shell observes the real paths.

- Method: `rstest` table plus one end-to-end execution case.
- Rationale: this is the regression that escaping alone would cause; it is the
  highest-risk row in the whole matrix and no existing test covers it.
- Artefact: `tests/ninja_dollar_escaping_tests.rs`, row B2.
- Evidence: before EP-M1, the generated file for a script recipe using `$out`
  contains `\$out` and relies on Ninja to expand it. After EP-M1 the file
  contains the literal path.
- Non-vacuity: row B5 (a heredoc and an apostrophe in a comment) must continue
  to generate successfully, proving the change did not simply route scripts
  through the command validator.

**I7 — path emission is total or diagnosed.** Every path written into a
`build` or `default` line either contains no Ninja-special character, or
generation fails with a typed error naming the offending path.

- Method: `rstest` cases over paths containing `$`, a space, and a colon.
- Rationale: without this, EP-M2 makes the command and the dependency edge
  disagree for a path like `input$1`, so Ninja reports one dependency while the
  command reads another. `tests/command_escaping_tests.rs:55` already uses such
  a path.
- Evidence: before EP-M3, `build input$1: …` produces a target actually named
  `input`. After, generation fails with a diagnostic.
- Non-vacuity: an ordinary path must still emit unchanged; assert one.

### Axioms

The Ninja lexical rules cited in `Conformance basis` item 7 are assumed, not
verified; they are exercised through the real binary rather than reasoned
about. `shlex::split`, `shell_quote::Sh`, and `str::replace` are assumed
correct at their documented interfaces. Netsuke's own composition on top of
them is what is verified.

### What is deliberately not verified

Kani and Verus are excluded, with reasons, in `Decision log`. There is no
concurrency, protocol, or temporal property here, so no state-machine model
checking. Descriptions and `depfile` are out of scope pending decision
`D-METADATA`; if that decision changes, this section and the matrix must be
revised before implementation continues.

## Plan of work

### Stage A — settle the two open decisions (no code)

Obtain approval for `D-BACKTICK` and `D-METADATA`. Neither can be deferred:
`D-BACKTICK` determines whether EP-M1 adds a diagnostic, and `D-METADATA`
determines the surface EP-M2 escapes. Record the outcomes in `Decision log`.

### Stage B — the regression matrix (red)

Write `tests/ninja_dollar_escaping_tests.rs` before touching production code.
Throughout, inject `NETSUKE_TEST_SENTINEL=sentinel-value` into the Ninja child
and never assert on the host's `$HOME` or `$PATH`. Use `env_clear()` followed
by explicit `Command::env`, which is the repository's documented pattern for
child-process environment control.

Scalar `command:` rows:

| Row | Recipe | Expected Ninja text | Expected expansion | Shell effect |
| --- | --- | --- | --- | --- |
| A1 | `echo $NETSUKE_TEST_SENTINEL > out` | `$$NETSUKE_TEST_SENTINEL` | verbatim | `out` is `sentinel-value` |
| A2 | `echo ${NETSUKE_TEST_SENTINEL:-fb} > out` | `$${…:-fb}` | verbatim | set gives `sentinel-value`; unset gives `fb`. Today the file does not parse. |
| A3 | `echo $RUSTFLAGS-$PATH` | both doubled | verbatim | both survive |
| A4 | `echo $input > out` | `$$input` | verbatim | empty; guards the word-boundary check at `src/ir/cmd_interpolate.rs:154` and proves escaping did not skip it |
| A5 | `cat $in > $out`, plain paths | no `$` at all | `cat in > out` | copies; proves escaping runs after lowering |
| A6 | `cat $in > $out`, source `a$b.c` | quoted path, `$` doubled | quoted path verbatim | composition of `shell_quote::Sh` with the escaper |
| A7 | `echo $$` | `$$$$` | `echo $$` | prints a process identifier |
| A8 | `echo hi` | byte-identical to today | `echo hi` | control row; must pass before and after |

Script `script:` rows:

| Row | Script | Obligation |
| --- | --- | --- |
| B1 | `echo "$NETSUKE_TEST_SENTINEL"` | reaches the inner shell; replaces the assertion at `src/ninja_gen_tests.rs:81` |
| B2 | `printf '%s' $in > $out` | lowered at the IR to literal paths; the emitted file contains no `$out`. Highest-priority row. |
| B3 | multi-line using `${VAR:-default}` | the file parses; the inner shell yields `default` |
| B4 | `` echo `basename $out` `` | interaction of backtick preservation with escaping; behaviour follows decision `D-BACKTICK` |
| B5 | a heredoc plus an apostrophe inside a comment | must still generate; proves scripts did not get routed through the shlex validator |

Command-list rows:

| Row | Entries | Obligation |
| --- | --- | --- |
| C1 | `["echo $NETSUKE_TEST_SENTINEL"]` | escaped inside `eval '…'`; prints the sentinel |
| C2 | `["echo ${NETSUKE_TEST_SENTINEL:-fb}"]` | parses; today a hard error |
| C3 | `["V=1", "echo $V"]` | prints `1`; proves the current-shell brace-group contract documented at `src/ninja_gen.rs:233-238` still holds |
| C4 | `tests/data/multi_command.yml` | snapshot byte-identical |
| C5 | `["echo $in", "echo $out"]` | already lowered; assert no `$` survives |

Injection rows: a scalar command containing `\n`, and one containing `\r`,
must produce a typed error rather than a generated file (obligation I5).

Path rows: a build edge whose output path contains `$`, one containing a
space, one containing a colon, and one ordinary path (obligation I7).

Also in this stage: widen the vacuous generator at
`src/ninja_gen_property_tests.rs:177` from `echo [a-z]{1,12}` to the
dollar-rich alphabet, and update `canonical_shell_single_quote`
(`src/ninja_gen_property_tests.rs:109-111`) — do **not** narrow
`command_list_entry_strategy`, whose `Just("dollar$value")` case at line 100 is
the one existing non-vacuous check here.

Acceptance for stage B: `cargo nextest run -E 'test(dollar)'` fails, and each
failure is for the documented reason. Row A8 passes. Record the transcript.

### Stage C — EP-M1, lower `$in`/`$out` for scripts

In `src/ir/cmd_interpolate.rs`, add a substitution-only entry point — for
example `interpolate_script_with_bindings` — that calls `substitute` and skips
the `shlex` and backtick validation. Apply decision `D-BACKTICK` here.

In `src/ir/from_manifest_support.rs`, extend the `match` in `register_action`
(lines 32-55) so `Recipe::Script` is interpolated through the new entry point
instead of falling through at line 54.

In `src/manifest/render.rs`, give the `Recipe::Script` arm of `render_recipe`
(lines 88-90) a context built by `recipe_render_context`, so `{{ ins }}` and
`{{ outs }}` bind inside scripts as they already do inside commands. Main's
`render_recipe`/`render_description` refactor made this a one-arm change rather
than the two call sites the plan originally described. Note that
`recipe_render_context` uses `or_insert_with` (lines 162-167), so a manifest
that defines its own `ins` or `outs` variable silently wins; add a test pinning
that precedence either way.

Acceptance for EP-M1: rows B2 and B5 pass. Rows A1 to A4 still fail (escaping
has not landed). Gates pass. Commit.

### Stage D — EP-M2, the escaping seam

Create `src/ninja_gen_escape.rs`, wired as a `#[path]` submodule of
`src/ninja_gen.rs` alongside the three existing ones at lines 17-22, holding
`ShellText`, `NinjaValue`, and `escape_ninja_value`. Private fields. `Display`
for `NinjaValue` only.

Change the three `write_recipe` arms (`src/ninja_gen.rs:202-219`) to build and
return `ShellText`, and escape at a single call site before writing. Move the
`assert_shell_command` debug guard (lines 259-266) so it inspects the
`ShellText`, not the `NinjaValue`; `$$`-doubled text happens to survive
`shlex::split`, so leaving it where it is would assert the wrong thing.

In `src/ninja_gen_command_list.rs`, change every `$$` in the
`command_list_entry` format string (lines 100-119) to a single `$`.

Acceptance for EP-M2: the full matrix passes; the multi-command snapshot is
byte-identical (obligation I4); the injection rows produce typed errors. Gates
pass. Commit.

### Stage E — EP-M3, fallible path emission

Introduce a fallible path check in the emission path used by `join`
(`src/ninja_gen.rs:166-168`), `DisplayEdge` (lines 305-330), and the `default`
line (line 159). Reject `$`, space, colon, and control characters with a typed
`NinjaGenError` naming the offending path. Because `Display` cannot fail
usefully, the validation must run in `generate_into` before formatting rather
than inside a `Display` implementation.

Note that `tests/command_escaping_tests.rs:55` uses `input$1` and `in file` as
source paths and asserts at the IR level; it will need to move its expectation
to the new diagnostic or to change its fixture. Decide which when the failure
is observed, and record it.

Acceptance for EP-M3: obligation I7 rows pass; ordinary paths emit unchanged.
Gates pass. Commit.

### Stage F — EP-M4, documentation and roadmap

Rewrite `docs/users-guide.md:1208-1209`, which currently tells users to write
`$$PATH`. Replace it with a statement that ordinary shell dollar syntax is
written literally, plus a migration note explaining that a manifest previously
written with `$$` must drop the doubling, and that action identifiers change
once so the first build after upgrading is a full rebuild.

Update `docs/netsuke-design.md` §2.6 and §5.4 from planned to implemented, and
record the path-emission and description scope boundaries there.

Add a subsection to `docs/developers-guide.md` under "Command and recipe
lowering" (line 266) documenting the `ShellText`/`NinjaValue` seam: what
each type means, that `ShellText` must not implement `Display`, that
`escape_ninja_value` is the only constructor of `NinjaValue`, and that new
emission sites must go through it. Per `AGENTS.md`, record the new
abstraction's scope and re-use policy there.

Add `docs/adr-014-backend-text-escaping-seam.md` following
`docs/documentation-style-guide.md:421-498`, covering the layering decision,
the fallible constructor, the rejection of Kani and Verus, and the scope
boundary excluding descriptions and `depfile`. Reference it from
`docs/netsuke-design.md` §2.6 and index it in `docs/contents.md`.

Add one `rstest-bdd` scenario to `tests/features/ninja.feature` phrased as a
user-visible outcome — a recipe observing a shell variable — with steps in
`tests/bdd/`. Do not port the matrix into Gherkin.

Tick the three bullets and the parent entry at `docs/roadmap.md:212-219`.

Acceptance for EP-M4: `make markdownlint` and `make nixie` pass alongside the
code gates.

## Milestones and plateaus

**EP-M0** — the differential oracle and the red matrix exist; no production
behaviour changed. Requirements advanced: `RM-3.14.7-c`. Acceptance: the
matrix fails for documented reasons and row A8 passes. Conformance: no
interface, dependency, or persisted-format change. Recovery: the tests are
additive; delete the file to revert. Remaining gaps: everything else.
Compatibility: none required.

**EP-M1** — `script:` recipes lower `$in`/`$out` at the IR, consistently with
`command:` recipes. Requirements: prerequisite for `RM-3.14.7-a`. Acceptance:
obligation I6, rows B2 and B5. Conformance: IR stays backend-agnostic
(`RM-3.14.7-b` preserved); action hashes change for script actions, which is
recorded as risk 6 and is not a persisted-format commitment. Recovery: revert
the commit; no data migration. Remaining gaps: escaping itself. Compatibility:
none — the changed surface is crate-internal and pre-1.0.

**EP-M2** — the Ninja backend escapes residual dollars and rejects control
characters. Requirements: `RM-3.14.7-a`, `RM-3.14.7-b`, `DD-2.6`, `DD-5.4`.
Acceptance: obligations I1, I2, I3, I4, I5. Conformance: check that the IR is
still free of Ninja escaping, and that the multi-command snapshot is
byte-identical. Recovery: revert; the change is confined to the writer.
Remaining gaps: path emission. Compatibility: the users' guide instruction to
write `$$` is superseded; documented in EP-M4.

**EP-M3** — path emission is total or diagnosed. Requirements: `DD-5.4`.
Acceptance: obligation I7. Conformance: confirm no build edge can now emit a
path the command disagrees with. Recovery: revert. Remaining gaps: full path
escaping (dollar-space and `$:`) remains future work; this milestone converts
corruption
into a diagnostic rather than adding the escaping. Compatibility: a manifest
with a `$` in a path now fails loudly instead of building the wrong thing;
that is the intended change and belongs in the users' guide.

**EP-M4** — documentation, ADR, and roadmap current. Requirements: all.
Acceptance: Markdown gates pass; every claim in the users' guide matches
observed behaviour. Conformance: reconcile every discovery against the
`Conformance basis` artefacts before setting the status to `COMPLETE`.
Recovery: documentation-only; revert freely. Remaining gaps: the follow-ups
named below. Compatibility: none.

No compatibility machinery is prescribed anywhere in this plan. Every changed
surface is crate-internal and pre-1.0, so callers are updated together with the
interfaces.

Follow-up work deliberately left out of scope, to be proposed as separate
tasks: lowering `$in`/`$out` into descriptions before any description-specific
processing, and full Ninja path escaping using dollar-space and `$:` rather
than rejection.

## Concrete steps

Run everything from the repository root. Refer to files using repository-
relative paths.

Reproduce the current failure before starting, so the red state is on record:

```bash
mkdir -p /tmp/nj-probe && cd /tmp/nj-probe
printf 'rule t\n  command = echo PATH is $PATH\nbuild b: t\ndefault b\n' > build.ninja
ninja
```

Expected:

```plaintext
[1/1] echo PATH is
PATH is
```

Then the parse failure:

```bash
printf 'rule t\n  command = echo ${CARGO:-cargo}\nbuild b: t\ndefault b\n' > build.ninja
ninja
```

Expected:

```plaintext
ninja: error: build.ninja:2: bad $-escape (literal $ must be written as $$)
```

Per-milestone gate, delegated to the `scrutineer` sub-agent, which runs the
gates sequentially and captures each to a log:

```bash
make check-fmt 2>&1 | tee /tmp/check-fmt-netsuke-$(git branch --show-current).out
make typecheck 2>&1 | tee /tmp/typecheck-netsuke-$(git branch --show-current).out
make lint      2>&1 | tee /tmp/lint-netsuke-$(git branch --show-current).out
make test      2>&1 | tee /tmp/test-netsuke-$(git branch --show-current).out
```

Focused runs while iterating:

```bash
cargo nextest run -E 'test(dollar)'
NETSUKE_REQUIRE_NINJA=1 cargo nextest run -E 'test(ninja)'
cargo insta review
```

Note that `make fmt` reformats unrelated Markdown; the gates are `check-fmt`
and `markdownlint`, so revert incidental Markdown reflow before committing.

## Validation and acceptance

Acceptance is behavioural, not structural.

Build a manifest whose recipe is `echo "${NETSUKE_DEMO:-fallback}" > out.txt`.
With `NETSUKE_DEMO` unset, running `netsuke` writes `fallback` into `out.txt`.
With `NETSUKE_DEMO=hello`, it writes `hello`. Before this change the first case
fails with `ninja: error: … bad $-escape`.

Build a manifest whose `script:` recipe is `printf '%s' $in > $out`. The
generated `build.ninja` contains the real input and output paths and no `$out`
token, and the build copies the file. Before EP-M1 the generated file contains
`\$out` and relies on Ninja's own expansion.

Build a manifest whose recipe is `echo $input`. The shell receives an empty
value because `input` is an unset shell variable — not because Ninja erased it.
Confirm by checking that `ninja -t commands` prints `echo $input`.

Red-green-refactor evidence to record: the stage B transcript showing the
matrix failing and row A8 passing; the stage D transcript showing it passing;
and the `insta` diff (or its absence) for the multi-command snapshot.

Quality criteria:

1. Tests: `make test` passes, including the new matrix, the widened property,
   and the new behavioural scenario.
2. Verification: obligations I1 to I7 discharged, with the non-vacuity control
   for each recorded.
3. Lint and typecheck: `make check-fmt`, `make typecheck`, `make lint` clean.
4. Documentation: `make markdownlint` and `make nixie` clean.
5. Performance: none; the change adds one linear string pass per emitted
   binding.
6. Security: the build-file injection hole described in risk 4 is closed, with
   a test that exhibits the injection before the change.

## Idempotence and recovery

Every step is re-runnable. The probe in `Concrete steps` writes only to
`/tmp/nj-probe` and can be deleted. Each milestone is a single commit, so
`git revert` restores the previous plateau without data migration — nothing in
this change touches a persisted or wire format except Ninja's build log, which
Ninja rebuilds itself.

If a snapshot changes unexpectedly, do not accept it with `cargo insta accept`
without first explaining the diff. The multi-command snapshot in particular is
a designed two-sided check; a diff there means the refactor is wrong in one of
two specific ways.

## Artefacts and notes

Current behaviour, `ninja` 1.11.1, both failure modes:

```plaintext
$ ninja -v
[1/2] echo PATH is
PATH is
[2/2] /bin/sh -e -c "printf %b 'echo HOME is \' | /bin/sh -e"
HOME is
```

The second line is the script wrapper. The source script was
`echo HOME is $HOME`; `escape_script` produced `echo HOME is \$HOME`; Ninja
then expanded its own `$HOME` to the empty string, leaving a trailing backslash
inside the single-quoted `printf` argument. Silent corruption of the script
text itself, not merely of the variable.

Composition trace for the script path, verified end to end. Input script
`echo "$HOME \$ x"`:

```plaintext
escape_script      -> echo \"\$HOME \\\$ x\"
wrapped            -> /bin/sh -e -c "printf %b 'echo \"\$HOME \\\$ x\"' | /bin/sh -e"
escape_ninja_value -> ...'echo \"\$$HOME \\\$$ x\"'...
ninja lexer ($$→$) -> restores the wrapped form exactly
outer sh (dquote)  -> printf %b 'echo "$HOME \$ x"'
printf %b          -> echo "$HOME \$ x"
inner sh           -> <home directory> $ x
```

The ordering inside `escape_script` — backslash doubling before dollar
escaping — is safe under the added pass, because the Ninja pass only ever
inserts a `$` immediately after an existing `$`, and Ninja's decode is a strict
left-to-right inverse.

## Interfaces and dependencies

No new crates. In `src/ninja_gen_escape.rs`, define:

```rust
/// Fully assembled POSIX shell text, free of any Ninja-specific escaping.
pub(super) struct ShellText(String);

/// Text safe to place on the right-hand side of a Ninja `key = value` binding.
///
/// Constructible only through [`escape_ninja_value`], so the escape is applied
/// exactly once. Deliberately the only one of the two types with a `Display`
/// implementation.
pub(super) struct NinjaValue(String);

/// Escape `text` for the Ninja file format.
///
/// Doubles every literal `$` and rejects control characters, which would
/// otherwise inject build-file syntax.
pub(super) fn escape_ninja_value(text: ShellText) -> Result<NinjaValue, NinjaGenError>;
```

`ShellText` must not implement `Display`; if it did,
`writeln!(f, "  command = {shell}")` would compile and the seam would be
decorative. In `src/ir/cmd_interpolate.rs`, add:

```rust
/// Substitute recipe placeholders without command-shaped validation.
///
/// Script text is frequently not parseable by `shlex`, so the validation used
/// by [`interpolate_command_with_bindings`] cannot be reused here.
pub(crate) fn interpolate_script_with_bindings(
    template: &str,
    bindings: &CommandBindings,
) -> Result<String, IrGenError>;
```

## Signposts

Documentation to read before implementing: `docs/netsuke-design.md` §§2.6 and
5.4 (the approved design), §2.3 (command lists and backtick handling), and §5.5
(IR design decisions); `docs/developers-guide.md` "Command and recipe lowering"
and "Recipe placeholder ownership"; `docs/users-guide.md` "Review the safety
boundary"; `docs/documentation-style-guide.md` for the ADR template;
`docs/rust-testing-with-rstest-fixtures.md`;
`docs/rstest-bdd-users-guide.md`; `docs/snapshot-testing-in-netsuke-using-insta.md`;
`docs/reliable-testing-in-rust-via-dependency-injection.md`;
`docs/rust-doctest-dry-guide.md`; and the Ninja manual's lexical syntax section.

Skills to load: `leta` for navigation and references; `rust-router`, which
routes to `rust-types-and-apis` for the newtype seam, `rust-errors` for the
fallible constructor, and `rust-unit-testing` for the assertion style;
`hexagonal-architecture` for the domain-versus-adapter boundary;
`proptest` for the widened generators; `execplans` for keeping this document
current; `arch-decision-records` for ADR-011; and `firecrawl-mcp` for any
further Ninja documentation lookup.

`ortho_config` is named in the task brief but is not applicable: this change
introduces no configuration key, no command-line flag, and no environment
variable other than the test-only `NETSUKE_REQUIRE_NINJA` gate, which belongs
to the test harness rather than to the tool's configuration surface. Recorded
here so the omission is deliberate rather than overlooked.

## Approval gate

This plan must be approved before implementation begins. Do not treat silence
as approval. On approval, start at stage A and settle `D-BACKTICK` and
`D-METADATA` before any code changes.

## Revision note

2026-08-24 — during EP-M1, moved the removal of the debug-only shell parser
guard for scripts forward from EP-M2. B5 showed that valid script language is
broader than `shlex` accepts, so retaining the guard would violate EP-M1's
explicit regression requirement. The later typed Ninja escaping seam remains
unchanged; it will validate Ninja syntax rather than impose command-shaped
shell validation on scripts.

2026-08-24 — implementation started after the user explicitly approved this
ExecPlan by requesting its implementation as written. The status is now `IN
PROGRESS`; the approval gate and Stage A are complete. The user-approved
resolutions for `D-BACKTICK` and `D-METADATA` are recorded in `Decision log`.
The remaining milestones and verification obligations are unchanged.

2026-08-24 — completed EP-M0 through EP-M4. The initially red differential
matrix became green after the script-lowering, typed escaping, and path
validation milestones. Full gates exposed and corrected stale `$$` examples,
scalar-command newlines in documentation macros, and command-list fixtures.
The final full deterministic run and gate-first CodeRabbit review passed with
zero findings; this plan is now `COMPLETE`.

2026-08-26 — rebasing the completed implementation onto `origin/main` at
`1d0cb167` required a deliberate integration with main's Ninja-generation
split and serial-dependency work. Main now owns ADR-011 through ADR-013, so
the escaping decision record is renumbered to ADR-014. The resolution keeps
main's `src/ninja_gen/mod.rs` layout, dyndep/path validation, and relocated
command-list process tests while retaining the completed shell-text escaping
seam, script placeholder lowering, required-Ninja CI contract, and migration
guidance. The status is temporarily `IN PROGRESS` until the named post-rebase
gates complete.

2026-08-27 — post-rebase gates found three integration repairs. Main added
`BuildEdge::dependency_order` and made private-item documentation mandatory,
so the dollar-escaping fixture now supplies `DependencyOrder::Parallel` and
the moved private helpers are documented. The rebase initially retained the
old branch's `validate_paths` guard, which rejected `$`, spaces, and colons
despite main's `path_syntax` explicitly escaping them; the stale guard was
removed and its test now covers only Ninja-unsupported `|`, tab, and newline.
Finally, the real-Ninja serial-runtime fixture supplied its arithmetic shell
expansion as the old Ninja-ready `$$((...))`. The new backend correctly doubles
raw shell dollars, so the fixture now supplies raw `$((...))` and `$i`. A
focused nextest run passes. The full named gates and final review remain
pending; status stays `IN PROGRESS`.

2026-08-27 — final post-rebase verification passed: `make check-fmt`,
`make test` (2,417 tests and doctests), `make typecheck`, `make lint`,
`make markdownlint`, `make nixie`, and `make test-workflow-contracts` (55
tests). `coderabbit review --agent` then completed with zero findings. The
rebase and its compatibility repairs are complete; status is `COMPLETE`.

2026-08-27 — post-implementation review reopened the plan. The escape
constructor borrowed `ShellText`, contradicting I3's ownership proof because a
caller could escape the same source text twice. The scalar property inspected
generated text without invoking Ninja and did not guarantee a `${…}` case.
Likewise, B2 and the BDD sentinel scenario asserted generated text without
observing a child shell. The corrective work restores a consuming conversion,
uses `ninja -t commands` for scalar properties with a deliberate `${…}`
control, executes B2 through real Ninja, and makes the BDD scenario assert the
produced file. Status is `IN PROGRESS` until the deterministic gates and a
gate-first CodeRabbit review pass.

2026-08-27 — the real-Ninja property support exceeded the repository's
400-line module limit when kept alongside the existing formatting properties.
It now lives in the private `ninja_gen_property_tests::ninja_oracle` test
submodule. Its scope is solely creating ephemeral `build.ninja` files and
querying `ninja -t commands out` for scalar-property cases; callers keep the
property and expected IR text in the parent module. It is not a production
abstraction or a general integration-test helper.

2026-08-27 — the correction set is complete. Focused real-Ninja checks passed:
the 128-case scalar differential property, all 21 dollar-escaping integration
tests, and the executable BDD sentinel scenario. The final deterministic suite
passed `make check-fmt`, `make test` (2,418 passed, 3 skipped), `make
typecheck`, `make lint`, `make markdownlint`, and `make nixie`. Gate-first
`coderabbit review --agent` completed with zero findings. I3 now consumes
`ShellText`, B2 runs through real Ninja with its declared input seeded, and
the BDD scenario observes the target's output. Status is `COMPLETE`.

2026-08-17 —

2026-08-17 — rebased onto `origin/main` at `7e5c2679` ("Add target
descriptions and netsuke help targets"). The rebase itself was clean: this
branch adds only one file. However, that commit restructured
`src/manifest/render.rs` and shifted every documentation section this plan
cites, so all line references were re-resolved against the new tree.

What changed. `render.rs` now routes description and recipe rendering through
the shared helpers `render_description` and `render_recipe`, so the EP-M1 edit
in stage C is a single-arm change to `render_recipe` rather than two separate
call sites. Targets gained descriptions, consumed by the new
`netsuke help targets` catalogue, while `src/ir/from_manifest.rs` deliberately
keeps target descriptions out of the generated Ninja file. Documentation
citations moved: users' guide 1152-1153 to 1208-1209, design §2.6 499-508 to
507-516, design §5.4 2041-2049 to 2059-2067, the `description = CC $out`
snippet 2054-2058 to 2072-2076, the backtick contract 257 to 265, and the
developers' guide anchor 199 to 266.

Why it matters. Decision `D-METADATA` now matches the writer: metadata remains
backend-neutral in the IR, then receives Ninja escaping only at emission. The
help catalogue continues to consume the unescaped IR description, so the
backend boundary does not leak Ninja syntax into non-Ninja consumers.

Effect on remaining work. The metadata emission contract is now explicit and
must be covered by the pending deterministic gates. EP-M1 remains complete;
the plan is `IN PROGRESS` while this review repair is validated.

2026-08-28 — Windows CI exposed raw CRLF output from `ninja -t commands`.
The property now removes only one final CRLF or LF terminator, rather than
trimming command text. This preserves trailing spaces and exposes every other
output difference. During the same reconciliation, the shared path validator
was restored to the approved EP-M3 policy: reject dollar, space, colon, pipe,
and control characters across ordinary build lines, defaults, and dyndep
sidecars. The rebase had retained main's path escaping despite the ExecPlan,
ADR-014, and developers' guide requiring rejection. The revised matrix covers
the three Ninja metacharacters in every emitted field. Status is `IN PROGRESS`
until the focused and complete gates and CodeRabbit review pass.

2026-08-28 — final evidence: the focused all-target real-Ninja property passed
locally; `make check-fmt`, `make test` (2,440 passed, 3 skipped), `make
typecheck`, `make lint`, `make doc-coverage` (98.99%), `make markdownlint`,
and `make nixie` passed. Gate-first `coderabbit review --agent` returned zero
findings. Status is `COMPLETE`; the pushed change will rerun the affected
Windows job for platform confirmation.

2026-08-28 — the Windows rerun confirmed that the CRLF correction passes the
real-Ninja property. Its only failure was the BDD sentinel recipe: Ninja uses
`cmd.exe` on Windows, whereas the fixture used POSIX-only `printf` and `$VAR`
syntax, so it never wrote the output file. A first cross-platform `echo`
replacement also failed because Ninja's Windows launcher does not reliably
resolve the shell built-in. A second attempt proved that Windows Ninja invokes
the command directly, so neither POSIX `$VAR` nor `cmd.exe` `%VAR%` expansion
occurs. The BDD scenario therefore verifies the cross-platform contract it can
observe: a generated command receives the explicit sentinel environment and
writes it to its target through the hosted `python` executable. The independent
real-Ninja property remains the shell-dollar parser oracle. `make check-fmt`,
`make test` (2,440 passed, 3 skipped), `make typecheck`, `make lint`, `make
doc-coverage` (98.99%), `make markdownlint`, and `make nixie` passed.
CodeRabbit reported zero findings. Status is `COMPLETE` pending the final
pushed Windows rerun.

2026-08-28 — that final Windows rerun confirmed both the CRLF-normalized
real-Ninja property and the user-visible BDD sentinel scenario. It then
exposed B2's platform boundary: the end-to-end script regression invokes the
documented `/bin/sh -e` backend, which is intentionally unavailable on
Windows. The real-execution regression now runs only on Unix, while B5
continues to validate script lowering on every platform. The next Windows run
will confirm this platform-specific test selection; status remains
`IN PROGRESS` until then.

2026-08-28 — the Windows selection rerun correctly skipped B2, and the
real-Ninja CRLF property and BDD sentinel scenario passed. It found one more
test with the same platform assumption: the shell-default execution test uses
the POSIX-only `printf` command and `${…}` expansion. It is now Unix-only for
the same reason as B2. Parser assertions remain cross-platform, and the BDD
scenario continues to exercise explicit child-process environment propagation
on Windows. Status remains `IN PROGRESS` pending its corrected Windows rerun.

2026-08-28 — the corrected Windows rerun reached Clippy before tests and
reported `ninja_output` as unused on Windows once its only two POSIX-shell
callers were Unix-only. It also exposed that the workspace's stored capability
directory was only read by that helper. The helper is therefore Unix-only, and
the constructor now uses the stored directory when writing `build.ninja`. This
preserves the capability-based workspace design and avoids a warning
suppression. Status remains `IN PROGRESS` pending another Windows rerun.

2026-08-28 — local Clippy then found that the temporary-directory binding
shadowed the completed `NinjaWorkspace`. It has been renamed to make the
ownership transition explicit. The Linux functional gates passed before that
lint-only correction; the full gate-first review sequence will rerun before
the next Windows confirmation.

2026-08-28 — final platform confirmation succeeded: GitHub Actions run
`33142627259` completed successfully, including `build-test-windows`. Its
Clippy, Whitaker, and full test steps all passed. This confirms that the
real-Ninja property accepts both CRLF and LF records without trimming command
whitespace; the prior Windows failure was eliminated, and no property failure
is hidden by record-terminator normalization. Status is `COMPLETE`.

2026-08-28 — review repair: documentation was reconciled with the current
implementation. Metadata fields are escaped at Ninja emission, while the IR
and help catalogue retain backend-neutral text; the ADR and design documents
now state that boundary. The developers' guide documents the required-Ninja
gate, and the users' and migration guides document rejected path and metadata
characters. Status returns to `IN PROGRESS` until deterministic validation and
the requested follow-up review complete.

2026-08-28 — focused B5 real-Ninja execution exposed an unexpected-EOF defect
in the script wrapper: `escape_script` used the wrong apostrophe sequence for
its double-quoted `/bin/sh -c` payload. The wrapper now emits `r"'\\''"`,
and the focused heredoc test passes with the observed output
`script inputdone\n`. Status remains `IN PROGRESS` pending the full gates and
follow-up review.
