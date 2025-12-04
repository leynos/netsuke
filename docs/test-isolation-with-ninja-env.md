# Test isolation with `NINJA_ENV`

Netsuke resolves the Ninja binary from the `NINJA_ENV` environment variable
before falling back to `ninja` on `PATH`. Tests should override `NINJA_ENV`
instead of mutating `PATH` so they can execute in parallel without stepping on
each other's environment.

## Why prefer `NINJA_ENV`

- Mutating `PATH` is global and risks races when tests run concurrently.
- `override_ninja_env` scopes changes via an `EnvGuard`, restoring the previous
  value even if the test fails.
- Keeping `PATH` untouched avoids coupling to the developer's shell setup.
- `override_ninja_env` also holds a process-wide lock for the guard's lifetime,
  preventing parallel tests from interleaving `NINJA_ENV` mutations.

## Fixture pattern

Use a fixture to create a fake Ninja executable and point `NINJA_ENV` at it.
The fixture keeps the temporary directory alive for the test duration and
automatically restores the environment on drop.

```rust
use rstest::fixture;
use test_support::env::{NinjaEnvGuard, SystemEnv, override_ninja_env};
use test_support::fake_ninja;

#[fixture]
fn ninja_in_env() -> anyhow::Result<(tempfile::TempDir, NinjaEnvGuard)> {
    let (ninja_dir, ninja_path) = fake_ninja(0)?;
    let env = SystemEnv::new();
    let guard = override_ninja_env(&env, ninja_path.as_path());
    Ok((ninja_dir, guard))
}
```

Inject the fixture into tests that need a controlled Ninja binary:

```rust
#[rstest]
fn run_build_uses_fake_ninja(
    (_, _guard): (tempfile::TempDir, NinjaEnvGuard),
) {
    // run the command-line interface (CLI) here; the guard restores NINJA_ENV
    // on drop
}
```

## Dos and don'ts

- Do keep the guard alive until after the CLI invocation so `NINJA_ENV` stays
  set.
- Do avoid explicit `drop` calls for `PathBuf` values; they do not own external
  resources.
- Don't add `#[serial]` purely to protect `PATH` mutations; prefer the fixture
  above to keep tests parallel-friendly.

## Precedence over `PATH`

`NINJA_ENV` should override any `ninja` found on `PATH`. When asserting this in
tests, place a failing fake Ninja on `PATH` with `prepend_dir_to_path` and set
`NINJA_ENV` to a working fake Ninja via `override_ninja_env`. The test should
pass only if `NINJA_ENV` is respected.
