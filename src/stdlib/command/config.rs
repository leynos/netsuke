//! Command helper configuration, option parsing, and pipe metadata.

use std::{ffi::OsString, io, process::Command, sync::Arc};

use camino::Utf8PathBuf;
use cap_std::fs_utf8::Dir;
use minijinja::{
    Error, ErrorKind,
    value::{Value, ValueKind},
};
use tempfile::{Builder, NamedTempFile};

use crate::localization::{self, keys};
use crate::stdlib::DEFAULT_COMMAND_TEMP_DIR;

use super::error::CommandFailure;

/// Shared configuration for shell helpers, including capture limits and
/// capability-scoped filesystem handles.
#[derive(Clone)]
pub(crate) struct CommandConfig {
    /// Maximum number of bytes buffered in memory when capturing `stdout`.
    pub(crate) max_capture_bytes: u64,
    /// Maximum number of bytes streamed into a tempfile when `stdout` or
    /// `stderr` run in streaming mode.
    pub(crate) max_stream_bytes: u64,
    /// Capability-scoped workspace root used to create temp directories.
    workspace_root: Arc<Dir>,
    /// Absolute UTF-8 workspace root path for host-side filesystem access.
    workspace_root_path: Option<Arc<Utf8PathBuf>>,
    /// Relative directory (beneath the workspace root) for helper tempfiles.
    temp_relative: Utf8PathBuf,
    /// Optional deterministic `PATH` supplied to spawned command helpers.
    command_path_override: Option<OsString>,
}

/// Owned inputs needed to construct command-helper configuration.
pub(crate) struct CommandConfigInit {
    /// Maximum number of bytes buffered in memory when capturing `stdout`.
    pub(crate) max_capture_bytes: u64,
    /// Maximum number of bytes streamed into a tempfile when `stdout` or
    /// `stderr` run in streaming mode.
    pub(crate) max_stream_bytes: u64,
    /// Capability-scoped workspace root used to create temp directories.
    pub(crate) workspace_root: Arc<Dir>,
    /// Absolute UTF-8 workspace root path for host-side filesystem access.
    pub(crate) workspace_root_path: Option<Arc<Utf8PathBuf>>,
    /// Optional deterministic `PATH` supplied to spawned command helpers.
    pub(crate) command_path_override: Option<OsString>,
}

impl CommandConfig {
    /// Construct a new command configuration with byte budgets and workspace
    /// handles. The two limits are interpreted in bytes.
    pub(crate) fn new(init: CommandConfigInit) -> Self {
        Self {
            max_capture_bytes: init.max_capture_bytes,
            max_stream_bytes: init.max_stream_bytes,
            workspace_root: init.workspace_root,
            workspace_root_path: init.workspace_root_path,
            temp_relative: Utf8PathBuf::from(DEFAULT_COMMAND_TEMP_DIR),
            command_path_override: init.command_path_override,
        }
    }

    /// Apply explicitly configured environment values at the child-process
    /// boundary.
    pub(super) fn configure_environment(&self, command: &mut Command) {
        if let Some(path) = &self.command_path_override {
            command.env("PATH", path);
        }
    }

    /// Report whether child commands receive an explicit `PATH` override.
    pub(super) const fn has_command_path_override(&self) -> bool {
        self.command_path_override.is_some()
    }

    /// Create a scoped tempfile for the supplied label beneath the configured
    /// relative directory. Callers must flush before persisting with
    /// `NamedTempFile::into_temp_path()`.
    pub(super) fn create_tempfile(&self, label: &str) -> io::Result<NamedTempFile> {
        let Some(root_path) = &self.workspace_root_path else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                localization::message(keys::COMMAND_TEMPFILE_ROOT_REQUIRED).to_string(),
            ));
        };

        // The capability-scoped handle creates the directory; the ambient
        // path below is only used to point the tempfile builder at it.
        self.workspace_root.create_dir_all(&self.temp_relative)?;
        let dir_path = root_path.join(&self.temp_relative);

        let prefix = sanitize_label(label);
        Builder::new()
            .prefix(&prefix)
            .suffix(".tmp")
            .tempfile_in(dir_path.as_std_path())
            .map_err(|err| {
                io::Error::new(
                    err.kind(),
                    localization::message(keys::COMMAND_TEMPFILE_CREATE_FAILED)
                        .with_arg("label", label)
                        .with_arg("details", err.to_string())
                        .to_string(),
                )
            })
    }
}

/// Sanitise a label into a filesystem-safe tempfile prefix, replacing
/// disallowed characters with `-` and falling back to `t` when empty.
fn sanitize_label(label: &str) -> String {
    let mut sanitized = String::with_capacity(label.len());
    for ch in label.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
            sanitized.push(ch);
        } else {
            sanitized.push('-');
        }
    }
    if sanitized.is_empty() {
        sanitized.push('t');
    }
    sanitized
}

/// Controls how helper stdout is materialised (in-memory capture or streaming
/// via a tempfile).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum OutputMode {
    /// Buffer output in memory.
    Capture,
    /// Stream output through a tempfile.
    Tempfile,
}

impl OutputMode {
    /// Returns the localization key used in diagnostics.
    pub(super) const fn label_key(self) -> &'static str {
        match self {
            Self::Capture => keys::COMMAND_OUTPUT_MODE_CAPTURE,
            Self::Tempfile => keys::COMMAND_OUTPUT_MODE_STREAMING,
        }
    }
}

/// Distinguishes between the stdout and stderr pipes so limits and file names
/// can be recorded accurately.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum OutputStream {
    /// The standard output pipe.
    Stdout,
    /// The standard error pipe.
    Stderr,
}

impl OutputStream {
    /// Returns the localization key used in human-readable errors.
    pub(super) const fn label_key(self) -> &'static str {
        match self {
            Self::Stdout => keys::COMMAND_OUTPUT_STREAM_STDOUT,
            Self::Stderr => keys::COMMAND_OUTPUT_STREAM_STDERR,
        }
    }

    /// Provides a short label used to name streamed tempfile outputs
    /// (e.g. `"stdout"`).
    pub(super) const fn tempfile_label(self) -> &'static str {
        match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
        }
    }

    /// Provides the label used when creating marker files for empty streams.
    pub(super) const fn empty_tempfile_label(self) -> &'static str {
        match self {
            Self::Stdout => "stdout-empty",
            Self::Stderr => "stderr-empty",
        }
    }
}

/// Encapsulates the stream, mode, and byte budget for a single pipe reader.
#[derive(Clone, Copy)]
pub(super) struct PipeSpec {
    /// The pipe (`stdout` or `stderr`) governed by this spec.
    stream: OutputStream,
    /// Whether the spec captures output in memory or streams to disk.
    mode: OutputMode,
    /// The configured byte ceiling for this stream.
    limit: u64,
}

impl PipeSpec {
    /// Constructs a new specification with a byte ceiling. `limit` is measured
    /// in bytes as supplied through `StdlibConfig`.
    pub(super) const fn new(stream: OutputStream, mode: OutputMode, limit: u64) -> Self {
        Self {
            stream,
            mode,
            limit,
        }
    }

    /// Returns the pipe (`stdout` or `stderr`) governed by this spec.
    pub(super) const fn stream(self) -> OutputStream {
        self.stream
    }

    /// Returns whether the spec captures output in memory or streams to disk.
    pub(super) const fn mode(self) -> OutputMode {
        self.mode
    }

    /// Returns the configured byte ceiling for this stream.
    pub(super) const fn limit(self) -> u64 {
        self.limit
    }

    /// Converts the immutable spec into a mutable `PipeLimit` tracker.
    pub(super) const fn into_limit(self) -> PipeLimit {
        PipeLimit {
            spec: self,
            consumed: 0,
        }
    }
}

/// Tracks how many bytes have been consumed relative to a `PipeSpec`.
pub(super) struct PipeLimit {
    /// The specification this tracker enforces.
    spec: PipeSpec,
    /// Bytes consumed so far towards the ceiling.
    consumed: u64,
}

impl PipeLimit {
    /// Records a successful read of `read` bytes, returning an error if the
    /// limit would be exceeded. For example, calling `record(512)` twice on a
    /// 1024-byte spec succeeds, while the third call errors with
    /// `CommandFailure::OutputLimit`.
    pub(super) fn record(&mut self, read: usize) -> Result<(), CommandFailure> {
        let bytes = read_size_to_u64(read);
        let new_total = add_saturating(self.consumed, bytes);
        if new_total > self.spec.limit() {
            return Err(CommandFailure::OutputLimit {
                stream: self.spec.stream(),
                mode: self.spec.mode(),
                limit: self.spec.limit(),
            });
        }
        self.consumed = new_total;
        Ok(())
    }
}

/// Convert a read size to `u64`, saturating on the (unreachable in practice)
/// overflow so an oversized read trips the output limit instead of panicking.
fn read_size_to_u64(read: usize) -> u64 {
    u64::try_from(read).unwrap_or(u64::MAX)
}

/// Add byte counts, saturating at `u64::MAX`; saturation exceeds every
/// configurable limit, so `record` reports `OutputLimit` rather than panicking.
const fn add_saturating(current: u64, delta: u64) -> u64 {
    current.saturating_add(delta)
}

/// Parsed view of the filter options provided by the template author.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CommandOptions {
    /// The requested output mode for the stdout pipe.
    stdout_mode: OutputMode,
}

impl CommandOptions {
    /// Parses helper options supplied as either a string or mapping. Returns
    /// `OutputMode::Capture` when the value is missing or `undefined`. For
    /// example, passing `{ 'mode': 'tempfile' }` selects streaming mode.
    pub(super) fn from_value(options: Option<Value>) -> Result<Self, Error> {
        let Some(raw) = options else {
            return Ok(Self::default());
        };

        if raw.is_undefined() {
            return Ok(Self::default());
        }

        match raw.kind() {
            ValueKind::String => {
                let Some(text) = raw.as_str() else {
                    return Err(Error::new(
                        ErrorKind::InvalidOperation,
                        localization::message(keys::COMMAND_OPTIONS_INVALID_UTF8).to_string(),
                    ));
                };
                Self::from_mode_str(text)
            }
            ValueKind::Map | ValueKind::Plain => {
                let mode_value = raw.get_attr("mode")?;
                if mode_value.is_undefined() {
                    return Ok(Self::default());
                }
                let Some(mode) = mode_value.as_str() else {
                    return Err(Error::new(
                        ErrorKind::InvalidOperation,
                        localization::message(keys::COMMAND_OPTION_MODE_NOT_STRING).to_string(),
                    ));
                };
                Self::from_mode_str(mode)
            }
            _ => Err(Error::new(
                ErrorKind::InvalidOperation,
                localization::message(keys::COMMAND_OPTIONS_INVALID_TYPE).to_string(),
            )),
        }
    }

    /// Parse a mode string into an `OutputMode`.
    ///
    /// # Errors
    ///
    /// Returns an error when `mode` is not `capture`, `tempfile`, `stream`, or
    /// `streaming`.
    fn from_mode_str(mode: &str) -> Result<Self, Error> {
        match mode {
            "capture" => Ok(Self {
                stdout_mode: OutputMode::Capture,
            }),
            "tempfile" | "stream" | "streaming" => Ok(Self {
                stdout_mode: OutputMode::Tempfile,
            }),
            other => Err(Error::new(
                ErrorKind::InvalidOperation,
                localization::message(keys::COMMAND_OUTPUT_MODE_UNSUPPORTED)
                    .with_arg("mode", other)
                    .to_string(),
            )),
        }
    }

    /// Returns the requested stdout mode so execution can choose between
    /// capture and streaming.
    pub(super) const fn stdout_mode(self) -> OutputMode {
        self.stdout_mode
    }
}

impl Default for CommandOptions {
    fn default() -> Self {
        Self {
            stdout_mode: OutputMode::Capture,
        }
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
