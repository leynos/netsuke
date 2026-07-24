//! Shared metadata for Netsuke's versioned JSON document envelopes.

use serde::Serialize;

pub(crate) const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Serialize, PartialEq, Eq)]
pub(crate) struct GeneratorInfo {
    pub(crate) name: &'static str,
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
