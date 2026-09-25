# RFC 0029: First-class host facts

## Preamble

- **RFC number:** 0029
- **Status:** Proposed
- **Created:** 2026-09-25
- **Scope:** Immutable, typed facts about Netsuke's planning environment.
- **Prior art:** Ansible fact gathering, with explicit Netsuke dispositions.
- **Release disposition:** Post-v0.1.0. Acceptance does not add a release gate.

### Number allocation and related proposals

RFCs 0013 to 0020 are reserved by PR #697; RFCs 0021 to 0025 already appear
on `main`; PR #747 reserves RFCs 0026 to 0028. This proposal takes 0029 above
those allocations. The allocation check found no existing host-facts PR.

This RFC complements [RFC 0006][stdlib-rfc], rather than adding another general
filter library. It integrates with [structured commands][commands-rfc],
[explicit shell selection][shell-rfc], [local includes][includes-rfc],
[local bundles][bundles-rfc], [manifest testing][testing-rfc], and
[optional typed inputs][inputs-rfc]. None of those proposals must be fully
implemented before a small platform-facts provider becomes useful.

## 1. Summary

Introduce an immutable `host` object for YAML and Jinja build manifests:

```yaml
host_facts:
  schema: 1

vars:
  test_program: "fixture-test{{ host.exe_suffix }}"

# Excerpt: the schema/version declaration and other manifest sections are omitted.
actions:
  - name: check-posix
    when: "{{ host.family == 'unix' }}"
    command: cargo test --all-targets
  - name: check-windows
    when: "{{ host.os == 'windows' }}"
    command: cargo test --all-targets
```

The declaration opts into a new reserved namespace without breaking an existing
manifest that happens to define a variable called `host`. With no `gather`
selection, it requests only the inexpensive `platform` group.

The initial schema defines three groups: `platform`, `distribution`, and
`kernel`. The latter two require explicit selection and operator permission.
The small platform slice can ship first; implementations reject unimplemented
groups instead of pretending to have gathered them.

Facts describe the environment in which the Netsuke planner process operates.
They do not describe the machine that built Netsuke, a cross-compilation output,
a container entered by a recipe, or an eventual remote executor.

The central contract is:

> Collect once through explicit providers, normalize once, then render against
> an immutable snapshot. Facts describe observations; they neither grant
> capabilities nor select execution policy.

Ordinary metadata discovery remains non-disclosing. Host-aware target discovery
requires explicit caller consent. No fact collector launches a shell, performs
a network request, reads the process environment, or executes local fact files.

All syntax and command additions below are proposals, not descriptions of
currently shipped functionality.

## 2. Problem and current boundaries

Replacing Makefiles currently leaves platform decisions in shell probes such
as `uname`, environment conventions such as `OS=Windows_NT`, or repeated Jinja
branches with repository-specific names. Netsuke's own Makefile uses
`BUILD_HOST_OS := $(shell uname -s)` to choose its Linux linker flag.

A first-class value model makes those decisions discoverable and testable.
However, an unrestricted machine inventory would introduce more problems than
it solves: hidden I/O, identifying data, stale observations, graph changes
caused by load, and incorrect cross-compilation decisions.

Several existing boundaries constrain the design:

- `src/stdlib/register.rs::register_manifest_query` deliberately rejects host
  inspection during target discovery. Read-only I/O is not automatically
  non-disclosing I/O.
- `env()` and `command_available()` already have their own authority and
  resolution rules. Host facts must not become a bypass for either helper.
- [ADR-026][env-adr] owns exact-name manifest environment policy.
- [ADR-008][env-seam-adr] requires explicit seams instead of ambient environment
  mutation in tests.
- [RFC 0025][maturity-rfc] preserves unannotated manifests as a supported use
  case. Facts must not become a compulsory annotation layer.

This RFC proposes one additional observation capability, not an inventory
manager, package manager, scheduler, or alternative configuration system.

## 3. Ansible prior art and disposition

Ansible exposes gathered system information through `ansible_facts`, supports
its use in Jinja and conditions, and can also inject prefixed top-level
variables. Its normal memory cache lasts for the playbook run; persistent cache
plugins extend that lifetime. The documentation warns that gathered time facts
can become stale.[^1]

`ansible.builtin.setup` distinguishes collection selection (`gather_subset`)
from output filtering (`filter`) and provides a gathering timeout. Its local
fact mechanism can execute fact files.[^2] `gather_facts` also documents that
parallel providers can merge conflicting keys in an unspecified order.[^3]
Ansible's `set_fact` creates mutable host variables and optionally cacheable
facts.[^4]

| Ansible precedent | Netsuke disposition |
| --- | --- |
| Namespaced facts in templates and conditions | Adopt one typed, immutable `host` object. |
| Prefixed top-level fact aliases | Reject; avoid shadowing and two spellings per field. |
| Selective collection versus output filtering | Adopt the distinction; filtering never authorizes collection. |
| Per-run memory cache | Adopt one invocation snapshot, not hidden persistent discovery. |
| Distribution and operating-system families | Adapt; platform family and distribution lineage are distinct. |
| Local fact files and executable collectors | Defer custom data; reject executable discovery in this RFC. |
| Mutable `set_fact` and registered results | Keep task inputs and runtime bindings separate from observations. |
| Parallel last-merged conflict resolution | Reject; each typed field has exactly one owning collector. |

_Table 1: Ansible ideas adopted, adapted, or deliberately excluded._

Ansible's `os_family` can denote a distribution family such as `RedHat`. In
this proposal `host.family` means `unix`, `windows`, or `other`; distribution
lineage appears separately under `host.distribution.id_like`. The names are
not a compatibility promise. Ansible's architecture and userspace facts also
motivate separating process ABI from native-machine observations.[^1]

Netsuke requires neither Ansible nor Python to collect these facts. This RFC
uses Ansible's model as prior art, not its inventory schema as an interchange
format.

## 4. What host means

### 4.1. Planner process, native machine, and build target

`host.os` and `host.arch` describe the operating-system interface and processor
architecture for which the running Netsuke process was compiled. They remain
meaningful under a compatibility layer, but do not claim physical hardware
identity. The platform collector uses the running binary's Rust target
constants and configuration, not environment variables from a parent build.
Rust documents these constants and target configuration separately.[^5][^6]

In particular, `host.arch` is the **planner process ABI architecture**. An
x86-64 Netsuke running through translation on an ARM machine still reports
`x86_64`. A native-machine observation, when reliable, belongs in the separately
requested `host.kernel.native_arch` field.

The `HOST` and `TARGET` variables used by Cargo build scripts refer to a
particular Cargo invocation.[^7] A manifest can pass an explicit Cargo target,
but doing so must not change Netsuke's `host` object.

### 4.2. Containers, compatibility layers, and shells

A Linux Netsuke inside a container reports its Linux process platform. Its
optional distribution facts come from that container's filesystem view; its
kernel facts may describe a shared kernel. Netsuke does not claim this reveals
the enclosing physical host or container image digest.

A Linux Netsuke in Windows Subsystem for Linux (WSL) reports `linux`. A Windows
Netsuke launched from Git Bash reports `windows`. The launching shell does not
change its process ABI, path separator, or executable suffix. A different
runtime under emulation can expose different optional observations; provenance
must preserve that distinction rather than infer a physical platform.

Facts do not identify the selected recipe interpreter. Shell choice remains
owned by RFC 0011. `host.family == 'unix'` does not prove that Bash exists;
`host.os == 'windows'` does not prove that PowerShell or Git Bash is configured.
Tool presence remains a resolver or explicit state/probe concern.

### 4.3. Remote execution and cross-compilation

The initial scope is local planning. A command invoking SSH, a container
engine, an emulator, or a cross-compiler receives no implicit remote facts.
Such contexts need explicit parameters or a later execution-context design.

Artefact names and compiler targets must derive from declared target inputs,
not from `host.arch`, unless the action intentionally builds for its planner
process platform. Observations never select a target triple automatically.

## 5. Manifest opt-in, authority, and compatibility

### 5.1. Root declaration

```yaml
host_facts:
  schema: 1
  gather:
    - platform
    - distribution
```

The root mapping accepts only `schema` and `gather`. `schema` is required and
must equal a supported integer schema version. `gather` defaults to
`[platform]`; otherwise it must be a non-empty, duplicate-free sequence of
literal group names containing `platform`. No wildcard, negation syntax,
Jinja, or environment interpolation is permitted in this declaration.

The group vocabulary for this RFC is closed. Later groups require a schema
extension and explicit selection; `all` is deliberately absent so an upgrade
cannot silently broaden collection. Unknown keys, versions, and groups fail
before any provider runs.

This opt-in requires an additive manifest-format version allocated at
implementation time. Examples omit a speculative `netsuke_version` value.
Older compilers must reject the new field clearly, not ignore it.

### 5.2. Authority ceiling

The project requests groups; it cannot grant collection authority. Trusted
operator configuration and explicit command-line choices establish an
allow-list using the existing configuration-provenance machinery.

The default grant permits only `platform` during an opted-in build. A request
for `distribution` or `kernel` without a trusted grant fails before I/O, with
the denied group and remediation in the diagnostic. Operators may also deny
platform disclosure. Denials take precedence over project requests.

The concrete policy key and command-line option must use the shared
OrthoConfig metadata path, not a separate parser or file format. This RFC
proposes the policy name `host_fact_policy.allowed_groups`; it does not assign
an environment-variable override that could silently spoof fact values.

### 5.3. Immutable namespace

With opt-in, `host` is reserved throughout manifest evaluation. Root variables,
per-entry variables, loop bindings, macro parameters, imports, and local Jinja
assignments must not shadow it. Validation covers template binding sites, not
only `vars.host`. The projection also rejects item and attribute mutation.

Without opt-in, existing user variables called `host` remain ordinary variables.
No warning, fact collection, or migration requirement applies. The unchanged
quickstart must remain an acceptance fixture.

## 6. Fact schema

### 6.1. Platform group

The platform group uses no filesystem, registry, environment, subprocess, or
network access. Its fields are never inferred from filenames or shell output.

| Field | Type | Contract |
| --- | --- | --- |
| `host.os` | string | Rust-style planner OS identifier: for example `linux`, `windows`, `macos`, or `freebsd`. |
| `host.family` | string | `windows`, `unix`, or `other`, with no distribution-family meaning. |
| `host.arch` | string | Planner process ABI architecture: for example `x86`, `x86_64`, `arm`, or `aarch64`. |
| `host.pointer_width` | integer | Pointer width in bits for the planner process ABI. |
| `host.endianness` | string | `little` or `big` for the planner process ABI. |
| `host.exe_suffix` | string | Platform executable suffix including its dot, or an empty string. |
| `host.path_separator` | string | The native process path-component separator, not a shell escape. |
| `host.path_list_separator` | string | Native environment path-list delimiter: `;` for Windows, `:` for Unix. |

_Table 2: Initial platform facts._

The provider uses `std::env::consts`, target configuration, and native lexical
path conventions.[^5][^6] The schema pins those meanings, not a heuristic alias
map derived from `uname`. For example, `macos` is not `Darwin`, and `aarch64`
is not the vendor spelling `arm64`.

New supported OS and architecture identifiers are additive values, not reasons
to guess `linux` or `x86_64`. A provider unable to supply a required field fails
with `unsupported_host_platform`. A manifest requiring a closed supported set
must explicitly reject other values instead of treating its final `else` branch
as a universal Windows or Unix case.

The executable suffix does not enumerate `PATHEXT`, guarantee executability, or
select an output suffix for a cross-compilation target. A path separator does
not prove case sensitivity, Unicode normalization, symlink support, or the
filesystem semantics of a mounted volume. Use lexical path helpers instead of
manual concatenation whenever the relevant helper exists.

### 6.2. Distribution group

The first distribution provider supports the Linux `os-release` interface:

| Field | Type | Contract |
| --- | --- | --- |
| `host.distribution.id` | string or null | Validated `ID`, without an invented fallback. |
| `host.distribution.id_like` | sequence of strings or null | Ordered `ID_LIKE` identifiers. Empty means an observed file omitted it; null means no usable observation. |
| `host.distribution.version_id` | string or null | `VERSION_ID` as opaque version text, not a SemVer assertion. |
| `host.distribution.name` | string or null | Bounded display name from `NAME`; not a command or path. |

_Table 3: Optional distribution facts._

Read `/etc/os-release` first and use `/usr/lib/os-release` only when the former
is missing. Do not merge them. The format specifies assignments without
variable expansion; duplicate keys use the later entry with a bounded warning.
A parser must implement these data rules, never source the file in a shell.[^8]

The provider opens only the two approved logical paths through its own narrow
system-file capability. It resolves at most eight symlink hops, rejects cycles,
and validates the final opened handle as a regular file. Relative system
symlinks remain valid; no directory traversal or arbitrary caller-selected path
is added to the manifest API. A missing final target counts as missing, but
permission, encoding, malformed-data, and limit failures do not authorize a
fallback to another identity source.

Read at most 64 KiB plus one overflow-detection byte. Bound each exposed string
to 256 UTF-8 bytes and `id_like` to 16 identifiers. Reject NUL and control
characters in exposed fields. Validate identifier grammar, preserve declared
lineage order, and ignore unrecognized fields without exposing their values.
Unknown assignments still have to be syntactically valid data.

On non-Linux platforms the group reports `unsupported` and all its fields are
null. No Windows registry crawl, macOS command invocation, or opportunistic
`lsb_release` fallback occurs. Lack of a distribution provider does not make
platform facts unavailable.

Distribution identity does not prove that a package manager, service manager,
compiler, or shared library exists. No `host.package_manager` inference enters
the initial schema.

### 6.3. Kernel group

The optional kernel provider returns `host.kernel.name`,
`host.kernel.release`, and `host.kernel.native_arch`, each a string or null.
OS-native, non-executing APIs supply these fields through a reviewed safe
adapter. The initial Unix adapter reads the equivalent of `uname` fields
without executing `uname`; the Windows adapter must use documented APIs rather
than parsing `ver` or PowerShell output.

`native_arch` is populated only when the adapter can establish its documented
meaning under translation. Otherwise it is null. It is not an alias for
`host.arch` or an unexamined copy of a kernel machine string. Windows APIs such
as `IsWow64Process2` explicitly distinguish process and native architectures;
that distinction illustrates why one unqualified architecture field is
insufficient.[^9]

Each exposed string has the same 256-byte bound and encoding rules as the
distribution group. Kernel release text is opaque; no SemVer conversion or
lexicographic version ordering is implied. Unsupported collection reports null
fields with a reason, not fabricated values.

Platform, kernel, and distribution may disagree legitimately in compatibility
or container contexts. Preserve each field's provider scope. Do not combine
them into a fictitious universal host identity.

### 6.4. Missing, denied, and invalid are different

For optional groups, all schema fields exist once the group is selected.
Field absence in a valid observation becomes null where the table permits it.
Whole-group outcomes are `collected`, `partial`, `absent`, or `unsupported`.
A denied group, malformed data, I/O failure, or exceeded bound is an error,
not `absent`, `false`, zero, or an empty mapping.

Accessing a known group not selected by `gather` produces
`host_fact_not_collected` without invoking a collector. Accessing an unknown
field produces `unknown_host_fact`, including in a `default` expression, so a
typo does not become a silent platform decision. Optional null fields can use
an explicit `is not none` check.

The serialized inspection envelope carries group outcomes and fixed provider
identifiers separately from the `host` values. Outcomes and semantic values
are hashable inputs; operational timing and detailed error strings are not.

## 7. Collection and evaluation lifecycle

### 7.1. One snapshot per invocation

1. Parse the literal opt-in and gather request.
2. Validate names, schema, namespace bindings, and the operator ceiling.
3. Construct the selected providers at the application boundary.
4. Collect groups once in canonical order: platform, distribution, kernel.
5. Validate and freeze one `HostFactsSnapshot`.
6. Inject a read-only projection into each relevant template environment.
7. Reuse that snapshot for expansion, validation, plan generation, and output.

Jinja field access performs no I/O and cannot gather another group. Repeated
`when`, variable, and recipe evaluations observe the same values. There is no
refresh operation midway through a build and no fact task in the Ninja graph.

This is a stable observation within an invocation, not an atomic snapshot of
the entire operating system. A changed file during collection either yields a
validated bounded observation or a collection error; it does not trigger an
unbounded retry loop. A subsequent invocation collects again.

The implementation bounds input sizes and API calls. It must not claim that a
thread timeout can cancel an arbitrary blocking kernel call. Collectors with
uncancellable, potentially unbounded discovery are outside the initial design;
a later collector needs an explicit cancellation and latency analysis.

### 7.2. Rendering and execution trust

Facts may appear in ordinary value templates and `when` expressions where the
existing schema permits them. They do not add another conditional language.
They cannot choose an include path, bundle URL, Git ref, or other field whose
own RFC requires a literal value.

A fact is data, never pre-quoted shell source or a filesystem capability. Values
from system files keep untrusted-data status through rendering. Direct
structured invocation retains typed argument boundaries. Legacy shell recipes
still require the existing quoting and interpolation protections; registering
facts must not mark them as safe shell fragments.

Changing `cwd`, capturing stdout, or overlaying a child's environment under
RFCs 0009 and 0010 cannot change the frozen planner facts. A child has no
`set_fact` channel back into planning.

### 7.3. Includes and bundles

All local fragments share the importing root's snapshot. An included file may
not declare another collector request. A bundle can declare a static requirement
for an already selected group in its compatibility metadata, but cannot widen
the root request or operator grant. Missing required facts fail before bundle
rendering.

The bundle observes the importing planner, not the bundle publisher or a remote
Git server. A bundle needing artefact target facts must accept explicit typed
target parameters. Bundle provenance and host provenance remain separate.

## 8. Metadata, inspection, and privacy

### 8.1. Preserve ordinary target discovery

`netsuke help targets` keeps its current non-disclosing default. It neither
collects facts nor renders recipes. For an opted-in manifest it installs an
unavailable host sentinel. If discovery evaluates a `host` reference in
metadata, variables needed for metadata, or a `when` condition, it returns a
typed `host_facts_disabled_in_query` diagnostic. It must not guess a platform,
silently hide targets, or report an incomplete list as complete.

The proposed explicit form is:

```bash
netsuke help targets --host-facts
```

That flag consents to host-aware discovery for this invocation only. It gathers
exactly the manifest's requested groups within the operator ceiling, reuses the
same provider implementation as planning, and still does not render recipes or
permit `env()`, `which()`, command execution, or network helpers. A project
setting cannot enable the flag on the caller's behalf.

This proposal deliberately accepts an actionable discovery error without
consent instead of adding symbolic evaluation of every host-dependent condition.
A future static listing of unresolved conditions requires its own contract.

### 8.2. Standalone inspection

Propose a read-only `facts` command integrated with the canonical command tree:

```bash
netsuke facts
netsuke --json facts --gather platform --filter os --filter arch
```

It requires no Netsukefile and does not parse a project manifest, acquire
bundles, or run targets. It consults trusted operator configuration only, not
project-local grants. Default selection is `platform`; `--gather` is an exact,
closed group selection. Repeated exact field filters select output, never
collection or authority. Unknown filters are errors.

The JSON envelope contains `schema_version`, `scope: planner_process`,
`collected_groups`, `facts`, group outcomes, and bounded provider identifiers.
Human output has one field per line and escapes control characters. Empty
values, null values, and unsupported groups remain visibly distinct.

A schema-description option must return the static field catalogue without
collecting any facts. Inspection and schema output use the existing CLI
metadata, localization, and JSON conventions, not hand-built parallel output.

### 8.3. Data minimization

The initial catalogue excludes hostnames, addresses, network interfaces,
usernames, home directories, machine IDs, serial numbers, environment dumps,
SSH keys, package inventories, timestamps, uptime, and filesystem inventories.
Raw provider documents never appear in diagnostics.

Observation provenance is not attestation. A container image or compromised
host can supply misleading system data. A content hash proves equality, not
truth, publisher identity, or authorization.

Telemetry uses only registered group names, provider categories, and outcome
categories. OS release strings, architectures from unknown providers, paths,
and full fact digests must not become unbounded metric labels.

## 9. Reproducibility, planning, and caching

### 9.1. Explicit planning input

Opted-in host facts are environmental planning inputs. The same manifest,
snapshot, and other declared planning inputs must compile identically; different
planners need not generate the same graph. This does not make a build hermetic
merely because collection uses no shell.

Compute a versioned digest from the fact schema, selected groups, normalized
values, and semantic group outcomes. Sort mapping keys, preserve meaningful
sequence order, and distinguish null from an empty value. Exclude timestamps,
collection durations, absolute source paths, localization, and diagnostic text.

The initial implementation fingerprints every selected group, not only fields
observed through dynamic Jinja lookup. This conservatively invalidates plans
when an unused selected fact changes. Dependency tracking may optimize it
later; correctness cannot depend on an incomplete textual scan for `host`.

### 9.2. Avoid stale Ninja work

Include the facts digest in Netsuke's generated-plan identity and per-edge
command signature using the existing backend-safe signature mechanism. Merely
writing it to a Ninja comment is insufficient: a changed fact must invalidate
relevant generated plans and affected work even when a textual command happens
to remain unchanged. A backend without an appropriate signature path needs a
versioned implicit input or equivalent tested invalidation mechanism.

Tests must prove unchanged snapshots remain a no-op, changed snapshots rebuild,
and removing a host-selected edge removes it from the regenerated graph. Facts
do not replace ordinary source-file dependency declarations.

No hidden persistent fact cache ships initially. Serialized snapshots are
provenance or test fixtures, not authoritative input for ordinary builds.

### 9.3. Planning host versus execution host

Internally generated immediate execution uses the frozen snapshot. Exported or
reused plans record their host-facts scope and semantic digest. Netsuke must
verify that context before executing a reused fact-dependent plan; mismatch
requires regeneration. It must not silently re-plan under new facts after
some edges have already run.

Native Ninja execution of an exported file bypasses Netsuke's revalidation.
Document that limitation and mark the export's planner context; do not claim a
security or reproducibility guarantee for that bypass. A future remote executor
needs its own platform contract, not a fake override of local `host`.

## 10. Resource and capability facts deliberately deferred

CPU topology, available parallelism, memory capacity, container quotas, and
current load answer different questions. Rust documents limitations in
`available_parallelism`, including platform and resource-limit effects.[^10]
Neither a processor count nor a memory total establishes available build budget.

The initial schema therefore does not expose `host.jobs`, free memory, or load.
It does not alter `-j`, child build flags, Ninja pools, or
[RFC 0024 contention classes][contention-rfc]. No new scheduler is required.

A later resource group must separate observed physical resources, effective
process constraints, and operator-approved budgets, define units and unknown
states, and decide which values are forbidden in semantic graph selection.
Volatile load should not silently invalidate all build artefacts.

Likewise, filesystem features, libc compatibility, executable presence,
virtualization identity, and network reachability need explicit independent
probes or contracts. None is inferred from `host.os` or distribution lineage.

## 11. Provider architecture and testing seams

The application owns collection; the manifest model owns schema and validation;
stdlib registration exposes only immutable values. Platform-specific APIs live
behind narrow feature-owned adapters, not inside Jinja functions or CLI schema
modules.

Add a `HostFactsProvider` seam returning a typed snapshot for a validated
request and grant. Its platform, distribution, and kernel adapters own disjoint
fields. A duplicate field contribution is an internal contract error, never a
last-writer-wins merge. Existing configuration and diagnostic types should be
reused rather than wrapped in a universal system-services abstraction.

Production wiring enters through `StdlibConfig` or the manifest evaluation
context that already transports explicit environment and clock seams. Every
render environment in one invocation borrows or shares the same snapshot.
`register_manifest_query` receives either the unavailable sentinel or the
explicitly authorized immutable projection.

Tests supply a `FixedHostFactsProvider` with no fallback to real host APIs.
Selecting an unavailable fixture group fails rather than reading the developer's
machine. No test calls `set_var`, changes a global cwd, executes `uname`, or
requires serialization to fake a platform.

Manifest tests under RFC 0007 should gain a typed host fixture through its
existing mock registry. Schema validation applies to fixtures too. Synthetic
facts may render and inspect a graph but must not authorize actual execution
for another platform or widen file/network capabilities. No ordinary build
flag permitting arbitrary host-value overrides ships with this RFC.

Implementation must preserve the workspace's unsafe-code prohibition. Select a
reviewed safe platform adapter or explicitly propose a dependency change; do not
attempt to override `forbid(unsafe_code)` with an expectation around FFI.

## 12. Diagnostics and compatibility rules

Stable diagnostic categories include `unsupported_host_fact_schema`,
`unknown_host_fact_group`, `host_fact_collection_denied`,
`host_namespace_shadowed`, `unknown_host_fact`, `host_fact_not_collected`,
`host_facts_disabled_in_query`, `host_fact_source_invalid`,
`host_fact_limit_exceeded`, and `host_plan_context_mismatch`.

Report the group, field, request origin, and relevant manifest span without
printing arbitrary system-file values. Optional source absence is an explicit
status, not an error log that leaks a path. Invalid encoding and I/O faults keep
their bounded error category without dumping source bytes.

Adding an optional field is an additive schema change. Changing an existing
field's meaning, type, normalization, default collection scope, or authority
requires a schema revision. New providers cannot silently broaden a previously
unsupported group's behaviour without a documented compatibility review.

The RFC does not accept an ADR, implement a collector, close a release task,
change current shell selection, or require migration of existing manifests.

## 13. Validation and delivery plan

### 13.1. Delivery slices

1. Accept the field meanings, opt-in, privacy contract, and schema allocation.
   Define the owned snapshot, grant, and diagnostic interfaces.
2. Implement the platform-only slice, immutable projection, namespace checks,
   explicit inspection, query consent, and fingerprint integration together.
3. Add separately authorized distribution and kernel providers with bounded
   parsing, native adapters, provenance, and unavailable-state tests.
4. Integrate typed fixtures with RFC 0007 and publish Linux, macOS, and Windows
   migration examples. Preserve the existing quickstart byte-for-byte.

The first usable slice must not depend on external Git bundles, remote
execution, a resource scheduler, or the complete RFC 0006 filter programme.
Optional collector delivery is not a new v0.1.0 admission requirement. Release
assignment belongs to review rather than this document silently changing #593
or #597.

### 13.2. Required tests

- Fixed Linux, macOS, Windows, and other-platform snapshots render identical
  expected results on every test runner. Integer widths stay integers.
- Process architecture differs from native architecture in translated fixtures;
  explicit build targets never mutate planner facts.
- Git Bash launching a Windows binary and WSL launching a Linux binary retain
  their respective process facts. Native smoke tests complement fixtures.
- Every name-binding site rejects `host` shadowing under opt-in. Without opt-in,
  an existing `vars.host` fixture still renders unchanged.
- Providers run once per requested group. Reading many fields, conditions,
  includes, and bundles never causes another collection call.
- Denied and unselected groups perform zero native or filesystem operations.
  Output filtering cannot widen or hide a collection request.
- Query mode never collects without caller consent and never executes recipes
  with consent. A recipe-only fact reference remains unevaluated by discovery.
- The distribution parser handles quotes, escapes, missing optional values,
  duplicate-key precedence, invalid UTF-8, NUL, oversized data, symlink cycles,
  and the standard relative `/etc/os-release` symlink. It never executes text.
- Kernel tests distinguish reliable native architecture from unknown and never
  copy process architecture merely to fill a missing native observation.
- Field access distinguishes null, empty, unknown, and not collected. Metadata
  produces a clear error rather than silently dropping conditioned targets.
- Snapshot hashing is deterministic across map insertion order, excludes
  operational metadata, and changes for altered semantic facts or outcomes.
- Real Ninja integration proves cache invalidation, no-op reuse, removal of
  previously selected edges, and rejection of incompatible reused plans.
- Child environment overlays, secure tempdirs, and captured runtime bindings
  cannot mutate facts or manufacture privileges through their values.
- JSON, help, diagnostics, and telemetry avoid environment values, identifying
  inventory, raw system files, and unbounded metric labels.

Property tests should generate bounded snapshots and requests, checking
normalization idempotence, deterministic digests, strict request/grant
intersection, and preservation of null versus empty values. Parser fuzzing
should use a finite seed corpus and explicit resource limits. A bounded model
check may verify the collect-once and query-consent state machine; it must use
real production validation functions rather than a copied toy implementation.

### 13.3. Downstream acceptance examples

Use the already proposed canaries, without modifying their release ownership:

- Repovec Appliance: replace a platform branch with `host.os` while keeping
  workspace lint scope and explicit build budgets unchanged.
- MXD: use host facts only for local process mechanics; retain PostgreSQL,
  SQLite, wireframe, and artefact target selection as explicit inputs.
- OrthoConfig: render Windows and Unix variants with fixed facts, then execute
  native smoke jobs proving path and interpreter behaviour independently.

The acceptance evidence must separate successful fixture rendering from native
execution. A simulated Windows snapshot on Linux is not a Windows smoke test.

## 14. Alternatives and open decisions

### 14.1. Shell or environment probes everywhere

Keeping `uname`, `OS`, and `PROCESSOR_ARCHITECTURE` in each manifest avoids new
schema but duplicates policy, complicates tests, and couples graph evaluation
to ambient process state. It remains a legacy escape hatch, not the proposed
first-class interface.

### 14.2. Copy the complete Ansible fact inventory

This introduces identifiers, mutable observations, executable plugins, and
external dependencies unrelated to build-platform selection. Borrow the
namespace and subset model, not an inventory service or a `set_fact` task.

### 14.3. Automatically inject facts into every template

This would collide with existing `host` variables and weaken metadata privacy.
Explicit opt-in and caller-authorized discovery make both changes reviewable.

### 14.4. Treat process facts as physical-host or target facts

This is concise but wrong under cross-compilation and translation. The RFC
chooses explicit process-ABI semantics and separately scoped native observation.
Review may prefer a longer field name such as `process_arch`; any rename must
retain the distinction before acceptance.

### 14.5. Lazily gather on attribute access or cache globally

Lazy I/O makes cost and authority depend on control flow. Global caches leak
between embedders and become stale. A fresh injected invocation snapshot has a
smaller contract. Persistent reuse remains a future explicit feature.

### 14.6. Remaining review decisions

Review should settle the final public spelling of the inspection command and
query-consent flag, the platform naming catalogue for less common targets, and
the safe native adapter dependencies. Those choices do not alter the required
privacy default, immutable snapshot, scope definitions, or operator ceiling.

## 15. Recommendation

Adopt an opt-in `host` namespace with a small platform baseline and separately
authorized distribution and kernel groups. Use Ansible's successful model of
facts as named data while keeping acquisition explicit, bounded, and separate
from task results and execution policy.

Ship the inexpensive platform slice with its testing, metadata-consent, and
cache-invalidation contracts. Add richer observations only where their scope,
privacy, latency, and portability can be stated precisely.

## References

External references were consulted on 2026-09-25. They are prior art or provider
contracts, not executable inputs to a Netsuke build.

[^1]: Ansible, [Discovering variables: facts and magic variables](https://docs.ansible.com/projects/ansible/latest/playbook_guide/playbooks_vars_facts.html).
[^2]: Ansible, [ansible.builtin.setup](https://docs.ansible.com/projects/ansible/latest/collections/ansible/builtin/setup_module.html).
[^3]: Ansible, [ansible.builtin.gather_facts](https://docs.ansible.com/projects/ansible/latest/collections/ansible/builtin/gather_facts_module.html).
[^4]: Ansible, [ansible.builtin.set_fact](https://docs.ansible.com/projects/ansible/latest/collections/ansible/builtin/set_fact_module.html).
[^5]: Rust, [std::env::consts](https://doc.rust-lang.org/std/env/consts/index.html).
[^6]: Rust Reference, [Conditional compilation](https://doc.rust-lang.org/reference/conditional-compilation.html).
[^7]: Cargo Book, [Environment variables](https://doc.rust-lang.org/cargo/reference/environment-variables.html).
[^8]: systemd, [os-release manual](https://www.man7.org/linux/man-pages/man5/os-release.5.html), upstream documentation reproduced by man7.org.
[^9]: Microsoft, [IsWow64Process2](https://learn.microsoft.com/en-us/windows/win32/api/wow64apiset/nf-wow64apiset-iswow64process2).
[^10]: Rust, [available_parallelism](https://doc.rust-lang.org/std/thread/fn.available_parallelism.html).

[stdlib-rfc]: 0006-ansible-inspired-template-standard-library.md
[commands-rfc]: 0001-structured-command-blocks.md
[shell-rfc]: 0011-allow-listed-structured-command-shells.md
[includes-rfc]: 0002-repository-relative-includes.md
[bundles-rfc]: 0003-versioned-local-bundles.md
[testing-rfc]: 0007-netsukefile-testing-framework.md
[inputs-rfc]: 0022-typed-task-inputs.md
[maturity-rfc]: 0025-progressive-enhancement-and-maturity-policies.md
[contention-rfc]: 0024-named-contention-classes.md
[env-adr]: ../adr-026-manifest-environment-access-policy.md
[env-seam-adr]: ../adr-008-environment-seam-taxonomy.md
