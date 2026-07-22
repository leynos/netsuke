# Issue 83: Support overriding the Ninja executable

## Purpose

This plan records the implementation status for issue #83, which lets users
select the Ninja executable with the `NETSUKE_NINJA` environment variable.
Success is observable when `netsuke build` selects every non-empty UTF-8
`NETSUKE_NINJA` value, even if spawning that executable later fails, falls back
to `ninja` only when the variable is unset, empty, or non-UTF-8, and logs the
resolved program in verbose command execution output.

## Current implementation

The production resolver is `resolve_ninja_program_utf8_with` in
`src/runner/process/ninja_program.rs`. It checks `NETSUKE_NINJA`, returns a
UTF-8 override when the value is non-empty, and falls back to `ninja` when the
value is unset, empty, or non-UTF-8. Each branch emits a debug-level tracing
event so subscribers can see whether the environment override or default
fallback was selected.

Ninja subprocess execution now uses the shared private helper
`run_ninja_internal` in `src/runner/process/mod.rs`. The helper owns the common
sequence of command creation, caller-specific configuration, and streaming
execution. Command execution telemetry is centralized in
`src/runner/process/command_logging.rs`, where arguments are redacted and the
program path is preserved for traceability.

User-facing documentation in `docs/users-guide.md` and design documentation in
`docs/netsuke-design.md` describe `NETSUKE_NINJA`. Internal process-module
documentation explains the resolution order and command logging boundary.

## Decisions and findings

`NETSUKE_NINJA=` is treated as an invalid override. This keeps the documented
"unset or invalid means fallback" rule true for an empty but set environment
variable and avoids surfacing a confusing `Command::new("")` spawn failure to
users.

The resolver property test now uses an independent oracle for UTF-8 values:
unset and empty values expect the default `ninja`, while non-empty UTF-8 values
expect the generated override. The non-UTF-8 property separately checks that
invalid byte strings fall back.

The binary-level verbose logging tests keep the bounded `cases: 16` proptest
configuration because each case starts the compiled `netsuke` binary and a fake
Ninja subprocess. The shared helper in `tests/logging_stderr_tests.rs` removes
the remaining duplicated fake-Ninja setup and log assertion while leaving the
fixture-specific setup separate.

Command logging intentionally redacts arguments rather than the program string.
The program string is part of the traceability contract for this feature, so it
remains visible in verbose logs and structured tracing events.

## Progress

- Completed resolver support for valid `NETSUKE_NINJA` overrides.
- Completed fallback behaviour for unset, empty, and non-UTF-8 override values.
- Completed verbose-log coverage for default and override command lines.
- Completed property coverage for resolver invariants.
- Completed shared helper extraction for Ninja process execution.
- Completed shared helper extraction for verbose logging integration tests.
- Completed user-facing and internal documentation for the feature.
- Completed all focused validation and project gates for the branch.

## Validation

Focused validation already run during the final review pass:

```sh
cargo test --lib runner::process::tests::resolve_ninja_program_utf8 -- --nocapture
cargo test --test logging_stderr_tests verbose_build_logs -- --nocapture
```

Both focused commands passed. The completed project gates also passed:

```sh
make check-fmt
make lint
make test
make markdownlint
make nixie
```
