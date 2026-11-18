//! Re-exports of otherwise-private `process` items for doctests only.
//!
//! Doctests compile as a separate crate and cannot reach `pub(crate)` or
//! private items in `process`, so this module surfaces the redaction helpers
//! and a handful of Ninja-invocation functions under `cfg(doctest)` to give doc
//! examples something to call without widening the crate's release-build API.

pub use super::redaction::{CommandArg, is_sensitive_arg, redact_argument, redact_sensitive_args};
pub use super::{
    create_temp_ninja_file, resolve_ninja_program, write_ninja_file, write_text_file_utf8,
};
