//! Command helper configuration, option parsing, and pipe metadata.

use std::{fs, io, sync::Arc};

use camino::Utf8PathBuf;
use cap_std::fs_utf8::Dir;
use minijinja::{
    Error, ErrorKind,
    value::{Value, ValueKind},
};
use tempfile::{Builder, NamedTempFile};

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
}

impl CommandConfig {
    /// Construct a new command configuration with byte budgets and workspace
    /// handles. The two limits are interpreted in bytes.
    pub(crate) fn new(
        max_capture_bytes: u64,
        max_stream_bytes: u64,
        workspace_root: Arc<Dir>,
        workspace_root_path: Option<Arc<Utf8PathBuf>>,
    ) -> Self {
        Self {
            max_capture_bytes,
            max_stream_bytes,
            workspace_root,
            workspace_root_path,
            temp_relative: Utf8PathBuf::from(DEFAULT_COMMAND_TEMP_DIR),
        }
    }

    /// Create a scoped tempfile for the supplied label beneath the configured
    /// relative directory. Callers must flush before persisting with
    /// `NamedTempFile::into_temp_path()`.
    pub(super) fn create_tempfile(&self, label: &str) -> io::Result<NamedTempFile> {
        let Some(root_path) = &self.workspace_root_path else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "workspace root path must be configured for command tempfiles",
            ));
        };

        self.workspace_root.create_dir_all(&self.temp_relative)?;
        let dir_path = root_path.join(&self.temp_relative);
        fs::create_dir_all(dir_path.as_std_path())?;

        let prefix = sanitize_label(label);
        Builder::new()
            .prefix(&prefix)
            .suffix(".tmp")
            .tempfile_in(dir_path.as_std_path())
            .map_err(|err| {
                io::Error::new(
                    err.kind(),
                    format!("failed to create command tempfile for '{label}': {err}"),
                )
            })
    }
}

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
    Capture,
    Tempfile,
}

impl OutputMode {
    /// Returns the label used in diagnostics. For example,
    /// `OutputMode::Capture.describe()` yields `"capture"`.
    pub(super) const fn describe(self) -> &'static str {
        match self {
            Self::Capture => "capture",
            Self::Tempfile => "streaming",
        }
    }
}

/// Distinguishes between the stdout and stderr pipes so limits and file names
/// can be recorded accurately.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum OutputStream {
    Stdout,
    Stderr,
}

impl OutputStream {
    /// Returns the stream name used in human-readable errors (for example,
    /// `OutputStream::Stdout.describe()` returns `"stdout"`).
    pub(super) const fn describe(self) -> &'static str {
        match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
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
    stream: OutputStream,
    mode: OutputMode,
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
    spec: PipeSpec,
    consumed: u64,
}

impl PipeLimit {
    /// Records a successful read of `read` bytes, returning an error if the
    /// limit would be exceeded. For example, calling `record(512)` twice on a
    /// 1024-byte spec succeeds, while the third call errors with
    /// `CommandFailure::OutputLimit`.
    pub(super) fn record(&mut self, read: usize) -> Result<(), CommandFailure> {
        let bytes = read_size_to_u64(read);
        let new_total = add_checked(self.consumed, bytes);
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

fn read_size_to_u64(read: usize) -> u64 {
    u64::try_from(read).expect("pipe read size overflow should be impossible")
}

fn add_checked(current: u64, delta: u64) -> u64 {
    current
        .checked_add(delta)
        .expect("pipe output size overflow should be impossible")
}

/// Parsed view of the filter options provided by the template author.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CommandOptions {
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
                        "command options string must be valid UTF-8",
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
                        "command option 'mode' must be a string",
                    ));
                };
                Self::from_mode_str(mode)
            }
            _ => Err(Error::new(
                ErrorKind::InvalidOperation,
                "command options must be a string or mapping",
            )),
        }
    }

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
                format!("unsupported command output mode '{other}'"),
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
mod tests {
    use super::*;
    use cap_std::{ambient_authority, fs_utf8::Dir};
    use tempfile::tempdir;

    fn test_command_config() -> (tempfile::TempDir, CommandConfig) {
        let temp = tempdir().expect("create command temp workspace");
        let path = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
            .expect("temp workspace should be valid UTF-8");
        let dir =
            Dir::open_ambient_dir(&path, ambient_authority()).expect("open temp workspace dir");
        let config = CommandConfig::new(1024, 2048, Arc::new(dir), Some(Arc::new(path)));
        (temp, config)
    }

    #[test]
    fn sanitize_label_replaces_disallowed_characters() {
        assert_eq!(sanitize_label("std:out/..*"), "std-out----");
    }

    #[test]
    fn command_tempfile_drop_removes_file() {
        let (_temp_dir, config) = test_command_config();
        let temp_path = {
            let tempfile = config.create_tempfile("stdout").expect("create temp file");
            let path = tempfile.path().to_path_buf();
            assert!(path.exists(), "tempfile should exist while handle is alive");
            path
        };
        assert!(
            !temp_path.exists(),
            "temporary file should be removed on drop"
        );
    }

    #[test]
    fn command_tempfile_into_path_persists_file() {
        let (_temp_dir, config) = test_command_config();
        let tempfile = config.create_tempfile("stdout").expect("create temp file");
        let expected = tempfile.path().to_path_buf();
        let kept = tempfile
            .into_temp_path()
            .keep()
            .map_err(|err| err.error)
            .expect("persist temp file");
        assert_eq!(kept.as_path(), expected.as_path());
        assert!(kept.as_path().exists());
        fs::remove_file(kept.as_path()).expect("cleanup persisted temp file");
    }

    #[test]
    fn command_tempdir_requires_workspace_root_path() {
        let temp = tempdir().expect("create temp workspace for command");
        let path = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
            .expect("temp workspace should be valid UTF-8");
        let dir =
            Dir::open_ambient_dir(&path, ambient_authority()).expect("open temp workspace dir");
        let config = CommandConfig::new(1024, 2048, Arc::new(dir), None);
        match config.create_tempfile("stdout") {
            Ok(_) => panic!("command temp dir should require workspace root path"),
            Err(err) => assert_eq!(err.kind(), io::ErrorKind::InvalidInput),
        }
    }
}
