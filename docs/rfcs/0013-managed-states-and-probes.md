# RFC 0013: Managed states and functional probes

## Preamble

- **RFC number:** 0013
- **Status:** Proposed
- **Created:** 2026-09-19
- **Scope:** Optional preparation contracts, not a new build scheduler
- **Implementation:** [Progressive-enhancement roadmap, phase 23][roadmap]

## 1. Summary

Add optional named states for things that must be prepared before a command
runs. A state describes a condition, the evidence needed to check it, and an
optional preparation recipe. Built-in probes serve the normal case; external
functional checks support project-specific conditions without a provider plugin
or a Nagios installation.

A plain `command: uv sync` remains valid. Adding a state is worthwhile only
when an author needs an explicit precondition, validated reuse, or a shared
preparation contract. No context, typed input, ownership declaration, maturity
policy, or bundle is compulsory. [RFC 0017][maturity] owns this shallow-end
compatibility requirement.

## 2. Problem and existing boundaries

A directory timestamp cannot establish that a virtual environment contains the
requested interpreter and packages. A successful previous installation cannot
establish that an external service still works. Conversely, a test that
requires an already-installed extension must not silently build it.

[RFC 0001][commands] owns structured execution; its working-directory,
temporary resource, and shell amendments remain authoritative. This RFC adds
execution units to that runner, not a template-time subprocess facility. Cargo,
uv, and other ecosystem tools retain dependency resolution and incremental
compilation.

## 3. Progressive authoring

The following proposed fragment uses a built-in presence probe. It promises
only that `build` is a directory, not that its contents form a valid build:

```yaml
states:
  build-directory:
    kind: directory
    path: build
    prepare:
      invoke: python -c "from pathlib import Path; Path('build').mkdir(exist_ok=True)"

actions:
  - name: prepare-directory
    command:
      ensure_state: build-directory
```

`kind` selects a documented built-in probe by default. There is no mandatory
`probe: builtin` boilerplate. Initial kinds are `directory`, `file`, and
`python-venv`. The first two check the declared object type without following
symlinks. They make no content-freshness claim. `python-venv` checks the actual
interpreter, its environment prefix, and any declared interpreter constraint;
it must not advertise package-set verification that it does not implement.

A richer proposed fragment keeps a functional check separate from preparation:

```yaml
states:
  dev-env:
    kind: python-venv
    path: .venv
    identity:
      files: [pyproject.toml, uv.lock, tools/check_dev_env.py]
    prepare:
      invoke: uv sync --locked --group dev
    probe:
      external:
        invoke: python tools/check_dev_env.py
      protocol: nagios

actions:
  - name: test
    command:
      - ensure_state: dev-env
      - invoke: uv run --no-sync pytest
```

Here the built-in environment checks still run. The external check adds a
condition; it cannot bypass the built-in checks or forge preparation identity.
The script is project-owned runtime code, not an example of shipped tooling.
Its declared condition must cover any package expectations on which consumers
rely. A lockfile digest records preparation inputs, not current installation
integrity.

`kind: custom` supports conditions without a suitable built-in kind. It
requires an explicit external probe and cannot acquire stronger verification
claims than that probe supplies. A custom state may omit `path` when it
observes a service; that does not authorize network access or remote mutation.

## 4. State definition and identity

State names use the existing declaration-name and namespace rules. Unknown
fields, duplicate names, unresolved references, and unsupported kinds fail
before execution. Definitions contain `kind`, kind-specific configuration, an
optional `path`, `identity`, optional `prepare`, optional `probe`, and optional
`accept_degraded`.

`identity.files` contains exact capability-scoped paths, not shell patterns.
Missing required files are errors; optional inputs need a future explicit
contract. The normalized definition, probe implementation/version, preparation
plan, relevant declared input values, declared file contents, and resolved
execution settings contribute to identity. Built-ins add their documented
interpreter/platform facts. Identity excludes irrelevant typed inputs.

Files used by an external probe, including project scripts, must participate in
identity. Bundled runtime files follow RFC 0003's resource inventory and digest
rules. Ambient tools cannot be described as pinned solely because their names
match. Diagnostics distinguish a declared command from a verified executable
identity and disclose unmodelled ambient dependencies.

A persistent record proves only that a particular preparation completed and
passed verification. It is never a reusable live probe result. Validate mutable
state on every state operation; do not cache success by a directory timestamp,
unqualified success stamp, or default time-to-live. Removing or corrupting a
record means no trusted preparation evidence, not success.

Without an `identity` declaration, readiness depends on current probes and no
prior preparation record is required. Declaring `identity` adds matching
preparation evidence as a readiness condition. Post-preparation verification
checks the newly prepared candidate identity plus fresh probes, not the stale
record it is about to replace; publication still follows successful checks.

For identities that require preparation evidence, a mismatch establishes
`not_ready` only after the resource can be inspected successfully. Permission
failure, unsupported inspection, and unreadable identity inputs remain errors or
`unknown`, not repair triggers. A custom functional condition without such
identity inputs can become ready through verification alone.

## 5. Operations and outcomes

The proposed command union adds exactly three single-key operations:

- `require_state: NAME` verifies readiness and never prepares anything.
- `ensure_state: NAME` verifies, prepares once when definitely not ready and
  preparation is declared and authorized, then verifies again.
- `prepare_state: NAME` explicitly runs preparation and subsequent verification,
  delegating any incremental work to the selected ecosystem tool.

An operation holds its state lease as described in section 8. Preparation must
finish successfully and post-verification must establish readiness before
Netsuke records success. There is no automatic repair loop or fallback runner.
Failure stops subsequent commands in the action. An absent preparation recipe
produces an actionable unmet-precondition result, not an inferred installer.

The result algebra is `ready`, `not_ready`, `degraded`, and `unknown`.
`degraded` fails a required readiness condition by default and never triggers
repair. A state may explicitly set `accept_degraded: true` only within operator
policy; the warning remains observable. `unknown` never authorizes automatic
preparation or satisfies a required condition. An explicit `prepare_state` is a
separate authorized mutation request, not a consequence of a failed probe.
Cancellation remains cancellation.

Combining built-in and external checks requires every condition to pass.
Inspection errors take precedence over a possible repair decision: an unknown
check prevents repair even when another check reports not-ready. A preparation
identity mismatch and a functional failure retain distinct reason codes.

## 6. External probe protocol

The optional `nagios` protocol uses the conventional plugin exit statuses[^1]:

| Exit | Probe result | Default state-operation behaviour                         |
| ---- | ------------ | --------------------------------------------------------- |
| 0    | `ready`      | Continue only if every other condition passes.            |
| 1    | `degraded`   | Stop without repair; explicit acceptance may continue.    |
| 2    | `not_ready`  | An ensure may prepare once; a requirement never prepares. |
| 3    | `unknown`    | Stop without repair.                                      |

Table 1: Nagios-style statuses and their Netsuke interpretation.

Other exits, signals, spawn failure, timeout, malformed output encoding, and
output-budget exhaustion produce `unknown` with a distinct execution reason. A
process's exit status is authoritative; a success-looking message cannot
convert a failing exit into readiness. The first stdout line supplies a human
summary. Remaining stdout and stderr are bounded diagnostic data. A `|` suffix
may be retained as uninterpreted performance data, but it never controls state,
identity, scheduling, or authorization. No metric parser or Nagios daemon is
required.

Proposed defaults are a ten-second wall-clock deadline and 64 KiB combined
stdout/stderr. Optional `probe.timeout_seconds` and `probe.max_output_bytes`
are positive integers; trusted operator limits cap requested bounds. Drain both
streams concurrently, enforce limits during collection, and terminate and reap
the owned process tree on timeout, cancellation, or excess output. No shell
redirection or unbounded capture is necessary. A probe cannot change
command-local runtime bindings in the consuming action.

Only ordinary structured invocation fields needed for a functional check are
accepted inside `probe.external`: `invoke`, `env`, `cwd`, and allowed `shell`
selection. Reject pipelines, stream files, runtime capture, nested state
operations, and cleanup there. Explicit named-shell probes remain possible
under RFC 0011; direct invocation is the default. Probe return codes are not
Netsuke's public CLI exit codes: the existing structured-result mapping owns
that translation.

Probes should be observational and idempotent, but executing arbitrary code
does not prove either property. The runner must not claim a read-only or
network sandbox that it does not supply. An operator can forbid external probes
or restrict their executable identities. Project configuration and bundle
content cannot weaken those restrictions. Lack of authorization is an error,
not a reason to bypass the probe. Do not run probes or preparation merely to
render help, inspect context, check a manifest, generate a graph, or preview
work.

## 7. Execution settings and security

Use the ordinary inherited environment, exact per-command overlays, trusted
shell selection, and capability-scoped working-directory rules. A new named
context system is not a prerequisite. If named contexts later become available,
resolve them through the same command plan rather than a state-private resolver.

Probe and preparation commands may intentionally use different tools. Record
both effective settings and diagnose accidental environment-root mismatches; do
not replace an unavailable requested interpreter with an ambient one. External
code has the same trust implications as a build recipe. State annotations do
not make an untrusted checkout safe to execute.

Redact probe arguments and output through the shared diagnostic policy, bound
messages, and escape terminal control sequences. Do not export raw probe output
as metric labels. Built-ins must use the injected environment and filesystem
seams, not process-global mutation or a separate configuration reader.

## 8. Scheduling, mutation, and durable records

Keep state operations inside ordinary Ninja-scheduled action edges. They are
not graph-discovery operations, and their results cannot change manifest-time
conditions or add undeclared dependencies. The action-plan codec must represent
them explicitly and reject unknown versions during replay.

State evidence is not itself a Ninja output. A state-using action must either
be an always-run action or explicitly opt into always-run execution; reject an
incremental file target with state operations until a separate
runtime-validation contract can guarantee that its checks actually run. This
avoids silently skipping a readiness check because an unrelated output is up to
date.

For managed mutable paths, acquire a workspace-scoped advisory lease for every
state referenced by an action before its first command, in canonical resource
order, and retain the leases through the last consumer in that action. These
are integrity locks for cooperating invocations, not a replacement scheduler.
Bound acquisition and never acquire a lease while recursively invoking Netsuke.
Reject different state identities claiming the same mutable path within one
selected build closure; isolated paths are the first-version remedy.

A standalone preparation action does not hold a lease for its dependants.
Consumers therefore need their own `require_state` or `ensure_state` operation.
Shared preparation may be skipped after fresh validation, but readiness cannot
be memoized across another action's mutation. Unmanaged commands and other
programs do not participate; the guarantee must say so explicitly.

Named contention classes in [RFC 0016][contention] can reduce contention before
action dispatch. They do not replace state leases across invocations. Shared
state identity and filesystem alias handling must use existing capability
anchors; ambiguous aliases or unsupported locking filesystems fail rather than
pretend to serialize access.

Store records in a versioned, bounded runtime namespace separate from dyndep
sidecars. Publish records atomically only after verification. Interrupted
preparation leaves no success record; partial resources remain unverified and
may need explicit remediation. Cleanup invalidates records under the same
lease. Do not automatically remove an environment after failure or run
undeclared teardown commands. Artefact ownership is a separate optional
contract.

## 9. Verification and acceptance

Unit and behavioural tests must cover all four results, all three operations,
post-preparation failure, identity changes, damaged records, denied probes, and
absence of a preparation recipe. Property tests must establish that unknown
results never authorize repair and only verified success creates a record.

External-probe fixtures must cover every exit code, empty and multiline output,
performance suffixes, non-UTF-8 data, terminal escapes, huge simultaneous
output, hung descendants, cancellation, and a failure message containing
apparent success text. Assert bounded collection and complete process reaping.

End-to-end tests must delete a package/interpreter component after successful
preparation, change identity inputs, contend on the same resource from separate
processes, and interrupt between preparation and record publication. A normal
hello-world build must start zero probes and create no state records.

The Cuprum canary must preserve its restricted extension-test selection and
verify that `require_state` reports a missing extension without running
Maturin. Measure setup reduction without counting a stale probe result as a
cache hit.

## 10. Alternatives, migration, and outstanding decisions

Timestamp-only stamps are insufficient for live readiness. A mandatory Nix-like
store would require a different workflow and is out of scope. External checks
alone would force every project to reinvent common checks; built-ins alone
would force plugin development for ordinary functional conditions.

Adoption is per action. Existing commands and environment management remain
supported, and no package installer becomes a prerequisite for unrelated work.
Allocate the manifest and persisted-plan versions during acceptance alongside
RFC 0001; examples here are proposed fragments, not current-release promises.

Before implementation, ratify the precise `python-venv` inspection contract,
portable lease implementation, runtime-record retention limits, and operator
probe-policy fields. A future uv-specific package-integrity probe should reuse
uv's supported interfaces rather than implement another resolver. None of these
choices may make bare command recipes depend on state machinery.

## 11. Recommendation

Deliver built-in probes and explicit operation semantics first, then external
functional checks with the same result algebra and process boundary. Keep
readiness, preparation evidence, and artefact ownership distinct.

[roadmap]: ../roadmap-progressive-enhancement.md#23-verified-preparation-without-mandatory-environments
[maturity]: 0017-progressive-enhancement-and-maturity-policies.md
[commands]: 0001-structured-command-blocks.md
[contention]: 0016-named-contention-classes.md
[^1]: [Nagios plugin development guidelines](https://nagios-plugins.org/doc/guidelines.html),
      plugin return codes.
