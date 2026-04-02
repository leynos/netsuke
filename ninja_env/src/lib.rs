#![forbid(unsafe_code)]

//! Shared environment constants used across netsuke crates (library, tests, and
//! helpers).

/// Environment variable override for the Ninja executable.
///
/// # Examples
///
/// ```rust,ignore
/// use std::ffi::OsStr;
/// use ninja_env::NINJA_ENV;
/// use test_support::env::VarGuard;
///
/// let _guard = VarGuard::set(NINJA_ENV, OsStr::new("/usr/bin/ninja"));
/// assert_eq!(
///     std::env::var(NINJA_ENV).expect("NINJA_ENV should be set"),
///     "/usr/bin/ninja",
/// );
/// // guard restores prior value on drop
/// ```
pub const NINJA_ENV: &str = "NETSUKE_NINJA";
