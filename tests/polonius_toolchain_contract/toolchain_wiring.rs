//! Assertions describing how one workflow job obtains its Rust channel.
//!
//! A job can take the pin from `rust-toolchain.toml` through the resolve step,
//! or not at all, and each shape has its own failure mode. These helpers read one already-parsed job and assert the shape its
//! [`ToolchainPin`] arm names.

use anyhow::{Context, Result, ensure};
use serde_yaml::Value as YamlValue;

use super::{ENV_TOOLCHAIN_INPUT, RESOLVE_TOOLCHAIN_ACTION, ToolchainPin, yaml_str};

/// Name one workflow job for diagnostics: its file path and job identifier.
///
/// The two strings always travel together and are only ever read to build a
/// message, so grouping them keeps the helpers that describe a job from
/// growing argument lists of interchangeable strings.
#[derive(Clone, Copy)]
pub struct JobRef<'a> {
    /// The workflow file the job is declared in.
    pub path: &'a str,
    /// The job's identifier within that file.
    pub job: &'a str,
}

impl std::fmt::Display for JobRef<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} job {}", self.path, self.job)
    }
}

/// Find the step invoking the shared action named by `expected_action`.
pub fn find_shared_action<'a>(
    steps: &'a [YamlValue],
    named: JobRef<'_>,
    expected_action: &str,
) -> Result<&'a YamlValue> {
    steps
        .iter()
        .find(|step| yaml_str(step, &["uses"]) == Some(expected_action))
        .with_context(|| format!("{named} should use {expected_action}"))
}

/// Where one job's toolchain plumbing reads from, as the workflow declares it.
pub struct ToolchainWiring<'a> {
    /// The job the assertions below describe.
    pub named: JobRef<'a>,
    /// The job's `NETSUKE_RUST_TOOLCHAIN` value, or `None` when it omits one.
    pub toolchain_env: Option<&'a str>,
    /// What the shared action receives as its `toolchain` input, or `None`.
    pub toolchain_input: Option<&'a str>,
    /// The job's steps, in order.
    pub steps: &'a [YamlValue],
    /// The step that invokes the shared action.
    pub shared_action: &'a YamlValue,
}

/// Assert the job obtains its channel the way `pin` describes.
pub fn ensure_toolchain_pin(wiring: &ToolchainWiring<'_>, pin: ToolchainPin) -> Result<()> {
    let ToolchainWiring {
        named,
        toolchain_env,
        toolchain_input,
        steps,
        shared_action,
    } = *wiring;
    match pin {
        ToolchainPin::FromFile => {
            // The channel is a property of `rust-toolchain.toml` alone: a job
            // that declares its own copy restates the date Dependabot moves.
            ensure!(
                toolchain_env.is_none(),
                "{named} declares NETSUKE_RUST_TOOLCHAIN {toolchain_env:?}; the \
                 resolve step must export it from rust-toolchain.toml instead"
            );
            // `env` resolves per job, so a job that reads the variable without
            // exporting it itself gets the empty string, and the shared action
            // would then take its rust-toolchain-file arm. The export has to
            // come before the step that reads it.
            let resolve = steps
                .iter()
                .position(|step| yaml_str(step, &["uses"]) == Some(RESOLVE_TOOLCHAIN_ACTION))
                .with_context(|| format!("{named} should run {RESOLVE_TOOLCHAIN_ACTION}"))?;
            let setup = steps
                .iter()
                .position(|step| std::ptr::eq(step, shared_action))
                .with_context(|| format!("{named} should contain its shared-action step"))?;
            ensure!(
                resolve < setup,
                "{named} runs {RESOLVE_TOOLCHAIN_ACTION} after the step that reads the pin"
            );
            ensure_pin_is_read(named, toolchain_input)
        }
        ToolchainPin::Absent => {
            ensure!(
                toolchain_env.is_none(),
                "{named} should not override NETSUKE_RUST_TOOLCHAIN, found {toolchain_env:?}"
            );
            ensure!(
                toolchain_input != Some(ENV_TOOLCHAIN_INPUT),
                "{named} reads {ENV_TOOLCHAIN_INPUT} without pinning NETSUKE_RUST_TOOLCHAIN, \
                 so the expression resolves to the empty string"
            );
            // This arm makes two assertions rather than returning a helper's
            // result, so it supplies the arm's value itself; `ensure!` expands
            // to an `if` without an `else` and is worth `()` on success.
            Ok(())
        }
    }
}

/// Assert the shared action is told to use the pinned channel: an empty
/// `toolchain` input selects the rust-toolchain-file arm and ignores the pin.
fn ensure_pin_is_read(named: JobRef<'_>, toolchain_input: Option<&str>) -> Result<()> {
    ensure!(
        toolchain_input == Some(ENV_TOOLCHAIN_INPUT),
        "{named} declares NETSUKE_RUST_TOOLCHAIN but passes {toolchain_input:?} to \
         `Setup Rust`, expected {ENV_TOOLCHAIN_INPUT:?}"
    );
    Ok(())
}
