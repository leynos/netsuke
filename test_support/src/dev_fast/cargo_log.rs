//! A fake `cargo` that records how it was called.
//!
//! The `dev-build` and `dev-test` recipes are worth asserting on directly: they
//! select a toolchain, pass a configuration fragment, and prepend the install
//! prefix to `PATH`. Recording the arguments and environment each invocation
//! received turns those into checkable facts rather than assumptions.

use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use std::fs;

use super::Sandbox;

/// Separator between records in the log. Chosen so it cannot collide with an
/// argument or a path.
const RECORD_SEPARATOR: &str = "--dev-fast-record--";

/// A `cargo` stand-in installed on the sandbox `PATH`, plus its log.
pub struct RecordingCargo {
    executable: Utf8PathBuf,
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
    pub fn executable(&self) -> &Utf8Path {
        &self.executable
    }

    /// Every invocation recorded so far, in order.
    ///
    /// An absent log means the fake was never called, which is reported as an
    /// empty list so a test can assert on that directly.
    pub fn invocations(&self) -> Result<Vec<CargoInvocation>> {
        if !self.log.as_std_path().exists() {
            return Ok(Vec::new());
        }
        let text = fs::read_to_string(self.log.as_std_path())
            .with_context(|| format!("read {}", self.log))?;
        Ok(text
            .split(RECORD_SEPARATOR)
            .filter(|record| !record.trim().is_empty())
            .map(CargoInvocation::parse)
            .collect())
    }

    /// The single recorded invocation, or an error naming how many there were.
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

/// One recorded `cargo` call.
pub struct CargoInvocation {
    arguments: Vec<String>,
    toolchain: String,
    path: String,
    target_dir: String,
    target_state: TargetState,
    touch_mtime: Option<i64>,
}

impl CargoInvocation {
    fn parse(record: &str) -> Self {
        let mut invocation = Self {
            arguments: Vec::new(),
            toolchain: String::new(),
            path: String::new(),
            target_dir: String::new(),
            target_state: TargetState::Unset,
            touch_mtime: None,
        };
        for line in record.lines() {
            let Some((field, value)) = line.split_once('\t') else {
                continue;
            };
            match field {
                "arguments" => {
                    invocation.arguments = value.split_whitespace().map(str::to_owned).collect();
                }
                "toolchain" => invocation.toolchain = value.to_owned(),
                "path" => invocation.path = value.to_owned(),
                "target_dir" => invocation.target_dir = value.to_owned(),
                "target_state" => {
                    invocation.target_state = match value {
                        "present" => TargetState::Present,
                        "absent" => TargetState::Absent,
                        _ => TargetState::Unset,
                    };
                }
                "touch_mtime" => invocation.touch_mtime = value.parse().ok(),
                _ => {}
            }
        }
        if invocation.target_dir.is_empty() {
            invocation.target_state = TargetState::Unset;
        }
        invocation
    }

    /// The arguments Cargo received. Recorded via `"$*"`, so an argument
    /// containing whitespace would be split; none of the `dev-fast` recipes
    /// pass one.
    pub fn arguments(&self) -> &[String] {
        &self.arguments
    }

    /// `RUSTUP_TOOLCHAIN` as seen by the invocation, empty when unset.
    pub fn toolchain(&self) -> &str {
        &self.toolchain
    }

    /// Whether `sequence` appears as consecutive arguments.
    ///
    /// Contiguity matters: it distinguishes `--config <fragment>` from the same
    /// two values appearing far apart for unrelated reasons.
    pub fn contains_sequence(&self, sequence: &[&str]) -> bool {
        if sequence.is_empty() || sequence.len() > self.arguments.len() {
            return false;
        }
        self.arguments
            .windows(sequence.len())
            .any(|window| window.iter().zip(sequence).all(|(got, want)| got == want))
    }

    /// Whether `directory` is the first entry on the invocation's `PATH`.
    pub fn path_starts_with(&self, directory: &Utf8Path) -> bool {
        self.path
            .split(':')
            .next()
            .is_some_and(|first| first == directory.as_str())
    }

    /// The recorded `PATH`, for failure messages.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The `CARGO_TARGET_DIR` the invocation was given, empty when unset.
    pub fn target_dir(&self) -> &str {
        &self.target_dir
    }

    /// Whether the target directory existed when the invocation started.
    pub fn target_state(&self) -> TargetState {
        self.target_state
    }

    /// The benchmark touch file's modification time, when one was configured.
    /// A benchmark touches that file between a variant's two passes, so the
    /// value distinguishes the pass that ran before the touch from the one
    /// after it.
    pub fn touch_mtime(&self) -> Option<i64> {
        self.touch_mtime
    }
}
