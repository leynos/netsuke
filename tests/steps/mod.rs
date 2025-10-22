//! Step utilities shared by Cucumber step modules.
//!
//! Provides helpers for stable error text (e.g., `display_error_chain`) used by
//! step definitions.
#![allow(
    clippy::shadow_reuse,
    clippy::shadow_unrelated,
    reason = "Cucumber step macros rebind capture names"
)]

mod cli_steps;
#[cfg(unix)]
mod fs_steps;
mod ir_steps;
mod manifest_steps;
mod ninja_steps;
mod process_steps;
pub(crate) mod stdlib_steps;
