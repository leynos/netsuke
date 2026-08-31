# Reliable testing in Rust via dependency injection

Environment variables are process-global state. A test that changes the harness
environment can race another test, leak a value after a failure, or make a
passing result depend on test order. Netsuke therefore injects environment
input; it never changes the harness process to arrange a test. The policy and
its three seam shapes are recorded in
[ADR-008](adr-008-environment-seam-taxonomy.md).

## Use `mockable::Env` at an environment boundary

Use `mockable::Env` when a boundary reads a precedence ladder, is exercised by
several tests, or is likely to acquire more environment input. Keep the trait
at the module-owned boundary, rather than introducing a shared environment
service. Production supplies `mockable::DefaultEnv`; tests supply a configured
`mockable::MockEnv`.

The Ninja resolver demonstrates the pattern. Its testable query receives the
reader, while the public production function supplies the default adapter:

```rust
use camino::Utf8PathBuf;
use mockable::{DefaultEnv, Env};
use std::path::PathBuf;

fn resolve_program_with(env: &impl Env) -> Utf8PathBuf {
    match env.os_string("NETSUKE_NINJA") {
        Some(value) if !value.is_empty() => Utf8PathBuf::from_path_buf(PathBuf::from(value))
            .unwrap_or_else(|_| Utf8PathBuf::from("ninja")),
        _ => Utf8PathBuf::from("ninja"),
    }
}

fn resolve_program() -> Utf8PathBuf {
    resolve_program_with(&DefaultEnv)
}
```

The unit test controls the response without writing an environment variable:

```rust
use mockable::MockEnv;
use std::ffi::OsString;

let mut env = MockEnv::new();
env.expect_os_string()
    .times(1)
    .withf(|key| key == "NETSUKE_NINJA")
    .return_const(Some(OsString::from("/opt/ninja")));

assert_eq!(resolve_program_with(&env), Utf8PathBuf::from("/opt/ninja"));
```

This pattern makes the lookup count and variable name part of the test
contract. A missing, empty, or non-UTF-8 value is another mock response, not a
reason to modify the process environment. Use a narrow closure instead when a
single caller reads one variable, and use the `EnvReader` `Arc` closure where a
registered `Send + Sync` callback requires it; ADR-008 defines those choices.

## Configure child processes explicitly

End-to-end tests are the sole exception because they configure a separate
process, not the harness. Clear the child's inherited environment and add only
the values the command requires:

```rust
let mut command = assert_cmd::Command::new(netsuke_executable);
command
    .env_clear()
    .env("HOME", isolated_home)
    .env("PATH", controlled_path)
    .env("NETSUKE_NINJA", ninja_path);
```

`Command::env` affects only the spawned command. It is not an endorsement of
`std::env::set_var` or `std::env::remove_var` in the test process.

## Retired anti-pattern: process-wide guards

`EnvLock`, `EnvVarGuard`, and equivalent scope guards are not alternative
testing strategies. They were abandoned because a lock merely serializes shared
mutable state: the dependency remains ambient, parallel and property tests lose
concurrency, and unrelated code can still observe the temporary value. Do not
add a lock, a `serial_test` annotation, or a guard to make in-process
environment mutation appear safe. Change the boundary to accept the injected
value instead.

## Checklist

- Pass environment input through a module-owned seam.
- Use `DefaultEnv` only at the production composition boundary.
- Use `MockEnv` to describe each unit-test response.
- Use `Command::env` only to configure an isolated child process.
- Never mutate or lock the harness process environment.
