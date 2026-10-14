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

> A bundle instance is identified by requested source and bundle identity,
> declared version, namespace, and rendered parameters; none of those inputs
> may be inferred from mutable ambient state.

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
├── tools/
│   └── run-quality-check
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
  runtime_resources:
    - path: tools/run-quality-check
      executable: true
  requires:
    netsuke: ">=0.2.0, <0.3.0"
    manifest: ">=1.1.0, <2.0.0"

parameters:
  cargo:
    type: string
    default: cargo
    expose: non-secret
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

`runtime_resources` is required, including when its value is an explicit empty
list. It names every regular file that the bundle makes available to a spawned
process or reads as runtime data but that is not reachable from the descriptor
or entry-manifest include graph. Each resource is a mapping with a required
`path` and `executable` boolean. The path is a literal, bundle-relative,
normalized path to one regular file. It may not contain Jinja, glob syntax,
parent traversal, or a symlink escape. A bundle must declare every executable
script, helper binary, configuration file, and data file it uses at runtime.
The declared `executable` value is the resource's platform-independent mode:
the canonical digest writes exactly one mode byte, `0x01` for `true` and `0x00`
for `false`; it never derives this value from host permission bits. The
resolver opens each declared resource through the selected bundle directory
capability and includes its path, mode byte, and exact bytes in the canonical
content digest. Duplicate resource paths are errors. The manifest compiler must
reject a bundle-relative runtime file reference that is neither graph-reachable
nor declared in `runtime_resources`.

## 5. Import syntax

### 5.1 Direct bundle path

A direct import names the expected bundle identity, one bundle directory, and
asserts an exact version:

```yaml
bundles:
  - name: df12.rust-quality
    source:
      path: build/bundles/rust-quality
    version: "=1.4.2"
    as: rust
    with:
      cargo: cargo
      features:
        - serde
        - tracing
```

The selected descriptor name and version must satisfy the import's `name` and
version requirements. A direct path does not search sibling directories.

### 5.2 Local catalogue resolution

A catalogue import names a directory whose immediate children are candidate
bundle directories:

```yaml
bundles:
  - name: df12.rust-quality
    source:
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

Netsuke examines immediate children in sorted byte order and classifies each
descriptor before version selection. A candidate is valid only when its
`bundle.name` and `bundle.version` fields are present and strictly parse as a
bundle identifier and Semantic Version, respectively. A missing or malformed
field is a malformed candidate: resolution fails with the candidate's
repository-relative path and parse diagnostic, regardless of the directory
name. Directory names are hints only and cannot repair or exclude malformed
metadata. A valid candidate declaring another name is diagnosed as an
out-of-scope candidate and does not participate in selection. A valid candidate
with the requested name but an incompatible version is simply not selected.

Selection chooses the highest semantic version satisfying the requirement.
Pre-release versions participate only when the requirement itself admits a
pre-release. Build metadata does not affect precedence.

### 5.3 Semantic Version requirements

Requirements are comma-separated comparator sets. Whitespace around commas and
comparators is insignificant. The grammar is:

```plaintext
requirement   = comparator ("," comparator)*
comparator    = operator? version | "^" version
operator      = "=" | ">" | ">=" | "<" | "<="
version       = complete | abbreviated
complete      = major "." minor "." patch ["-" prerelease] ["+" build]
abbreviated   = major ["." minor]
```

`major`, `minor`, and `patch` are non-negative decimal integers without leading
zeroes, except for zero itself. Pre-release and build identifiers use the
Semantic Versioning 2.0.0 rules. Only a complete three-component version may
include pre-release or build metadata; abbreviated versions are valid only when
neither metadata component is present. A bare three-component version and its
`=` form mean an exact core-version requirement. Because build metadata is
ignored for requirement matching and precedence, `=1.4.2` matches a candidate
declaring `1.4.2+local`. A requirement containing a pre-release identifier
matches only candidates with the same core tuple and exactly the same ordered
pre-release identifiers; build metadata remains ignored for matching and
precedence. A bare `1.4` means `>=1.4.0, <1.5.0`, and a bare `1` means
`>=1.0.0, <2.0.0`. When an operator is present, omitted components are zero, so
`>=1.4` means `>=1.4.0` and `<2` means `<2.0.0`.

The caret operator admits compatible changes: `^1.4` means `>=1.4.0, <2.0.0`;
`^0.4` means `>=0.4.0, <0.5.0`; and `^0.0.3` means `>=0.0.3, <0.0.4`. A
candidate with a pre-release identifier is excluded unless at least one
comparator in the set contains a pre-release identifier with the same major,
minor, and patch tuple. Thus, `^1.4` excludes `1.4.0-rc.1`, while
`>=1.4.0-rc.1, <1.5.0` admits it. Build metadata is ignored for precedence and
requirement matching.

The resolver applies every comparator in a set, then selects the highest
matching version after applying the pre-release rule. These selection vectors
are normative:

| Requirement            | Candidates                          | Result                |
| ---------------------- | ----------------------------------- | --------------------- |
| `^1.4`                 | `1.3.9`, `1.4.1`, `1.9.0`, `2.0.0`  | select `1.9.0`        |
| `^1.4`                 | `1.3.9`, `2.0.0`                    | no compatible version |
| `>=1.4, <2.0`          | `1.4.0`, `1.9.9`, `2.0.0`           | select `1.9.9`        |
| bare `1.4`             | `1.4.0`, `1.4.9`, `1.5.0`           | select `1.4.9`        |
| `^1.4`                 | `1.4.0-rc.1`, `1.4.1-beta`, `1.4.1` | select `1.4.1`        |
| `>=1.4.0-rc.1, <1.5.0` | `1.4.0-rc.2`, `1.4.0-beta`          | select `1.4.0-rc.2`   |
| `=1.4.2`               | `1.4.2+local`                       | select sole candidate |
| `=1.4.2`               | `1.4.2+one`, `1.4.2+two`            | ambiguity error       |
| `=1.4.2-rc.1`          | `1.4.2-rc.1`                        | select sole candidate |
| `=1.4.2-rc.1`          | `1.4.2-rc.2`                        | no compatible version |
| `=1.4.2-rc.1`          | `1.4.2-rc.1+local`                  | select sole candidate |

Table 2: Semantic Version requirement selection vectors.

Malformed descriptors fail during candidate classification, before any vector
is selected. Candidates with valid metadata but another name or an incompatible
version are excluded without changing the highest-compatible selection among
valid candidates. After filtering by declared bundle name and requirement, if
more than one candidate has the highest-precedence matching version, resolution
fails with a deterministic ambiguity diagnostic naming every tied candidate's
repository-relative path and declared version in sorted byte order. This
applies regardless of canonical content digest; a digest cannot break the tie.

### 5.4 Import fields

| Field              | Type                         | Default             | Meaning                             |
| ------------------ | ---------------------------- | ------------------- | ----------------------------------- |
| `name`             | bundle identifier            | required            | Expected declared bundle identity.  |
| `source.path`      | local directory              | one source required | Direct bundle directory.            |
| `source.catalogue` | local directory              | one source required | Directory of candidate bundles.     |
| `version`          | semantic version requirement | required            | Accepted bundle version range.      |
| `as`               | identifier                   | required            | Namespace for this bundle instance. |
| `with`             | parameter mapping            | empty               | Explicit parameter values.          |
| `lock`             | `required` or `update`       | `required` in CI    | Lock disposition.                   |

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

Parameter values are redacted in human diagnostics, JSON metadata, and debug
metadata by default. A declaration may opt into exposure only with the exact
annotation `expose: non-secret`; no other annotation exposes a value. This
annotation is an assertion by the bundle author that the value is safe to
display, not a request to infer safety from the parameter name or type. The
future explicit `secret` parameter type remains always redacted and cannot be
combined with `expose: non-secret`.

Parameter expressions in the importing manifest may use the ordinary pure
manifest context, but they must render to the declared type. They may not
invoke network, subprocess, clock, or unrestricted filesystem helpers.

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
- no candidate has the requested declared bundle name;
- the selected bundle rejects the running Netsuke version;
- the selected bundle rejects the active manifest-format version;
- candidate identity or version is ambiguous;
- an exact direct-path assertion does not match; or
- the lock record selects different content.

Netsuke must not silently choose an older candidate because a newer compatible
candidate contains malformed metadata. Candidate classification fails before
selection when any descriptor has a missing or malformed `bundle.name` or
`bundle.version`; the failure includes the candidate's repository-relative path
and parse diagnostic. This fail-closed rule applies even when the directory
name would look outside the requested range, because directory names are not
authoritative. Only candidates with strictly valid metadata can be excluded as
out of scope or incompatible and then omitted from highest-compatible selection.

## 10. Tagged Git relationship

This RFC remains local-only, but it reserves provenance fields needed for
tagged Git resolution under RFC 0004:

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
- every regular file named by `runtime_resources`;
- normalized paths relative to the selected bundle root, encoded with `/` as
  the separator;
- file type metadata and, for each declared runtime resource, exactly one
  platform-independent mode byte (`0x01` for executable and `0x00` for
  non-executable); and
- exact file bytes.

The canonical stream sorts those bundle-root-relative paths bytewise and
encodes each record as its normalized path, file-type marker, declared mode
byte when present, and length-prefixed exact bytes. Resource mode bytes come
only from the descriptor's `executable` boolean, never from host filesystem
permission bits. The bundle root is not part of a record, so relocating an
otherwise identical bundle to another workspace or Git subdirectory preserves
its digest.

The canonicalization excludes:

- `.git` data;
- filesystem timestamps, owners, and inode numbers;
- files neither reachable from the bundle composition nor named by
  `runtime_resources`; and
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
content-digest mismatch. Local development may offer a deliberate unlocked
mode, but it must be explicit in human and JSON output and should not be the CI
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
bounded provenance object without leaking absolute paths or parameter values.
Parameter values remain redacted unless their declaration carries the explicit
`expose: non-secret` annotation. Secret-shaped parameters must be passed
through a future explicit `secret` type, which remains redacted regardless of
annotations.

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

Equivalent placement beneath existing commands is acceptable if it avoids a new
top-level noun.

Human and JSON output should show:

- namespace;
- bundle name and selected version;
- source kind and relative path;
- content digest;
- lock status;
- exported declaration names; and
- compatibility requirements.

Parameter values are redacted by default and may be displayed only when the
schema carries `expose: non-secret`; secret-typed values are always redacted.

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
- comparator-set, omitted-component, caret, and pre-release selection vectors;
- malformed candidate and duplicate identity/version failures;
- catalogue candidates with distinct declared names, proving selection uses the
  import's required name before comparing compatible versions;
- typed parameter defaults, missing values, type failures, and unknown keys;
- private declaration isolation and explicit export tests;
- multiple instances under distinct namespaces;
- nested bundle cycles;
- symlink and lexical boundary escapes;
- content-digest stability across timestamp changes;
- digest changes for reachable and declared runtime-resource byte, mode, and
  path changes;
- resource mode digest changes use descriptor `executable: true` versus
  `executable: false`, independent of host permission bits;
- missing, escaping, non-regular, and undeclared runtime-resource failures;
- lock verification and atomic update tests;
- JSON provenance snapshots proving default redaction and explicit
  `expose: non-secret` opt-in; and
- property tests over bounded version catalogues and parameter maps.

Cross-platform tests must cover path spelling and Windows case behaviour
without making version selection depend on host directory enumeration.

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
