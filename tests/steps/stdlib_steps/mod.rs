//! Cucumber step implementations for stdlib path, file, network, and command helpers.

// NOTE: These module-level expectations cannot be scoped more narrowly while
// we rely on cucumber's step macros; they repeatedly shadow captured names.
// We will remove the allowance once the suite migrates to `rstest-bdd`.
#![expect(
    clippy::shadow_reuse,
    reason = "Cucumber step macros reuse parameter identifiers for captures"
)]

mod assertions;
mod parsing;
mod rendering;
mod types;
mod workspace;

#[expect(
    unused_imports,
    reason = "Re-export step functions for Cucumber discovery"
)]
pub(crate) use assertions::*;
#[expect(
    unused_imports,
    reason = "Re-export step functions for Cucumber discovery"
)]
pub(crate) use parsing::{
    extract_host_from_url, parse_expected_offset, parse_iso_timestamp, server_host,
};
#[expect(
    unused_imports,
    reason = "Re-export step functions for Cucumber discovery"
)]
pub(crate) use rendering::*;
#[expect(
    unused_imports,
    reason = "Re-export step functions for Cucumber discovery"
)]
pub(crate) use types::{FileContent, RelativePath, TemplateContent, TemplatePath};
#[expect(
    unused_imports,
    reason = "Re-export step functions for Cucumber discovery"
)]
pub(crate) use workspace::*;
