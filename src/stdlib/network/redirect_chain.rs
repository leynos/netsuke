//! Transport-independent decisions for one policy-checked fetch redirect chain.
//!
//! The adapter in [`super::redirect`] owns the HTTP client, the telemetry, the
//! `Location` header parse, and the localized diagnostics. This module owns the
//! decisions those concerns wrap: hop accounting, loop detection, cross-origin
//! credential stripping, and the per-hop network-policy evaluation. Every target
//! it sees has already been resolved, and nothing here performs I/O or builds
//! user-facing text, so unit and property tests drive exactly the transitions
//! the adapter would.

use std::collections::BTreeSet;

use url::Url;

use super::{NetworkPolicy, NetworkPolicyViolation};

/// Maximum number of redirects accepted for one `fetch` request.
pub(super) const FETCH_REDIRECT_LIMIT: usize = 5;

/// One accepted redirect transition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct RedirectTransition {
    /// Resolved URL whose request is dispatched next.
    pub(super) next_url: Url,
    /// One-based hop number the chain moved to.
    pub(super) hop: usize,
}

/// Reason a redirect response did not advance the chain.
///
/// Every variant reports a *redirect decision*: what the chain concluded about
/// an already-resolved target. Reading and resolving the `Location` header is
/// transport work, so an absent or unparsable header is an adapter diagnostic
/// rather than a variant here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum RedirectRejection {
    /// Credentials could not be removed before a cross-origin hop.
    CredentialsNotRemovable {
        /// URL whose credentials could not be removed.
        current_url: Url,
    },
    /// The chain had already accepted every permitted redirect.
    LimitExceeded {
        /// Resolved target the limit refused.
        target: Url,
        /// Maximum number of redirects one chain may accept.
        limit: usize,
    },
    /// The resolved target had already been requested in this chain.
    Loop {
        /// Repeated target.
        target: Url,
    },
    /// The active policy refused the resolved target.
    Policy {
        /// Refused target.
        target: Url,
        /// Violation the policy reported. Boxed so the rejection stays small
        /// enough for the `result_large_err` lint.
        violation: Box<NetworkPolicyViolation>,
    },
}

/// State of one policy-checked redirect chain.
#[derive(Debug)]
pub(super) struct RedirectChain<'policy> {
    /// Policy every hop must satisfy.
    policy: &'policy NetworkPolicy,
    /// URL whose request the adapter dispatches next.
    current_url: Url,
    /// Fragment-free forms of the targets already requested in this chain.
    visited: BTreeSet<String>,
    /// Number of redirects accepted so far.
    hops: usize,
}

impl<'policy> RedirectChain<'policy> {
    /// Start a chain at the caller-supplied URL.
    #[must_use]
    pub(super) fn new(url: &Url, policy: &'policy NetworkPolicy) -> Self {
        let current_url = url.clone();
        let visited = BTreeSet::from([loop_detection_key(&current_url)]);
        Self {
            policy,
            current_url,
            visited,
            hops: 0,
        }
    }

    /// Return the URL whose request is dispatched next.
    #[must_use]
    pub(super) const fn current_url(&self) -> &Url {
        &self.current_url
    }

    /// Return the number of redirects accepted so far.
    #[must_use]
    pub(super) const fn hops(&self) -> usize {
        self.hops
    }

    /// Redact, track, and authorize an already-resolved redirect target.
    ///
    /// The caller resolves the `Location` header into `target` first, because
    /// reading a header is transport work. Everything below decides whether
    /// that target may be requested.
    ///
    /// # Errors
    ///
    /// Returns a [`RedirectRejection`] when the chain has already accepted
    /// [`FETCH_REDIRECT_LIMIT`] redirects, when credentials cannot be stripped
    /// for a cross-origin hop, when the target repeats an earlier one, or when
    /// the configured policy refuses the target. The checks run in exactly that
    /// order, so the policy check is the last word before the caller dispatches
    /// the request.
    pub(super) fn advance(
        &mut self,
        mut target: Url,
    ) -> Result<RedirectTransition, RedirectRejection> {
        self.reject_excessive_redirects(&target)?;
        redact_cross_origin_userinfo(&self.current_url, &mut target)?;
        self.reject_redirect_loop(&target)?;
        self.evaluate_target(&target)?;
        self.current_url = target.clone();
        self.hops = self.hops.saturating_add(1);
        Ok(RedirectTransition {
            next_url: target,
            hop: self.hops,
        })
    }

    /// Refuse a target that would exceed the redirect limit.
    fn reject_excessive_redirects(&self, target: &Url) -> Result<(), RedirectRejection> {
        if self.hops < FETCH_REDIRECT_LIMIT {
            return Ok(());
        }
        Err(RedirectRejection::LimitExceeded {
            target: target.clone(),
            limit: FETCH_REDIRECT_LIMIT,
        })
    }

    /// Refuse a target whose resource was already requested in this chain.
    fn reject_redirect_loop(&mut self, target: &Url) -> Result<(), RedirectRejection> {
        if self.visited.insert(loop_detection_key(target)) {
            return Ok(());
        }
        Err(RedirectRejection::Loop {
            target: target.clone(),
        })
    }

    /// Refuse a target the active policy does not permit.
    fn evaluate_target(&self, target: &Url) -> Result<(), RedirectRejection> {
        self.policy
            .evaluate(target)
            .map_err(|violation| RedirectRejection::Policy {
                target: target.clone(),
                violation: Box::new(violation),
            })
    }
}

/// Return `url` as a complete URL string with any fragment removed.
///
/// A fragment is never sent to the server, so two URLs differing only in their
/// fragment name the same request. Keying the visited set on the
/// fragment-bearing form would let a redirect that only swaps one fragment for
/// another repeat that request until the hop limit stopped it. Only this key is
/// normalized: the chain's `current_url` and the target it reports keep their
/// fragments, because redirect resolution and diagnostics must still show the
/// URL the server sent.
fn loop_detection_key(url: &Url) -> String {
    let mut without_fragment = url.clone();
    without_fragment.set_fragment(None);
    without_fragment.as_str().to_owned()
}

/// Remove credentials that must not cross an origin boundary.
///
/// # Errors
///
/// Returns [`RedirectRejection::CredentialsNotRemovable`] when a cross-origin
/// target refuses credential removal and must not be requested as-is.
fn redact_cross_origin_userinfo(
    current_url: &Url,
    next_url: &mut Url,
) -> Result<(), RedirectRejection> {
    if current_url.origin() == next_url.origin() {
        return Ok(());
    }
    let unremovable = || RedirectRejection::CredentialsNotRemovable {
        current_url: current_url.clone(),
    };
    next_url.set_username("").map_err(|()| unremovable())?;
    next_url.set_password(None).map_err(|()| unremovable())
}

#[cfg(test)]
#[path = "redirect_chain_tests.rs"]
mod tests;
