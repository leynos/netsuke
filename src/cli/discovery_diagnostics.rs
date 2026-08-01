//! Bounded diagnostics for configuration discovery.
//!
//! These helpers keep tracing output free of full paths and formatted parser
//! errors: a path contributes only a correlation hash and its file name, and a
//! load failure contributes a [`ConfigLoadFailureKind`] rather than the error
//! text.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;
use tracing::{debug, trace, warn};

/// Classifies an explicit configuration load failure without retaining error text.
///
/// An absent file is [`Self::Missing`]. Every other failure to load or parse the
/// selected file is [`Self::LoadError`], covering malformed syntax in any
/// supported format as well as I/O and permission errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ConfigLoadFailureKind {
    /// The selected configuration file does not exist.
    Missing,
    /// The selected file exists but could not be loaded or parsed.
    LoadError,
}

/// Trace one environment lookup using bounded path fields.
pub(super) fn trace_config_path_variable(var_name: &str, path: Option<&Path>) {
    trace!(
        var_name,
        found = path.is_some(),
        path_hash = path.map(path_hash).as_deref(),
        path_file_name = ?path.and_then(Path::file_name),
        "read config path variable"
    );
}

/// Warn that an explicit `path` failed with `failure_kind`.
///
/// The event exposes the failure class, file name, and correlation hash, but
/// neither the full path nor the formatted parser or I/O error.
pub(super) fn warn_explicit_config_load_failed(path: &Path, failure_kind: ConfigLoadFailureKind) {
    warn!(
        path_hash = %path_hash(path),
        path_file_name = ?path.file_name(),
        failure_kind = ?failure_kind,
        "explicit config load failed"
    );
}

/// Emit `message` with bounded fields identifying `path`.
pub(super) fn debug_config_path(message: &'static str, path: &Path) {
    debug!(
        path_hash = %path_hash(path),
        path_file_name = ?path.file_name(),
        message
    );
}

/// Emit `message` with presence and bounded fields for an optional path string.
pub(super) fn debug_optional_config_path(message: &'static str, path: Option<&str>) {
    debug!(
        path_hash = path.map(|value| short_hash(value.as_bytes())).as_deref(),
        path_file_name = ?path.and_then(|value| Path::new(value).file_name()),
        path_present = path.is_some(),
        message
    );
}

/// Return a stable-width correlation identifier for `value`.
///
/// This bounds log cardinality; it is not a cryptographic digest and must not
/// be used as a security boundary.
pub(super) fn short_hash(value: &[u8]) -> String {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Return the bounded correlation hash for `path`.
pub(super) fn path_hash(path: &Path) -> String {
    short_hash(path.to_string_lossy().as_bytes())
}
