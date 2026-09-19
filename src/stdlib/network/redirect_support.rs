//! Localized diagnostics for failed and refused fetch hops.
//!
//! This private support module is owned exclusively by [`super::redirect`]. It
//! renders the message a failure becomes and the redaction every message
//! shares, which keeps the adapter — the HTTP client, the chain budget, the
//! telemetry, and the `Location` header parse — below the repository's
//! 400-line cap without widening its surface. Only `redirect` may import these
//! helpers.
//!
//! The split is by concern, not by size. Nothing here decides anything: every
//! input is a failure or refusal the adapter already produced, so the redirect
//! *decisions* stay in [`super::super::redirect_chain`] and the closed
//! telemetry category that names each one stays with the adapter. Keep it that
//! way — a helper that needs to inspect a header, a status, or a chain belongs
//! in the adapter.

use minijinja::{Error, ErrorKind};
use url::Url;

use super::super::redirect_chain::RedirectRejection;
use super::LocationFailure;
use crate::localization::{self, keys};

/// Build the localized diagnostic for a fetch that could not complete.
pub(super) fn fetch_failed_error(url: &Url, details: &str) -> Error {
    Error::new(
        ErrorKind::InvalidOperation,
        localization::message(keys::STDLIB_FETCH_FAILED)
            .with_arg("url", redacted_url(url))
            .with_arg("details", details)
            .to_string(),
    )
}

/// Build the localized diagnostic for an unusable `Location` header.
///
/// An absent header and an unresolvable value have different messages, so the
/// failure picks the message rather than the caller.
pub(super) fn location_failure_error(current_url: &Url, failure: LocationFailure) -> Error {
    match failure {
        LocationFailure::Missing => Error::new(
            ErrorKind::InvalidOperation,
            localization::message(keys::STDLIB_FETCH_REDIRECT_LOCATION_MISSING)
                .with_arg("url", redacted_url(current_url))
                .to_string(),
        ),
        LocationFailure::Unparsable => {
            invalid_location_error(current_url, "Location could not be resolved")
        }
    }
}

/// Build the localized diagnostic for a refused redirect.
///
/// Credential removal has no dedicated message; it reuses the invalid-location
/// diagnostic with a redacted location and a reason that names the failure.
pub(super) fn rejection_error(rejection: &RedirectRejection) -> Error {
    match rejection {
        RedirectRejection::CredentialsNotRemovable { current_url } => {
            invalid_location_error(current_url, "Credentials could not be removed")
        }
        RedirectRejection::LimitExceeded { target, limit } => Error::new(
            ErrorKind::InvalidOperation,
            localization::message(keys::STDLIB_FETCH_REDIRECT_LIMIT_EXCEEDED)
                .with_arg("url", redacted_url(target))
                .with_arg("limit", *limit)
                .to_string(),
        ),
        RedirectRejection::Loop { target } => Error::new(
            ErrorKind::InvalidOperation,
            localization::message(keys::STDLIB_FETCH_REDIRECT_LOOP)
                .with_arg("url", redacted_url(target))
                .to_string(),
        ),
        RedirectRejection::Policy { target, violation } => Error::new(
            ErrorKind::InvalidOperation,
            localization::message(keys::STDLIB_FETCH_REDIRECT_DISALLOWED)
                .with_arg("url", redacted_url(target))
                .with_arg("details", violation.to_string())
                .to_string(),
        ),
    }
}

/// Construct the localized "invalid redirect location" diagnostic.
///
/// One message covers both an unresolvable `Location` and a location whose
/// credentials could not be stripped; only `details` distinguishes them. The
/// location itself is never interpolated, because the value that failed may
/// carry credentials, so `<redacted>` stands in for it while the redacted
/// `current_url` names the response the header came from.
fn invalid_location_error(current_url: &Url, details: &str) -> Error {
    Error::new(
        ErrorKind::InvalidOperation,
        localization::message(keys::STDLIB_FETCH_REDIRECT_LOCATION_INVALID)
            .with_arg("url", redacted_url(current_url))
            .with_arg("location", "<redacted>")
            .with_arg("details", details)
            .to_string(),
    )
}

/// Render `url` without userinfo, query, or fragment for diagnostics.
///
/// A query string often carries a token or a signature and a fragment may carry
/// state, so neither belongs in user-facing text. Both are cleared inside the
/// same success branch as the userinfo, which keeps the cannot-be-a-base
/// fallback unchanged.
pub(super) fn redacted_url(url: &Url) -> String {
    let mut redacted = url.clone();
    if redacted.set_username("").is_ok() && redacted.set_password(None).is_ok() {
        redacted.set_query(None);
        redacted.set_fragment(None);
        redacted.to_string()
    } else {
        String::from("<redacted URL>")
    }
}
