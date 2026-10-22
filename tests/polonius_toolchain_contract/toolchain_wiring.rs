//! Assertions describing how one workflow job obtains its Rust channel.
//!
//! A job can take the pin from its own `env`, from a reusable workflow's
//! `rust-toolchain` input, or not at all, and each shape has its own failure
//! mode. These helpers read one already-parsed job and assert the shape its
//! [`ToolchainPin`] arm names.

use anyhow::{Context, Result, ensure};
use serde_yaml::Value as YamlValue;

use super::{ENV_TOOLCHAIN_INPUT, INPUT_TOOLCHAIN_ENV, ToolchainPin, pinned_toolchain, yaml_str};

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
}

/// Assert the job obtains its channel the way `pin` describes.
pub fn ensure_toolchain_pin(wiring: &ToolchainWiring<'_>, pin: ToolchainPin) -> Result<()> {
    let ToolchainWiring {
        named,
        toolchain_env,
        toolchain_input,
    } = *wiring;
    match pin {
        ToolchainPin::JobEnv => {
            let expected = pinned_toolchain()?;
            let toolchain = toolchain_env
                .with_context(|| format!("{named} should pin NETSUKE_RUST_TOOLCHAIN"))?;
            ensure!(
                toolchain == expected,
                "{named} pins {toolchain:?}, but rust-toolchain.toml pins {expected:?}"
            );
            // A pin nothing reads is not a pin: `env` resolves per job, so a
            // step naming a variable only another job declared expands to the
            // empty string rather than failing, and the two disagree in silence.
            ensure_pin_is_read(named, toolchain_input)
        }
        ToolchainPin::CallerInput => {
            ensure!(
                toolchain_env == Some(INPUT_TOOLCHAIN_ENV),
                "{named} should read NETSUKE_RUST_TOOLCHAIN from the caller's \
                 `rust-toolchain` input, found {toolchain_env:?}"
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
