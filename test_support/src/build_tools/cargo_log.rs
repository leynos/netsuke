//! A fake `cargo` that records how it was called.
//!
//! The build and gate recipes are worth asserting on directly: they compose
//! `RUSTFLAGS` and prepend the install prefix to `PATH`. Recording the
//! arguments and environment each invocation received turns those into
//! checkable facts rather than assumptions. `RUSTFLAGS` in particular cannot be
//! inferred from a command line: Cargo reads it from the environment, and an
//! externally set value silently displaces every `rustflags` table in
//! `.cargo/config.toml`.

use anyhow::{Context, Result, bail};
use camino::{Utf8Path, Utf8PathBuf};
use std::collections::HashMap;
use std::io::ErrorKind;

use super::Sandbox;
use crate::fs;

/// Separator between records in the log. Chosen so it cannot collide with an
/// argument or a path.
const RECORD_SEPARATOR: &str = "--build-tools-record--";

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

/// One recorded `cargo` call.
pub struct CargoInvocation {
    /// Arguments passed to the fake Cargo, split on whitespace.
    arguments: Vec<String>,
    /// `RUSTUP_TOOLCHAIN` in force during the invocation.
    toolchain: String,
    /// The `PATH` the invocation saw, recorded verbatim.
    path: String,
    /// `CARGO_BUILD_BUILD_DIR` as the invocation saw it, empty when unset.
    build_dir: String,
    /// `RUSTFLAGS` as the invocation saw it, or `None` when it was unset.
    ///
    /// Unset and empty are different facts here and must not be conflated: an
    /// empty `RUSTFLAGS` is still an assignment, and an assignment is what
    /// displaces the configuration file's `rustflags` tables, so the release
    /// recipe's correctness turns on which of the two it produced.
    rustflags: Option<String>,
    /// `RUSTC_WRAPPER` as the invocation saw it, or `None` when it was unset.
    ///
    /// Unset and empty differ here for the same reason they do for
    /// `RUSTFLAGS`. A developer's shell commonly exports a compiler wrapper
    /// that chains to `sccache`, and an inherited wrapper turns a benchmark's
    /// clean pass into a cache read. Only an assignment to the empty string
    /// displaces it, so which of the two the recipe produced is the fact worth
    /// recording.
    wrapper: Option<String>,
    /// `RUSTC_WORKSPACE_WRAPPER` as the invocation saw it, `None` when unset.
    ///
    /// Recorded separately because Cargo honours it independently: clearing
    /// only `RUSTC_WRAPPER` still leaves the workspace's own crates wrapped.
    workspace_wrapper: Option<String>,
    /// `CARGO_ENCODED_RUSTFLAGS` as the invocation saw it, `None` when unset.
    ///
    /// Unset and empty are not interchangeable, which is why this records
    /// which one happened rather than just the value. Cargo reads the encoded
    /// variable before `RUSTFLAGS` and takes the first source it finds, so an
    /// *empty* one is not a no-op: it is still a source, and it displaces every
    /// variant's own flags. Only removing the variable leaves `RUSTFLAGS` to
    /// decide the build.
    encoded_rustflags: Option<String>,
    /// The `CARGO_TARGET_DIR` value, empty when unset.
    target_dir: String,
    /// Whether the target directory existed at invocation start.
    target_state: TargetState,
    /// The benchmark touch file's modification time, when one was configured.
    touch_mtime: Option<i64>,
}

impl CargoInvocation {
    /// Parse one record, rejecting anything the fake could not have written.
    ///
    /// A silently-defaulted field would let a harness bug read as a legitimate
    /// observation — an unrecognised `target_state`, say, would look exactly
    /// like "no target directory was set". Every field the fake emits is
    /// therefore required, and every value must be one it can produce.
    fn parse(record: &str) -> Result<Self> {
        let mut fields: HashMap<&str, &str> = HashMap::new();
        for line in record.lines().filter(|line| !line.trim().is_empty()) {
            let (field, value) = line
                .split_once('\t')
                .with_context(|| format!("malformed record line `{line}`"))?;
            fields.insert(field, value);
        }

        let take = |name: &str| -> Result<String> {
            fields
                .get(name)
                .map(|value| (*value).to_owned())
                .with_context(|| format!("record is missing the `{name}` field"))
        };

        let target_dir = take("target_dir")?;
        let target_state = match (target_dir.is_empty(), take("target_state")?.as_str()) {
            (true, _) => TargetState::Unset,
            (false, "present") => TargetState::Present,
            (false, "absent") => TargetState::Absent,
            (false, other) => bail!("unrecognised target_state `{other}`"),
        };

        let raw_mtime = take("touch_mtime")?;
        let touch_mtime = if raw_mtime.is_empty() {
            None
        } else {
            Some(
                raw_mtime
                    .parse()
                    .with_context(|| format!("malformed touch_mtime `{raw_mtime}`"))?,
            )
        };

        // Each pair is a value and the marker that says whether it was
        // assigned at all; an empty value with no marker means "unset", which
        // is a different fact from "assigned to nothing".
        let optional = |value: &str, marker: &str| -> Result<Option<String>> {
            if take(marker)?.is_empty() {
                Ok(None)
            } else {
                Ok(Some(take(value)?))
            }
        };

        let rustflags = optional("rustflags", "rustflags_set")?;
        let wrapper = optional("wrapper", "wrapper_set")?;
        let workspace_wrapper = optional("workspace_wrapper", "workspace_wrapper_set")?;
        let encoded_rustflags = optional("encoded_rustflags", "encoded_rustflags_set")?;

        Ok(Self {
            arguments: take("arguments")?
                .split_whitespace()
                .map(str::to_owned)
                .collect(),
            toolchain: take("toolchain")?,
            path: take("path")?,
            build_dir: take("build_dir")?,
            rustflags,
            wrapper,
            workspace_wrapper,
            encoded_rustflags,
            target_dir,
            target_state,
            touch_mtime,
        })
    }

    /// The arguments Cargo received. Recorded via `"$*"`, so an argument
    /// containing whitespace would be split; none of the build recipes
    /// pass one.
    #[must_use]
    pub fn arguments(&self) -> &[String] {
        &self.arguments
    }

    /// `RUSTUP_TOOLCHAIN` as seen by the invocation, empty when unset.
    #[must_use]
    pub fn toolchain(&self) -> &str {
        &self.toolchain
    }

    /// Whether `sequence` appears as consecutive arguments.
    ///
    /// Contiguity matters: it distinguishes `--config <fragment>` from the same
    /// two values appearing far apart for unrelated reasons.
    #[must_use]
    pub fn contains_sequence(&self, sequence: &[&str]) -> bool {
        if sequence.is_empty() || sequence.len() > self.arguments.len() {
            return false;
        }
        self.arguments
            .windows(sequence.len())
            .any(|window| window.iter().zip(sequence).all(|(got, want)| got == want))
    }

    /// Whether `directory` is the first entry on the invocation's `PATH`.
    #[must_use]
    pub fn path_starts_with(&self, directory: &Utf8Path) -> bool {
        self.path
            .split(':')
            .next()
            .is_some_and(|first| first == directory.as_str())
    }

    /// The recorded `PATH`, for failure messages.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The `CARGO_BUILD_BUILD_DIR` the invocation saw, empty when unset.
    ///
    /// A benchmark's clean pass is only clean if Cargo's intermediates live
    /// under the directory the harness removed, so this is the fact that
    /// distinguishes a cold build from a warm one wearing its clothes.
    #[must_use]
    pub fn build_dir(&self) -> &str {
        &self.build_dir
    }

    /// `RUSTFLAGS` as the invocation saw it, `None` when never assigned.
    #[must_use]
    pub fn rustflags(&self) -> Option<&str> {
        self.rustflags.as_deref()
    }

    /// `RUSTC_WRAPPER` as the invocation saw it, `None` when never assigned.
    #[must_use]
    pub fn wrapper(&self) -> Option<&str> {
        self.wrapper.as_deref()
    }

    /// `RUSTC_WORKSPACE_WRAPPER` as seen, `None` when never assigned.
    #[must_use]
    pub fn workspace_wrapper(&self) -> Option<&str> {
        self.workspace_wrapper.as_deref()
    }

    /// Whether both compiler wrappers were assigned and empty.
    ///
    /// This is the measurable form of "the build really compiled". An
    /// inherited wrapper that chains to a compilation cache makes a clean
    /// build a cache read, which looks exactly like a fast compiler.
    #[must_use]
    pub fn wrappers_cleared(&self) -> bool {
        self.wrapper.as_deref() == Some("") && self.workspace_wrapper.as_deref() == Some("")
    }

    /// `CARGO_ENCODED_RUSTFLAGS` as seen, `None` when unset.
    #[must_use]
    pub fn encoded_rustflags(&self) -> Option<&str> {
        self.encoded_rustflags.as_deref()
    }

    /// Whether every flag in `flags` appears in the recorded `RUSTFLAGS`.
    ///
    /// Membership rather than equality: the recipes append to whatever the
    /// caller already set, so the surrounding value is not the subject.
    #[must_use]
    pub fn rustflags_contain(&self, flags: &[&str]) -> bool {
        self.rustflags.as_ref().is_some_and(|value| {
            let present: Vec<&str> = value.split_whitespace().collect();
            flags.iter().all(|flag| present.contains(flag))
        })
    }

    /// The `CARGO_TARGET_DIR` the invocation was given, empty when unset.
    #[must_use]
    pub fn target_dir(&self) -> &str {
        &self.target_dir
    }

    /// Whether the target directory existed when the invocation started.
    #[must_use]
    pub const fn target_state(&self) -> TargetState {
        self.target_state
    }

    /// The benchmark touch file's modification time, when one was configured.
    /// A benchmark touches that file between a variant's two passes, so the
    /// value distinguishes the pass that ran before the touch from the one
    /// after it.
    #[must_use]
    pub const fn touch_mtime(&self) -> Option<i64> {
        self.touch_mtime
    }
}
