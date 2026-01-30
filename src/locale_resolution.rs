//! Locale resolution helpers for CLI parsing and runtime diagnostics.
//!
//! These helpers centralize locale precedence rules and normalization so both
//! clap help and runtime diagnostics resolve the same locale.

use crate::cli;
use crate::cli::Cli;
use ortho_config::LanguageIdentifier;
use std::ffi::OsString;
use std::str::FromStr;

/// Environment variable name used to override the locale.
pub const NETSUKE_LOCALE_ENV: &str = "NETSUKE_LOCALE";

/// Read-only environment access used for locale resolution.
pub trait EnvProvider {
    /// Fetch the environment variable value for `key`.
    fn var(&self, key: &str) -> Option<String>;
}

/// Environment provider backed by the process environment.
#[derive(Debug, Default, Copy, Clone)]
pub struct SystemEnv;

impl EnvProvider for SystemEnv {
    fn var(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }
}

/// System locale provider for the current host.
pub trait SystemLocale {
    /// Return the system locale string when available.
    fn system_locale(&self) -> Option<String>;
}

/// System locale provider backed by `sys-locale`.
#[derive(Debug, Default, Copy, Clone)]
pub struct SysLocale;

impl SystemLocale for SysLocale {
    fn system_locale(&self) -> Option<String> {
        sys_locale::get_locale()
    }
}

/// Normalize a raw locale string into a valid BCP 47 language tag.
///
/// This strips encoding suffixes (for example `.UTF-8`), removes variant
/// sections (for example `@latin`), replaces underscores with hyphens, and
/// validates the result using `LanguageIdentifier`.
///
/// # Examples
///
/// ```rust
/// use netsuke::locale_resolution::normalize_locale_tag;
///
/// assert_eq!(normalize_locale_tag("en_US.UTF-8"), Some("en-US".to_string()));
/// assert_eq!(normalize_locale_tag("es-ES"), Some("es-ES".to_string()));
/// assert_eq!(normalize_locale_tag("invalid"), None);
/// ```
#[must_use]
pub fn normalize_locale_tag(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let stripped = trimmed.split(['.', '@']).next().unwrap_or_default().trim();
    if stripped.is_empty() {
        return None;
    }
    let candidate = stripped.replace('_', "-");
    LanguageIdentifier::from_str(&candidate)
        .ok()
        .map(|lang| lang.to_string())
}

fn select_locale<'a>(candidates: impl IntoIterator<Item = Option<&'a str>>) -> Option<String> {
    for raw in candidates.into_iter().flatten() {
        if let Some(normalized) = normalize_locale_tag(raw) {
            return Some(normalized);
        }
    }
    None
}

/// Resolve the locale used for clap parsing (help and error messages).
///
/// Precedence is `--locale` (when supplied) followed by `NETSUKE_LOCALE`, and
/// finally the system default. The returned locale is normalized; when no
/// valid locale is found, `None` is returned so callers fall back to English.
///
/// # Examples
///
/// ```rust
/// use netsuke::locale_resolution::{resolve_startup_locale, EnvProvider, SystemLocale};
/// use std::ffi::OsString;
///
/// struct StubEnv(Option<String>);
/// impl EnvProvider for StubEnv {
///     fn var(&self, key: &str) -> Option<String> {
///         (key == "NETSUKE_LOCALE").then(|| self.0.clone()).flatten()
///     }
/// }
///
/// struct StubSystem(Option<String>);
/// impl SystemLocale for StubSystem {
///     fn system_locale(&self) -> Option<String> {
///         self.0.clone()
///     }
/// }
///
/// let args = vec![
///     OsString::from("netsuke"),
///     OsString::from("--locale"),
///     OsString::from("es-ES"),
/// ];
/// let locale = resolve_startup_locale(
///     &args,
///     &StubEnv(None),
///     &StubSystem(Some("en_US".into())),
/// );
/// assert_eq!(locale.as_deref(), Some("es-ES"));
/// ```
#[must_use]
pub fn resolve_startup_locale(
    args: &[OsString],
    env: &impl EnvProvider,
    system: &impl SystemLocale,
) -> Option<String> {
    let cli_hint = cli::locale_hint_from_args(args);
    let env_locale = env.var(NETSUKE_LOCALE_ENV);
    let system_locale = system.system_locale();
    select_locale([
        cli_hint.as_deref(),
        env_locale.as_deref(),
        system_locale.as_deref(),
    ])
}

/// Resolve the locale used for runtime diagnostics.
///
/// The merged CLI configuration already includes configuration files,
/// environment variables, and explicit CLI overrides. When no valid locale is
/// present in the merged configuration, the system default is used. The
/// returned locale is normalized; when no valid locale is found, `None` is
/// returned so callers fall back to English.
///
/// # Examples
///
/// ```rust
/// use netsuke::cli::Cli;
/// use netsuke::locale_resolution::{resolve_runtime_locale, SystemLocale};
///
/// struct StubSystem(Option<String>);
/// impl SystemLocale for StubSystem {
///     fn system_locale(&self) -> Option<String> {
///         self.0.clone()
///     }
/// }
///
/// let cli = Cli { locale: Some("es-ES".to_string()), ..Cli::default() };
/// let locale = resolve_runtime_locale(&cli, &StubSystem(Some("en_US".into())));
/// assert_eq!(locale.as_deref(), Some("es-ES"));
/// ```
#[must_use]
pub fn resolve_runtime_locale(merged: &Cli, system: &impl SystemLocale) -> Option<String> {
    let system_locale = system.system_locale();
    select_locale([merged.locale.as_deref(), system_locale.as_deref()])
}
