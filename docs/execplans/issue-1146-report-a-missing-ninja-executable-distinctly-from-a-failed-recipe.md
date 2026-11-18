# Issue 1146: Report a missing Ninja executable distinctly from a failed recipe

Status: IN PROGRESS

## Purpose / big picture

Issue [#1146](https://github.com/leynos/netsuke/issues/1146) reports that two
unrelated Ninja failures end with the same summary, which names the temporary
Ninja build file rather than the problem:

```text
✖ Error: running ninja with build file /tmp/netsuke.0kBQnM.ninja
```

A missing or non-executable Ninja program and a recipe that exited
unsuccessfully are different faults. Each must reach the user as a typed,
localized diagnostic with a stable code. The spawn diagnostic names the program
Netsuke tried and offers a remediation hint; the exit diagnostic states that
Ninja reported a failure and gives its exit status. Neither summary names the
temporary build file.

## Constraints

- Keep every public `io::Result` function unchanged in signature and behaviour,
  including `ErrorKind::NotFound`, `raw_os_error()`, and the legacy
  `ninja exited with …` text.
- Classify each failure where it occurs with a crate-private typed result.
- Do not add `running {program} with build file {path}` context to either
  classified `RunnerError` variant; retain it for other I/O failures.
- Keep command-list attribution visible in the new cause chain.
- Do not edit `public_api_fixture/` or the completed issue-83 ExecPlan.
- Preserve the policy that a forwarding loss does not override the child
  result.

## Progress

- [x] Re-read the issue, its plan, and the current process/diagnostic layers.
- [x] Phase 1: typed `NinjaInvocationError` in the process layer, with
  classification tests for a missing program, an unsuccessful exit, and a
  signal-terminated Ninja.
- [x] Phase 2: localized `RunnerError` variants, both catalogues updated, and
  human and JSON renderer tests.
- [x] Phase 3a: CLI coverage for missing, absent-override, non-executable, and
  failing Ninja across `build` and `clean`, both progress policies, and human
  and JSON modes; the command-list and tool-subcommand tests updated.
- [x] Phase 3b: quick start, users' guide, design document, and developers'
  guide updated.
- [ ] Phase 3c: run the full gate set and clear CodeRabbit review.
  - [x] `make check-fmt`, `make lint`, and `make doc-coverage` (98.29%).
  - [x] `make test` was run far enough to expose one regression in the bounded
    legacy-recipe telemetry classifier; both new `RunnerError` variants now map
    to the existing `ninja_io` category, with a unit test for each.
  - [x] `make test`, the workflow, documentation, and Markdown gates, and
    `make typecheck` pass on the complete change (6041 tests).
  - [x] CodeRabbit reported six findings and one unverified finding; all seven
    are applied:
    - the JSON and human diagnostic tests derive the platform-defined
      `ExitStatus` wording (`exit status` on Unix, `exit code` on Windows)
      from the status value instead of hard-coding one form;
    - the developers' guide describes `into_io_error` accurately, and the
      users' guide describes how each mode handles Ninja's output;
    - the Arabic help isolates the literal `PATH` and `NETSUKE_NINJA` with
      U+200E, the mark the catalogue already uses for embedded Latin tokens;
    - the Vietnamese summary names a general build failure rather than a
      compilation failure;
    - the quick start shows the PowerShell `$env:NETSUKE_NINJA` form beside
      the POSIX one;
    - the `exit_status` test helpers take one `u8`, so only the platform
      adapter differs between the `cfg` branches.
  - [ ] Re-run the gates for these review fixes.
  - [ ] Push and open the draft pull request.

## Plan of work

### Phase 1: Typed Ninja invocation failures

Add a crate-private `NinjaInvocationError` with `Spawn { program, source }`,
`Exit(NinjaExitError)`, and `Io(io::Error)`. `NinjaExitError` stores the full
`ExitStatus`, the operation, and optional pre-rendered attribution. Reproduce
`ninja exited with {status}…` exactly in its `Display`. Classify spawn failures
in `run_command_and_stream_with_context`, unsuccessful exits in
`child_exit.rs`, and missing-pipe and wait failures in `output_forwarding`.
Internal entry points return the typed result; public adapters convert it to
`io::Error`.

### Phase 2: Runner diagnostics

Add four Fluent keys with consistent variables to all 35 catalogues. Add
`RunnerError::NinjaSpawnFailed` (code `netsuke::runner::ninja_spawn_failed`,
escaped program, localized summary and help, `io::Error` source) and
`RunnerError::NinjaFailed` (code `netsuke::runner::ninja_failed`, status,
operation, localized status summary, `NinjaExitError` source). Map typed spawn
and exit failures in `execute_build` and `execute_ninja_tool` on both progress
branches without temporary-file context. Add human and JSON renderer tests.

### Phase 3: End-to-end coverage and documentation

Cover missing, non-executable, and failing Ninja through the CLI for `build` and
`clean`, with progress always and never, in human and JSON modes. Update the
quick start, users' guide, design document, and developers' guide. Run the full
gate set.

## Validation and acceptance

- `make check-fmt`, `make lint`, `make typecheck`, `make doc-coverage`,
  `make test`, `make fmt`, `make markdownlint`, and `make nixie` pass.
- `tests/assert_cmd_tests/runner_diagnostics.rs` proves spawn and exit
  behaviour in human output with no temporary-file path.
- `tests/logging_stderr/json.rs` proves one JSON diagnostic document with the
  new codes, help, and causes.
- The legacy process tests and public API fixture pass unchanged.

## Decision log

- Classify at the failure site rather than inferring from `io::ErrorKind`, which
  cannot distinguish a spawn failure from other `NotFound` sources.
- Use one spawn code for missing and non-executable programs; the OS cause
  distinguishes them without overstating the diagnosis.
- Record the decision in `docs/netsuke-design.md` rather than a new ADR,
  because it applies existing ADR-0035 and ADR-0046 direction.
- Derive the expected status text from `ExitStatus` in tests. Its `Display`
  implementation is platform-defined, so a hard-coded `exit status: 7` would
  pass on Unix and fail on Windows without naming the real cause.
- Mark embedded Latin identifiers in Arabic text with U+200E rather than
  restating them in a separate clause; the catalogue already uses that mark for
  `--rule` and `--limit`.

## Surprises & discoveries

- `NinjaExitError` must be nameable by the public `RunnerError` variant; it is
  re-exported from `runner` and documented. Its constructor is public because
  the `netsuke` binary's renderer tests live outside the library crate and
  cannot see a `pub(crate)` item.
- Fluent wraps every placeable in bidi isolation controls, so text assertions
  compare with U+2068 and U+2069 stripped rather than reproducing them.
- `instrument_ninja_build` was generic over `io::Result`; it now carries
  `<T, E>` and records `error_category = "io"` from the error's own chain, so
  the telemetry contract is unchanged while the typed error survives.
- The command-list snapshot is unchanged: its helper slices the cause line
  from `ninja exited with`, and the attribution remains a cause of the new exit
  diagnostic.
- The bounded `failure_category` label on `LEGACY_RECIPE_EXECUTIONS_TOTAL`
  classified every `RunnerError` as `manifest`. That was harmless while Ninja
  failures arrived as bare `io::Error`s, but the new `NinjaSpawnFailed` and
  `NinjaFailed` variants would have been blamed on the manifest stage. Both now
  map to the existing `ninja_io` label, so the fixed label set does not grow.
- The Kani proof-scope contract computes a harness closure that treats any
  `impl` header naming a reached type as reaching the implementation. A manual
  `impl std::error::Error` or `impl From<…> for io::Error` in the new module
  therefore pulled `src/runner` into the closure and cascaded across the
  workspace. Deriving `thiserror::Error` and keeping one inherent conversion
  method keeps the closure identical to the base commit.
- Whitaker compiles the workspace with `nightly-2026-05-28`, older than the
  project pin. `Option::map_or_default`, which Clippy suggests, is still gated
  there; `Option::map_or_else` satisfies both toolchains.
- `make lint` and `make test` must be re-run after any source edit: the first
  full `make test` stopped after the first failing binary, so a green summary
  from a partial run says nothing about the tests it never scheduled.

## Outcomes & retrospective

The two faults now have separate codes, summaries, and causes.
`ninja_spawn_failed` names the program and offers the install and
`NETSUKE_NINJA` remedies; `ninja_failed` reports Ninja's status and the
operation. Neither summary repeats the temporary build file. Command-list
attribution and the legacy `ninja exited with …` cause survive inside the exit
diagnostic, and the public `io::Result` contracts are unchanged.

The main lesson is that the fault must be classified where it is produced. By
the time an `io::Error` reaches the runner, `ErrorKind::NotFound` cannot say
whether the program was absent or a pipe was missing, which is what forced the
old catch-all context. A typed result at the process seam is what let the
runner choose a diagnostic without guessing.
