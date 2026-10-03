# Architectural decision record (ADR) 043: Consolidated structured-command execution contract

## Status

Accepted. The structured-command amendments share one execution contract:
relative stream paths resolve against the block's resolved `cwd`; absolute
`cwd` is rejected; the ephemeral `capture_stdout` and the named
`stdout: { env: NAME }` capture remain distinct facilities; and the per-stage
`temp_dir` environment binding and the secure-temporary working directory
remain distinct facilities that may combine. The consolidated surface reserves
manifest-format minor version `1.1.0`.

## Date

2026-10-03

## Context and problem statement

Four documents describe the structured command block, and they disagree.
[RFC 0001](rfcs/0001-structured-command-blocks.md) is the base contract,
[RFC 0009](rfcs/0009-structured-command-working-directories.md) adds `cwd`,
[RFC 0010](rfcs/0010-runtime-bindings-and-secure-tempdirs.md) adds runtime
bindings and secure temporary directories, and
[RFC 0011](rfcs/0011-allow-listed-structured-command-shells.md) adds named
shells. [ADR-019](adr-019-structured-command-shell-selection.md) already
depends on the combined semantics: it requires named shells to retain "all
Netsuke-managed environment, working-directory, standard-stream, capture,
temporary-directory, sequencing, and pipeline semantics from RFC 0001 sections
10.3 to 14", but the sections it names do not say one consistent thing.

Three conflicts had to be settled before any of the amendments could be
implemented, because a compiler and an action runner cannot implement two rules
at once:

1. **The base for relative stream paths.** RFC 0001 section 9 says the block or
   stage directory "is the base for all relative stream paths", and section
   12.1 resolves every stream path "relative to the block's resolved `cwd`".
   RFC 0009 section 11 said the opposite — that stream paths stay relative to
   the effective workspace root after `-C` — and, in doing so, misdescribed the
   RFC 0001 text it cited. RFC 0011 section 6.2 agreed with RFC 0001. The same
   section pair also disagreed about absolute `cwd`: RFC 0001 section 9 said
   "an absolute path remains absolute", while RFC 0009 section 5 rejected
   absolute paths as an escape from the workspace capability.
2. **Two temporary-directory spellings.** RFC 0001 section 11.3 defines
   `temp_dir: true` as a private per-stage directory bound to `TMPDIR`, `TMP`,
   and `TEMP`. RFC 0010 section 6.4 defines `cwd: { tempdir: {} }` and
   `cwd: { tempdir: { env: NAME } }` as a secure directory that becomes the
   working directory, and section 10.4 states that Netsuke does not implicitly
   rewrite the conventional variables. Nothing said whether both survive, how
   they combine, or which wins.
3. **Two standard-output capture spellings.** RFC 0001 section 12.3 defines
   `capture_stdout: LIMIT` as a bounded ephemeral sink. RFC 0010 section 6.1
   makes `stdout` a union with a named `{ env, chomp, max_bytes }` capture form
   that commits a runtime binding. Nothing said how the two relate, and RFC
   0011 section 6.2 mentioned both without distinguishing them.

The discrepancies are not cosmetic. A stream-path base decides where a build
writes its artefacts and whether a relocated command silently changes the
graph. The temporary-directory question decides whether a child's temporary
files land in a directory that survives the stage. The capture question decides
whether a captured value can be consumed at all. Roadmap task 12.1.1 —
"Consolidate and ratify the amended execution contract" — owns the decision and
also requires this record to cover the next available manifest minor version,
action-plan versioning, and bounded termination and stale-plan cleanup policies.

### The RFC 0009 misstatement is a drafting error, not a competing design

RFC 0001 has carried both statements since its first commit (`7101bdc6`, "RFC:
structured command blocks and argv templates (#573)"). Section 9's
`cwd`-is-the-base sentence and section 12.1's `cwd`-relative resolution were
present at birth, so RFC 0001 was internally consistent on this point from the
start.

RFC 0009 and RFC 0010 were added later, in a single commit (`ac10b783`) that
did not touch RFC 0001. RFC 0009 section 11 was then written asserting that RFC
0001 "currently resolves" stream paths against the effective working directory
and that this amendment "preserves that rule". Both halves of that sentence
contradict the RFC 0001 text they cite. RFC 0011 section 6.2, added afterwards
(`f2f63a08`), took the `cwd`-relative reading.

So the disagreement is one document's mistaken summary of another, and the
acceptance below corrects the summary rather than reversing the base contract.
That matters for ADR-019, whose "sections 10.3 to 14" reference only makes
sense if the base contract stands.

## Decision Drivers

- **One contract for four documents.** A manifest must behave the same way
  whether a block is direct, `shell: true`, or a named shell, and a reader must
  be able to settle a question by reading one rule rather than three.
- **Capability confinement.** A rendered manifest string must not expand the
  process's ambient filesystem authority. This principle already governs `cwd`
  resolution, executable resolution, and captured path text.
- **Deterministic graph artefacts.** A stage's declared inputs and outputs must
  be derivable from its declaration alone, without mentally applying every
  other stage's process directory.
- **Least surprise for a Makefile reader.** A block that names a directory
  should behave like a shell redirect from that directory.
- **No feature loss.** RFC 0001, RFC 0009, and RFC 0010 each motivate a
  capability that a migration needs. Consolidation must not delete a capability
  to resolve a conflict.
- **Trust boundary preservation.** ADR-019's shell registry, ADR-014's
  safe-failure rule, and the workspace capability model stay as accepted.

## Decision

### The stream-path base is the stage's resolved `cwd`

A relative `stdin`, `stdout`, `stderr`, or `tee` path resolves against the
block's resolved `cwd`, exactly as RFC 0001 sections 9 and 12.1 state. An absent
`cwd` resolves to the effective `-C` directory, so the base is always defined.

The base composes once:

1. the effective `-C` directory;
2. the stage's rendered `cwd`, normalized relative to (1);
3. the rendered stream path, normalized relative to (2).

Each stage composes its own path. Two stages in one pipeline with different
`cwd` values therefore resolve the same relative stream path to two different
absolute paths, and neither inherits the other's directory. This holds in
direct mode, `shell: true`, and every named shell.

RFC 0009 section 11 is corrected to state this rule and to record that its
previous text misdescribed RFC 0001. RFC 0009 section 20.3 becomes a rejected
alternative rather than a deferred one, and RFC 0009 section 3.2's non-goal
list and section 19's test list are aligned.

### An absolute `cwd` is rejected

A rendered `cwd` must normalize inside the effective workspace capability.
Absolute paths are invalid in the initial surface, and lexical or symlink
escapes are rejected. RFC 0001 section 9's "an absolute path remains absolute"
sentence is removed as inconsistent with the confinement rule RFC 0009 section
5 states.

The alternative — allowing an absolute path while keeping stream paths relative
to it — was rejected because it grants ambient filesystem authority through a
rendered string, weakens bundle portability, and would make the stream-path
base escape the workspace as well. An explicit capability-scoped external
directory handle remains the designed extension, as RFC 0009 section 5 already
anticipates.

### Both capture forms remain, with different lifetimes

`capture_stdout: LIMIT` and `stdout: { env: NAME }` are different facilities
and both stay in the initial surface.

`capture_stdout` is an ephemeral raw-byte sink. Its result lives in the
runner's bounded action result until the unit completes, is never published as
a binding, is never visible to a later unit, and stores only the limit in the
action plan. `stdout: { env: NAME }` commits a sequence-local runtime binding
that later items in the same sequence can consume, is validated as UTF-8 text,
applies `chomp` and `max_bytes`, and records its name, limits, and provenance
in the plan while never recording its value.

Both are standard-output sinks, so a stage selects exactly one. The exclusivity
rule in RFC 0001 section 12.3 is extended to name the environment capture
explicitly, RFC 0010 section 7.4 states it from its side, and both validation
sets reject a stage that selects more than one.

### Both temporary-directory spellings remain, and they combine

`temp_dir: true` and `cwd: { tempdir: ... }` are different facilities and both
stay in the initial surface. Roadmap task 14.2.2 already scheduled them as
separate implementation work; this record fixes the semantics they share.

- `temp_dir: true` creates a private per-stage directory, binds the stage's
  `TMPDIR`, `TMP`, and `TEMP` to it after the `env` overlay, and removes it
  when the stage and its relays finish. It does not change the working
  directory, it publishes nothing to a later unit, and it stores nothing in the
  plan.
- `cwd: { tempdir: {} }` creates a secure directory, sets the child's working
  directory to it, and removes it when the unit finishes.
  `cwd: { tempdir: { env: NAME } }` additionally publishes a sequence-local
  directory binding that later units in the sequence can consume. Neither form
  rewrites a conventional variable.

When a block declares both, the process working directory is the secure
temporary directory from the `cwd` form while the conventional variables name
the stage-private directory from `temp_dir`. Each facility keeps its own owner
and cleanup path; both are removed on the same completion paths; neither
directory is a subtree of the other; and Netsuke creates no link between them.
A tool that honours the conventional variables writes into the stage-private
directory, and a tool that honours its process working directory writes into
the secure temporary directory.

This is deliberate rather than a gap. `temp_dir` exists so that tools which
consult the conventional variables get a private directory without the manifest
enumerating the platform's variable names. The tempdir `cwd` form exists so
that a stage can run inside a secured directory with a typed capability that
ordinary text cannot forge. They answer different questions, and the
combination is the only way to get both properties at once. A manifest that
needs the two to agree either sets the variables itself with `temp_dir: false`
or waits for typed runtime binding references in environment overlays, which
RFC 0010 section 19 defers to a later amendment.

### Both forms materialize before the first pipeline stage starts

RFC 0010 section 10.3 requires every secure temporary directory and directory
binding needed by any stage of a structured pipeline to exist before the first
spawn. The per-stage `temp_dir` directory follows the same rule, so no stage
observes a partially prepared unit. A pipeline with `n` stages and
`temp_dir: true` creates `n` private directories even when the stages share one
named secure tempdir.

### Bounded termination is defined

RFC 0010 section 7.1 requires an over-limit capture to terminate the unit
"using RFC 0001's bounded termination policy", but RFC 0001 never named one.
RFC 0001 section 13.3 now defines it. When Netsuke must stop a started
execution unit it closes its managed pipe, relay, and tee endpoints; requests
termination of every still-running child using the documented process-group or
job-object policy; waits a bounded interval; escalates to forceful termination
for any child still running; and reaps every child and joins every relay before
reporting. Children are never abandoned because a termination request was slow.

The same policy covers stage-spawn failure, pipe, relay, or tee I/O failure,
capture-limit overflow, cancellation, and timeout. The execution IR carries the
resolved process-group or job-object mode, and a plan whose mode a runner
cannot represent is rejected rather than executed with an unbounded wait.

### Stale-plan cleanup is bounded

RFC 0001 section 17.4 now states the policy for both plan arms. Build-time
plans live in the private temporary directory Netsuke created for that build,
and cleanup removes only unleased plans inside it rather than scanning an
arbitrary directory. Persistent sidecars are cleaned through the recorded
manifest-local index rather than by walking the filesystem; a sidecar named by
the current index or protected by a live lease is retained, and an interrupted
publication leaves the previous complete association usable.

Cleanup covers a bounded, enumerable set of paths and fails closed: when it
cannot establish that a plan is unreferenced and unleased it retains the plan
and reports the condition. It never removes a plan a concurrent Netsuke process
may still execute.

### Version allocation

The consolidated structured-command surface reserves manifest-format minor
version `1.1.0`, the number RFC 0001 section 19.2 and RFC 0011 section 9
already proposed. RFC 0009, RFC 0010, and RFC 0011 land inside that single
increment rather than taking one each, because all four amend the same mapping
schema. Roadmap tasks 16.1.1 and 17.1.1 must not claim `1.1.0` for an unrelated
schema; they coordinate the next number, not this one.

Persisted action plans use an independent version namespace. A plan is
versioned by the variants it contains, so a runner that cannot represent a
`cwd` selector, a binding, or a capture form rejects the plan instead of
ignoring the field. Plan versions advance when the persisted vocabulary
changes, not when the manifest-format minor version does, and the two numbers
are recorded separately.

## Goals and non-goals

- Goals:
  - one contract, stated once, that RFC 0001, RFC 0009, RFC 0010, RFC 0011, and
    ADR-019 all agree with;
  - every disputed case settled, including temporary-directory precedence and
    lifetime;
  - direct, default-shell, and named-shell implementations sharing one path
    contract rather than selecting contradictory paragraphs;
  - no capability lost, so roadmap tasks 13.1.5, 14.1.2, 14.2.2, and 14.2.3 can
    proceed against a fixed target; and
  - bounded termination and stale-plan cleanup recorded so the action-runner
    work has a stated policy.
- Non-goals:
  - promoting RFC 0001, RFC 0009, RFC 0010, or RFC 0011 out of `Proposed`. This
    record is the acceptance artefact; the RFCs remain proposals that a later
    implementation folds into the base text, exactly as RFC 0009 section 1
    already describes.
  - implementing any of the surface. Roadmap task 12.1.1 ratifies the contract;
    tasks 12.1.2 onward implement it.
  - merging `temp_dir` into `cwd: { tempdir: ... }`, or merging
    `capture_stdout` into `stdout: { env: NAME }`;
  - typed runtime binding references in environment overlays, an external
    directory capability, or object-valued stream paths, all of which remain
    deferred; and
  - changing ADR-019's shell registry, trusted-configuration authority,
    lowering, diagnostics, or safety model.

## Known risks and limitations

- **Relocated commands move their artefacts.** A block moved to a different
  `cwd` writes relative stream destinations to a different path. That is the
  intended shell-redirect behaviour, but a manifest author who previously
  assumed a fixed workspace base must audit relative stream paths when the
  surface ships.
- **Temporary directories and relative stream paths interact.** A relative
  stream path under a secure-tempdir `cwd` resolves inside a directory that is
  deleted when the stage or sequence finishes. RFC 0010 section 9.5 states the
  rule so the interaction is explicit rather than discovered at build time.
- **Both capture forms keep two code paths.** The ephemeral sink and the named
  binding share a byte-counting core but diverge on commit, encoding,
  redaction, and plan representation. This is the cost of keeping both
  capabilities.
- **Two tempdir facilities keep two owners.** `temp_dir` and the tempdir `cwd`
  form have separate lifecycles and separate cleanup paths, so a combination
  produces two removals per stage. Fault injection must cover both.
- **Version coordination is a convention, not a mechanism.** Nothing in the
  build enforces that 16.1.1 and 17.1.1 respect the `1.1.0` reservation; the
  coordination is a task dependency in the roadmap.

## Architectural Rationale

The decision follows the same principle as ADR-019: keep manifest authority
narrow, keep the domain value resolved before it reaches an adapter, and make
the trust boundary visible. A `cwd` that cannot escape the workspace, a stream
path that resolves against a single declared base, a capture that declares
whether it outlives its producer, and a temporary directory that declares
whether it is an environment binding or a working directory are all instances
of the same rule — the manifest says what it means, and Netsuke does exactly
that.

Composition happens once, at manifest compilation, and the result is carried as
typed data into the execution IR. No stage re-derives a base from ambient
state, no runner consults configuration again, and no failure path silently
chooses a default over a rejected value.

## References

- Issue [#803](https://github.com/leynos/netsuke/issues/803), which records the
  three conflicts.
- [RFC 0001: Structured command blocks and
  argv templates](rfcs/0001-structured-command-blocks.md) sections 6.2, 9,
  10.3, 11.3, 12.1, 12.3, 13.3, 15.3, 17.3, 17.4, 19.2, and 21.
- [RFC 0009: Structured-command working
  directories](rfcs/0009-structured-command-working-directories.md) sections
  3.2, 5, 8, 11, 19, and 20.3.
- [RFC 0010: Runtime bindings and secure
  tempdirs](rfcs/0010-runtime-bindings-and-secure-tempdirs.md) sections 6.1,
  6.4, 7.4, 9.5, 10.3, 10.4, 10.5, 11.5, and 19.
- [RFC 0011: Allow-listed structured-command
  shells](rfcs/0011-allow-listed-structured-command-shells.md) sections 6.2 and
  9.
- [ADR-019: Select allow-listed structured-command
  shells](adr-019-structured-command-shell-selection.md).
- [ADR-014: Backend text escaping
  seam](adr-014-backend-text-escaping-seam.md) for the safe-failure rule.
- [Roadmap](roadmap.md) task 12.1.1 and the dependent tasks 11.3.2, 12.2.3,
  13.1.5, 14.1.2, 14.2.2, 14.2.3, 16.1.1, and 17.1.1.
