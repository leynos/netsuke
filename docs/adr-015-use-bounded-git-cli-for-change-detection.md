# Architecture decision record (ADR): Use bounded Git CLI queries for change detection

## Status

Proposed.

## Date

2026-08-22

## Context and problem statement

Netsuke needs a template helper that returns paths changed between two Git
commits. The implementation must preserve Git's range semantics and unusual
path names without exposing arbitrary command arguments or shell parsing to a
manifest. It must also keep child output bounded and remain testable without
mutating `PATH`, changing the process working directory, or depending on the
repository that runs the test suite.

The repository has command runners for arbitrary shell filters and Ninja build
processes, but neither owns fixed, read-only Git queries. Adding a native Git
library would create another interpretation of revision resolution,
configuration, and merge-base behaviour.

## Decision

Implement `git_changed_files()` through a feature-private `GitRepository` port
with a bounded Git CLI adapter. The port belongs to
`src/stdlib/change_detection/` and permits only three fixed operations:

- resolve one caller endpoint to a commit object ID with the fixed vector
  `git --no-lazy-fetch rev-parse --verify --end-of-options <endpoint>^{commit}`;
- find every best merge base for a three-dot comparison with the fixed vector
  `git --no-lazy-fetch merge-base --all <left> <right>`; and
- obtain NUL-delimited changed paths with the fixed vector
  `git --no-lazy-fetch diff --no-ext-diff --no-textconv --no-renames
  --name-only -z --diff-filter=ACDMRTUXB <base> <right> --`.

The adapter resolves caller input before diffing. Later operations receive only
validated hexadecimal object IDs, never the original revision text. The diff
disables external diff drivers, text conversion, and rename detection. It
inherits no stdin, reads stdout and stderr concurrently, and applies the
standard-library command capture limit to both streams.

Git's top-level `--no-lazy-fetch` option is required for every vector. A
missing promisor object therefore fails locally instead of triggering remote
contact.

The port is not a general Git or command service. Only the change-detection
module may call it, and no caller may provide flags, subcommands, pathspecs, or
environment mutations. Production receives the absolute workspace path and
optional command `PATH` override through `StdlibConfig`; tests supply scripted
port responses.

The associated pure `matches_glob()` filter enforces fixed v1 preflight limits
of 64 supplied patterns and 65,536 aggregate UTF-8 pattern bytes. Duplicate
patterns count towards both limits. The filter rejects an over-limit set before
compiling or allocating any compiled patterns; these limits are not
configurable.

## Rationale

- **Git remains the semantic authority.** Revision peeling, object lookup, and
  merge-base computation follow the Git implementation installed for the
  repository workflow.
- **The command surface is closed.** Parsing a strict range and resolving it to
  object IDs prevents template text from entering an option or pathname
  position.
- **Path fidelity is explicit.** `git --no-lazy-fetch diff --name-only -z`
  preserves newlines and other non-NUL bytes until Netsuke performs its
  required UTF-8 check.
- **Resource use stays bounded.** The adapter shares the stdlib capture budget
  and drains both output streams rather than using an unbounded convenience
  call.
- **Tests remain isolated.** A feature-private port supports parser, error, and
  composition tests without process-global environment or directory changes.

## Consequences

Netsuke requires a discoverable Git executable when a manifest invokes
`git_changed_files()`. Registration still succeeds when Git or an absolute
workspace path is unavailable; invocation reports the missing dependency.

Git queries mark the standard-library render impure immediately before the
first process starts. Validation failures remain pure. Manifest-query
environments reject the function because discovery must not inspect repository
state.

The adapter must retain fixed argv tests, output-limit tests, and
low-cardinality telemetry. A future Git-backed feature must either fit the
three declared operations or propose a new decision; it must not widen the port
into arbitrary Git execution.

The `matches_glob()` filter must retain tests for its fixed pattern-count and
pattern-byte preflight limits, including duplicate-pattern accounting and
rejection before compilation. The limits are part of the v1 contract rather
than configuration.

## Outstanding decisions

Before this proposal is accepted, resolve:

- whether Git operations should receive a dedicated timeout or continue using
  the shared command wait policy.

## Alternatives considered

- **Run a shell pipeline from the template.** Rejected because every manifest
  would own shell quoting, option separation, platform syntax, and path record
  parsing.
- **Use `git2`.** Rejected because it adds a substantial native dependency and
  a second semantic implementation for behaviour Git already owns.
- **Reuse the arbitrary shell-filter runner as the public boundary.** Rejected
  because that runner admits user-selected commands and output modes, while
  change detection needs a closed read-only protocol.
- **Reuse the Ninja process boundary.** Rejected because Ninja execution owns
  long-running build lifecycle, status parsing, and reporter integration that
  do not belong to a manifest query.

## Implementation references

- Detailed contract and verification:
  [`docs/git-change-detection-helpers-design.md`](git-change-detection-helpers-design.md)
- Standard-library configuration and registration:
  [`src/stdlib/config/mod.rs`](../src/stdlib/config/mod.rs) and
  [`src/stdlib/register.rs`](../src/stdlib/register.rs)
- Existing bounded command primitives:
  [`src/stdlib/command/execution.rs`](../src/stdlib/command/execution.rs) and
  [`src/stdlib/command/pipes.rs`](../src/stdlib/command/pipes.rs)
- Delivery roadmap:
  [`docs/roadmap.md`](roadmap.md#6-change-aware-manifest-planning)
