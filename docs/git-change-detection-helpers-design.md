# Git change-detection helpers design

- **Status:** Proposed
- **Audience:** Netsuke maintainers, reviewers, and manifest authors
- **Last updated:** 2026-08-22
- **Companion documents:**
  - [Netsuke design](netsuke-design.md)
  - [Template standard-library guide](stdlib-yaml-and-jinja-guide.md)
  - [ADR-014](adr-014-use-bounded-git-cli-for-change-detection.md)
  - [Roadmap](roadmap.md)

## 1. Design context

Netsuke evaluates manifest-time `when` expressions before it builds the static
intermediate representation (IR). A manifest can select work from filesystem,
environment, and executable state, but it cannot ask which paths differ between
two Git commits. Continuous Integration (CI) manifests must consequently run
every language-specific check or reconstruct change detection through an impure
shell pipeline.

The shell form is difficult to compose with Jinja values and easy to get wrong:
newline-delimited Git output cannot represent every valid path, a revision can
be mistaken for an option, and shell quoting differs by platform. Git already
defines commit-range and path-output contracts; Netsuke should expose a narrow,
typed adapter rather than make each manifest reproduce the command protocol.
Git documents two-dot and three-dot revision notation in
[gitrevisions][gitrevisions] and provides NUL-delimited path output through
`git diff --name-only -z`.[^1]

The motivating workflow is a change-aware quality gate:

```yaml
targets:
  - name: lint-rust
    when: >-
      git_changed_files('origin/main...HEAD')
      | matches_glob('**/*.rs', 'Cargo.toml', 'Cargo.lock')
    command: cargo clippy --workspace --all-targets --all-features -- -D warnings
```

The helper decides whether the target enters the generated graph. It does not
select files for the linter or change Ninja's incremental dependency model.

## 2. Goals and non-goals

### 2.1. Goals

- Expose `git_changed_files(range)` as a MiniJinja function returning a stable
  sequence of paths changed between two commits.
- Expose `matches_glob()` as a pure filter that returns true when any input
  path matches any supplied pattern.
- Preserve Git's familiar two-dot and three-dot meanings while rejecting
  ambiguous or unsafe range inputs before starting a child process.
- Preserve unusual valid path names by parsing NUL-delimited Git output and
  fail explicitly when a path cannot be represented as a MiniJinja string.
- Bound subprocess output, errors, and telemetry cardinality.
- Reuse the repository's existing `glob` dependency and matching policy rather
  than create a second pattern language.
- Keep process and repository state injectable so tests do not mutate the
  process environment or depend on the checkout that runs the suite.

### 2.2. Non-goals

- Accept arbitrary `git diff` arguments, pathspecs, working-tree comparisons,
  staged changes, or untracked files.
- Infer a CI provider's base branch, fetch missing commits, or contact a remote.
- Return Git status codes, patches, line counts, object IDs, or rename pairs.
- Run a selected linter against only the returned files. The helper controls
  manifest selection; each target still owns its command and arguments.
- Replace Ninja's dependency graph, depfiles, or ordinary timestamp checks.
- Provide a general Git command API to manifests.
- Match non-UTF-8 repository paths lossily.

## 3. Public template contract

The normative signatures are:

```text
git_changed_files(range: string) -> list<string>
files | matches_glob(pattern: string, ...patterns: string) -> bool
```

The intended composition reads naturally in a bare manifest expression:

```jinja
git_changed_files('origin/main...HEAD') | matches_glob('**/*.rs')
```

`git_changed_files()` returns an empty list when the selected commits have
identical trees. `matches_glob()` returns false for an empty input list. It
requires at least one pattern; accepting no patterns would conceal a manifest
mistake behind a constant false result.

The filter accepts a MiniJinja sequence whose members are strings. It rejects a
scalar, mapping, undefined value, or sequence containing a non-string member.
It accepts patterns as variadic string arguments, so a manifest does not need
to construct a temporary pattern list. The public v1 contract does not accept a
sequence as the pattern argument; that alternative can be added compatibly if
real manifests need data-driven pattern sets.

`git_changed_files` joins MiniJinja's global namespace and must join the
manifest loader's `RESERVED_VAR_NAMES`; otherwise a manifest variable could
replace the function after registration. `matches_glob` occupies MiniJinja's
separate filter namespace and must join the stdlib filter inventory used by
registration tests.

## 4. Commit-range semantics

### 4.1. Accepted grammar

The function accepts exactly one of these forms:

```text
<left>..<right>
<left>...<right>
```

Each endpoint must be non-empty, contain no Unicode whitespace, and not begin
with `-`. The parser recognizes `...` before `..`, requires exactly one range
operator, and rejects a fourth consecutive dot. Endpoint text remains a Git
revision expression, so forms such as `HEAD~2`, tags, branch names, and full
object IDs remain available. Missing endpoints, single revisions, multiple
ranges, and space-separated revisions are errors.

This grammar intentionally narrows Git's general revision language. Git permits
omitted range endpoints to default to `HEAD`, while `git diff <commit>`
compares a commit with the working tree.[^2] Neither behaviour belongs in a
function whose output claims to describe a commit range.

### 4.2. Endpoint resolution

The Git adapter resolves each endpoint with:

```text
git rev-parse --verify --end-of-options <endpoint>^{commit}
```

The `^{commit}` suffix requires the result to peel to a commit. Netsuke accepts
exactly one hexadecimal object ID terminated by a newline and passes only that
object ID to later Git commands. This two-stage protocol prevents the original
revision expression from becoming an option or pathname in `git diff`.

For `A..B`, Netsuke compares resolved commit `A` with resolved commit `B`. This
matches `git diff A..B`, for which the two dots do not denote the reachability
set used by commands such as `git log`; they select two diff endpoints.[^3]

For `A...B`, Netsuke runs `git merge-base --all A B`. Exactly one merge base
must exist. Netsuke compares that base with `B`, matching Git's documented
three-dot diff direction. No merge base is a history error. Multiple best merge
bases are also an error because choosing one would make the result depend on an
unspecified selection in criss-cross history.

### 4.3. Changed-path selection

The final comparison is equivalent to:

```text
git diff --no-ext-diff --no-textconv --no-renames --name-only -z \
  --diff-filter=ACDMRTUXB <base> <right> --
```

Netsuke supplies every argument directly to the child process; no shell parses
the range or object IDs. `--` closes option parsing before any future pathspec.
`--no-ext-diff` and `--no-textconv` prevent repository or user configuration
from executing external diff helpers. `--no-renames` avoids similarity
thresholds and represents a rename as deletion of the old path plus addition of
the new path. That conservative representation is useful for gate selection:
renaming `src/lib.rs` to `src/lib.txt` still selects checks for Rust and for
text.

The diff filter names every status that can carry a path. Submodules appear as
their repository-relative entry path when their recorded commit changes. The
function does not descend into submodules.

## 5. Changed-path result contract

Git emits path names as bytes separated by NUL. Netsuke splits on NUL before
performing any text conversion, rejects an empty interior record, and requires
each path to be valid UTF-8. MiniJinja strings cannot preserve arbitrary byte
paths, so an explicit error is safer than replacement characters that could
match the wrong glob.

Each returned path must be:

- relative to the selected repository worktree;
- free of a leading `./`;
- normalized to `/` separators on every platform;
- non-empty; and
- present only once in the result.

The implementation sorts paths lexicographically by their normalized UTF-8
bytes and removes duplicates. Git does not need to promise output ordering for
Netsuke to provide deterministic template values. Sorting also makes rendered
manifests and diagnostic fixtures stable across Git versions.

Netsuke preserves deletion paths even though those paths no longer exist in the
right-hand tree. `matches_glob()` is lexical and never consults the filesystem,
so deleted paths remain valid filter inputs.

## 6. Glob-filter semantics

`matches_glob()` compiles every pattern before matching any path. If any
pattern is invalid, the filter fails rather than returning a partial result. It
returns true on the first `(path, pattern)` pair that matches and false only
after exhausting both collections.

The filter uses [`glob::Pattern`][glob-pattern] and the same `MatchOptions`
policy as Netsuke's existing `glob()` helper:

- matching is case-sensitive on every platform;
- separators are literal, so `*` does not cross `/`;
- `**` can match recursively across separators;
- wildcard tokens may match leading-dot path components; and
- patterns and candidates use `/` as the public separator.

Unlike `glob()`, this filter performs no directory walk, metadata lookup, or
symbolic-link resolution. It applies patterns to supplied strings only. The
capability rules in
[ADR-010](adr-010-scope-glob-capability-to-literal-prefix.md) therefore do not
apply to its execution, although sharing syntax avoids forcing authors to learn
two meanings for `**/*.rs`.

Pattern matching has worst-case work proportional to the number of input paths
times the number of patterns. The changed-path output byte limit bounds the
path side. V1 accepts any non-empty variadic pattern set because manifests are
trusted executable configuration, but the implementation must compile each
distinct pattern once per filter call, not once per path. A configurable
pattern budget needs measured evidence and is deferred.

## 7. Architecture and ownership

Figure 1 shows the data flow from a manifest expression to its Boolean gate.

```mermaid
flowchart LR
    T[MiniJinja template] --> P[Range parser]
    P --> R[Resolve endpoints to commits]
    R --> B[Select right endpoint or unique merge base]
    B --> D[Bounded Git diff]
    D --> N[Normalize, sort, and de-duplicate paths]
    N --> G[Any path matches any glob]
    G --> W[Manifest-time when decision]
```

*Figure 1: change-detection evaluation flow.*

The `src/stdlib/change_detection/` module owns the feature. Its internal
boundaries are:

- `range` parses the public grammar into `CommitRange` and never starts Git;
- `git` resolves revisions, selects merge bases, and obtains NUL-delimited
  paths through a `GitRepository` port;
- `glob` validates MiniJinja values and applies compiled patterns; and
- `mod` registers the function and filter and maps domain errors to
  `minijinja::Error`.

`GitRepository` is a feature-private test seam, not a general repository
service. Only `change_detection` may call it. The production adapter may run
only the three fixed operations in §4; callers cannot supply flags, commands,
or pathspecs. Tests provide scripted responses without changing `PATH`, the
current directory, or the checkout. Future Git-backed helpers must justify
extending this port against this ownership rule instead of treating it as a
generic command escape hatch.

This module is a new abstraction because repository-wide inspection found no
existing Git process boundary. The shell-filter runner accepts arbitrary
commands and output modes, while the Ninja runner owns long-running build
processes; neither expresses fixed, read-only Git queries. The new adapter may
reuse bounded pipe-reading primitives where their types remain neutral, but it
must not expose shell-filter configuration or Ninja lifecycle state.

`StdlibConfig` supplies three existing inputs: the absolute workspace-root
path, the command `PATH` override, and the command capture-byte limit. The
standard `register()` and manifest loader already provide an absolute root. An
embedder that constructs `StdlibConfig::new(Dir)` without
`with_workspace_root_path()` can still register the library, but
`git_changed_files()` fails on invocation with a missing-workspace-path error.
`matches_glob()` remains fully available.

## 8. Registration, purity, and query mode

Ordinary standard-library registration installs both helpers.
`git_changed_files()` captures the Git adapter, the workspace configuration,
and the shared `StdlibState` impurity flag. It flips the flag immediately
before the first Git process starts, including executions that later fail.
Syntax rejection and missing configuration leave the render pure because they
did not observe repository state. Marking successful and attempted Git queries
impure prevents a cached render from surviving a change in refs, repository
config, or available objects.

`matches_glob()` is pure: its result depends only on its value and arguments.
It is registered in both ordinary and manifest-query environments.

Manifest-query environments must install a deliberate failing stub for
`git_changed_files()`, as they already do for `glob()`, `env()`, and command
helpers. Discovery commands must not expose host or repository state. The stub
uses the existing manifest-query diagnostic shape so callers receive a named,
corrective failure instead of an undefined-function error.

## 9. Failure and resource boundaries

The module uses a semantic internal error enum and converts it to MiniJinja
errors only at registration. Its observable categories are:

| Condition                                              | MiniJinja kind     | Behaviour                                                          |
| ------------------------------------------------------ | ------------------ | ------------------------------------------------------------------ |
| Wrong value type, no glob patterns, or malformed range | `InvalidOperation` | Reject before host inspection.                                     |
| Invalid glob syntax                                    | `SyntaxError`      | Name the invalid pattern and parser detail.                        |
| Missing workspace path or Git executable               | `InvalidOperation` | Explain the required configuration or executable.                  |
| Unknown or non-commit revision                         | `InvalidOperation` | Identify the endpoint without printing command output unboundedly. |
| No merge base or multiple merge bases                  | `InvalidOperation` | Reject the three-dot comparison.                                   |
| Git exit failure                                       | `InvalidOperation` | Include the operation, exit status, and bounded stderr.            |
| Output exceeds the configured capture limit            | `InvalidOperation` | Report the byte limit and operation.                               |
| Non-UTF-8 or malformed NUL output                      | `InvalidOperation` | Reject the whole result.                                           |

The adapter applies `StdlibConfig::with_command_max_output_bytes()` to each
stdout stream. It also bounds stderr to the same limit and drains both streams
concurrently to avoid child-process deadlock. No command inherits stdin. The
initial implementation uses the existing command-helper wait policy; adding a
separate Git timeout is deferred until Netsuke exposes one coherent subprocess
timeout setting.

Error text may contain the caller's range because authors need to correct it,
but telemetry must not. Trace spans use the fixed operation names
`resolve_commit`, `merge_base`, and `diff_paths`, plus the bounded outcome
`success` or `error`. Metrics may count calls and record duration with those
closed labels. Object IDs, revisions, repository paths, glob patterns, and Git
stderr never become metric labels or tracing fields.

## 10. Correctness and verification

Correct implementation requires these invariants:

1. **Range confinement:** every accepted input yields exactly two resolved
   commit object IDs; no caller text reaches a Git option position after
   resolution.
2. **Direction:** `A..B` compares `A` to `B`; `A...B` compares the unique merge
   base of `A` and `B` to `B`.
3. **Path fidelity:** every NUL-delimited UTF-8 path appears once in normalized,
   sorted output, including embedded newlines.
4. **Conservative renames:** a pure rename contributes both old and new paths.
5. **Any-to-any matching:** the filter result equals
   `exists(path, pattern): pattern matches path` for every valid input.
6. **Fail-closed parsing:** one invalid pattern or path member prevents a
   Boolean result.
7. **Purity transition:** validation failures remain pure; starting Git marks
   the render impure, whether Git succeeds or fails.

Property-based tests should generate valid and invalid range strings, path
sets, and pattern sets to check invariants 1, 5, and 6 against a simple
reference predicate. Example-backed tests should use temporary repositories to
pin two-dot direction, three-dot direction, deletions, newline-bearing names,
rename representation, no and multiple merge bases, missing Git, output caps,
and query-mode rejection. The production adapter's fixed argv must be asserted
at the injected port so a future edit cannot silently re-enable external diff
drivers or rename detection.

The design has one material interaction surface: range kind (`..` or `...`) ×
change kind (add, modify, delete, rename, or submodule) × glob result (match or
miss). A table-driven end-to-end manifest suite must cover each change kind at
least once and both range kinds with both Boolean outcomes. Exhaustive formal
proof is disproportionate because Git defines repository history and diff
semantics outside Netsuke; the suite verifies Netsuke's parser, argv, byte
protocol, normalization, and composition boundaries instead.

The verification does not prove Git's own diff implementation or filesystem
encoding behaviour. Pinning object IDs, disabling configurable diff hooks, and
testing against the CI Git implementation make that residual dependency
explicit.

## 11. Alternatives considered

### 11.1. Use `shell('git diff ...')`

Rejected. It exposes shell quoting, newline parsing, platform command syntax,
and option injection to every manifest. It also returns text rather than a
typed path sequence.

### 11.2. Accept arbitrary Git revision arguments

Rejected. A string containing multiple argv tokens needs a parser whose rules
would differ from both a shell and Git. Passing it to a shell is unsafe;
splitting on whitespace rejects valid expressions and accepts option-like
inputs. The strict two-endpoint grammar serves the stated use case.

### 11.3. Include working-tree and untracked changes

Rejected for v1. Working-tree, index, and untracked-file views have distinct
semantics and need an explicit mode contract. Mixing them into a commit-range
function would make CI and local results diverge.

### 11.4. Use `git2`

Rejected. Netsuke already depends operationally on Git workflows, while `git2`
would add a substantial native dependency and reproduce Git CLI configuration
and merge-base edge cases. Fixed argv plus bounded byte parsing keeps the
adapter small and auditable.

### 11.5. Add `globset`

Rejected. `globset` efficiently matches multiple patterns, but adding it would
create a second glob implementation and invite semantic drift from `glob()`.
The output byte limit bounds changed lists. Compiling the existing `glob`
patterns once and applying an any-to-any loop avoids another pattern language;
implementation evidence can justify a specialized matcher later if this loop
becomes a measured bottleneck.

### 11.6. Enable rename detection

Rejected. Similarity thresholds can vary with configuration and content.
Returning old and new paths is deterministic and conservatively selects every
gate affected by a rename.

## 12. Compatibility and delivery

Both helpers are additive to the template standard library. A manifest variable
named `git_changed_files` becomes a reserved-name error instead of shadowing
the helper. Other existing manifests, configuration builders, and public Rust
types retain their current behaviour. Embedders without an absolute workspace
path see an invocation-time error only if they call the new Git helper.

The implementation must update the template standard-library guide, the core
Netsuke design's function and filter catalogues, the developer guide's
registration and injected-port conventions, localized diagnostics, and the
manifest reserved-name registry. The roadmap deliberately separates the
callable function from the composition and documentation slice so each review
has one operational purpose.

## References

- [Git revision selection][gitrevisions], Git project, accessed 2026-08-22.
- [Git diff documentation][git-diff], Git project, accessed 2026-08-22.
- [`glob::Pattern` documentation][glob-pattern], Rust `glob` crate, accessed
  2026-08-22.
- [MiniJinja `Environment` documentation][minijinja-environment], accessed
  2026-08-22.

[gitrevisions]: https://git-scm.com/docs/gitrevisions
[git-diff]: https://git-scm.com/docs/git-diff
[glob-pattern]: https://docs.rs/glob/0.3.4/glob/struct.Pattern.html
[minijinja-environment]: https://docs.rs/minijinja/2.21.0/minijinja/struct.Environment.html

[^1]: `-z` makes output unquoted and terminates path records with NUL when used
    with `--name-only`.
[^2]: Git documents omitted range endpoints as `HEAD` and the single-commit
    `git diff` form as a worktree comparison.
[^3]: Git explicitly notes that range notation in `git diff` denotes endpoints,
    not a range of commits.
