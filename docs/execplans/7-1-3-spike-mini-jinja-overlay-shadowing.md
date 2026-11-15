# Spike MiniJinja overlay shadowing for macro substitution (7.1.3)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: DRAFT

Branch: `7-1-3-spike-mini-jinja-overlay-shadowing`

Roadmap item: `7.1.3` in `docs/roadmap.md`.

The plan has been revised after a four-perspective expert review and awaits
explicit approval. Approval also accepts the design deviations listed under
`Proposed deviations requiring approval`. Implementation must not begin before
then. Publishing this document is not evidence that any experiment has run, and
roadmap 7.1.3 stays unchecked until EP-M5 closes.

## Purpose / big picture

Netsuke's planned `netsuke test` command (roadmap phase 7) lets a test replace
a manifest macro with a *stand-in*, or *spy* on it, while the rest of the
manifest compiles through the same pipeline as a real build. A stand-in is a
test-supplied macro that answers instead of the original. A spy records each
call and then passes it through to the original macro. Both need a *journal*, a
record of every call that assertions later inspect.

Today a manifest such as

```yaml
macros:
  - signature: "compile_cmd(src, flags='')"
    body: "cc {{ flags }} -c {{ src }}"
targets:
  - name: main.o
    command: "{{ compile_cmd('main.c') }}"
```

compiles `compile_cmd` into an internal template named
`__manifest_macro_0_compile_cmd` and makes it reachable in three ways. First,
it is an environment *global*: a Rust callable built by `make_macro_fn`.
Second, it appears in the *prelude*, a string of
`{% from '<template>' import <name> %}` statements stored in the
`__netsuke_manifest_macro_imports` global (`MACRO_IMPORTS_GLOBAL`) and
prepended to every rendered template. Third, it is captured inside the bodies
of other manifest macros when the prelude imports them. MiniJinja resolves a
template's local names (including imports) before globals, so replacing the
global alone with `Environment::add_function` changes nothing for a direct
template call. Technical design §5.4 therefore says that macro substitution
must rewrite the prelude, and roadmap 7.1.3 asks for a *spike*, a bounded
experiment, that proves how.

This spike delivers executable evidence, not the user-facing feature. After it
lands, a maintainer can run the suites named in `Concrete steps` and observe:

1. Characterization tests pin the eight engine facts (`MJ-1` to `MJ-8`, under
   `Verification plan`) that constrain the design. They show that
   `add_function` replaces a global, that a prelude import shadows it, that a
   macro body captures other macros at build time, and that a captured Rust
   handle survives replacement but cannot carry a call block (`{% call %}`)
   across MiniJinja render states.
2. A test-only installer rebinds each selected prelude record, in place, to a
   generated *journalling wrapper macro*. In substitute mode the wrapper
   delegates to a compiled stand-in; in spy mode it delegates to the original
   template of the record it replaces. Every dispatched call is observed
   exactly once, on the template path, the expression path, and from inside
   other macros. Unselected macros, call blocks, strict undefined behaviour,
   telemetry, and resource budgets keep their current behaviour.
3. Two negative controls (NC, a deliberately defective variant kept as a
   permanent test) demonstrate that the observation assertions reject the two
   defective designs: global replacement alone (NC1), and the literal §5.4
   reading of importing the bare stand-in (NC2). A third control (NC3) shows
   that the recursion guard catches a self-calling wrapper.
4. Either the rebinding mechanism passes every acceptance item and the
   filtered-registration fallback is recorded as unnecessary with that
   evidence, or the fallback is built and passes the same items.

Ordinary builds, manifest queries, the public `TemplateOverlays` API, and every
user-visible output stay unchanged. Promotion into the runner belongs to
roadmap 7.3.3, which depends on this spike.

## Proposed deviations requiring approval

Each item below departs from an approved upstream document. The execplans
procedure requires acceptance before implementation; approving this plan
accepts all of them. Each lists its affected identifiers, the options, and the
document change made in EP-M5.

- DEV-1, wrapper instead of bare stand-in (TD-5.4, lines 448-450). §5.4 says
  the prelude entry is re-added "bound to the stand-in's compiled template".
  That import journals nothing on the template path, contradicting §5.4's own
  journalling requirement and UX §8.5. Options: (a) rebind to a generated
  journalling wrapper (chosen, D1); (b) drop the import and fall through to a
  Rust global wrapper, which loses call blocks (MJ-3). Change: rewrite §5.4 and
  record the decision in a new ADR.
- DEV-2, spy capture by template reference (TD-7, lines ~655-661;
  `docs/adrs/adr-0070-deny-by-default-test-mode-stdlib.md:113`). Both say a spy
  resolves to the handle captured from `Environment::globals`
  (`OverlayCall::shadowed`). For macros that handle cannot carry call blocks
  (MJ-3). Options: (a) capture each prelude record's template (chosen, D3); (b)
  keep the value handle and declare call blocks unsupported for spied macros.
  Change: amend §7 for macros only; the new ADR records that it refines
  ADR-0070's spy statement for macros while functions keep `shadowed`.
- DEV-3, `macro_substitutions` stays deferred (TD-4.3, line 274). §4.3
  assigns the `macro_substitutions` loader option to 7.1.3. This spike adds no
  public option, because the journal (7.3.1) and the name rule (ADR-0070) must
  be designed first. Change: §4.3 reassigns the field to 7.3.3.
- DEV-4, test-only installer (TD-4.3, line ~318, constraint C9: overlays are
  "not a test-only `cfg`"). The spike installer is `#[cfg(test)]` because it
  has no production caller yet, and AGENTS.md forbids dead-code expectations.
  Change: §4.3 notes the spike exemption; 7.3.3 removes the gate when it wires
  the installer through `TemplateOverlays`.
- DEV-5, §14 correction (TD-14, lines 1144-1149). §14 says 7.1.2 settled
  replacement and capture. 7.1.2 tested functions only
  (`src/manifest/tests/overlays/registration.rs:140`). Change: §14 cites this
  spike's tests instead.

## Constraints

- Implementation begins only after explicit user approval of this plan,
  including the deviations above.
- Use the locked MiniJinja dependency (2.24.0, default features plus `fuel`
  and `loader`) and the production macro compilation, invocation, rendering,
  and resource-budget paths. Do not emulate the engine in tests.
- Preserve ordinary builds and manifest queries exactly: rendered output,
  error kinds, localized diagnostics, telemetry (ADR-0009), budget accounting
  (ADR-0050), and the type and text of the `__netsuke_manifest_macro_imports`
  global, which templates can read and whose length is charged to the source
  budget.
- Do not change the public API. `TemplateOverlays`, `OverlayCallable`,
  `OverlayCall`, `TestSurface`, and `ManifestLoadOptions` stay as they are, and
  `TemplateOverlays::with_function` keeps rejecting manifest-macro names
  (`src/manifest/test_surface/overlays.rs:249-250`).
- Production changes are limited to these behaviour-neutral refactors, all
  inside `src/manifest/jinja_macros/`:
  1. a new `imports.rs` holding the moved `register_macro_import` and
     `macro_imports`, plus two extracted pure helpers,
     `macro_template_name(index, name)` and `import_statement(template, name)`;
  2. a `compile_macro_template` helper extracted from
     `register_macro_with_budget` (source charge, `add_template_owned`, and
     macro validation), still called by production registration;
  3. widening `capture_macro` in `invocation.rs` from private to
     `pub(super)`.
  The substitution installer, its kernel, and its observer are compiled only
  under `#[cfg(test)]` in `src/manifest/jinja_macros/substitution/`. No
  `#[expect(dead_code)]` is permitted; anything only the installer uses is
  itself `#[cfg(test)]`.
- Separate policy from engine adaptation. The rebinding kernel (which records
  bind which names) is a pure function over plain data. Template generation,
  registration, and callable construction stay in the MiniJinja adapter. Do not
  add a port trait or a general-purpose abstraction.
- No new configuration surface, CLI flag, or environment input is needed.
  Existing `ortho_config` layering and localized help are untouched. Any later
  configuration belongs to 7.3.x and must use `ortho_config`
  (`docs/ortho-config-users-guide.md`).
- No `unsafe`, no lifetime extension of MiniJinja `State` or `Captured` values,
  no in-process environment mutation (AGENTS.md; ADR-0008), no network or
  command execution, and no new dependencies. `rstest`, `rstest-bdd`,
  `googletest`, `pretty_assertions`, `insta`, `proptest`, and `metrics-util`
  are already dev-dependencies.
- Code files stay at or below 400 lines. `src/manifest/jinja_macros/mod.rs`
  has 374 lines and `invocation.rs` has 381, so new logic goes in new files.
- Respect the Polonius posture and pinned nightly (ADR-0006;
  `docs/polonius.md`): lookups return references, keys are cloned only on
  insertion, and no `-Z` flags are added.
- Follow the AGENTS.md test rules: every `assert!`, `assert_eq!` (including
  `pretty_assertions::assert_eq!`) and `prop_assert*` carries a message;
  fallible fixtures return `anyhow::Result` and tests return `Result`; no
  `expect` in shared fixtures; every module has `//!` and every helper `///`
  documentation.
- Never bypass compiler admission, isolate the Cargo cache, or use `/tmp` as a
  build target. Run gates sequentially, one at a time.
- Do not rewrite completed ExecPlans. Do not tick 7.1.4 or 7.3.3.

## Tolerances (exception triggers)

ExecPlan files are excluded from every count, including this file. The
comparison base is the literal commit `5c707ef33`; if the branch is rebased
again, update this line and record the new base under `Decision log`. Count
committed work only (stage new files with `git add -N` first when counting a
dirty tree):

```sh
git diff --numstat 5c707ef33 HEAD -- . ':(exclude)docs/execplans/' |
  awk '{ net = $1 - $2
         cat = ($3 ~ /^src\// || $3 ~ /proptest-regressions\//) ? "code" \
             : ($3 ~ /^docs\//) ? "docs" : "config"
         lines[cat] += net; files[cat]++ }
       END { for (c in lines) printf "%s files=%d net=%d\n", c, files[c], lines[c] }'
```

The metric is net lines (additions minus deletions). Code is everything under
`src/` plus committed proptest regression files. Documentation is `docs/` except
`docs/execplans/`. Configuration is everything else, chiefly
`tools/kani/proof-scope.toml`; a regenerated `typos.toml` or edited
`typos.local.toml` also counts here. `make fmt` reformats Markdown across the
repository; revert any unrelated file it touches before counting. Each category
is enforced independently.

The budgets follow from the file enumeration in `Context and orientation`:
about 870 production and 2,350 test lines (3,220 code lines in 17 files), 1
configuration line, and about 360 documentation lines in 7 files. The history
there shows earlier plans under-estimating test volume and documentation
fan-out, so each budget carries roughly 20% contingency.

- Files: stop if more than 20 code files, 2 configuration files, or 9
  documentation files change.
- Code: stop above 3,900 net lines.
- Configuration: stop above 10 net lines.
- Documentation: stop above 450 net lines.
- Interface: stop if any public item changes, or if a production change goes
  beyond the three refactors named in `Constraints`.
- Dependencies: stop if any dependency or feature flag would be added.
- Architecture: stop if the evidence requires changing ADR-0070's name rule,
  the loader registration order (TD-4.3), or ordinary-build semantics.
- Semantics: stop if both the rebinding mechanism and the filtered fallback
  fail any acceptance item from A1 to A10. Do not narrow the acceptance
  criteria.
- Axioms: stop if an EP-M1 characterization falsifies an engine axiom that a
  decision depends on; revise the affected decisions and seek re-approval.
- Iterations: stop if a focused test still fails after five fix attempts on the
  same root cause.
- Ambiguity: stop and present options if the journal shape needs a user-facing
  decision beyond the spike's recorded, reversible choice (D4).

There is no wall-clock limit. A healthy, queued compiler-admission pool is not
a failure; record infrastructure faults separately from semantic failures.

## Risks

- Risk: the generated wrapper changes macro semantics (default evaluation,
  strict undefined handling, keyword validation, `caller` acceptance in either
  style). Severity: high. Likelihood: medium. Mitigation: build the wrapper
  from the engine's own `arguments` and `caller` attributes, never by parsing
  signatures, and compare against the unsubstituted environment in a property
  over generated signatures and call shapes (A6).
- Risk: a test passes on output text while the wrapper is bypassed, calls the
  target twice, or (in substitute mode) also runs the original. Severity: high.
  Likelihood: high, since the literal §5.4 design bypasses the wrapper.
  Mitigation: every fixture macro increments a *probe*, a counting test global,
  so tests assert observation counts, target counts, and original counts
  together; NC1 and NC2 run through the same verdict function as the real
  mechanism.
- Risk: calls between manifest macros depend on prelude order, because a macro
  body captures earlier imports at build time and otherwise the global (MJ-8).
  A rebinding that appends records, or that wraps only the last record of a
  duplicated name, changes which macro an earlier-defined caller reaches.
  Severity: high. Likelihood: certain without in-place, per-record rebinding.
  Mitigation: D2 rebinds every record in place to its own wrapper; A3 checks
  bindings from inside macro bodies as well as at template level.
- Risk: spy passthrough through the captured Rust global loses call blocks.
  Severity: high. Likelihood: certain (MJ-3). Mitigation: capture by template
  reference (D3), and pin the value-handle failure as a characterization.
- Risk: a stand-in that calls the public name recurses through the global,
  where each hop starts a fresh state at depth zero, so only fuel stops it and
  the Rust stack may overflow first. Severity: medium. Likelihood: low.
  Mitigation: the installer rejects such a stand-in at installation (D6), and
  A5 pins the rejection.
- Risk: the wrapper's extra import and call frame change budget accounting
  (fuel per render, recursion depth, source bytes). Severity: medium.
  Likelihood: medium. Mitigation: ordinary builds stay byte-identical; for
  substituted environments assert typed exhaustion at each boundary, the exact
  source-byte limit of the rewritten prelude, and recursion-limit parity (A8).
- Risk: the record list the installer rebuilds drifts from the live prelude.
  Severity: medium. Likelihood: low. Mitigation: production and installer share
  `macro_template_name` and `import_statement`, and the installer compares its
  rebuilt text with the live global before rewriting, failing on mismatch (A2).
- Risk: a new production module trips the Kani proof-scope contract
  (`tests/workflow_contracts/kani_proof_scope_test.py`). Severity: low.
  Likelihood: certain, since `src/manifest/jinja_macros/mod.rs` is already in
  `tools/kani/proof-scope.toml`. Mitigation: add exactly the path the contract
  names when `make test-workflow-contracts` reports it.
- Risk: the upstream documents disagree, and the spike silently picks a side.
  Severity: medium. Likelihood: certain. Mitigation: DEV-1 to DEV-5 make each
  choice explicit; the UX journal-namespace conflict (`mocks.<name>` in UX §8.4
  versus `substitutes.<name>` in UX §8.5) is recorded as an open question for
  7.3.1, not resolved here.

## Progress

- [x] (2026-11-04) First draft: two Wyvern passes, six-perspective review,
  draft PR #1088 opened against baseline `b99c9c92b`.
- [x] (2026-11-15T00:05+01:00) Adopted the existing planning branch, rebased
  it onto `main` at `5c707ef33`, and confirmed that the macro, overlay, and
  loader sources are unchanged since the first draft.
- [x] (2026-11-15T00:20+01:00) Three Wyvern reconnaissance passes covered
  design documents, gate infrastructure, and macro and overlay code. MiniJinja
  2.24.0 source reading and Firecrawl research established the engine axioms.
- [x] (2026-11-15T00:30+01:00) Rewrote the plan; committed `1556fc3c5` and
  pushed it to `origin/7-1-3-spike-mini-jinja-overlay-shadowing` with
  `--force-with-lease` after the documentation gates passed.
- [x] (2026-11-15T00:50+01:00) Four-perspective expert review (engine
  semantics, verification strategy, architecture and scope, process and
  documentation) returned 4 blockers and about 30 major findings.
- [x] (2026-11-15T01:30+01:00) Revised the plan to absorb every finding
  (see `Revision note`).
- [ ] Run the full gate set, commit, push, and refresh draft PR #1088.
- [ ] Obtain explicit implementation approval, including DEV-1 to DEV-5.
- [ ] EP-M1: characterize the engine boundary.
- [ ] EP-M2: extract the import and compile helpers.
- [ ] EP-M3: prove per-record prelude rebinding with journalling wrappers.
- [ ] EP-M4: decide the fallback: record it as unnecessary, or build it.
- [ ] EP-M5: record the design, write the ADR, and mark 7.1.3 done.

## Surprises & discoveries

- Observation: `when` expressions and string `foreach` expressions never see
  the prelude. Evidence: `evaluate_with_state`
  (`src/manifest/jinja_macros/expression.rs:24-57`) renders
  `{% set __netsuke_expression_result = (...) %}` without imports. Impact:
  calls there resolve through the global, so one observation mechanism must
  cover both paths.
- Observation: a MiniJinja macro value records the identity of the render state
  that built it, and `Macro::call` refuses a call from any other state. Imports
  and nested evaluation keep the importer's identity. Evidence:
  `vm/macro_object.rs:47-53`, `vm/mod.rs:118-150`, `vm/mod.rs:831-900`; the
  maintainer's write-up quotes the same check.[^learnings] Impact: a call block
  created in the render state cannot be invoked inside the fresh state that
  `make_macro_fn` builds.
- Observation: a macro body binds its free names when the macro is built, not
  when it is called. `Enclose` stores `Context::load(name)`, which walks the
  importer's frames before falling back to globals (`vm/context.rs:193-201`,
  `:240-262`). Impact: a macro calling a manifest macro imported *earlier*
  reaches the imported value in the same state, so call blocks work; one
  calling a macro imported *later* reaches the global, so call blocks fail with
  "template state went away". This existing, user-visible order-dependence is
  untested today, and ADR-0009's claim that imports "never reach
  `make_macro_fn`" (its lines 51-55) is inaccurate for it.
- Observation: `make_macro_fn` captures a template name, a macro name, and a
  budget, not a macro value (`invocation.rs:32-55`). Impact: the captured
  global stays callable from later states for as long as its template stays
  registered.
- Observation: Netsuke's own call-block tests declare `caller` as an ordinary
  parameter (`src/manifest/tests/macros/mod.rs:38`, `:106-108`). A declared
  `caller` appears in the macro's `arguments`, and its `caller` attribute is
  false (`compiler/codegen.rs:505-506`). Impact: the wrapper must detect both
  styles, and the journal must not store the state-bound caller macro.
- Observation: the docs.rs syntax summary, extracted through Firecrawl, claims
  that macros accept unlimited positional and keyword arguments. The 2.24.0
  source contradicts it: signatures allow plain identifiers only
  (`compiler/parser.rs:1253-1276`), and `Macro::call` returns
  `TooManyArguments` and rejects unknown keywords. Impact: the wrapper's
  parameter list taken from `arguments` is complete and exact.
- Observation: technical design §5.4 contradicts itself, §14 overstates what
  7.1.2 settled, and §4.3 assigns a loader option to this item that it cannot
  responsibly add. Impact: DEV-1 to DEV-5.
- Observation: `make proptest` selects modules whose path segment ends in
  `proptests` or `property_tests` (`Makefile:325`); none of the existing
  property modules under `src/manifest` match, and a misnamed module would be
  skipped silently, because the filter already matches other tests. Impact:
  name the new module `substitution_property_tests` and verify selection with a
  listing count (`Concrete steps`).

## Decision log

Entries are by the planning agent unless stated otherwise.

- D1 (2026-11-15). Decision: rebind prelude records to generated journalling
  wrapper macros rather than to the bare stand-in (DEV-1). Rationale: a bare
  import journals nothing on the template path; dropping the import loses call
  blocks (MJ-3). Reversible: yes; the mechanism is internal and test-only until
  7.3.3.
- D2 (2026-11-15). Decision: rebind every record of a selected name in place,
  each to its own wrapper template, and set the global to `make_macro_fn` over
  the wrapper of the name's last record. Rationale: MJ-8 makes macro-to-macro
  calls depend on record position; appending, or wrapping only the last record
  of a duplicated name, would change which macro earlier-defined callers reach.
  One wrapper serves all three routes (template, expression, in-macro), so the
  observation record is identical on each.
- D3 (2026-11-15). Decision: a spy wrapper targets the template of the record
  it replaces, captured before rebinding with the macro's `arguments` and
  `caller` attributes (DEV-2). Rationale: a value handle cannot carry call
  blocks (MJ-3), and a per-record target preserves duplicate-name semantics.
  The global-value capture is still pinned as working for calls without
  `caller`, because §7 relies on it for functions.
- D4 (2026-11-15). Decision: the spike's observation records the public name,
  the wrapper's bound parameters in declaration order (omitted arguments as
  undefined), and whether a call block was supplied (either `caller` style); a
  caller macro is replaced by a marker and never stored. The observation is
  made before delegation and does not record the outcome. Spy recursion inside
  an original reaches the original directly (MJ-8 self-binding), so only the
  outermost call is observed. Rationale: this preserves the omitted/passed
  distinction and lets the target apply its own defaults; mapping to the UX
  `args` shape (UX §8.4) belongs to 7.3.1 and 7.3.3. Reversible: yes.
- D5 (2026-11-15, revised after review). Decision: keep the prelude a `String`
  global. Extract `macro_template_name` and `import_statement` so production
  and the installer share one format; the `cfg(test)` installer rebuilds the
  ordered record list from the manifest's macro definitions, checks it against
  the live global, rebinds, and writes the new text back. Rationale: the
  earlier typed-table design changed what templates see under the global, added
  an object downcast to every render, and touched telemetry's
  `has_macro_imports`. This design keeps production byte-identical by
  construction and still gives the kernel typed records. A typed table is
  deferred to 7.3.3 if production rebinding needs it. Alternative rejected:
  parsing the prelude string back into records.
- D6 (2026-11-15). Decision: in substitute mode the wrapper takes the
  stand-in's signature, so arity is the stand-in's and is validated at call
  time (TD-5.4, "as with ordinary manifest macros"); the observation records
  the stand-in's parameters under the public name. Installation rejects a
  stand-in whose undeclared names include the public name, unless the stand-in
  is itself named that (self-recursion stays in-state). Rationale: UX §8.5's
  example stand-in has different parameter names from the original, so keyword
  forwarding by the original's names cannot work; the rejection prevents a
  global-hop loop bounded only by fuel.
- D7 (2026-11-15). Decision: do not amend ADR-0070 or the `TemplateOverlays`
  name rule in this spike. EP-M5 writes a new ADR and adds 7.3.3 handoff
  sub-bullets to the roadmap: promote the installer through `TemplateOverlays`,
  amend ADR-0070, settle the journal namespace with 7.3.1, and decide whether
  manifest templates may reach the observer global.
- D8 (2026-11-15). Decision: Verus and Kani are not selected (see
  `Formal verification decision`).
- D9 (2026-11-15). Decision: seeded faults are applied as uncommitted patches
  (`git apply`, run, `git apply -R`), never committed and never stashed; their
  patch text and failing test names go under `Artefacts`. They are not
  registered in `SUPPLEMENTAL_PROPERTY_PATCHES`
  (`tests/kani_mutation_evidence_tests/contracts.rs:37-51`), whose list is kept
  narrow and whose targets are production properties, while this spike's kernel
  is test-only. Proptest regression entries produced by fault runs are deleted;
  only regressions from genuine failures are committed.
- D10 (2026-11-15). Decision: installation charges stand-in and wrapper source
  to the shared `ManifestBudget` through `compile_macro_template`. A failed
  installation leaves the environment unchanged but does not refund those
  charges, matching how a failed manifest load consumes budget. A7 asserts the
  remaining budget. Rationale: `ManifestBudget` has no source refund, and a
  test load that fails installation is abandoned.
- D11 (2026-11-15). Decision: the installer rejects, with typed
  `MacroDoubleError` variants, two doubles for one name, a spy for a name with
  no prelude record, a target or wrapper parameter using the
  `__netsuke_double_` prefix, and an existing global or manifest variable named
  like the observer. Rationale: each would shadow the wrapper's enclosed names
  or create the duplicate-double suite error of UX §8.2.
- D12 (2026-11-15). Decision: adopt the existing branch and draft PR #1088,
  rebase onto `main`, and update the remote with `--force-with-lease`. Made at
  the user's direction; the earlier planning session is retired.

## Outcomes & retrospective

Planning only. No experiment, implementation, or Rust gate has run for this
spike. The revision identified the engine facts that rule out the literal §5.4
design and the order-dependence of macro-to-macro calls, replaced the design
with per-record journalling wrappers, and stated the evidence each milestone
must produce. Fill this section with results, rejected candidates, and lessons
at each milestone.

## Context and orientation

Baseline: `5c707ef33` (`origin/main`, 2026-11-15); the planning branch is
rebased onto it. `Cargo.toml:116` requires `minijinja = "2.24.0"` with `fuel`
and `loader` on top of default features; `Cargo.lock` resolves 2.24.0. The
package is `netsuke-build` and its library crate is `netsuke`.
`rust-toolchain.toml` pins the dated nightly that enables Polonius Alpha and
the next-generation trait solver.

Environment construction lives in `src/manifest/mod.rs:193-233`
(`build_template_environment`). It creates an `Environment<'static>` with
`UndefinedBehavior::Strict`, then registers host helpers, the stdlib, manifest
`vars`, manifest macros (`register_manifest_macros_with_budget`), and finally
test overlays (`register_overlays`). Expansion (`expand_foreach_with_budget`,
`src/manifest/expand/mod.rs:145`) and rendering (`render_manifest_with_budget`,
`src/manifest/render/mod.rs:30`) follow.

Macro compilation lives in `src/manifest/jinja_macros/mod.rs`.
`register_macro_with_budget` (`:190-215`) parses the name, builds
`{% macro <signature> %}<body>{% endmacro %}` under the template name
`__manifest_macro_{index}_{name}`, charges the source budget, adds the
template, validates it by rendering once (`validate_macro`), appends an import
through `register_macro_import` (`:361-365`), and installs `make_macro_fn` as a
global. `render_template_at` (`:310-358`) prepends the prelude, charges its
length, reserves fuel, renders in a fuel-bounded clone, and maps exhaustion to
typed budget errors; it is the only reader of the prelude. The template path
covers every rendered field (`src/manifest/render/mod.rs`) and the template
fallback for `when` (`render_when_template`). The expression path covers
compiled `when` conditions (`evaluate_when_expression`) and string `foreach`
expressions (`src/manifest/expand/evaluation.rs:216-243`).
`src/manifest/jinja_macros/invocation.rs` owns `make_macro_fn`, `invoke_macro`
(a fresh captured state per call, macro-stage fuel, an output charge), and
`capture_macro`; `call.rs` owns keyword forwarding (`call_macro_value`).

Function overlays live in `src/manifest/test_surface/overlays.rs`.
`register_overlays` validates every name first, then installs each overlay,
capturing the displaced global through `Environment::globals`
(`find_registered`, `:355-360`). The rejection of manifest-macro names is at
`:242-254`.

Existing tests to reuse: `src/manifest/tests/macros/mod.rs` (arguments,
defaults, call blocks, repeated renders); `src/manifest/tests/overlays/`
(function replacement and `shadowed` passthrough, `registration.rs:140`);
`src/manifest/tests/budget/` (budget boundaries; its `small_limits` fixture in
`mod.rs` is private, so EP-M3 makes it `pub(super)` or copies the limits);
`src/manifest/tests/macro_invocation_telemetry.rs` (`DebuggingRecorder` pattern;
`imported_macro_render_does_not_emit_invocation_metrics` at `:253`); and the
`invocation.rs` test module (expression fallback and a proptest whose strategy
A6 reuses).

### File enumeration and budget history

Planned code files. Production: `jinja_macros/imports.rs` (new, ~90 lines),
`jinja_macros/mod.rs` and `invocation.rs` (edited, ~30 net lines), and
`jinja_macros/substitution/{mod.rs, kernel.rs, wrapper.rs}` (new, `cfg(test)`,
~250 lines each). Tests, all new under
`src/manifest/tests/macros/substitution/`: `mod.rs` (fixtures, probe, observer,
verdict function), `characterization.rs`, `kernel.rs`, `rebinding.rs`,
`call_shapes.rs`, `installation.rs`, `telemetry.rs`, `pipeline.rs`, and
`substitution_property_tests/{mod.rs, equivalence.rs}`, about 235 lines each;
plus a one-line edit to `src/manifest/tests/macros/mod.rs`. That is 17 files
and about 3,220 lines. Configuration: one line in
`tools/kani/proof-scope.toml`. Documentation: technical design (~120), the new
ADR (~150), developers' guide (~50), roadmap (~15), `docs/contents.md` (~4),
and possibly `docs/polonius.md` and `docs/repository-layout.md` (~20).

History. The previous revision of this plan allowed 12 files and 800 net lines
combined. Roadmap 7.1.2 set 42 files and 2,400 lines and finished at 81 files
and 6,603 lines, accepted as an exception, largely because whole directories
were counted. Plan 4.4.1 was raised from 14 files and 700 lines to 18 and 1,200
because the original would have fired on the expected path. Plan 7.1.1's
tolerance fired partly because the plan file itself was counted, which this
plan's exclusion prevents. The recurring causes were test volume and
documentation fan-out, not approved scope growth, so the budgets enumerate test
files explicitly and carry roughly 20% contingency.

## Conformance basis

Upstream artefacts at baseline `5c707ef33`:

- `docs/roadmap.md` 7.1.3 (lines 1537-1545): RM-1, pin `add_function`
  replacement; RM-2, rewrite the prelude for substituted names; RM-3, pin
  runner-side handle capture for spy passthrough; RM-4, fall back to filtered
  registration if shadowing fails. Also 7.1.4 (dogfooding, consumes this
  evidence), 7.3.1 (journal), and 7.3.3 (production substitution doubles).
- `docs/netsuke-test-framework-technical-design.md`: TD-4.3 (loader options,
  registration order, overlay name rule, constraint C9), TD-5.4 (macro
  substitution), TD-7 (spy handle, lock release before delegation, journal
  origins), TD-12 (invariants I1 isolation and I4 fidelity), TD-14 (phasing and
  fallback).
- `docs/netsuke-test-framework-ux-design.md` (the user-experience design):
  §8.1 (spy semantics), §8.2 (call entries and duplicate doubles), §8.4 (the
  journal and `calls[n].args`), §8.5 (`substitute`).
- `docs/rfcs/rfc-0007-netsukefile-testing-framework.md`: seams-first rollout;
  the macro substitution overlay is one of two seams.
- `docs/netsuke-design.md` §4.3 (user-defined macros and `caller` support) and
  its resource-budget sections.
- Architectural decision records (ADRs):
  `docs/adrs/adr-0006-adopt-the-polonius-borrow-checker-on-a-pinned-nightly-toolchain.md`,
  `docs/adrs/adr-0008-environment-seam-taxonomy.md`,
  `docs/adrs/adr-0009-bounded-redacted-telemetry-for-manifest-evaluation.md`,
  `docs/adrs/adr-0050-bound-manifest-template-evaluation.md`, and
  `docs/adrs/adr-0070-deny-by-default-test-mode-stdlib.md`.

There is no Terms of Reference revision specific to this item; the project
scope in `docs/terms-of-reference.md` applies unchanged.

Trace links:

- RM-1 → EP-M1 → A1 (`characterization::*`).
- RM-2 → EP-M2 and EP-M3 → A2 to A6, A9, A10.
- RM-3 → EP-M1 and EP-M3 → A1 (value handle) and A7 (template reference).
- RM-4 → EP-M4 → A11.
- TD-12 I1 and I4 → A7 and A8 (isolation, atomic installation, budgets) and
  A2 (ordinary-build fidelity).
- DEV-1 to DEV-5 → EP-M5 documentation and ADR.

## Verification plan

### Abstract model

Model the prelude independently of MiniJinja as a finite sequence `P` of import
records `r = (t, n)`, where `t` is a template name and `n` a public macro name.
The binding of a name in a prefix is the template of its last record there:

```plaintext
bind(P[..k], n) = t   where (t, n) = P[i], i < k, and no j with i < j < k has P[j].n = n
bind(P[..k], n) = ⊥   when no record before position k names n
bind(P, n)      = bind(P[..|P|], n)
```

A selection `S` is a set of public names. A wrapper assignment `w` maps each
record index `i` with `P[i].n ∈ S` to a wrapper template `w(i)`. For substitute
mode, names in `S` with no record get one appended wrapper record. Rebinding is:

```plaintext
rebind(P, w) = [ (w(i) if P[i].n ∈ S else P[i].t, P[i].n) for i in 0..|P| ]
               ++ [ (w(n), n) for n in S where bind(P, n) = ⊥, in selection order ]
```

The engine bridge has three parts. At template level, rendering
`prelude(P) ++ body` binds each name `n` in the body to the macro exported by
`bind(P, n)`, or to the global when that is `⊥` (MJ-1, MJ-2). Inside the body
of the macro imported at position `i`, a free name `n` binds to the macro
itself when `n = P[i].n`, otherwise to `bind(P[..i], n)`, otherwise to the
global current at that render (MJ-8). On the expression path every name binds
to the global (MJ-1). Installation applies `rebind` to the prelude, adds
stand-in and wrapper templates under private names, sets each selected global to
`make_macro_fn` over the wrapper of the name's last record, and registers the
observer, all on a candidate environment that replaces the original only on
success.

### Axioms (trusted, pinned by EP-M1)

These facts come from the MiniJinja 2.24.0 source in the shared Cargo registry
(`~/.cargo/registry/src/*/minijinja-2.24.0/src/`). They are treated as trusted
interface facts, not verified internally, and each is pinned by a
characterization test in A1.

- MJ-1, lookup order: locals, then the render context, then environment
  globals (`vm/context.rs:240-262`).
- MJ-2, imports: `{% from T import n as a %}` evaluates `T` in the current
  state and assigns `a` as a local; a later assignment overwrites an earlier one
  (`compiler/codegen.rs:405-418`).
- MJ-3, state identity: a macro records `state.id` when built and refuses a
  call from a state with a different identity; imports and `eval_macro` keep
  the identity (`vm/macro_object.rs:47-53`, `vm/mod.rs:118-150`, `:1118-1136`).
  The refusal happens when the stale macro is called, for example when a target
  invokes a foreign `caller()`.
- MJ-4, globals: `add_function` is `add_global`, a map insert that replaces any
  existing value; `globals()` yields clones (`environment.rs:761-790`).
- MJ-5, cloning: `Environment::clone` copies globals copy-on-write and copies
  the template store, so later inserts on a clone do not leak
  (`environment.rs:70-85`, `loader.rs:26-31`).
- MJ-6, undefined values: macro defaults apply exactly when an argument is
  undefined (`codegen.rs:489-499`). Under strict undefined behaviour,
  string-typed Rust arguments (`String`, `Cow<str>`, string input) reject
  undefined, while `Value`, `&Value`, and `Rest<Value>` accept it; macro
  arguments and list elements accept it (`value/argtypes.rs:609-713`,
  `:800-823`, `:1082-1121`; `vm/mod.rs:426-434`).
- MJ-7, arity and `caller`: extra positional arguments fail with
  `TooManyArguments`, unknown keywords and duplicate arguments fail, and a
  missing argument becomes undefined. A macro accepts `caller` when its body
  references `caller` without declaring it (its `caller` attribute is true) or
  when `caller` is a declared parameter (it appears in `arguments`). The
  compiled macro exposes `arguments` and `caller` attributes
  (`vm/macro_object.rs:31-121`; `codegen.rs:505-526`).
- MJ-8, build-time capture: when a macro is built, each free name in its body
  is stored in its closure from `Context::load`, so it binds to the macro
  itself (self-reference), else to an earlier import in the importing frames,
  else to the global current at that moment (`vm/context.rs:176-201`;
  `vm/mod.rs:779-790`; `codegen.rs:535-537`).

### Invariants and lemmas

The lemma sketches are hypotheses about the proof structure; implementation may
show some to be unnecessary or in need of refinement.

- INV-1, positional rebinding: `rebind(P, w)` has the records of `P` at the
  same positions with the same names; a selected record `i` carries `w(i)` and
  an unselected record is unchanged; appended records name only selected names
  absent from `P`. L-1 (prefix binding): for every prefix `k` and selected `n`,
  `bind(rebind(P, w)[..k], n) = w(i*)` where `i*` is the index of
  `bind(P[..k], n)`. Sketch: the map preserves names and positions, so the last
  record naming `n` before `k` keeps its index.
- INV-2, frame preservation: for every prefix `k` and unselected `n`,
  `bind(rebind(P, w)[..k], n) = bind(P[..k], n)`. Sketch: unselected records
  are untouched and appended records come after every original position.
- INV-3, prelude fidelity: the record list rebuilt from the manifest's macro
  definitions renders byte-identically to the live
  `__netsuke_manifest_macro_imports` global for every manifest, and a manifest
  with no macros has no such global. After EP-M2, the global's text for any
  manifest equals its text before EP-M2.
- INV-4, single dispatch: every call the wrapper accepts produces exactly one
  observation followed by exactly one target invocation. A call rejected by the
  wrapper's arity, unknown-keyword, or duplicate-argument checks produces
  neither. A call accepted by the wrapper whose target then fails (for example
  a missing required argument under strict mode) produces one observation and
  the baseline error. An observer error propagates with its kind and the target
  is not invoked.
- INV-5, substitute exclusivity: in substitute mode the original macro of a
  substituted name is never invoked through the public name.
- INV-6, non-reentrancy: the wrapper body refers only to its private target
  alias, its parameters, and the observer global; the generator never emits the
  public name.
- INV-7, passthrough fidelity: in spy mode, for every template over the macro
  set, the substituted environment yields the same output or the same error
  kind as the baseline, including both `caller` styles, nested calls in either
  registration order, self-recursion, and repeated renders.
- INV-8, capture validity: a template-reference capture stays valid for the
  environment's lifetime because installation never removes a template. A
  captured global value stays invocable from later states for calls without a
  call block; a target that invokes a call block passed through it fails with
  `InvalidOperation` (MJ-3).
- INV-9, atomic installation: installation either succeeds with templates,
  prelude, and globals all updated, or fails with the environment observably
  unchanged; budget charges follow D10.
- INV-10, isolation: installation in one environment does not affect another
  environment or a clone taken before installation (MJ-5). A clone taken after
  installation shares the observer, which is pinned for 7.3.3.
- INV-11, budget preservation: every path still charges the shared
  `ManifestBudget`; exhaustion surfaces as the same typed budget error and
  stage class as before; ordinary builds charge exactly what they did (follows
  from INV-3).
- INV-12, path agreement: for calls without a call block, the template path,
  the expression path, and in-macro calls produce equal output and equal
  observation records.
- INV-13, telemetry (ADR-0009): in spy mode a template-path call emits no
  `netsuke_manifest_macro_invocations_total` increment; an expression-path call
  emits exactly one; render counters and labels match the baseline; the
  observer adds no labels.

### Methods per obligation

Paths below are under `src/manifest/tests/macros/substitution/` unless stated
otherwise. Every fixture macro begins with `{% do probe('<id>') %}`, where
`probe` is a counting test global, so tests can count target and original
invocations. Each item names its method, evidence, discharge condition, and
non-vacuity check.

- A1 (RM-1, RM-3, MJ-1 to MJ-8, INV-8), named `rstest` tests in
  `characterization.rs`. One test per axiom: a replaced global resolves to the
  new callable while a clone taken from `globals()` still returns the old
  sentinel (macro-specific complement to `registration.rs:140`); a prelude
  import shadows the replaced global for a direct template call but not on the
  expression path; the last of two imports of one name wins; inserting into a
  clone does not leak; an undefined argument triggers the default, an observer
  receiving `[undefined]` succeeds and a `String`-typed function argument
  rejects undefined; `TooManyArguments`, unknown keyword, `caller` rejected
  when unreferenced, and the exact `arguments` and `caller` attribute values
  for both `caller` styles; and build-time capture (self-reference, earlier
  import in the same state with a working call block, later import through the
  global with a failing call block). A captured macro global is invoked after
  replacement from a later render with positional and keyword arguments, and a
  target that calls `caller()` through it fails with `InvalidOperation`. An
  `en_localizer` test pins the full error chain of a macro compile failure
  (`MANIFEST_MACRO_COMPILE_FAILED`, `MANIFEST_MACRO_REGISTER_FAILED`) before
  EP-M2 touches it, and an inline snapshot pins the prelude text of a
  three-macro manifest. Discharge: all pass. Non-vacuity: each test asserts
  both sentinels or both outcomes, so a broken prelude or a broken capture
  flips one of them. These characterize existing behaviour and may pass on
  first run; record that honestly.
- A2 (RM-2, INV-3), a `proptest` in `substitution_property_tests/mod.rs`:
  generate zero to eight macro definitions with names from
  `{m, mm, m_1, compile_cmd}` (duplicates allowed), register them through
  `register_manifest_macros_with_budget`, and assert that the installer's
  rebuilt records render to the live global, and that zero definitions leave
  the global absent (so `has_macro_imports` telemetry is unchanged). The A1
  prelude snapshot pins the format itself across EP-M2. Seeded faults: an
  off-by-one record index in the rebuild, and skipping duplicate names; each
  must fail.
- A3 (RM-2, INV-1, INV-2, L-1), in `kernel.rs` and
  `substitution_property_tests/mod.rs`. A bounded-exhaustive `#[test]`
  enumerates every prelude over `{m, mm, m_1}` of length at most four (121
  preludes) against every selection drawn from `{m, mm, m_1, unbound}` (16
  selections), 1,936 cases, and checks INV-1 and INV-2 at every prefix against
  an independent oracle that recomputes `bind` by reverse scan. A `proptest`
  extends this to preludes of up to twelve records using
  `proptest::sample::subsequence` for selections, whose uniform size gives
  empty and full selections without filtering. A second property builds a real
  environment whose macros return per-template sentinels, including macros that
  call earlier and later names, and checks template-level and in-macro outputs
  against the model (the bridge to MJ-2 and MJ-8). Non-vacuity: the exhaustive
  test covers by construction the joint conditions that expose the faults (a
  selected short name with an unselected longer one, a selected duplicated
  name, an earlier caller of a selected name). Seeded faults: prefix matching
  with `starts_with`; rebinding only the first duplicate; wrapping only the
  last record of a name; dropping unselected records; appending instead of
  rebinding in place. Each must fail a named test.
- A4 (RM-2, INV-4, INV-5, INV-6), `rstest` tables in `rebinding.rs`. One
  verdict function, `check_observed(...) -> Result<(), Mismatch>`, compares
  output, observations (with `googletest` matchers such as `elements_are!`),
  and probe counts. An `rstest` table runs it over the real mechanism, NC1
  (global replacement alone, direct template calls only), and NC2 (bare
  stand-in import) in substitute and spy modes, with the expected verdict per
  row: the mechanism passes; NC1 fails on zero observations with original
  output; NC2 fails on zero observations with stand-in output. Substitute mode
  asserts original probe zero and stand-in probe one; spy mode asserts original
  probe one. Further cases: an observer that returns an error (propagated, no
  target probe); nested calls record outer before inner. NC3 generates a
  wrapper that calls its public name; the observer's ceiling of one dispatch
  per top-level call stops the recursion at depth two with a typed error, and a
  positive test runs the real generator for public names `caller`,
  `__netsuke_double_target`-like names, and parameters named like the observer
  under the same ceiling. Seeded faults: a wrapper that calls its target twice
  and discards one result; a substitute wrapper that also calls the original.
  The `RecordingObserver` uses `try_lock`, so a re-entrant lock becomes an
  error rather than a hang; TD-7's lock release before delegation holds by
  construction, since observation completes before the target call.
- A5 (INV-4, INV-7, D6), `rstest` cases in `call_shapes.rs` for both modes.
  Spy cases compare with the baseline environment; substitute cases assert the
  stand-in's arity. Shapes: zero parameters; positional, keyword, and mixed
  arguments; defaults omitted and overridden; too many positional arguments;
  unknown keyword; duplicate argument; missing required argument under strict
  mode (one observation, then the baseline error); a call block against a
  target with `caller` referenced and with `caller` declared, with and without
  `caller()` arguments; a call block against a target that accepts no `caller`;
  nested calls to a callee registered earlier and later, each with a plain call
  and a call block, each entered from a template and from an expression;
  self-recursion (only the outermost call observed); and repeated renders.
  Substitute-only cases: a stand-in with fewer parameters, different keyword
  names, and a stand-in referencing the public name (rejected at installation).
  Every case states its expected observation and probe counts, so equal output
  alone cannot pass a bypassed wrapper.
- A6 (INV-7, INV-12), a `proptest` in
  `substitution_property_tests/equivalence.rs` over generated signatures (zero
  to three parameters, optional defaults, either `caller` style) and call
  shapes (a positional prefix, a keyword subset, omitted arguments, sometimes
  one extra positional or unknown keyword) with values drawn from identifiers,
  integers, lists, and `none`. The independent oracle is the baseline
  environment without substitution. For each case, assert equal output or equal
  `ErrorKind` on the template path and the expression path, the expected
  observation count, and equal observation records across paths. Seeded faults:
  a wrapper that always forwards `caller`; a wrapper that drops its last
  parameter.
- A7 (RM-3, INV-8, INV-9, INV-10, D10, D11), `rstest` tests in
  `installation.rs`. A spy captured before rebinding passes calls, including
  call blocks, to the original of its own record after the capture scope ends
  and across three renders, including a duplicated name whose earlier record is
  reached from an earlier-defined caller. Installation of two doubles whose
  second stand-in fails to compile returns `MacroDoubleError::StandInCompile`
  naming the double, and the environment's prelude text, global names, and
  template lookups are unchanged, with the remaining budget equal to the
  documented consumption. Each D11 rejection returns its variant. Installing
  into one clone leaves a sibling clone's output and observations unchanged; a
  clone taken after installation shares the observer. Seeded fault: mutating
  the input environment before the failing compile must fail the
  unchanged-environment assertion.
- A8 (INV-11), budget tests in `installation.rs`: fuel exhaustion inside a
  substituted macro on the template path (render stage) and the expression path
  (macro stage); the per-value output cap; and source bytes, succeeding at a
  limit equal to the rewritten prelude plus the template body and failing one
  byte below. A chain of wrapped macros reaches the recursion limit with the
  same error kind as the unwrapped chain. Each test asserts the typed failure
  and its stage, not just an error.
- A9 (INV-13), tests in `telemetry.rs` using the `DebuggingRecorder` pattern
  from `src/manifest/tests/macro_invocation_telemetry.rs`: a spied
  template-path call emits zero invocation increments, an expression-path call
  exactly one (two would mean the wrapper reached its target through a global),
  and render counters and labels equal the baseline.
- A10 (RM-2, AGENTS.md boundary rule), one test in `pipeline.rs` that installs
  doubles and then runs `expand_foreach_with_budget` and
  `render_manifest_with_budget` over a manifest exercising a string `foreach`,
  a compiled `when`, a template-fallback `when`, and rendered fields, asserting
  rendered output and the observation sequence.
- A11 (RM-4): if EP-M3 discharges A1 to A10, record in this plan and in the ADR
  that filtered registration was not needed, citing the passing test names.
  Never claim that an unbuilt fallback passed. Otherwise EP-M4 builds the
  fallback and reruns A3 to A10 against it.

Snapshot tests with `insta` use inline snapshots (no external files) for
stable, reviewer-relevant multivariant output: the generated wrapper source for
five shapes (no parameters, parameters, `caller` referenced, `caller` declared,
and a stand-in whose name differs from the public name), and the rendered
prelude after a mixed rebinding with a duplicated name. Each is paired with the
semantic assertions above. The generator is deterministic, so no normalization
is needed.

### Formal verification decision

Verus and Kani are not selected (D8). INV-1, INV-2, and L-1 concern a pure,
pointwise list transformation. Bounded-exhaustive enumeration covers every
prelude up to four records over a three-name alphabet, which is the
completeness a Kani harness would give at the same bound, and property tests
with an independent oracle and seeded faults cover larger inputs. Kani could
not reach the kernel in any case: the Kani lane builds without `cfg(test)`
(`KANI_FLAGS` is empty and there is no `--tests`, `Makefile:53`, `:836-837`),
and the kernel is test-only. The production helpers (`import_statement`,
`macro_template_name`) are string formatting whose String-heavy harnesses have
exceeded the repository's Kani memory cap before. A Verus proof of `rebind`
would restate its definition. INV-4 to INV-13 depend on MiniJinja's semantics
(MJ-1 to MJ-8); a formal model would have to assume exactly the facts under
test, so real-engine contract tests are the appropriate rigour. The residual
gap is that the axioms are pinned by examples rather than proved over the
engine, which is accepted under the rule that third-party internals are not
verified. No `assume`, `admit`, or trusted proof body is introduced, there is
no Verus milestone, and `make verus` is not an acceptance command. If 7.3.3
adds composition policy over doubles (spy-of-substitute, ordering, or merging
of overlays), revisit this decision before implementing it.

### Suggested BDD features (handoff, non-normative)

The spike adds no user-visible behaviour, CLI surface, or loader contract, and
its installer is `cfg(test)`, unreachable from the integration harness in
`tests/`. So no `rstest-bdd` scenario or `assert_cmd` end-to-end test is added,
and `docs/users-guide.md` is unchanged. The scenarios below record the
behaviour this spike makes possible, as input to 7.3.3. They are not
implemented here, and their journal wording must be revised once 7.3.1 settles
the journal shape (D4 records omitted arguments as undefined, which 7.3.3 may
map to a shorter list):

```gherkin
Feature: Macro substitution under netsuke test
  Scenario: A substituted macro renders the stand-in and journals the call
    Given a manifest whose target command calls compile_cmd('main.c')
    And a test that substitutes compile_cmd with a stand-in returning "stub"
    When the test case runs
    Then the rendered command is "stub"
    And the journal records one call to compile_cmd

  Scenario: A spied macro passes through, including call blocks
    Given a manifest macro wrap that renders caller() between brackets
    And a test that spies on wrap
    When a target renders a call block through wrap
    Then the rendered text equals the unspied build
    And the journal records one call to wrap

  Scenario: An unrelated macro with a similar name is untouched
    Given manifest macros m and mm
    And a test that substitutes m
    When a target renders both
    Then mm renders its original output and the journal records nothing for mm
```

## Plan of work

### EP-M1: characterize the engine boundary

Assigned: RM-1, RM-3 (value-handle half), A1. Compatibility layers: none.
Re-read this plan, AGENTS.md, and the sources named in
`Context and orientation`. Sweep for existing helpers with CodeGraph and Leta
before writing new ones (AGENTS.md abstraction policy). Create
`src/manifest/tests/macros/substitution/mod.rs` with a module comment, shared
`rstest` fixtures that return `anyhow::Result` (an environment builder reusing
`register_manifest_macros_with_budget`, the counting `probe` global, and
sentinel templates), and `characterization.rs` with the A1 tests. Wire the
module from `src/manifest/tests/macros/mod.rs`.

End state: a passing characterization suite that explains, from evidence, why
global replacement fails on the template path, why value-handle passthrough
cannot carry call blocks, and how macro-to-macro calls bind. Acceptance: the
gate set passes and every A1 test appears in `cargo nextest list`. Conformance
check: no production change; every axiom matches observed behaviour. If an
axiom is falsified, stop under the axiom tolerance. Recovery: revert the
commit. Remaining gaps: RM-2 and RM-4.

### EP-M2: extract the import and compile helpers

Assigned: RM-2 (format sharing), A1's prelude snapshot and error-chain test as
the guard for INV-3. Compatibility layers: none; every caller changes in the
same commit. Refactor under the green A1 guard: create
`src/manifest/jinja_macros/imports.rs` holding `macro_template_name`,
`import_statement`, `register_macro_import`, and `macro_imports`, with
production registration using the helpers. Extract
`compile_macro_template(env, request, budget)` with a
`MacroTemplateRequest { template_name, source, macro_name }` struct, borrowing
the budget, and have `register_macro_with_budget` call it. Widen
`capture_macro` to `pub(super)`. Show that the guard is live by applying a
seeded fault to `import_statement` (one changed character) as an uncommitted
patch: the A1 prelude snapshot must fail; then reverse it. Every existing
macro, budget, telemetry, and overlay test must pass unchanged.

Run `make test-workflow-contracts`: the Kani proof-scope contract will name
`src/manifest/jinja_macros/imports.rs`, because `jinja_macros/mod.rs` is in
scope; add exactly that line to `tools/kani/proof-scope.toml` in alphabetical
position. The `cfg(test)` substitution module is skipped by the contract.

End state: ordinary behaviour byte-identical and guarded, with the shared
format helpers ready for the installer. Acceptance: A1 still passes, the fault
demonstration is recorded, and the gate set passes. Conformance check: only the
three authorized production refactors; no public change. Recovery: revert the
commit; EP-M1 stands alone. Remaining gaps: the mechanism (EP-M3) and RM-4.

### EP-M3: prove per-record prelude rebinding with journalling wrappers

Assigned: RM-2, RM-3 (template-reference half), A2 to A10. Compatibility
layers: none. This is a prototyping milestone: its code is `cfg(test)` and will
be promoted by 7.3.3 or deleted.

Red: write A4's mechanism rows against a stub `install_macro_doubles` that
performs only `add_function` replacement (the NC1 behaviour). Run them and
record the expected failure: original output and zero observations on the
template path.

Green: implement `src/manifest/jinja_macros/substitution/` under
`#[cfg(test)]`, declared from `jinja_macros/mod.rs` and re-exported to the test
modules with `#[cfg(test)] pub(crate) use`, as `call_macro_value` already is
(`mod.rs:28-29`). The pieces are:

- `kernel.rs`: `ImportRecord { template, name }`, `rebuild_records` (from the
  manifest's `MacroDefinition` list through `macro_template_name`), `rebind`,
  and `render_prelude` (through `import_statement`). This is the pure policy
  half: plain data, no engine types beyond strings.
- `wrapper.rs`:
  `CapturedMacro { template, macro_name, arguments, caller_style }` captured
  through `capture_macro`, where `caller_style` is `Referenced`, `Declared`, or
  `None`; and `wrapper_source(public_name, target)`, which generates (shown for
  a target with parameters `a1` to `ak` and a referenced `caller`):

  ```jinja
  {% from '<target template>' import <target macro> as __netsuke_double_target %}
  {% macro <public name>(<a1>, ..., <ak>) %}
  {%- do __netsuke_double_observe('<public name>', [<a1>, ..., <ak>], caller is defined) -%}
  {{ __netsuke_double_target(<a1>, ..., <ak>, caller=caller) }}
  {%- endmacro %}
  ```

  When the target does not accept `caller`, the flag is `false` and
  `caller=caller` is omitted, so the wrapper rejects call blocks exactly as the
  target does. When `caller` is a declared parameter, it stays in the parameter
  list and forwarding list but the generator passes it to the observer only as
  the flag, never in the argument list. Whitespace control keeps the wrapper's
  output equal to the target's.
- `mod.rs`: `MacroDouble { public_name, mode }` with
  `DoubleMode::{Substitute { stand_in: MacroDefinition }, Spy}`;
  `MacroObservation { name, args, has_caller }`; `MacroObserver`, an
  `Arc<dyn Fn(&MacroObservation) -> Result<(), minijinja::Error> + Send + Sync>`
  mirroring `OverlayCallable` and forming the seam 7.3.1's journal will use;
  `MacroDoubleError` (a `thiserror` enum with variants for stand-in compile
  failure, missing spy target, duplicate double, reserved-name collision, a
  stand-in referencing the public name, and prelude drift); and
  `install_macro_doubles(env, &DoubleInstallation) -> Result<(), MacroDoubleError>`
  where `DoubleInstallation { manifest_macros, doubles, observer, budget }`
  borrows the loader's `ManifestBudget`. It works on `env.clone()`: validates
  names (D11), rebuilds and checks the records (A2), compiles each stand-in
  through `compile_macro_template` under `__netsuke_double_{k}_{name}` and
  checks its undeclared names (D6), captures each selected record's target,
  compiles one wrapper per record under `__netsuke_double_wrapper_{i}_{name}`,
  writes the rebound prelude, sets each selected global to `make_macro_fn` over
  its last record's wrapper, registers the observer global, and assigns the
  candidate to `*env` only on success.

Test-side `RecordingObserver` (in the test `mod.rs`) appends observations under
a `try_lock`, replaces macro values with a `<caller>` marker, can enforce a
per-call ceiling (NC3), and maps a poisoned or contended lock to a
`minijinja::Error`. Then add A2 to A10 and the snapshots, keeping each file
under 400 lines, and remove any abandoned prototype code before committing.

Verify property selection before and after with a listing count (see
`Concrete steps`); the count must rise by the number of new properties.

End state: substitution and spying work on all three routes with every
acceptance item demonstrated, or a recorded failure that triggers EP-M4.
Acceptance: A2 to A10 pass; the seeded faults have been run and recorded; the
gate set passes. Conformance check: public API unchanged; ordinary builds
unchanged (existing suites and A1 snapshot); mechanism matches D1 to D6, D10,
and D11. Recovery: EP-M2 is a coherent plateau; revert EP-M3 commits if needed.
Remaining gaps: runner wiring, journal format, and the ADR-0070 amendment, all
owned by 7.3.1 and 7.3.3.

### EP-M4: decide the fallback

Assigned: RM-4, A11. Compatibility layers: none. Shadowing fails if any A3 to
A10 item cannot pass because a post-registration edit of the environment is
unsound, for example a stale binding captured at registration that rebinding
cannot reach, a target that cannot be resolved after installation, or a budget
or telemetry boundary that a post-hoc edit cannot preserve. If none fails,
record under `Decision log` and `Outcomes` that filtered registration was
unnecessary, naming the passing tests, and proceed to EP-M5.

If shadowing fails, implement filtered registration in
`jinja_macros/substitution/` under `#[cfg(test)]`: a test-only sibling of
`register_manifest_macros_with_budget` that takes the names to withhold,
compiles each withheld original under its normal template name, and appends the
wrapper record and global in the original's position instead of the original's.
Rerun A3 to A10 against it, delete the failed rebinding path, and record both
outcomes. Changing the production signature of
`register_manifest_macros_with_budget` is outside this plan's tolerances.

End state: an evidenced decision for RM-4. Acceptance: the decision entry cites
passing tests; if the fallback was built, A3 to A10 pass against it and the
gate set passes. Conformance check: as for EP-M3. Recovery: as for EP-M3.
Remaining gaps: none for RM-4.

### EP-M5: documentation, ADR, and roadmap

Assigned: DEV-1 to DEV-5 and the documentation duties in AGENTS.md.
Compatibility layers: none. Use the `scribe` agent for mechanical edits, with
the facts supplied from this plan; otherwise edit directly.

- Technical design §5.4: replace the bare-stand-in import with per-record
  wrapper rebinding, the three binding routes, the call-block constraint, the
  substitute-mode signature rule (D6), the spike's observation shape (D4), and
  the test evidence. §7: macro spies capture by template reference. §4.3: the
  `macro_substitutions` reassignment (line 274), the C9 spike exemption (line
  ~318), and the need for a substitution overlay kind distinct from
  `with_function`. §14: replace the claim at lines 1144-1149 with this spike's
  evidence and the fallback outcome.
- A new ADR at
  `docs/adrs/adr-NNNN-macro-substitution-rebinds-imports-to-journalling-wrappers.md`.
  Allocate `NNNN` only after sweeping every remote branch for the highest
  minted number, because open branches hold numbers above `main`'s highest
  (0074 at baseline) and a published number is never reused:

  ```sh
  git fetch --all --prune --quiet
  git for-each-ref --format='%(refname)' refs/remotes |
    while read -r ref; do
      git ls-tree -r --name-only "$ref" -- docs/adrs/ 2>/dev/null
    done | grep -oE 'adr-[0-9]{4}' | sort -u | tail -n 1
  ```

  The ADR follows the style guide: status `Accepted` with the date and a
  one-sentence summary (given approval of DEV-1 and DEV-2), a `## Date`
  section, the context (MJ-1 to MJ-3, MJ-8), the decision (D1 to D3, D5, D6),
  rejected alternatives (bare stand-in import, global fallthrough, value-handle
  capture, typed table, string parsing), and consequences: ADR-0070 needs an
  amendment in 7.3.3; the UX journal-namespace question is open for 7.3.1.
  Reference it from technical design §5.4.
- `docs/developers-guide.md`, under "Template rendering and macro
  registration" (around line 12175): the shared format helpers and their reuse
  policy (they are the only producers of prelude text and template names;
  callers are limited to `jinja_macros`), `compile_macro_template`, the
  ownership and permitted call sites of the `cfg(test)` installer (manifest
  macro tests only, pending 7.3.3), the order-dependence of macro-to-macro
  calls, and the focused commands.
- `docs/polonius.md`: no entry is expected, since the spike adds no
  borrow-returning production accessor; add one under "Evolutions that compile
  under both checkers" if implementation introduces one.
- `docs/contents.md`: index the ADR. `docs/repository-layout.md` indexes
  `src/manifest/` at directory level, so it changes only if that listing would
  become wrong.
- `docs/roadmap.md`: add 7.3.3 sub-bullets for the handoff in D7, then mark
  7.1.3 and its four sub-bullets `[x]` only after A1 to A11 are discharged,
  using a minimal checkbox edit. Leave 7.1.4 and 7.3.3 unchecked.
- Raise two GitHub issues: the existing order-dependence of call blocks between
  manifest macros (a user-visible quirk this spike characterizes), and the
  inaccurate ADR-0009 statement about imports and `make_macro_fn`.
- `docs/users-guide.md`: no change, because there is no user-visible
  behaviour; confirm during the final diff review.

End state: documents match the evidence; this plan's status is `COMPLETE`.
Acceptance: the gate set passes; `Progress` and `Outcomes` describe the actual
evidence; no deviation remains unrecorded. Conformance check: every DEV item is
reflected in the documents named above. Recovery: revert the documentation
commit; the code milestones stand. Remaining gaps: those handed to 7.3.1 and
7.3.3.

At every milestone boundary, confirm that the assigned requirements are met,
that the design still matches the decisions, that no axiom was falsified, that
no public interface, dependency, or persistent format was added, and that the
trace links are current. Any unapproved deviation is recorded under
`Decision log`, `Status` is set to `BLOCKED`, and work waits.

## Milestones and plateaus

- EP-M1 plateau: characterization tests only; the repository is unchanged in
  behaviour and the axioms are evidenced.
- EP-M2 plateau: shared format and compile helpers in production; behaviour
  byte-identical; guard demonstrated live.
- EP-M3 plateau: a working test-only substitution mechanism with full evidence,
  or a recorded failure.
- EP-M4 plateau: RM-4 decided with evidence.
- EP-M5 plateau: documentation, ADR, issues, and roadmap reconciled; plan
  complete.

Each plateau is safe to stop at: later milestones add evidence or documentation
but never require reverting an earlier one.

## Concrete steps

Run every command from the repository root on the task branch. Gate runs are
delegated to the `scrutineer` agent, which runs them one at a time; without
that agent, run the same commands directly. Capture each command's output with
`set -o pipefail` and `tee`, using the log name
`/tmp/<target>-netsuke-7-1-3-spike-mini-jinja-overlay-shadowing.out`, where
`<target>` is the make target name.

During red-green work in any milestone:

```sh
set -o pipefail
make test-nextest 2>&1 | tee /tmp/test-nextest-netsuke-7-1-3-spike-mini-jinja-overlay-shadowing.out
make proptest 2>&1 | tee /tmp/proptest-netsuke-7-1-3-spike-mini-jinja-overlay-shadowing.out
```

`make test-nextest` runs the full non-doctest suite; it is not a filtered run,
so do not describe it as one. Record test names, counts, exit status, and each
expected red failure under `Artefacts`.

Before and after EP-M3, confirm property selection by count, using the filter
from `Makefile:325`:

```sh
cargo nextest list --workspace --all-targets --all-features \
  -E 'test(/(^|::)[a-z0-9_]*proptests($|::)/) | test(/(^|::)[a-z0-9_]*property_tests($|::)/)' |
  tee /tmp/proptest-list-netsuke-7-1-3-spike-mini-jinja-overlay-shadowing.out | grep -c '::'
grep substitution_property_tests /tmp/proptest-list-netsuke-7-1-3-spike-mini-jinja-overlay-shadowing.out
```

Expected evidence after EP-M3 resembles the following; copy the real lines from
the listing rather than relying on this form:

```plaintext
netsuke-build manifest::tests::macros::substitution::rebinding::mechanism_observes_template_path_call
netsuke-build manifest::tests::macros::substitution::substitution_property_tests::rebinding_matches_binding_oracle
```

Seeded faults (EP-M2 and EP-M3) are applied without committing:

```sh
git apply /path/to/fault.patch
make test-nextest 2>&1 | tee /tmp/fault-netsuke-7-1-3-spike-mini-jinja-overlay-shadowing.out
git apply -R /path/to/fault.patch
git status --short   # must show no change from the fault
```

Keep fault patches outside the worktree (for example in a scratch directory
under `/tmp`) and record their text under `Artefacts`. Delete any proptest
regression entries the fault run wrote.

After every milestone, run this gate set sequentially on the same candidate,
each with `tee` capture as above. Every command must exit 0:

```sh
make fmt
make check-fmt
make typecheck
make lint
make doc-coverage
make test
make test-workflow-contracts
make markdownlint
make nixie
```

`make test` runs `tests/execplan_status_contract_tests.rs`, which reads every
`docs/execplans/` file. `make test-workflow-contracts` runs
`tests/workflow_contracts/kani_proof_scope_test.py`, which checks
`tools/kani/proof-scope.toml` against the module tree. Repeat the affected
gates after any fix, and never commit a red candidate. Warnings remain errors,
and doc coverage stays at or above 80%. The plan-only commits that publish this
document use the same gate set.

## Validation and acceptance

The spike is accepted when all of the following hold on one candidate:

- the gate set above passes, with logs recorded;
- A1 to A10 pass, and `cargo nextest list` shows every named test;
- `make proptest` runs the new `substitution_property_tests` properties;
- every seeded fault named under A2, A3, A4, A6, and A7 failed its named test
  when applied and left no trace when reversed;
- NC1, NC2, and NC3 produce their expected verdicts;
- A11 is recorded with evidence; and
- the documents named in EP-M5 reflect DEV-1 to DEV-5 and the evidence.

Quality criteria: no new warnings, no lint suppressions, no file over 400
lines, doc coverage at or above 80%, and every assertion carries a message.

## Idempotence and recovery

Every test builds fresh environments, probes, and observers; no mutable
registry or counter is shared between cases. Installation never mutates the
input environment on failure, so re-running a test cannot accumulate imports.
Seeded faults are reversible patches and never enter history. Each milestone is
one or more gated commits, each a usable rollback point. Never reset unrelated
changes, use a bare `git stash`, or terminate another agent's builds.

## Interfaces and dependencies

Production, after EP-M2 (visibility `pub(super)` within `jinja_macros`, with a
`#[cfg(test)] pub(crate) use` re-export where the test modules need an item):

```rust
pub(super) fn macro_template_name(index: usize, name: &str) -> String;
pub(super) fn import_statement(template: &str, name: &str) -> String;
pub(super) struct MacroTemplateRequest<'a> {
    pub(super) template_name: &'a str,
    pub(super) source: String,
    pub(super) macro_name: &'a str,
}
pub(super) fn compile_macro_template(
    env: &mut Environment<'static>,
    request: MacroTemplateRequest<'_>,
    budget: &ManifestBudget,
) -> anyhow::Result<()>;
pub(super) fn capture_macro<'source>(
    env: &'source Environment<'source>,
    template_name: &str,
    macro_name: &str,
) -> Result<(Captured<'source>, Value), Error>;
```

Test-only, after EP-M3, under `#[cfg(test)]` in
`src/manifest/jinja_macros/substitution/`: `ImportRecord`, `rebuild_records`,
`rebind`, `render_prelude`, `CapturedMacro`, `CallerStyle`, `wrapper_source`,
`MacroDouble`, `DoubleMode`, `MacroObservation`, `MacroObserver`,
`DoubleInstallation`, `MacroDoubleError`, and `install_macro_doubles`. No
dependency changes are needed.

## Documentation and skill signposts

Read first: `docs/contents.md`, `docs/repository-layout.md`,
`docs/documentation-style-guide.md`, `docs/netsuke-design.md` §4.3, and the
technical and UX design sections listed in `Conformance basis`. Testing
guidance: `docs/rust-testing-with-rstest-fixtures.md`,
`docs/rstest-bdd-users-guide.md`,
`docs/reliable-testing-in-rust-via-dependency-injection.md`, and
`docs/rust-doctest-dry-guide.md` (for doc examples on the helpers).
Configuration (not changed here): `docs/ortho-config-users-guide.md`. Borrowing:
`docs/polonius.md`. Gates and property selection: `docs/developers-guide.md`
"Test execution" and "Property-based testing with proptest".

Skills: `execplans` (this plan); `rust-router`, with `rust-unit-testing` and
`proptest`; `rust-verification` if D8 is revisited; `codegraph-mcp` and `leta`
for navigation; `hexagonal-architecture` for the policy and adapter split;
`nll-to-polonius` only if a borrow error appears; `commit-message`,
`pr-creation`, and `logisphere-design-review`; and `firecrawl-mcp` for external
research. Agents: `scrutineer` runs gates; `scribe` makes mechanical
documentation edits; `wyvern` does read-only reconnaissance.

## Artefacts and notes

Record here during execution: gate log paths and outcomes, A1 to A11 test
names, red-stage failure excerpts, seeded-fault patch text and the tests each
failed, the property-listing counts, the ADR number and path, the issue links,
and the final implementation commits.

Research inputs (2026-11-15): MiniJinja 2.24.0 source in the shared Cargo
registry (paths under `Axioms`); the docs.rs `Environment`[^env-docs] and
syntax[^syntax-docs] pages, retrieved through Firecrawl; and the maintainer's
write-up on the virtual machine design.[^learnings]

## Revision note

- 2026-11-04: initial approval-gated plan against `b99c9c92b`.
- 2026-11-04, expert review: clarified both dispatch paths and the fallback's
  final bindings, and required budget boundaries and bounded negative controls.
- 2026-11-15, source research: rebased onto `5c707ef33`; replaced the design
  with a generated journalling wrapper and template-reference spy capture;
  added the abstract model, invariants, and the formal-verification decision.
- 2026-11-15, four-perspective expert review. What changed: rebinding is now
  in place and per record, because macro bodies capture other macros at build
  time (MJ-8); spy capture is per record; the typed import table is replaced by
  shared format helpers and a `cfg(test)` record rebuild, so the prelude global
  keeps its type and text; MJ-6 and MJ-7 were corrected and MJ-8 added;
  substitute mode now follows the stand-in's signature and rejects stand-ins
  that call the public name; installation hygiene, budget consumption, and the
  observer interface are specified; tests gained probes for target counts, a
  shared verdict function for the negative controls, a bounded-exhaustive
  kernel test, an equivalence property against the baseline, telemetry and
  pipeline tests, and error-propagation cases; seeded faults are uncommitted
  patches; property selection is verified by count; deviations from the
  approved design are listed for explicit approval; and the tolerances,
  interfaces, gate rationale, and template sections were corrected. Why:
  reviewers found that the earlier design broke macro-to-macro calls with
  duplicate names, that several tests could not fail when the implementation
  was wrong, and that some stated engine facts were wrong. Effect on remaining
  work: EP-M1 grows to pin all eight axioms, EP-M2 shrinks to helper
  extraction, and EP-M3 carries the larger verification load.

[^env-docs]: <https://docs.rs/minijinja/2.24.0/minijinja/struct.Environment.html>

[^syntax-docs]: <https://docs.rs/minijinja/2.24.0/minijinja/syntax/index.html>

[^learnings]: Armin Ronacher, "MiniJinja: Learnings from Building a Template
    Engine in Rust", 2024, <https://lucumr.pocoo.org/2024/8/27/minijinja/>.
