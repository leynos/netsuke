# RFC 0012: Property-based testing of generated build scripts

## Preamble

- **RFC number:** 0012
- **Status:** Proposed
- **Created:** 2026-09-02

## Summary

This RFC extends the Netsukefile testing framework proposed in
[RFC 0007](0007-netsukefile-testing-framework.md) with a lightweight,
declarative property-testing capability. The extension lets Netsukefile authors
state invariants over families of manifests rather than single fixed fixtures,
with the generated build script — principally its command invocations and
environment constructions — as the subject under test.

The extension has three parts. First, a new `result.actions` view projects each
build edge's tokenized command line and constructed environment through a
pinned shell, giving assertions a structured, host-independent surface. Second,
a bounded `forall` block adds declarative input generation with deterministic
expansion, seeded sampling, and replayable failures. Third, a closed vocabulary
of built-in metamorphic relations (permutation, irrelevant insertion,
consistent renaming, and input removal) checks the generator's structural
invariants without requiring user-authored predicates.

The design deliberately follows the repository's established idiom for
build-artefact property testing: pure validators over structured projections,
bounded generated mutations, pinned seeds, and persisted regressions, as
practised in `tests/workflow_contracts/` and `tests/makefile_test_target/`.

## Problem

RFC 0007's dialect asserts on one manifest at a time, and its result views
expose the serialized manifest, a graph summary, and the raw Ninja text. Two
gaps prevent authors from testing the properties that matter most for a
build-script generator:

- **Invocations and environments are not observable.** The graph view
  exposes target names, dependencies, and rule names only; the technical design
  notes that its `description` field is discovery metadata, not the rule
  command. The only remaining surface is the raw Ninja string, and the
  [technical design §9](../netsuke-test-framework-technical-design.md) warns
  that recipe text is host-shell-sensitive because rendering flows through
  `RecipeShell::host_default()`. String assertions over `result.ninja` are
  therefore fragile across platforms, and per-action environment maps are not
  exposed at all.
- **Single fixtures cannot express invariants.** Properties such as "the
  generated script is invariant under manifest declaration order", "every
  declared source appears exactly once in the compile invocation", or "the
  constructed environment is closed over the declared variables" quantify over
  families of manifests. The
  [UX design §15](../netsuke-test-framework-ux-design.md) defers even simple
  data-driven case tables, so each additional input shape today costs a
  handwritten case, and omission bugs (a mutation the author never thought to
  write down) go untested.

Netsuke's own verification demonstrates the value of closing this gap: the
[technical design §12](../netsuke-test-framework-technical-design.md)
property-tests fixture teardown ordering with `proptest`, and the workflow
contract suites in `tests/workflow_contracts/` generate valid and mutated step
sequences with Hypothesis so that "omissions, duplicate gates, and incorrect
setup order cannot be mistaken for valid states". Netsukefile authors have no
equivalent instrument for their own build scripts.

## Current state

The plan-mode pipeline composes `load_manifest`, `build_graph`, and
`generate_ninja`, exposing `result.manifest`, `result.graph`, and
`result.ninja` (technical design §4). The views are documented as stable,
additive-only surfaces decoupled from internal intermediate representation (IR)
types, which makes a new projection the sanctioned extension path.

The dialect is declarative and closed: predicate functions and dynamic response
handlers are deliberately absent (UX design §16), and general scripting is a
rejected alternative (UX design §17). Assertions are MiniJinja expressions plus
structured `equals`, `contains`, `matches`, and `expect_failure` forms (UX
design §11). Parameterized case tables, snapshot assertions, and session-scoped
fixtures are deferred (UX design §15).

The repository's existing property suites establish the house idiom this RFC
adopts:

- `tests/makefile_test_target/rustdocflags.rs` models Makefile recipes as
  extracted structured values, asserts contracts on the extracted token rather
  than raw text, and uses a completeness check so new recipes opt into the
  contract deliberately.
- `tests/workflow_contracts/ci_transition_property_test.py` and
  `runner_placement_properties_test.py` layer bounded, derandomized Hypothesis
  properties over pure validators, generating valid sequences plus `missing`,
  `duplicate`, and `misordered` mutations.
- `proptest-regressions/` persists failing seeds so regressions replay
  deterministically.

## Goals and non-goals

- Goals:
  - Give assertions a structured, host-independent view of every build
    edge's command invocation and constructed environment.
  - Let a test case quantify over a bounded, declaratively specified
    family of inputs, with deterministic default execution and seed-based
    replay of failures.
  - Provide built-in metamorphic relations for the generator's structural
    invariants: determinism, order invariance, locality, and rename
    isomorphism.
  - Keep the dialect closed: no user-authored predicate functions, no
    embedded scripting, and no new expression engine.
  - Keep every property inside the plan/apply split: properties observe
    plan-mode projections and never execute the build.
- Non-goals:
  - A general-purpose property-testing framework for concerns outside the
    Netsukefile under test. Netsuke's own Rust and Python suites keep
    their existing tools.
  - Fuzzing Ninja, the shell, or any spawned process; execution-mode
    testing remains deferred as in RFC 0007.
  - Unbounded random generation, wall-clock-dependent seeding, or
    coverage-guided exploration.
  - Sophisticated shrinking. Minimization is limited to delta reduction
    over the drawn tuple.

## Proposed design

### 1. The `result.actions` view

A fourth pipeline projection, `result.actions`, becomes available after
`build_graph`. It is produced from the IR with the recipe shell pinned through
the `from_manifest_for_shell` seam the technical design §9 already anticipates,
so identical manifests yield identical projections on every host.

The following table lists the fields exposed for each action.

| Field     | Meaning                                                 |
| --------- | ------------------------------------------------------- |
| `target`  | The build edge's primary output target name.            |
| `rule`    | The owning rule name.                                   |
| `argv`    | The tokenized command, post-interpolation, pre-quoting. |
| `env`     | The constructed environment map for the invocation.     |
| `cwd`     | The working directory, when the action declares one.    |
| `inputs`  | The resolved `$in` bindings.                            |
| `outputs` | The resolved `$out` bindings.                           |
| `pool`    | The assigned pool, when present.                        |
| `depfile` | The depfile path template, when present.                |
| `dyndep`  | The dyndep binding, when present.                       |

_Table 1: Fields of an entry in the `result.actions` view._

`argv` is deliberately the shell-independent token list: assertions about
quoting belong to the escaping seam's own tests, not to Netsukefile authors.
The view carries helpers mirroring the graph view's surface: `action(target)`,
`actions_for_rule(name)`, and `has_action(target)`.

The projections use a canonical order. `result.actions` sorts first by the
primary key `target`, then by `rule`, and finally by the canonical
serialization of `argv`, `env`, `cwd`, `inputs`, `outputs`, `pool`, `depfile`,
and `dyndep` as tie-breakers. `actions_for_rule(name)` applies the same
ordering to its filtered entries, with `target` as its primary key and the same
canonical serialization as its tie-breaker. Map keys are serialized in
lexicographic order. Identical manifests therefore produce identical ordered
projections regardless of declaration order or IR iteration order, and
quantified-action results use that same order.

Because the result views are additive-only, this projection introduces no
compatibility burden on existing tests, and internal IR types remain unexposed.

### 2. Quantified assertions

Two assertion forms join the `then` vocabulary, both evaluated by the existing
MiniJinja engine with no new expression semantics:

- `for_all_actions`: evaluates each listed expression once per entry in
  `result.actions`, binding `action`.
- `for_all_targets`: the analogue over the graph view, binding `target`.

A failing quantified assertion reports the binding that falsified it, alongside
the substituted actual values, using the FAIL versus ERROR taxonomy of UX
design §11.3 unchanged. Environment data is redacted at every external
boundary: environment keys are replaced with stable opaque key tokens, and
environment values are replaced with the fixed `<redacted>` marker. The
redactor applies to `result.actions.env`, substituted values in failure
reports, and persisted regression artefacts; assertions still compare the
constructed environment semantically, so redaction does not change whether a
case is a FAIL or an ERROR.

### 3. The `forall` block

A test case may declare a `forall` mapping from binding names to domain
constructors. The runner expands the domains into a finite case family and
evaluates the case's `steps` once per drawn tuple; drawn values are available
wherever `let` bindings are (UX design §7).

For screen readers: the following YAML example declares a property test that
draws a target name and a flag ordering, builds the graph, and asserts
environment closure and source uniqueness over every generated action.

```yaml
netsuke_test_version: "1.1"

test_command_env_is_closed_over_declared_vars:
  description: Generated invocations never leak ambient environment.
  forall:
    target_name: { path_fragment: { max_len: 12 } }
    flag_order: { permutations_of: ["-O2", "-g", "-Wall"] }
  steps:
    - given:
        let:
          declared_env: []
          name: "{{ target_name }}"
          flags: "{{ flag_order }}"
    - when: build_graph
    - then:
        for_all_actions:
          - "action.env | keys | reject('in', declared_env) | length == 0"
          - "action.inputs | unique | length == action.inputs | length"
```

The domain vocabulary is closed, mirroring the mock matcher vocabulary's
parse-time compilation (technical design §7). The following table lists the
initial constructors.

| Constructor       | Draws                               | Expansion      |
| ----------------- | ----------------------------------- | -------------- |
| `one_of`          | One member of a finite set.         | Exhaustive     |
| `permutations_of` | One ordering of the listed items.   | Exhaustive[^1] |
| `int`             | An integer from an inclusive range. | Exhaustive[^1] |
| `path_fragment`   | A sandbox-safe relative path token. | Sampled        |
| `list_of`         | A bounded list over a child domain. | Sampled        |

_Table 2: Initial `forall` domain constructors._

Expansion is deterministic. When the Cartesian product of the declared domains
is at most the expansion ceiling, the family is enumerated exhaustively. Above
the ceiling, the runner samples with a pseudo-random number generator seeded
with the fixed default seed `0`; the seed appears verbatim in every failure
report. `netsuke test --seed <n>` selects the reported seed, but replays the
reported case only when the generated tuple inputs persisted with that report
are also available. A seed without the tuple inputs is insufficient for replay.
This mirrors the `derandomize=True` and pinned-`@example` convention of the
workflow contract suites and the `proptest-regressions/` replay convention.

On failure, the runner delta-reduces the drawn tuple towards domain minima and
reports the smallest falsifying binding set it finds, then persists the seed
and tuple in a regression file under the test tree so subsequent runs replay it
first.

### 4. Built-in metamorphic relations

The highest-value properties of a build-script generator relate two runs of the
pipeline rather than inspecting one output. These need no user-authored
predicates, so they ship as a closed `mutations` vocabulary: the runner derives
a mutated manifest, runs the plan-mode pipeline on both, and asserts the named
relation.

The following table lists the initial mutations and their relations; they
correspond one-to-one with the `missing`, `duplicate`, and `misordered`
mutation strategy proven in `tests/workflow_contracts/`.

| Mutation                   | Relation asserted                          |
| -------------------------- | ------------------------------------------ |
| `permute_declarations`     | Actions and Ninja output are identical.    |
| `insert_irrelevant_target` | Pre-existing actions are unchanged.        |
| `rename_target`            | Actions are isomorphic under the renaming. |
| `remove_declared_input`    | The declared `expect_failure` is raised.   |

_Table 3: Built-in metamorphic mutations and their asserted relations._

For screen readers: the following YAML example asserts that a manifest's
generated build script is invariant under declaration reordering and locally
unaffected by unrelated additions.

```yaml
netsuke_test_version: "1.1"

test_generation_is_order_invariant:
  description: Declaration order never changes the generated script.
  subject:
    manifest: fixtures/webapp/Netsukefile
  mutations:
    - permute_declarations
    - insert_irrelevant_target
```

Mutations compose with `forall`: each drawn tuple runs the base and mutated
pipelines. Both runs execute inside the same killable child-case architecture
of technical design §3, so a divergent or non-terminating mutant cannot wedge
the runner.

### 5. Engine and determinism

Internally the runner reuses `proptest` — already a dependency of Netsuke's own
verification obligations (technical design §12) — for its random number
generator and regression-file conventions rather than inventing new machinery.
No `proptest` type or trait appears in the dialect or the public library
surface.

All generation obeys the framework's determinism invariants: no ambient clock,
no ambient environment, and no network (technical design §1). The default run
is fully deterministic; randomness enters only through an explicit `--seed`
override, and every report names the seed in effect.

### 6. Coverage completeness lint

Following the completeness-check convention of
`tests/makefile_test_target/rustdocflags.rs`, `netsuke test` gains an opt-in
warning: when a rule interpolates a command or constructs an environment and no
test in the discovered suite covers one of its actions with a quantified or
metamorphic assertion, the run reports the uncovered rule. New invocation
shapes therefore opt into the property contract deliberately instead of
drifting past it. The lint is advisory by default and promoted to a failure with
`--strict-coverage`.

## Requirements

### Functional requirements

- `result.actions` exposes every build edge's rule, tokenized argv,
  environment map, inputs, outputs, and pool, depfile, dyndep, and
  working-directory bindings where present.
- `for_all_actions` and `for_all_targets` evaluate MiniJinja expressions
  per entry and report the falsifying binding on failure.
- `forall` accepts the closed domain vocabulary of Table 2, expands
  exhaustively at or below the ceiling, and samples deterministically above it.
- Failure reports name the seed and the minimized drawn tuple;
  `netsuke test --seed` replays a reported failure exactly.
- Regression tuples persist under the test tree and replay before fresh
  generation.
- The `mutations` vocabulary of Table 3 derives the mutated manifest,
  runs both pipelines in the case sandbox, and asserts the named relation.
- The coverage lint reports rules with uncovered invocations and fails
  the run only under `--strict-coverage`.

### Technical requirements

- The actions projection pins the recipe shell via
  `from_manifest_for_shell`; identical manifests project identically on all
  supported hosts.
- The dialect remains closed: no user-defined predicate functions,
  callbacks, or scripting enter the schema.
- Properties run entirely in plan mode; no build execution, network, or
  ambient environment access occurs (invariant I5 of technical design §12).
- Generation is bounded: the expansion ceiling, sample count, and
  per-case deadline are enforced limits, consistent with the small-bounds
  convention of [ADR-004](../adr-004-bound-kani-ir-harnesses-to-small-n.md).
- `netsuke_test_version` gains a minor increment (1.1); files using
  `forall`, `mutations`, or `result.actions` declare it, and 1.0 runners reject
  them under RFC 0007's version contract.

## Compatibility and migration

The extension is additive. Existing 1.0 test files parse and run unchanged; the
new view, blocks, and flags appear only when a file declares
`netsuke_test_version: "1.1"`. Because RFC 0007's version contract accepts a
minor at most the supported minor, a 1.0 runner fails closed on 1.1 files with
its standard unsupported-version diagnostic rather than misparsing them.

The only build-path change is the shell-pinning constructor, which the
technical design already plans as a seam; the host-default path is untouched.
The actions projection reads the IR that `build_graph` already produces and
adds no new pipeline stage.

Delivery sequences after the RFC 0007 result views land: the actions view
extends the projection layer of phase 7.5, generation and mutations build on
the case supervisor of phase 7.5.2, and the coverage lint arrives last. The
roadmap records the phasing.

## Alternatives considered

### Option A: expose a general property-testing DSL

Embedding user-authored predicates, strategies, or shrinkers in the YAML
dialect (or bridging to `proptest` or Hypothesis syntax) would maximize
expressive power. It was rejected because the dialect is deliberately
declarative and closed (UX design §§16-17); an escape hatch would reintroduce
the scripting surface RFC 0007 explicitly rejected, and its power is not needed
for the structural invariants at stake.

### Option B: assert properties over raw Ninja text

Using the existing `result.ninja` view with generated inputs would avoid a new
projection. It was rejected because recipe text is host-shell-sensitive
(technical design §9), so string-level properties are either flaky across hosts
or weakened to the point of vacuity, and environment construction is invisible
in the Ninja text entirely.

### Option C: keep property testing in Netsuke's own test suites

Netsuke's Rust and Python suites could grow generator properties without any
dialect change. It was rejected as the sole approach because it serves the
wrong audience: Netsukefile authors cannot express invariants about their own
manifests there. It remains the right home for properties of Netsuke's
internals, and this RFC changes nothing about those suites.

### Option D: unbounded manifest fuzzing

A coverage-guided fuzzer over manifest syntax would find deeper crashes. It was
rejected for this dialect because it is nondeterministic by design, sized for
continuous fuzzing infrastructure rather than a test command that must be
deterministic, bounded, and CI-friendly, and aimed at Netsuke's parser rather
than at the author's own build script. Hostile-input fuzzing of Netsuke itself
is tracked separately by [RFC 0008](0008-code-health.md).

## Open questions

- What is the right default expansion ceiling and sample count? The
  workflow contract suites bound Hypothesis at 24 examples; the fixture
  teardown property bounds graphs at 16 nodes. A default in that range seems
  right, but the choice should follow measurement of realistic suites.
- Should the `result.actions` view land inside RFC 0007's phase 7.5.4
  result-view task rather than waiting for this extension? Landing it early
  would let example-based tests use it immediately.
- Does the environment projection need redaction alignment with
  [ADR-009](../adr-009-bounded-redacted-manifest-telemetry.md) when failure
  reports print constructed environments?
- Is delta reduction over the drawn tuple sufficient minimization in
  practice, or do sampled list domains need element-wise shrinking?

## Recommendation

Adopt the extension as specified: the pinned-shell `result.actions` projection
first, then declarative `forall` generation with seeded deterministic replay,
then the metamorphic `mutations` vocabulary, and finally the coverage lint. The
design adds the missing observability and quantification with no new expression
engine, no scripting surface, and no departure from the plan/apply split, and
it transplants a property-testing idiom the repository has already proven on
its Makefile and workflow artefacts.

______________________________________________________________________

[^1]: Exhaustive at or below the expansion ceiling; sampled above it.
