use std::io::{self, ErrorKind as IoErrorKind};

use camino::Utf8Path;
use minijinja::{Error, ErrorKind};

pub(super) fn io_to_error(path: &Utf8Path, action: &str, err: io::Error) -> Error {
    let io_kind = err.kind();
    let label = io_error_kind_label(io_kind);
    let detail = err.to_string();

    let message = if detail.is_empty() {
        format!("{action} failed for {path}: {label} [{io_kind:?}]")
    } else if detail.to_ascii_lowercase().contains(label) {
        format!("{action} failed for {path}: {detail} [{io_kind:?}]")
    } else {
        format!("{action} failed for {path}: {label} [{io_kind:?}] ({detail})")
    };

    Error::new(ErrorKind::InvalidOperation, message).with_source(err)
}

fn io_error_kind_label(kind: IoErrorKind) -> &'static str {
    match kind {
        IoErrorKind::NotFound => "not found",
        IoErrorKind::PermissionDenied => "permission denied",
        IoErrorKind::AlreadyExists => "already exists",
        IoErrorKind::InvalidInput => "invalid input",
        IoErrorKind::InvalidData => "invalid data",
        IoErrorKind::TimedOut => "timed out",
        IoErrorKind::Interrupted => "interrupted",
        IoErrorKind::WouldBlock => "operation would block",
        IoErrorKind::WriteZero => "write zero",
        IoErrorKind::UnexpectedEof => "unexpected end of file",
        IoErrorKind::BrokenPipe => "broken pipe",
        IoErrorKind::ConnectionRefused => "connection refused",
        IoErrorKind::ConnectionReset => "connection reset",
        IoErrorKind::ConnectionAborted => "connection aborted",
        IoErrorKind::NotConnected => "not connected",
        IoErrorKind::AddrInUse => "address in use",
        IoErrorKind::AddrNotAvailable => "address not available",
        IoErrorKind::OutOfMemory => "out of memory",
        IoErrorKind::Unsupported => "unsupported operation",
        IoErrorKind::FileTooLarge => "file too large",
        IoErrorKind::ResourceBusy => "resource busy",
        IoErrorKind::ExecutableFileBusy => "executable busy",
        IoErrorKind::Deadlock => "deadlock",
        IoErrorKind::CrossesDevices => "cross-device link",
        IoErrorKind::TooManyLinks => "too many links",
        IoErrorKind::InvalidFilename => "invalid filename",
        IoErrorKind::ArgumentListTooLong => "argument list too long",
        IoErrorKind::StaleNetworkFileHandle => "stale network file handle",
        IoErrorKind::StorageFull => "storage full",
        IoErrorKind::NotSeekable => "not seekable",
        IoErrorKind::NetworkDown => "network down",
        IoErrorKind::NetworkUnreachable => "network unreachable",
        IoErrorKind::HostUnreachable => "host unreachable",
        _ => "io error",
    }
}
