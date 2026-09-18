//! Policy-aware HTTP redirect handling for the fetch adapter.
//!
//! The adapter owns the HTTP client, the chain budget, the bounded telemetry,
//! and the localized diagnostics. It also owns the `Location` header parse:
//! reading a response header is transport work, so an absent or unparsable
//! header is diagnosed here rather than inside the chain. Every *decision*
//! about the resolved target comes from [`super::redirect_chain`], so the
//! redirect state machine is testable without a socket and this module stays a
//! thin composition of transport, metrics, and user-facing text.

use std::{
    error::Error as _,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use minijinja::Error;
use url::Url;

use super::redirect_chain::{RedirectChain, RedirectRejection};
use super::telemetry;
use super::{NetworkPolicy, network_policy_rejection_reason};

/// Keep the localized diagnostics and their shared redaction below the cap.
#[path = "redirect_support.rs"]
mod support;

#[cfg(test)]
use support::redacted_url;
use support::{fetch_failed_error, location_failure_error, rejection_error};

/// Wall-clock budget for one whole redirect chain, shared by every hop.
///
/// A chain that granted each hop a fresh timeout could run for as many times
/// this bound as the redirect limit allows, so each request instead receives
/// only the time still left in the chain.
const FETCH_CHAIN_BUDGET: Duration = Duration::from_secs(60);

/// Why a redirect response named no usable target.
///
/// Both cases describe the HTTP response rather than a redirect decision, so
/// this type stays local to the adapter and never reaches the chain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LocationFailure {
    /// The redirect response carried no `Location` header.
    Missing,
    /// The `Location` value could not be resolved against the current URL.
    Unparsable,
}

/// Dispatch a policy-checked GET request with bounded redirects and timeouts.
///
/// # Errors
///
/// Returns an error when a redirect carries no usable `Location`, loops,
/// exceeds the limit, or violates `policy`, when the chain exhausts
/// [`FETCH_CHAIN_BUDGET`], or when `ureq` cannot connect to the server, send the
/// request, receive the response, or complete within one of the configured
/// timeouts, including unsuccessful HTTP responses.
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

        let hop = follow_redirect(&mut chain, &response)?;
        record_followed_redirect(hop);
    }
}

/// Resolve one redirect response and judge it, recording any refusal.
///
/// The two ways a redirect can fail — a header the adapter cannot resolve and a
/// target the chain refuses — both end here, so the dispatch loop has a single
/// failure path and every refusal is reported in the same shape. An absent
/// header and an unresolvable value are distinct failures only in the reason
/// and message they carry.
///
/// # Errors
///
/// Returns the localized diagnostic for an unusable `Location` header, or for
/// the refusal the chain reported, after recording each in telemetry and the
/// log.
fn follow_redirect(
    chain: &mut RedirectChain<'_>,
    response: &ureq::Response,
) -> Result<usize, Error> {
    let target = resolve_location(chain, response.header("Location")).map_err(|failure| {
        report_location_failure(chain, failure, chain.hops().saturating_add(1))
    })?;
    chain
        .advance(target)
        .map(|transition| transition.hop)
        .map_err(|rejection| report_refused_redirect(&rejection, chain.hops().saturating_add(1)))
}

/// Resolve the raw `Location` header into the target the chain will judge.
///
/// Relative values resolve against the URL whose request produced the response,
/// which is the chain's current URL. An absent header and an unresolvable value
/// are distinct failures, because each has its own bounded reason and its own
/// localized message.
///
/// # Errors
///
/// Returns [`LocationFailure::Missing`] when `location` is `None`, and
/// [`LocationFailure::Unparsable`] when the value cannot be joined to the
/// current URL.
fn resolve_location(
    chain: &RedirectChain<'_>,
    location: Option<&str>,
) -> Result<Url, LocationFailure> {
    let Some(raw_location) = location else {
        return Err(LocationFailure::Missing);
    };
    chain
        .current_url()
        .join(raw_location)
        .map_err(|_err| LocationFailure::Unparsable)
}

impl LocationFailure {
    /// Return the closed telemetry category for this failure.
    const fn reason(self) -> &'static str {
        match self {
            Self::Missing => "location_missing",
            Self::Unparsable => "location_invalid",
        }
    }
}

/// Report whether `status` carries redirect semantics that preserve GET.
///
/// Reading a status code is HTTP interpretation, so the predicate lives here
/// with the rest of the transport concerns rather than in the pure chain.
#[must_use]
const fn is_supported_redirect_status(status: u16) -> bool {
    matches!(status, 301 | 302 | 303 | 307 | 308)
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
            Err(fetch_failed_error(
                url,
                "Redirect chain exceeded its deadline",
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
        .map_err(|err| {
            // Log the host, not the full URL, which may carry userinfo. The
            // category is a closed value, so the log stays bounded while still
            // separating a timeout from a refused connection or a bad status.
            tracing::warn!(
                host = url.host_str().unwrap_or(""),
                error_category = ureq_failure_category(&err),
                "fetch request failed"
            );
            fetch_failed_error(url, "HTTP request failed")
        })
}

/// Classify a `ureq` failure into the closed `error_category` vocabulary.
///
/// Every failure of a hop is otherwise indistinguishable in the log, so the
/// category separates an unsuccessful HTTP response from a connection, a
/// timeout, a malformed response, and an unusable URL.
fn ureq_failure_category(err: &ureq::Error) -> &'static str {
    match err {
        ureq::Error::Status(..) => "http_status",
        ureq::Error::Transport(transport) => {
            transport_failure_category(transport.kind(), is_timed_out(err))
        }
    }
}

/// Return the closed category for one transport failure.
///
/// The transport carries only its own error kind, which folds connect, DNS, and
/// proxy failures together and reports a timeout as a plain I/O error, so the
/// timed-out flag distinguishes the two I/O cases.
const fn transport_failure_category(kind: ureq::ErrorKind, timed_out: bool) -> &'static str {
    match kind {
        ureq::ErrorKind::Dns
        | ureq::ErrorKind::ConnectionFailed
        | ureq::ErrorKind::ProxyConnect
        | ureq::ErrorKind::ProxyUnauthorized => "connection",
        ureq::ErrorKind::Io if timed_out => "timeout",
        ureq::ErrorKind::Io => "io",
        ureq::ErrorKind::BadStatus | ureq::ErrorKind::BadHeader => "protocol",
        ureq::ErrorKind::InvalidUrl
        | ureq::ErrorKind::UnknownScheme
        | ureq::ErrorKind::InvalidProxyUrl => "invalid_url",
        ureq::ErrorKind::InsecureRequestHttpsOnly
        | ureq::ErrorKind::TooManyRedirects
        | ureq::ErrorKind::HTTP => "other",
    }
}

/// Report whether a transport failure's source is a timed-out I/O error.
fn is_timed_out(err: &ureq::Error) -> bool {
    err.source()
        .and_then(|source| source.downcast_ref::<std::io::Error>())
        .is_some_and(|io_err| io_err.kind() == std::io::ErrorKind::TimedOut)
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
///
/// Every refusal is logged, not only a policy rejection: the counter alone
/// cannot show which hop of which fetch was refused. Each event carries exactly
/// the four bounded fields ADR-023 permits a redirect decision to emit, using
/// the closed vocabulary of the decision it reports.
fn report_refused_redirect(rejection: &RedirectRejection, hop: usize) -> Error {
    let refusal = failure_category(rejection);
    telemetry::record_redirect_refused(refusal);
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
    } else {
        warn_refused_redirect(hop, refusal);
    }
    rejection_error(rejection)
}

/// Record one unusable `Location` header and build its localized diagnostic.
///
/// This is the adapter's counterpart to [`report_refused_redirect`] for the two
/// failures the chain can no longer report. It emits the same four bounded
/// fields with the same closed vocabulary, so a header failure and a redirect
/// refusal are indistinguishable in shape and differ only by `redirect_failure`.
fn report_location_failure(
    chain: &RedirectChain<'_>,
    failure: LocationFailure,
    hop: usize,
) -> Error {
    let reason = failure.reason();
    telemetry::record_redirect_refused(reason);
    warn_refused_redirect(hop, reason);
    location_failure_error(chain.current_url(), failure)
}

/// Log one refused redirect with its four bounded fields.
///
/// Both refusal paths — an unusable header and a chain decision — share this
/// event, so every refusal logs the same shape and they differ only by the
/// closed `redirect_failure` reason they carry.
fn warn_refused_redirect(hop: usize, reason: &'static str) {
    tracing::warn!(
        operation = "fetch",
        redirect_outcome = "rejected",
        redirect_failure = reason,
        hop,
        "fetch redirect refused"
    );
}

/// Return the closed telemetry category for a refused redirect.
const fn failure_category(rejection: &RedirectRejection) -> &'static str {
    match rejection {
        RedirectRejection::CredentialsNotRemovable { .. } => "credentials_not_removable",
        RedirectRejection::LimitExceeded { .. } => "limit_exceeded",
        RedirectRejection::Loop { .. } => "loop",
        RedirectRejection::Policy { .. } => "policy_rejected",
    }
}

#[cfg(test)]
#[path = "redirect_adapter_tests.rs"]
mod tests;
