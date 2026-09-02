# Architecture decision record (ADR): Revalidate every fetch redirect

## Status

Accepted.

## Date

2026-09-02.

## Context and issue

`fetch()` evaluates the caller-supplied URL against `NetworkPolicy`, which
limits schemes and hosts before opening a connection. The HTTP client formerly
followed redirects itself, so an allowed origin could redirect a manifest fetch
to a link-local address, blocked host, or non-allowlisted host without another
policy decision. That gap turns a permitted request into a server-side request
forgery opportunity. Issue #647 requires the least-privilege policy to cover
each outbound hop rather than only the initial URL.

## Decision

Disable ureq's automatic redirects and follow redirects in the fetch adapter.
Before every redirected connection, resolve `Location` relative to the current
URL, remove URL credentials when the origin changes, and evaluate the resolved
target against `NetworkPolicy`. The adapter accepts at most five redirects and
rejects a repeated target.

The cache identity remains the original caller-supplied URL. A cache miss
validates every redirect hop before its response body is written under that
original key. A cache hit opens no outbound connection; the original URL is
still evaluated before the entry is read.

## Rationale

- **Policy is an outbound-hop invariant.** Checking the target before each
  request ensures a redirect cannot bypass scheme, allowlist, blocklist, or
  missing-host checks. An HTTPS-to-HTTP downgrade is therefore rejected unless
  `http` is explicitly allowed and its host passes the same policy.
- **Manual handling makes ordering auditable.** Disabling ureq redirects makes
  the policy check visibly precede every redirected network operation.
- **The loop is finite and deterministic.** Relative `Location` values resolve
  against the preceding URL, five accepted redirects is the upper bound, and a
  repeated resolved URL produces a loop error rather than another request.
- **Telemetry and diagnostics stay redacted.** Redirect decisions emit only
  operation, outcome, reason, and hop fields. They never emit a location, URL,
  host, or userinfo. Error URLs remove userinfo before localization, following
  ADR-009's bounded-redaction contract.

## Consequences

- Redirect responses without `Location`, invalid locations, policy rejections,
  loops, and over-limit chains now have distinct localized diagnostics.
- GET remains the request method at every accepted hop. No caller-derived
  headers are configured on redirected requests, and cross-origin URL
  credentials are stripped before the next request.
- Redirected responses have one cache entry per original fetch URL, not one per
  final destination. Cached and uncached cache-miss paths therefore apply the
  same hop policy before a body can be stored.

## Alternatives considered

- **Retain ureq automatic redirects.** Rejected because the default redirect
  handler has no Netsuke policy callback before each destination connection.
- **Check only a final response URL.** Rejected because the disallowed request
  has already occurred by the time a final URL is available.
- **Use redirect destinations as cache keys.** Rejected because callers request
  the original URL and an allowed endpoint can legitimately change its final
  location. Recording the original request as the identity preserves existing
  cache semantics without allowing an unchecked hop.

## Implementation references

- Redirect adapter and redacted diagnostics:
  [`src/stdlib/network/redirect.rs`](../src/stdlib/network/redirect.rs)
- Policy evaluation:
  [`src/stdlib/network/policy/mod.rs`](../src/stdlib/network/policy/mod.rs)
- Original-URL cache key:
  [`src/stdlib/network/cache.rs`](../src/stdlib/network/cache.rs)
- Two-server and cache coverage:
  [`tests/std_filter_tests/network_redirect_tests.rs`](../tests/std_filter_tests/network_redirect_tests.rs)
  and
  [`src/stdlib/network/redirect_tests.rs`](../src/stdlib/network/redirect_tests.rs)
