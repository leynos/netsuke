# Architectural decision record (ADR) 034: Runtime annotation introspection in workflow contracts

## Status

Accepted.

## Date

2026-09-21

## Context and problem statement

The contract modules under `tests/workflow_contracts/` annotate several
functions with names imported only under `typing.TYPE_CHECKING`. Resolving
those annotations at runtime raises `NameError`. Because the repository targets
a Python 3.14 baseline, PEP 649 defers annotation evaluation, so the modules
import cleanly and every gate passes; the failure appears only when something
resolves an annotation rather than loading the module.

Four facts bound the problem. Each was measured against the pinned toolchain
rather than assumed.

**PEP 649 hides the failure from the loader.** Under the 3.14 baseline the
affected modules import without error, and the workflow-contract suite that
imports them passes.

**The future import is forbidden here.** The df12 house lint refuses
`from __future__ import annotations` by name on a 3.14 baseline:

```plaintext
C9112: Remove 'from __future__ import annotations' on a 3.14+ baseline
(redundant-future-annotations)
```

No module in this repository uses it, which is consistent with the rule rather
than an oversight. Applying the fix reviewers suggested on
[#728](https://github.com/leynos/netsuke/pull/728) and
[#729](https://github.com/leynos/netsuke/pull/729) reds `make lint-python`.

**The current placement is Ruff's own requirement.** `TC003`
(`typing-only-standard-library-import`) fires on a runtime import used only in
annotations, so moving the import back out of the `TYPE_CHECKING` block trades
one gate failure for another.

**The failure is real, and predates both pull requests.** It is true on `main`
for names such as `fractions`, `cabc`, `Path`, `pl`, and `CmdMox`. A sweep of
every module in `tests/workflow_contracts/` that resolves each module's own
functions with `typing.get_type_hints` finds **23 of 77 modules and 85
functions** affected. The oldest of the two modules named in the original
report dates to [#688](https://github.com/leynos/netsuke/pull/688), well before
the work that raised the question.

Nothing in the repository reads these annotations at runtime. `ty` resolves
them statically and passes; pytest never introspects them.

### The loader gate is not the mechanism that would catch this

The obvious candidate is to extend `lint-workflow-scripts`, whose recipe
comment claims that loading a module "catches a definition-time failure such as
an annotation naming a TYPE_CHECKING-only import". Measured on a minimal module
with exactly this shape, under the baseline, it would not:

```plaintext
--- runpy.run_path, the loader gate's own mechanism ---
loaded with no error
--- accessing __annotations__ ---
NameError: name 'fractions' is not defined
--- typing.get_type_hints ---
NameError: name 'fractions' is not defined
```

PEP 649 is what makes the loader sufficient no longer. Loading a module defers
annotation evaluation rather than performing it, so the gate stays useful for
other definition-time failures but not for the one its comment names. That
comment is stale for the 3.14 baseline and is corrected alongside this record.

A check that *would* catch it resolves the annotations rather than merely
loading the module: walk each module's functions and call
`typing.get_type_hints` on each. That is a different gate, not an extension of
the existing one, and it would fail on `main` today across the 23 modules above
until those annotations are addressed, so adopting it means deciding what to do
about them in the same breath.

## Decision drivers

- Keep the `TYPE_CHECKING` idiom intact. It is what `TC003` requires, and it is
  uniform across the affected modules.
- Introduce no suppression. The repository treats lint silencing as a last
  resort, and the idiom's spread would turn a per-module suppression into a
  blanket one.
- Introduce no new gate for a failure mode nothing has hit. Every gate passes
  today and no consumer resolves these annotations.
- Make the position citable, so the question is not reopened from scratch by
  each reviewer who notices the asymmetry.
- Correct the stale claim in the loader gate's comment, which is the one part
  of this that is straightforwardly wrong.

## Requirements

### Functional requirements

- The `TYPE_CHECKING` blocks and the annotations themselves stay unchanged in
  every affected module.
- The `lint-workflow-scripts` recipe body stays unchanged: its loader loop
  continues to run and continues to be useful for definition-time failures
  other than this one.
- The decision must name a reopening condition concrete enough to be
  recognized when met.

### Technical requirements

- `make lint-python`, `make typecheck`, and `make test-workflow-contracts` must
  stay green, with no new suppression and no new lint disablement.
- The decision must be discoverable from the documentation index and from the
  code it governs.

## Options considered

### Option A: Import the names at runtime

Move the annotation-only imports out of the `TYPE_CHECKING` block and suppress
`TC003` at each site. This makes `get_type_hints` work without touching any
annotation.

It costs one suppression per affected module — 23 of them today, and one more
for each module that adopts the idiom. `TC003` would otherwise fire on every
one, so the suppression is not incidental to the option but constitutive of it.
It also imports names at runtime that no runtime path uses.

### Option B: Quote the annotations

Write the annotations as string literals. The annotations then resolve without
the name being bound, so `get_type_hints` works and `TC003` stays satisfied
with no suppression.

The cost is one mechanical edit per annotation — 85 today — and a quoting
convention that reads as incidental rather than deliberate. A reader has no
signal that the quotes are load-bearing, and a later contributor who removes
one as noise silently reintroduces the failure. `TC003` continues to fire on
the underlying import regardless, so the `TYPE_CHECKING` block stays.

### Option C: Record that runtime introspection is unsupported (chosen)

Leave the code unchanged and record that these modules are read statically by
`ty` and executed by pytest, and that neither resolves annotations at runtime.
Declare runtime annotation introspection outside the supported surface of
`tests/workflow_contracts/`, and write that down where a reader will find it.

This costs nothing at the call site and keeps the idiom uniform. Its risk is
that a future consumer resolves an annotation and meets a `NameError` that this
record says is expected rather than repairable.

### Option D: Extend the loader gate to `tests/`

Walk `tests/workflow_contracts/` in `lint-workflow-scripts` as well as
`.github/scripts`. Rejected: measured above, `runpy.run_path` loads a module
with this shape without error under PEP 649, so the extension would not catch
the failure it was proposed for. It would still add the cost of loading every
test module to a gate whose subject is the trusted workflow helpers.

| Topic                         | A: runtime import | B: quoted annotations | C: record non-support | D: extend loader |
| ----------------------------- | ----------------- | --------------------- | --------------------- | ---------------- |
| Makes `get_type_hints` work   | Yes               | Yes                   | No                    | No               |
| Sites to change today         | 23                | 85                    | 0                     | 0                |
| New suppressions              | 23                | 0                     | 0                     | 0                |
| Cost scales with the idiom    | Yes               | Yes                   | No                    | No               |
| Keeps `TC003` satisfied       | Only via suppress | Yes                   | Yes                   | Yes              |
| Catches this class of failure | Yes               | Yes                   | No                    | No               |
| Reader can tell it was chosen | Yes, suppression  | No, reads as noise    | Yes, this record      | Yes              |

*Table 1: Comparison of the options considered.*

## Decision outcome

Adopt **Option C**. Runtime annotation introspection is not a supported use of
the contract modules under `tests/workflow_contracts/`. They are read
statically by `ty` and executed by pytest, and neither resolves annotations at
runtime.

The `TYPE_CHECKING` blocks stay as they are, the annotations stay as they are,
and no gate is added. The stale `lint-workflow-scripts` recipe comment is
corrected to stop claiming that loading a module catches an annotation naming a
`TYPE_CHECKING`-only import, and to name this record for the class it no longer
reaches.

## Rationale

The decision turns on where the cost and the benefit sit.

The benefit of Options A and B is hypothetical. Nothing in the repository
resolves these annotations, so neither option changes any observed behaviour;
both buy readiness for a consumer that does not exist. The cost, meanwhile, is
real and scales with the idiom rather than with the benefit: 23 suppressions
under Option A, 85 mechanical edits under Option B, and one more of whichever
per module that adopts the pattern.

That asymmetry is what makes this a documentation decision rather than a code
change. The failure mode is not that the code is wrong; it is that the code has
an undocumented sharp edge, and that the gate comment describing how the edge
would be caught is no longer true. Option C addresses both without paying a
cost that grows.

Option D was rejected on measurement rather than on judgement. It is the
intuitive fix — the existing gate already loads workflow modules and already
claims to catch this — and the claim is simply no longer accurate under PEP

1. Extending a gate that cannot observe the failure would have added cost and
a false assurance.

Options A and B are not rejected on merit. Both would work. They are deferred,
and the record states what each costs, so the work is a known quantity if the
revisit gate is ever met.

## Revisit gate

Reopen this decision when any code or gate must resolve annotations on these
modules at runtime — that is, when something calls `typing.get_type_hints`, or
reads `__annotations__`, on a function defined in `tests/workflow_contracts/`
outside of a test that is explicitly checking this behaviour.

At that point the affected set is known and the choice between Options A and B
can be made on its merits. Option B is the narrower edit and introduces no
suppression; Option A is more legible at the site. Both are bounded by the
sweep recorded here, which a single script re-derives in seconds.

## Consequences

- The `TYPE_CHECKING` idiom stays uniform across `tests/workflow_contracts/`,
  with no suppressions and no quoting convention.
- `ty` and pytest are unaffected: both already ignore the issue.
- No new gate is introduced, so `make lint` and `make test` keep their current
  cost.
- The `lint-workflow-scripts` comment no longer overstates what loading a
  module proves, which removes a trap for the next person who reads it and
  trusts it.
- A future consumer that resolves these annotations will meet a `NameError`
  that this record describes but does not repair. That is the accepted risk,
  and the revisit gate is how it is meant to be caught.

## Known risks and limitations

- The decision rests on the absence of a consumer. That absence is verified by
  inspection of the repository, not by a gate, so a consumer added outside this
  repository would not be detected until it failed.
- The affected set is stated as a measurement at a point in time. It will grow
  as modules adopt the idiom, and the figures above should be re-derived rather
  than quoted if the question is reopened.
- Correcting the loader gate's comment does not give it a replacement
  mechanism. Definition-time failures that PEP 649 does not defer — a name used
  at module scope, an invalid decorator — are still caught by loading the
  module, and remain the gate's subject.

## References

- [#728](https://github.com/leynos/netsuke/pull/728) and
  [#729](https://github.com/leynos/netsuke/pull/729) raised this during review
  and each declined the change on the grounds recorded here.
- [#730](https://github.com/leynos/netsuke/issues/730) is the issue this record
  resolves.
- [#688](https://github.com/leynos/netsuke/pull/688) introduced the older of
  the two modules first reported.
- [PEP 649](https://peps.python.org/pep-0649/) defines the deferred annotation
  evaluation that makes loading a module insufficient.
- [`tests/workflow_contracts/python_shell_interpreter_test.py`](../tests/workflow_contracts/python_shell_interpreter_test.py)
  holds the `shell: python` steps to the same baseline and records the incident
  that motivated the interpreter check.
- [ADR-028](adr-028-defer-split-build-dir-harness-trim.md) is the repository's
  other decision to defer work behind a stated revisit gate rather than close
  it.
