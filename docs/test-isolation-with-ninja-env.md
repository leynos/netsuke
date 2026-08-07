# Test isolation for Ninja selection

Netsuke resolves its production Ninja binary from `NETSUKE_NINJA` before
falling back to `ninja` on `PATH`. Tests must not mutate either variable in the
harness process. Choose the isolation seam that matches the boundary under test.

## In-process runner tests

Call `runner::run_with_ninja_program` with the fake executable path. This
exercises command dispatch and Ninja invocation without changing global state:

```rust
use netsuke::runner;
use test_support::fake_ninja;

let (_ninja_dir, ninja_path) = fake_ninja(0)?;
runner::run_with_ninja_program(&cli, output_prefs, &ninja_path)?;
# Ok::<(), anyhow::Error>(())
```

Keep the returned temporary directory alive until the runner finishes.

To control the environment of the spawned Ninja process itself, pass a
`CommandEnv` through `runner::run_ninja_with` or `runner::run_ninja_tool_with`.
A `PATH` injected this way affects the commands Ninja launches, not which
Ninja program runs — provided the program is an absolute or otherwise
resolved path. Selection happens first, via `NETSUKE_NINJA` or an explicitly
injected programme path, and the resolved program is passed to `Command`
as given; a bare relative name such as `ninja` would still be looked up in
the child's `PATH` on Unix, so callers pass resolved paths.

## End-to-end tests

End-to-end tests may set `NETSUKE_NINJA` or `PATH` on the spawned command.
Clear the inherited environment first, then add only the values required by the
scenario:

```rust
let mut command = assert_cmd::Command::new(netsuke_executable);
command
    .env_clear()
    .env("HOME", isolated_home)
    .env("PATH", controlled_path)
    .env("NETSUKE_NINJA", ninja_path);
```

This preserves the production precedence rule while confining all mutation to
the child process. BDD steps use `mutate_env_var` to populate
`TestWorld::env_vars_forward`; `build_netsuke_command` applies that map after
calling `env_clear()`.

## Rules

- Do inject the Ninja programme path for in-process runner tests.
- Do use `Command::env` for child-process integration tests.
- Do keep `PATH` and `HOME` explicit in hermetic subprocess environments.
- Do not call `std::env::set_var` or `std::env::remove_var` in a test harness.
- Do not serialize tests to make process-environment mutation appear safe.
