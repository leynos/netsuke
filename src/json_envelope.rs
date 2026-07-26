//! Shared metadata for Netsuke's versioned JSON document envelopes.

use serde::Serialize;

/// Schema version stamped into every JSON envelope so consumers can detect
/// incompatible document shapes across Netsuke releases.
pub(crate) const SCHEMA_VERSION: u32 = 1;

/// Identifies the tool that produced a JSON envelope, embedded in each
/// document so downstream consumers can attribute and version the output.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub(crate) struct GeneratorInfo {
    /// Producing tool's name (always `"netsuke"`).
    pub(crate) name: &'static str,
    /// Producing tool's semantic version, taken from `CARGO_PKG_VERSION`.
    pub(crate) version: &'static str,
}

impl GeneratorInfo {
    pub(crate) const fn current() -> Self {
        Self {
            name: "netsuke",
            version: env!("CARGO_PKG_VERSION"),
        }
    }
}
