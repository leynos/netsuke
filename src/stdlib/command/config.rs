//! Command helper configuration, option parsing, and pipe metadata.

use std::{
    fs,
    io::{self, Write},
    sync::Arc,
};

use camino::Utf8PathBuf;
use cap_std::fs_utf8::Dir;
use minijinja::{
    Error, ErrorKind,
    value::{Value, ValueKind},
};
use tempfile::{Builder, NamedTempFile};

use crate::stdlib::DEFAULT_COMMAND_TEMP_DIR;

use super::error::CommandFailure;

#[derive(Clone)]
pub(crate) struct CommandConfig {
    pub(crate) max_capture_bytes: u64,
    pub(crate) max_stream_bytes: u64,
    temp_dir: CommandTempDir,
}

impl CommandConfig {
    pub(crate) fn new(
        max_capture_bytes: u64,
        max_stream_bytes: u64,
        workspace_root: Arc<Dir>,
        workspace_root_path: Option<Arc<Utf8PathBuf>>,
    ) -> Self {
        Self {
            max_capture_bytes,
            max_stream_bytes,
            temp_dir: CommandTempDir::new(workspace_root, workspace_root_path),
        }
    }

    pub(super) fn create_tempfile(&self, label: &str) -> io::Result<CommandTempFile> {
        self.temp_dir.create(label)
    }
}

#[derive(Clone)]
struct CommandTempDir {
    workspace_root: Arc<Dir>,
    workspace_root_path: Option<Arc<Utf8PathBuf>>,
    relative: Utf8PathBuf,
}

impl CommandTempDir {
    fn new(workspace_root: Arc<Dir>, workspace_root_path: Option<Arc<Utf8PathBuf>>) -> Self {
        Self {
            workspace_root,
            workspace_root_path,
            relative: Utf8PathBuf::from(DEFAULT_COMMAND_TEMP_DIR),
        }
    }

    fn create(&self, label: &str) -> io::Result<CommandTempFile> {
        let Some(root_path) = &self.workspace_root_path else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "workspace root path must be configured for command tempfiles",
            ));
        };
        self.workspace_root.create_dir_all(&self.relative)?;
        let dir_path = root_path.join(&self.relative);
        fs::create_dir_all(dir_path.as_std_path())?;

        let prefix = sanitize_label(label);
        let mut builder = Builder::new();
        builder.prefix(&prefix);
        builder.suffix(".tmp");

        let file = builder.tempfile_in(dir_path.as_std_path()).map_err(|err| {
            io::Error::new(
                err.kind(),
                format!("failed to create command tempfile for '{label}': {err}"),
            )
        })?;

        Ok(CommandTempFile::new(file))
    }
}

pub(super) struct CommandTempFile {
    file: NamedTempFile,
}

impl CommandTempFile {
    #[expect(
        clippy::missing_const_for_fn,
        reason = "NamedTempFile creation is inherently runtime-only"
    )]
    fn new(file: NamedTempFile) -> Self {
        Self { file }
    }

    #[expect(
        clippy::missing_const_for_fn,
        reason = "Mutable handles cannot be borrowed in const contexts"
    )]
    pub(super) fn as_file_mut(&mut self) -> &mut NamedTempFile {
        &mut self.file
    }

    #[cfg(test)]
    pub(super) fn path(&self) -> &std::path::Path {
        self.file.path()
    }

    pub(super) fn into_path(mut self) -> io::Result<Utf8PathBuf> {
        self.file.flush()?;
        let temp_path = self.file.into_temp_path();
        let path = temp_path.keep().map_err(|err| err.error)?;
        Utf8PathBuf::from_path_buf(path).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "command tempfile path is not valid UTF-8",
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum OutputMode {
    Capture,
    Tempfile,
}

impl OutputMode {
    pub(super) const fn describe(self) -> &'static str {
        match self {
            Self::Capture => "capture",
            Self::Tempfile => "streaming",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum OutputStream {
    Stdout,
    Stderr,
}

impl OutputStream {
    pub(super) const fn describe(self) -> &'static str {
        match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
        }
    }

    pub(super) const fn tempfile_label(self) -> &'static str {
        match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
        }
    }

    pub(super) const fn empty_tempfile_label(self) -> &'static str {
        match self {
            Self::Stdout => "stdout-empty",
            Self::Stderr => "stderr-empty",
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct PipeSpec {
    stream: OutputStream,
    mode: OutputMode,
    limit: u64,
}

impl PipeSpec {
    pub(super) const fn new(stream: OutputStream, mode: OutputMode, limit: u64) -> Self {
        Self {
            stream,
            mode,
            limit,
        }
    }

    pub(super) const fn stream(self) -> OutputStream {
        self.stream
    }

    pub(super) const fn mode(self) -> OutputMode {
        self.mode
    }

    pub(super) const fn limit(self) -> u64 {
        self.limit
    }

    pub(super) const fn into_limit(self) -> PipeLimit {
        PipeLimit {
            spec: self,
            consumed: 0,
        }
    }
}

pub(super) struct PipeLimit {
    spec: PipeSpec,
    consumed: u64,
}

impl PipeLimit {
    pub(super) fn record(&mut self, read: usize) -> Result<(), CommandFailure> {
        let bytes = u64::try_from(read)
            .map_err(|_| CommandFailure::Io(io::Error::other("pipe read size overflow")))?;
        let new_total = self
            .consumed
            .checked_add(bytes)
            .ok_or_else(|| CommandFailure::Io(io::Error::other("pipe output size overflow")))?;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CommandOptions {
    stdout_mode: OutputMode,
}

impl CommandOptions {
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
            tempfile.path().to_path_buf()
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
        let persisted = tempfile.into_path().expect("persist temp file");
        assert_eq!(persisted.as_std_path(), expected.as_path());
        assert!(persisted.as_std_path().exists());
        fs::remove_file(persisted.as_std_path()).expect("cleanup persisted temp file");
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
