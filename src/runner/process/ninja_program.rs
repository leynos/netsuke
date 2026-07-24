//! Ninja executable resolution at the runner's environment boundary.
//!
//! This module owns selection between `NETSUKE_NINJA` and the default program.
//! Process construction consumes only its resolved paths; other adapters must
//! not read or interpret the override independently.

use super::super::{NINJA_ENV, NINJA_PROGRAM};
use camino::Utf8PathBuf;
use std::{env, ffi::OsString, path::PathBuf};
use tracing::debug;

/// Resolves Ninja with an injectable environment reader for testability.
///
/// This variant avoids mutating the process environment when testing
/// resolution behaviour. It selects a non-empty UTF-8 `NETSUKE_NINJA`
/// override, falls back to [`NINJA_PROGRAM`] when the override is unset,
/// empty, or non-UTF-8, and emits resolution diagnostics at this boundary.
pub(super) fn resolve_ninja_program_utf8_with<F>(mut read_env: F) -> Utf8PathBuf
where
    F: FnMut(&str) -> Option<OsString>,
{
    read_env(NINJA_ENV).map_or_else(
        || {
            debug!(
                ninja_program = NINJA_PROGRAM,
                source = "fallback",
                "Resolved Ninja executable from default program",
            );
            Utf8PathBuf::from(NINJA_PROGRAM)
        },
        |value| {
            let path = PathBuf::from(value);
            if path.as_os_str().is_empty() {
                debug!(
                    fallback_program = NINJA_PROGRAM,
                    source = "fallback",
                    "Ignoring empty Ninja executable override",
                );
                Utf8PathBuf::from(NINJA_PROGRAM)
            } else {
                match Utf8PathBuf::from_path_buf(path) {
                    Ok(program) => {
                        debug!(
                            ninja_program = %program,
                            source = NINJA_ENV,
                            "Resolved Ninja executable from environment override",
                        );
                        program
                    }
                    Err(non_utf8_path) => {
                        debug!(
                            configured_ninja = %non_utf8_path.to_string_lossy(),
                            fallback_program = NINJA_PROGRAM,
                            source = "fallback",
                            "Ignoring non-UTF-8 Ninja executable override",
                        );
                        Utf8PathBuf::from(NINJA_PROGRAM)
                    }
                }
            }
        },
    )
}

/// Resolve the configured Ninja executable as a UTF-8 path.
#[must_use]
pub fn resolve_ninja_program_utf8() -> Utf8PathBuf {
    resolve_ninja_program_utf8_with(|key| env::var_os(key))
}

/// Resolve the configured Ninja executable as a general platform path.
#[must_use]
pub fn resolve_ninja_program() -> PathBuf {
    resolve_ninja_program_utf8().into()
}
