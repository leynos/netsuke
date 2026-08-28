//! Shared validation limits and error construction for the CLI module tree.
//!
//! These items are needed by both [`super::config`] (layered-configuration
//! validation) and [`super::parsing`] (Clap value validation), so they live in
//! their own leaf module rather than in [`super`]. Keeping them free of
//! dependencies lets the build script compile [`super::config`] without
//! dragging in the rest of the `cli` subtree.

use ortho_config::OrthoError;
use std::sync::Arc;

/// Maximum number of jobs accepted by the CLI.
pub(super) const MAX_JOBS: usize = 64;

/// Build a validation error for `key` with `message`.
pub(super) fn validation_error(key: &str, message: &str) -> Arc<OrthoError> {
    Arc::new(OrthoError::Validation {
        key: key.to_owned(),
        message: message.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    //! Unit tests for CLI validation limits and errors.

    use super::*;

    #[test]
    fn max_jobs_matches_the_cli_contract() {
        assert_eq!(MAX_JOBS, 64);
    }

    #[test]
    fn validation_error_preserves_its_key_and_message() {
        let error = validation_error("jobs", "message");
        let OrthoError::Validation { key, message } = error.as_ref() else {
            panic!("validation_error should construct OrthoError::Validation");
        };

        assert_eq!(key, "jobs");
        assert_eq!(message, "message");
    }
}
