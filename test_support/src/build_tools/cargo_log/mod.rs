//! A fake `cargo` that records how it was called.
//!
//! The build and gate recipes are worth asserting on directly: they compose
//! `RUSTFLAGS` and prepend the install prefix to `PATH`. Recording the
//! arguments and environment each invocation received turns those into
//! checkable facts rather than assumptions. `RUSTFLAGS` in particular cannot be
//! inferred from a command line: Cargo reads it from the environment, and an
//! externally set value silently displaces every `rustflags` table in
//! `.cargo/config.toml`.

use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use std::io::ErrorKind;

use super::Sandbox;
use crate::fs;

mod invocation;

pub use invocation::CargoInvocation;

/// Separator between records in the log. Chosen so it cannot collide with an
/// argument or a path.
///
/// Visible to the child module because the reader and the writer must agree on
/// it: a test that built a record with a different separator would be checking
/// a format the fake never emits.
pub(super) const RECORD_SEPARATOR: &str = "--build-tools-record--";

/// A `cargo` stand-in installed on the sandbox `PATH`, plus its log.
pub struct RecordingCargo {
    /// The fake `cargo` executable installed in the sandbox.
    executable: Utf8PathBuf,
    /// The invocation log file under the sandbox's `HOME`.
    log: Utf8PathBuf,
}

impl RecordingCargo {
    /// Install the fake into the sandbox's `bin`, logging under its `HOME`.
    ///
    /// The fake reports whether its target directory already existed and then
    /// creates it. That makes a benchmark's clean-then-incremental cycle
    /// directly observable: the clean pass sees `absent`, because the harness
    /// removed the directory, and the incremental pass that follows sees
    /// `present`.
    ///
    /// # Errors
    ///
    /// Returns an error if the Cargo logger cannot be installed in the sandbox.
    pub fn install(sandbox: &Sandbox) -> Result<Self> {
        let log = sandbox.home().join("cargo-invocations.log");
        let body = format!(
            concat!(
                "target_dir=\"${{CARGO_TARGET_DIR:-}}\"\n",
                "if [ -n \"$target_dir\" ] && [ -d \"$target_dir\" ]; then\n",
                "  target_state=present\n",
                "else\n",
                "  target_state=absent\n",
                "fi\n",
                "[ -z \"$target_dir\" ] || mkdir -p \"$target_dir\"\n",
                "touch_file=\"${{BENCH_TOUCH_FILE:-}}\"\n",
                "if [ -n \"$touch_file\" ] && [ -e \"$touch_file\" ]; then\n",
                "  touch_mtime=$(stat -c %Y \"$touch_file\")\n",
                "else\n",
                "  touch_mtime=\n",
                "fi\n",
                "{{\n",
                "  echo '{separator}'\n",
                "  printf 'arguments\\t%s\\n' \"$*\"\n",
                "  printf 'toolchain\\t%s\\n' \"${{RUSTUP_TOOLCHAIN:-}}\"\n",
                "  printf 'path\\t%s\\n' \"${{PATH:-}}\"\n",
                "  printf 'rustflags\\t%s\\n' \"${{RUSTFLAGS-}}\"\n",
                "  printf 'rustflags_set\\t%s\\n' \"${{RUSTFLAGS+yes}}\"\n",
                "  printf 'wrapper\\t%s\\n' \"${{RUSTC_WRAPPER-}}\"\n",
                "  printf 'wrapper_set\\t%s\\n' \"${{RUSTC_WRAPPER+yes}}\"\n",
                "  printf 'workspace_wrapper\\t%s\\n' \"${{RUSTC_WORKSPACE_WRAPPER-}}\"\n",
                "  printf 'workspace_wrapper_set\\t%s\\n' \"${{RUSTC_WORKSPACE_WRAPPER+yes}}\"\n",
                "  printf 'encoded_rustflags\\t%s\\n' \"${{CARGO_ENCODED_RUSTFLAGS-}}\"\n",
                "  printf 'encoded_rustflags_set\\t%s\\n' \"${{CARGO_ENCODED_RUSTFLAGS+yes}}\"\n",
                "  printf 'build_dir\\t%s\\n' \"${{CARGO_BUILD_BUILD_DIR:-}}\"\n",
                "  printf 'target_dir\\t%s\\n' \"$target_dir\"\n",
                "  printf 'target_state\\t%s\\n' \"$target_state\"\n",
                "  printf 'touch_mtime\\t%s\\n' \"$touch_mtime\"\n",
                "}} >> '{log}'\n",
                "exit 0"
            ),
            separator = RECORD_SEPARATOR,
            log = log,
        );
        let executable = sandbox.write_fake(&sandbox.bin(), "cargo", &body)?;
        Ok(Self { executable, log })
    }

    /// The fake's path, for passing as the `CARGO` Make variable.
    #[must_use]
    pub fn executable(&self) -> &Utf8Path {
        &self.executable
    }

    /// Every invocation recorded so far, in order.
    ///
    /// A log that was never created means the fake was never called, reported
    /// as an empty list so a test can assert on that directly. Any other read
    /// failure is propagated: a permission or I/O error must not masquerade as
    /// "cargo did not run", which is exactly the conclusion some tests draw.
    ///
    /// # Errors
    ///
    /// Returns an error if the invocation log cannot be read or parsed.
    pub fn invocations(&self) -> Result<Vec<CargoInvocation>> {
        let text = match fs::read_to_string(&self.log) {
            Ok(text) => text,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => {
                return Err(error).with_context(|| format!("read {}", self.log));
            }
        };
        text.split(RECORD_SEPARATOR)
            .filter(|record| !record.trim().is_empty())
            .map(CargoInvocation::parse)
            .collect()
    }

    /// The single recorded invocation, or an error naming how many there were.
    ///
    /// # Errors
    ///
    /// Returns an error unless the log contains exactly one valid invocation.
    pub fn sole_invocation(&self) -> Result<CargoInvocation> {
        let mut invocations = self.invocations()?;
        match invocations.len() {
            1 => Ok(invocations.remove(0)),
            count => anyhow::bail!("expected exactly one cargo invocation, recorded {count}"),
        }
    }
}

/// Whether the invocation's target directory existed when it started.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TargetState {
    /// A clean build: the harness had removed the directory.
    Absent,
    /// An incremental build: a previous pass left the directory behind.
    Present,
    /// No `CARGO_TARGET_DIR` was set, so the question does not arise.
    Unset,
}
