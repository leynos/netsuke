# Spike MiniJinja macro overlay shadowing (7.1.3)

Status: DRAFT

Implementation requires explicit approval. This document is a plan, not
evidence that the spike has run or that macro substitution is available to
users.

This ExecPlan is a living document. Keep its constraints, tolerances, risks,
progress, discoveries, decisions, outcomes, conformance basis, and verification
plan current after each milestone.

## Purpose / big picture

Prove how the future Netsukefile test runner can replace a manifest macro while
retaining the original callable for a spy: a wrapper that observes a call and
then delegates to the original. A successful spike demonstrates the substituted
output, exactly one wrapper invocation per call, and unchanged ordinary builds.
It supplies executable evidence for roadmap 7.3.3, not the test dialect, mock
engine, journal, or `netsuke test` command itself.

An overlay is a callable installed under an existing template name. A prelude
is the generated sequence of Jinja import statements prepended to rendered
strings. Both matter: replacing an environment global cannot override a name
that the prelude imports into the template's local scope.

## Constraints

- Implement only after approval. Keep roadmap 7.1.3 unchecked during planning.
- Use the real MiniJinja dependency and production macro compilation,
  invocation, rendering, and resource-budget paths. Do not emulate the engine.
- Preserve ordinary build and manifest-query behaviour, existing macro argument
  handling, call blocks, strict undefined values, and budget diagnostics.
- Keep this an additive, internal spike. Use test-only candidate installation
  and observation code; do not expose a production macro-substitution API or
  relax `TemplateOverlays::with_function` namespace rejection. Promotion to the
  runner belongs to 7.3.3. Small production helper extractions are permitted
  only to exercise the real path without copying its semantics.
- Separate selection policy (which name is substituted) from the MiniJinja
  adapter (templates, globals, imports, and callable values). Do not transplant
  a new directory architecture or introduce a general-purpose port.
- No CLI flags or configuration sources are needed. Preserve existing
  `ortho_config` layering and localized help. Any later configuration surface
  must use `ortho_config`, not a parallel parser or ambient environment reads.
- No unsafe lifetime extension, process-environment mutation, host network
  access, command execution, or new runtime dependencies. Existing `rstest`,
  `rstest-bdd`, `googletest`, `pretty_assertions`, `insta`, and `proptest` are
  authorized test tools. Never bypass compiler admission or isolate Cargo's
  shared cache. Do not use `/tmp` as a build target.
- Keep code files at most 400 lines. Preserve Polonius borrow-returning forms
  and the pinned toolchain; add no borrow-checker or trait-solver flags.

## Tolerances (exception triggers)

Stop implementation, record the evidence and options, set `Status: BLOCKED`,
and seek explicit acceptance if the solution needs a public interface, dialect,
configuration, dependency, or ordinary-build semantic change; unsafe code; or a
new business-policy contract. Stop if both the prelude candidate and filtered
registration fail the required wrapper, argument, call-block, or budget tests.

Allow at most 12 non-ExecPlan files and 800 net added lines of code and other
documentation. Exclude all ExecPlan files from both counts. A larger change
requires a revised scope. There is no time limit. A healthy compiler-admission
queue is not a failed experiment; record infrastructure failures separately.

## Risks

- High impact, likely: a direct stand-in import returns the right text while
  bypassing the wrapper. Assert dispatch counts and arguments, not just text.
- High impact, plausible: a captured raw macro value belongs to an expired or
  different rendering state. Capture the registered Rust callable instead and
  exercise repeated calls after the capture scope ends.
- High impact, plausible: call blocks or nested macros behave differently via
  the global-function fallback. Test both template and expression paths; do not
  declare success based on simple positional calls alone.
- Medium impact, plausible: string replacement changes another macro with a
  similar name or leaves stale imports. Generate a prelude from known ordered
  registration records; do not parse arbitrary user Jinja with a regular
  expression. Test empty, partial, and complete substitution sets.
- High impact, plausible: an alternate invocation path loses shared source,
  fuel, or output accounting. Retain `ManifestBudget` and test exhaustion.

## Progress

- [x] (2026-11-04) Rename the branch and inspect repository instructions.
- [x] (2026-11-04) Complete two Wyvern reconnaissance passes and inspect the
  prerequisite, macro path, and dependency documentation.
- [x] (2026-11-04) Complete six-perspective community-of-experts review and
  reconcile dispatch, verification, budget, and command findings.
- [ ] Validate, commit, and publish this plan as a draft pull request.
- [ ] Obtain explicit implementation approval.
- [ ] EP-M1: characterize replacement, import precedence, and handle capture.
- [ ] EP-M2: validate prelude rewriting or the filtered-registration fallback.
- [ ] EP-M3: reconcile documentation, run final gates, and mark 7.1.3 done.

## Surprises & discoveries

Roadmap 7.1.2 is complete and the loader seam exists. Ordinary function
overlays already capture a displaced callable through `Environment::globals`
before replacement. Macro names remain intentionally rejected by that API. The
spike must not mistake existing function tests for macro substitution evidence.

Technical design §14 says replacement and capture were settled by 7.1.2, while
§5.4 and roadmap 7.1.3 require macro-specific evidence. Interpret the former as
function-only evidence and clarify it when recording spike results.

The prelude and expression paths are distinct. `make_macro_fn` explicitly
creates a short-lived captured state for expression calls; template imports
support call blocks in the active state. This is a feasibility boundary, not a
reason to retain borrowed state unsafely.

## Decision log

- 2026-11-04: Keep the spike test-only except for minimal shared-path
  extractions. Rationale: roadmap 7.3.3 owns production substitution doubles;
  7.1.3 needs executable feasibility evidence before that implementation.
- 2026-11-04: Treat replacement semantics and capture portability as hypotheses
  requiring real dependency tests. Documentation establishes the callable API
  but does not establish every interaction with Netsuke's generated imports.
- 2026-11-04: Require wrapper observation on the rewritten-import path. A
  stand-in result without observation does not discharge the future journal
  requirement. Compare a wrapper-preserving import with filtered registration
  before choosing one; record the result in technical design §5.4.

## Outcomes & retrospective

Planning has identified the existing seams and the distinction between global
replacement and template-local imports. No implementation, experiment, or Rust
validation has been performed by this plan-authoring task. Implementation
outcomes and exact test evidence remain pending approval.

## Context and orientation

The baseline is commit `b99c9c92b554263eea78bba429cd4c57d6b80ce0`. `Cargo.toml`
requires MiniJinja 2.24.0 with `fuel` and `loader`; `Cargo.lock` resolves
2.24.0. `rust-toolchain.toml` pins `nightly-2026-10-22`; repository policy
identifies Polonius Alpha as the borrow-checking posture.

`src/manifest/mod.rs` builds the template environment, registers variables,
registers manifest macros, then registers overlays before expansion.
`src/manifest/loader_options.rs` owns `ManifestLoadOptions` and registration
modes. `src/manifest/test_surface/overlays.rs` owns `TemplateOverlays`,
`OverlayCallable`, and `OverlayCall`. `register_overlay` captures the existing
registered value before replacing it; `OverlayCall::shadowed` exposes it to the
callback, and trailing keyword arguments are retained when forwarding.

`src/manifest/jinja_macros/mod.rs::register_macro_with_budget` compiles each
macro into `__manifest_macro_{index}_{name}`, appends an import through
`register_macro_import`, and installs `make_macro_fn` as a global.
`MACRO_IMPORTS_GLOBAL` stores the generated prelude. `render_template_at`
prepends it and renders in a bounded clone of the environment.
`src/manifest/jinja_macros/invocation.rs` owns the registered Rust callback and
its per-invocation capture; `call.rs` owns macro argument forwarding.

Existing tests in `src/manifest/tests/macros/mod.rs` cover positional and
keyword arguments, defaults, call blocks, and repeated renders. Tests in
`src/manifest/tests/overlays/registration.rs` cover function replacement and
passthrough with call counts. Namespace tests in the adjacent `namespaces.rs`
protect existing rejections. Reuse these paths and fixtures before introducing
helpers. Record ownership and permitted call sites for any new shared helper in
the technical design and developers' guide.

## Conformance basis

Use the following repository documents at the baseline revision above. They are
upstream design inputs, not a claim that every proposed feature is implemented:

- [Roadmap](../roadmap.md), 7.1.2, 7.1.3, 7.1.4, and 7.3.3: prerequisite,
  spike acceptance, later differential dogfooding, and production doubles.
- [RFC 0007](../rfcs/rfc-0007-netsukefile-testing-framework.md): shared compiler
  pipeline and seams-first rollout. The canonical filename includes `rfc-`.
- [Technical design](../netsuke-test-framework-technical-design.md), §§4.3,
  5.4, 7, 12, and 14: loader seam, prelude rewrite, spy capture, invariants,
  and filtered-registration contingency.
- [UX design](../netsuke-test-framework-ux-design.md), §8: substitution and
  spy semantics; [primary design](../netsuke-design.md): macro and compiler
  semantics; [terms of reference](../terms-of-reference.md): project scope.
- [ADR-0008](../adrs/adr-0008-environment-seam-taxonomy.md): injected inputs;
  [Polonius guidance](../polonius.md): borrow-sensitive implementation policy.

Trace R1 (`add_function` replacement) to EP-M1/A1; R2 (rewritten prelude) to
EP-M2/A2; R3 (runner-side captured original) to EP-M1 and EP-M2/A3; R4
(filtered registration if shadowing fails) to EP-M2/A4. Preserve build fidelity
and per-load isolation through EP-M2/A5. EP-M3 reconciles all five evidence
items with design §§5.4 and 14 and the roadmap.

## Verification plan

Axioms are the safe Rust and MiniJinja public interfaces, the project's macro
parser, and existing compiler-stage contracts. Test repository integration
against the locked engine; do not claim to prove MiniJinja internals. Consult
[Environment][engine-environment], [Template][engine-template], and
[State][engine-state], retrieved through Firecrawl on 2026-11-04.
`add_function` wraps a function as a global; `globals` enumerates registered
values; `State::call_macro` looks up a name in its state. None of those facts
alone proves cross-state macro capture safety.

A1: two distinct functions registered under one global name must resolve to the
second, while a retained first callable still returns its original result. Use
`rstest` in a new `src/manifest/tests/macros/overlay_shadowing.rs`, wired from
`macros/mod.rs`. Return visibly different sentinel strings and count both
calls. A negative control that skips replacement must fail the second-result
assertion. This characterizes an existing interface and may pass immediately;
record that honestly rather than manufacturing a failing dependency test.

A2: for a selected macro, rendered templates and compiled expressions must
reach the stand-in through the observation wrapper exactly once per invocation.
Unselected macros retain their original result. First demonstrate that a global
replacement with the old prelude produces the original result and zero wrapper
calls. The desired substitution regression must fail before the candidate
rewrite. Use `googletest` for result and call-shape checks and
`pretty_assertions` for rendered strings. Include positional arguments, keyword
arguments, defaults, wrong arity, invalid stand-in syntax, missing stand-in,
undefined values, call blocks, nested calls, and repeated renders. Snapshot
only stable multivariant output or diagnostics where exact comparison is
useful; use `insta` and inspect every accepted snapshot.

A3: capture the original registered callable before replacement, retain its
compiled template under its original internal name, and invoke it through the
spy after the temporary capture scope has ended. Exercise two calls with
different arguments, repeated renders, keyword forwarding, original errors, and
two separate environments. Assert the wrapper count and the original result
together. A wrapper that resolves the public substituted name again must be
rejected by a bounded observer guard that errors on the second dispatch; never
trigger unbounded Rust recursion as a negative control. Do not capture a raw
macro from one `State` and assume it can be called from another.

A4: if the rewritten import cannot preserve observation or supported call
shapes, use filtered registration. Its final environment retains the original
and stand-in compiled templates under private internal names, has no import
binding for the selected public name, and exposes the observation wrapper as
that public global. Suppress or remove public imports generated while compiling
the stand-in through the production path; otherwise the bypass returns. The
wrapper delegates through the captured original or stand-in Rust callable,
never through a lookup of its own public name. Rerun A2/A3, including call
blocks. This is an evidence-selected fallback, not a runtime retry or dual
implementation. A missing original template must fail the passthrough witness.
If call blocks or any other required contract fail, stop with a documented
blocker rather than narrowing semantics or marking the spike complete.

A5: selection must leave every unselected macro binding and each independent
load unchanged. Add `overlay_proptests` under the new macro test module with a
`proptest` generator of 0–8 unique valid names and substitution subsets.
Include similar names such as `m` and `mm`, no substitutions, all
substitutions, and order permutations. Build expected name-to-sentinel answers
independently from the generated prelude; render calls against the real engine.
Record coverage of empty, partial, and full selections, use the default 256
generated cases, and retain minimized regressions. A deliberately wrong
template binding for an unselected import must be detected. Also compare
unaffected import records and exercise an unselected call block: deleting an
import alone can leave simple output unchanged because the global supplies a
fallback. This samples a broad input space; it is not an exhaustive proof.

Preserve finite boundary contracts with `src/manifest/tests/budget/boundary.rs`
and distinct substitution tests for source bytes, per-evaluation fuel,
aggregate fuel, and output bytes. Require exact-limit success and one-over
failure where the existing budget supports that distinction; preserve the typed
budget failure and stage. No new business logic or mathematical lemma is
introduced by this adapter spike. Proptest plus real-engine contract tests are
proportionate; Kani and Verus would not establish the external engine's
semantics. If promotion introduces a new contractual policy or lemma, revise
the plan and add a substantive Verus proof before implementing that policy.

## Plan of work and milestones

### EP-M1: characterize the existing boundary

After approval, re-read the baseline, instructions, and named source paths.
Sweep for existing registration/prelude helpers with CodeGraph and Leta before
adding anything. Add the A1 tests, the global-only negative control, and A3
capture tests using the existing `make_macro_fn` representation. Keep candidate
code in the macro test module. Record precise ownership and lifetime evidence.

The plateau is a passing characterization suite explaining why global-only
macro replacement fails. It advances R1/R3 without promising substitution. Run
the full sequential milestone gates below and commit. Recovery is to fix only
the new tests or revert this atomic commit; EP-M2 remains outstanding. There is
no compatibility shim or production API change.

### EP-M2: prove the candidate and select the fallback if necessary

Add the failing desired-behaviour tests before candidate changes. In test-only
installation code, compile stand-ins through the production registration path,
capture originals before replacement, and rebuild the generated prelude for the
selected names. Keep unaffected imports in their original order. The imported
replacement must preserve observation: a direct import of the bare stand-in is
insufficient. Prototype an explicitly unproven compiled wrapper entrypoint that
invokes the observer and delegates to the stand-in in the active template
state, then verify arguments, call blocks, nested calls, and budgets
explicitly. Template rendering imports this wrapper macro; expression
evaluation uses the registered Rust wrapper. Require both paths to reach the
same stand-in and record identical call data. These are two engine entry paths
to one substitution mechanism.

Treat both global replacement and prelude replacement as one installation: all
names and stand-ins must validate before publishing either half to the test
environment. Test that an invalid candidate leaves an existing environment
unchanged, or construct and discard a private candidate environment on failure.
Do not mutate the baseline environment incrementally and continue after error.

If the candidate fails A2/A3, record the failed case and try A4's filtered
registration. Keep only the successful mechanism and the useful negative
control; remove abandoned prototype machinery. A fallback requiring behaviour
outside the stated constraints needs approval, even though filtered
registration itself is already an authorized experiment.

This plateau discharges R2/R3/R4 through A2–A5 and records which mechanism is
viable. Verify that ordinary loaders and namespace protections are unchanged.
No runner journal is introduced: a local counter and captured arguments model
observation only. Record the exact gap between this harness and later 7.3.3
production wiring. Run all milestone gates and commit. On failure, retain the
last green commit and evidence; do not weaken the acceptance criteria.

### EP-M3: record the result and hand off

Update technical design §§5.4 and 14 with the chosen mechanism, rejected
alternative, callable lifetime, call-block limits, and test evidence. If the
result changes the architecture substantively, write an ADR under `docs/adrs/`
using the next free number and reference it from the technical design. Update
`docs/developers-guide.md` with internal ownership and rerun commands. Update
`docs/repository-layout.md` if a new support module is introduced and index new
documentation in `docs/contents.md`.

This spike should not change user behaviour, so do not advertise a new command
in `docs/users-guide.md`. If approved scope changes make user behaviour
visible, update that guide and add behavioural and end-to-end tests before
completion. Use `ortho_config` for any approved configuration and localized
help additions.

Reconcile R1–R4 and A1–A5 with the upstream documents, record full gate
results, and mark roadmap 7.1.3 and its acceptance bullets done only after the
spike succeeds. For R4 record either the verified fallback or that the
successful primary path made fallback unnecessary; never claim an unrun
fallback passed. Leave 7.1.4 and 7.3.3 unchecked. Use the `mapsplice` skill for
the roadmap edit. Commit this coherent final plateau. Set this plan `COMPLETE`
only when its progress and outcomes describe the actual evidence and no
deviation remains.

At every milestone, check assigned requirements, architecture boundaries,
upstream assumptions, dependencies, public surfaces, persistent formats, and
trace links. Any unapproved deviation follows the exception procedure above.

## Concrete steps and validation

Run commands from the repository root on the task branch. Delegate full gates
to the `scrutineer` agent, with one gate active at a time and output captured
using `set -o pipefail` and `tee` under `/tmp`. First run relevant existing
macro and overlay tests; after each test or candidate edit rerun the focused
suite. The verified Makefile test command is:

```sh
set -o pipefail
make test-nextest \
  2>&1 | tee /tmp/713-macro-tests.out
```

The Makefile does not consume `NEXTEST_FILTER`; this command honestly runs the
full non-doctest suite with repository flags. Do not describe it as a focused
run. Record test names, counts, exit status, and each expected red failure.
Require the evidence items assigned to the current milestone, then all
applicable A1–A5 items at final acceptance. `make proptest` selects property
modules, including the planned `overlay_proptests`, and rejects zero-test runs.

After every major implementation milestone run, sequentially:

```sh
make fmt
make check-fmt
make typecheck
make lint
make doc-coverage
make test
make markdownlint
make nixie
```

Capture a separate log for each command. All must exit zero on the same final
candidate; warnings remain errors, doc coverage remains at least 80%, and
pending compiler admission is not a pass. Follow AGENTS.md for conditional
workflow, package, coverage, and documentation-contract gates if those paths
are changed. Repeat affected gates after fixes; do not commit a red candidate.

Behaviour-driven tests with `rstest-bdd` and command end-to-end tests are not
required for a strictly test-only feasibility harness: it adds no user flow or
production loader contract. If shared production logic is changed beyond a
behaviour-neutral extraction, add scenarios to the existing manifest BDD
harness and exercise the public loader boundary before accepting that change.
Do not invent a `netsuke test` invocation before the command exists.

For this plan-only pull request, run `make fmt`, `make check-fmt`,
`make markdownlint`, `make nixie`, and `git diff --check` sequentially. Rust
implementation gates above are delivery requirements for the future approved
milestones, not evidence claimed by publication of this document.

## Idempotence and recovery

Tests use fresh environments and observation state per case. Never share a
mutable registry or counter across cases. Re-running a test must not accumulate
imports. Keep production macros and captured original templates intact until
all calls using them have completed. Keep each milestone in a gated atomic
commit; revert only task-owned changes when recovery is required. Preserve
failed experiment logs and distinguish a semantic failure from infrastructure
failure. Never reset unrelated changes or terminate another agent's builds.

## Documentation and skill signposts

Read
[documentation contents](../contents.md), [repository layout](../repository-layout.md),
and the [documentation style guide](../documentation-style-guide.md) first.
Follow [Rust fixtures][rust-fixtures], [doctest guidance][doctests],
[dependency injection][injection], [OrthoConfig][ortho], and
[rstest-bdd](../rstest-bdd-users-guide.md) for applicable implementation facets.

Load `execplans`, `codegraph-mcp`, `rust-router`, and `hexagonal-architecture`.
Use `leta` for symbol navigation, `rust-unit-testing` for fixtures and
assertions, `proptest` for generated invariants, and `firecrawl-mcp` for
external research. Use `logisphere-experts` to challenge the design,
`commit-message` for atomic commits, and `pr-creation` for the draft handoff.
Keep engine-specific values in the adapter; do not add a domain dependency on
MiniJinja to justify this spike.

## Artefacts and notes

Planning evidence is shared with the Wyvern team through context pack
`pk_nzypnvnu`. The repository paths and obligations above are the durable
handoff; the plan must remain usable after the pack expires. Record experiment
logs, exact dependency revision, successful test names, rejected-candidate
evidence, and the final implementation commit here during execution.

Revision note (2026-11-04): Initial approval-gated plan, based on current
source, two reconnaissance passes, and version-specific upstream API research.
No implementation is authorized by this document's publication.

[engine-environment]: https://docs.rs/minijinja/2.24.0/minijinja/struct.Environment.html

[engine-template]: https://docs.rs/minijinja/2.24.0/minijinja/struct.Template.html

[engine-state]: https://docs.rs/minijinja/2.24.0/minijinja/struct.State.html

[rust-fixtures]: ../rust-testing-with-rstest-fixtures.md

[doctests]: ../rust-doctest-dry-guide.md

[injection]: ../reliable-testing-in-rust-via-dependency-injection.md

[ortho]: ../ortho-config-users-guide.md

Revision note (2026-11-04, expert review): Clarify both dispatch paths and the
fallback's final bindings; replace an ineffective mutation and unsupported
filter command; require all four budget boundaries and bounded negative
controls. These changes strengthen the same spike without authorizing runner
implementation.
