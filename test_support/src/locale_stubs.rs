//! Stub locale providers for tests.
//!
//! These helpers implement the locale-resolution traits so tests can inject
//! deterministic environment and system locales.

use netsuke::locale_resolution::{self, EnvProvider, SystemLocale};

/// Stub environment provider for locale resolution.
#[derive(Debug, Default, Clone)]
pub struct StubEnv {
    /// Optional locale value to return for `NETSUKE_LOCALE`.
    pub locale: Option<String>,
}

impl StubEnv {
    /// Create a stub environment with the provided locale.
    pub fn with_locale(locale: impl Into<String>) -> Self {
        Self {
            locale: Some(locale.into()),
        }
    }
}

impl EnvProvider for StubEnv {
    fn var(&self, key: &str) -> Option<String> {
        if key == locale_resolution::NETSUKE_LOCALE_ENV {
            return self.locale.clone();
        }
        None
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
