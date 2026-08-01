//! Stub locale providers for tests.
//!
//! These helpers implement the locale-resolution traits so tests can inject
//! deterministic environment and system locales.

use netsuke::locale_resolution::{self, LocaleEnvProvider, SystemLocale};
use std::collections::HashMap;

/// Stub environment provider for locale resolution.
///
/// Answers only the variables it was given, and **panics** on any other key.
///
/// The permissive alternative — returning `None` for anything unrecognised —
/// hides exactly the change a test double should catch. Were the code under
/// test altered to read a differently-named variable, through a rename, a typo,
/// or a new precedence rung, a permissive stub would quietly answer `None` and
/// the test would still pass while asserting nothing about the new read. The
/// panic converts that silent pass into a failure naming the unexpected key.
///
/// `Default` is deliberately **not** implemented. On a strict stub it would
/// mean "deny every read", so `StubEnv::default()` would compile and then
/// panic at run time for the common "no locale set" case. Requiring
/// [`StubEnv::without_locale`] makes that intent explicit at compile time.
#[derive(Debug, Clone)]
pub struct StubEnv {
    values: HashMap<String, String>,
    allowed: Vec<String>,
}

impl StubEnv {
    /// Create a stub declaring nothing; every read panics until one is added.
    #[must_use]
    pub fn strict() -> Self {
        Self {
            values: HashMap::new(),
            allowed: Vec::new(),
        }
    }

    /// Create a stub answering `NETSUKE_LOCALE` with `locale`.
    #[must_use]
    pub fn with_locale(locale: impl Into<String>) -> Self {
        Self::strict().with_var(locale_resolution::NETSUKE_LOCALE_ENV, locale)
    }

    /// Create a stub in which `NETSUKE_LOCALE` is unset but may be read.
    ///
    /// Distinct from a stub that never expected the read at all: an unset
    /// variable is a legitimate case to exercise.
    #[must_use]
    pub fn without_locale() -> Self {
        Self::strict().allowing(locale_resolution::NETSUKE_LOCALE_ENV)
    }

    /// Answer `key` with `value`.
    #[must_use]
    pub fn with_var(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let key = key.into();
        self.allowed.push(key.clone());
        self.values.insert(key, value.into());
        self
    }

    /// Permit `key` to be read, reporting it as unset.
    ///
    /// Needed because an unset variable is a legitimate case to test, and must
    /// be distinguishable from a variable the test never expected to be read.
    #[must_use]
    pub fn allowing(mut self, key: impl Into<String>) -> Self {
        self.allowed.push(key.into());
        self
    }
}

impl LocaleEnvProvider for StubEnv {
    fn var(&self, key: &str) -> Option<String> {
        assert!(
            self.allowed.iter().any(|allowed| allowed == key),
            "StubEnv was asked for {key:?}, which the test did not declare. \
             Declare it with `.with_var(..)` or `.allowing(..)` if the read is \
             intended; otherwise the code under test is reading a variable the \
             test does not know about."
        );
        self.values.get(key).cloned()
    }
}

/// Stub system locale provider for locale resolution.
#[derive(Debug, Default, Clone)]
pub struct StubSystemLocale {
    /// Optional system locale value to return.
    pub locale: Option<String>,
}

impl StubSystemLocale {
    /// Create a stub system locale with the provided value.
    pub fn with_locale(locale: impl Into<String>) -> Self {
        Self {
            locale: Some(locale.into()),
        }
    }
}

impl SystemLocale for StubSystemLocale {
    fn system_locale(&self) -> Option<String> {
        self.locale.clone()
    }
}
