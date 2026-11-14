# Spike MiniJinja overlay shadowing for macro substitution (7.1.3)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: DRAFT

Branch: `7-1-3-spike-mini-jinja-overlay-shadowing`

Roadmap item: `7.1.3` in `docs/roadmap.md`.

The plan is revised and awaiting explicit approval. Implementation must not
begin until the user approves it. Publishing this document is not evidence that
any experiment has run, and roadmap 7.1.3 stays unchecked until EP-M5 closes.

## Purpose / big picture

Netsuke's planned `netsuke test` command (roadmap phase 7) lets a test replace
a manifest macro with a *stand-in*, or *spy* on it, while the rest of the
manifest compiles through the same pipeline as a real build. A stand-in is a
test-supplied macro that answers instead of the original. A spy records each
call and then passes it through to the original macro. Both need a *journal*: a
record of every call, which assertions later inspect.

Today a manifest such as

```yaml
macros:
  - signature: "compile_cmd(src, flags='')"
    body: "cc {{ flags }} -c {{ src }}"
targets:
  - name: main.o
    command: "{{ compile_cmd('main.c') }}"
```

compiles `compile_cmd` into an internal template and makes it reachable twice:
as an environment global (a Rust callable) and through a generated *prelude*, a
string of `{% from '<template>' import <name> %}` statements stored in the
`MACRO_IMPORTS_GLOBAL` global and prepended to every rendered template. An
import binds a template-local name, and MiniJinja resolves locals before
globals, so replacing the global alone (`Environment::add_function`) changes
nothing on the template path. Technical design §5.4 therefore says that macro
substitution must rewrite the prelude, and roadmap 7.1.3 asks for a spike that
proves how.

This spike delivers executable evidence, not the user-facing feature. After it
lands, a maintainer can run the focused suite named in `Concrete steps` and
observe:

1. The characterization tests pin the engine facts that constrain the design:
   `add_function` replaces a global, a prelude import shadows it, and a
   captured Rust handle survives replacement but cannot carry a call block
   (`{% call %}`) across MiniJinja render states.
2. A test-only installer rebinds selected prelude entries to a generated
   *journalling wrapper macro*. In substitute mode the wrapper delegates to a
   compiled stand-in; in spy mode it delegates to the captured original. Every
   call is observed exactly once on both the template path and the expression
   path. Unselected macros, call blocks, strict undefined behaviour, and
   resource budgets keep their current behaviour.
3. Two negative controls fail the observation assertions for the two
   defective designs: global replacement alone, and the literal §5.4 reading of
   importing the bare stand-in.
4. Either the rebinding mechanism passes every acceptance item and the
   filtered-registration fallback is recorded as unnecessary with that
   evidence, or the fallback is built and passes the same acceptance items.

Ordinary builds, manifest queries, the public `TemplateOverlays` API, and every
user-visible output stay unchanged. Promotion into the runner belongs to
roadmap 7.3.3, which depends on this spike.

## Constraints

- Implementation begins only after explicit user approval of this plan.
- Use the locked MiniJinja dependency (2.24.0, features `fuel` and `loader`)
  and the production macro compilation, invocation, rendering, and
  resource-budget paths. Do not emulate the engine in tests.
- Preserve the behaviour of ordinary builds and manifest queries: rendered
  output, error kinds, localized diagnostics, telemetry (ADR-0009), and budget
  accounting (ADR-0050). The prelude text that `render_template_at` prepends
  must stay byte-identical for every manifest, because its length is charged to
  the source budget.
- Do not change the public API. `TemplateOverlays`, `OverlayCallable`,
  `OverlayCall`, `TestSurface`, and `ManifestLoadOptions` stay as they are.
  `TemplateOverlays::with_function` keeps rejecting manifest-macro names
  (ADR-0070, `src/manifest/test_surface/overlays.rs:249-250`). Changing that
  rule is 7.3.3's decision; this spike records the evidence for it.
- Production changes are limited to two behaviour-neutral refactors inside
  `src/manifest/jinja_macros/`: a typed import table replacing the concatenated
  prelude string, and the extraction of the compile-and-validate step from
  `register_macro_with_budget`. The substitution installer and its observer are
  compiled only under `#[cfg(test)]`, as the existing `register_macro` helper
  is. No `#[expect(dead_code)]` is permitted.
- Separate policy from engine adaptation. The rebinding kernel (which records
  bind which names) is a pure function over plain data. Template generation,
  registration, and callable construction stay in the MiniJinja adapter. Do not
  introduce a port trait, a general-purpose abstraction, or a new directory
  architecture for this spike.
- No new configuration surface, CLI flag, or environment input is needed.
  Existing `ortho_config` layering and localized help are untouched. Any later
  configuration belongs to 7.3.x and must use `ortho_config`
  (`docs/ortho-config-users-guide.md`).
- No `unsafe`, no lifetime extension of MiniJinja `State` or `Captured` values,
  no in-process environment mutation (AGENTS.md, ADR-0008), no network or
  command execution, and no new dependencies. `rstest`, `rstest-bdd`,
  `googletest`, `pretty_assertions`, `insta`, and `proptest` are already
  authorized dev-dependencies.
- Code files stay at or below 400 lines. `src/manifest/jinja_macros/mod.rs`
  has 374 lines and `invocation.rs` has 381, so new logic goes in new files.
- Respect the Polonius posture and pinned nightly (ADR-0006,
  `docs/polonius.md`): lookups return references, keys are cloned only on
  insertion, and no `-Z` flags are added. Record any new borrow-returning
  accessor in `docs/polonius.md`.
- Never bypass compiler admission, isolate the Cargo cache, or use `/tmp` as a
  build target. Run gates sequentially, delegated to `scrutineer`.
- Do not rewrite completed ExecPlans. Do not mark 7.1.4 or 7.3.3.

## Tolerances (exception triggers)

The comparison base is the merge base with `origin/main` at the start of EP-M1.
Count with
`git diff --numstat "$(git merge-base origin/main HEAD)" -- . ':(exclude)docs/execplans/'`.
ExecPlan files are excluded from every count, including this file. Measure net
lines (additions minus deletions). Classify by path: code is everything under
`src/` (production and tests, including inline snapshots); configuration is
`tools/kani/proof-scope.toml` and any other declarative manifest; documentation
is everything under `docs/` except `docs/execplans/`.

Survey and history. The previous revision of this plan allowed 12 files and 800
net lines combined. Comparable plans show repeated under-estimation: 7.1.2 set
42 files and 2,400 lines and finished at 81 files and 6,603 lines (accepted as
an exception); 4.4.1 was raised from 14 files and 700 lines to 18 and 1,200;
7.1.1's scope tolerance fired and was accepted after completion. The causes
were whole-directory counting, test volume, and documentation fan-out, not
approved scope growth. The estimate below therefore enumerates files: two new
production files (~150 and ~300 lines including the `cfg(test)` installer), two
edited production files, seven new test files of 120 to 320 lines each, and one
edited test module. That totals about 1,750 lines of code. Documentation covers
the technical design, a new ADR, the developers' guide, `docs/polonius.md`,
`docs/contents.md`, `docs/repository-layout.md`, and the roadmap, about 380
lines.

- Files: stop if more than 13 code files, 2 configuration files, or 9
  documentation files change.
- Code: stop above 2,100 net lines (about 20% contingency).
- Configuration: stop above 10 net lines.
- Documentation: stop above 500 net lines.
- Interface: stop if any public item changes, or if a production change goes
  beyond the two refactors named in `Constraints`.
- Dependencies: stop if any dependency or feature flag would be added.
- Architecture: stop if the evidence requires changing ADR-0070's name rule,
  the loader registration order (technical design §4.3), or ordinary-build
  semantics within this spike.
- Semantics: stop if both the rebinding mechanism and the filtered fallback
  fail any acceptance item A1 to A9. Do not narrow the acceptance criteria.
- Iterations: stop if a focused test still fails after five fix attempts on the
  same root cause.
- Ambiguity: stop and present options if the journal argument shape needs a
  user-facing decision beyond the spike's recorded, reversible choice (D4).

There is no wall-clock limit. A healthy, queued compiler-admission pool is not
a failure; record infrastructure faults separately from semantic failures.

## Risks

- Risk: the generated wrapper changes macro semantics, such as default
  evaluation, strict undefined handling, keyword validation, or `caller`
  acceptance. Severity: high. Likelihood: medium. Mitigation: build the wrapper
  from the engine's own `arguments` and `caller` attributes, not by parsing
  signatures. Assert observational equivalence against the unsubstituted
  environment for each call shape (A5, A6).
- Risk: a test passes on output text while the wrapper is bypassed. Severity:
  high. Likelihood: high, since the literal §5.4 design does exactly this.
  Mitigation: every acceptance item asserts observation counts and order with
  the output. NC1 and NC2 prove that the assertions reject the bypass.
- Risk: spy passthrough through the captured Rust global loses call blocks,
  because MiniJinja rejects calling a macro from a different render state.
  Severity: high. Likelihood: certain per source reading (MJ-3). Mitigation:
  pin the failure as a characterization test. Spy passthrough on the template
  path uses a template reference, not the global value (D3).
- Risk: rebinding perturbs unselected or similarly named macros, or duplicate
  definitions of one name. Severity: medium. Likelihood: medium. Mitigation:
  rebind exact names in a typed table rather than editing strings, and run
  property tests with prefix-similar names, duplicates, and empty, partial, and
  full selections (A3).
- Risk: the wrapper's extra import and call frame changes budget accounting,
  for example fuel per render, recursion depth, or source bytes. Severity:
  medium. Likelihood: medium. Mitigation: keep the prelude byte-identical for
  ordinary builds. For substituted environments, assert typed exhaustion at
  each budget boundary and exact source-byte limits computed from the rewritten
  prelude (A8).
- Risk: the typed import table is not a faithful drop-in for the string
  prelude. Severity: medium. Likelihood: low. Mitigation: a property test
  against the legacy concatenation, kept as a test oracle, proves byte-identity
  before any substitution code lands (A2).
- Risk: a new `src/manifest/jinja_macros/` module trips the Kani proof-scope
  contract (`tests/workflow_contracts/kani_proof_scope_test.py`). Severity:
  low. Likelihood: high. Mitigation: run `make test-workflow-contracts`. Add
  exactly the paths its assertion names to `tools/kani/proof-scope.toml`.
- Risk: the design documents disagree, and the spike silently picks a side.
  Severity: medium. Likelihood: certain. §5.4 imports the bare stand-in while
  §5.4's first paragraph requires a journalling wrapper. §14 claims 7.1.2
  settled macro capture. §4.3 and ADR-0070 forbid macro-named overlays.
  Mitigation: resolve each point explicitly in EP-M5 documentation and in a new
  ADR. Leave the ADR-0070 amendment to 7.3.3 (D6).

## Progress

- [x] (2026-11-04) First draft: two Wyvern passes, six-perspective review,
  draft PR #1088 opened against baseline `b99c9c92b`.
- [x] (2026-11-15) Adopted the existing planning branch, rebased it onto
  `main` at `5c707ef33`, and confirmed that the macro, overlay, and loader
  sources are unchanged since the first draft.
- [x] (2026-11-15) Three Wyvern reconnaissance passes covered design
  documents, verification and gate infrastructure, and macro and overlay code
  paths. MiniJinja 2.24.0 source reading established the engine axioms
  MJ-1–MJ-7, and Firecrawl checked the upstream documentation and history.
- [x] (2026-11-15) Rewrote the plan around the generated journalling wrapper,
  the template-reference capture, and the proof-first verification model.
- [ ] Community-of-experts review and revision.
- [ ] Gate, commit, push, and refresh draft PR #1088.
- [ ] Obtain explicit implementation approval.
- [ ] EP-M1: characterize the engine boundary.
- [ ] EP-M2: introduce the typed import table and the compile extraction.
- [ ] EP-M3: prove prelude rebinding with journalling wrappers.
- [ ] EP-M4: decide the fallback: record it as unnecessary, or build it.
- [ ] EP-M5: record the design, write the ADR, and mark 7.1.3 done.

## Surprises & discoveries

- Observation: `when` expressions and string `foreach` expressions never see
  the prelude. Evidence: `evaluate_with_state`
  (`src/manifest/jinja_macros/expression.rs:24-57`) renders
  `{% set __netsuke_expression_result = (...) %}` without prepending imports.
  Impact: macro calls in those expressions resolve through the global, so
  `add_function` replacement alone already reaches them. A substitution must
  cover both paths with one observation mechanism.
- Observation: a MiniJinja macro value records the identity of the render state
  that built it, and `Macro::call` refuses a call from any other state. Imports
  and nested macro evaluation keep the importing state's identity. Evidence:
  `minijinja-2.24.0/src/vm/macro_object.rs:47-53`, `vm/mod.rs:118-150`
  (`eval_macro` copies `state.id`), `vm/mod.rs:831-900` (`perform_include`
  evaluates in place). The behaviour is upstream design, quoted in the
  maintainer's 2024 write-up. Impact: a `caller` block created in the render
  state cannot be invoked inside the fresh state that `make_macro_fn` builds.
  Spies that pass through a captured Rust handle therefore cannot support call
  blocks on the template path.
- Observation: `make_macro_fn` captures a template name, a macro name, and a
  budget, not a macro value (`invocation.rs:32-55`). Impact: the captured
  global stays callable from any later state for as long as its template stays
  registered. Capture by template reference is portable; capture by macro value
  is not.
- Observation: MiniJinja applies a macro default only when the argument is
  undefined (`compiler/codegen.rs:489-499`). Under strict undefined behaviour,
  Rust function arguments of type `Value` or `&Value` reject undefined values
  (`value/argtypes.rs:609-698`), but macro arguments do not. Impact: a wrapper
  with no defaults can forward omitted arguments as undefined and keep the
  target's default semantics. Its observer must receive the arguments as one
  list value, not as separate Rust arguments.
- Observation: the docs.rs syntax summary, extracted through Firecrawl, claims
  that macros accept unlimited positional and keyword arguments. The 2.24.0
  source contradicts it: `Macro::call` returns `TooManyArguments` and rejects
  unknown keywords. Impact: treat the source as authoritative. A wrapper must
  reproduce the target's exact arity.
- Observation: technical design §5.4 contradicts itself. It requires a
  journalling wrapper, then specifies an import bound to the bare stand-in,
  which bypasses the wrapper on the template path. §14 says 7.1.2 settled
  replacement and capture, but 7.1.2 tested functions only
  (`src/manifest/tests/overlays/registration.rs`). Impact: EP-M5 corrects both
  sections.
- Observation: `make proptest` selects only modules whose path segment ends in
  `proptests` or `property_tests` (`Makefile:325`). None of the existing
  property modules under `src/manifest` match. Impact: name the new suite
  `substitution_property_tests` so the focused target selects it.

## Decision log

- D1 (2026-11-15). Decision: rebind prelude entries to a generated journalling
  wrapper macro rather than to the bare stand-in. Rationale: an import of the
  bare stand-in renders substituted text and journals nothing on the template
  path, which would violate §5.4's own journalling requirement and UX §8.5. The
  strongest alternative, dropping the import so the name falls through to a
  Rust global wrapper, loses call blocks (MJ-3). Reversible: yes. The mechanism
  remains internal and `cfg(test)` until 7.3.3.
- D2 (2026-11-15). Decision: use one wrapper for both entry paths. The template
  path imports the wrapper macro; the expression path replaces the global with
  `make_macro_fn` over the wrapper template. Rationale: a single observation
  site gives identical journal records on both paths. A separate Rust observer
  on the expression path would need its own argument normalization. Expression
  calls already lack call-block support, so the change costs nothing there.
- D3 (2026-11-15). Decision: a macro spy captures a *template reference*, the
  last import binding of the public name, with the macro's `arguments` and
  `caller` attributes. It does not capture the global value. Rationale: MJ-3
  makes a value handle unable to carry call blocks across states. A template
  reference resolves inside the active state. The global-value capture is still
  pinned as working for non-caller calls, because technical design §7 relies on
  it for functions.
- D4 (2026-11-15). Decision: the spike's observer records the wrapper's bound
  parameters in declaration order, with omitted arguments as undefined, and a
  flag for the presence of a `caller` block. Rationale: this preserves the
  distinction between omitted and passed arguments and lets the target apply
  its own defaults. Mapping to the UX `args` list shape (UX §8.2) belongs to
  7.3.1 and 7.3.3. Reversible: yes. The observer is test-only.
- D5 (2026-11-15). Decision: replace the concatenated prelude string with a
  typed, ordered import table stored as a MiniJinja object global, and render
  the identical text from it. Rationale: rebinding by name over typed records
  removes string parsing and makes the kernel a pure function suitable for
  property testing. The new file also keeps `mod.rs` under 400 lines.
  Alternative: parse the generated string with a strict inverse of its own
  format. That is rejected because it adds a parser whose only job is to undo a
  representation choice.
- D6 (2026-11-15). Decision: do not amend ADR-0070 or the
  `TemplateOverlays` name rule in this spike. Write a new ADR in EP-M5 that
  records the mechanism, and state that 7.3.3 must amend ADR-0070. Most likely
  that amendment adds a dedicated substitution entry rather than relaxing
  `with_function`. Rationale: the spike changes no public surface, and the rule
  change needs the 7.3.x dialect design to choose its shape.
- D7 (2026-11-15). Decision: Verus and Kani are not selected. Conventional
  tests and property tests discharge every obligation (see
  `Verification plan`). Rationale: the only repository-owned kernel, the
  rebinding map, is a pointwise transformation whose proof would restate its
  definition. The load-bearing semantics belong to MiniJinja, and a Verus model
  could only assume them. Re-evaluate if 7.3.3 introduces composition policy
  (for example spy-of-substitute, ordering, or multi-overlay merging).
- D8 (2026-11-15). Decision: adopt the existing branch and draft PR #1088,
  rebase the branch onto `main`, and update the remote with
  `--force-with-lease`. Rationale: the user directed the adoption and the
  rebase and retired the earlier planning session, so no other worktree depends
  on the published history.

## Outcomes & retrospective

Planning only. No experiment, implementation, or Rust gate has run for this
spike. The plan revision identified the engine facts that rule out the literal
§5.4 design. It replaced that design with a wrapper-based mechanism and stated
the evidence each milestone must produce. Fill this section with results,
rejected candidates, and lessons at each milestone.

## Context and orientation

Baseline: `5c707ef33` (`origin/main`, 2026-11-15); the planning branch is
rebased onto it. `Cargo.toml:116` requires `minijinja = "2.24.0"` with `fuel`
and `loader`; `Cargo.lock` resolves 2.24.0. `rust-toolchain.toml` pins the
dated nightly that enables Polonius Alpha and the next-generation trait solver.

Environment construction lives in `src/manifest/mod.rs:193-233`
(`build_template_environment`). It creates an `Environment<'static>` with
`UndefinedBehavior::Strict` and registers host helpers, then the stdlib, then
manifest `vars`, then manifest macros (`register_manifest_macros_with_budget`),
and finally test overlays (`register_overlays`). Expansion and rendering follow
in `expand_and_render`.

Macro compilation lives in `src/manifest/jinja_macros/mod.rs`.
`register_macro_with_budget` (`:190-215`) parses the name, builds
`{% macro <signature> %}<body>{% endmacro %}` under the template name
`__manifest_macro_{index}_{name}`, and charges the source budget. It then adds
the template, validates it by rendering once (`validate_macro`), appends an
import through `register_macro_import` (`:361-365`), and installs
`make_macro_fn` as a global. `render_template_at` (`:310-358`) prepends the
prelude, charges its length, reserves fuel, renders in a fuel-bounded clone,
and maps exhaustion to typed budget errors. It is the only reader of the
prelude. `src/manifest/jinja_macros/invocation.rs` owns `make_macro_fn`,
`invoke_macro` (fresh captured state per call, Macro-stage fuel, output
charge), and `capture_macro`. `call.rs` owns keyword forwarding
(`call_macro_value`).

The template path covers every rendered field (`src/manifest/render/mod.rs`),
plus the template fallback for `when` (`render_when_template`). The expression
path covers compiled `when` conditions (`evaluate_when_expression`) and string
`foreach` expressions (`src/manifest/expand/evaluation.rs:216-243`).

Function overlays live in `src/manifest/test_surface/overlays.rs`.
`register_overlays` validates every name first and then installs each overlay,
capturing the displaced global through `Environment::globals`
(`find_registered`, `:355-360`). The rejection of manifest-macro names is at
`:242-254`.

Existing tests to reuse: `src/manifest/tests/macros/mod.rs` covers arguments,
defaults, call blocks, and repeated renders. `src/manifest/tests/overlays/`
covers function replacement, `shadowed` passthrough, and namespaces.
`src/manifest/tests/budget/` covers budget boundaries. The test module in
`invocation.rs` covers the expression fallback and holds an existing proptest.

Engine axioms, read from `~/.cargo/registry/src/*/minijinja-2.24.0/`. They are
trusted interface facts, each pinned by a characterization test in EP-M1:

- MJ-1, lookup order: locals, then the render context, then environment globals
  (`vm/context.rs:240-262`).
- MJ-2, imports: `{% from T import n as a %}` evaluates `T` in the current
  state and assigns `a` as a local; later assignments overwrite earlier ones
  (`compiler/codegen.rs:406-418`).
- MJ-3, state identity: a macro records `state.id` when built and refuses a call
  from a state with a different identity. Imports and `eval_macro` keep the
  identity (`vm/macro_object.rs:47-53`, `vm/mod.rs:118-150`, `:1118-1136`).
- MJ-4, globals: `add_function` is `add_global`, a map insert that replaces any
  existing value; `globals()` yields clones (`environment.rs:761-790`).
- MJ-5, cloning: `Environment::clone` copies globals copy-on-write and copies
  the template store, so later inserts on a clone do not leak
  (`environment.rs:70-85`, `loader.rs:26-31`).
- MJ-6, undefined values: macro defaults apply exactly when an argument is
  undefined. Strict mode rejects undefined `Value`/`&Value` Rust function
  arguments but not macro arguments or list elements (`codegen.rs:489-499`,
  `value/argtypes.rs:609-698`).
- MJ-7, arity: extra positional arguments fail with `TooManyArguments`, unknown
  keywords fail, and `caller` is accepted only when the macro body references
  it. The compiled macro exposes `arguments` and `caller` attributes
  (`vm/macro_object.rs:31-41`, `:55-121`; `codegen.rs:505-526`).

## Conformance basis

Upstream artefacts at baseline `5c707ef33`:

- `docs/roadmap.md` 7.1.3 (lines 1537-1545): RM-1, pin `add_function`
  replacement; RM-2, rewrite the prelude for substituted names; RM-3, pin
  runner-side handle capture for spy passthrough; RM-4, fall back to filtered
  registration if shadowing fails. Also 7.1.4 (dogfooding, consumes this
  evidence), 7.3.1 (journal), and 7.3.3 (production substitution doubles).
- `docs/netsuke-test-framework-technical-design.md` §4.3 (TD-4.3: loader
  options, registration order, overlay name rule), §5.4 (TD-5.4: macro
  substitution), §7 (TD-7: spy handle, lock release before delegation, journal
  origins), §12 (TD-12: invariants I1 isolation, I4 fidelity), §14 (TD-14:
  phasing and fallback).
- `docs/netsuke-test-framework-ux-design.md` §8.1 (spy semantics), §8.2 (call
  args), §8.5 (`substitute`, journal under `substitutes.<name>`).
- `docs/rfcs/rfc-0007-netsukefile-testing-framework.md`: seams-first rollout;
  the macro substitution overlay is one of two seams.
- `docs/netsuke-design.md` §4.3 (user-defined macros, `caller` support) and
  the resource-budget sections; ADR-0009 (render and macro telemetry
  boundaries); ADR-0050 (bounded evaluation); ADR-0070 (deny-by-default test
  stdlib and overlay name rule); ADR-0008 (injected environment seams);
  ADR-0006 and `docs/polonius.md` (borrow-checking posture).

There is no Terms of Reference revision specific to this item; the project
scope in `docs/terms-of-reference.md` applies unchanged.

Trace links:

- RM-1 → EP-M1 → A1 (`characterization::add_function_replaces_*`,
  `characterization::prelude_import_shadows_*`).
- RM-2 → EP-M2 and EP-M3 → A2, A3, A4, A5, A6 (import-table fidelity,
  rebinding model, single observation, call shapes, path agreement).
- RM-3 → EP-M1 and EP-M3 → A1 and A7 (value-handle capture pinned; template
  reference capture for spies, including call blocks).
- RM-4 → EP-M4 → A9 (fallback decision record, or fallback evidence).
- TD-12 I1 and I4 → A8 and A9 (isolation, atomic installation, budget and
  ordinary-build fidelity).
- TD-5.4 and TD-14 corrections → EP-M5 documentation and ADR.

## Verification plan

### Abstract model

Model the prelude independently of MiniJinja as a finite sequence `P` of import
records `r = (t, n)`, where `t` is a template name and `n` a public macro name.
The binding of a name is the template of its last record:

```plaintext
bind(P, n) = t  where (t, n) = P[i] and no j > i has P[j].n = n
bind(P, n) = ⊥  when no record names n
```

A selection `σ` is a finite partial map from public names to wrapper templates.
Rebinding is defined as:

```plaintext
rebind(P, σ) = map(P, r ↦ if r.n ∈ dom σ then (σ(r.n), r.n) else r)
               ++ [(σ(n), n) for n in dom σ, in selection order, if bind(P, n) = ⊥]
```

Installation applies `rebind` to the import table, adds the wrapper and
stand-in templates under private names, and sets each selected global to
`make_macro_fn(wrapper)`, all on a candidate environment that replaces the
original only on success. The engine bridge (MJ-1 and MJ-2) states that
rendering `prelude(P) ++ body` binds each name `n` in the body to the macro
exported by `bind(P, n)`, or to the global when `bind(P, n) = ⊥`.

### Invariants and lemma sketches

The lemma statements below are hypotheses about the proof structure.
Implementation may show some to be unnecessary or in need of refinement.

- INV-1, selected binding: for all `n ∈ dom σ`,
  `bind(rebind(P, σ), n) = σ(n)`. Sketch: the map preserves record names, so
  the positions naming `n` are unchanged, and each maps to `σ(n)`. If none
  exists, the appended record supplies `σ(n)` and no later record names `n`.
- INV-2, frame preservation: for all `n ∉ dom σ`,
  `bind(rebind(P, σ), n) = bind(P, n)`, and the subsequence of unselected
  records is equal, in order, to that of `P`. Sketch: unselected records are
  untouched, and appended records name only selected names.
- INV-3, prelude fidelity: for every record sequence, the import table renders
  byte-identical text to the legacy concatenation
  `concat("{% from '" t "' import " n " %}")`. This is a refinement relation
  between the new representation and the old one.
- INV-4, single observation: every successful call of a selected name, on
  either path, produces exactly one observation, followed by exactly one target
  invocation. A call rejected for arity or an unknown keyword produces neither.
  Sketch: the wrapper body is one observe statement followed by one aliased
  target call (by construction), and MJ-7 rejects the wrapper before its body
  runs.
- INV-5, non-reentrancy: the wrapper body never resolves the public name. It
  refers only to the private alias and the observer global. This is structural:
  the generator emits no reference to `n`. It is witnessed by the bounded
  observer guard in NC3.
- INV-6, passthrough fidelity: in spy mode, for every template over the macro
  set, including call blocks and nested calls, the substituted environment
  renders the same output or the same error kind as the baseline environment.
  Sketch: the wrapper reproduces arity (MJ-7), forwards omitted arguments as
  undefined so target defaults apply (MJ-6), and forwards `caller` exactly when
  the target references it. Imports keep state identity (MJ-3).
- INV-7, capture validity: a template-reference capture stays valid for the
  environment's lifetime because installation never removes a template. A
  captured global value stays invocable from later states for calls without a
  `caller` block, and fails with `InvalidOperation` for calls with one (MJ-3).
- INV-8, atomic installation: installation either succeeds with the templates,
  the import table, and the globals all updated, or fails with the environment
  observably unchanged.
- INV-9, isolation: installation in one environment, or in a clone, does not
  affect another (MJ-5; technical design I1).
- INV-10, budget preservation: every path still charges the shared
  `ManifestBudget`. Exhaustion surfaces as the same typed budget error and
  stage class as before; for ordinary builds, charges are unchanged (follows
  from INV-3).
- INV-11, path agreement: for calls without `caller`, the template path and
  the expression path produce equal output and equal observation records.

### Method selection and evidence

Each obligation names its method, artefact, command, evidence, discharge
condition, and non-vacuity check. All test paths are under
`src/manifest/tests/macros/substitution/` unless stated otherwise. The focused
command for each is `make test-nextest` (full suite) or `make proptest`
(property modules only); the developers' guide forbids claiming a narrower run
than the command performs.

- A1 (RM-1, RM-3, MJ-1–MJ-4, INV-7): named `rstest` tests in
  `characterization.rs`. They cover four facts. A global replaced with
  `add_function` resolves to the new callable, and a clone taken through
  `globals()` beforehand still returns the old sentinel. With the generated
  prelude in place, the replacement does not change template-path output but
  does change expression-path output. A captured macro global stays callable
  from a later render after replacement, with positional and keyword arguments.
  The same handle fails with `InvalidOperation` ("template state went away")
  when invoked with a `caller` block. Discharge: all pass. Each test asserts
  both sentinels. The shadowing test and the expression-path test use the same
  environment, so each is the other's control: a broken prelude would flip both
  outcomes. These tests characterize existing behaviour and may pass on first
  run; record that rather than manufacture a red test.
- A2 (RM-2, INV-3): a `proptest` in `substitution_property_tests.rs`
  generates zero to eight records from names drawn from a small alphabet that
  includes `m`, `mm`, and `m_1`, with duplicates allowed. It asserts that the
  table's rendered text equals the legacy concatenation, which is kept in the
  test module as an oracle. Existing macro, budget, and telemetry suites must
  pass unchanged after the refactor. Seeded fault: a one-character format
  change in the renderer must fail the property. Use 256 cases (the proptest
  default) and keep any minimized regressions.
- A3 (RM-2, INV-1, INV-2): a `proptest` over the same generator plus a
  selection subset and appended unbound names. An independent oracle computes
  `bind` from scratch (a reverse scan) and checks it against the rebound table
  for every name in the universe. It also checks that the unselected
  subsequence is unchanged. A second property renders a real environment whose
  macros return per-template sentinels and checks every name's output against
  the oracle. This bridges the model to MJ-1 and MJ-2. Non-vacuity: a
  deterministic `TestRunner` seeded with a fixed RNG classifies 512 generated
  cases. It asserts at least 10% each for empty selections, full selections,
  partial selections, cases with duplicate names, prefix-similar pairs, and
  unbound-name substitutions. Seeded faults, applied in a scratch commit and
  reverted: prefix matching with `starts_with`, rebinding only the first
  duplicate, and dropping unselected records. Each must fail a named property.
  Record the failing test names under `Artefacts`.
- A4 (RM-2, INV-4, INV-5): `rstest` tables in `rebinding.rs`. For substitute
  mode and spy mode, assert the output and the exact observation sequence (with
  `googletest` matchers such as `elements_are!`) for template-path renders and
  compiled `when` expressions. The negative controls must fail the same
  assertions when the mechanism is replaced. NC1, global replacement alone:
  original output and zero observations on the template path. NC2, the literal
  §5.4 design (import the bare stand-in): stand-in output and zero
  observations. NC3, a wrapper deliberately generated to call the public name:
  a bounded observer that returns an error on its second dispatch within one
  call stops the recursion at depth two and reports it. Unbounded Rust
  recursion must never be triggered. Each NC is a permanent test that asserts
  the defect is observable, not a test that is expected to fail.
- A5 (INV-6): `rstest` cases in `call_shapes.rs` comparing spy-mode output with
  the baseline environment. Cases cover zero parameters; positional, keyword,
  and mixed arguments; defaults omitted and overridden; too many positional
  arguments; an unknown keyword; a missing required argument under strict
  undefined behaviour; a call block with and without `caller()` arguments; a
  call block against a target that does not reference `caller`; nested macros
  that call other substituted macros; and repeated renders of the same
  environment. Errors compare by `ErrorKind`, and output compares with
  `pretty_assertions`. Non-vacuity: every case also asserts the observation
  count (one, or zero for rejected calls), so equal output alone cannot pass a
  bypassed wrapper.
- A6 (INV-11): a `proptest` in `substitution_property_tests.rs` generates
  identifier-safe positional and keyword values (the same strategy shape as the
  existing `compiled_expression_forwards_generated_arguments`) and asserts
  equal output and equal observation records for the template path and the
  expression path.
- A7 (RM-3, INV-7, TD-7 lock rule): `rstest` tests in `installation.rs`. A spy
  captures the template reference before rebinding. It passes calls, including
  call blocks, to the original after the capture scope has ended and across
  three separate renders. A spied macro whose original calls another observed
  macro completes without deadlock, which shows that the observer's lock is
  released before delegation. Deleting the original template (in a fault test
  that builds an environment without it) must fail installation with a typed
  error.
- A8 (INV-8, INV-9, INV-10): tests in `installation.rs` and `budget.rs`.
  Installing two doubles where the second stand-in fails to compile returns an
  error, and the original environment's prelude text, global names, and
  template lookups are unchanged. Installing into one clone leaves a sibling
  clone's output and observations unchanged. For budgets, each test asserts the
  typed failure and its stage: fuel exhaustion inside a substituted macro on
  the template path (render stage), the same on the expression path (macro
  stage), the per-value output cap, and source bytes. For source bytes, assert
  success at a limit equal to the rewritten prelude plus the body, and failure
  at one byte less. Reuse helpers from `src/manifest/tests/budget/boundary.rs`
  rather than copying them.
- A9 (RM-4): if EP-M3 discharges A1 to A8, record in this plan and in the ADR
  that filtered registration was not needed, citing the passing test names. Do
  not claim that an unbuilt fallback passed. Otherwise, EP-M4 builds the
  fallback and reruns A3 to A8 against it.

Snapshot tests with `insta` (inline snapshots, no external files) pin stable
multivariant outputs that reviewers should read. They cover the generated
wrapper source for four shapes (no parameters, parameters,
`caller`-referencing, and a stand-in whose name differs from the target), and
the rendered prelude after a mixed rebinding. Each is paired with the semantic
assertions above. The generator is deterministic, so no normalization is needed.

### Why these methods, and why not Kani or Verus

INV-1 to INV-3 are properties of a pure, pointwise list transformation over an
unbounded input space. Property tests with an independent oracle, coverage
classification, and seeded faults give proportionate assurance. A Verus proof
would mirror the definition of `rebind` and add no insight. A Kani harness over
`Vec<String>` would be expensive and would add mutation-evidence and
proof-scope obligations (`docs/developers-guide.md` Kani harness inventory) for
no additional confidence. INV-4 to INV-11 depend on MiniJinja's semantics
(MJ-1–MJ-7). A formal model would have to assume exactly the facts under test,
so real-engine contract tests are the appropriate rigour. The residual gap is
that the axioms are pinned by examples, not proved over the engine. This is
accepted under the rule that third-party internals are not verified. No
`assume`, `admit`, or trusted proof body is introduced. No Verus milestone
exists, so `make verus` is not an acceptance command for this plan. If 7.3.3
adds composition policy over doubles, revisit D7 before implementing it.

### Behavioural and end-to-end tests

The spike adds no user-visible behaviour, CLI surface, or loader contract. Its
installer is `cfg(test)` and is unreachable from the integration-test harness in
`tests/`. So no `rstest-bdd` scenario or `assert_cmd` end-to-end test is
added, and `docs/users-guide.md` is not changed. For 7.3.3, the following
proposed acceptance scenarios record the behaviour this spike makes possible.
They are not implemented here, and they live in
`tests/features/manifest_test_substitution.feature` when 7.3.3 adds them:

```gherkin
Feature: Macro substitution under netsuke test
  Scenario: A substituted macro renders the stand-in and journals the call
    Given a manifest whose target command calls compile_cmd('main.c')
    And a test that substitutes compile_cmd with a stand-in returning "stub"
    When the test case runs
    Then the rendered command is "stub"
    And substitutes.compile_cmd records one call with args ["main.c"]

  Scenario: A spied macro passes through, including call blocks
    Given a manifest macro wrap that renders caller() between brackets
    And a test that spies on wrap
    When a target renders a call block through wrap
    Then the rendered text equals the unspied build
    And the spy records one call

  Scenario: An unrelated macro with a similar name is untouched
    Given manifest macros m and mm
    And a test that substitutes m
    When a target renders both
    Then mm renders its original output and records nothing
```

## Plan of work and milestones

### EP-M1: characterize the engine boundary

Assigned: RM-1, RM-3 (value-handle half), A1. Re-read this plan, AGENTS.md, and
the sources named in `Context and orientation`. Sweep for existing helpers with
CodeGraph and Leta before writing new ones (AGENTS.md abstraction policy).
Create `src/manifest/tests/macros/substitution/mod.rs` with a module doc, shared
`rstest` fixtures (a manifest-macro environment builder that reuses
`register_manifest_macros_with_budget`, and a sentinel template), and
`characterization.rs` with the A1 tests. Wire the module from
`src/manifest/tests/macros/mod.rs`.

End state: a passing characterization suite that explains, from evidence, why
global replacement fails on the template path and why value-handle passthrough
cannot carry call blocks. No production change. Acceptance: the milestone gate
set below passes, and the A1 test names appear in the nextest log. Conformance
check: no public or production change, so the axioms in this plan match
observed behaviour. If an axiom is falsified, stop, record it under
`Surprises`, and revise D1 to D3 before EP-M2. Recovery: revert the single
commit. Remaining gaps: RM-2 and RM-4.

### EP-M2: typed import table and compile extraction

Assigned: RM-2 (representation), A2, INV-3. Red: add the A2 property test
against a `MacroImportTable` that does not yet exist. It fails to compile,
which is the expected red state for a new type; record the compiler error.
Green: create `src/manifest/jinja_macros/imports.rs` with:

- an ordered list of `ImportRecord { template, name }` values;
- `push`;
- `binding(&self, name) -> Option<&ImportRecord>`, a borrow-returning last-wins
  lookup;
- `render_prelude`; and
- an `Object` implementation so the table can be stored under
  `MACRO_IMPORTS_GLOBAL`.

`register_macro_import` clones the table, pushes one record, and stores the
result again (copy-on-write, as before). `render_template_at` downcasts the
global and renders the prelude. Move `macro_imports` and
`register_macro_import` into the new file. Extract
`compile_macro_template(env, signature_and_body, template_name, budget)` from
`register_macro_with_budget`, covering the source charge, `add_template_owned`,
and `validate_macro`, and leave the import and the global registration in the
caller. Every existing test must pass unchanged.

Run `make test-workflow-contracts`. If the Kani proof-scope contract names
`src/manifest/jinja_macros/imports.rs`, add exactly that line to
`tools/kani/proof-scope.toml` in alphabetical position. Record the `binding`
accessor in `docs/polonius.md` as a borrow-returning lookup.

End state: ordinary behaviour is byte-identical and proven by A2, with the
prelude representation ready for rebinding. Acceptance: A2 passes, the existing
macro, budget, telemetry, and overlay suites pass unchanged, and the full gate
set passes. Conformance check: no public change; the refactor stays within the
authorized boundary. Recovery: revert the commit; EP-M1 stands alone.

### EP-M3: prelude rebinding with journalling wrappers

Assigned: RM-2, RM-3 (template-reference half), A3 to A8. This milestone is a
prototyping milestone in the ExecPlan sense. Its code is `cfg(test)` and will
either be promoted by 7.3.3 or deleted.

Red: write A4's substitute-mode and spy-mode tests against a stub
`install_macro_doubles` that performs only `add_function` replacement (the NC1
behaviour). Run them and record the expected failure: original output and zero
observations on the template path.

Green: implement `src/manifest/jinja_macros/substitution.rs` under
`#[cfg(test)]`, declared from `jinja_macros/mod.rs`. Its pieces are as follows.

- `MacroDouble { public_name, mode }`, where `mode` is
  `Substitute { stand_in: MacroDefinition }` or `Spy`. This is the policy
  input: plain data with no engine types.
- `rebind(&MacroImportTable, &[(name, wrapper_template)]) -> MacroImportTable`,
  the pure kernel defined in the model.
- `CapturedMacro { template, macro_name, arguments, references_caller }`,
  captured from `binding` plus the engine's `arguments` and `caller` attributes
  through `capture_macro`.
- `wrapper_source(public_name, target: &CapturedMacro) -> String`, which
  generates:

  ```jinja
  {% from '<target template>' import <target macro> as __netsuke_double_target %}
  {% macro <public name>(<a1>, ..., <ak>) %}
  {%- do __netsuke_double_observe('<public name>', [<a1>, ..., <ak>], <caller flag>) -%}
  {{ __netsuke_double_target(<a1>, ..., <ak>[, caller=caller]) }}
  {%- endmacro %}
  ```

  Here `<caller flag>` and `caller=caller` appear only when the target
  references `caller`, so the wrapper accepts `caller` exactly when the target
  does. Whitespace control keeps the wrapper's output equal to the target's.
- `install_macro_doubles(env, doubles, observer) -> Result<(), MacroDoubleError>`,
  which works on `env.clone()`. It compiles each stand-in through
  `compile_macro_template` under `__netsuke_double_{k}_{name}`, captures each
  target, compiles each wrapper under `__netsuke_double_wrapper_{k}_{name}`,
  stores the rebound table, sets each selected global to `make_macro_fn` over
  the wrapper, registers the observer global, and assigns the candidate back to
  `*env` only on success.
- A test-local `RecordingObserver` that appends an observation under a mutex
  and releases the lock before returning. It can also enforce a call ceiling
  (for NC3).

Then add A3, A5, A6, A7, A8, and the snapshots. Keep each test file below 400
lines: `rebinding.rs`, `call_shapes.rs`, `installation.rs`, `budget.rs`, and
`substitution_property_tests.rs`. Remove any abandoned prototype code before
committing; keep only the selected mechanism and the permanent negative
controls.

End state: substitution and spying work on both paths, with every acceptance
item demonstrated, or a recorded failure that triggers EP-M4. Acceptance: A3 to
A8 pass; `make proptest` reports the `substitution_property_tests` cases; the
seeded faults for A2 and A3 have been run and recorded; the full gate set
passes. Conformance check: the public API is unchanged; ordinary builds are
unchanged (existing suites); the mechanism matches D1 to D4. Recovery: the
EP-M2 commit is a coherent plateau; revert EP-M3 commits if needed. Remaining
gap: the runner wiring, the journal format, and the ADR-0070 amendment, which
belong to 7.3.1 and 7.3.3.

### EP-M4: fallback decision

Assigned: RM-4, A9. Shadowing fails if any A3 to A8 item cannot pass because a
post-registration edit of the environment is unsound. Examples: a stale import
or global that cannot be rebound, a captured target that cannot be resolved
after installation, or a budget or telemetry boundary that a post-hoc edit
cannot preserve. If none fails, update this plan's `Decision log` and
`Outcomes` to record that filtered registration was unnecessary, naming the A3
to A8 tests. Then proceed to EP-M5.

If shadowing fails, implement filtered registration under `#[cfg(test)]`.
`register_manifest_macros_with_budget` gains a test-only sibling that takes the
set of names to withhold. For each withheld name it compiles the original
template under its normal private name but appends no import and installs no
global. The installer then appends the wrapper imports and globals. Rerun A3 to
A8 against it, delete the failed rebinding path, and record both outcomes.
Changing the production signature of `register_manifest_macros_with_budget` is
outside this plan's tolerances and requires approval.

End state: a recorded, evidenced decision for RM-4. Acceptance: the decision
entry cites passing tests; if the fallback was built, A3 to A8 pass against it
and the full gate set passes. Recovery: as for EP-M3.

### EP-M5: documentation, ADR, and roadmap

Assigned: the TD-5.4 and TD-14 corrections, and the documentation duties in
AGENTS.md. Use the `scribe` agent for mechanical edits, with the facts supplied
from this plan.

- Technical design §5.4: replace the bare-stand-in import with the wrapper
  mechanism, the template-reference capture, the two-path dispatch, the call
  block constraint, the journal argument shape chosen for the spike (D4), and
  the test evidence. Section §14: correct the claim that 7.1.2 settled macro
  capture, cite the spike's tests, and record the fallback outcome. Section
  §4.3: note that macro substitution needs an overlay kind distinct from
  `with_function`, with the name rule to be amended by 7.3.3. Section §7: note
  that macro spies capture by template reference.
- A new ADR under `docs/adrs/`, written as
  `adr-NNNN-macro-substitution-rebinds-imports-to-journalling-wrappers.md`.
  Allocate `NNNN` only after sweeping every remote branch for the highest
  minted number, because open branches hold numbers above `main`'s highest
  (0074 at baseline) and a published number is never reused. The ADR has status
  `Accepted` with the date. It records the context (MJ-1–MJ-3), the decision
  (D1–D3, D5), the rejected alternatives (bare stand-in import, global
  fallthrough, value-handle capture, and string parsing), and the consequence
  that ADR-0070 needs an amendment in 7.3.3. Reference it from technical design
  §5.4.
- `docs/developers-guide.md`, under "Template rendering and macro
  registration": the import table, `compile_macro_template`, the ownership and
  permitted call sites of the `cfg(test)` installer (manifest macro tests only,
  pending 7.3.3), and the focused commands. Add the reuse policy for
  `MacroImportTable`: it is the only writer and reader of the prelude, and its
  callers are limited to `jinja_macros`.
- `docs/repository-layout.md` and `docs/contents.md`: index the new modules
  and the ADR. `docs/polonius.md`: already updated in EP-M2.
- `docs/roadmap.md`: mark 7.1.3 and its four sub-bullets `[x]` only after A1 to
  A9 are discharged, using the `mapsplice` skill or a minimal checkbox edit.
  Leave 7.1.4 and 7.3.3 unchecked.
- `docs/users-guide.md`: no change, because there is no user-visible
  behaviour. Confirm that during the final diff review.

End state: the documents match the evidence, and the plan's status is
`COMPLETE`. Acceptance: the documentation gates and the full gate set pass;
`Progress` and `Outcomes` describe the actual evidence; no deviation remains
unrecorded.

At every milestone boundary, run the conformance check. Confirm that the
assigned requirements are met, that the design still matches D1 to D7, that no
axiom was falsified, that no public interface, dependency, or persistent format
was added, and that the trace links are current. Any unapproved deviation
follows the exception procedure: record it under `Decision log`, set
`Status: BLOCKED`, and wait.

## Concrete steps

Run every command from the repository root on the task branch. Delegate gate
runs to `scrutineer`, one gate at a time, with each command's output captured
through `set -o pipefail` and `tee` to
`/tmp/$ACTION-netsuke-7-1-3-spike-mini-jinja-overlay-shadowing.out`.

During red-green work, run the focused suites:

```sh
set -o pipefail
make test-nextest 2>&1 | tee /tmp/test-nextest-netsuke-7-1-3-spike-mini-jinja-overlay-shadowing.out
make proptest 2>&1 | tee /tmp/proptest-netsuke-7-1-3-spike-mini-jinja-overlay-shadowing.out
```

`make test-nextest` runs the full non-doctest suite with the repository's
flags. It is not a filtered run, so do not describe it as one. Record test
names, counts, exit status, and each expected red failure in `Artefacts`.
`make proptest` uses `--no-tests fail`, so a misnamed property module fails
loudly. Expected evidence after EP-M3 includes lines such as:

```plaintext
PASS [ ...] netsuke manifest::tests::macros::substitution::rebinding::substitute_mode_observes_template_path_call
PASS [ ...] netsuke manifest::tests::macros::substitution::substitution_property_tests::rebinding_matches_binding_oracle
```

After every milestone, run this gate set sequentially on the same candidate.
Every command must exit 0:

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

`make test-workflow-contracts` is required because this work changes a
`docs/execplans/` file, which Rust and workflow contracts read, and may change
`tools/kani/proof-scope.toml`. Repeat the affected gates after any fix. Never
commit a red candidate. Warnings remain errors, and doc coverage stays at or
above 80%.

For the plan-only commit that publishes this document, run the same gate set,
because ExecPlan files are read by `make test` and
`make test-workflow-contracts`.

## Idempotence and recovery

Every test builds fresh environments and a fresh observer. No mutable registry
or counter is shared between cases. Installation never mutates the input
environment on failure, so re-running a test cannot accumulate imports. Seeded
fault experiments are applied in a scratch commit and reverted with
`git revert`, never with a reset that could discard task work. Each milestone
is one or more gated commits, and each is a usable rollback point. Never reset
unrelated changes, stash with a bare `git stash`, or terminate another agent's
builds.

## Interfaces and dependencies

Production, crate-private, after EP-M2:

```rust
pub(super) struct MacroImportTable { /* ordered ImportRecord values */ }
pub(super) struct ImportRecord { pub(super) template: String, pub(super) name: String }
impl MacroImportTable {
    pub(super) fn push(&mut self, record: ImportRecord);
    pub(super) fn binding(&self, name: &str) -> Option<&ImportRecord>;
    pub(super) fn render_prelude(&self) -> String;
}
pub(super) fn compile_macro_template(
    env: &mut Environment<'static>,
    request: &MacroTemplateRequest<'_>,
    budget: &ManifestBudget,
) -> anyhow::Result<()>;
```

Test-only, after EP-M3, under `#[cfg(test)]` in `jinja_macros/substitution.rs`:
`MacroDouble`, `DoubleMode`, `CapturedMacro`, `rebind`, `wrapper_source`,
`install_macro_doubles`, and `MacroDoubleError` (a `thiserror` enum with
variants for compile failure, missing target, and invalid stand-in). No
dependency changes are needed.

## Documentation and skill signposts

Read first: `docs/contents.md`, `docs/repository-layout.md`,
`docs/documentation-style-guide.md`, `docs/netsuke-design.md` §4.3, and the
technical design and UX design sections listed in `Conformance basis`. Testing
guidance: `docs/rust-testing-with-rstest-fixtures.md`,
`docs/rstest-bdd-users-guide.md`,
`docs/reliable-testing-in-rust-via-dependency-injection.md`, and
`docs/rust-doctest-dry-guide.md` (for doc examples on `MacroImportTable`).
Configuration (not changed here): `docs/ortho-config-users-guide.md`. Borrowing:
`docs/polonius.md`.

Skills: `execplans` (this plan), `rust-router` with `rust-unit-testing` and
`proptest`, and `rust-verification` if D7 is revisited. Use `codegraph-mcp` and
`leta` for navigation, `hexagonal-architecture` for the policy and adapter
split, and `nll-to-polonius` only if a borrow error appears. Use
`commit-message` for commits, `pr-creation` for the PR,
`logisphere-design-review` for design challenge, and `firecrawl-mcp` for any
external research. Agents: `scrutineer` runs the gates, `scribe` makes the
documentation edits, and `wyvern` does read-only reconnaissance.

## Artefacts and notes

Record here during execution: gate log paths and outcomes, A1 to A9 test names,
red-stage failure excerpts, seeded-fault results, the ADR number and path, and
the final implementation commits.

Research inputs (2026-11-15): MiniJinja 2.24.0 source in the shared Cargo
registry (paths cited under `Context and orientation`); docs.rs
[Environment][engine-environment] and [syntax][engine-syntax] pages, retrieved
through Firecrawl; and the maintainer's write-up on the VM design
[MiniJinja learnings][minijinja-learnings], which quotes the state-identity
check.

[engine-environment]: https://docs.rs/minijinja/2.24.0/minijinja/struct.Environment.html

[engine-syntax]: https://docs.rs/minijinja/2.24.0/minijinja/syntax/index.html

[minijinja-learnings]: https://lucumr.pocoo.org/2024/8/27/minijinja/

Revision note (2026-11-04): initial approval-gated plan against `b99c9c92b`.

Revision note (2026-11-04, expert review): clarified both dispatch paths and
the fallback's final bindings, and required budget boundaries and bounded
negative controls.

Revision note (2026-11-15): rebased the plan onto `5c707ef33` and rewrote it
after source-level engine research. The changes and their reasons are:

- The mechanism is now a generated journalling wrapper imported in place of
  the original (D1, D2), because the literal §5.4 design journals nothing on
  the template path.
- Spy capture is by template reference (D3), because MJ-3 rules out call blocks
  through value handles.
- A typed import table makes rebinding a pure kernel (D5).
- The plan adds an abstract model, invariants INV-1 to INV-11, method
  selection with non-vacuity evidence, and an explicit decision against Kani
  and Verus (D7).
- It corrects the property-module naming for `make proptest`, adds
  `make test-workflow-contracts` and the proof-scope step, and re-derives the
  tolerances from comparable plans.

Remaining work is unchanged in kind: approval, then EP-M1 to EP-M5.
