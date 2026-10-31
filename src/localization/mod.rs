//! Helpers for rendering user-facing strings via Fluent resources.
//!
//! This module owns the global `Localizer` handle used throughout Netsuke so
//! errors and diagnostics can render translated copy without threading
//! localizer references everywhere. The default localizer uses the embedded
//! English catalogue, while callers can override it (for example in `main`) to
//! respect `--locale` or `NETSUKE_LOCALE`.

/// Declare Fluent message keys and their stable string identifiers.
///
/// Defined here rather than in `keys` so that file stays a plain table of
/// keys; `macro_rules!` scoping is textual, so the child module sees it.
macro_rules! define_keys {
    ($($name:ident => $value:literal,)+) => {
        $(#[doc = "Fluent message key."] pub const $name: &str = $value;)+
        /// Every Fluent message key this table declares, in declaration order.
        ///
        /// Each table carries its own slice because the key set is split
        /// across four files; `ALL_KEYS` is the union.
        pub const TABLE_KEYS: &[&str] = &[$($name),+];
    };
}

mod check_keys;

mod clap_error_keys;

mod manifest_keys;

pub mod keys;

/// The locale registry, which lives at the crate root so that it depends on
/// nothing: this module reaches `cli_localization` to build the default
/// localizer, and `cli_localization` reads the registry. Keeping the registry
/// here would close that loop into a cycle.
pub use crate::locale_catalogues as locales;

use ortho_config::{LocalizationArgs, Localizer};
use std::fmt;
use std::sync::{Arc, OnceLock, RwLock};

/// All Fluent message keys referenced by Netsuke.
///
/// Each table publishes its own `TABLE_KEYS` slice, and this is their union.
/// The split into `check_keys`, `clap_error_keys`, and `manifest_keys` exists
/// only to keep every module inside Whitaker's line cap, so a key moved into
/// any table has to stay in this set — which is why the union is assembled
/// from the table slices rather than shadowed by whichever `define_keys!` runs
/// last. It lives here rather than in `keys` because no single table owns it.
///
/// The union is assembled at compile time, so the type stays the plain slice
/// the single table used to publish: callers can still take `ALL_KEYS` as a
/// `&'static [&'static str]` rather than reaching through a lazily built
/// `Vec`.
pub const ALL_KEYS: &[&str] = &ALL_KEYS_ARRAY;

/// Number of keys in [`ALL_KEYS`], summed from every table.
const ALL_KEYS_LEN: usize = keys::TABLE_KEYS.len()
    + check_keys::TABLE_KEYS.len()
    + clap_error_keys::TABLE_KEYS.len()
    + manifest_keys::TABLE_KEYS.len();

/// Every declared key, concatenated in table order.
const ALL_KEYS_ARRAY: [&str; ALL_KEYS_LEN] = assemble_all_keys();

/// Concatenate the key tables into one array at compile time.
///
/// Slices cannot be joined in a constant, so the tables are copied element by
/// element. A `const fn` is what keeps the published type unchanged; deferring
/// the same work to a `LazyLock` would expose the `Vec` it builds instead.
const fn assemble_all_keys() -> [&'static str; ALL_KEYS_LEN] {
    let mut all = [""; ALL_KEYS_LEN];
    let after_general = append_keys(&mut all, 0, keys::TABLE_KEYS);
    let after_check = append_keys(&mut all, after_general, check_keys::TABLE_KEYS);
    let after_clap = append_keys(&mut all, after_check, clap_error_keys::TABLE_KEYS);
    let _ = append_keys(&mut all, after_clap, manifest_keys::TABLE_KEYS);
    all
}

/// Copy `table` into `all` from `start`, returning the next free index.
///
/// The destination is sized as the sum of the tables, so every index
/// this writes is in bounds. Indexing is nonetheless the only form that works
/// here: `slice::get` and `slice::get_mut` are not yet `const`, and a
/// `LazyLock` alternative would change the published type of [`ALL_KEYS`],
/// which is the regression this function exists to avoid. The suppression is
/// therefore confined to this one copy loop.
#[expect(
    clippy::indexing_slicing,
    reason = "the destination length is the exact sum of the tables copied into it"
)]
const fn append_keys(
    all: &mut [&'static str; ALL_KEYS_LEN],
    start: usize,
    table: &'static [&'static str],
) -> usize {
    let mut offset = 0;
    while offset < table.len() {
        all[start + offset] = table[offset];
        offset += 1;
    }
    start + table.len()
}

/// Compile-time assertion that the aggregate keeps its published slice type.
const _: &[&str] = ALL_KEYS;

/// Process-global localizer storage, initialized on first use.
static LOCALIZER: OnceLock<RwLock<Arc<dyn Localizer>>> = OnceLock::new();

/// Return the process-global localizer storage, initializing it with the
/// system default when first accessed.
fn localizer_storage() -> &'static RwLock<Arc<dyn Localizer>> {
    // Keep the key registry referenced so dead-code lints do not discard it.
    let _ = keys::ALL_KEYS;
    let _ = clap_error_keys::TABLE_KEYS;
    let _ = check_keys::TABLE_KEYS;
    let _ = manifest_keys::TABLE_KEYS;
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
    /// Localizer in effect before the guard was created.
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

// Compile-time assertions that the public setters keep their signatures.
/// Compile-time assertion that `set_localizer` keeps its signature.
const _: fn(Arc<dyn Localizer>) = set_localizer;

/// Compile-time assertion that `set_localizer_for_tests` keeps its signature.
const _: fn(Arc<dyn Localizer>) -> LocalizerGuard = set_localizer_for_tests;


/// Render a Fluent message key with optional arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalizedMessage {
    /// Fluent message key to look up.
    key: &'static str,
    /// Named arguments to interpolate into the message.
    args: Vec<(&'static str, MessageArg)>,
}

/// One named argument's value, kept typed until the Fluent lookup.
///
/// Fluent only selects a CLDR plural variant (`[one]`, `[few]`, ...) for a
/// numeric argument; a string argument matches no plural category and always
/// falls through to the default variant. Counts are therefore carried as
/// numbers rather than pre-rendered text.
#[derive(Debug, Clone, PartialEq, Eq)]
enum MessageArg {
    /// Text interpolated verbatim.
    Text(String),
    /// A count that selects plural variants.
    Count(usize),
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
        self.args.push((name, MessageArg::Text(value.to_string())));
        self
    }

    /// Attach a named count, which Fluent can use to select a plural variant.
    ///
    /// ```
    /// use netsuke::localization::{self, keys};
    ///
    /// let message = localization::message(keys::EXAMPLE_FILES_PROCESSED).with_count("count", 1);
    /// assert!(message.to_string().contains('1'));
    /// ```
    #[must_use]
    pub fn with_count(mut self, name: &'static str, count: usize) -> Self {
        self.args.push((name, MessageArg::Count(count)));
        self
    }

    /// Build the `LocalizationArgs` map, returning `None` when no arguments
    /// are attached.
    fn args_map(&self) -> Option<LocalizationArgs<'_>> {
        if self.args.is_empty() {
            return None;
        }
        let mut args = LocalizationArgs::default();
        for (name, arg) in &self.args {
            let value = match arg {
                MessageArg::Text(text) => text.clone().into(),
                MessageArg::Count(count) => (*count).into(),
            };
            args.insert(*name, value);
        }
        Some(args)
    }

    /// Render this message with a localizer already resolved by its caller.
    ///
    /// # Examples
    ///
    /// A generation operation can resolve its locale once, then render each
    /// label through that handle: `message.render_with(localizer)`.
    #[must_use]
    pub(crate) fn render_with(&self, localizer: &dyn Localizer) -> String {
        let args = self.args_map();
        localizer.message(self.key, args.as_ref(), self.key)
    }
}

impl fmt::Display for LocalizedMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let localizer = localizer();
        f.write_str(&self.render_with(localizer.as_ref()))
    }
}

/// Convenience helper to build a localized message.
#[must_use]
pub const fn message(key: &'static str) -> LocalizedMessage {
    LocalizedMessage::new(key)
}

#[cfg(test)]
mod tests {
    //! Guards on the key aggregates this module assembles.

    use super::{ALL_KEYS, check_keys, clap_error_keys, keys, manifest_keys};

    // A key moved out of `keys.rs` to keep that module inside the line cap
    // used to be listed by the aggregate automatically. Comparing against the
    // table slices rather than a written out list keeps the assertion true
    // when any table gains a key.
    #[test]
    fn all_keys_is_the_union_of_every_table() {
        let all = ALL_KEYS;
        assert_eq!(
            all.len(),
            keys::TABLE_KEYS.len()
                + check_keys::TABLE_KEYS.len()
                + clap_error_keys::TABLE_KEYS.len()
                + manifest_keys::TABLE_KEYS.len()
        );
        for expected in keys::TABLE_KEYS
            .iter()
            .chain(check_keys::TABLE_KEYS)
            .chain(clap_error_keys::TABLE_KEYS)
            .chain(manifest_keys::TABLE_KEYS)
        {
            assert!(
                all.contains(expected),
                "ALL_KEYS should contain `{expected}`"
            );
        }
        assert!(
            all.contains(&clap_error_keys::CLAP_ERROR_TOO_MANY_VALUES),
            "the clap error keys must stay in the public aggregate"
        );
    }
}
