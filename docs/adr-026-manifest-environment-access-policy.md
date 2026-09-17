# Architecture decision record (ADR): Keep project environment policy below the operator ceiling

## Status

Accepted.

## Date

2026-09-17

## Context and problem statement

The Jinja `env()` helper lets a manifest read process environment variables
while Netsuke renders it. That is useful — a manifest can select a profile or a
registry from its environment — but it is an unrestricted ingress path for
whatever the process holds, including credentials and CI tokens. A manifest
authored by whoever owns the checkout decides which names to request, and the
process holds values the operator owns.

The fetch boundary solved the same shape of problem in
[ADR-021](adr-021-trust-aware-fetch-policy-merge.md): netsuke combines
configuration from built-in defaults, system and user files, the primary project
`.netsuke.toml`, environment variables, and CLI options, and that ordinary
precedence is suitable for presentation and build preferences but is not a safe
merge rule for *grants*. A project checkout is less trusted than the operator
who launches Netsuke. Applying generic file precedence and vector append
semantics to an environment allowlist would let a project grant itself access
to any inherited variable, which is precisely the authority the boundary needs
to withhold.

Environment access needs the same trust-aware treatment as fetch policy, with
one deliberate difference. There is no operator opt-in here equivalent to
`trust_project_fetch_policy`: unlike a narrower network grant, widening
environment access has no operator-side legitimate use, so the decision is
unconditional.

## Decision

Add `manifest::EnvAccessPolicy`, an exact-name allow and block policy for the
manifest `env()` port. Adopt it as the sole manifest environment grant surface,
and treat only the primary project `.netsuke.toml` as a project policy request
before generic configuration merging.

The policy itself:

- An empty allowlist is permissive, preserving existing default-allow
  behaviour for every manifest that configures nothing.
- Once at least one effective `env_allow_var` entry exists, the allowlist is
  active and any name outside it is denied.
- `env_block_var` entries deny only their exact names while no allowlist is
  active, and always override a matching allow entry. A block is therefore
  monotonic: adding one can only restrict.
- Matching is exact. There is no glob or pattern support, so the policy can be
  reviewed by inspection and cannot drift with pattern semantics.
- Names are compared using the host environment's lookup semantics —
  case-sensitive on other platforms and case-insensitive on Windows — because
  the policy must not be looser than the lookup it guards. A case-insensitive
  lookup with case-sensitive policy matching would expose exactly the value an
  operator blocked.

Trust handling for the *grants*, following ADR-021's reconciliation shape:

- Discovery preserves primary-file provenance and removes that layer's
  `env_allow_var` field from the generic layer, so project allow entries cannot
  widen the operator's effective allowlist or activate default-deny.
- Project `env_block_var` entries stay in the ordinary merge and remain
  cumulative, so a lower-trust project can add a restriction but cannot remove
  one.
- Files loaded through `extends` remain ordinary file layers by design. Only
  the exact primary project path is quarantined.

Enforcement belongs at the registered `env()` call boundary, not at the reader.
The registered closure delegates to `env_var_with`, which evaluates the
requested name before invoking `manifest::EnvReader`, so a denied lookup cannot
obtain a process value at all. A denial returns one fixed, localized diagnostic
and emits only a bounded `failure_kind="blocked"` trace field; neither the
requested name nor its value appears in the diagnostic or the trace.

The violation is a domain-shaped error. `EnvPolicyViolation::Blocked` carries
no localization payload, so `EnvAccessPolicy` stays evaluable without the
localization framework; the manifest adapter renders the operator-facing text
at the boundary where environment diagnostics are already localized.

## Rationale

Evaluating the policy before the reader runs is what makes the boundary
meaningful. A check performed on the returned value would already have read the
secret into the process, and would leave the value reachable from a diagnostic,
a trace, or a panic. Checking the name first keeps the denied value out of
Netsuke entirely, which is why the deny-path tests assert reader non-invocation
rather than only a message.

Keeping diagnostics name-free and value-free avoids turning the refusal into a
disclosure of its own. An error that echoed the requested name would confirm
which credentials exist in the environment, which is the same class of leak as
returning the value.

Exact matching and the empty-allowlist default keep the change compatible.
Netsuke ships with no allowlist configured, so existing manifests behave
exactly as before, and a policy that has been configured can be read off the
file that configured it.

Mirroring ADR-021's quarantine rather than inventing a second trust model keeps
one reconciliation concept in the codebase, and keeps the reasoning that "a
lower-trust layer may restrict but not widen" uniform across policy surfaces.

## Consequences

- Manifest environment access is no longer governed by the manifest alone; the
  operator can bound it, and a primary project cannot widen its own bound.
- Operators gain an exact, inspectable allow and block surface through the
  ordinary configuration layers, `NETSUKE_` environment variables, and CLI
  flags.
- Every manifest that configures no list keeps default-allow behaviour, so the
  change is opt-in for tightening and adds no migration burden.
- Permitted values still reach the rendered manifest. The policy reduces secret
  ingress; it does not make rendered manifest content safe to log, and existing
  output and logging discipline still applies.
- A future pattern or prefix matching feature would weaken the inspectability
  property the exact-match rule provides, and would need its own decision.

## Alternatives considered

- **Filter the value after the reader returns.** Rejected because the process
  has already read the secret, and the value remains reachable from any
  diagnostic or trace that formats the read path.
- **Add an operator opt-in analogous to `trust_project_fetch_policy`.**
  Rejected because there is no operator use case for letting an untrusted
  checkout widen access to inherited environment variables. The fetch opt-in
  exists because project network grants are legitimately useful; environment
  grants of that kind are not.
- **Apply glob or prefix matching.** Rejected for this change because it makes
  the effective policy depend on pattern semantics a reviewer must simulate,
  and because exact names are sufficient for the credential-ingress cases the
  policy targets.
- **Let the manifest carry its own environment grants.** Rejected because the
  manifest is the untrusted requester; a grant that travels with the request is
  not a control.
- **Match case-sensitively on every platform.** Rejected because Windows
  resolves environment names case-insensitively, so a case-sensitive policy
  would leave a bypass on exactly the platform where the mismatch is invisible
  in testing.
- **Quarantine project block entries alongside allows.** Rejected because
  blocks only restrict. Removing them would discard a restriction the project
  can legitimately contribute.

## Implementation references

- Policy domain type:
  [`src/manifest/env_policy/mod.rs`](../src/manifest/env_policy/mod.rs)
- Enforcement at the `env()` registration boundary and the localized
  diagnostic: [`src/manifest/env_reader.rs`](../src/manifest/env_reader.rs)
- Project-request capture and quarantine:
  [`src/cli/discovery_project_policy.rs`](../src/cli/discovery_project_policy.rs)
- Configuration composition into the policy:
  [`src/cli_policy.rs`](../src/cli_policy.rs)
- Loader plumbing:
  [`src/manifest/path_loaders.rs`](../src/manifest/path_loaders.rs) and
  [`src/manifest/query.rs`](../src/manifest/query.rs)
- Deny-path and diagnostic coverage:
  [`tests/manifest_env_tests.rs`](../tests/manifest_env_tests.rs)
- User-facing policy guidance:
  [`users-guide.md`](users-guide.md#control-manifest-environment-access)

## Related decisions

- [ADR-004: Explicit config selection outside OrthoConfig][adr-004]
- [ADR-008: Environment seam taxonomy][adr-008]
- [ADR-021: Keep project fetch policy below the operator ceiling][adr-021]

[adr-004]: adr-004-explicit-config-selection-outside-orthoconfig.md
[adr-008]: adr-008-environment-seam-taxonomy.md
[adr-021]: adr-021-trust-aware-fetch-policy-merge.md
