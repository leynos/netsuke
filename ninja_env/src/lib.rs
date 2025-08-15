#![forbid(unsafe_code)]

//! Shared environment constants used across netsuke crates (library, tests, and
//! helpers).

/// Environment variable override for the Ninja executable.
///
/// # Examples
///
/// ```
/// use ninja_env::NINJA_ENV;
/// std::env::set_var(NINJA_ENV, "/usr/bin/ninja");
/// assert_eq!(
///     std::env::var(NINJA_ENV).expect("NINJA_ENV should be set"),
///     "/usr/bin/ninja",
/// );
/// std::env::remove_var(NINJA_ENV);
/// ```
pub const NINJA_ENV: &str = "NETSUKE_NINJA";
