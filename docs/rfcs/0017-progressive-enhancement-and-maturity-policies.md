# RFC 0017: Progressive enhancement and maturity policies

## Preamble

- **RFC number:** 0017
- **Status:** Proposed
- **Created:** 2026-09-19
- **Scope:** Shallow-end compatibility and opt-in contract enforcement
- **Implementation:** [Progressive-enhancement roadmap, phases 20 and
  25][roadmap]

## 1. Summary

Make progressive enhancement a compatibility requirement, not introductory
marketing. Netsuke must remain useful as a small command runner and build
compiler. States, typed inputs, named execution settings, owned artefacts, and
contention declarations add local guarantees only when explicitly adopted.

Add optional maturity policies for projects and operators that want to require
particular declarations. Policy adoption must resemble enabling selected type
checker rules: ordinary unannotated manifests remain legitimate, and stricter
coverage is not a new installation prerequisite. No maturity score, automatic
promotion, or mandatory project tier is necessary.

## 2. The shallow-end contract

The current quickstart's first manifest remains a release acceptance fixture:

```yaml
netsuke_version: "1.0.0"

targets:
  - name: hello.txt
    command: "echo 'Hello from Netsuke!' > hello.txt"

defaults:
  - hello.txt
```

The manifest requires Netsuke and Ninja, not a typed input, context registry,
state provider, ownership model, package manager, policy file, or network
connection. The current list-of-mappings target shape and manifest version must
not be replaced with an invented onboarding dialect.

These requirements apply to every RFC in this proposal set:

- A plain command retains its existing ambient environment, working-directory,
  shell, interpolation, and scheduling semantics. Simplicity must not silently
  change a legacy shell string into direct argv execution.
- Opting one action into a feature does not require annotating its neighbours or
  aggregates. An unannotated aggregate may depend on an annotated action.
- Unused state or context declarations never cause runtime probes, installation,
  network access, or resource acquisition. Existing template-time capability
  behaviour remains governed by its own policy; these additions introduce no new
  inspection-time execution.
- Ordinary variables remain supported. No heuristic promotes `uv sync` to a
  state, a redirection to an ownership claim, or a Cargo command to a pool.
- The straightforward command escape hatch remains available under the default
  policy. A selected stricter policy may reject it with a local explanation, but
  a plugin must not be the only way to execute an unusual tool.
- New opt-in schemas must validate correctly. Optional adoption is not
  permission to ignore malformed typed inputs, unsafe paths, or unknown state
  operations.

The first page of onboarding must teach only the existing core. Subsequent
examples introduce one feature when it solves a visible problem, show the
simpler alternative, and state the added guarantee and its limits. Basic usage
is a supported destination, not a temporary migration stage.

## 3. Relationship to existing plans

RFC 0001 owns structured execution, RFCs 0002 to 0004 own composition, and
roadmap phase 5 owns profiles, inspection, and their OrthoConfig integration.
Issue #592's semantic linter owns reusable manifest analysis. This RFC adds
maturity-rule selection and trust-aware enforcement over that analysis; it must
not establish a competing linter, parser, configuration loader, or JSON
envelope.

[RFC 0013][states] supplies preparation contracts, [RFC 0014][inputs] supplies
input contracts, [RFC 0015][artefacts] supplies ownership, and
[RFC 0016][contention] supplies pool-backed contention. Named execution contexts
remain a compatible extension point, not a sixth prerequisite hidden in these
five RFCs. The maturity schema may add a context rule only after that separate
surface has an accepted definition and an implementation.

Do not repurpose RFC 0008's repository health tiers or RFC 0005's release
admission policy as user-manifest maturity. Their purposes, trust sources, and
failure meanings differ.

## 4. Proposed policy shape

A proposed root policy can require one useful guarantee at a time:

```yaml
maturity:
  rules:
    - id: structured-commands
      severity: warn
      select: [publish]
    - id: owned-cleanup
      severity: error
      select: [clean-build]
```

The default is an empty rule list. There is no default warning about missing
annotations. Every rule contains `id`, `severity`, and an explicit `select` list
of action/target identities. Initial severities are `off`, `warn`, and `error`.
Exact names keep the first scope contract small; bounded patterns can follow
only with a reviewed match and namespace contract.

Unknown rules, duplicate rule/subject combinations, unknown selectors, and
unsupported severities fail validation. A policy cannot claim enforcement of a
feature that the installed version does not understand. Emit a
version/capability remedy rather than silently skip the rule.

Rule-specific `subjects` lists refine what the rule checks, where required.
Selectors name the graph nodes to inspect, not arbitrary paths or command-text
regular expressions. Evaluation uses resolved declarations and provenance.

## 5. Initial rule contracts

The initial rules have deliberately narrow, checkable meanings:

| Rule | Required evidence on selected nodes |
| --- | --- |
| `structured-commands` | Every resolved executable recipe unit is structured; legacy shell strings fail coverage. Explicit structured shell selection is not a claim of direct-argv safety. |
| `typed-inputs` | Each explicitly named configuration subject in `subjects` resolves to a typed input contract, not only an untyped variable. Internal variables are not automatically public inputs. |
| `owned-cleanup` | A selected cleanup action contains explicit `clean_owned` operations or nonexecuting aggregation only; arbitrary executable deletion recipes cannot satisfy the declaration contract. |
| `verified-states` | Each state named in `subjects` has a `require_state` or `ensure_state` operation before its first non-state command unit; a probe in a different action is insufficient. |
| `contention-declared` | Each selected executable edge resolves an explicit valid contention class. Dependency-only aggregates are not executable subjects. |

Table 1: Initial declaration-coverage rules, not whole-program safety proofs.

`typed-inputs` and `verified-states` require nonempty explicit `subjects`. The
first implementation must not guess which variables are external knobs or which
arbitrary command consumes an environment. For `verified-states`, the policy's
selected action/subject pair asserts that consumption relationship; analysis
requires the readiness operations before the first non-state command unit,
rather than guessing a consumption point inside a shell command. This is a
conservative opt-in recipe-order contract, not inferred whole-program dataflow.

The ordinary schema enforces correctness regardless of rule severity. For
example, an invalid path in `clean_owned` remains an error even when
`owned-cleanup` is off. A structured command using a shell is still subject to
existing shell policy. Passing these rules does not establish hermeticity,
reproducibility, correct cleanup ownership, or safe untrusted-code execution.

## 6. Scope and non-contagion

Resolve policy against the selected build closure, retaining distinct definition
and invocation provenance. A rule selecting `publish` applies when that node
will execute; it does not make an unrelated `hello.txt` build fail coverage.
Global syntax and reference errors remain errors even outside the closure.
`netsuke check` can inspect the whole manifest and report each explicit scope.

Aggregate selection must not implicitly make every dependency strict. If a
future closure selector is added, its spelling must be explicit and inspection
must enumerate its bounded expansion. Rule references within a selected recipe
remain part of that recipe and cannot hide legacy execution from its coverage
check. Imported private nodes retain qualified provenance without becoming
publicly selectable through an accidental export.

A root may impose policy on imported public actions. An imported fragment or
bundle cannot relax its importer. Bundle-local strengthening applies only to
that instance, never unrelated root targets. Unknown private selectors fail;
implicit namespace wildcarding is not permitted.

## 7. Trust-aware composition and profiles

Use the existing configuration provenance and trusted operator boundary. An
automatically discovered project file, imported bundle, or explicitly chosen
`--config` file is not thereby trusted to weaken operator policy.

For overlapping rule/subject scopes, combine severity monotonically: `off < warn
< error`. Project and bundle declarations can strengthen but cannot lower an
operator floor. Expand and normalize scopes before combining them so renaming a
selector or splitting a rule cannot hide an overlap. Constraints specific to a
rule also combine without widening allowed behaviour.

Profiles may select reviewed policy sets using the existing profile machinery.
Record the policy source and effective rule set; merely choosing a development
profile must not disable an operator error. Do not introduce a public
`--ignore-policy` escape hatch. An author may remove a project-owned optional
rule when no stronger authority requires it, but cannot bypass the runtime
capability boundary.

The initial release needs no universal `strict` preset. A future preset must
have a versioned, enumerable rule set and cannot gain new blocking rules on an
unrelated software upgrade. Report-only adoption precedes error enforcement.
Narrow exceptions require an independently reviewed future contract; do not ship
a blanket suppression file that silently makes strict mode meaningless.

## 8. Evaluation and diagnostics

Run declaration coverage through the semantic linter's typed inventory. Separate
pure policy evaluation from manifest loading, capability observation, and runner
effects. Evaluate applicable errors before starting any selected user action.
Inspection and dry-run do not execute state probes to satisfy a maturity rule;
they check declaration evidence only.

A warning reports a gap without changing a successful command's exit status. An
error uses the existing validation/policy failure class and stops execution.
Human and JSON output include rule ID, severity, selected subject, definition
span, policy-source span, and one local remedy. Reuse Fluent localization,
structured-result envelopes, and redaction metadata; do not leak input values or
probe output in diagnostics or metric labels.

`context --json` describes supported rules and effective settings through the
existing metadata surface. `check --json` reports findings. Neither depends on a
new `explain` command, whose separate roadmap evaluation remains unresolved.
Supported basic manifests must have no new maturity messages under default
settings, including verbose warnings that imply untyped usage is deprecated.

## 9. Acceptance, learning loop, and rollout

Retain the exact quickstart fixture and a small three-command project. Snapshot
parsed declarations, generated commands, default diagnostics, and selected
execution behaviour before and after every feature. Compare observed child
arguments and filesystem effects, not just apparent YAML similarity.

Add one-feature-only examples: a typed worker input without a context, a state
without typed inputs, one cleanup root without a state, and one pool on a legacy
command. Combine annotated and unannotated actions under an ordinary aggregate.
Assert that unrelated invocation starts no probes and creates no state records.

Property-test severity monotonicity, scope composition, order independence,
namespace resolution, and inability to weaken operator constraints. End-to-end
tests must cover profiles, imported policies, unknown rule versions, warning
versus error exits, selected versus whole-manifest checks, and generated-plan
replay under the applicable trusted policy.

Document and measure onboarding separately from the Cuprum migration. The
quickstart may not gain required declarations. Each enhancement must demonstrate
its local benefit and explicitly identify any remaining shell helper; moving
boilerplate to an unreviewed imaginary bundle does not count as simplification.
Do not claim a usability improvement from line count alone.

## 10. Alternatives and outstanding decisions

Mandatory maturity levels would make advanced features contagious. Automatically
promoting projects by size or feature count would alter semantics unexpectedly.
A global strict mode with an evolving implicit rule list would make upgrades
break otherwise unchanged manifests. Separate validators per feature would
duplicate source handling and reporting.

Ratify policy-source placement within the shared configuration contract,
selected-closure inspection metadata, and the semantic linter's reusable
inventory boundary before implementation. Rule-specific exceptions, named
presets, pattern selectors, and context-coverage rules remain deferred.

## 11. Recommendation

Ratify the shallow-end compatibility contract before expanding the language.
Deliver a small, opt-in, scope-explicit rule mechanism over the shared linter,
with stronger policies controlled by the appropriate authority rather than
imposed on every Netsuke user.

[roadmap]: ../roadmap-progressive-enhancement.md
[states]: 0013-managed-states-and-probes.md
[inputs]: 0014-typed-task-inputs.md
[artefacts]: 0015-artefact-ownership-and-scoped-cleanup.md
[contention]: 0016-named-contention-classes.md
