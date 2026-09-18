# Architectural decision record (ADR) 023: Revalidate every fetch redirect

## Status

Accepted.

## Date

2026-09-02

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

### Budget, telemetry, and retention

One wall-clock budget covers the whole redirect chain, not each hop: every
request receives only the time still remaining in the chain, so a chain cannot
consume the budget once per hop. An exhausted budget ends the chain before the
next hop is dispatched, and the connect, read, and write timeouts remain in
force.

The adapter emits four bounded metric families and nothing else:

- `netsuke_stdlib_fetch_total`, labelled `outcome=success|failure`.
- `netsuke_stdlib_fetch_duration_seconds`, a histogram with no labels.
- `netsuke_stdlib_fetch_policy_total`, labelled `outcome=allowed|rejected`, with
  `policy_reason` one of `allowed`, `scheme_not_allowed`, `missing_host`,
  `host_not_allowlisted`, or `host_blocked`.
- `netsuke_stdlib_fetch_redirect_total`, labelled `outcome=followed|rejected`,
  with `redirect_failure` one of `none`, `limit_exceeded`, `loop`,
  `location_missing`, `location_invalid`, `credentials_not_removable`, or
  `policy_rejected`.

Every label value comes from a closed set declared in
[`src/stdlib/network/telemetry.rs`](../src/stdlib/network/telemetry.rs), so the
number of series is fixed by the code and never by input. No series carries a
URL, host, location, or userinfo, which keeps the counter cardinality bounded
under ADR-009's redaction contract. The library only emits these series.
Installing a recorder and deciding what to retain stays the application's
decision under ADR-013, so no stdlib fetch series is added to the in-process
recorder allowlist.

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

- Pure redirect decisions about an already-resolved target — the hop limit, loop
  detection, cross-origin credential removal, and the ordering of the policy
  check — in
  [`src/stdlib/network/redirect_chain.rs`](../src/stdlib/network/redirect_chain.rs),
  with unit and property tests in
  [`src/stdlib/network/redirect_chain_tests.rs`](../src/stdlib/network/redirect_chain_tests.rs)
- The fetch adapter that composes the transport, the chain budget, telemetry,
  and localized diagnostics. It reads the supported redirect statuses and
  resolves the `Location` header into a typed target before the chain sees it,
  in [`src/stdlib/network/redirect.rs`](../src/stdlib/network/redirect.rs),
  tested by
  [`src/stdlib/network/redirect_adapter_tests.rs`](../src/stdlib/network/redirect_adapter_tests.rs)
- The metric names and their closed label vocabularies in
  [`src/stdlib/network/telemetry.rs`](../src/stdlib/network/telemetry.rs),
  tested by
  [`src/stdlib/network/telemetry_tests.rs`](../src/stdlib/network/telemetry_tests.rs)
- Policy evaluation in
  [`src/stdlib/network/policy/mod.rs`](../src/stdlib/network/policy/mod.rs)
- The original-URL cache key in
  [`src/stdlib/network/cache.rs`](../src/stdlib/network/cache.rs)
- End-to-end coverage of every supported redirect status, the method used at
  each hop, and multi-hop refusal in
  [`tests/std_filter_tests/network_redirect_chain_tests.rs`](../tests/std_filter_tests/network_redirect_chain_tests.rs),
  with two-server and cache coverage in
  [`tests/std_filter_tests/network_redirect_tests.rs`](../tests/std_filter_tests/network_redirect_tests.rs)
  and
  [`src/stdlib/network/redirect_tests.rs`](../src/stdlib/network/redirect_tests.rs)

## Addendum — 2026-09-18: resolve the Location header at the transport boundary

The decision above is unchanged. This addendum settles a review follow-up from
[issue #705](https://github.com/leynos/netsuke/issues/705) about the *shape* of
the transport/domain boundary, not about the policy check the decision adds.
The adapter now resolves the `Location` header and owns the "absent" and
"present but unparsable" diagnostics, and
[`RedirectChain::advance`](../src/stdlib/network/redirect_chain.rs) receives an
already-resolved `Url` instead of a raw header string. The chain no longer
parses the header, so its `RedirectRejection` vocabulary is reduced to the four
redirect *decisions* it actually makes: `CredentialsNotRemovable`,
`LimitExceeded`, `Loop`, and `Policy`.

The rationale is the dependency rule. A `Location` header is an HTTP response
fact, and only the adapter can observe it; the chain is a transport-independent
state machine that performs no I/O. The two header-level variants therefore
forced the domain to name a transport failure in its own vocabulary and to
localize a fact it never reads. Moving the parse to the adapter leaves each
module with one concern: the adapter reads and validates the header and reports
its bounded diagnostics, and the chain decides only whether a resolved target
may be requested.

The invariants the original decision relies on do not change:

- **Check order.** The adapter still reads and resolves the header, then the
  chain applies, in order, `reject_excessive_redirects`,
  `redact_cross_origin_userinfo`, `reject_redirect_loop`, and
  `evaluate_target`. The policy check still precedes every redirected
  connection.
- **The four-field bound.** Redirect telemetry and trace events still emit
  exactly `operation`, `outcome`, `reason`, and `hop`. Resolving the header in
  the adapter did not reintroduce a location, URL, host, or userinfo into any
  emitted or logged field; the new diagnostics carry a redacted `current_url`
  only, exactly as the variants they replace did.
- **Metric cardinality.** `location_missing` and `location_invalid` remain in
  the closed `redirect_failure` vocabulary of
  [`src/stdlib/network/telemetry.rs`](../src/stdlib/network/telemetry.rs). The
  adapter still records both reasons, so the series count stays fixed by the
  code. The two Fluent keys and their messages are reused unchanged in every
  locale, so the user-visible text is unchanged too.
