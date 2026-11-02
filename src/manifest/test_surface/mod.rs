//! Capabilities granted to a manifest loaded under `netsuke test`.
//!
//! The surface is deny-by-default. It carries a [`StdlibConfig`] so the caller
//! can pin the recipe dialect and, by injecting a clock, make `now()`
//! deterministic; an [`EnvReader`] so `env()` answers from an explicit seam
//! rather than the process environment; and the [`TemplateOverlays`] a test
//! supplies in place of the helpers the surface withholds. Without an injected
//! reader every lookup reports "not set", which is what lets a test manifest
//! observe that the boundary was withheld without mutating the process.
//!
//! Keeping the payload here rather than in the loader options means the
//! loader's registration enum never has to grow a second capability field. The
//! overlay machinery lives in [`overlays`], which also owns the namespace rules
//! the loader consults before it registers anything.

mod overlays;

pub use overlays::{
    OverlayCall, OverlayCallable, OverlayNameKind, TemplateOverlayError, TemplateOverlays,
};
pub(crate) use overlays::{OverlayNamespace, overlay_namespace, register_overlays};

use super::EnvReader;
use super::env::disabled_env_reader;
use crate::stdlib::StdlibConfig;

/// The capabilities granted to a manifest under `netsuke test`.
pub struct TestSurface {
    /// Stdlib configuration, including the clock `now()` may read.
    config: StdlibConfig,
    /// Reader behind `env()`; refuses every lookup by default.
    reader: EnvReader,
    /// Template functions registered in place of withheld helpers.
    overlays: TemplateOverlays,
}

impl TestSurface {
    /// Start from a stdlib configuration.
    ///
    /// `now()` is real only when the configuration carries an injected clock;
    /// with the ambient clock the helper refuses, because the host clock is a
    /// read a test must not depend on.
    #[must_use]
    pub fn new(config: StdlibConfig) -> Self {
        Self {
            config,
            reader: disabled_env_reader(),
            overlays: TemplateOverlays::default(),
        }
    }

    /// Answer `env()` from this reader instead of refusing every lookup.
    #[must_use]
    pub fn with_env_reader(mut self, reader: EnvReader) -> Self {
        self.reader = reader;
        self
    }

    /// Register these template functions for the load.
    ///
    /// An overlay replaces a same-named helper and is visible to `foreach`,
    /// `when`, and every rendered field, so a test can stand in for a helper
    /// the surface refuses without granting that helper itself.
    #[must_use]
    pub fn with_overlays(mut self, overlays: TemplateOverlays) -> Self {
        self.overlays = overlays;
        self
    }

    /// Return the reader behind `env()` for this surface.
    pub(crate) const fn reader(&self) -> &EnvReader {
        &self.reader
    }

    /// Return the stdlib configuration this surface grants.
    pub(crate) const fn config(&self) -> &StdlibConfig {
        &self.config
    }

    /// Return the template overlays this surface grants.
    pub(crate) const fn overlays(&self) -> &TemplateOverlays {
        &self.overlays
    }
}
