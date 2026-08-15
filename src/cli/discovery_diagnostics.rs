//! Bounded diagnostics for configuration discovery.
//!
//! These helpers keep tracing output free of full paths and formatted parser
//! errors: a path contributes only a correlation hash and its file name, and a
//! load failure contributes a [`ConfigLoadFailureKind`] rather than the error
//! text.

use std::collections::hash_map::DefaultHasher;
use std::ffi::OsString;
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

/// Bounded warning metadata retained when an explicit config load fails.
#[derive(Clone, Debug)]
pub(super) struct ConfigLoadWarning {
    path: BoundedConfigPath,
    failure_kind: ConfigLoadFailureKind,
}

impl ConfigLoadWarning {
    /// Capture a load failure without retaining its raw path or error text.
    pub(super) fn new(path: &Path, failure_kind: ConfigLoadFailureKind) -> Self {
        Self {
            path: BoundedConfigPath::from_path(Some(path)),
            failure_kind,
        }
    }

    /// Emit the fixed explicit-load warning from bounded metadata.
    pub(super) fn emit(&self) {
        warn_explicit_config_load_failed_from_fields(&self.path, self.failure_kind);
    }
}

/// Bounded path fields retained for deferred discovery diagnostics.
///
/// This stores only the correlation hash, file name, and presence bit needed
/// to replay a diagnostic event. It deliberately excludes the full path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct BoundedConfigPath {
    pub(super) hash: Option<String>,
    pub(super) file_name: Option<OsString>,
    pub(super) is_present: bool,
}

impl BoundedConfigPath {
    /// Capture bounded fields from an optional path without retaining it.
    pub(super) fn from_path(path: Option<&Path>) -> Self {
        Self {
            hash: path.map(path_hash),
            file_name: path.and_then(Path::file_name).map(OsString::from),
            is_present: path.is_some(),
        }
    }
}

/// Replay one environment lookup from retained bounded fields.
pub(super) fn trace_config_path_variable_from_fields(var_name: &str, path: &BoundedConfigPath) {
    trace!(
        var_name,
        found = path.is_present,
        path_hash = path.hash.as_deref(),
        path_file_name = ?path.file_name,
        "read config path variable"
    );
}

/// Replay the unchanged explicit-load warning from bounded metadata.
pub(super) fn warn_explicit_config_load_failed_from_fields(
    path: &BoundedConfigPath,
    failure_kind: ConfigLoadFailureKind,
) {
    let path_hash = path.hash.as_deref().unwrap_or_default();
    warn!(
        path_hash = %path_hash,
        path_file_name = ?path.file_name,
        failure_kind = ?failure_kind,
        "explicit config load failed"
    );
}

/// Replay an explicit-path diagnostic from retained bounded fields.
pub(super) fn debug_config_path_from_fields(message: &'static str, path: &BoundedConfigPath) {
    let path_hash = path.hash.as_deref().unwrap_or_default();
    debug!(
        path_hash = %path_hash,
        path_file_name = ?path.file_name,
        message
    );
}

/// Replay an optional project-scope path diagnostic from bounded fields.
pub(super) fn debug_optional_config_path_from_fields(
    message: &'static str,
    path: &BoundedConfigPath,
) {
    debug!(
        path_hash = path.hash.as_deref(),
        path_file_name = ?path.file_name,
        path_present = path.is_present,
        message
    );
}

/// Return a stable-width correlation identifier for `value`.
///
/// This unkeyed hash does not conceal or confidentially redact guessable
/// values. Its purpose is limited to bounding log cardinality and correlating
/// events within one run; it is not a cryptographic digest or security
/// boundary.
pub(super) fn short_hash(value: &[u8]) -> String {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Return the bounded, run-local correlation hash for `path`.
///
/// The unkeyed hash does not conceal or confidentially redact a guessable path.
/// It only bounds log cardinality and correlates events within one run; it is
/// neither a cryptographic digest nor a security boundary.
pub(super) fn path_hash(path: &Path) -> String {
    short_hash(path.to_string_lossy().as_bytes())
}
