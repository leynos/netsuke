//! Wall-clock configuration on [`StdlibConfig`].
//!
//! The `now()` helper reports the current instant, which makes any render that
//! calls it unrepeatable. The builder here swaps the host clock for a caller
//! supplied [`ClockProvider`] so tests and other callers can pin the instant.
//! Grouping it with the configuration surface keeps `config/mod.rs` to the
//! shared settings rather than one module per layer.

use super::StdlibConfig;
use crate::stdlib::time::{ClockProvider, WallClock};

impl StdlibConfig {
    /// Replace the wall-clock source backing `now()`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use minijinja::Environment;
    /// use netsuke::stdlib::{self, StdlibConfig, fixed_clock};
    /// use time::macros::datetime;
    ///
    /// let instant = datetime!(2026-06-08 12:00:00 UTC);
    /// let config = StdlibConfig::from_current_dir()
    ///     .expect("open workspace")
    ///     .with_clock(fixed_clock(instant));
    ///
    /// let mut env = Environment::new();
    /// stdlib::register_with_config(&mut env, config).expect("register stdlib");
    /// let rendered = env.render_str("{{ now() }}", ()).expect("render");
    /// assert_eq!(rendered, "2026-06-08T12:00:00Z");
    /// ```
    #[must_use]
    pub fn with_clock(mut self, provider: ClockProvider) -> Self {
        self.clock = WallClock::new(provider);
        self
    }

    /// Return the wall-clock source backing the `now()` helper.
    pub(crate) const fn clock(&self) -> &WallClock {
        &self.clock
    }
}
