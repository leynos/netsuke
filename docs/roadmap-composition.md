# Netsuke composition roadmap

This document continues the active [Netsuke roadmap](roadmap.md) with phases
16 to 19. Task identifiers remain globally unique across both documents and
the completed-foundations archive. Each phase states a hypothesis, each step
answers one delivery question, and each task names dependencies, source
contracts, and observable acceptance criteria. The roadmap promises no dates.

RFC 0002 owns plain source decomposition; RFC 0003 owns local bundle
instantiation; RFC 0004 owns later external acquisition. RFCs 0009 and 0010
already have execution tasks in phases 12 to 14, and phase 11 owns RFC 0011.
Task 17.4.4 adds their composition-specific integration rather than duplicating
those implementations. Proposed RFCs remain proposed until their explicit
acceptance tasks complete.

## 16. Repository-relative manifest composition

Hypothesis: an author can split a monolithic Netsukefile into local fragments
without changing its graph, executing recipes during discovery, or making
composition depend on filesystem iteration order.

This phase implements [RFC 0002](rfcs/0002-repository-relative-includes.md).
It shares provenance concepts with RFC 0001, not an action-runner dependency;
legacy recipes can validate this entire slice. Versioned instantiation belongs
in phase 17, and networking belongs only in phase 18. Phase numbers identify
work, not a requirement to finish every preceding phase.

### 16.1. Load each local composition unit once through a capability

This step asks whether literal source decomposition can provide deterministic
identity and useful failure chains before rendering begins. Its result fixes
the traversal and provenance contract used by later bundle instances.

- [ ] 16.1.1. Ratify fragment scope and shared provenance contracts.
  - [ ] Classify root-only settings and ordered sections; decide fragment
    markers, namespace coverage for pools and providers, and the typed
    namespace-variable projection. Specify source spans and include chains
    without requiring the RFC 0001 runner to exist.
  - [ ] Record the additive schema gate, bounded composition limits, and
    reuse boundaries in the design and any required ADR. Coordinate version
    allocation with 12.1.1 without coupling their implementation schedules.
  - See [RFC 0002 §§5-10 and 16](rfcs/0002-repository-relative-includes.md).
  - Success: one documented schema and provenance contract resolves every
    initial-scope decision without enabling dynamic paths or silent overrides.
- [ ] 16.1.2. Parse literal includes and open confined source units.
  - Requires 16.1.1.
  - [ ] Accept scalar and closed mapping include forms; reject unknown keys,
    empty paths, Jinja, globs, NUL, and invalid namespaces. Resolve each path
    from its including file through the effective workspace capability.
  - [ ] Retain original bytes, spans, and canonical identities; reject
    absolute, lexical and symlink escapes, non-files, unreadable inputs, and
    unsupported non-UTF-8 paths or content with distinct bounded diagnostics.
  - See [RFC 0002 §§5, 6, and 9](rfcs/0002-repository-relative-includes.md).
  - Success: nested-directory and Windows path fixtures resolve the intended
    files, while generated escape cases cannot open content outside the root.
- [ ] 16.1.3. Compose includes in deterministic depth-first post-order.
  - Requires 16.1.2.
  - [ ] Initialize one traversal-wide visited map and active stack, including
    the root identity; visit declared siblings in order and append the
    including unit last. Keep visited state after leaving the active stack.
  - [ ] Distinguish a cycle from a repeated load, including diamonds and
    alternate path or symlink spellings; report the closing site and both
    first and attempted include chains without losing source spans.
  - See [RFC 0002 §§6 and 14](rfcs/0002-repository-relative-includes.md).
  - Success: the `C, A, B, R` example and bounded graph properties preserve
    order and unique emission; tractable Kani traversal checks cover the
    production state machine rather than a disconnected model.

### 16.2. Preserve declaration meaning through composition and rendering

This step asks whether composition can retain the existing compiler semantics
without becoming a generic YAML merge language. Its result determines whether
extracting a fragment preserves both references and rebuild behaviour.

- [ ] 16.2.1. Merge typed sections with namespace-aware reference resolution.
  - Requires 16.1.3.
  - [ ] Qualify declarations and their internal references, project variables
    into the agreed typed namespace, and concatenate only schema-designated
    ordered sections. Validate references against the complete composition.
  - [ ] Reject duplicate qualified declarations and variable keys, including
    nested mapping replacement, with both source sites and remediation.
    Reject fragment-owned root settings rather than ignoring them.
  - See [RFC 0002 §§5.1 and 7](rfcs/0002-repository-relative-includes.md).
  - Success: namespace and collision fixtures demonstrate no implicit deep
    merge, last-writer-wins replacement, or unresolved reference rewriting.
- [ ] 16.2.2. Feed composed units through the existing manifest compiler.
  - Requires 16.2.1.
  - [ ] Build the composed variable context before ordinary Jinja, `foreach`,
    and `when` evaluation; retain original fragment provenance through AST,
    IR, and Ninja generation instead of reparsing synthetic merged YAML.
  - [ ] Prevent templates from synthesizing new structural includes, preserve
    existing control-key scopes, and keep no-include manifests unchanged.
  - See [RFC 0002 §§8, 10, and 12](rfcs/0002-repository-relative-includes.md).
  - Success: differential compiler fixtures preserve the legacy graph and
    report expansion and recipe errors at their original fragment spans.
- [ ] 16.2.3. Bind compilation fingerprints to the entire source composition.
  - Requires 16.2.2.
  - [ ] Hash canonical relative identities, exact bytes in composition order,
    namespace selections, and compiler and schema versions; include every
    source unit in generated-state invalidation without inspecting timestamps.
  - [ ] Preserve fragment provenance in generated-state diagnostics and keep
    absolute host paths out of public identities.
  - See [RFC 0002 §10](rfcs/0002-repository-relative-includes.md).
  - Success: any included-byte or namespace change invalidates the fingerprint,
    while metadata-only touches and hash-map allocation history do not.

### 16.3. Make local extraction usable in builds and metadata queries

This step asks whether fragment extraction remains safe through public commands
rather than only resolver tests. Its migration evidence gates the local bundle
foundation, not the unrelated structured-command release.

- [ ] 16.3.1. Expose composed declarations through restricted metadata queries.
  - Requires 16.2.2 and 16.2.3.
  - [ ] Integrate `help targets`, graph inspection, and JSON provenance with
    the composed loader and existing restricted Jinja environment. Parse
    includes but never render or execute recipe fields for discovery.
  - [ ] Report malformed, missing, cyclic, and duplicate source graphs with
    bounded relative origins and namespaces; do not add network capability.
  - See [RFC 0002 §§8 and 11](rfcs/0002-repository-relative-includes.md).
  - Success: instrumented queries expose included targets while command,
    recipe-only helper, and network probes record zero invocations.
- [ ] 16.3.2. Add the cross-platform include and extraction interaction suite.
  - Requires 16.3.1.
  - [ ] Cross nested and sibling includes, namespaces, controls, collisions,
    aliases, cycles, and `-C` with build, generate, help, graph, and JSON
    paths. Use legacy recipes so this slice has no structured-runner blocker.
  - [ ] Extract a representative monolithic canary into local fragments;
    compare logical graphs, ordered recipe semantics, and target discovery,
    accounting only for intentional provenance and fingerprint changes.
  - See [RFC 0002 §§12-14](rfcs/0002-repository-relative-includes.md).
  - Success: Unix-like and Windows runs preserve outputs and failures, and
    recipe-observation sentinels remain untouched during metadata discovery.
- [ ] 16.3.3. Document and release the local include contract.
  - Requires 16.3.2.
  - [ ] Publish executed extraction examples, fragment scope, namespace and
    collision rules, path confinement, and diagnostic guidance; update design,
    user, developer, contents, and repository-layout documentation as needed.
  - [ ] Record schema compatibility and the include portion of issue `#598`
    with the canonical quality-gate evidence, without claiming bundle or
    structured-command completion.
  - See [RFC 0002 §§12-14 and 17](rfcs/0002-repository-relative-includes.md).
  - Success: documented examples reproduce the canary, old manifests retain
    their behaviour, and older parsers reject the new syntax clearly.

## 17. Reusable, versioned local manifest bundles

Hypothesis: explicit parameters, private exports, deterministic version
selection, and reviewed content locks let repositories share quality-gate
bundles without copying monolithic manifests or depending on ambient Git state.

This phase implements [RFC 0003](rfcs/0003-versioned-local-bundles.md) over
phase 16. Local composition must remain independently usable and offline;
17.4.4 separately validates its interaction with the structured execution track.
No local bundle operation gains external acquisition authority from phase 18.

### 17.1. Fix bundle identity and parameter meaning before instantiation

This step asks whether the descriptor, version language, and import interface
are precise enough to give every instance one meaning. Its outcome fixes the
contracts that catalogue selection, hashing, and locking must share.

- [ ] 17.1.1. Ratify bundle versions, path anchors, locks, and CLI ownership.
  - Requires 16.3.3.
  - [ ] Reconcile RFC 0003 §5.3's exact-prerelease prose with its comparator
    example admitting `1.4.0-rc.2` after `>=1.4.0-rc.1`; freeze all normative
    vectors rather than inheriting a library's different bare-version rules.
    Resolve bundle-relative versus including-file-relative nested paths.
  - [ ] Decide canonical tree and parameter encodings, lock filename/schema,
    configured catalogue roots, bounds, unlocked-development policy, and
    explicit local update authority. Resolve `bundle inspect` against the
    canonical `get` vocabulary; reserve RFC 0004's `bundle fetch` and
    `--update-lock`, then update the public vocabulary before CLI work.
  - See [RFC 0003 §§5, 8, 11, 12, 15, and
    20](rfcs/0003-versioned-local-bundles.md).
  - Success: accepted decisions and matching examples define source anchors,
    version vectors, lock mutation, and byte encodings without ambient choices.
- [ ] 17.1.2. Parse strict bundle descriptors and local imports.
  - Requires 17.1.1.
  - [ ] Validate bundle identity/version, entry manifest, Netsuke and manifest
    compatibility, required `runtime_resources` even when empty, exports,
    parameter declarations, and exactly one local source with a namespace.
  - [ ] Reject unknown keys, dynamic descriptor fields, invalid direct exact
    assertions, duplicate resource paths, missing executable Booleans, and
    malformed metadata before evaluating entry-manifest expressions.
  - See [RFC 0003 §§4, 5, 9, and 16](rfcs/0003-versioned-local-bundles.md).
  - Success: schema and version-gate fixtures cover every descriptor/import
    field and reject incompatible bundles before entry-manifest evaluation.
- [ ] 17.1.3. Normalize typed parameters in an isolated pure context.
  - Requires 17.1.2.
  - [ ] Implement all seven initial parameter types, defaults, required-value
    and unknown-key checks, immutable `bundle.params`, and deterministic
    parameter digests. Exclude unrelated importer variables from bundle scope.
  - [ ] Permit only pure importer expressions; deny subprocess, network,
    clock, environment enumeration, and unrestricted filesystem access.
    Redact values unless explicitly annotated `expose: non-secret`.
  - See [RFC 0003 §§6, 13, and 14](rfcs/0003-versioned-local-bundles.md).
  - Success: generated typed maps normalize deterministically, effectful
    expressions fail without invoking adapters, and metadata remains redacted
    except for the exact supported exposure annotation.

### 17.2. Instantiate local bundles without exposing private declarations

This step asks whether one bundle can support multiple consumers and versions
without conflating source decomposition with instantiation. Its result defines
the selected, isolated content that the digest verifier receives.

- [ ] 17.2.1. Resolve direct bundle paths through selected directory
  capabilities.
  - Requires 17.1.3 and 16.2.2.
  - [ ] Resolve literal bundle paths with exact name/version assertions;
    compose their entry fragments through the include loader using the agreed
    path anchors and a bundle-root capability, not importer-wide authority.
  - [ ] Reject sibling discovery, lexical and symlink escapes, root-only
    settings, and compatibility failures; do not search nearby versions or
    invoke Git because a local directory contains `.git` metadata.
  - See [RFC 0003 §§5.1, 8-10, and 14](rfcs/0003-versioned-local-bundles.md).
  - Success: direct imports select exactly the asserted bundle and identical
    bytes behave identically with absent, dirty, or detached Git metadata.
- [ ] 17.2.2. Enforce private exports and distinct bundle instances.
  - Requires 17.2.1.
  - [ ] Qualify internal references and expose only descriptor-listed names;
    validate missing exports, duplicate namespace use, and prohibited aliases.
    Permit exported entry points to depend on inaccessible private helpers.
  - [ ] Identify instances by requested source, identity, declared version,
    namespace, and normalized parameters; permit distinct instances without
    weakening repeated-plain-include rejection within their own compositions.
  - See [RFC 0003 §§1, 5.4, 7, and 8](rfcs/0003-versioned-local-bundles.md).
  - Success: two differently parameterized namespaces remain isolated, private
    cross-bundle references fail, and valid exported dependencies still build.
- [ ] 17.2.3. Compose nested local imports with bounded instance-cycle checks.
  - Requires 17.2.2.
  - [ ] Resolve child instances before parents, keep child namespaces private
    to the parent, and retain full nested provenance. Reject implicit
    re-exports and cycles using canonical instance identity.
  - [ ] Bound recursion and instance expansion so changing parameters cannot
    bypass termination; preserve the include loader's per-instance ownership.
  - See [RFC 0003 §§8, 13, and 18](rfcs/0003-versioned-local-bundles.md).
  - Success: bounded generated import graphs distinguish legitimate repeated
    instances from cycles and fail expansion limits before unbounded work.
- [ ] 17.2.4. Select catalogue versions deterministically and fail closed.
  - Requires 17.1.1, 17.1.2, and 17.2.2.
  - [ ] Classify immediate child descriptors in sorted byte order before
    selection. Any missing or malformed name/version fails the catalogue,
    regardless of directory spelling; valid other-name candidates stay out
    of scope, and valid incompatible versions remain unselected.
  - [ ] Apply the ratified comparator, abbreviation, caret, and prerelease
    rules, ignoring build metadata for precedence. Reject all tied highest
    candidates with sorted paths and versions, even when their digests match.
  - See [RFC 0003 §§5.2, 5.3, 9, and
    18](rfcs/0003-versioned-local-bundles.md).
  - Success: every normative vector and generated catalogue is independent
    of enumeration order, and neither malformed metadata nor a digest tie
    silently selects an older or arbitrary candidate.

### 17.3. Bind local selections to exact content and explicit lock changes

This step asks whether local reuse remains reviewable when files change without
a version bump. Its result supplies one verifier for both local files and the
later Git-tree adapter, without making Git identity the bundle definition.

- [ ] 17.3.1. Implement the canonical bundle-tree and resource verifier.
  - Requires 17.2.3 and 17.2.4.
  - [ ] Encode the versioned SHA-256 stream over graph-reachable regular files
    plus declared runtime resources, byte-sorted normalized root-relative
    paths, file types, declared resource mode bytes, and length-prefixed exact
    content. Never derive resource mode from host permission bits.
  - [ ] Open resources through the selected capability; reject missing,
    escaping, non-regular, and undeclared bundle-relative runtime references.
    Exclude Git data, timestamps, owners, generated state, and unrelated files.
  - See [RFC 0003 §§4, 11, 14, and 18](rfcs/0003-versioned-local-bundles.md).
  - Success: cross-platform golden streams preserve digests under relocation
    and metadata changes, change for reachable bytes/paths or declared modes,
    and reject every invalid runtime-resource class before execution.
- [ ] 17.3.2. Verify versioned local locks without mutating build inputs.
  - Requires 17.3.1 and 17.1.3.
  - [ ] Bind namespace, source root/path, name, version, parameter digest,
    content digest, and format versions; reject missing, malformed, ambiguous,
    or mismatched required records before compiling a selected bundle.
  - [ ] Keep normal CI/release operation locked and read-only. Make any
    ratified unlocked local mode explicit in human and JSON results; neither
    that mode nor manifest `lock: update` alone grants write authority.
  - See [RFC 0003 §§5.4, 11-13, and 16](rfcs/0003-versioned-local-bundles.md).
  - Success: unchanged-version content edits fail lock verification, ordinary
    builds preserve lock bytes, and records contain no absolute host paths.
- [ ] 17.3.3. Implement explicit, atomic local lock updates.
  - Requires 17.3.2.
  - [ ] Wire the local update operation ratified in 17.1.1; validate the
    complete new selection before atomic publication and coordinate concurrent
    writers. Preserve the previous usable lock on failure or cancellation.
  - [ ] Show bounded old/new identity, version, parameter-digest, and content-
    digest changes without disclosing parameter values; keep all work local.
  - See [RFC 0003 §§12, 15, 18, and 20](rfcs/0003-versioned-local-bundles.md).
  - Success: fault-injection and concurrency fixtures never publish partial
    selections, and only explicit update authority can change the lock.

### 17.4. Validate reusable bundles and their execution-context boundary

This step asks whether local bundles actually remove duplicated estate work
while retaining private interfaces and trustworthy discovery. A separate final
task joins the execution track so local-only delivery does not wait for it.

- [ ] 17.4.1. Expose bounded bundle metadata through the canonical CLI.
  - Requires 17.3.2 and 16.3.1.
  - [ ] Implement the ratified list, get, and verification surfaces and
    integrate help/graph/JSON views with names, exports, compatibility, source,
    lock status, and nested provenance. Never execute recipes for discovery.
  - [ ] Apply exact exposure annotations consistently to human, JSON, and
    debug output; do not serialize secret values or absolute host paths.
  - See [RFC 0003 §§13-15](rfcs/0003-versioned-local-bundles.md).
  - Success: snapshots expose the public interface and actionable failures,
    while private declarations, default-redacted values, and recipe sentinels
    remain inaccessible through every metadata surface.
- [ ] 17.4.2. Add local bundle migration and cross-platform interaction
  canaries.
  - Requires 17.3.3 and 17.4.1.
  - [ ] Package one repeated quality-gate set and consume it from at least
    two downstream repositories, retaining representative checked-in fixtures,
    namespaces, parameters, locks, and logical graph equivalence.
  - [ ] Cross direct/catalogue sources, nested imports, exports, parameter
    errors, version ties, resource mutations, and lock modes on Unix-like and
    Windows hosts. Use legacy-compatible recipes for independent delivery.
  - See [RFC 0003 §§16-18](rfcs/0003-versioned-local-bundles.md).
  - Success: recorded canaries eliminate copied declarations and reproduce
    offline; invalid selections fail before recipes run, without ambient Git.
- [ ] 17.4.3. Document and release the verified local bundle contract.
  - Requires 17.4.2.
  - [ ] Publish descriptor, parameter, export, SemVer, runtime-resource,
    digest, and lock-update guidance; reconcile the design, user, developer,
    contents, and repository-layout references with tested examples.
  - [ ] Record the additive version gate and canonical quality-gate evidence
    for the local-bundle part of issues `#593` and `#598`, preserving old
    manifests and rejecting unsupported schemas rather than ignoring fields.
  - See [RFC 0003 §§15-18 and 21](rfcs/0003-versioned-local-bundles.md).
  - Success: examples match the canaries, every initial contract has acceptance
    evidence, and local use never depends on external bundle acquisition.
- [ ] 17.4.4. Validate composed recipes across execution and shell boundaries.
  - Requires 16.3.3, 17.4.3, 14.3.3, and 11.3.3.
  - [ ] Exercise fragment and bundle recipes with literal/environment/tempdir
    `cwd`, typed captures, stderr pipes, rule/script boundaries, and direct,
    default-shell, and named-shell execution through builds and saved plans.
  - [ ] Keep process directories anchored to the importing workspace or a
    typed runner capability, not the declaring fragment/cache. Preserve
    source, include, and bundle provenance, binding isolation, and resource
    verification, and the ratified stream base; bundle content must not add
    shell definitions or replace a resolved shell through `PATH`, bindings,
    or placement.
  - See [RFC 0009 §§11-16](rfcs/0009-structured-command-working-directories.md),
    [RFC 0010 §§5, 9, 12, and
    17](rfcs/0010-runtime-bindings-and-secure-tempdirs.md),
    and [RFC 0011 §§5-8](rfcs/0011-allow-listed-structured-command-shells.md).
  - Success: the combined canaries for issue `#598` preserve outputs and
    failure provenance without widening authority, leaking bindings, or
    rebuilding a second shell resolver for composed recipes.

## 18. Digest-pinned external acquisition with offline builds

Hypothesis: explicit, bounded Git acquisition and verified cached content let
repositories consume tagged bundles reproducibly while ordinary builds and
metadata queries remain network-free and unable to change reviewed locks.

This phase implements [RFC 0004](rfcs/0004-digest-pinned-external-bundles.md)
only after 16.3.3 and 17.4.3 establish the local semantics. Its acceptance may
precede implementation, but external acquisition is not a prerequisite for the
local slices or for structured-command release. A remote tag is a discovery
handle, not proof of immutable content or publisher identity.

### 18.1. Establish one explicit and bounded acquisition boundary

This step asks whether the external source and transport policy can exclude
ambient Git behaviour before any remote object is trusted. Its decisions fix
what the object resolver and CLI may request.

- [ ] 18.1.1. Ratify external transport, cache, and update policy.
  - Requires 16.3.3 and 17.4.3.
  - [ ] Select an embedded Git implementation or narrow argv-based adapter
    from a capability inventory; settle SHA-1/SHA-256 support, URL and host
    policy, enterprise credentials, TLS and redirects, and finite transfer,
    object, pack, repository, peel-depth, time, and stale-cleanup limits.
  - [ ] Define offline/locked/update/refresh semantics and legal
    `--update-lock` combinations; reserve network and verified-cache
    publication to `bundle fetch`. Keep signatures optional and resolve the
    RFC's optional-signature implementation note through phase 19.
  - See [RFC 0004 §§6, 10-14, 18, and
    21](rfcs/0004-digest-pinned-external-bundles.md).
  - Success: one accepted policy records enforceable bounds and command
    authority without enabling network in local loaders or metadata queries.
- [ ] 18.1.2. Parse exact, digest-qualified Git bundle sources.
  - Requires 18.1.1 and 17.1.2.
  - [ ] Extend the closed source union with literal URL, exactly one tag or
    full commit, confined subdirectory, mandatory algorithm-qualified digest,
    optional object assertions, and `allow_shallow`; reject unknown keys.
  - [ ] Normalize tags only into `refs/tags/`; reject branch namespaces,
    `HEAD`, revision expressions, abbreviated object IDs, dynamic values,
    malformed digests, embedded URL credentials, and escaping subdirectories.
  - See [RFC 0004 §§5-7, 12, and
    17](rfcs/0004-digest-pinned-external-bundles.md).
  - Success: bounded properties and both Git object-format fixtures preserve
    exact identities and fail invalid source shapes before any transport call.
- [ ] 18.1.3. Implement the policy-controlled Git transport adapter.
  - Requires 18.1.1 and 18.1.2.
  - [ ] Use explicit injected transport, credentials, and owner-protected
    temporary storage; enforce all accepted size/time/redirect limits and
    terminate owned operations on cancellation without shell pipelines.
  - [ ] Disable hooks, clean/smudge and checkout filters, submodule/LFS
    acquisition, and unauthorized credential helpers or ambient configuration.
    Keep this port distinct from phase 6's local Git changeset operations.
  - See [RFC 0004 §§6.2, 10, 12, and
    13](rfcs/0004-digest-pinned-external-bundles.md).
  - Success: controlled-server and injected-failure cases enforce every bound,
    record no forbidden helper execution, and redact credentials in all sinks.

### 18.2. Verify exact Git objects as ordinary bundle content

This step asks whether object provenance and bundle meaning stay separate.
Its outcome must feed the existing local verifier, not introduce a checkout-
dependent definition of the canonical digest.

- [ ] 18.2.1. Acquire only the requested exact ref or commit.
  - Requires 18.1.3.
  - [ ] Request one normalized tag or full commit using bounded exact-ref
    acquisition; honour shallow policy without fetching unrelated branches or
    falling back to the remote default when an object is unavailable.
  - [ ] Verify repository object format and commit object type; report
    unadvertised-object refusal explicitly instead of broadening authority.
  - See [RFC 0004 §§6.1, 6.2, 7, and
    19](rfcs/0004-digest-pinned-external-bundles.md).
  - Success: protocol traces contain only allowed exact requests and fixtures
    reject short IDs, wrong types, missing refs, and object-format mismatches.
- [ ] 18.2.2. Retain bounded annotated-tag and lightweight-tag provenance.
  - Requires 18.2.1.
  - [ ] Record each tag object's declared target type and ID, peel a bounded
    cycle-checked chain to a commit, and reject tree/blob targets. Distinguish
    lightweight tags with no invented tag-object identity.
  - [ ] Verify optional manifest object assertions and preserve the requested
    and normalized tag, peel chain, final commit, and source identity.
  - See [RFC 0004 §§6.3-6.5, 9, and
    19](rfcs/0004-digest-pinned-external-bundles.md).
  - Success: generated and fixed chains terminate deterministically, reject
    cycles and excess depth, and never imply that annotation means signature.
- [ ] 18.2.3. Verify selected Git-tree bundles through the canonical verifier.
  - Requires 18.2.2 and 17.3.1.
  - [ ] Traverse subdirectory components as tree directories, rejecting
    symlinks before locating a regular descriptor. Verify descriptor identity,
    version, compatibility, graph-reachable files, and declared resources
    without hooks, a normal filtered checkout, or Gitlink traversal.
  - [ ] Compare the canonical digest with the reviewed manifest and lock;
    preserve declared resource modes and reject unsupported LFS materialization
    rather than fetching it. Unrelated upstream files remain outside the hash.
  - See [RFC 0004 §§8, 13, and 14](rfcs/0004-digest-pinned-external-bundles.md)
    and [RFC 0003 §§4 and 11](rfcs/0003-versioned-local-bundles.md).
  - Success: equivalent local and selected-tree bundles produce identical
    canonical streams after relocation, while reachable-byte or resource-mode
    changes and every directory/symlink/Gitlink escape fail verification.

### 18.3. Make cached provenance trustworthy without granting write authority

This step asks whether offline reuse can preserve both content and origin.
Its outcome gives builds and queries a verified read path separate from the
fetch command's publication and explicit lock-update capabilities.

- [ ] 18.3.1. Validate external locks and detect observed provenance drift.
  - Requires 18.2.3 and 17.3.2.
  - [ ] Extend lock records with canonical repository/subdirectory, requested
    tag and kind, tag object and peeled commit, object format, resolver and
    canonical-tree versions, parameter/content digests, and verification
    policy. Exclude informational acquisition timestamps from reproducibility.
  - [ ] Reject unexpected tag or commit identities even when digest bytes
    match; retain old/new provenance for explicit updates. Cache-only builds
    verify recorded identities, not an unobserved current remote tag value.
  - See [RFC 0004 §§6.5, 9, 14, and
    15](rfcs/0004-digest-pinned-external-bundles.md).
  - Success: drift fixtures fail for same-digest and changed-digest retagging,
    while locks expose neither credentials nor absolute cache paths.
- [ ] 18.3.2. Publish owner-protected verified caches with bounded leases.
  - Requires 18.3.1.
  - [ ] Key bundle entries by content digest and canonical-tree/metadata
    versions; keep bounded repository/object transport state separate.
    Validate complete entries before atomic publication through a fetch-owned
    write capability and coordinate concurrent readers and writers by leases.
  - [ ] Revalidate descriptor, versions, digest, and lock on every hit;
    reject corruption and clean abandoned temporary entries through bounded
    scans without trusting partial files or removing active leased entries.
  - See [RFC 0004 §§10 and 11](rfcs/0004-digest-pinned-external-bundles.md).
  - Success: interruption and concurrency cases expose only complete verified
    entries, retain valid prior state, and never turn a hit into a bypass.
- [ ] 18.3.3. Integrate cache-only bundles into builds and metadata discovery.
  - Requires 18.3.2, 16.3.1, and 17.4.1.
  - [ ] Give ordinary build/generate and metadata paths only verified-cache
    read access in every configured mode. Fail a miss with namespace and
    expected digest instead of fetching, publishing cache state, or updating
    locks; propagate verified relative provenance into composition.
  - [ ] Keep discovery recipe-free, even for external bundles, and prevent
    update-like manifest settings from acquiring a write or network port.
  - See [RFC 0004 §§4.2, 9-11, 14, and
    19](rfcs/0004-digest-pinned-external-bundles.md).
  - Success: instrumented cache hits, misses, corruption, and lock failures
    record zero network, cache-publication, lock-write, or recipe calls from
    metadata queries; ordinary builds likewise retain the cache-only boundary.

### 18.4. Deliver explicit acquisition and reproduce builds offline

This step asks whether the operator can review provenance changes once and
subsequently build without contacting the source. Its canary distinguishes
content pinning from mutable tag discovery and optional publisher trust.

- [ ] 18.4.1. Wire `bundle fetch` with explicit lock-update authorization.
  - Requires 18.3.1, 18.3.2, and 18.3.3.
  - [ ] Implement ratified offline, locked, update, and refresh modes under
    the sole acquisition command. Without `--update-lock`, preserve lock
    files in every mode; reject illegal combinations before network access.
  - [ ] Permit authorized lock replacement only after full verification and
    atomic publication preparation. Show bounded old/new tag, commit, version,
    and digest changes; retain typed failures and low-cardinality telemetry.
  - See [RFC 0004 §§9, 10, 15, and
    16](rfcs/0004-digest-pinned-external-bundles.md).
  - Success: the command/mode/flag matrix proves offline makes no request,
    ordinary fetch preserves lock bytes, and failed authorized updates leave
    the previous valid selection usable without leaking credentials.
- [ ] 18.4.2. Add the adversarial protocol, cache, and migration matrix.
  - Requires 18.4.1.
  - [ ] Use a local protocol test server for exact commits, both tag kinds,
    nested peel chains, wrong object formats, refused fetches, redirects,
    forbidden helpers, transfer limits, retagging, and wrong-subtree digests.
  - [ ] Cross cache hit/miss/corruption, concurrent publication, stale cleanup,
    every network mode, lock flags, and metadata discovery; migrate an unchanged
    vendored bundle and compare logical graphs after external verification.
  - See [RFC 0004 §§17-19](rfcs/0004-digest-pinned-external-bundles.md).
  - Success: deterministic offline reruns need no public internet, every
    negative fixture fails at its named boundary, and provenance changes
    remain visible even when the canonical bundle content does not change.
- [ ] 18.4.3. Document and release the external-source contract.
  - Requires 18.4.2.
  - [ ] Publish executed fetch, verification, update, refresh, offline, and
    cache-miss guidance; document limits, credential ownership, retagging,
    resource coverage, and the distinction between digests and signatures.
  - [ ] Record one downstream tagged-bundle canary and offline graph evidence;
    align CLI/schema versions, security, design, user, developer, and layout
    guidance, and run canonical quality gates without widening local authority.
  - See [RFC 0004 §§14-19 and 22](rfcs/0004-digest-pinned-external-bundles.md).
  - Success: the documented canary builds from a verified cache with network
    disabled, unsupported schemas fail clearly, and no ordinary build or
    discovery command can mutate its reviewed external lock.

## 19. Evidence-led composition and provenance extensions

Hypothesis: local and externally pinned bundle canaries can identify useful
extensions without weakening deterministic composition, private interfaces,
or the explicit acquisition boundary of the initial release.

These tasks decide scope only; they do not block phases 16 to 18 or reopen
rejected implicit network, mutable-ref, or silent-override behaviour. Phase 15
continues to own deferred structured-command extensions under its existing IDs.

### 19.1. Evaluate extensions only against recorded consumer needs

This step asks which remaining authoring or trust problems justify new public
contracts. Each outcome either defines a separately reviewed follow-on design
or retains the current boundary explicitly.

- [ ] 19.1.1. Decide whether local composition needs a broader public surface.
  - Requires 17.4.3.
  - [ ] Use migration evidence to assess extraction tooling, additional
    fragment-scoped settings, richer parameter constraints, and nested-bundle
    re-exports; specify namespace, compatibility, and capability effects before
    authorizing any extension.
  - See [RFC 0002 §16](rfcs/0002-repository-relative-includes.md) and
    [RFC 0003 §§6, 8, and 20](rfcs/0003-versioned-local-bundles.md).
  - Success: each candidate has a named consumer and separate accepted design,
    or an explicit defer/reject outcome without changing initial semantics.
- [ ] 19.1.2. Decide optional publisher-identity policy without replacing
  digests.
  - Requires 18.4.3.
  - [ ] Reconcile the optional signed-tag item in RFC 0004's implementation
    outline with its deferred signature/transparency policy; assess approved
    keys, identities, attestations, and transparency only through explicit
    operator configuration and a separate security-reviewed contract.
  - See [RFC 0004 §§4.2, 14, 18, and
    21](rfcs/0004-digest-pinned-external-bundles.md).
  - Success: any accepted policy remains additive to mandatory digest and lock
    verification, with no claim that an unsigned or annotated tag proves trust.
- [ ] 19.1.3. Decide additional source forms without implicit acquisition.
  - Requires 18.4.3.
  - [ ] Assess digest-pinned archives, registry discovery, SCP-style URLs,
    verified LFS sources, and trust-class cache sharing only where canaries
    justify them; reject proposals that weaken exact identity, reviewed
    content, bounded transport, or cache-only build and query semantics.
  - See [RFC 0004 §§12, 13, 20, and
    21](rfcs/0004-digest-pinned-external-bundles.md).
  - Success: every accepted source extension has a separate canonicalization,
    provenance, authority, and migration contract before implementation.
