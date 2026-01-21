//! Helpers for rendering user-facing strings via Fluent resources.
//!
//! This module owns the global `Localizer` handle used throughout Netsuke so
//! errors and diagnostics can render translated copy without threading
//! localizer references everywhere. The default localizer uses the embedded
//! English catalogue, while callers can override it (for example in `main`) to
//! respect `--locale` or `NETSUKE_LOCALE`.

pub mod keys;

use ortho_config::{LocalizationArgs, Localizer};
use std::fmt;
use std::sync::{Arc, OnceLock, RwLock};

static LOCALIZER: OnceLock<RwLock<Arc<dyn Localizer>>> = OnceLock::new();

fn localizer_storage() -> &'static RwLock<Arc<dyn Localizer>> {
    // Keep the key registry referenced so dead-code lints do not discard it.
    let _ = keys::ALL_KEYS;
    LOCALIZER.get_or_init(|| {
        let default = crate::cli_localization::build_localizer(None);
        RwLock::new(Arc::from(default))
    })
}

/// Replace the global localizer used for error rendering.
pub fn set_localizer(localizer: Arc<dyn Localizer>) {
    let lock = localizer_storage();
    let mut guard = lock
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *guard = localizer;
}

/// Returns the active localizer.
#[must_use]
pub fn localizer() -> Arc<dyn Localizer> {
    let lock = localizer_storage();
    let guard = lock
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    Arc::clone(&guard)
}

/// Scoped helper that restores the previous localizer when dropped.
pub struct LocalizerGuard {
    previous: Arc<dyn Localizer>,
}

impl Drop for LocalizerGuard {
    fn drop(&mut self) {
        set_localizer(Arc::clone(&self.previous));
    }
}

/// Override the localizer within a test scope.
#[must_use]
pub fn set_localizer_for_tests(new_localizer: Arc<dyn Localizer>) -> LocalizerGuard {
    let previous = localizer();
    set_localizer(new_localizer);
    LocalizerGuard { previous }
}

// Compile-time assertions that the public setters keep their signatures.
const _: fn(Arc<dyn Localizer>) = set_localizer;
const _: fn(Arc<dyn Localizer>) -> LocalizerGuard = set_localizer_for_tests;

/// Render a Fluent message key with optional arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalizedMessage {
    key: &'static str,
    args: Vec<(&'static str, String)>,
}

impl LocalizedMessage {
    /// Create a new localized message with no arguments.
    #[must_use]
    pub const fn new(key: &'static str) -> Self {
        Self {
            key,
            args: Vec::new(),
        }
    }

    /// Attach a named argument to the Fluent lookup.
    #[must_use]
    #[expect(
        clippy::needless_pass_by_value,
        reason = "Accepting owned values keeps call sites ergonomic for temporaries."
    )]
    pub fn with_arg(mut self, name: &'static str, value: impl ToString) -> Self {
        self.args.push((name, value.to_string()));
        self
    }

    fn args_map(&self) -> Option<LocalizationArgs<'_>> {
        if self.args.is_empty() {
            return None;
        }
        let mut args = LocalizationArgs::default();
        for (name, value) in &self.args {
            args.insert(*name, value.clone().into());
        }
        Some(args)
    }
}

impl fmt::Display for LocalizedMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let localizer = localizer();
        let args = self.args_map();
        let message = localizer.message(self.key, args.as_ref(), self.key);
        f.write_str(&message)
    }
}

/// Convenience helper to build a localized message.
#[must_use]
pub const fn message(key: &'static str) -> LocalizedMessage {
    LocalizedMessage::new(key)
}
