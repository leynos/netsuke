//! Ninja executable resolution at the runner's environment boundary.
//!
//! This module owns selection between `NETSUKE_NINJA` and the default program.
//! Process construction consumes only its resolved paths; other adapters must
//! not read or interpret the override independently.
//!
//! The environment reaches the resolver through [`mockable::Env`] (#488):
//! production supplies [`mockable::DefaultEnv`], the crate's one explicit
//! process-environment adapter for this boundary, and tests supply
//! `mockable::MockEnv` so every branch runs without process mutation.

use super::super::{NINJA_ENV, NINJA_PROGRAM};
use camino::Utf8PathBuf;
use mockable::Env;
use std::path::PathBuf;
use tracing::debug;

/// Resolves Ninja with an injectable environment for testability.
///
/// This variant avoids mutating the process environment when testing
/// resolution behaviour. It selects a non-empty UTF-8 `NETSUKE_NINJA`
/// override, falls back to [`NINJA_PROGRAM`] when the override is unset,
/// empty, or non-UTF-8, and emits resolution diagnostics at this boundary.
pub(super) fn resolve_ninja_program_utf8_with(env: &impl Env) -> Utf8PathBuf {
    env.os_string(NINJA_ENV).map_or_else(
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
                    ninja_program = NINJA_PROGRAM,
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
                            ninja_program = NINJA_PROGRAM,
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

/// Resolve Ninja as a general platform path with an injectable environment.
///
/// Compiled for tests only: production reaches the platform-path form through
/// [`resolve_ninja_program`], which shares the UTF-8 resolution below.
#[cfg(test)]
pub(super) fn resolve_ninja_program_with(env: &impl Env) -> PathBuf {
    resolve_ninja_program_utf8_with(env).into()
}

/// Resolve the configured Ninja executable as a UTF-8 path.
#[must_use]
pub fn resolve_ninja_program_utf8() -> Utf8PathBuf {
    resolve_ninja_program_utf8_with(&mockable::DefaultEnv)
}

/// Resolve the configured Ninja executable as a general platform path.
#[must_use]
pub fn resolve_ninja_program() -> PathBuf {
    resolve_ninja_program_utf8().into()
}
#[cfg(test)]
mod tests {
    //! Tests for Ninja executable-resolution tracing.

    use super::*;
    use crate::test_tracing_capture::with_test_subscriber;
    use mockable::MockEnv;
    use proptest::prelude::*;
    use rstest::{fixture, rstest};
    use std::ffi::OsString;
    #[cfg(unix)]
    use std::path::PathBuf;
    use tracing_subscriber::filter::LevelFilter;

    /// Build an environment that returns `value` for exactly one Ninja lookup.
    ///
    /// The key expectation is part of the contract (#488): a resolver that
    /// reads any other variable, or reads more than once, fails these tests
    /// rather than silently consulting something else. Consumers override the
    /// answer with `#[with(...)]`; `None` models an unset variable.
    #[fixture]
    fn ninja_env(#[default(None)] value: Option<OsString>) -> MockEnv {
        let mut env = MockEnv::new();
        env.expect_os_string()
            .times(1)
            .withf(|key| key == NINJA_ENV)
            .return_const(value);
        env
    }

    /// Verify override resolution records its selected program and source.
    #[rstest]
    fn resolve_ninja_program_utf8_logs_override_resolution(
        #[with(Some(OsString::from("/opt/ninja")))] ninja_env: MockEnv,
    ) {
        let (resolved, events) = with_test_subscriber(LevelFilter::DEBUG, |captured| {
            let resolved = resolve_ninja_program_utf8_with(&ninja_env);
            (resolved, captured.snapshot())
        });

        assert_eq!(resolved, Utf8PathBuf::from("/opt/ninja"));
        let [event] = events.as_slice() else {
            panic!("expected one Ninja-resolution event, got {events:?}");
        };
        assert!(
            event.contains("ninja_program=/opt/ninja")
                && event.contains("source=\"NETSUKE_NINJA\""),
            "override resolution should identify its program and source: {event}"
        );
    }

    /// Verify fallback resolution records its selected program and source.
    #[rstest]
    fn resolve_ninja_program_utf8_logs_fallback_resolution(ninja_env: MockEnv) {
        let (resolved, events) = with_test_subscriber(LevelFilter::DEBUG, |captured| {
            let resolved = resolve_ninja_program_utf8_with(&ninja_env);
            (resolved, captured.snapshot())
        });

        assert_eq!(resolved, Utf8PathBuf::from(NINJA_PROGRAM));
        let [event] = events.as_slice() else {
            panic!("expected one Ninja-resolution event, got {events:?}");
        };
        assert!(
            event.contains(&format!("ninja_program={NINJA_PROGRAM:?}"))
                && event.contains("source=\"fallback\""),
            "fallback resolution should identify its program and source: {event}"
        );
    }

    /// Verify UTF-8 resolution favours the non-empty environment override.
    #[rstest]
    fn resolve_ninja_program_utf8_prefers_env_override(
        #[with(Some(OsString::from("/opt/ninja")))] ninja_env: MockEnv,
    ) {
        let resolved = resolve_ninja_program_utf8_with(&ninja_env);
        assert_eq!(resolved, Utf8PathBuf::from("/opt/ninja"));
    }

    /// Verify UTF-8 resolution defaults when the override is absent.
    #[rstest]
    fn resolve_ninja_program_utf8_defaults_without_override(ninja_env: MockEnv) {
        let resolved = resolve_ninja_program_utf8_with(&ninja_env);
        assert_eq!(resolved, Utf8PathBuf::from(NINJA_PROGRAM));
    }

    /// Verify UTF-8 resolution defaults when the override is empty.
    #[rstest]
    fn resolve_ninja_program_utf8_defaults_for_empty_override(
        #[with(Some(OsString::new()))] ninja_env: MockEnv,
    ) {
        let resolved = resolve_ninja_program_utf8_with(&ninja_env);
        assert_eq!(resolved, Utf8PathBuf::from(NINJA_PROGRAM));
    }

    /// Verify UTF-8 resolution defaults when the override is not UTF-8.
    #[cfg(unix)]
    #[rstest]
    fn resolve_ninja_program_utf8_ignores_invalid_utf8_override(
        #[with(Some(invalid_utf8_override()))] ninja_env: MockEnv,
    ) {
        let resolved = resolve_ninja_program_utf8_with(&ninja_env);
        assert_eq!(resolved, Utf8PathBuf::from(NINJA_PROGRAM));
    }

    /// Build a non-UTF-8 override value for Unix-only resolution tests.
    #[cfg(unix)]
    fn invalid_utf8_override() -> OsString {
        use std::os::unix::ffi::OsStringExt;

        OsString::from_vec(vec![0xff, b'n', b'i', b'n', b'j', b'a'])
    }

    /// Verify platform-path resolution shares the UTF-8 resolver's result.
    #[rstest]
    fn resolve_ninja_program_with_converts_the_resolved_path(
        #[with(Some(OsString::from("/opt/ninja")))] ninja_env: MockEnv,
    ) {
        let resolved = resolve_ninja_program_with(&ninja_env);
        assert_eq!(resolved, PathBuf::from("/opt/ninja"));
    }

    // `proptest!` owns the generated function signature, so rstest cannot
    // inject the fixture. Calling it directly keeps the one-read, exact-key
    // contract without weakening property coverage.
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

            let resolved = resolve_ninja_program_utf8_with(&ninja_env(env_value));

            prop_assert_eq!(resolved, expected);
        }
    }

    // As above, the fixture is called directly because `proptest!` generates
    // the function signature and leaves no parameter for rstest to inject.
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

            let resolved = resolve_ninja_program_utf8_with(&ninja_env(Some(env_value)));

            prop_assert_eq!(resolved, expected);
        }
    }
}
