# Architecture decision record (ADR): Require UTF-8 Ninja invocation paths

## Status

Accepted.

## Date

2026-08-30

## Context and problem statement

The Ninja invocation chain historically accepted `std::path::Path` for the
build file, working directory and resolved Ninja executable. Internally, the
runner already uses `camino::Utf8Path` for most filesystem work, but conversion
between the two types occurred at several points. That let a non-UTF-8 CLI path
travel through the runner until an unrelated later operation produced the
diagnostic.

`AGENTS.md` and `CRUSH.md` establish the informal convention to use `camino`
for filesystem paths. The design document gives the same boundary rationale:
filesystem concerns enter at the manifest-to-IR boundary, and the Ninja process
adapter owns command construction. Existing error-wrapping conventions also
distinguish encoding failures at their boundary, including `ResolveError`, the
`RUNNER_*_UTF8` keys and the `MANIFEST_*_NON_UTF8` keys.

`src/cli/discovery_paths.rs` is a deliberate `std::path` carve-out. It must
inspect host-provided discovery paths, whose encoding Netsuke cannot constrain.
Child-process environment payloads likewise remain `OsStr` and `OsString`,
because environment-variable values genuinely need not be UTF-8.

## Decision

Require UTF-8 paths throughout the Ninja invocation chain. `Cli.file`,
`Cli.directory`, the resolved Ninja program, generated temporary build files,
Ninja request bundles and runner adapter interfaces use `Utf8Path` or
`Utf8PathBuf`. Conversion to `std::path::Path` occurs only at the lossless
`std::process::Command` call boundary.

Reject a non-UTF-8 build-file or working-directory value at the earliest local
boundary: custom Clap parsers for command-line values and post-merge validation
for configuration-file and environment values. Each path receives a localized,
specific diagnostic instead of failing later in the runner.

`path_with_dir_prepended` does not exist in this repository. It refers to the
already-removed `prepend_dir_to_path` helper, so it requires no migration.

## Rationale

- **One runner vocabulary.** A complete migration avoids two conventions in a
  single invocation chain and makes the UTF-8 invariant visible in signatures.
- **Early, explainable failure.** CLI and configuration boundaries can name the
  invalid input and localize the error before any manifest or process work.
- **Narrow platform boundary.** `Command` continues to receive `Path` only
  where the `Utf8Path` conversion is lossless, preserving interoperability with
  the standard process API without admitting non-UTF-8 input downstream.
- **Deliberate exceptions.** Discovery and child-environment payloads retain
  their platform-native representations because they model external data rather
  than Netsuke build paths.

## Consequences

- Build files and working directories whose names are not valid UTF-8 are not
  supported by Netsuke and fail before runner execution.
- New Ninja invocation APIs must accept `Utf8Path` values unless they are an
  explicit host-boundary adapter with an early encoding diagnostic.
- Environment variables continue to use `OsStr` and `OsString`; this decision
  does not impose a UTF-8 requirement on child-process payloads.

## Alternatives considered

- **Retain `std::path` throughout the runner.** Rejected because it obscures
  the established `camino` convention and spreads repeated conversions and
  delayed encoding failure paths through the implementation.
- **Migrate only the request bundles added for #497.** Rejected because the
  existing and new runner entry points deliberately mirror one another; a
  partial migration would create inconsistent conventions in one module.
- **Continue accepting non-UTF-8 build paths.** Rejected because it leaves
  support accidental and diagnostics dependent on arbitrary downstream code.

## Implementation references

- Path convention: [`AGENTS.md`](../AGENTS.md) and [`CRUSH.md`](../CRUSH.md)
- Design boundary: [Netsuke design, section 3](netsuke-design.md#section-3-parsing-and-deserialization-strategy)
  and [section 6](netsuke-design.md#section-6-process-management-and-secure-execution)
- CLI boundary: [`src/cli/`](../src/cli/)
- Ninja process adapter: [`src/runner/process/`](../src/runner/process/)
- Runner adapter: [`src/runner/ninja_process_adapter.rs`](../src/runner/ninja_process_adapter.rs)
