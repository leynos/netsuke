//! Policy-aware HTTP redirect handling for the fetch adapter.
//!
//! The adapter owns the HTTP client, the chain budget, the bounded telemetry,
//! and the localized diagnostics. Every decision it makes comes from
//! [`super::redirect_chain`], so the redirect state machine is testable without
//! a socket and this module stays a thin composition of transport, metrics, and
//! user-facing text.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use minijinja::{Error, ErrorKind};
use url::Url;

use super::redirect_chain::{RedirectChain, RedirectRejection, is_supported_redirect_status};
use super::telemetry;
use super::{NetworkPolicy, network_policy_rejection_reason};
use crate::localization::{self, keys};

/// Wall-clock budget for one whole redirect chain, shared by every hop.
///
/// A chain that granted each hop a fresh timeout could run for as many times
/// this bound as the redirect limit allows, so each request instead receives
/// only the time still left in the chain.
const FETCH_CHAIN_BUDGET: Duration = Duration::from_secs(60);

/// Dispatch a policy-checked GET request with bounded redirects and timeouts.
///
/// # Errors
///
/// Returns an error when a redirect is malformed, loops, exceeds the limit, or
/// violates `policy`, when the chain exhausts [`FETCH_CHAIN_BUDGET`], or when
/// `ureq` cannot connect to the server, send the request, receive the response,
/// or complete within one of the configured timeouts, including unsuccessful
/// HTTP responses.
pub(super) fn dispatch_request(
    url: &Url,
    policy: &NetworkPolicy,
    impure: &Arc<AtomicBool>,
) -> Result<ureq::Response, Error> {
    impure.store(true, Ordering::Relaxed);
    let agent = build_redirect_agent();
    let deadline = Instant::now() + FETCH_CHAIN_BUDGET;
    let mut chain = RedirectChain::new(url, policy);

    loop {
        let remaining = remaining_budget(deadline, chain.current_url())?;
        let response = dispatch_hop(&agent, chain.current_url(), remaining)?;

        if !is_supported_redirect_status(response.status()) {
            return Ok(response);
        }

        match chain.advance(response.header("Location")) {
            Ok(transition) => record_followed_redirect(transition.hop),
            Err(rejection) => {
                let refused_hop = chain.hops().saturating_add(1);
                return Err(report_refused_redirect(&rejection, refused_hop));
            }
        }
    }
}

/// Build a ureq agent that returns every redirect response to the caller.
///
/// The connect timeout is deliberately independent of the chain budget: ureq
/// applies `timeout_connect` in place of the per-request timeout, so a single
/// hop may still spend its connect allowance. The read and write timeouts are
/// defaults that [`dispatch_hop`] supersedes with the remaining budget.
fn build_redirect_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .redirects(0)
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(30))
        .timeout_write(Duration::from_secs(30))
        .build()
}

/// Return the time left in the chain budget, refusing an exhausted chain.
///
/// # Errors
///
/// Returns an error once the shared chain deadline has been reached, so an
/// expired budget ends the chain promptly instead of granting the next hop a
/// fresh timeout.
fn remaining_budget(deadline: Instant, url: &Url) -> Result<Duration, Error> {
    match deadline.checked_duration_since(Instant::now()) {
        Some(remaining) if !remaining.is_zero() => Ok(remaining),
        _ => {
            // Log the host, not the full URL, which may carry userinfo.
            tracing::warn!(
                host = url.host_str().unwrap_or(""),
                "fetch redirect chain exhausted its budget"
            );
            Err(Error::new(
                ErrorKind::InvalidOperation,
                localization::message(keys::STDLIB_FETCH_FAILED)
                    .with_arg("url", redacted_url(url))
                    .with_arg("details", "Redirect chain exceeded its deadline")
                    .to_string(),
            ))
        }
    }
}

/// Send one GET request that must finish within the remaining chain budget.
fn dispatch_hop(
    agent: &ureq::Agent,
    url: &Url,
    remaining: Duration,
) -> Result<ureq::Response, Error> {
    agent
        .get(url.as_str())
        .timeout(remaining)
        .call()
        .map_err(|_err| {
            // Log the host, not the full URL, which may carry userinfo.
            tracing::warn!(host = url.host_str().unwrap_or(""), "fetch request failed");
            Error::new(
                ErrorKind::InvalidOperation,
                localization::message(keys::STDLIB_FETCH_FAILED)
                    .with_arg("url", redacted_url(url))
                    .with_arg("details", "HTTP request failed")
                    .to_string(),
            )
        })
}

/// Record one accepted redirect and log its bounded policy decision.
fn record_followed_redirect(hop: usize) {
    telemetry::record_redirect_followed();
    telemetry::record_policy_decision("allowed", "allowed");
    tracing::debug!(
        operation = "fetch",
        policy_outcome = "allowed",
        hop,
        "network policy allowed fetch redirect"
    );
}

/// Record one refused redirect and build its localized diagnostic.
fn report_refused_redirect(rejection: &RedirectRejection, hop: usize) -> Error {
    telemetry::record_redirect_refused(failure_category(rejection));
    if let RedirectRejection::Policy { violation, .. } = rejection {
        let reason = network_policy_rejection_reason(violation);
        telemetry::record_policy_decision("rejected", reason);
        tracing::warn!(
            operation = "fetch",
            policy_outcome = "rejected",
            policy_reason = reason,
            hop,
            "network policy rejected fetch redirect"
        );
    }
    rejection_error(rejection)
}

/// Return the closed telemetry category for a refused redirect.
const fn failure_category(rejection: &RedirectRejection) -> &'static str {
    match rejection {
        RedirectRejection::LocationMissing { .. } => "location_missing",
        RedirectRejection::LocationInvalid { .. } => "location_invalid",
        RedirectRejection::CredentialsNotRemovable { .. } => "credentials_not_removable",
        RedirectRejection::LimitExceeded { .. } => "limit_exceeded",
        RedirectRejection::Loop { .. } => "loop",
        RedirectRejection::Policy { .. } => "policy_rejected",
    }
}

/// Build the localized diagnostic for a refused redirect.
///
/// Credential removal has no dedicated message; it reuses the invalid-location
/// diagnostic with a redacted location and a reason that names the failure.
fn rejection_error(rejection: &RedirectRejection) -> Error {
    match rejection {
        RedirectRejection::LocationMissing { current_url } => Error::new(
            ErrorKind::InvalidOperation,
            localization::message(keys::STDLIB_FETCH_REDIRECT_LOCATION_MISSING)
                .with_arg("url", redacted_url(current_url))
                .to_string(),
        ),
        RedirectRejection::LocationInvalid { current_url } => {
            redirect_location_invalid_error(current_url)
        }
        RedirectRejection::CredentialsNotRemovable { current_url } => Error::new(
            ErrorKind::InvalidOperation,
            localization::message(keys::STDLIB_FETCH_REDIRECT_LOCATION_INVALID)
                .with_arg("url", redacted_url(current_url))
                .with_arg("location", "<redacted>")
                .with_arg("details", "Credentials could not be removed")
                .to_string(),
        ),
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

/// Construct a redacted invalid-redirect-location error.
fn redirect_location_invalid_error(current_url: &Url) -> Error {
    Error::new(
        ErrorKind::InvalidOperation,
        localization::message(keys::STDLIB_FETCH_REDIRECT_LOCATION_INVALID)
            .with_arg("url", redacted_url(current_url))
            .with_arg("location", "<redacted>")
            .with_arg("details", "Location could not be resolved")
            .to_string(),
    )
}

/// Render `url` without userinfo for diagnostics.
fn redacted_url(url: &Url) -> String {
    let mut redacted = url.clone();
    if redacted.set_username("").is_ok() && redacted.set_password(None).is_ok() {
        redacted.to_string()
    } else {
        String::from("<redacted URL>")
    }
}

#[cfg(test)]
#[path = "redirect_adapter_tests.rs"]
mod tests;
