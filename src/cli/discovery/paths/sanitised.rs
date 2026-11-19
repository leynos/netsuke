//! Environment view that hides unusable user base values from the fallback.
//!
//! `OrthoConfig` 0.10.0 filters only empty base values, so its own scan would
//! resolve a relative one against the working directory and load configuration
//! from wherever the tool happened to run. [`SanitisedUserBases`] applies the
//! predicate the Netsuke user pass uses, so a value that pass rejected cannot
//! come back through the dependency's fallback lookups.

use ortho_config::{EnvSource, SharedEnvSource};
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::{Path, PathBuf};

/// The user base variables whose unusable values must read as unset.
///
/// The list mirrors the bases `paths::user_scope_candidates` derives from. The
/// system-scope variable is deliberately absent: its handling stays with the
/// dependency.
const USER_BASE_KEYS: [&str; 5] = [
    "XDG_CONFIG_HOME",
    "APPDATA",
    "LOCALAPPDATA",
    "HOME",
    "USERPROFILE",
];

/// Report whether `value` may be used as a configuration base directory.
///
/// This is the single predicate behind both the candidate derivation and the
/// sanitised view handed to the dependency's fallback, so the two cannot
/// disagree about which bases are usable. The predicate is `is_absolute()`
/// rather than `has_root()`: on Windows a bare leading separator such as
/// `\config` is rooted but carries no drive, so it resolves on whichever drive
/// the process happens to run from.
pub(in crate::cli::discovery) fn is_usable_base(value: &OsStr) -> bool {
    Path::new(value).is_absolute()
}

/// An environment view that hides unusable user base values from the fallback.
///
/// Handing the dependency the raw environment would let its own scan resolve a
/// relative base against the working directory, the hazard the user pass's
/// filter exists to remove. This view applies the same predicate to the
/// dependency's lookups.
pub(in crate::cli::discovery) struct SanitisedUserBases {
    /// The environment whose lookups this view filters.
    source: SharedEnvSource,
}

impl SanitisedUserBases {
    /// Wrap `source` so unusable user base values read as unset.
    pub(in crate::cli::discovery) fn new(source: SharedEnvSource) -> Self {
        Self { source }
    }
}

impl fmt::Debug for SanitisedUserBases {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SanitisedUserBases")
            .field("source", &self.source)
            .finish()
    }
}

impl EnvSource for SanitisedUserBases {
    fn get(&self, key: &str) -> Option<OsString> {
        let value = self.source.get(key)?;
        if USER_BASE_KEYS.contains(&key) && !is_usable_base(&value) {
            return None;
        }
        Some(value)
    }

    fn home_fallback(&self) -> Option<PathBuf> {
        self.source.home_fallback()
    }

    fn config_dir_fallback(&self) -> Option<PathBuf> {
        self.source.config_dir_fallback()
    }
}
