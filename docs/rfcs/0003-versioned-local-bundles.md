# RFC 0003: Versioned local manifest bundles

## Preamble

- **RFC number:** 0003
- **Status:** Proposed
- **Created:** 2026-08-26
- **Target:** Reusable, parameterized Netsuke manifest composition
- **Depends on:** RFC 0002 repository-relative includes

## 1. Summary

This RFC builds a reusable bundle model on top of repository-relative includes.
A bundle is a local directory with explicit identity, semantic version,
parameters, exports, compatibility requirements, content provenance, and one
entry manifest.

Bundles remain local in this RFC. Netsuke performs no network access and does
not clone repositories. A bundle may be vendored into the current repository,
checked out as part of a larger workspace, or supplied through an explicitly
selected local bundle catalogue.

The central rule is:

> A bundle instance is identified by source identity, declared version,
> namespace, and rendered parameters; none of those inputs may be inferred from
> mutable ambient state.

The design supports two local selection forms:

- a direct path to one bundle, with an exact version assertion; and
- a local catalogue containing several semantic versions, with deterministic
  highest-compatible selection.

A lock record captures the selected local path, bundle identity, version,
parameter digest, and canonical content digest. This makes local composition
reviewable and prepares the provenance model for the external Git boundary in
RFC 0004.

## 2. Problem

RFC 0002 splits a large Netsukefile into local fragments but deliberately
rejects repeated inclusion and duplicate names. That is correct for source-file
decomposition, but it does not solve reusable quality-gate packages such as:

- a standard Rust format, lint, rustdoc, Whitaker, test, and audit suite;
- a Python format, Ruff, type-check, and pytest suite;
- a release packaging workflow;
- a documentation and spelling bundle; or
- an organization-wide CI contract instantiated with repository-specific
  feature lanes and tool versions.

Copying fragments between repositories loses version identity. Adding implicit
overrides makes updates difficult to review. A reusable bundle needs a public
contract describing what it imports, what it exports, which parameters it
accepts, and which Netsuke versions it supports.

Semantic version selection also needs a deterministic local rule. Choosing the
first directory returned by the filesystem, silently accepting an incompatible
bundle, or treating a directory name as authoritative would undermine
reproducibility.

## 3. Goals and non-goals

### 3.1 Goals

This RFC aims to:

- define a versioned, reusable local bundle format;
- permit typed, declared parameters with defaults and validation;
- define explicit exports rather than exposing every internal declaration;
- instantiate the same bundle more than once under distinct namespaces;
- resolve local semantic versions deterministically;
- verify bundle metadata against selected version constraints;
- retain source, parameter, and bundle provenance through diagnostics and JSON;
- record a canonical content digest and lock information;
- remain fully offline; and
- provide the local semantic foundation for RFC 0004.

### 3.2 Non-goals

This RFC does not:

- fetch or clone a Git repository;
- resolve remote tags, branches, or commits;
- trust a bundle merely because its directory name resembles a version;
- permit undeclared parameters;
- permit a bundle to access declarations outside its import contract;
- silently expose private rules, targets, actions, variables, or providers;
- define cryptographic publisher identity or signature verification; or
- allow floating external dependencies.

## 4. Bundle layout

A bundle directory contains one required metadata file and one entry manifest:

```text
rust-quality/
├── NetsukeBundle.yaml
├── Netsukefile.bundle.yaml
├── fragments/
│   ├── lint.yaml
│   └── test.yaml
└── README.md
```

`NetsukeBundle.yaml` is not a build manifest. It is a small, strictly parsed
bundle descriptor. `Netsukefile.bundle.yaml` is the entry fragment composed
using RFC 0002 semantics.

The descriptor has this shape:

```yaml
bundle:
  name: df12.rust-quality
  version: 1.4.2
  manifest: Netsukefile.bundle.yaml
  requires:
    netsuke: ">=0.2.0, <0.3.0"
    manifest: ">=1.1.0, <2.0.0"

parameters:
  cargo:
    type: string
    default: cargo
  features:
    type: sequence<string>
    default: []
  deny_warnings:
    type: bool
    default: true

exports:
  actions:
    - check-fmt
    - lint
    - test
    - all
  variables:
    - cargo
    - features
```

Unknown descriptor keys are errors. The descriptor is parsed without Jinja.

## 5. Import syntax

### 5.1 Direct bundle path

A direct import names one bundle directory and asserts an exact version:

```yaml
bundles:
  - source:
      path: build/bundles/rust-quality
    version: "=1.4.2"
    as: rust
    with:
      cargo: cargo
      features:
        - serde
        - tracing
```

The selected bundle's descriptor version must satisfy the requirement. A direct
path does not search sibling directories.

### 5.2 Local catalogue resolution

A catalogue import names a directory whose immediate children are candidate
bundle directories:

```yaml
bundles:
  - source:
      catalogue: build/bundle-catalogue/rust-quality
    version: "^1.4"
    as: rust
```

A possible catalogue is:

```text
build/bundle-catalogue/rust-quality/
├── 1.3.9/
│   └── NetsukeBundle.yaml
├── 1.4.1/
│   └── NetsukeBundle.yaml
├── 1.4.2/
│   └── NetsukeBundle.yaml
└── 2.0.0/
    └── NetsukeBundle.yaml
```

Netsuke examines immediate children in sorted byte order, parses each descriptor,
and groups candidates by the descriptor's declared bundle name. Directory names
are hints only. They need not equal the version, although a mismatch should
produce a diagnostic or policy warning because it is likely confusing.

Selection chooses the highest stable semantic version satisfying the
requirement. Pre-release versions participate only when the requirement itself
admits a pre-release. Build metadata does not affect precedence.

Two candidates with the same bundle name and semantic version but different
canonical content digests are an ambiguity error.

### 5.3 Import fields

| Field | Type | Default | Meaning |
| --- | --- | --- | --- |
| `source.path` | local directory | one source required | Direct bundle directory. |
| `source.catalogue` | local directory | one source required | Directory of candidate bundles. |
| `version` | semantic version requirement | required | Accepted bundle version range. |
| `as` | identifier | required | Namespace for this bundle instance. |
| `with` | parameter mapping | empty | Explicit parameter values. |
| `lock` | `required` or `update` | `required` in CI | Lock disposition. |

Table 1: Local bundle import fields.

A bundle namespace must be unique in the root composition.

## 6. Parameter model

Parameters are declared in the bundle descriptor and supplied through `with`.
The initial type vocabulary is:

- `string`;
- `bool`;
- `integer`;
- `path`;
- `sequence<string>`;
- `sequence<path>`; and
- `mapping<string, string>`.

Every parameter may define a default. A parameter without a default is
required. Unknown supplied parameters are errors. Values are validated before
the entry manifest's Jinja expressions evaluate.

Parameter expressions in the importing manifest may use the ordinary pure
manifest context, but they must render to the declared type. They may not invoke
network, subprocess, clock, or unrestricted filesystem helpers.

Inside a bundle, parameters appear under a dedicated immutable object:

```jinja
{{ bundle.params.cargo }}
{{ bundle.params.features }}
```

A bundle may not mutate a parameter or observe the importer's unrelated
variables.

The normalized parameter value contributes to bundle instance identity and the
compiled graph fingerprint.

## 7. Export and namespace semantics

A bundle is private by default. Only descriptor-listed declarations become
visible to the importer.

Internal rules, variables, targets, and actions remain available within the
bundle's own composition but cannot be referenced from the root or another
bundle.

An imported action `lint` from namespace `rust` is exposed as:

```text
rust::lint
```

Internal references are rewritten before global resolution. Exports may refer
to private bundle declarations, but consumers cannot bypass the exported entry
point to reach them.

Export lists are validated against the composed bundle. Missing exports are
errors. A declaration may not be exported under two names in the initial
surface.

## 8. Bundle composition

The entry manifest and its local fragments compose under RFC 0002 with these
additional boundaries:

- include paths remain relative to the bundle directory;
- symlinks may not escape the bundle root;
- the bundle cannot include files from the importing repository outside its own
  directory;
- root-only manifest settings remain forbidden;
- bundle parameters enter before bundle-local Jinja evaluation; and
- only declared exports leave the bundle namespace.

A bundle may import another local bundle. Nested imports resolve before the
parent bundle, and their namespaces are private to the parent unless explicitly
re-exported by a later RFC.

The import graph must remain acyclic by canonical bundle instance identity.

## 9. Version and compatibility semantics

Bundle versions follow Semantic Versioning 2.0.0.

The descriptor's `requires.netsuke` and `requires.manifest` requirements are
checked before parsing the entry manifest beyond the minimum needed for safe
diagnostics.

A bundle import fails when:

- no candidate satisfies the requested version;
- the selected bundle rejects the running Netsuke version;
- the selected bundle rejects the active manifest-format version;
- candidate identity or version is ambiguous;
- an exact direct-path assertion does not match; or
- the lock record selects different content.

Netsuke must not silently choose an older candidate because a newer compatible
candidate contains malformed metadata. Malformed candidates within the searched
version range make resolution fail with provenance.

## 10. Tagged Git relationship

This RFC remains local-only, but it reserves provenance fields needed for tagged
Git resolution under RFC 0004:

- requested source kind;
- requested version requirement;
- bundle name and declared version;
- source repository identity when known;
- requested Git tag when supplied by a later resolver;
- tag object identifier for annotated tags;
- peeled commit identifier;
- bundle subdirectory;
- canonical content digest; and
- lock-record version.

A locally checked-out bundle may carry advisory Git provenance generated by a
vendor/update tool, but local resolution does not invoke Git or trust the
working tree's current tag. The external resolver in RFC 0004 owns exact tag
lookup and verification.

This separation avoids a surprising rule where the same local files resolve
differently merely because `.git` is present.

## 11. Canonical content digest

Netsuke computes a digest over a canonical bundle tree containing:

- every regular file reachable from the descriptor and entry-manifest include
  graph;
- normalized workspace-relative paths within the bundle;
- file type and executable bit where semantically relevant; and
- exact file bytes.

The canonicalization excludes:

- `.git` data;
- filesystem timestamps, owners, and inode numbers;
- files not reachable from the bundle composition unless the descriptor lists
  them as runtime resources; and
- generated Netsuke state.

The initial algorithm is SHA-256 with an algorithm-qualified representation:

```text
sha256:4e9f...
```

The canonical tree format is versioned so a future algorithm or normalization
change cannot silently reinterpret an old digest.

## 12. Lock records

A repository using bundles stores deterministic selections in a versioned
Netsuke lock file. One conceptual record is:

```yaml
bundles:
  rust:
    name: df12.rust-quality
    version: 1.4.2
    source:
      kind: local-catalogue
      path: build/bundle-catalogue/rust-quality/1.4.2
    parameters_sha256: sha256:8b25...
    content_sha256: sha256:4e9f...
```

Normal CI and release builds require the lock record to match. An explicit
update command may select a newer compatible version and atomically rewrite the
record.

The lock file never contains absolute host paths. Paths are relative to the
workspace root or an explicitly named local catalogue root.

Changing bundle files without changing the declared version is visible as a
content-digest mismatch. Local development may offer a deliberate unlocked mode,
but it must be explicit in human and JSON output and should not be the CI
default.

## 13. Provenance and diagnostics

Every declaration originating from a bundle retains:

- import namespace;
- bundle name and version;
- direct path or catalogue selection;
- descriptor and source file spans;
- parameter source and normalized parameter digest;
- canonical content digest; and
- nested bundle chain.

Human diagnostics should remain concise. JSON metadata may expose the complete
bounded provenance object without leaking absolute paths or parameter values
that may contain secrets.

Secret-shaped parameters must be passed through a future explicit secret type;
the initial parameter model should document that ordinary values may appear in
debug metadata and therefore must not carry credentials.

## 14. Security and capability boundaries

Bundle paths and catalogue paths are literal, repository-relative paths opened
through directory capabilities. Symlinks may not escape their selected bundle
or catalogue boundary.

Bundle metadata and manifests cannot:

- request network access during composition;
- discover arbitrary sibling files;
- invoke Git;
- read undeclared importer variables;
- enumerate the process environment; or
- export undeclared private implementation details.

A bundle's recipes retain the same execution capabilities as ordinary manifest
recipes. Bundle provenance does not make an unsafe command safe.

## 15. CLI and metadata surface

The following conceptual commands should exist, subject to the CLI vocabulary
decision:

```console
netsuke bundle list
netsuke bundle inspect rust
netsuke bundle update rust
netsuke bundle verify
```

Equivalent placement beneath existing commands is acceptable if it avoids a
new top-level noun.

Human and JSON output should show:

- namespace;
- bundle name and selected version;
- source kind and relative path;
- content digest;
- lock status;
- exported declaration names; and
- compatibility requirements.

Parameter values are redacted or summarized by type unless the schema marks
them safe for display.

## 16. Compatibility and migration

RFC 0002 includes remain valid and do not acquire version semantics.

A local fragment can migrate into a bundle by:

1. adding a descriptor;
2. naming and versioning the bundle;
3. declaring parameters instead of reading importer variables;
4. listing exports;
5. importing it under a namespace; and
6. generating a lock record.

A bundle descriptor or import syntax requires an additive manifest-format minor
version. Older Netsuke versions must reject it cleanly.

## 17. Implementation plan

### Phase 1: descriptor and types

- Implement strict bundle descriptor parsing.
- Add semantic version and requirement validation.
- Add typed parameter schemas and normalization.

### Phase 2: local direct imports

- Resolve direct paths through capabilities.
- Compose entry manifests under RFC 0002.
- Implement private-by-default exports and namespace rewriting.

### Phase 3: catalogue resolution

- Enumerate immediate candidate directories in sorted order.
- Parse descriptors and select the highest compatible stable version.
- Reject duplicate identity/version candidates.

### Phase 4: digest and lock

- Define the versioned canonical bundle-tree format.
- Compute content and parameter digests.
- Add atomic lock verification and update flows.

### Phase 5: metadata and downstream canary

- Expose bounded provenance in human and JSON output.
- Package one repeated estate quality-gate set as a versioned local bundle and
  consume it from at least two downstream repositories.

## 18. Test strategy

The implementation must include:

- direct exact-version success and mismatch tests;
- catalogue highest-compatible selection;
- stable versus pre-release selection;
- malformed candidate and duplicate identity/version failures;
- typed parameter defaults, missing values, type failures, and unknown keys;
- private declaration isolation and explicit export tests;
- multiple instances under distinct namespaces;
- nested bundle cycles;
- symlink and lexical boundary escapes;
- content-digest stability across timestamp changes;
- digest changes for reachable byte, mode, and path changes;
- lock verification and atomic update tests;
- JSON provenance snapshots with redaction; and
- property tests over bounded version catalogues and parameter maps.

Cross-platform tests must cover path spelling and Windows case behaviour without
making version selection depend on host directory enumeration.

## 19. Alternatives considered

### 19.1 Treat any included directory as a bundle

Without metadata, parameters, exports, and compatibility requirements, a
directory is merely a larger fragment. Rejected.

### 19.2 Use directory names as versions

Directory names are convenient for humans but not authoritative and can drift
from the bundle's actual contract. Rejected; descriptor versions decide.

### 19.3 Last-writer-wins customization

Allowing an importer to redefine bundle variables hides its supported parameter
surface and makes updates fragile. Rejected in favour of declared parameters.

### 19.4 Resolve local Git tags directly

A working tree can be detached, dirty, shallow, missing tags, or embedded
without `.git`. Making local composition depend on repository metadata would
produce different results for identical files. Rejected here; RFC 0004 owns
explicit tagged Git resolution.

## 20. Open questions

- Should local catalogue roots be declared globally or only per import?
- Should bundle parameters support enums and constrained strings in the initial
  implementation?
- Should runtime resource files be declared in the descriptor or inferred from
  explicit manifest fields?
- Should local lock updates require an explicit `--update` flag or a dedicated
  command?
- What stable vocabulary should describe an unlocked development import?

## 21. Recommendation

Adopt versioned local bundles as a strict layer above repository-relative
includes: explicit identity, SemVer, typed parameters, private-by-default
exports, deterministic local resolution, canonical digests, and lock records.

Do not couple this local model to ambient Git state or networking. That clean
boundary lets RFC 0004 add exact tagged Git resolution and external provenance
without changing what a bundle means.