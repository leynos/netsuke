//! Acceptance tests for trusted shell definitions.
//!
//! Accepting a definition is not enough on its own: startup must also let the
//! manifest compile, and must not require the named program until a recipe
//! actually runs it.

use super::{Startup, definition_body, stderr_of};
use anyhow::{Result, ensure};
// Only the Unix-only stub-Ninja test attaches context to a failure, and only it
// touches the filesystem helper, so both imports are gated to keep the module
// warning-free on every platform.
#[cfg(unix)]
use anyhow::Context;
#[cfg(unix)]
use test_support::fs as test_fs;

/// Prove a trusted definition does not block a build driven by a stub Ninja.
///
/// `NETSUKE_NINJA` names the program `build` launches, so a stub script
/// records the invocation without needing Ninja, a recipe shell, or any
/// configured executable to exist. The test is Unix-only because the stub is a
/// script; the existing `FakeNinjaFixture` tests take the same approach.
#[cfg(unix)]
#[test]
fn trusted_definition_builds_through_a_controlled_ninja() -> Result<()> {
    let startup = Startup::new()?;
    startup.write("user/netsuke/config.toml", &definition_body("dash"))?;
    let marker = startup.path().join("ninja-ran");
    let stub = startup.write(
        "stub/ninja",
        &format!("#!/bin/sh\n: > \"{}\"\nexit 0\n", marker.display()),
    )?;
    test_fs::set_mode(&stub, 0o755).context("make the stub Ninja executable")?;

    let output = startup
        .command(&["build", "hello"])
        .env("NETSUKE_NINJA", &stub)
        .output()
        .context("run netsuke build with a stub Ninja")?;

    ensure!(
        output.status.success(),
        "a trusted definition should not block a build, got: {}",
        stderr_of(&output)
    );
    ensure!(
        marker.exists(),
        "the stub Ninja should have been launched at {}",
        marker.display()
    );
    Ok(())
}

#[test]
fn trusted_definition_loads_without_the_named_program() -> Result<()> {
    let startup = Startup::new()?;
    startup.write("user/netsuke/config.toml", &definition_body("dash"))?;

    let output = startup.generate()?;

    ensure!(
        output.status.success(),
        "a trusted definition should not require the named program at load time, got: {}",
        stderr_of(&output)
    );
    ensure!(
        startup.sentinel_exists(),
        "a trusted definition should allow the manifest to compile"
    );
    Ok(())
}
