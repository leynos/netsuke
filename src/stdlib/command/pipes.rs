//! Pipe reader management for command execution.

use std::{
    io::{self, Read, Write},
    sync::Arc,
    thread,
};

#[cfg(test)]
use super::config::OutputStream;
use super::{
    config::{CommandConfig, OutputMode, PipeLimit, PipeSpec},
    error::CommandFailure,
    result::PipeOutcome,
};
use camino::Utf8PathBuf;

const PIPE_CHUNK_SIZE: usize = 8192;

pub(super) fn spawn_pipe_reader<R>(
    pipe: Option<R>,
    spec: PipeSpec,
    config: Arc<CommandConfig>,
) -> Option<thread::JoinHandle<Result<PipeOutcome, CommandFailure>>>
where
    R: Read + Send + 'static,
{
    pipe.map(|reader| thread::spawn(move || read_pipe(reader, spec, config.as_ref())))
}

pub(super) fn join_reader(
    reader_handle: Option<thread::JoinHandle<Result<PipeOutcome, CommandFailure>>>,
    spec: PipeSpec,
    config: &CommandConfig,
) -> Result<PipeOutcome, CommandFailure> {
    match reader_handle {
        Some(join_handle) => join_handle
            .join()
            .map_err(|_| CommandFailure::Io(io::Error::other("pipe reader panicked")))?,
        None => {
            if matches!(spec.mode(), OutputMode::Tempfile) {
                create_empty_tempfile(config, spec.stream().empty_tempfile_label())
                    .map(PipeOutcome::Tempfile)
            } else {
                Ok(PipeOutcome::Bytes(Vec::new()))
            }
        }
    }
}

pub(super) fn cleanup_readers(
    stdout_reader: &mut Option<thread::JoinHandle<Result<PipeOutcome, CommandFailure>>>,
    stderr_reader: &mut Option<thread::JoinHandle<Result<PipeOutcome, CommandFailure>>>,
    stdin_handle: &mut Option<thread::JoinHandle<io::Result<()>>>,
) {
    join_pipe_for_cleanup("stdout", stdout_reader);
    join_pipe_for_cleanup("stderr", stderr_reader);
    if let Some(handle) = stdin_handle.take()
        && let Err(join_err) = handle.join()
    {
        tracing::warn!("stdin writer thread panicked: {join_err:?}");
    }
}

pub(super) fn handle_stdin_result(
    stdin_handle: Option<thread::JoinHandle<io::Result<()>>>,
    status: Option<i32>,
    stderr: &[u8],
) -> Result<(), CommandFailure> {
    let Some(handle) = stdin_handle else {
        return Ok(());
    };

    match handle.join() {
        Ok(Ok(())) => Ok(()),
        Ok(Err(err)) => {
            if err.kind() == io::ErrorKind::BrokenPipe {
                if status == Some(0) {
                    return Ok(());
                }
                return Err(CommandFailure::BrokenPipe {
                    source: err,
                    status,
                    stderr: stderr.to_vec(),
                });
            }
            Err(CommandFailure::Io(err))
        }
        Err(_) => Err(CommandFailure::Io(io::Error::other(
            "stdin writer panicked",
        ))),
    }
}

fn read_pipe<R>(
    reader: R,
    spec: PipeSpec,
    config: &CommandConfig,
) -> Result<PipeOutcome, CommandFailure>
where
    R: Read,
{
    let limit = spec.into_limit();
    match spec.mode() {
        OutputMode::Capture => read_pipe_capture(reader, limit),
        OutputMode::Tempfile => {
            read_pipe_tempfile(reader, limit, spec.stream().tempfile_label(), config)
        }
    }
}

fn read_pipe_capture<R>(mut reader: R, mut limit: PipeLimit) -> Result<PipeOutcome, CommandFailure>
where
    R: Read,
{
    let mut buf = Vec::new();
    let mut chunk = [0_u8; PIPE_CHUNK_SIZE];
    loop {
        let read = reader.read(&mut chunk).map_err(CommandFailure::Io)?;
        if read == 0 {
            break;
        }
        limit.record(read)?;
        buf.extend(chunk.iter().take(read).copied());
    }
    Ok(PipeOutcome::Bytes(buf))
}

fn read_pipe_tempfile<R>(
    mut reader: R,
    mut limit: PipeLimit,
    label: &str,
    config: &CommandConfig,
) -> Result<PipeOutcome, CommandFailure>
where
    R: Read,
{
    let mut tempfile = config.create_tempfile(label).map_err(CommandFailure::Io)?;
    let mut chunk = [0_u8; PIPE_CHUNK_SIZE];
    loop {
        let read = reader.read(&mut chunk).map_err(CommandFailure::Io)?;
        if read == 0 {
            break;
        }
        limit.record(read)?;
        #[expect(
            clippy::indexing_slicing,
            reason = "Read::read guarantees `read` does not exceed `chunk.len()`"
        )]
        tempfile
            .as_file_mut()
            .write_all(&chunk[..read])
            .map_err(CommandFailure::Io)?;
    }
    tempfile.as_file_mut().flush().map_err(CommandFailure::Io)?;
    let path = tempfile.into_path().map_err(CommandFailure::Io)?;
    Ok(PipeOutcome::Tempfile(path))
}

fn create_empty_tempfile(
    config: &CommandConfig,
    label: &str,
) -> Result<Utf8PathBuf, CommandFailure> {
    let tempfile = config.create_tempfile(label).map_err(CommandFailure::Io)?;
    tempfile.into_path().map_err(CommandFailure::Io)
}

fn join_pipe_for_cleanup(
    label: &str,
    reader_handle: &mut Option<thread::JoinHandle<Result<PipeOutcome, CommandFailure>>>,
) {
    if let Some(join_handle) = reader_handle.take() {
        match join_handle.join() {
            Ok(Ok(_)) => {}
            Ok(Err(err)) => {
                tracing::warn!(stream = label, ?err, "pipe reader failed during cleanup");
            }
            Err(join_err) => {
                tracing::warn!(stream = label, ?join_err, "pipe reader thread panicked");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stdlib::{DEFAULT_COMMAND_MAX_OUTPUT_BYTES, DEFAULT_COMMAND_MAX_STREAM_BYTES};
    use cap_std::{ambient_authority, fs_utf8::Dir};
    use std::{fs, io::Cursor};
    use tempfile::tempdir;

    fn test_command_config() -> (tempfile::TempDir, CommandConfig) {
        let temp = tempdir().expect("create command temp workspace");
        let path = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
            .expect("temp workspace should be valid UTF-8");
        let dir =
            Dir::open_ambient_dir(&path, ambient_authority()).expect("open temp workspace dir");
        let config = CommandConfig::new(
            DEFAULT_COMMAND_MAX_OUTPUT_BYTES,
            DEFAULT_COMMAND_MAX_STREAM_BYTES,
            Arc::new(dir),
            Some(Arc::new(path)),
        );
        (temp, config)
    }

    fn assert_output_limit_error(
        outcome: Result<PipeOutcome, CommandFailure>,
        expected_stream: OutputStream,
        expected_mode: OutputMode,
        expected_limit: u64,
    ) {
        let err =
            outcome.expect_err("expected command to exceed the configured output limit for test");
        match err {
            CommandFailure::OutputLimit {
                stream,
                mode,
                limit,
            } => {
                assert_eq!(stream, expected_stream);
                assert_eq!(mode, expected_mode);
                assert_eq!(limit, expected_limit);
            }
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn read_pipe_capture_collects_bytes_within_limit() {
        let data = b"payload".to_vec();
        let outcome = read_pipe_capture(
            Cursor::new(data.clone()),
            PipeSpec::new(OutputStream::Stdout, OutputMode::Capture, 128).into_limit(),
        )
        .expect("capture should succeed within the configured limit");
        match outcome {
            PipeOutcome::Bytes(buf) => assert_eq!(buf, data),
            PipeOutcome::Tempfile(_) => panic!("capture mode should emit bytes"),
        }
    }

    #[test]
    fn read_pipe_capture_reports_limit_exceedance() {
        let outcome = read_pipe_capture(
            Cursor::new(vec![0_u8; 16]),
            PipeSpec::new(OutputStream::Stdout, OutputMode::Capture, 8).into_limit(),
        );
        assert_output_limit_error(outcome, OutputStream::Stdout, OutputMode::Capture, 8);
    }

    #[test]
    fn read_pipe_tempfile_writes_streamed_data() {
        let payload = vec![b'x'; 32];
        let (_temp_dir, config) = test_command_config();
        let outcome = read_pipe_tempfile(
            Cursor::new(payload.clone()),
            PipeSpec::new(OutputStream::Stdout, OutputMode::Tempfile, 64).into_limit(),
            "stdout",
            &config,
        )
        .expect("streaming should succeed within the configured limit");
        let path = match outcome {
            PipeOutcome::Tempfile(path) => path,
            PipeOutcome::Bytes(_) => panic!("streaming mode should emit a tempfile path"),
        };
        let disk = fs::read(path.as_std_path()).expect("read streamed output");
        assert_eq!(disk, payload);
        fs::remove_file(path.as_std_path()).expect("cleanup streamed file");
    }

    #[test]
    fn read_pipe_tempfile_respects_stream_limit() {
        let (_temp_dir, config) = test_command_config();
        let outcome = read_pipe_tempfile(
            Cursor::new(vec![b'y'; 32]),
            PipeSpec::new(OutputStream::Stdout, OutputMode::Tempfile, 8).into_limit(),
            "stdout",
            &config,
        );
        assert_output_limit_error(outcome, OutputStream::Stdout, OutputMode::Tempfile, 8);
    }
}
