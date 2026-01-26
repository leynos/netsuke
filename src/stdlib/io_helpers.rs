//! IO error adapters for stdlib helpers.
//!
//! Convert `io::Error` values into `MiniJinja` `InvalidOperation` diagnostics
//! with localized labels and action context.

use std::io::{self, ErrorKind as IoErrorKind};

use camino::Utf8Path;
use minijinja::{Error, ErrorKind};

use crate::localization::{self, LocalizedMessage, keys};

/// Variants for IO error message formatting.
enum IoMessageVariant {
    /// No detail available; use label only.
    Failed,
    /// Detail contains the label; use detail only.
    FailedWithDetail,
    /// Detail does not contain label; use both.
    FailedWithLabelAndDetail,
}

/// Determine which message variant to use based on detail and label content.
fn io_message_variant(detail: &str, label: &str) -> IoMessageVariant {
    if detail.is_empty() {
        IoMessageVariant::Failed
    } else if detail
        .to_lowercase()
        .contains(label.to_lowercase().as_str())
    {
        IoMessageVariant::FailedWithDetail
    } else {
        IoMessageVariant::FailedWithLabelAndDetail
    }
}

pub(crate) fn io_to_error(path: &Utf8Path, action: &LocalizedMessage, err: io::Error) -> Error {
    let io_kind = err.kind();
    let label = localization::message(io_error_kind_label(io_kind)).to_string();
    let action_text = action.to_string();
    let detail = err.to_string();

    let message = match io_message_variant(&detail, &label) {
        IoMessageVariant::Failed => localization::message(keys::STDLIB_PATH_IO_FAILED)
            .with_arg("action", &action_text)
            .with_arg("path", path.as_str())
            .with_arg("label", &label)
            .with_arg("kind", format!("{io_kind:?}"))
            .to_string(),
        IoMessageVariant::FailedWithDetail => {
            localization::message(keys::STDLIB_PATH_IO_FAILED_WITH_DETAIL)
                .with_arg("action", &action_text)
                .with_arg("path", path.as_str())
                .with_arg("detail", &detail)
                .with_arg("kind", format!("{io_kind:?}"))
                .to_string()
        }
        IoMessageVariant::FailedWithLabelAndDetail => {
            localization::message(keys::STDLIB_PATH_IO_FAILED_WITH_LABEL_AND_DETAIL)
                .with_arg("action", &action_text)
                .with_arg("path", path.as_str())
                .with_arg("label", &label)
                .with_arg("kind", format!("{io_kind:?}"))
                .with_arg("detail", &detail)
                .to_string()
        }
    };

    Error::new(ErrorKind::InvalidOperation, message).with_source(err)
}

pub(crate) fn io_action_error(
    template_key: &'static str,
    action: &LocalizedMessage,
    path: &Utf8Path,
    err: io::Error,
) -> Error {
    let message = localization::message(template_key)
        .with_arg("action", action.to_string())
        .with_arg("path", path.as_str())
        .with_arg("details", err.to_string())
        .to_string();
    Error::new(ErrorKind::InvalidOperation, message).with_source(err)
}

const fn io_error_kind_label(kind: IoErrorKind) -> &'static str {
    match kind {
        IoErrorKind::NotFound => keys::STDLIB_PATH_IO_NOT_FOUND,
        IoErrorKind::PermissionDenied => keys::STDLIB_PATH_IO_PERMISSION_DENIED,
        IoErrorKind::AlreadyExists => keys::STDLIB_PATH_IO_ALREADY_EXISTS,
        IoErrorKind::InvalidInput => keys::STDLIB_PATH_IO_INVALID_INPUT,
        IoErrorKind::InvalidData => keys::STDLIB_PATH_IO_INVALID_DATA,
        IoErrorKind::TimedOut => keys::STDLIB_PATH_IO_TIMED_OUT,
        IoErrorKind::Interrupted => keys::STDLIB_PATH_IO_INTERRUPTED,
        IoErrorKind::WouldBlock => keys::STDLIB_PATH_IO_WOULD_BLOCK,
        IoErrorKind::WriteZero => keys::STDLIB_PATH_IO_WRITE_ZERO,
        IoErrorKind::UnexpectedEof => keys::STDLIB_PATH_IO_UNEXPECTED_EOF,
        IoErrorKind::BrokenPipe => keys::STDLIB_PATH_IO_BROKEN_PIPE,
        IoErrorKind::ConnectionRefused => keys::STDLIB_PATH_IO_CONNECTION_REFUSED,
        IoErrorKind::ConnectionReset => keys::STDLIB_PATH_IO_CONNECTION_RESET,
        IoErrorKind::ConnectionAborted => keys::STDLIB_PATH_IO_CONNECTION_ABORTED,
        IoErrorKind::NotConnected => keys::STDLIB_PATH_IO_NOT_CONNECTED,
        IoErrorKind::AddrInUse => keys::STDLIB_PATH_IO_ADDR_IN_USE,
        IoErrorKind::AddrNotAvailable => keys::STDLIB_PATH_IO_ADDR_NOT_AVAILABLE,
        IoErrorKind::OutOfMemory => keys::STDLIB_PATH_IO_OUT_OF_MEMORY,
        IoErrorKind::Unsupported => keys::STDLIB_PATH_IO_UNSUPPORTED,
        IoErrorKind::FileTooLarge => keys::STDLIB_PATH_IO_FILE_TOO_LARGE,
        IoErrorKind::ResourceBusy => keys::STDLIB_PATH_IO_RESOURCE_BUSY,
        IoErrorKind::ExecutableFileBusy => keys::STDLIB_PATH_IO_EXECUTABLE_BUSY,
        IoErrorKind::Deadlock => keys::STDLIB_PATH_IO_DEADLOCK,
        IoErrorKind::CrossesDevices => keys::STDLIB_PATH_IO_CROSSES_DEVICES,
        IoErrorKind::TooManyLinks => keys::STDLIB_PATH_IO_TOO_MANY_LINKS,
        IoErrorKind::InvalidFilename => keys::STDLIB_PATH_IO_INVALID_FILENAME,
        IoErrorKind::ArgumentListTooLong => keys::STDLIB_PATH_IO_ARG_LIST_TOO_LONG,
        IoErrorKind::StaleNetworkFileHandle => keys::STDLIB_PATH_IO_STALE_HANDLE,
        IoErrorKind::StorageFull => keys::STDLIB_PATH_IO_STORAGE_FULL,
        IoErrorKind::NotSeekable => keys::STDLIB_PATH_IO_NOT_SEEKABLE,
        IoErrorKind::NetworkDown => keys::STDLIB_PATH_IO_NETWORK_DOWN,
        IoErrorKind::NetworkUnreachable => keys::STDLIB_PATH_IO_NETWORK_UNREACHABLE,
        IoErrorKind::HostUnreachable => keys::STDLIB_PATH_IO_HOST_UNREACHABLE,
        _ => keys::STDLIB_PATH_IO_OTHER,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;
    use rstest::rstest;
    use test_support::{localizer_test_lock, set_en_localizer};

    #[rstest]
    #[case(io::Error::new(io::ErrorKind::NotFound, ""), "not found")]
    #[case(
        io::Error::new(io::ErrorKind::PermissionDenied, "permission denied"),
        "permission denied"
    )]
    #[case(
        io::Error::new(io::ErrorKind::UnexpectedEof, "unexpected end of file"),
        "unexpected end of file"
    )]
    fn io_to_error_includes_label(#[case] err: io::Error, #[case] expected_label: &str) {
        let _lock = localizer_test_lock();
        let _guard = set_en_localizer();
        let path = Utf8PathBuf::from("/tmp/example");
        let error = io_to_error(
            path.as_path(),
            &localization::message(keys::STDLIB_PATH_ACTION_READ),
            err,
        );
        assert_eq!(error.kind(), ErrorKind::InvalidOperation);
        let text = error.to_string();
        let expected_action = localization::message(keys::STDLIB_PATH_ACTION_READ).to_string();
        assert!(text.contains(&expected_action));
        assert!(text.contains(expected_label));
    }

    #[rstest]
    #[case(io::ErrorKind::AddrInUse, keys::STDLIB_PATH_IO_ADDR_IN_USE)]
    #[case(io::ErrorKind::Other, keys::STDLIB_PATH_IO_OTHER)]
    fn io_error_kind_label_matches_expected(#[case] kind: io::ErrorKind, #[case] expected: &str) {
        assert_eq!(io_error_kind_label(kind), expected);
    }
}
