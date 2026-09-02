//! Policy-aware HTTP redirect handling for the fetch adapter.

use std::{
    collections::BTreeSet,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use minijinja::{Error, ErrorKind};
use url::Url;

use super::{NetworkPolicy, network_policy_rejection_reason};
use crate::localization::{self, keys};

/// Maximum redirect hops accepted for one `fetch` request.
const FETCH_REDIRECT_LIMIT: usize = 5;

/// Maintain the state that belongs to one policy-checked redirect chain.
struct RedirectState<'policy> {
    /// Retain the policy that must approve every redirect hop.
    policy: &'policy NetworkPolicy,
    /// Store the URL to dispatch on the current iteration.
    current_url: Url,
    /// Remember resolved targets to terminate cycles deterministically.
    visited: BTreeSet<String>,
    /// Count accepted redirects independently from request attempts.
    hops: usize,
}

impl<'policy> RedirectState<'policy> {
    /// Initialise a redirect chain at the caller-supplied URL.
    fn new(url: &Url, policy: &'policy NetworkPolicy) -> Self {
        let current_url = url.clone();
        let visited = BTreeSet::from([current_url.as_str().to_owned()]);
        Self {
            policy,
            current_url,
            visited,
            hops: 0,
        }
    }

    /// Return the URL whose request is about to be dispatched.
    const fn current_url(&self) -> &Url {
        &self.current_url
    }

    /// Resolve, redact, track, and authorise the next redirect target.
    ///
    /// # Errors
    ///
    /// Returns an error when the location is missing, invalid, repeated, over
    /// the redirect limit, or rejected by the configured policy.
    fn advance(&mut self, response: &ureq::Response) -> Result<(), Error> {
        let location = response.header("Location").ok_or_else(|| {
            Error::new(
                ErrorKind::InvalidOperation,
                localization::message(keys::STDLIB_FETCH_REDIRECT_LOCATION_MISSING)
                    .with_arg("url", redacted_url(&self.current_url))
                    .to_string(),
            )
        })?;
        let mut next_url = self
            .current_url
            .join(location)
            .map_err(|_err| redirect_location_invalid_error(&self.current_url))?;
        reject_excessive_redirects(self.hops, &next_url)?;
        redact_cross_origin_userinfo(&self.current_url, &mut next_url)?;
        reject_redirect_loop(&mut self.visited, &next_url)?;
        evaluate_redirect_target(self.policy, &next_url, self.hops + 1)?;
        self.current_url = next_url;
        self.hops += 1;
        Ok(())
    }
}

/// Dispatch a policy-checked GET request with bounded redirects and timeouts.
///
/// # Errors
///
/// Returns an error when a redirect is malformed, loops, exceeds the limit, or
/// violates `policy`, or when `ureq` cannot connect to the server, send the
/// request, receive the response, or complete within one of the configured
/// timeouts, including unsuccessful HTTP responses.
pub(super) fn dispatch_request(
    url: &Url,
    policy: &NetworkPolicy,
    impure: &Arc<AtomicBool>,
) -> Result<ureq::Response, Error> {
    impure.store(true, Ordering::Relaxed);
    let agent = build_redirect_agent();
    let mut redirect = RedirectState::new(url, policy);

    loop {
        let response = dispatch_hop(&agent, redirect.current_url())?;

        if !is_redirect(&response) {
            return Ok(response);
        }

        redirect.advance(&response)?;
    }
}

/// Build a ureq agent that returns every redirect response to the caller.
fn build_redirect_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .redirects(0)
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(30))
        .timeout_write(Duration::from_secs(30))
        .timeout(Duration::from_secs(60))
        .build()
}

/// Send one GET request while keeping its URL out of failure diagnostics.
fn dispatch_hop(agent: &ureq::Agent, url: &Url) -> Result<ureq::Response, Error> {
    agent.get(url.as_str()).call().map_err(|_err| {
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

/// Determine whether a response requires manual redirect handling.
fn is_redirect(response: &ureq::Response) -> bool {
    (300..400).contains(&response.status())
}

/// Reject a redirect that exceeds the configured hop limit.
fn reject_excessive_redirects(hops: usize, next_url: &Url) -> Result<(), Error> {
    if hops < FETCH_REDIRECT_LIMIT {
        return Ok(());
    }
    Err(Error::new(
        ErrorKind::InvalidOperation,
        localization::message(keys::STDLIB_FETCH_REDIRECT_LIMIT_EXCEEDED)
            .with_arg("url", redacted_url(next_url))
            .with_arg("limit", FETCH_REDIRECT_LIMIT)
            .to_string(),
    ))
}

/// Remove credentials that must not cross an origin boundary.
fn redact_cross_origin_userinfo(current_url: &Url, next_url: &mut Url) -> Result<(), Error> {
    if current_url.origin() == next_url.origin() {
        return Ok(());
    }
    next_url
        .set_username("")
        .map_err(|()| redirect_location_invalid_error(current_url))?;
    next_url
        .set_password(None)
        .map_err(|()| redirect_location_invalid_error(current_url))
}

/// Reject a redirect target that was already requested in this chain.
fn reject_redirect_loop(visited: &mut BTreeSet<String>, next_url: &Url) -> Result<(), Error> {
    if visited.insert(next_url.as_str().to_owned()) {
        return Ok(());
    }
    Err(Error::new(
        ErrorKind::InvalidOperation,
        localization::message(keys::STDLIB_FETCH_REDIRECT_LOOP)
            .with_arg("url", redacted_url(next_url))
            .to_string(),
    ))
}

/// Evaluate one redirect target and emit its bounded policy decision.
///
/// # Errors
///
/// Returns an error when `policy` rejects `next_url` before a connection opens.
fn evaluate_redirect_target(
    policy: &NetworkPolicy,
    next_url: &Url,
    hop: usize,
) -> Result<(), Error> {
    match policy.evaluate(next_url) {
        Ok(()) => {
            tracing::debug!(
                operation = "fetch",
                policy_outcome = "allowed",
                hop,
                "network policy allowed fetch redirect"
            );
            Ok(())
        }
        Err(violation) => {
            tracing::warn!(
                operation = "fetch",
                policy_outcome = "rejected",
                policy_reason = network_policy_rejection_reason(&violation),
                hop,
                "network policy rejected fetch redirect"
            );
            Err(Error::new(
                ErrorKind::InvalidOperation,
                localization::message(keys::STDLIB_FETCH_REDIRECT_DISALLOWED)
                    .with_arg("url", redacted_url(next_url))
                    .with_arg("details", violation.to_string())
                    .to_string(),
            ))
        }
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
mod tests {
    //! Verify redirect-specific policy behaviour without opening connections.

    use super::*;

    #[test]
    fn redirect_policy_rejects_http_until_explicitly_allowed() {
        let target = Url::parse("http://allowed.example/resource")
            .expect("HTTP redirect target should parse");
        let rejection = evaluate_redirect_target(&NetworkPolicy::default(), &target, 1)
            .expect_err("default HTTPS-only policy should reject an HTTP redirect");
        assert!(rejection.to_string().contains("Redirect URL"));

        let policy = NetworkPolicy::default()
            .allow_scheme("http")
            .expect("HTTP scheme should be valid");
        evaluate_redirect_target(&policy, &target, 1)
            .expect("explicit HTTP policy should allow its redirect target");
    }
}
