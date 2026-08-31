# Test isolation for Ninja selection

Netsuke resolves the Ninja programme from `NETSUKE_NINJA`, then falls back to
`ninja`. Issue [#488](https://github.com/leynos/netsuke/issues/488) made that
lookup an injected `mockable::Env` seam: the resolver consumes an `Env`, the
production boundary supplies `DefaultEnv`, and its unit tests supply `MockEnv`.
Neither `NETSUKE_NINJA` nor `PATH` is mutated in the test harness.

## Test the resolver with `MockEnv`

`resolve_ninja_program_utf8_with` is the module-owned resolver query. Its tests
set one expectation for `NETSUKE_NINJA`, which verifies both the chosen value
and the lookup boundary:

```rust
use camino::Utf8PathBuf;
use mockable::MockEnv;
use std::ffi::OsString;

let mut env = MockEnv::new();
env.expect_os_string()
    .times(1)
    .withf(|key| key == NINJA_ENV)
    .return_const(Some(OsString::from("/opt/ninja")));

assert_eq!(
    resolve_ninja_program_utf8_with(&env),
    Utf8PathBuf::from("/opt/ninja")
);
```

Model an unset override with `None`, an empty override with an empty
`OsString`, and a non-UTF-8 override on platforms that support it. These cases
exercise the real precedence behaviour without depending on runner order or a
process-global guard.

## Test runner execution with an explicit programme

Runner tests that are about command dispatch or Ninja invocation do not need to
exercise environment resolution. Inject the resolved fake executable through
`runner::run_with_ninja_program` instead:

```rust
use netsuke::runner;
use test_support::fake_ninja;

let (_ninja_dir, ninja_path) = fake_ninja(0)?;
runner::run_with_ninja_program(&cli, output_prefs, &ninja_path)?;
# Ok::<(), anyhow::Error>(())
```

Keep the returned temporary directory alive until the runner finishes. To
control the environment for commands Ninja launches, pass a `CommandEnv` to
`runner::run_ninja_with` or `runner::run_ninja_tool_with`. A child `PATH`
affects commands launched by Ninja; it does not replace an already selected,
resolved Ninja programme.

## End-to-end child processes

An end-to-end test may configure `NETSUKE_NINJA` or `PATH` on its spawned
command. Clear the inherited child environment first, then add only the values
needed by the scenario:

```rust
let mut command = assert_cmd::Command::new(netsuke_executable);
command
    .env_clear()
    .env("HOME", isolated_home)
    .env("PATH", controlled_path)
    .env("NETSUKE_NINJA", ninja_path);
```

This is permitted because `Command::env` changes only the child process. BDD
steps record their values in `TestWorld::env_vars_forward`, and
`build_netsuke_command` applies that map after `env_clear()`.

## Rules

- Use `MockEnv` to test `NETSUKE_NINJA` resolution.
- Use an explicit programme path to test in-process runner behaviour.
- Use `Command::env` only to configure a hermetic child process.
- Do not call `std::env::set_var` or `std::env::remove_var` in a test harness.
- Do not use `EnvLock`, a guard, or test serialization to conceal global state.
