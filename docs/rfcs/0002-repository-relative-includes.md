# RFC 0002: Repository-relative manifest includes

## Preamble

- **RFC number:** 0002
- **Status:** Proposed
- **Created:** 2026-08-26
- **Target:** Netsuke manifest composition
- **Depends on:** RFC 0001 only for shared provenance and diagnostics concepts

## 1. Summary

This RFC introduces local, repository-relative manifest includes with a fixed
resolution order, deterministic merge semantics, cycle detection, retained
source provenance, and no network access.

The initial feature is deliberately conservative. Included files are static
manifest fragments. Include paths are literal paths rather than Jinja
expressions or glob patterns, every resolved file must remain inside the
workspace boundary, and duplicate declarations are errors unless a future RFC
adds an explicit override construct.

The central rule is:

> Composition order is explicit, stable, and incapable of silently replacing a
> declaration.

A root manifest includes fragments in declaration order. Each fragment first
composes its own includes, then contributes its declarations. The including
file contributes last. This produces a deterministic depth-first post-order
without depending on directory iteration, hash-map order, or filesystem
metadata order.

## 2. Problem

Real Netsukefiles repeat the same local task groups:

- Rust formatting, Clippy, rustdoc, testing, and Whitaker actions;
- Python formatting, linting, type checking, and tests;
- release, audit, spelling, and documentation gates;
- feature-lane matrices; and
- local helper rules shared by several subprojects.

Jinja macros can reduce repetition inside one manifest, but they do not provide
source-file boundaries. Copying one large Netsukefile between repositories
creates drift; splitting commands into shell scripts loses graph metadata and
discoverable action descriptions.

A generic YAML include is not enough. Netsuke must define:

- which path anchors an include;
- whether a symlink may escape the repository;
- the order in which nested fragments compose;
- what happens when two files define the same name;
- when Jinja evaluates relative to composition;
- how source spans survive into diagnostics;
- whether `help targets` can inspect the graph without rendering recipes; and
- which content contributes to the generated graph hash.

Without a normative contract, two implementations can produce different graphs
from the same files while both claiming to support includes.

## 3. Goals and non-goals

### 3.1 Goals

This RFC aims to:

- split one repository's manifest into reviewable local fragments;
- resolve every include relative to the file containing the include;
- keep all initial include access within the effective workspace boundary;
- define one deterministic nested-composition order;
- reject cycles and duplicate declaration identities before graph generation;
- preserve file and source-span provenance through expansion and diagnostics;
- make every included byte part of graph invalidation and reproducibility;
- keep metadata queries side-effect-free; and
- provide a foundation for versioned bundles without prematurely adding a
  package manager.

### 3.2 Non-goals

This RFC does not:

- fetch network resources;
- resolve Git repositories, commits, branches, or tags;
- select among semantic versions;
- parameterize reusable bundles;
- permit Jinja, `env()`, `glob()`, or command output in include paths;
- silently override duplicate variables, rules, targets, or actions;
- include files outside the effective workspace root;
- define remote trust or signature policy; or
- make included fragments independently executable entry points.

RFC 0003 adds versioned local bundles. RFC 0004 defines the later,
digest-pinned external Git boundary.

## 4. Terminology

- **Root manifest:** The Netsukefile selected by the CLI.
- **Fragment:** A YAML document loaded only through `includes`.
- **Including file:** The file whose `includes` entry names another fragment.
- **Composition unit:** One parsed root manifest or fragment with retained
  provenance.
- **Effective workspace root:** The workspace selected after CLI `-C` and
  manifest selection rules.
- **Canonical include identity:** The capability-relative, symlink-resolved file
  identity used for cycle and duplicate-load detection.
- **Composition order:** The total order in which declarations enter the
  composed manifest.

## 5. Manifest syntax

A manifest or fragment may define a top-level `includes` sequence:

```yaml
includes:
  - path: build/netsuke/rust-quality.yaml
  - path: build/netsuke/release.yaml
```

The compact scalar form is equivalent:

```yaml
includes:
  - build/netsuke/rust-quality.yaml
  - build/netsuke/release.yaml
```

The initial mapping accepts exactly these keys:

| Field  | Type             | Default  | Meaning                                       |
| ------ | ---------------- | -------- | --------------------------------------------- |
| `path` | non-empty string | required | Fragment path relative to the including file. |
| `as`   | identifier       | absent   | Namespace the included declarations.          |

Table 1: Local include fields.

Unknown keys are errors. `path` and `as` are literal YAML strings. Jinja
expressions and structural Jinja blocks are invalid in both fields.

### 5.1 Namespaced includes

An include may place exported declaration names beneath a namespace:

```yaml
includes:
  - path: build/netsuke/rust-quality.yaml
    as: rust_quality
```

The namespace applies to named rules, targets, and actions. For example, `lint`
becomes `rust_quality::lint`. References inside the fragment are rewritten
within that namespace before cross-fragment resolution.

Fragment variables appear beneath one mapping value named by the namespace:

```jinja
{{ rust_quality.vars.clippy_flags }}
```

The exact user-facing variable projection may be refined during implementation,
but it must remain a typed namespace rather than concatenating strings into
flat variable names.

An unnamespaced include contributes declarations directly and is therefore more
likely to encounter duplicate-name errors.

## 6. Resolution algorithm

For each composition unit, Netsuke performs these steps:

1. Initialize a composition-wide `visited` map keyed by canonical include
   identity. The map retains the first include chain that loaded each identity.
2. Parse YAML sufficiently to identify `includes` without rendering Jinja.
3. For each include in declaration order:
   1. resolve `path` against the including file's parent directory;
   2. open the path through the workspace directory capability;
   3. resolve symlinks and obtain its canonical include identity;
   4. reject paths outside the effective workspace boundary;
   5. reject a canonical identity already active on the recursion stack;
   6. reject a canonical identity already present in `visited`, reporting the
      first include chain and the attempted include chain;
   7. add the identity and its current chain to `visited`;
   8. parse the fragment;
   9. recursively compose that fragment's includes; and
   10. append the fragment's declarations to the composition stream.
4. Append the including file's own declarations.
5. Validate declaration identities and references over the complete stream.
6. Evaluate manifest-time Jinja and expand `foreach` and `when` using the
   composed context.
7. Build and validate the IR.

For root `R` including `A` then `B`, where `A` includes `C`, the composition
order is:

```text
C, A, B, R
```

This order does not imply that later declarations replace earlier ones.
Duplicates remain errors.

### 6.1 Repeated includes

Including the same canonical fragment more than once is an error, even when it
would enter through different relative spellings or symlinks. The
composition-wide `visited` map catches this after the first load, even when the
first identity is no longer on the recursion stack. The duplicate-identity
diagnostic shows both the chain that first loaded the fragment and the
attempted chain that reached it again.

A future bundle RFC may permit multiple instantiated copies through distinct
namespaces and parameters. Plain includes model source decomposition, not
instantiation.

### 6.2 Cycle diagnostics

A cycle reports the complete include chain in lexical include order:

```text
Netsukefile -> build/common.yaml -> build/rust.yaml -> build/common.yaml
```

The error identifies the closing include site and the first active site for the
same canonical identity.

## 7. Deterministic merge semantics

Composition operates on typed manifest sections rather than generic YAML map
merging.

### 7.1 Named declarations

Rules, targets, and actions are keyed by their final qualified names. Two
composition units may not contribute the same final name.

The error must include:

- the duplicate qualified name;
- both source files and source spans;
- the include chains that made each declaration visible; and
- a remediation such as adding `as`, renaming one declaration, or extracting a
  parameterized bundle under RFC 0003.

### 7.2 Variables

Within one namespace, duplicate variable keys are errors. Nested mappings are
not deep-merged implicitly. A variable mapping is one declared value with one
source of truth.

This refusal is intentional. Generic deep merge requires policy for sequences,
nulls, scalar-versus-mapping conflicts, and ordering. Silent last-writer-wins
would make include reordering behaviourally significant in ways that are hard
to review.

Manifest authors may construct derived mappings explicitly with Jinja filters
such as a future deterministic `combine` helper.

### 7.3 Root-only settings

Settings that control the complete compilation, including manifest-format
selection and workspace-wide defaults, may appear only in the root manifest
unless their schema explicitly declares fragment scope.

A fragment containing a root-only setting is rejected at that fragment's source
span. It is not ignored and does not compete by composition order.

### 7.4 Sequence sections

Any schema section whose order is semantically meaningful concatenates in the
composition order defined in section 6. The schema must identify such sections
explicitly; implementations must not apply generic YAML sequence concatenation
to unknown fields.

## 8. Jinja and control-key semantics

Includes resolve before ordinary manifest-time Jinja evaluation. Include paths
therefore cannot depend on variables, the environment, the filesystem beyond
the literal path, the clock, or network helpers.

After composition:

- root and fragment variables form one typed context subject to namespace and
  duplicate rules;
- `foreach` and `when` evaluate at their existing permitted scopes;
- metadata queries use the established restricted Jinja environment; and
- recipe-only helpers remain unavailable during `help targets`.

A fragment cannot use Jinja to create another `includes` key after parsing.
Structural composition remains visible in YAML source.

## 9. Path and capability semantics

Each include is opened relative to a directory capability for the including
file. Lexical `..` segments are normalized before access, and symlinks are
resolved before the workspace-boundary check.

The initial implementation rejects:

- absolute include paths;
- paths that escape the effective workspace root;
- paths containing NUL;
- non-file objects;
- unreadable files; and
- non-UTF-8 manifest paths or content where the existing manifest contract
  requires UTF-8.

Diagnostics should distinguish missing, unreadable, outside-workspace,
symlink-escape, duplicate-load, and cycle failures.

## 10. Provenance, hashing, and generated state

Every declaration and rendered field retains:

- physical source file;
- source span;
- include chain;
- applied namespace; and
- composed declaration identity.

The compiled-manifest fingerprint includes, in composition order:

- the relative canonical identity of every composition unit;
- the complete bytes of every unit;
- namespace selections; and
- the manifest schema and compiler versions.

Touching metadata without changing bytes does not alter the fingerprint.
Changing included bytes does.

Generated Ninja and sidecar diagnostics must point to original fragment spans,
not to a synthetic merged YAML document.

## 11. Metadata and agent-facing behaviour

`netsuke help targets`, graph inspection, and JSON metadata must expose the
composed declarations while retaining their origin.

Structured output should include bounded fields such as:

```json
{
  "name": "rust_quality::lint",
  "source": "build/netsuke/rust-quality.yaml",
  "namespace": "rust_quality"
}
```

It must not expose absolute host paths when a workspace-relative path is
sufficient.

Metadata discovery resolves and parses includes but does not render or execute
recipe fields. A malformed or cyclic include graph is still a metadata error.

## 12. Compatibility and migration

Manifests without `includes` retain their current behaviour.

A monolithic manifest may migrate mechanically:

1. move one coherent declaration group into a fragment;
2. add a literal include path;
3. add a namespace if names would collide;
4. update external references to qualified names; and
5. compare graph and help snapshots before removing the original declarations.

Because duplicate declarations are errors, migration cannot accidentally leave
both the copied and extracted version active.

## 13. Implementation plan

### Phase 1: composition AST

- Add literal `includes` syntax and deny unknown fields.
- Retain source-file and source-span provenance for every parsed unit.
- Implement capability-relative path resolution and canonical identity.

### Phase 2: resolver and cycle validation

- Implement deterministic depth-first post-order traversal.
- Add active-stack cycle detection and repeated-identity rejection.
- Add namespace qualification and internal-reference rewriting.

### Phase 3: typed merge

- Compose schema sections explicitly.
- Reject duplicate variables and named declarations with dual-source
  diagnostics.
- Enforce root-only settings.

### Phase 4: compiler and metadata integration

- Feed the composed AST into existing Jinja, `foreach`, `when`, IR, help, graph,
  and Ninja generation stages.
- Include all fragments and namespaces in graph fingerprints.

### Phase 5: documentation and migration canaries

- Add user and developer guidance.
- Split a representative downstream Netsukefile into local fragments and prove
  graph equivalence.

## 14. Test strategy

The implementation must include:

- relative-path resolution from nested directories;
- root, nested, and sibling includes;
- deterministic order snapshots;
- duplicate declaration and variable diagnostics;
- namespace qualification and internal-reference tests;
- lexical and symlink workspace escapes;
- direct, indirect, and symlink-mediated cycles;
- repeated includes through different spellings;
- metadata queries that do not evaluate recipes;
- source-span diagnostics from included files;
- graph-fingerprint changes for byte changes but not metadata-only changes;
- property tests generating bounded acyclic and cyclic include graphs; and
- Windows path and case-behaviour tests.

A Kani harness may check the bounded traversal state machine and the invariant
that every emitted composition unit appears exactly once.

## 15. Alternatives considered

### 15.1 YAML merge keys

YAML anchors and merge keys operate inside one parsed document, have awkward
cross-file semantics, and do not preserve Netsuke declaration identities or
include provenance. Rejected.

### 15.2 Jinja include/import

Template-level includes evaluate too late, mix source composition with value
rendering, and could make metadata queries execute build-time helpers. Rejected
for manifest structure.

### 15.3 Last-writer-wins map merging

This is compact but makes include order an implicit override language and hides
stale duplicate declarations. Rejected for the initial surface.

### 15.4 Globbed includes

Directory iteration and broad glob capability complicate order, review,
provenance, and injection boundaries. Explicit literal paths are adequate for
source decomposition. Rejected.

## 16. Open questions

- Should fragments use a distinct top-level `kind: fragment` marker, or is being
  reached through `includes` sufficient?
- Should an include namespace also qualify pools and future provider names?
- Should tooling offer a formatter-assisted extraction command?
- Which root settings, if any, should later gain explicit fragment scope?

These questions may refine the schema but must not weaken literal path
resolution, duplicate rejection, or deterministic composition order.

## 17. Recommendation

Adopt literal, repository-relative includes with depth-first post-order
composition, workspace capability confinement, retained provenance, optional
namespaces, and duplicate rejection.

This is enough to split large Netsukefiles safely. It intentionally leaves
version selection, parameterization, and network trust to later RFCs rather
than smuggling a package manager into an `include` key.
