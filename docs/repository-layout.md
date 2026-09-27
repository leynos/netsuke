# Repository layout

This document explains the major paths in the Netsuke repository and the
responsibilities attached to each area. It is an orientation guide for
contributors and does not replace the source code, design document, or
developer's guide as the source of truth for behaviour.

## Top-level structure

The following tree is a simplified orientation map. It omits generated build
output and some leaf files so the long-lived structure remains visible.

```plaintext
.
├── .cargo/
├── .github/
│   ├── actions/
│   └── workflows/
├── cyclopts/
├── docs/
│   ├── archive/
│   ├── execplans/
│   └── rfcs/
├── examples/
│   └── hello-world/
├── installer/
├── locales/
├── scripts/
├── src/
│   ├── cli/
│   ├── ir/
│   ├── localization/
│   ├── manifest/
│   ├── ninja_gen/
│   ├── runner/
│   ├── snapshots/
│   ├── shell_word.rs
│   └── stdlib/
├── test_support/
├── tests/
│   ├── bdd/
│   ├── cli_tests/
│   ├── data/
│   ├── features/
│   ├── features_unix/
│   ├── fixtures/
│   └── snapshots/
└── tools/
    ├── kani/
    └── mold/
```

## Path responsibilities

- `AGENTS.md`: Repository-specific agent instructions, quality gates, and
  coding rules.
- `Cargo.toml` and `Cargo.lock`: Workspace package metadata and locked
  dependency graph.
- `Makefile`: Canonical quality-gate and workflow commands. Prefer these
  targets over direct tool invocations.
- `README.md`: Public project overview and first contact documentation.
- `README.de.md`, `README.es.md`, `README.fr.md`, `README.ja.md`,
  `README.pt-BR.md`, and `README.zh-CN.md`: Translated editions of the project
  overview, linked from the localization menu at the top of each README. They
  follow [the localization glossary](localization-glossary.md) and are exempt
  from the en-GB-oxendict spelling gate via `typos.local.toml`.
- README maintenance: mirror section changes in all six translated editions,
  preserving heading order and levels, examples, and safety boundaries. Run
  `bash scripts/check-readme-parity.sh` to compare heading counts and level
  sequences; review translated meaning and example content separately. The
  script is a manual reviewer aid and is not wired into continuous integration.
- `.cargo/`: Cargo configuration that Cargo auto-discovers. It holds the
  repository's build standard: the `rustflags` every build takes — the parallel
  `rustc` frontend, plus the `mold` linker under a Linux-only `cfg` table. It
  names no codegen backend, and a contract refuses one. Because it reaches
  release and coverage builds too, those two shapes are excluded by assigning
  `RUSTFLAGS` at the point they run; a setting that is only safe for the
  development loop does not belong here. See
  [developers' guide](developers-guide.md).
- `.github/actions/`: Reusable GitHub Actions used by workflow definitions.
- `.github/workflows/`: Continuous Integration (CI), release, packaging, and
  repository automation workflows.
- `cyclopts/`: Local Python typing support for release and packaging helper
  scripts.
- `docs/`: Long-lived project documentation, guides, design documents, decision
  records, and planning material.
- `docs/archive/`: Historical planning documents retained for traceability
  after active roadmap work moves on.
- `docs/execplans/`: Execution plans used as implementation handoff documents
  for scoped tasks. Each plan opens with a `Status:` line drawn from the closed
  set `DRAFT | APPROVED | IN PROGRESS | BLOCKED | COMPLETE`; the plan's
  `Progress` and `Outcomes & retrospective` sections, and its roadmap checkbox,
  are authoritative over that line. The status vocabulary is defined under
  *ExecPlan* in the [documentation style guide](documentation-style-guide.md).
- `docs/rfcs/`: Requests for Comments proposing changes that need technical
  review before they become binding, named `NNNN-short-topic.md` and numbered
  in allocation order. Numbers are never reused, and are never renumbered after
  publication. Gaps are therefore expected rather than erroneous: a number may
  be reserved, drafted on another branch, or deliberately skipped, so an unused
  number is not free for reallocation. Check every remote branch for
  `docs/rfcs/` files before allocating one.
- `examples/`: Example Netsuke manifests and minimal runnable sample projects.
- `installer/`: Installer packaging assets and platform-specific packaging
  definitions.
- `locales/`: Fluent localization catalogues, one `<tag>/messages.ftl` per
  supported locale tag, so regional and script variants such as `es-419`,
  `pt-BR`, and `zh-Hant` each keep their own directory. The authoritative list
  of tags lives in `src/locale/catalogues.rs`; see the
  [translator guide](translators-guide.md).
- `scripts/`: Shell and helper scripts used by quality gates, release help
  generation, packaging, and formal checks.
- `src/`: Main `netsuke-build` Rust package source code.
- `src/cli/`: Command-line configuration, parsing, validation, and merge logic.
- `src/host/`: Shared host-pattern syntax and runtime hostname matching;
  the build script compiles only the pattern module.
- `src/ir/`: Intermediate representation generation, interpolation, graph, and
  cycle logic.
- `src/locale/`: Shipped Fluent catalogue registry and locale resolution,
  exposed through the historical root module paths.
- `src/localization/`: Localization key definitions and runtime localization
  support.
- `src/manifest/`: Manifest parsing, expansion, rendering, diagnostics, and
  manifest-specific tests.
- `src/ninja_gen/`: Ninja rendering and staged-dyndep bundle generation.
- `src/output/`: Terminal output mode and formatting preferences, exposed
  through the historical `output_mode` and `output_prefs` module paths.
- `src/runner/`: Process execution, path handling, runner errors, command
  orchestration, capability-injected dyndep publication, and bounded generation
  and publication telemetry, including `graph/generation_telemetry.rs`,
  `dyndep/generation_telemetry.rs`, and `process/dyndep/telemetry.rs`.
- `src/snapshots/`: Checked-in `insta` snapshots for source-level snapshot
  tests.
- `src/shell_word.rs`: The single encoding of one string as a recipe shell
  word, for a named dialect. It is a leaf: IR lowering, Ninja rendering, and
  the template filters all depend on it, and it depends on nothing above them.
  See the [developers guide](developers-guide.md) for the paths that
  deliberately do *not* route through it.
- `src/stdlib/`: Netsuke standard library modules exposed to manifest
  rendering.
- `src/stdlib/command/`: Structured-command wrappers, including
  `child_argument.rs` (renamed from `quote.rs`), which spells one argument for
  the interpreter a structured command runs under, including `cmd.exe`.
- `src/stdlib/recipe_text/`: The template-facing `shell_quote` and `shell_join`
  filters, with their dialect telemetry. The filter adapter validates arguments
  and resolves the dialect; the encoding itself is `src/shell_word.rs`.
- `test_support/`: Shared Rust test-support crate used by integration and
  behavioural tests.
- `tests/`: Integration tests, behavioural tests, test data, fixtures, and
  snapshots.
- `tests/bdd/`: `rstest-bdd` step definitions, fixtures, and behavioural-test
  support code.
- `tests/cli_tests/`: Command-line interface integration test modules.
- `tests/data/`: Manifest fixtures and other structured test inputs.
- `tests/features/`: Cross-platform behavioural feature files.
- `tests/features_unix/`: Unix-specific behavioural feature files.
- `tests/snapshots/`: Checked-in integration-test snapshots.
- `tools/kani/`: Kani formal-verification harness configuration and related
  local tooling, including `proof-scope.toml`, the paths whose change runs the
  Kani proofs on a pull request (see
  [ADR-039](adr-039-change-scoped-kani-gate.md)).
- `tools/mold/`: Pinned `mold` linker release version and the SHA-256 checksums
  used to verify the downloaded release artefacts.

## Internal support module ownership

### `src/ir/cycle/support.rs`

`src/ir/cycle/mod.rs` owns this support module and declares it `pub(super)`, so
it is nameable only within `ir`. Its `pub(in crate::ir)` comparisons are
likewise limited to the IR implementation; they must not be re-exported from
the crate or used by non-IR modules.

`first_byte_cmp` owns the bounded string-comparison semantics for Kani builds.
Under `cfg(kani)`, it orders non-empty strings by their first UTF-8 byte,
orders an empty string before a non-empty string, and treats two empty strings
as equal. Its only direct consumers are `path_cmp`, which adapts cycle paths
with `Utf8Path::as_str`, and `sort_utils::string_cmp`, which adapts manifest
rule names. Future IR code may reuse it only when its symbolic inputs have that
same single-byte contract; ordinary builds must keep their full lexical
comparison, and a caller with different semantics must own a separate local
comparator.

This composition keeps the Kani approximation in one owner while leaving the
cycle and manifest modules responsible for adapting their domain values. It is
not a general-purpose string-sorting utility.

### `src/ir/from_manifest/support/sort_utils.rs`

Kani-friendly deterministic sorting and comparison helpers, owned by
`src/ir/from_manifest/support/mod.rs` (which declares `mod sort_utils;`). It
provides `insertion_sort_by`, `sort_strings`, `sort_paths`, and
`has_seen_output`, which the manifest-to-IR rule-resolution and
duplicate-output detection paths consume. Its Kani `string_cmp` adapts rule
names to the `cycle::support::first_byte_cmp` contract; it must not duplicate
or redefine that byte-ordering semantics. Keep the local sorting algorithms
dependency-free and deterministic so the Kani harnesses in
`src/ir/from_manifest/verification.rs` can verify bounded symbolic input, and
do not move them out to a shared utility crate.

### `src/ir/cycle/detector.rs`

The depth-first traversal state machine, owned by `src/ir/cycle/mod.rs` through
its private `mod detector;` declaration. It provides `CycleDetector`,
`VisitState`, and traversal result types used by the production `analyse` entry
point and its Kani presence-only variant. The module is private to `ir::cycle`;
its test and verification children reach the types through the parent module's
private re-exports. Keep graph traversal state here, while path comparison and
cycle canonicalization remain owned by `src/ir/cycle/support.rs`.

### `src/diagnostic_json/support.rs`

Private helpers for the machine-readable diagnostic document in
`src/diagnostic_json/mod.rs`. It owns the span extraction, cause collection,
help and URL rendering, and fallback-payload machinery, exposing them as
`pub(super)` items re-imported by the parent. Only `src/diagnostic_json/mod.rs`
may call into it. The schema remains defined by the parent module; this file is
a size split, not a second schema owner.

### `src/diagnostic_json/tests/shape_tests/excerpt_tests.rs`

The source-excerpt guard over rendered diagnostic documents, declared by
`src/diagnostic_json/tests/shape_tests/mod.rs` through a plain `mod`
declaration. The guard walks a document's causes *and* those of its nested
`related` entries because the serializer renders a related diagnostic as a full
entry of the same shape; a top-level-only walk would leave those cause chains
unguarded. It covers the diagnostic paths, where a normalized cause renders the
failing location through `source` and `labels` and an excerpt in `causes` would
duplicate it. The plain path is deliberately not normalized:
`render_error_json` leaves `source`, `primary_span`, and `labels` empty, so the
cause chain is the only location channel a plain error has, and `causes` is
documented as the error-cause chain itself. One case drives a real excerpt
through `render_error_json` so the guard cannot pass by inspecting nothing; the
snapshot-producing cases stay in `src/diagnostic_json/tests/mod.rs` because
insta derives a snapshot's filename from the module path that asserted it.

### `src/stdlib/command/error_support.rs`

Detail types and message-append helpers for command-failure rendering in
`src/stdlib/command/error.rs`, which declares
`#[path = "error_support.rs"] mod support;`. It owns `ExitDetails`,
`LimitExceeded`, `append_exit_status`, and `append_stderr`, and is reachable
only from that error module. Keep the localized-message keys it uses alongside
the other stdlib command keys rather than introducing a separate key namespace.

### `src/stdlib/time/format.rs`

ISO-8601 rendering for the standard-library time values, owned by
`src/stdlib/time/mod.rs` (which declares `mod format;`). It renders offset
datetimes and UTC offsets to ISO-8601 without a zero-valued fractional part. It
also exposes the `TimeDeltaValue` and `TimestampValue` MiniJinja object types
the parent predicates downcast. Only the time module may import it.

### `src/status/indicatif.rs`

The `indicatif`-backed progress reporter and rendering helpers, owned by
`src/status/mod.rs` through its private `mod indicatif;` declaration. It
provides the crate's `IndicatifReporter` export and the shared stage/completion
rendering helpers used by the accessible reporter. Only `status/mod.rs` and its
test module may reach this private support module; callers use the reporter
re-export from `status`.

### `src/stdlib/which/env/path_support.rs`

Path parsing and Windows executable-candidate construction, owned by
`src/stdlib/which/env/mod.rs`, which declares it as a child module. It owns
`PathEntry`, `PATH` and `PATHEXT` normalization, UTF-8 current-directory
conversion, and Windows candidate generation. Only `which::env` imports it;
lookup modules retain their existing access through `which::env`'s narrow
`pub(super)` re-exports. The split is purely to keep the environment snapshot
adapter below the 400-line cap, not a new resolution boundary.

### `src/stdlib/network/redirect/support.rs`

Localized diagnostics for failed and refused fetch hops, owned by
`src/stdlib/network/redirect/mod.rs` through its `mod support;` declaration. It
owns `fetch_failed_error`, `location_failure_error`, `rejection_error`, and the
`redacted_url` helper every diagnostic renders through. Only `redirect` imports
it. The split keeps the redirect adapter — the HTTP client, the chain budget,
the bounded telemetry, and the `Location` header parse — below the 400-line
cap, not a new boundary: nothing in it decides anything, and it must never grow
a helper that inspects a header, a status, or a chain because those are the
adapter's concerns.

### `src/stdlib/network/redirect/tests/location.rs`

Unit tests for the adapter's `Location` header parse and its diagnostics,
declared under `src/stdlib/network/redirect/tests/mod.rs` with the
directory-module path `tests::location`. It pins the resolver, the closed
`redirect_failure` reason each header failure is counted under, the localized
message it renders, and the four bounded trace fields the refusal logs. The
snapshot-producing cases stay in the parent module: insta derives a snapshot's
filename from the module path that asserted it, and the files under
`src/snapshots/network_redirect/` keep stable names.

### `src/stdlib/network/tests_support.rs`

Shared support for the network tests, declared by `src/stdlib/network/mod.rs`
through `#[path = "tests_support.rs"]`. It owns the shared `REDIRECT_USER` and
`REDIRECT_SECRET` constants and the URL helpers that apply them.
`credentialed_url` preserves the caller's path; the current and target helpers,
`credentialed_current_url` and `credentialed_target_url`, use `/start` and
`/next`, respectively. `credentialed_loopback_url` preserves the fixture's host
and port but normalizes its path to `/start` so fixture-backed diagnostics
remain stable. Use `credentialed_url` when a loopback case needs a different
path. These helpers return errors for malformed URLs or URLs that do not accept
userinfo; redirect tests should use them instead of duplicating credential
literals.

### `test_support/src/check_ninja_tests.rs`

Unix-only unit coverage for the fake-Ninja factories, owned by
`test_support/src/check_ninja.rs` through a test-gated `#[path]` declaration.
It exercises the `-C` directory argument contract through the public factory
only. Keep fixture assertions here and production test-helper behaviour in
`check_ninja.rs`; this split keeps the public helper below the 400-line cap.

### `test_support/src/http/raw.rs`

The raw-response payload for the local HTTP fixture. `HttpResponse` composes a
response: a status line, a header block ending in a blank line, and a
`Content-Length` that matches the body it carries. It does not validate the
status or header values that callers supply, so it is not a guard against a
status that is not three digits or a value containing a line break.
`RawHttpResponse` is the stronger separation: it emits bytes verbatim, so a
case can present a status line, header block, or framing no client accepts. The
two are separate types rather than one type with an escape hatch, so a case
that means to send malformed bytes cannot reach the composed path by accident.
Both implement the crate-private `FinishResponse` trait, which carries the
bytes, and `finish_response` performs the write and the write-side shutdown for
either. That trait exists so the two payloads share one completion contract; it
is not an extension point, and `raw`'s surface is crate-private except for
`RawHttpResponse` itself.

Completion is the reason this module exists. `finish_response` writes the whole
payload and then calls `shutdown(Shutdown::Write)`. Dropping the stream instead
closes both directions at once, and a server that closes while the client's
request bytes are still unread makes the platform answer with a reset, which
discards the response the client had not yet consumed. The client then reports
a transport failure, on Windows Winsock `WSAECONNABORTED` (10053), in place of
the wire-level fault the payload was written to provoke. Shutting down write
alone sends the end of the response as a FIN while the read side stays open to
drain the request, so the client sees exactly the configured bytes. This is
also why a test must not stand a bare `TcpListener` in place of the fixture:
such a listener closes without that shutdown and races the client, which is how
`stdlib::network::redirect::error_tests::protocol_failures_are_classified_from_a_live_response`
came to fail on Windows after the `ureq` 3 bump. The fixture still reads the
request's header block before it answers, so the request bytes are consumed
rather than left to force a reset. A request *body* is deliberately not
consumed: the fixture answers on the header block alone, so it is for bodyless
requests, which is what every fixture case sends.

The `#[cfg(test)] rendered_exchange` helper drives one request through the same
completion path a real client sees and returns both the client's bytes and the
request bytes the fixture consumed. It reads exactly and against no deadline,
so the fixture's lifecycle tests infer nothing from elapsed time.

`RawHttpResponse` is composed with `spawn_raw_http_server`, which follows the
same accept, read, and shutdown contract as the checked wrappers and returns
the same `(String, Arc<AtomicUsize>, HttpServer)` tuple so a case can assert
the malformed response was actually solicited. Keep a raw payload for a
deliberately malformed response; use `HttpResponse` for a valid one that merely
needs an unusual status.

`test_support/src/http/raw_tests.rs` is the fixture's own test-gated `#[path]`
child, declared by `mod.rs`. It pins the bytes each path emits, the end of the
connection after them, and the fixture's consumption of the request, and it
belongs to this fixture rather than to any production module.

### `test_support/src/http/accept.rs`

Connection acceptance for the local HTTP fixture, split out of
`test_support/src/http/mod.rs` to keep the fixture configuration below the
400-line cap. It owns `AcceptWait`, the retry rules that make polling a
non-blocking listener safe, and the accept loop itself. The parent module
declares it `mod accept;`, and its surface is `pub(super)`, so nothing outside
the fixture can reach it. The wait policy stays in `HttpServerConfig`; this
module only carries the wait out.

### `test_support/src/http/config.rs`

Timeout configuration for the local HTTP fixture, split out of
`test_support/src/http/mod.rs` for the same 400-line reason as `accept.rs`. It
owns `HttpServerConfig`, the three `NETSUKE_TEST_HTTP_*` override names, and
the duration parse that reads them. Its accessors are `pub(super)`, so the
fixture's own loops can ask it for a deadline or a poll interval while nothing
outside the fixture can configure one. `config_tests.rs` is its `#[path]`
child, declared by `config.rs`. `raw_tests.rs` is declared by `mod.rs`.

### `src/ir/cmd_interpolate/property_tests/support.rs`

This test-only support module is owned by the command-interpolation property
tests. It may contain their generators, independent specifications, and shared
assertions, but it must not be used by production code or the Kani harnesses.
Keep those proof and production boundaries explicit; move a helper here only
when it serves more than one command-interpolation property test.

### `src/ir/cmd_interpolate/power_shell_tests.rs`

This test-only module owns the command-interpolation cases for protected
PowerShell contexts. Keep those cases here to hold the parent test module below
the 400-line cap; production code must not depend on this test module.

## Placement conventions

Keep every source file below the 400-line cap enforced by Whitaker's
`module_max_lines` lint (see [Whitaker's guide](whitaker-users-guide.md)).
Split large modules by concern while preserving narrow visibility for
implementation helpers. A shared name prefix represents a module hierarchy: put
its children under a directory module named for that prefix, use `mod.rs` for
the directory module, and declare children with plain `mod child;` statements.
Drop the parent prefix from child filenames.

Do not use `#[path]` to reach a sibling or parent file. Keep it only when a
specific requirement still needs it, and add a comment explaining that reason.
When a shared prefix joins genuinely unrelated concerns, keep them separate and
give each a shorter, precise name. The
[support module ownership inventory](#internal-support-module-ownership)
records owners, declarations, permitted callers, and rationale, including any
justified `#[path]` use.

The former `runner/process/command_*` siblings were a naming coincidence:
`environment.rs` composes child-process overrides, `logging.rs` records
redacted invocations, and `list_failure_telemetry.rs` measures attributed
command-list failures. Each remains directly under `process/` with a name for
its own concern.

When adding a justified `#[path]` support module, keep it private to its owner,
give it a `//!` header stating the split reason and ownership, cap its visible
surface at `pub(super)` unless a documented internal caller requires wider
visibility, and add its entry to the ownership inventory above.

Place user-facing documentation under `docs/`, then link it from
[contents.md](contents.md). Use [users-guide.md](users-guide.md) for behaviour
that users or operators need to understand,
[developers-guide.md](developers-guide.md) for maintainer workflows, and
[netsuke-design.md](netsuke-design.md) for architecture and design rationale.

Place new production Rust modules under the `src/` subtree that owns the
feature boundary. Use `test_support/` for reusable integration-test helpers and
keep one-off fixtures close to the tests that consume them.

Place all proposed changes requiring technical review under `docs/rfcs/`, using
numbering and status conventions in the documentation style guide. Link each
RFC from `docs/contents.md` when it is first committed.

The crates.io package is named `netsuke-build`, while its library and binary
targets remain named `netsuke`. Keep command-line help, manual pages, release
artefacts, and operating-system packages aligned with the `netsuke` target name
rather than the Cargo package name.
[ADR-007](adr-007-publish-as-netsuke-build.md) records the decision, and the
[developer guide](developers-guide.md) describes how the build script, release
packaging, and `cargo binstall` metadata honour it.

Place feature files in `tests/features/` unless the behaviour depends on
Unix-specific platform contracts, in which case use `tests/features_unix/`.
Place generated or approved snapshot files under the existing `src/snapshots/`
or `tests/snapshots/` hierarchy that matches the test owner.

Netsuke runtime state belongs under `.netsuke/` in the effective working
directory, never in the repository layout itself. In particular,
`.netsuke/dyndep` contains immutable content-addressed sidecars for serial
dependencies and `.netsuke/serial` is a reserved generated-gate namespace;
manifest outputs must not claim either path. Sidecar-capable commands retain
the current bundle, at most 32 obsolete `.dd` files, and 1 MiB of obsolete
`.dd` bytes. They remove stale `.tmp` files while holding the exclusive
directory lease; `clean` prunes only after successful `ninja -t clean`. An
older arbitrary `generate --output` manifest may therefore need regeneration
after a later command. See [ADR-012](adr-012-bound-dyndep-sidecar-retention.md).
