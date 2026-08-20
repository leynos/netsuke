# RFC 0002: Repository-wide code-health contracts and fuzzing

## Preamble

- **RFC number:** 0002
- **Status:** Proposed
- **Created:** 2026-08-11

## Summary

Netsuke should make its repository-wide health policy executable. A single
validator should check GitHub workflow policy, gate self-consistency, exception
hygiene, and documentation links and references. Scheduled `cargo-fuzz` targets
should exercise hostile manifest, Jinja, interpolation, path, and
Ninja-emission inputs. Pull requests should receive fast, deterministic
blocking signals, while longer coverage and mutation signals remain scheduled
and clearly labelled.

This proposal is inspired by the actual upstream [VTCode repository][vtcode],
but is not a copy of its implementation. The pinned
[VTCode workflow-policy script][vtcode-policy] and
[cargo-fuzz manifest][vtcode-fuzz-manifest] show useful patterns; Netsuke must
adapt them to its own gates and stronger safety policy.

## Problem

Netsuke has several good checks, but their contracts are distributed across
Make targets, workflow YAML, scripts, and documentation. Existing workflow
contract tests cover important release and mutation-testing callers, yet they
do not provide one policy for every workflow. A workflow can therefore refer to
a missing Make target, profile, configuration file, local reusable workflow, or
stale exception without a consistent failure at the point of change.

The repository also has no coverage-guided fuzzing boundary for the input
surfaces most likely to turn malformed data into a panic, an unexpected path,
or invalid Ninja output. Example tests and Proptest provide valuable structured
coverage, but hostile byte streams and combinations of malformed YAML, Jinja,
paths, and interpolation deserve a separate harness.

Finally, health signals are not uniformly classified as per-pull-request
blocking or scheduled information. This makes it harder to distinguish a
regression that must stop a merge from a trend that needs investigation. The
drift in the
[formal-verification guide](../formal-verification-methods-in-netsuke.md),
which describes verification support as absent despite current Proptest and
Kani work, is a concrete example of the cost of that ambiguity.

## Current state

The top-level [Makefile](../../Makefile) exposes `check-fmt`, `lint`, `test`,
`test-workflow-contracts`, `markdownlint`, `nixie`, changed-tooling checks, and
Kani targets. The [main CI workflow](../../.github/workflows/ci.yml) runs
formatting, linting, spelling, workflow contracts, the Rust test suite,
changed-line coverage, and a PR Kani smoke job. The
[mutation-testing workflow](../../.github/workflows/mutation-testing.yml) is
scheduled and calls a pinned reusable workflow. These are useful existing
contracts and should remain authoritative for their respective checks.

Cargo already denies `unsafe` code and broad classes of Clippy hazards. The
repository also uses Proptest, Kani, changed-line coverage, and scheduled
mutation testing. Fuzzing therefore supplements the current verification
portfolio; it must not weaken or silently replace any of those tools. The
repository's no-blanket-retry policy also applies to the new jobs: a retry may
address infrastructure recovery only when it is explicitly scoped and
observable, never a failing health assertion.

The existing workflow files use pinned action commits, but policy validation is
currently spread across individual tests. The current contract-test layout is a
suitable home for focused fixtures and regression cases while the validator
itself remains a small, repository-level tool.

## Goals and non-goals

- Goals:
  - Validate every GitHub workflow's security and reference policy with one
    deterministic, reviewable command.
  - Prove that workflow references, Make targets, profiles, configuration
    files, and declared health jobs exist and agree with one another.
  - Add scheduled, resource-bounded `cargo-fuzz` targets for the hostile input
    surfaces that cross the manifest-to-Ninja boundary.
  - Classify health signals by their per-PR or scheduled role, with explicit
    ownership, failure meaning, and escalation.
  - Make exceptions finite, owned, justified, scoped, and time-bounded.
  - Detect broken documentation links and stale references to gates or tools.
  - Preserve the existing unsafe, Clippy/Whitaker, Proptest, Kani, coverage,
    mutation, and no-blanket-retry policies.
- Non-goals:
  - Replacing unit, integration, behavioural, snapshot, Proptest, Kani,
    coverage, mutation, or Whitaker checks with fuzzing.
  - Introducing a new release-security policy; release admission belongs to
    [RFC 0001](0001-release-hardening.md).
  - Requiring full mutation testing, full coverage, or every fuzz target on
    every pull request.
  - Copying VTCode's workflow scripts, action choices, or policy exceptions.
  - Expanding the proposal into a runtime sandbox, a new build system, or a
    change to Netsuke's manifest semantics.

## Proposed design

### Repository-wide workflow-policy validator

Add one deterministic validator, called by an existing or new Make target, that
parses every workflow under `.github/workflows/` and reports violations with a
file, YAML path, rule identifier, and remediation. The validator should be
testable without GitHub credentials and should consume repository files only.

The initial policy should enforce these invariants:

1. Every external action and reusable workflow reference uses a full,
   lower-case, 40-character commit SHA. Local references beginning with `./`
   are permitted only when the referenced file exists in the repository.
2. Workflow and job permissions are explicit. The default token permission is
   empty, and a job may add only the scopes it needs. Any broader scope is an
   exception with an owner, rationale, issue or pull-request reference, and
   expiry.
3. `pull_request_target` jobs have an explicit policy exception and cannot
   execute untrusted checkout content or interpolate untrusted pull-request
   text into a shell command.
4. Every `needs` job name, local workflow, local action, script path, Make
   target, nextest profile, and configuration path named by a workflow exists.
5. Workflow triggers, concurrency, and cancellation settings follow the
   repository policy. A job must not hide a failed health check behind
   `continue-on-error` unless that job is classified as scheduled or has an
   explicit, unexpired exception.
6. Health jobs identify their tier in their name or metadata, and their
   failure or informational status is not contradictory to the tier registry.

The validator must parse YAML rather than use regular expressions for YAML
structure. It may use narrow text inspection for shell commands and comments,
but each such rule must have a fixture proving that quoting, multiline values,
and comments do not create false positives.

### Gate self-consistency contracts

The validator should build a reference inventory before applying policy. The
inventory is the set of paths, targets, profiles, jobs, and tools named by
workflows and the canonical Makefile. A reference is valid only when its
resolved target exists and its invocation shape agrees with the owning contract.

The first contracts should include the following:

- A literal `make TARGET` in a workflow names a declared Make target, including
  the repository's documented phony targets.
- A local reusable workflow or action resolves to a tracked file of the
  expected kind.
- A `--profile NAME` argument resolves to a `[profile.NAME]` section in the
  configured nextest file; a referenced tool-version file exists and is
  non-empty.
- A path passed to a workflow cache, upload, coverage, mutation, Kani, or
  spelling step exists at checkout time or is created by an earlier step in the
  same job.
- Every `needs` reference names a job in that workflow, and every declared
  tier has at least one producing job.
- A documentation link or indexed documentation path resolves to a tracked
  file. References to `make TARGET` and named quality gates resolve to current
  targets or to an explicitly marked historical reference.

Existing focused contract tests remain valuable. They should call the shared
inventory and add domain-specific assertions, rather than duplicate parsers or
silence a failure that the repository-wide validator has found.

### Scheduled cargo-fuzz targets

Create a fuzz workspace using `cargo-fuzz` with one target per boundary. The
initial target names are descriptive and may be adjusted during implementation,
but the coverage contract is fixed:

- `manifest_yaml`: arbitrary bytes are parsed as hostile manifest input.
- `jinja_expansion`: malformed templates, undefined values, nested control
  flow, and expansion-size pressure are exercised.
- `command_interpolation`: placeholder boundaries, quoting, backticks,
  repeated substitutions, and malformed UTF-8 are exercised.
- `path_processing`: absolute, parent-traversal, separator, NUL, and very
  deep path inputs are exercised within the current capability boundary.
- `ninja_emission`: valid and rejected IR values are rendered, with hostile
  names and command text included.

Each harness must be a pure library boundary: it must not invoke a user's
shell, run Ninja, access the network, or write outside a test-controlled
temporary directory. Invalid UTF-8 and malformed syntax are inputs to reject,
not reasons for the harness to panic. Harnesses must bound input and output
sizes, avoid unbounded recursion, and make the same input produce the same
classification and output.

The first assertions should encode current behaviour, not invent a new language
contract:

- Parsing and expansion return a typed error or a bounded value, never a panic.
- Successful expansion preserves the existing control-key and binding rules.
- Interpolation rewrites only the existing whole-placeholder forms and
  rejects unmatched delimiters according to the current contract.
- Path handling preserves the permitted root and rejects inputs that escape
  it.
- Ninja emission is deterministic for the same IR and either produces output
  accepted by the existing renderer contract or a typed error.

Run short smoke corpora on pull requests only when they fit the deterministic
blocking budget. Run longer fuzzing sessions on a schedule, archive new
crashers as minimized regression inputs, and make every crash a blocking
regression test before deleting or quarantining it.

### Health tiers and exception/allowlist registry

Maintain a small machine-readable registry describing each health signal: name,
owning workflow or Make target, tier, blocking status, schedule, owner, and
failure response. The registry is documentation for humans and input to the
self-consistency validator; it is not a second implementation of a gate.

Per-PR blocking signals should include workflow-policy validation, gate
self-consistency, formatting, linting, the existing Rust tests, workflow
contracts, and documentation consistency. Existing changed-line coverage and PR
Kani smoke remain blocking where they are already required. Scheduled mutation
testing, extended fuzzing, full coverage trend reporting, and other
resource-heavy analyses remain scheduled signals until their cost and stability
justify promotion.

Every exception must be represented in the registry with exactly one owner, one
rationale, one issue or pull-request reference, a narrow rule and file scope, a
creation date, and an expiry date. The validator rejects expired, duplicate,
ownerless, broad, or unknown exceptions. It also rejects a policy rule that is
bypassed inline when no corresponding registry entry exists. Expiring an
exception is a deliberate review event; silently extending it is not an
acceptable repair. An allowlist entry is an exception with the same metadata
and expiry requirements, never a wildcard bypass for a whole workflow or
directory.

### Documentation consistency

The documentation check should begin with finite, high-value contracts:

- links in `docs/contents.md` and Markdown files resolve to existing files or
  valid external URLs;
- examples naming Make targets, workflow files, profiles, or tool-version
  files resolve to the current repository;
- the documented per-PR and scheduled gate lists match the tier registry; and
- statements about Proptest, Kani, coverage, mutation, and fuzzing are checked
  against the current workflow and Makefile contracts.

The check should report stale prose for correction rather than rewrite prose
automatically. Markdown formatting, `en-GB-oxendict` spelling, and Mermaid
validation remain their existing gates.

### Inspiration and boundary

The actual upstream [VTCode repository][vtcode] provides the inspiration for
repository workflow policy and a cargo-fuzz layout. Its
[workflow-policy script][vtcode-policy] and [fuzz runner][vtcode-fuzz-runner]
are useful reference points at commit
`f188bcb0e47d7386886ab0c3db7e338b297a3d07`. Netsuke should not copy them
verbatim: the pinned evidence shows a workflow case where an unpinned checkout
would violate Netsuke's own policy, and Netsuke's existing gate and permission
contracts differ.

## Requirements

### Functional requirements

- The validator must inspect every tracked workflow and fail with actionable,
  stable rule identifiers.
- A clean checkout of Netsuke must have zero unresolved workflow, Make target,
  profile, configuration, job, and documentation references.
- Five fuzz targets must cover the manifest, Jinja, interpolation,
  path, and Ninja-emission boundaries, with malformed input cases included.
- Every health signal must have exactly one tier, owner, schedule or PR event,
  and failure meaning.
- Every exception must be narrow, owned, justified, referenced, and unexpired.
- Fuzz crashes must become reproducible regression inputs before the corpus is
  considered healthy.

### Technical requirements

- Keep all workflow action and reusable-workflow references pinned to full
  commit SHAs, with no untracked mutable-reference escape hatch.
- Preserve `unsafe` forbiddance, warnings-denied Clippy and Whitaker, Proptest,
  Kani, changed-line coverage, scheduled mutation testing, and no blanket
  retries.
- Keep fuzz harnesses deterministic, resource-bounded, offline, and free of
  shell or Ninja execution.
- Keep per-PR checks deterministic and fast enough for normal review; put
  unbounded or resource-heavy work in scheduled jobs with explicit budgets.
- Test every validator rule with valid and invalid fixtures, including YAML
  quoting, multiline commands, local references, and documented exceptions.
- Use the repository's existing Markdown formatting and spelling conventions,
  including 80-column prose wrapping and en-GB-oxendict spelling.

### Acceptance criteria

- A clean checkout passes the validator, and fixture tests demonstrate a
  failure for every policy class: mutable action reference, permission
  escalation, unsafe `pull_request_target` use, missing reference, broken tier,
  and invalid exception.
- Every tracked workflow has zero unresolved local references, Make targets,
  profiles, configuration paths, job dependencies, or documentation links.
- No external workflow reference is mutable, and every exception has all
  required metadata, a narrow scope, and an expiry later than the validation
  date.
- Each of the five fuzz boundaries has a checked-in smoke corpus containing
  valid, malformed, and boundary inputs, and each target completes its smoke
  run without a panic, timeout, or uncontrolled filesystem access.
- Twenty consecutive scheduled fuzz runs publish the target name, corpus
  revision, execution budget, and crash result; any new crash is reproduced by
  a deterministic regression test before the run is considered healthy.
- The per-PR policy and documentation checks complete in under two minutes on
  the standard CI runner across twenty measured runs, without a blanket retry.
- The tier registry and the workflow inventory agree for every health signal,
  and the documentation checker reports zero stale gate or tool claims on the
  release branch.

## Compatibility and migration

The proposal is additive to application behaviour. It changes CI acceptance
only for repository policy violations and introduces no new manifest syntax or
runtime dependency for Netsuke users. Migration should proceed in phases so
existing contributors can distinguish policy defects from product defects.

### Phase 1: Inventory and report

Extract the workflow, Make target, profile, configuration, health-tier, and
documentation inventories. Run the validator in report-only mode and record
real findings as small, owned work items. Do not create a permanent baseline
that hides existing violations; each accepted exception must be explicit and
expiring.

### Phase 2: Block deterministic contracts

Add fixture-driven validator tests and make workflow-policy, self-consistency,
exception hygiene, and documentation-link checks blocking on pull requests.
Keep existing focused workflow tests, but have them share the inventory where
appropriate. Correct the known formal-verification documentation drift in the
same phase.

### Phase 3: Add scheduled fuzzing

Add the fuzz workspace, bounded smoke corpus, and the five boundary targets.
Run smoke cases where practical and longer sessions on a scheduled workflow.
Publish minimized crashers as regression fixtures, retain failure artefacts,
and document the schedule and resource budget in the tier registry.

### Phase 4: Ratchet and review

Measure policy findings, fuzz execution, crash regressions, coverage trends,
mutation survivors, and exception age for at least one release cycle. Promote
only stable scheduled signals to per-PR blocking status. Review the exception
registry each release and remove entries whose underlying policy defect is
fixed.

### Failure modes and responses

- Failure in a blocking check stops the pull request with the rule identifier
  and remediation. The validator must not downgrade the failure because a
  reference is inconvenient to parse.
- A scheduled fuzz failure opens or updates an owned investigation with its
  corpus, revision, and artefact links. It must not be hidden by a blanket
  retry or represented as a passing check.
- A fuzz timeout, out-of-memory result, or uncontrolled filesystem access is a
  harness defect and blocks promotion of that target until bounded.
- Tool or runner outages are reported separately from product failures and may
  use a narrowly scoped, observable infrastructure retry if policy permits.
- A false-positive validator result is corrected with a fixture and a rule
  change, not with a broad allowlist entry. An exception is acceptable only
  while the issue, owner, and expiry remain visible.

## Alternatives considered

### Option A: Copy VTCode's scripts and workflow layout

This would be quick, but it would import assumptions about action pinning,
permissions, tools, and repository structure. The
[VTCode workflow-policy script][vtcode-policy] is inspiration rather than a
compatible contract, and copying it would risk weakening Netsuke's existing
gates. Rejected.

### Option B: Keep only focused workflow contract tests

Focused tests are useful for detailed caller semantics, but they do not provide
complete inventory coverage or one consistent exception policy. They also leave
documentation and gate self-consistency outside the contract. Rejected as the
complete design; retained as a complement.

### Option C: Run all fuzzing and mutation testing on every pull request

This would produce strong signals but would make review latency and runner
capacity unpredictable. It would encourage retries or truncated runs when the
real problem is resource pressure. Rejected; use bounded PR smoke checks and
scheduled depth.

### Option D: Use a hosted fuzzing service first

A hosted service could provide longer campaigns, but it adds credentials,
external state, and operational dependency before the harness contracts are
stable. Rejected for the first phase; the local `cargo-fuzz` boundary can later
feed a hosted service if an accepted design provides the required controls.

## Open questions

- Which runner and schedule provide enough fuzzing budget without competing
  with release, coverage, or mutation workflows?
- Should the fuzz corpus live under `fuzz/corpus/` in Git, in workflow
  artefacts, or in both, and what size limit should apply to checked-in inputs?
- Which existing workflow permissions need temporary exceptions, and who owns
  their expiry review?
- Should the first documentation consistency check validate only repository
  links and named gates, or also check examples against generated CLI help?
- Which changed-line coverage and mutation trends, if any, should become
  blocking after the first release-cycle measurement?
- Should the tier registry be YAML, TOML, or a typed Rust/Python data file so
  its schema is easiest to validate without duplicating workflow parsing?

## References

- [Actual upstream VTCode repository][vtcode]
- [VTCode workflow-policy script at a pinned commit][vtcode-policy]
- [VTCode cargo-fuzz manifest at a pinned commit][vtcode-fuzz-manifest]
- [VTCode fuzz runner at a pinned commit][vtcode-fuzz-runner]
- [GitHub Actions secure-use guidance][github-actions-security]
- [cargo-fuzz project documentation][cargo-fuzz]

[cargo-fuzz]: https://github.com/rust-fuzz/cargo-fuzz
[github-actions-security]: https://docs.github.com/en/actions/reference/security/secure-use
[vtcode]: https://github.com/vinhnx/VTCode
[vtcode-fuzz-manifest]: https://github.com/vinhnx/VTCode/blob/f188bcb0e47d7386886ab0c3db7e338b297a3d07/fuzz/Cargo.toml#L1-L46
[vtcode-fuzz-runner]: https://github.com/vinhnx/VTCode/blob/f188bcb0e47d7386886ab0c3db7e338b297a3d07/scripts/fuzz-test.sh#L1-L104
[vtcode-policy]: https://github.com/vinhnx/VTCode/blob/f188bcb0e47d7386886ab0c3db7e338b297a3d07/scripts/check_workflow_security.sh#L1-L55

## Recommendation

Adopt the repository-wide validator and tier registry first, then add the
bounded scheduled fuzz targets once their harness contracts are covered by
fixtures. This gives Netsuke immediate, deterministic protection against
workflow and documentation drift while preserving its stronger existing
verification policy. Treat VTCode as a useful upstream reference, pin all
external workflow references, and make every exception and scheduled signal
visible, owned, and measurable.
