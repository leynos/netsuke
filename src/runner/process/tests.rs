//! Unit and property tests for Ninja process helpers.

use super::*;
use camino::Utf8PathBuf;
use proptest::prelude::*;
use std::ffi::OsString;

#[test]
fn resolve_ninja_program_utf8_prefers_env_override() {
    let resolved = resolve_ninja_program_utf8_with(|_| Some(OsString::from("/opt/ninja")));
    assert_eq!(resolved, Utf8PathBuf::from("/opt/ninja"));
}

#[test]
fn resolve_ninja_program_utf8_defaults_without_override() {
    let resolved = resolve_ninja_program_utf8_with(|_| None);
    assert_eq!(resolved, Utf8PathBuf::from(NINJA_PROGRAM));
}

#[test]
fn resolve_ninja_program_utf8_defaults_for_empty_override() {
    let resolved = resolve_ninja_program_utf8_with(|_| Some(OsString::new()));
    assert_eq!(resolved, Utf8PathBuf::from(NINJA_PROGRAM));
}

#[cfg(unix)]
#[test]
fn resolve_ninja_program_utf8_ignores_invalid_utf8_override() {
    use std::os::unix::ffi::OsStringExt;

    let resolved = resolve_ninja_program_utf8_with(|_| {
        Some(OsString::from_vec(vec![0xff, b'n', b'i', b'n', b'j', b'a']))
    });
    assert_eq!(resolved, Utf8PathBuf::from(NINJA_PROGRAM));
}

proptest! {
    #[test]
    fn resolve_ninja_program_utf8_matches_utf8_env_invariant(
        override_value in prop::option::of(".*")
    ) {
        let env_value = override_value.clone().map(OsString::from);
        let expected = match override_value {
            Some(value) if !value.is_empty() => Utf8PathBuf::from(value),
            _ => Utf8PathBuf::from(NINJA_PROGRAM),
        };

        let resolved = resolve_ninja_program_utf8_with(|_| env_value.clone());

        prop_assert_eq!(resolved, expected);
    }
}

#[cfg(unix)]
proptest! {
    #[test]
    fn resolve_ninja_program_utf8_falls_back_for_non_utf8_env_values(
        bytes in prop::collection::vec(any::<u8>(), 0..32)
    ) {
        use std::os::unix::ffi::OsStringExt;

        let env_value = OsString::from_vec(bytes);
        let expected = if env_value.as_os_str().is_empty() {
            Utf8PathBuf::from(NINJA_PROGRAM)
        } else {
            Utf8PathBuf::from_path_buf(PathBuf::from(env_value.clone()))
                .unwrap_or_else(|_| Utf8PathBuf::from(NINJA_PROGRAM))
        };

        let resolved = resolve_ninja_program_utf8_with(|_| Some(env_value.clone()));

        prop_assert_eq!(resolved, expected);
    }
}
