//! One recorded `cargo` invocation, and the reading of a logged record.
//!
//! Split from the parent module, which holds the fake that writes the log.
//! The two halves are read at different times — the fake runs during a Make
//! target, and this parses what it left behind — and keeping them in one file
//! put it past the repository's 400-line module limit.

use anyhow::{Context, Result, bail};
use camino::Utf8Path;
use std::collections::HashMap;

use super::TargetState;

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
    pub(super) fn parse(record: &str) -> Result<Self> {
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
    ///
    /// Each flag is matched as an independent token, which is right for
    /// self-contained flags such as `-Zthreads=8`. It is wrong for a flag that
    /// takes a separate value: `["-D", "warnings"]` is satisfied here by
    /// `-D dead_code warnings`, where the `-D` applies to something else
    /// entirely. Use [`rustflags_contain_sequence`](Self::rustflags_contain_sequence)
    /// for those.
    #[must_use]
    pub fn rustflags_contain(&self, flags: &[&str]) -> bool {
        self.rustflags.as_ref().is_some_and(|value| {
            let present: Vec<&str> = value.split_whitespace().collect();
            flags.iter().all(|flag| present.contains(flag))
        })
    }

    /// Whether `sequence` appears as consecutive tokens in the recorded
    /// `RUSTFLAGS`.
    ///
    /// The form to assert an option and its value with. A membership check
    /// accepts the two tokens anywhere in the value, so it cannot tell
    /// `-D warnings` from `-D dead_code warnings`, and a gate contract written
    /// that way passes for a recipe that never denied warnings at all.
    #[must_use]
    pub fn rustflags_contain_sequence(&self, sequence: &[&str]) -> bool {
        if sequence.is_empty() {
            return false;
        }
        self.rustflags.as_ref().is_some_and(|value| {
            let present: Vec<&str> = value.split_whitespace().collect();
            present.len() >= sequence.len()
                && present
                    .windows(sequence.len())
                    .any(|window| window == sequence)
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

#[cfg(test)]
#[path = "invocation_tests.rs"]
mod tests;
