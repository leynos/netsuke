//! Cucumber step implementations for stdlib path, file, network, and command helpers.

// NOTE: This module-level allowance cannot be scoped more narrowly while we
// rely on cucumber's step macros; they repeatedly shadow captured names.
// We will address the allowance when the suite migrates to `rstest-bdd`.
#![expect(
    clippy::shadow_reuse,
    reason = "Cucumber step macros reuse parameter identifiers for captures"
)]

mod assertions;
mod config;
mod parsing;
mod policy;
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
pub(crate) use config::*;
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
pub(crate) use policy::*;
#[expect(
    unused_imports,
    reason = "Re-export step functions for Cucumber discovery"
)]
pub(crate) use rendering::*;
#[expect(
    unused_imports,
    reason = "Re-export step functions for Cucumber discovery"
)]
pub(crate) use types::{
    ExpectedFragment, ExpectedOffset, ExpectedOutput, FileContent, RelativePath, ServerBody,
    TemplateContent, TemplatePath,
};
#[expect(
    unused_imports,
    reason = "Re-export step functions for Cucumber discovery"
)]
pub(crate) use workspace::*;
