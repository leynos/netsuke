# Netsuke test framework UX and semantic design

## Front matter

- **Status:** Draft.
- **Scope:** The user-facing surface of the Netsukefile testing framework: the
  test tree, the YAML test dialect, the `given`/`when`/`then` semantics, the
  mocking model, fixtures, the `netsuke test` command, and reporting. The
  implementation architecture lives in the companion
  [technical design](netsuke-test-framework-technical-design.md).
- **Primary audience:** Netsukefile authors writing tests, and reviewers
  evaluating the test dialect before implementation.
- **Governing documents:**
  [RFC 0007](rfcs/0007-netsukefile-testing-framework.md) proposes and
  positions this feature;
  [netsuke-design.md](netsuke-design.md) defines the manifest language and
  compiler pipeline the tests exercise;
  [ADR-003](adr-003-actions-foreach-when-scope.md) governs manifest control
  keys; [ADR-008](adr-008-environment-seam-taxonomy.md) governs environment
  injection. Where this document conflicts with an accepted ADR, the ADR
  takes precedence.

## 1. Problem statement

Netsuke compiles a YAML-plus-Jinja manifest into a typed abstract syntax tree
(AST), an intermediate representation (IR) build graph, and a Ninja file. A
Netsukefile of any sophistication contains real logic: `foreach` expansion,
`when` conditions, macros, environment probes, globbing, and command
availability branches. Today the only way to check that logic is to run the
build and inspect the result by hand, which is slow, environment-dependent,
and impossible to automate for the negative cases (a target that must _not_
be generated, a manifest that must fail with a specific diagnostic).

The framework described here gives Netsukefile authors a first-class
`netsuke test` command and a YAML test dialect. Tests evaluate manifests
through the same compiler pipeline as `netsuke build`, with declared mocks
substituted at named seams, and assert against structured results: the
rendered manifest, the IR graph, the generated Ninja text, and recorded mock
calls. Tests are deterministic by default: no network, no wall-clock
dependency, no mutation of the project root, and no leaked mocks.

### 1.1. Design intent

Three commitments shape every decision below:

- **Same compiler, declared substitutions.** Tests run the real loader, AST,
  IR builder, and Ninja generator. Mocks are declarative substitutions at
  named seams, never a re-implementation of manifest semantics. This is the
  lesson of Act, whose emulation of GitHub Actions drifts permanently from
  the real service;[^1] Netsuke avoids the problem structurally because the
  test runner and the build share one implementation.
- **Plan-mode is the unit-testing story.** OpenTofu and Terraform's test
  framework distinguishes `command = plan` (validate logic, create nothing)
  from `command = apply`.[^2] Netsuke adopts the same split: the default
  test phases stop at generated Ninja text and never execute build
  commands. Execution is a deferred, explicitly gated escalation.
- **Failure output is a feature.** Open Policy Agent (OPA) prints the
  failing expression with the actual value of every variable
  substituted;[^3] shellmock prints the configuration stanza that would
  have matched an unexpected call.[^4] Netsuke test reports adopt both
  behaviours.

## 2. Glossary

- **Test file:** a YAML file in the test tree containing one or more test
  cases and optional shared declarations.
- **Test case:** a named entry whose key starts with `test_`, holding a
  description, tags, and an ordered list of steps.
- **Step:** one `given`/`when`/`then` group within a test case. Each step
  contains at least one of the three sections.
- **Subject manifest:** the Netsukefile a test case evaluates.
- **Fixture:** a named lifecycle object with setup actions, exported values,
  and teardown actions, requested by name from a step's `given`.
- **Double:** a declared substitution for a callable in the subject
  manifest's template environment. Doubles come in three kinds: _stub_
  (canned answer, never verified), _mock_ (canned answer, verified against
  declared expectations), and _spy_ (records calls and passes through to
  the real implementation).
- **Journal:** the chronological record of every call made to any double
  during a test case.
- **Support file:** a test-tree file whose name starts with `_`. It may
  declare `vars`, `macros`, and `fixtures` for import but contains no test
  cases.

## 3. Test tree and discovery

### 3.1. Location and configuration

Tests live in a `netsuke-tests` directory beside the Netsukefile by default.
An optional top-level `tests` block in the Netsukefile reconfigures
discovery:

```yaml
netsuke_version: "1.2.0"

tests:
  root: netsuke-tests
  include:
    - "**/*.yml"
    - "**/*.yaml"
  exclude:
    - "**/_*.yml"
    - "**/_*.yaml"
```

The values above are also the defaults when the block is absent. The `tests`
block is tool configuration, not build data: its values are not visible to
manifest templates, and `tests.root` accepts no Jinja, because discovery must
work before the subject manifest's template environment exists.

Files matching an `exclude` pattern that start with `_` are support files
(§2); they are loaded only when imported. Any other excluded file is ignored
entirely.

### 3.2. Empty runs fail

A `netsuke test` invocation that discovers no test files, or whose filters
select zero cases, exits with the usage-error code rather than reporting
success. OPA added `--fail-on-empty` after silently green pipelines shipped
with a glob that matched nothing;[^3] Netsuke makes that behaviour the
default and provides `--allow-empty` for intentionally empty suites.

## 4. Test file structure

A test file is a YAML mapping with a fixed set of known keys plus dynamic
`test_*` keys:

```yaml
netsuke_test_version: "1.0"   # required in every test file

imports:                       # optional: support files, paths relative to
  - _fixtures.yml              # this file, confined to the test tree

vars: {}                       # optional suite-local test variables

macros: []                     # optional, same shape as Netsukefile macros

fixtures: {}                   # optional fixture definitions (§9)

test_compile_target:           # one or more test cases
  ...
```

Macros are declared exactly as in a Netsukefile — `signature` and `body` —
so authors carry one mental model between manifests and tests.

`netsuke_test_version` is a `MAJOR.MINOR` string, not a full semantic
version. The runner accepts a file when the major version matches a
supported major and the minor version is at most the supported minor;
anything else is rejected with a located diagnostic and the usage-error
exit code, though `--list` still enumerates such files. Because the
dialect denies unknown keys everywhere, every addition — a new matcher, a
new assertion form — is a minor-version event, and older runners reject
newer files by design rather than misreading them. Breaking changes to
existing semantics require a major-version bump.

Unknown top-level keys that do not start with `test_` are rejected with a
diagnostic naming the nearest known key. This catches `fixture:` for
`fixtures:` at parse time rather than as a silently ignored block. The same
rule applies inside every nested structure: the test dialect has no
open-ended maps except where values are explicitly user-named (`vars`,
`fixtures`, `test_*`, `let`, mock names).

## 5. Test cases and steps

The smallest useful test needs no fixtures, no doubles, and no `given` at
all:

```yaml
netsuke_test_version: "1.0"

test_manifest_compiles:
  steps:
    - when: generate_ninja
      then:
        - result.ok
        - result.graph.has_target("build/main.o")
```

A fuller case adds a description, tags, and context:

```yaml
test_generates_object_targets:
  description: foreach expands one object target per discovered source.
  tags: [manifest, foreach]
  subject:
    manifest: ../Netsukefile     # optional; defaults per §10
  steps:
    - given:
        fixtures: [tiny_c_project]
        let:
          glob: mock(args=["src/*.c"], returns=["src/main.c"])
      when: generate_ninja
      then:
        - result.ok
        - contains(result.ninja, "build build/main.o:")
```

Rules:

- `steps` must contain at least one item.
- Each step must contain at least one of `given`, `when`, or `then`.
- Steps execute in order. Context established by `given` persists across
  later steps of the same case; nothing persists between cases.
- `description` is optional but reported when present; `tags` drive the
  `--tag`/`--skip-tag` filters.
- An optional case-level `timeout` (seconds) overrides the run-level
  per-case timeout (§12).

Test case names must match `test_[A-Za-z0-9_]+` and are reported as
`<file>::<name>`, following the naming-convention discovery that every
surveyed framework converged on (`*.tftest.hcl`, `*_test.go`, `test_`
rules).[^2][^3]

## 6. Expression semantics

Test expressions use MiniJinja expression syntax — the same engine that
evaluates manifest `when` conditions — so authors reuse the manifest
language's filters and functions.

The dialect distinguishes _expression fields_ from _template fields_:

- **Expression fields** (`let` values, scalar `then` entries, assertion
  operands) are bare expressions. `sources | length` is valid;
  `"{{ build_dir }}/main.o"` is rejected with a diagnostic explaining the
  distinction.
- **Template fields** (fixture file contents, paths in filesystem actions)
  are Jinja templates in which `{{ ... }}` interpolation is expected.

This split prevents the two-Jinja-dialects-in-one-file ambiguity: a field is
always one or the other, never context-dependent. The complete
classification:

| Field | Class |
| --- | --- |
| `let` values | expression |
| scalar `then` entries; `equals.actual`, `contains.value`, `matches.value` | expression |
| `equals.expected`, `contains.needle`, `matches.regex` | literal |
| `env.set` values, `clock.now`, mock `returns`/`raises` values | literal |
| fixture `setup`/`teardown` paths and `write.text` | template |
| fixture `exports` values | template |
| `given.fs` paths and contents | template |
| action arguments (for example `manifest:`) | template |
| `subject.manifest` | template |

_Table 1: Field classification. Literal fields are plain YAML values with
no evaluation of either kind._

Scope rule: `let` bindings are visible only to test expressions. A binding
whose value is a double declaration additionally installs that name into
the subject manifest's template environment; a plain binding never is. The
names `mock`, `stub`, `spy`, and `substitute` are reserved callables in
expression fields.

`let` bindings evaluate in document order and may reference earlier bindings
and fixture exports:

```yaml
given:
  fixtures: [tiny_c_project]
  let:
    manifest_path: fixtures.tiny_c_project.manifest
    source_count: 2
    label: "'objects: ' ~ source_count"
```

## 7. `given` semantics

`given` prepares the hermetic context for the step. It never invokes the
compiler pipeline. Supported sections, applied in this order:

1. `fixtures` — resolve and set up requested fixtures (§9).
2. `env` — set and unset environment values seen by the subject manifest's
   `env()` function. The host process environment is never mutated; values
   flow through Netsuke's injected environment reader. Fixture `env`
   actions contribute to the same case-level map first; `given.env` wins on
   conflict.
3. `fs` — structured filesystem operations (`mkdir`, `write`, `copy`,
   `remove`) inside the test sandbox.
4. `let` — evaluate bindings in document order (§6). A binding whose value
   is a `mock(...)`, `stub(...)`, `spy(...)`, or `substitute(...)` call
   declares a double (§8) rather than a plain value.
5. `mocks` — the structured block form of double declarations (§8.2).
6. `clock` — fix the value returned by the stdlib `now()` function.
7. `subject` — override the subject manifest for this and later steps.

```yaml
given:
  env:
    set:
      CC: clang
    unset:
      - RUSTFLAGS
  clock:
    now: "2026-06-08T12:00:00Z"
  let:
    glob: mock(args=["src/*.c"], returns=["src/main.c"])
```

Network access is denied in tests regardless of `given`: an unmocked
`fetch()` call fails the action with a diagnostic explaining how to declare
a stub for the URL. Determinism is the default; ambient reality is opt-in
per seam.

## 8. The mocking model

### 8.1. Doubles: stub, mock, spy

The dialect names its three kinds of double explicitly, following the
taxonomy shared by flexmock and cmd-mox:[^5][^6]

- `stub(...)` — returns a canned value; calls are journalled but never
  verified. Unmatched calls on a stub return the declared `default`, or
  MiniJinja `Undefined` when none is declared.
- `mock(...)` — returns a canned value and is verified: every declared
  expectation must be satisfied by the end of the case, and any call that
  matches no declared expectation fails the action immediately.
- `spy(...)` — journals every call and passes through to the _effective_
  implementation under test configuration: the sandbox-rooted `glob` and
  file tests, the fixed clock for `now()`, the real manifest macro for a
  substituted name. Spying `fetch` is a suite error because the effective
  implementation under the deny-all network policy can only fail; declare
  a stub or mock instead.

Shorthand forms in `let` desugar to the structured form:

```yaml
let:
  glob: mock(args=["src/*.c"], returns=["src/main.c"])
  cc:   stub(returns="clang")
  now:  spy()
```

The strictness ladder is deliberate: a loose stub is the one-line default
for don't-care collaborators, and full expectation machinery is graduated
opt-in. Mockito's community documented over-mocking as the primary failure
mode of expressive mock frameworks;[^7] the dialect keeps the terse form
tersest.

### 8.2. Structured declarations and call configurations

The structured `mocks` block is the full-fidelity form:

```yaml
given:
  mocks:
    fetch:
      kind: mock
      calls:
        - args: ["https://example.test/toolchain.json"]
          returns: '{"compiler": "clang"}'
        - args: [{ starts_with: "https://mirror." }]
          returns: '{"compiler": "gcc"}'
          times: 2
        - raises:
            message: unexpected fetch
    cc_name:
      kind: stub
      default: clang
```

Declaring the same double name twice in one case — in `let` shorthand and
the `mocks` block, or across steps — is a suite error, not a merge.

Semantics:

- `calls` is a first-match-wins configuration list: each incoming call takes
  the first entry whose matchers accept it, so specific entries precede
  catch-all entries. This is shellmock's model,[^4] and it is more robust than
  positional call scripts ("the nth call must be exactly this"), the
  record-replay rigidity that made pymox-style tests brittle.[^8]
- Matching is **unordered by default**. `ordered: true` on a double opts its
  entries into declaration-order matching. Every surveyed library that
  enforced global ordering by default is remembered for brittle tests;
  every modern one makes ordering opt-in.[^5][^7]
- `times: N` is a maximum, not a quota. An entry may match up to N calls.
  Once those are spent, the next call that would have matched it falls
  through to later entries instead, and fails dispatch if none accepts.
  Fewer than N matches — including none — is not itself a failure, so
  `times` never doubles as a minimum-call assertion. Entries without `times`
  match any number of calls. To require that a call happened, assert on the
  journal (`mocks.<name>.call_count`) or use a `mock`, whose declared
  entries must all be satisfied by end of case.
- `returns` supplies a YAML value returned as the MiniJinja value;
  `raises` supplies a structured template error instead.

### 8.3. Argument matchers

Arguments match by exact equality unless a matcher object is used. The
matcher vocabulary is closed:

| Matcher | Meaning |
| --- | --- |
| `eq: <value>` | accepts exactly this value (the literal escape hatch) |
| `any: true` | accepts any value |
| `is_a: string` | accepts values of the named type |
| `regex: "^src/"` | accepts strings matching the pattern |
| `contains: ".c"` | accepts strings or lists containing the needle |
| `starts_with: "src/"` | accepts strings with the prefix |
| `not: <matcher>` | negates the wrapped matcher |

_Table 2: Argument matcher vocabulary._

A bare argument matches by exact equality; `eq:` exists so a literal
one-key mapping that happens to spell a matcher name can still be matched.
Equality is structural over the YAML-to-template value conversion:
integers and floats compare numerically, strings never equal numbers, and
sequences and mappings compare element-wise. `is_a` accepts exactly
`string`, `integer`, `float`, `number`, `boolean`, `list`, `map`, and
`none`.

Predicate functions and dynamic response handlers are deliberately absent:
they are imperative logic and do not survive translation into a data
dialect. A case that needs computed behaviour should restructure, or wait
for the deferred fixture-script escape hatch.

### 8.4. Verification and the journal

At the end of each test case the runner verifies:

- every `mock` expectation was satisfied (unmet expectations fail the
  case), and
- every declared double was used at least once. An unused double is
  reported as an _unnecessary double_ warning — Mockito's
  `UnnecessaryStubbingException` insight, softened to a warning with
  `lenient: true` as the per-double opt-out.[^7]

Every call to every double lands in the case's journal, addressed as
`mocks.<name>` for every kind — the colloquial name is kept because it is
what authors reach for, and inventing a `doubles.` namespace would trade
familiarity for taxonomy. The journal is bounded: a double whose call
count exceeds the per-double ceiling (default 10,000) turns the case into
an error naming the runaway, rather than exhausting memory. Assertions
read the journal:

```yaml
then:
  - mocks.glob.call_count == 1
  - mocks.glob.calls[0].args == ["src/*.c"]
```

When a strict mock receives an unmatched call, the failure report prints
the observed call and the YAML entry that would have accepted it:

```plaintext
FAIL compile.yml::test_compile_target
  mock 'fetch' received an unmatched call:
    fetch("https://example.test/versions.json")
  no configured entry matched. A matching entry would be:
    - args: ["https://example.test/versions.json"]
      returns: <value>
```

### 8.5. Macro substitution

`substitute("name")` swaps a manifest macro (or installs a new callable)
with a stand-in macro declared in the test file:

```yaml
macros:
  - signature: "stand_in_compile(src, obj)"
    body: |
      STUB {{ src }} -> {{ obj }}

test_compile_uses_macro:
  steps:
    - given:
        let:
          compile_cmd: substitute("stand_in_compile")
      when: generate_ninja
      then:
        - contains(result.ninja, "STUB")
        - substitutes.compile_cmd.call_count == 1
```

The stand-in must exist in the test file or an imported support file.
Substituted macros journal their calls under `substitutes.<name>`.
Substitution scope is one test case.

### 8.6. What can be mocked

| Seam | Mechanism | Example |
| --- | --- | --- |
| Template functions | `mock`/`stub`/`spy` double | `glob`, `which`, `fetch`, `command_available` |
| Environment variables | `given.env` | `env("CC")` |
| Clock | `given.clock` | `now()` |
| Manifest macros | `substitute(...)` | `compile_cmd(...)` |
| Filesystem observations | fixtures and `given.fs` | file tests, `glob` against real sandbox files |

_Table 3: Mockable seams and their mechanisms._

Filters and Jinja tests (`"clang" | which`, `path is file`) are not
mockable in the first version; filesystem fixtures cover most file-test
cases with real files, which is both simpler and higher fidelity.

## 9. Fixtures

Fixtures are lifecycle objects: ordered setup actions, exported values, and
ordered teardown actions.

```yaml
fixtures:
  tiny_c_project:
    description: Minimal C project with one source and a Netsukefile.
    setup:
      - tmpdir: project
      - mkdir: "{{ project }}/src"
      - write:
          path: "{{ project }}/src/main.c"
          text: |
            int main(void) { return 0; }
      - write:
          path: "{{ project }}/Netsukefile"
          text: |
            netsuke_version: "1.2.0"
            targets:
              - name: build/main.o
                command: "cc -c src/main.c -o build/main.o"
                sources: src/main.c
    exports:
      root: "{{ project }}"
      manifest: "{{ project }}/Netsukefile"
```

Fields: `description`, `uses` (fixture dependencies), `params` (defaults,
overridable at request time), `setup`, `exports`, `teardown`. The action
vocabulary is `tmpdir`, `mkdir`, `write`, `copy`, `remove`, and `env`. All
paths resolve inside the per-case sandbox; a fixture cannot touch the
project root or the host filesystem.

Lifecycle guarantees:

- Fixtures set up in dependency order; each at most once per case.
- Teardown runs in reverse setup order, for every fixture whose setup
  completed, regardless of later setup failures, action failures, or
  assertion failures.
- A failing teardown never masks the case result: the remaining stack
  still unwinds, every teardown error is aggregated into the report, the
  case is marked errored, and its sandbox is retained as if `--keep` had
  been passed.
- All fixtures are case-scoped. File- and session-scoped fixtures are
  deferred until the isolation model has tests of its own; Terraform's
  shared-state `state_key` sharp edges and Molecule's driver sprawl both
  counsel starting with the trivially safe default.[^2][^9]

An arbitrary-command `run` action is deferred with execution generally
(§10). `--keep` preserves the sandbox of failing cases for inspection,
mirroring Molecule's `--destroy=never` escape hatch.[^9]

## 10. `when` semantics

`when` invokes named pipeline actions against the subject manifest. Scalar,
list-of-scalars, and object forms are accepted:

```yaml
when: generate_ninja

when:
  - load_manifest
  - build_graph

when:
  - generate_ninja:
      manifest: "{{ fixtures.tiny_c_project.manifest }}"
```

Actions, in pipeline order:

| Action | Runs | Result carries |
| --- | --- | --- |
| `load_manifest` | ingest, parse, expand, deserialize, render | `result.manifest` |
| `build_graph` | `load_manifest` + IR lowering | `result.graph` |
| `generate_ninja` | `build_graph` + Ninja generation | `result.ninja` |

_Table 4: Pipeline actions._

Later actions imply the earlier stages, so most cases write exactly one
`when`. Within a step, the pipeline runs once: each action in the list
extends the previous action's artefacts rather than re-running the loader,
so a `when` of all three actions evaluates the manifest's templates
exactly once and journal counts are independent of how many actions name
the stages. Each action replaces `result` and appends to `results`, so a
multi-action step can compare stages. An action in a _later step_ starts a
fresh pipeline pass, and its template evaluations journal again.

The subject manifest resolves in precedence order: the action's `manifest`
argument, the step's `given.subject`, the case's `subject`, then the
Netsukefile of the enclosing project. A subject manifest's own `tests`
block is inert during test execution: discovery is driven solely by the
project whose `netsuke test` invocation is running, and the runner never
recurses.

Every subject path an author writes is confined. Two roots are approved:
the per-case sandbox, which holds fixture-created manifests, and the
enclosing project's own Netsukefile, a read-only exception so that
testing the manifest beside the test tree keeps working. Paths are
validated after template evaluation and before the manifest is opened,
because the value only exists once its interpolation has run. The rules
are the same for all three author-supplied sources — the action's
`manifest` argument, `given.subject`, and the case's `subject`:

- Absolute paths are rejected, so naming `/etc/Netsukefile` fails with a
  located diagnostic rather than reading it.
- Relative paths resolve against the case sandbox and must stay inside
  it. `../../outside/Netsukefile` is rejected, as is a path whose
  existing components resolve through a symlink leaving the root.
- Ordinary fixture paths keep working because a fixture writes its
  manifest inside the sandbox and exports a path within it.
- The enclosing project's Netsukefile is readable without being writable.
  Approving it as a subject grants read access only; nothing in a test can
  write to the project root.

Confinement is what makes the sandbox guarantee true of the subject
manifest as well as of fixture files. Without it a test could read any
file the invoking user can read, which is wider than testing a build
manifest requires.

Build execution (`execute`) is designed but deferred: the command surface
reserves `--allow-execute`, and no action in the first version spawns Ninja
or any build command. Fidelity note, following Act's practice of publishing
its gaps:[^1] until `execute` ships, `netsuke test` validates everything up
to and including the generated Ninja text, and nothing about the behaviour
of the commands within it.

## 11. `then` semantics

### 11.1. Assertion forms

Scalar entries are MiniJinja boolean expressions; object entries are
structured assertions with richer diffs:

```yaml
then:
  - result.ok
  - result.graph.targets | length == 3
  - equals:
      actual: result.graph.edge_count
      expected: 4
  - contains:
      value: result.ninja
      needle: "default app"
  - matches:
      value: result.error.message
      regex: "missing.*rule"
```

Assertion helper functions available in expressions: `contains`,
`starts_with`, `ends_with`, `matches`, `file_exists`, `file_contains`.

### 11.2. Result model

```yaml
result:
  action: generate_ninja
  ok: true
  manifest: <manifest view>
  graph: <graph view>
  ninja: "<generated text>"
  error:
    code: null
    message: null
```

`result.graph` is a stable, additive-only assertion surface, deliberately
distinct from any internal graph type. Its fields: `targets` (a sorted
list of target views, each with `name`, `sources`, `deps`,
`order_only_deps`, `dependency_order`, `phony`, and `description`),
`rules` (sorted rule names), `default_targets`, and `edge_count`.
`description` is the target's discovery metadata — the text
`netsuke help targets` renders — not the
rule description Ninja reports during execution; a test asserting on
progress output must read the rule instead. `dependency_order` is
`parallel` or `serial`, so a manifest that serializes its dependencies
can be asserted on directly rather than by pattern-matching the generated
Ninja. Its helper methods:
`has_target`, `has_rule`, `has_edge`, and `target(name)`. Fields and
helpers are never removed or repurposed within a dialect major version. The
helpers exist so authors never parse raw Ninja for structural questions:

```yaml
then:
  - result.graph.has_target("build/main.o")
  - result.graph.has_rule("compile")
  - result.graph.target("build/main.o").sources == ["src/main.c"]
```

### 11.3. Failure taxonomy: FAIL versus ERROR

An assertion that evaluates to false is a **failure**. An assertion whose
evaluation raises — an undefined name, a type mismatch — is an **error**,
reported distinctly, following OPA's FAIL/ERROR/SKIP taxonomy.[^3] A typo'd
`result.grpah` must not masquerade as an ordinary red assertion.

On failure, the report prints the expression with the actual value of each
referenced name substituted:

```plaintext
  then[1]: result.graph.targets | length == 3
    result.graph.targets | length = 2
```

### 11.4. Expecting failure

Negative tests name the diagnostic they expect rather than asserting bare
failure, so a different bug cannot satisfy the test:

```yaml
when: load_manifest
then:
  - expect_failure:
      code: netsuke::manifest::parse
      message_contains: "unknown field"
```

`expect_failure` passes when the preceding action failed with a diagnostic
matching every provided field, and fails when the action succeeded or
failed differently. `code` matches exactly; `message_contains` is a
case-sensitive substring test. Diagnostic codes become public contract the
moment a test matches on them, so `code` matching ships only once the
diagnostic-stack migration settles and the code namespace is declared
stable; until then, `message_contains` carries negative tests. Terraform's
`expect_failures` confusion — expected failures interacting badly with
phase defaults — is avoided by attaching the expectation to an explicit
`then` after an explicit `when`.[^2]

## 12. The `netsuke test` command

```plaintext
netsuke test [FILTER...]

Arguments:
  FILTER                   Case selectors: file paths, file::case names, or
                           substring patterns

Options:
  --tests-dir <DIR>        Override tests.root
  --list                   List discovered cases without running them
  --tag <TAG>              Run only cases with this tag (repeatable)
  --skip-tag <TAG>         Exclude cases with this tag (repeatable)
  --fail-fast              Stop after the first failing case
  --timeout <SECS>         Per-case wall-clock budget (default 60)
  --keep                   Preserve sandboxes of failing cases
  --allow-empty            Succeed when zero cases are selected
```

`--json` and `--jobs` are the existing global flags, not new per-command
options; `test` consumes them with their established semantics.

Under `--jobs > 1`, `--fail-fast` stops scheduling rather than cancelling
work in flight: cases already running continue to completion, so their
fixture teardown and journal handling are unaffected by the stop. No new
case is started once a failure is observed. Cases that never start are
reported as `skipped`. A suite with more selected cases than `--jobs`
therefore exercises both halves of this rule in one run: the cases already
dispatched finish normally, and the remainder are skipped rather than run.
Both the human summary and the JSON `summary` counts (§13) therefore add up
to the full selected-case count, not the number executed: every selected
case appears exactly once, whether run or skipped.

A case that exceeds its timeout is reported as errored, with whatever
journal it produced attached, its fixtures torn down, and its sandbox
retained for inspection. The deadline is absolute: each case runs in its
own child process, so a case that stops cooperating — a runaway template
expression that never yields — is terminated rather than waited on. A
pathological manifest cannot hang the run. This requires a platform that
supports spawning and terminating child processes; Linux and Windows are
covered by continuous integration.

Exit codes:

| Code | Meaning |
| --- | --- |
| 0 | all selected cases passed |
| 1 | at least one case failed or errored |
| 2 | invalid suite, invalid selector, or zero cases without `--allow-empty` |
| 3 | internal runner error |
| 130 | interrupted |

_Table 5: `netsuke test` exit codes._

The command follows the established display policy flags (`--color`,
`--emoji`, `--progress`, `--accessibility`) and the stream-purity contract,
which turns on whether the _run_ completed rather than whether every case
passed. A completed run — all passed, some failed or errored, or
interrupted — writes exactly one JSON document to stdout and nothing to
stderr; failing cases are reported inside that document because they are
what automation reads. A command failure that produces no report (invalid
suite or selector, an empty selection without `--allow-empty`, or an
internal runner error) instead leaves stdout empty and writes one
diagnostic document to stderr. All human-facing strings are localized like
the rest of the command-line interface (CLI) surface.

Interruption (Ctrl-C) stops scheduling, then terminates every case still
running and waits for each child to exit. With no child still alive, the
parent performs the cleanup those children can no longer do themselves,
reaps every child to collect its status, applies the usual `--keep`
decision to the run sandbox — removing it, or retaining it and printing
the path — and exits 130. In `--json` mode the run still emits exactly one
document, marked `"interrupted": true`. Every selected case still appears
exactly once: a case the parent terminated mid-run is errored, carrying an
interruption diagnostic and whatever journal it had produced, and a case
that never started is skipped. Waiting for exit before cleanup matters on
Windows, where a
still-terminating child can hold sandbox handles open; ordering the
shutdown this way is what stops an interrupted run leaving orphaned
children or half-removed sandboxes behind.

A case whose sandbox cannot be provisioned is errored; the run aborts with
exit 3 only when the run root itself cannot be created.

## 13. Reporting

Human output:

```plaintext
netsuke test

PASS compile.yml::test_generates_object_targets
FAIL compile.yml::test_substituted_macro_receives_source

  then[1]: substitutes.compile_cmd.calls[0].args[0] == "src/main.c"
    substitutes.compile_cmd.calls[0].args[0] = "src/lib.c"

2 cases: 1 passed, 1 failed
```

The JSON document mirrors the human report with stable fields:

```json
{
  "format_version": 1,
  "summary": { "total": 2, "passed": 1, "failed": 1, "errored": 0,
               "skipped": 0 },
  "cases": [
    {
      "id": "compile.yml::test_generates_object_targets",
      "status": "passed",
      "duration_ms": 12
    },
    {
      "id": "compile.yml::test_substituted_macro_receives_source",
      "status": "failed",
      "duration_ms": 9,
      "failures": [
        {
          "assertion": "then[1]",
          "expression": "substitutes.compile_cmd.calls[0].args[0] == \"src/main.c\"",
          "rendered": "substitutes.compile_cmd.calls[0].args[0] = \"src/lib.c\""
        }
      ]
    }
  ]
}
```

`status` is one of `passed`, `failed`, `errored`, or `skipped`; `failed`
and `errored` are distinct end to end (§11.3), and both map to exit
code 1. `format_version` increments on any non-additive report change.
Substituted values in `rendered` fields are truncated beyond a few
kibibytes with an explicit elision marker, in both human and JSON output,
so one failing assertion over a large Ninja file cannot balloon the
report. JUnit XML output is a deferred addition; Terraform's
file-to-testsuite, run-to-testcase mapping is the template to follow when
it lands.[^2]

## 14. Worked example

### 14.1. Quick start

A test tree needs nothing beyond a Netsukefile and one test file in the
default `netsuke-tests` directory:

```plaintext
project/
├── Netsukefile
└── netsuke-tests/
    └── hello.yml
```

The subject manifest declares one target with a literal command and no
sources:

```yaml
netsuke_version: "1.2.0"

targets:
  - name: build/hello.txt
    command: echo hello > build/hello.txt
```

The test file declares one case with one step that generates Ninja and
asserts the target exists:

```yaml
netsuke_test_version: "1.0"

test_hello_target_is_generated:
  steps:
    - when: generate_ninja
      then:
        - result.ok
        - result.graph.has_target("build/hello.txt")
```

Run it from the project root:

```plaintext
netsuke test
```

A passing run reports one case:

```plaintext
netsuke test

PASS hello.yml::test_hello_target_is_generated

1 case: 1 passed
```

This case needs no external tools, no compiler, no network access, and no
real filesystem fixtures: `generate_ninja` runs entirely against the
in-memory pipeline described in §10.

### 14.2. Worked example

Subject `Netsukefile`:

```yaml
netsuke_version: "1.2.0"

tests:
  root: netsuke-tests

macros:
  - signature: "compile_cmd(src, obj)"
    body: |
      {{ env('CC') }} -c {{ src }} -o {{ obj }}

targets:
  - foreach: glob('src/*.c')
    when: item | basename != 'skip.c'
    name: "build/{{ item | basename | with_suffix('.o') }}"
    command: "{{ compile_cmd(item, 'build/' ~ (item | basename | with_suffix('.o'))) }}"
    sources: "{{ item }}"

defaults:
  - build/main.o
```

Test file `netsuke-tests/compile.yml`:

```yaml
netsuke_test_version: "1.0"

macros:
  - signature: "stand_in_compile(src, obj)"
    body: |
      STUB {{ src }} -> {{ obj }}

test_skips_filtered_sources:
  description: foreach expands sources; when filters skip.c; the compile
    macro can be substituted.
  tags: [manifest, foreach]
  steps:
    - given:
        env:
          set:
            CC: clang
        let:
          glob: mock(args=["src/*.c"],
                     returns=["src/main.c", "src/skip.c"])
          compile_cmd: substitute("stand_in_compile")
      when: generate_ninja
      then:
        - result.ok
        - result.graph.has_target("build/main.o")
        - not result.graph.has_target("build/skip.o")
        - contains(result.ninja, "STUB src/main.c -> build/main.o")
        - mocks.glob.call_count == 1
        - substitutes.compile_cmd.call_count == 1
```

The case verifies `foreach` expansion, `when` filtering, environment-driven
command construction, and macro wiring — without a compiler installed,
without touching the real filesystem, and identically on every machine.

## 15. Non-goals and deferred features

Non-goals:

- Replacing Netsuke's own Rust test suites. This framework tests
  Netsukefiles as user artefacts; `cargo nextest` continues to test Netsuke
  the implementation.
- General-purpose scripting. The dialect is declarative by design; logic
  that does not fit belongs in the manifest under test or in a future
  execution phase.

Deferred, in likely delivery order:

1. Build execution (`execute` action, `--allow-execute`) and fixture shell
   commands (`--allow-fixture-scripts`).
2. Filter and Jinja-test doubles.
3. File- and session-scoped fixtures.
4. Data-driven case tables (parameterized matrices), following OPA's named
   subcase reporting.[^3]
5. Snapshot assertions against generated Ninja.
6. JUnit XML output; an idempotence check asserting that regenerating from
   an unchanged manifest yields byte-identical Ninja.

## 16. Risks and trade-offs

- The dialect is declarative and closed: predicate functions and dynamic
  response handlers are deliberately absent (§8.3). A case that needs
  computed behaviour has no escape hatch until fixture scripts land (§15).
- Strictness defaults trade ceremony for early failure: an unmatched call
  on a `mock` fails the action immediately, and an unused double warns by
  default (§8.4). Authors pay for this with `lenient: true` opt-outs on
  legitimately unused doubles.
- Plan-mode-only means `netsuke test` validates everything up to and
  including the generated Ninja text, and nothing about the behaviour of
  the commands within it, until `execute` ships (§10).
- Tests are sandbox-rooted: fixtures cannot touch the project root or the
  host filesystem (§9). A manifest that legitimately reads the project
  tree therefore behaves differently under test than under a real build.
- Every dialect addition — a new matcher, a new assertion form — is a
  minor-version event, and older runners reject newer files by design
  (§4). This is deliberate rigidity, traded for the guarantee that a
  runner never silently misreads a newer test file.

## 17. Rejected alternatives

RFC 0007 evaluates these alternatives in full; this section names them and
the conclusion only.

- **An instrumented general-purpose-language harness.** Rejected in favour
  of a closed declarative dialect: a general-purpose language reopens the
  drift risk that same-compiler, declared-substitution design exists to
  close (§1.1).
- **Assertions embedded in the Netsukefile.** Rejected because it mixes
  build data with test data and couples the manifest's shape to its own
  verification, contrary to the separation this framework establishes
  between subject manifest and test file (§4).
- **Snapshot-only testing.** Rejected as the sole assertion style, though
  snapshot assertions against generated Ninja remain a deferred addition
  (§15): structured assertions against `result.graph` give better failure
  output (§11) than diffing raw Ninja text.

[^1]: Act documents its unsupported-functionality list and positions itself
    as fast pre-flight rather than a substitute oracle:
    <https://nektosact.com/not_supported.html>.

[^2]: Terraform/OpenTofu test framework: run blocks, `command = plan`,
    mocking, and `expect_failures` semantics:
    <https://developer.hashicorp.com/terraform/language/tests> and
    <https://developer.hashicorp.com/terraform/language/tests/mocking>.

[^3]: OPA policy testing: `test_` discovery, `with` substitution,
    `--var-values`, and `--fail-on-empty`:
    <https://www.openpolicyagent.org/docs/policy-testing>.

[^4]: shellmock: first-match configuration lists, call journal, and
    suggested configurations for unexpected calls:
    <https://github.com/boschresearch/shellmock>.

[^5]: flexmock: stub/mock/spy taxonomy, opt-in ordering, teardown-time
    verification: <https://flexmock.readthedocs.io/en/latest/>.

[^6]: cmd-mox: stub/mock/spy controller API, invocation journal, and
    record-replay-verify lifecycle: <https://github.com/leynos/cmd-mox>.

[^7]: Mockito: act-then-assert stubbing, strict stubbing and
    `UnnecessaryStubbingException`, and over-mocking guidance:
    <https://github.com/mockito/mockito/wiki/How-to-write-good-tests>.

[^8]: pymox record-replay model and its rigidity:
    <https://github.com/ivancrneto/pymox>.

[^9]: Ansible Molecule: phase sequence, fast inner-loop subcommands, and
    `--destroy=never`:
    <https://docs.ansible.com/projects/molecule/getting-started-playbooks/>.
