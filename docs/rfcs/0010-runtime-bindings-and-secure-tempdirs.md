# RFC 0010: Runtime bindings and secure execution contexts

## Preamble

- **RFC number:** 0010
- **Amends:** RFC 0001, Structured command blocks and argv templates
- **Also refines:** RFC 0009, Structured-command working directories
- **Status:** Proposed
- **Created:** 2026-08-26
- **Target:** Structured command execution context and action-runner IR

## 1. Summary

This amendment adds four related structured-command capabilities:

1. capture a command's standard output into a named environment binding;
2. connect one pipeline stage's standard error to the next stage's standard
   input;
3. select `cwd` from a named environment binding; and
4. execute in a securely created temporary directory, optionally binding that
   directory to an environment name for later commands.

The features share one action-local runtime binding context. The runner owns
that context and passes its values to child processes without mutating
Netsuke's own process environment.

A typical sequence is:

```yaml
command:
  - invoke: workspace-locator --relative
    stdout:
      env: WORKSPACE_DIR
  - invoke: cargo test --all-targets
    cwd:
      env: WORKSPACE_DIR
```

A secure temporary workspace is:

```yaml
command:
  - invoke: prepare-fixture
    cwd:
      tempdir:
        env: FIXTURE_DIR
  - invoke: verify-fixture
    cwd:
      env: FIXTURE_DIR
```

A diagnostic pipeline may use standard error as its data stream:

```yaml
command:
  - invoke: compiler broken-input.rs
    pipe: stderr
  - invoke: diagnostic-normalizer
```

The central safety rules are:

> Runtime bindings are scoped data owned by the action runner, not mutations of
> the parent process environment.
>
> Text captured from a child cannot grant filesystem authority merely by
> containing an absolute path.
>
> A runner-created temporary directory carries an explicit directory
> capability and is cleaned on every completion path.

Upon acceptance, RFC 0001 must be read as if its stream, pipeline, environment,
validation, action-runner, security, and test sections included the semantics
below. The working-directory amendment must be read as if `cwd` accepted the
additional environment and temporary-directory forms in section 6. RFC 0009
therefore remains the literal-path foundation for these additional forms.

The implementation PR should fold this amendment into RFC 0001 and its
working-directory text before the RFC moves from Proposed to Accepted.

## 2. Motivation

Several common Make and shell patterns remain awkward even after argv-safe
structured commands gain literal `cwd` support.

### 2.1 Capture a discovered value

Builds often run a small discovery command and export its result:

```sh
WORKSPACE_DIR=$(tool locate-workspace)
cd "$WORKSPACE_DIR"
cargo test
```

Encoding this through shell command substitution loses the direct-process
safety model. Writing the result to a file is possible but creates unnecessary
filesystem state and forces every consumer to parse the file.

### 2.2 Parse diagnostics rather than ordinary output

Some tools deliberately write machine-readable or normalization-worthy data to
standard error. Shell pipelines express this with implementation-specific file
descriptor syntax. A structured pipeline should state which output stream feeds
the next stage without relying on `2>&1`, process substitution, or PowerShell
redirection rules.

### 2.3 Select a directory at runtime

The literal `cwd` amendment covers repository-known directories but not a path
selected by a previous command, supplied through a controlled environment
binding, or created for one action execution.

### 2.4 Use a private temporary workspace

Build and test tasks frequently need a scratch directory for extraction,
fixture generation, signing inputs, package assembly, or untrusted intermediate
files. Calling `mktemp`, `%TEMP%`, or a PowerShell helper from shell source has
platform-specific quoting, permission, cleanup, and race behaviour.

Temporary-directory creation is a capability and lifecycle concern. It belongs
in the action runner rather than in recipe text.

## 3. Goals and non-goals

### 3.1 Goals

This amendment aims to:

- capture bounded UTF-8 standard output into a named runtime environment
  binding;
- make that binding available to later execution units in one command sequence;
- preserve child-process environment isolation;
- let `cwd` consume a named runtime or inherited environment value;
- distinguish ordinary text paths from runner-owned directory capabilities;
- securely create, expose, use, and remove temporary directories;
- support standard-error pipelines without merging streams;
- retain deterministic validation and precise source provenance;
- define failure and cleanup behaviour across commands and pipelines; and
- preserve existing RFC 0001 and legacy-command behaviour.

### 3.2 Non-goals

This amendment does not:

- expose captured values to manifest-time Jinja;
- propagate a child's environment mutations back to Netsuke or later children;
- capture arbitrary binary output into environment variables;
- capture unbounded output;
- merge standard output and standard error into one pipe;
- pipe both output streams to one downstream standard input;
- make a runtime binding visible across independent Ninja edges;
- export a runtime binding as a target output or cache key;
- permit arbitrary absolute `cwd` values from inherited environment variables;
- preserve temporary directories after success or failure;
- provide a directory stack;
- infer graph inputs or outputs from captured values or temporary files; or
- replace explicit build artefacts with hidden temporary state.

A future RFC may define typed action outputs that cross graph edges. This
amendment is intentionally limited to one action-runner sequence.

## 4. Terminology

- **Runtime binding:** A named value held by the action runner and injected into
  later child environments.
- **Text binding:** A UTF-8 runtime binding without filesystem authority.
- **Directory binding:** A runtime binding paired with a runner-owned directory
  capability.
- **Binding context:** The ordered, action-local set of runtime bindings visible
  to one command sequence.
- **Producer:** A command field that creates a runtime binding.
- **Consumer:** A later command field that reads a runtime binding.
- **Secure temporary directory:** An unpredictable, owner-private directory
  created and removed by the action runner through platform process and
  filesystem APIs.
- **Selected pipeline stream:** The one output stream connected to the next
  structured pipeline stage's standard input.

## 5. Binding-context semantics

### 5.1 Scope

Every structured command sequence receives a fresh binding context when the
action runner begins the sequence. The context is destroyed when that sequence
finishes, whether by success, failure, cancellation, or spawn error.

A sequence means the heterogeneous `command` list owned by one action, target,
or rule execution unit. A single structured command also has a context, though
a captured value has no later consumer unless a future action-output RFC uses
it.

Bindings are visible to later items in lexical execution order. They do not
cross:

- independent Ninja edges;
- separate actions or targets;
- rule-reference boundaries defined by RFC 0001;
- a newly invoked Netsuke process; or
- manifest evaluation.

Script items and newly spawned legacy shell groups within the same
action-runner sequence receive the current binding context as part of their
child environment. An all-string legacy command list remains one legacy shell
group and cannot produce a structured runtime binding.

### 5.2 Effective child environment

For each child process, Netsuke constructs the effective environment in this
order:

1. the runner's inherited child-environment base;
2. sequence-local runtime bindings; and
3. the structured command's explicit `env` overlay.

Later layers win for the child process only. An explicit command `env` entry
may shadow a runtime binding for that command without changing the stored
binding.

The parent Netsuke process environment is never modified.

### 5.3 Names and collisions

Runtime binding names are literal YAML strings. Jinja is not permitted in a
binding name.

For portable behaviour, names must match:

```text
[A-Za-z_][A-Za-z0-9_]*
```

Netsuke compares binding names case-insensitively on Windows and
case-sensitively on other supported platforms. The original spelling is
preserved for child environment construction and diagnostics.

Two producers in one binding scope may not bind the same normalized name. A
producer may shadow an inherited environment variable, but the diagnostic and
structured plan record that shadowing explicitly.

### 5.4 Commit semantics

A producer reserves its binding name during plan validation. The value becomes
visible only after its execution unit succeeds.

If the producer fails, is cancelled, exceeds a capture limit, emits invalid
text, or encounters a cleanup error before commit, Netsuke does not publish the
binding.

For a structured pipeline, a captured value commits only after every stage has
completed successfully according to RFC 0001's pipeline policy.

## 6. Manifest syntax

### 6.1 Standard output to an environment binding

RFC 0001's `stdout` field becomes a union. Its existing path-string form
remains unchanged. The new mapping form captures standard output:

```yaml
stdout:
  env: WORKSPACE_DIR
  chomp: true
  max_bytes: 65536
```

| Field       | Type                      | Default  | Meaning                          |
| ----------- | ------------------------- | -------- | -------------------------------- |
| `env`       | portable environment name | required | Runtime binding to create.       |
| `chomp`     | Boolean                   | `true`   | Remove one trailing line ending. |
| `max_bytes` | positive integer          | `65536`  | Maximum captured byte count.     |

Table 1: Standard-output environment capture fields.

Unknown keys are errors. The initial manifest-format maximum for `max_bytes` is
16 MiB. A smaller implementation hard limit is invalid because the manifest
contract must be portable across runners.

`chomp: true` removes at most one final line ending:

- `\r\n` is removed as one line ending;
- otherwise one final `\n` is removed; and
- all other bytes are preserved.

It does not trim spaces, tabs, additional blank lines, or an isolated final
carriage return.

Example:

```yaml
command:
  - invoke: printf-workspace-path
    stdout:
      env: WORKSPACE_DIR
  - invoke: cargo check --all-targets
    cwd:
      env: WORKSPACE_DIR
```

### 6.2 Pipeline source selection

RFC 0001's Boolean `pipe` field becomes a backwards-compatible union:

```yaml
pipe: false
pipe: true
pipe: stdout
pipe: stderr
```

The meanings are:

- `false`: no pipe to the next stage;
- `true`: compatibility spelling for `stdout`;
- `stdout`: connect this stage's standard output to the next stage's standard
  input; and
- `stderr`: connect this stage's standard error to the next stage's standard
  input.

“Standard error to standard input” means the current stage's writable standard
error stream becomes the immediately following stage's readable standard input.
It never attaches a process's own writable standard error handle to its own
read-only standard input handle.

Example:

```yaml
command:
  - invoke: compiler invalid-source.rs
    pipe: stderr
  - invoke: normalize-diagnostics
    stdout: normalized-errors.txt
```

Merged standard output and standard error remain outside RFC 0001's initial
surface.

### 6.3 Working directory from an environment name

The `cwd` field gains a mapping form:

```yaml
cwd:
  env: WORKSPACE_DIR
```

The mapping contains exactly one `env` key. The name follows section 5.3.

The runner resolves the name from the effective child environment described in
section 5.2. A sequence-local directory binding retains its directory
capability. An ordinary inherited, captured, or explicitly overlaid value is a
text binding and receives the path validation in section 9.

### 6.4 Secure temporary working directory

The `cwd` field also gains a temporary-directory form:

```yaml
cwd:
  tempdir: {}
```

This creates a private temporary directory for one execution unit, sets the
child process working directory to it, and removes it after that unit finishes.
The empty mapping is required so a literal repository directory named `tempdir`
remains unambiguous.

A named form publishes a directory binding:

```yaml
cwd:
  tempdir:
    env: FIXTURE_DIR
```

When `env` is present:

- the runner creates the directory before the execution unit starts;
- the current child receives the path under `FIXTURE_DIR`;
- the command runs inside that directory;
- later execution units in the same sequence inherit the binding;
- `cwd: { env: FIXTURE_DIR }` consumes the retained directory capability; and
- the runner removes the directory when the entire sequence finishes.

The `tempdir` mapping accepts exactly one optional field:

| Field | Type                      | Default | Meaning                                     |
| ----- | ------------------------- | ------- | ------------------------------------------- |
| `env` | portable environment name | absent  | Publish a sequence-local directory binding. |

Table 2: Secure temporary-directory fields.

Retention, custom prefixes, user-selected roots, and keep-on-failure modes are
not part of the initial surface. Security-sensitive temporary data is removed
on every completion path by default.

## 7. Standard-output capture semantics

### 7.1 Byte collection

The action runner drains the child's standard output continuously to avoid pipe
back-pressure. It counts raw bytes before UTF-8 decoding or line-ending removal.

If the byte count would exceed `max_bytes`, the runner:

1. stops accepting additional bytes;
2. terminates the execution unit using RFC 0001's bounded termination policy;
3. reaps every affected process;
4. reports `stdout_capture_limit_exceeded`; and
5. does not publish the binding.

The runner must not retain unbounded data while waiting for a child to exit.

### 7.2 Text conversion

After successful process completion, Netsuke decodes the captured bytes as
strict UTF-8. Invalid UTF-8 and embedded NUL produce typed errors and prevent
binding commit.

The initial surface has no locale-dependent decoding and no lossy conversion. A
command that needs binary capture must write a declared file artefact instead.

After optional `chomp`, the resulting string may be empty and may contain
embedded newlines. It is inserted into later child environments exactly as a
text value.

### 7.3 Visibility and redaction

Human and JSON diagnostics may expose:

- binding name;
- capture limit;
- captured byte count;
- whether one line ending was removed; and
- success or failure category.

They must not expose the captured value by default. Captured output may contain
credentials, tokens, paths, or user data. It must not become a metric label,
span name, cache key, or generated Ninja comment.

### 7.4 Stream conflicts

A standard-output environment capture is invalid when the same stage also:

- pipes standard output to the next stage;
- redirects standard output to a path;
- tees standard output under RFC 0001; or
- requests another standard-output sink.

It may coexist with `pipe: stderr`, because the selected pipeline stream is
standard error and standard output remains available for capture.

In a pipeline, only a stage whose standard output is not selected for a later
stage may capture that output. The binding commits after complete pipeline
success.

## 8. Standard-error pipeline semantics

### 8.1 Raw stream behaviour

`pipe: stderr` transfers raw bytes. Netsuke does not decode, normalize, frame,
or add prefixes to the stream.

The current stage's standard output follows its independently configured sink:
inheritance, file redirection, teeing, or environment capture.

The current stage's standard error is consumed by the pipe and is not inherited
by the terminal. A future tee-to-pipe surface may duplicate it explicitly, but
implicit duplication is not permitted.

### 8.2 Validation

`pipe: stderr` is invalid when:

- the stage is the final structured stage in a pipeline;
- the next item is a rule reference, script item, or legacy shell boundary;
- the next stage specifies its own `stdin` source;
- the current stage specifies a standard-error path or another standard-error
  sink; or
- the current stage also requests standard-output piping.

Exactly one predecessor stream may feed one stage's standard input.

### 8.3 Exit and cleanup behaviour

RFC 0001's pipeline exit policy remains unchanged. Selecting standard error as
the data stream does not turn a non-zero producer status into success.

When a stage cannot spawn or the pipeline is cancelled, Netsuke closes every
pipe endpoint, terminates started children, drains or abandons streams
according to the bounded cleanup policy, and reaps all processes.

Diagnostics identify the selected stream and the source and destination stages.

## 9. Environment-selected working directories

### 9.1 Resolution order

For `cwd: { env: NAME }`, Netsuke reads `NAME` after applying the environment
precedence in section 5.2.

The value must be present, non-empty, valid UTF-8, and free of NUL.

A relative text value is resolved against Netsuke's effective working directory
after CLI `-C`, exactly like the literal path form in the working-directory
amendment.

### 9.2 Text values and filesystem authority

An ordinary text value receives the same confinement as a literal `cwd`:

- lexical normalization must remain inside the effective workspace;
- symlink resolution must not escape the workspace capability;
- the path must identify a directory at spawn time; and
- an absolute path is rejected in the initial surface.

This rule applies whether the text originated from:

- the inherited environment;
- an explicit command `env` overlay; or
- standard-output capture.

A child cannot gain external-directory authority merely by printing an absolute
path and having it captured into a binding.

### 9.3 Directory bindings

A secure temporary-directory producer creates a typed directory binding. The
binding contains:

- the child-visible path string;
- a runner-owned directory capability;
- lifecycle ownership; and
- producer provenance.

`cwd: { env: NAME }` may use that capability even when the temporary directory
lives outside the workspace. The permission does not derive from the path
string and cannot be forged by a text binding with the same spelling.

If a per-command `env` overlay shadows a directory binding's name, the value is
resolved as ordinary text for that command and the external directory
capability is not inherited through the shadow.

### 9.4 Executable and argument behaviour

After resolving the directory, direct and shell execution follow RFC 0009:

- bare executable names use the effective child `PATH`;
- relative executable paths containing a separator resolve from `cwd`;
- arguments are passed unchanged; and
- shell mode receives the directory through the process API, not generated
  `cd` source.

## 10. Secure temporary-directory semantics

### 10.1 Root selection

The runner selects its temporary root once through application configuration or
the platform secure-temporary-directory API at the composition boundary.

A command's `env` overlay, runtime bindings, or captured output cannot redirect
temporary-directory creation to another root.

The root is opened and retained through a directory capability before creating
an execution directory.

### 10.2 Creation

The runner creates each temporary directory atomically with an unpredictable
cryptographic name and no pre-existing path reuse.

On Unix-like platforms, the directory is owner-only, equivalent to mode `0700`,
before the path becomes visible to a child. On Windows, the directory receives
an access-control list restricted to the current security principal and
required system access.

The implementation must not create a world-readable directory and tighten it
afterwards.

Netsuke rejects a temporary root or selected directory that resolves through an
unexpected symlink or non-directory object.

### 10.3 Binding and pipeline preparation

A command-scoped temporary directory is created immediately before its
execution unit and removed immediately after that unit has been reaped and all
stream tasks have completed.

A named sequence-scoped temporary directory is created when its producer is
prepared and removed after the complete sequence finishes.

For a structured pipeline, the runner materializes every secure temporary
directory and directory binding required by any stage before spawning the first
stage. This lets one stage consume a named directory binding created by another
stage's declaration without introducing a race between concurrent process
starts.

If preparation of any directory fails, no pipeline stage starts.

### 10.4 Child environment

When the tempdir form contains `env`, the current and later children receive
the absolute child-visible path under that environment name.

Netsuke does not implicitly rewrite `TMPDIR`, `TMP`, `TEMP`, `HOME`, or any
other conventional variable. A manifest requiring those values must set them
explicitly through `env`, preferably to the named binding once runtime binding
references are supported in environment overlays by a later amendment.

The child always starts with its process working directory set to the secure
temporary directory.

### 10.5 Cleanup

Cleanup runs after success, failure, cancellation, timeout, signal, spawn
error, and pipeline setup failure.

The runner:

1. waits for or terminates every owned child process;
2. closes stream tasks and handles;
3. recursively removes entries through the retained directory capability;
4. does not follow symlinks during removal; and
5. removes the root execution directory last.

A cleanup failure is not silently ignored:

- after otherwise successful execution, cleanup failure makes the execution
  unit fail;
- after an existing execution failure, cleanup failure is attached as a
  secondary cause; and
- structured output reports both categories without exposing sensitive file
  names unnecessarily.

The initial surface does not retain failed temporary directories for debugging.
A future retention feature must be explicit, security-reviewed, and disabled by
default.

### 10.6 Background descendants

Netsuke owns only processes created through the structured execution unit and
its documented process-group or job-object policy. A child that deliberately
detaches an untracked descendant may keep files or handles alive and cause
cleanup failure.

The diagnostic must state that detached descendants are incompatible with
secure temporary-directory cleanup unless a future supervised-background-task
model owns them.

## 11. Interaction examples

### 11.1 Capture a repository-relative workspace path

```yaml
command:
  - invoke: project-tool print-build-root --relative
    stdout:
      env: BUILD_ROOT
  - invoke: cargo clippy --all-targets
    cwd:
      env: BUILD_ROOT
```

The captured value is text. It may select a directory inside the effective
workspace but cannot escape it.

### 11.2 Share a secure fixture directory

```yaml
command:
  - invoke: fixture-generator
    cwd:
      tempdir:
        env: FIXTURE_DIR
  - invoke: fixture-validator .
    cwd:
      env: FIXTURE_DIR
```

`FIXTURE_DIR` is both an environment value and a typed directory capability.
The directory survives between the two commands and is removed after the
sequence.

### 11.3 Capture stdout while piping stderr

```yaml
command:
  - invoke: compiler source.rs
    stdout:
      env: COMPILER_SUMMARY
    pipe: stderr
  - invoke: diagnostic-normalizer
```

The compiler's standard error feeds the normalizer. Its standard output is
captured separately and commits only if the complete pipeline succeeds.

### 11.4 Reject an untrusted absolute path

```yaml
command:
  - invoke: untrusted-tool print-directory
    stdout:
      env: DIRECTORY
  - invoke: inspect
    cwd:
      env: DIRECTORY
```

If `DIRECTORY` contains `/tmp/outside` or an absolute Windows path, the second
command fails before spawn because the text binding carries no external
directory capability.

## 12. Execution IR amendment

The execution IR gains typed stream, binding, and directory selectors. One
illustrative shape is:

```rust
pub enum PipeStream {
    None,
    Stdout,
    Stderr,
}

pub struct StdoutEnvCapture {
    pub name: EnvName,
    pub chomp: bool,
    pub max_bytes: usize,
}

pub enum StdoutSink {
    Inherit,
    File(Utf8PathBuf),
    Tee(Utf8PathBuf),
    Environment(StdoutEnvCapture),
    Pipe,
}

pub enum WorkingDirectory {
    WorkspaceRoot,
    WorkspacePath(Utf8PathBuf),
    Environment(EnvName),
    SecureTempdir {
        binding: Option<EnvName>,
    },
}

pub struct RuntimeBindingContext {
    pub values: BTreeMap<EnvName, BindingValue>,
}

pub enum BindingValue {
    Text(String),
    Directory(DirectoryBinding),
}
```

The exact Rust names may change, but the distinctions are normative:

- selected pipeline stream is not a Boolean in the IR;
- environment capture is a bounded text sink;
- `cwd` text and directory capabilities remain different variants; and
- the binding context is action-runner state, not a process-global environment
  map.

Generated action plans reserve producer names and include source provenance,
limits, selectors, and lifecycle scope. They never include runtime-captured
values or runtime-generated temporary paths.

A runner that does not understand these plan variants must reject the plan
schema rather than ignoring them.

## 13. Validation additions

Manifest compilation rejects:

- unknown capture, pipe, `cwd`, or tempdir keys;
- invalid or dynamic environment names;
- duplicate normalized producer names in one binding scope;
- zero or excessive capture limits;
- standard-output capture combined with another standard-output sink;
- `pipe: stderr` combined with another standard-error sink;
- any pipe on the terminal stage;
- a downstream stage with both a predecessor pipe and explicit `stdin`;
- two predecessor streams targeting one standard input;
- a pipeline crossing a rule, script, or legacy boundary;
- a tempdir mapping containing fields other than optional `env`; and
- an action plan whose runner schema cannot represent the required variants.

Runtime validation rejects:

- missing or empty environment-selected directories;
- invalid UTF-8 or NUL in captured values or directory variables;
- workspace escape from text-selected `cwd`;
- missing or non-directory paths;
- capture overflow;
- invalid captured UTF-8;
- secure temporary-directory creation or permission failure; and
- incomplete secure cleanup.

## 14. Diagnostics and observability

Every failure identifies:

- enclosing action, target, or rule;
- command-list item and pipeline stage;
- source file and span;
- selected stream or working-directory source;
- binding name where applicable;
- failure category; and
- include or bundle provenance.

Human output may display a workspace-relative path when useful.
Runtime-generated secure temporary paths and captured values are redacted by
default.

Bounded telemetry may record:

- capture requested and result category;
- size bucket, not exact content;
- selected pipeline stream;
- literal, environment, or secure-tempdir `cwd` source;
- command-scoped or sequence-scoped tempdir lifetime; and
- cleanup result category.

Binding names, values, arbitrary paths, tag names, and command output must not
be metric labels.

## 15. Security properties

This amendment provides these properties:

- stdout capture cannot change argv or manifest structure;
- captured text does not mutate Netsuke's parent environment;
- capture is bounded before decoding;
- captured text cannot manufacture an external directory capability;
- stderr piping uses owned process handles rather than shell descriptor syntax;
- secure tempdirs are private before child access;
- temporary paths are unpredictable and never reused deliberately;
- cleanup uses retained directory authority and does not follow symlinks; and
- runtime-generated values do not enter graph fingerprints or metadata by
  accident.

The amendment does not make executed programs trustworthy. A child retains its
normal process and filesystem authority beyond the selected working directory
unless another sandbox policy restricts it.

## 16. Compatibility and migration

Existing RFC 0001 forms remain valid:

- `stdout: path` retains file-redirection semantics;
- `pipe: false` and `pipe: true` retain their meanings, with `true` normalized
  to `stdout`;
- literal string `cwd` retains RFC 0009's semantics; and
- legacy command strings and command-string lists remain unchanged.

Mechanical migrations include:

```yaml
# Before
command: WORKSPACE_DIR=$(tool locate) && cd "$WORKSPACE_DIR" && cargo test

# After
command:
  - invoke: tool locate --relative
    stdout:
      env: WORKSPACE_DIR
  - invoke: cargo test
    cwd:
      env: WORKSPACE_DIR
```

and:

```yaml
# Before
command: compiler source.rs 2>&1 | normalizer

# After, when diagnostics are written only to stderr
command:
  - invoke: compiler source.rs
    pipe: stderr
  - invoke: normalizer
```

The latter is not equivalent to merged `2>&1`; standard output remains a
separate stream. A manifest requiring merged streams must retain explicit shell
mode until a later RFC defines it.

## 17. Test strategy

The implementation must include:

### Standard-output capture

- default one-line-ending chomp and `chomp: false` preservation;
- empty output;
- embedded newlines;
- CRLF and LF endings;
- invalid UTF-8 and NUL rejection;
- exact limit, one byte over limit, and bounded process termination;
- value redaction in human and JSON diagnostics;
- binding commit only after success;
- duplicate producer-name rejection; and
- Windows case-insensitive name collisions.

### Environment-selected `cwd`

- inherited, captured, explicit-overlay, and directory-binding sources;
- precedence between sequence bindings and command overlays;
- relative workspace path success;
- absolute and symlink escape rejection for text values;
- absolute secure-tempdir capability success;
- missing, empty, non-directory, invalid UTF-8, and NUL failures; and
- relative executable resolution from the selected directory.

### Standard-error pipelines

- raw byte preservation;
- standard output remaining independent;
- conflict with standard-error file redirection;
- conflict with downstream explicit standard input;
- final-stage rejection;
- producer and consumer failure propagation;
- cancellation and reaping; and
- capture-stdout plus pipe-stderr coexistence.

### Secure temporary directories

- owner-only Unix permissions before child access;
- restricted Windows access control;
- unpredictable unique names under concurrency;
- command-scoped and sequence-scoped lifetime;
- use across adjacent commands and pipeline stages;
- creation failure before any pipeline spawn;
- cleanup after success, failure, cancellation, timeout, and spawn error;
- no symlink following during recursive removal;
- cleanup failure as primary or secondary error;
- detached descendant or open-handle failure diagnostics; and
- no leakage of absolute temporary paths into plans, snapshots, or metrics.

Property tests should generate bounded command sequences and assert:

- a binding is visible only after its producer commits;
- no text binding acquires directory capability;
- every created secure tempdir has exactly one cleanup owner; and
- pipeline stream selection has at most one source for each downstream stdin.

A bounded Kani harness may model binding reservation, commit, command failure,
and cleanup state transitions.

## 18. Alternatives considered

### 18.1 Shell command substitution

This is concise but reintroduces shell parsing, platform differences, and
unbounded implicit capture. Rejected for structured mode.

### 18.2 Write every discovered value to a file

Files are appropriate for declared artefacts and binary data, but excessive for
small action-local text values. Rejected as the only mechanism.

### 18.3 Allow captured output in manifest-time Jinja

Runtime values do not exist when the static Ninja graph is compiled. Rejected.

### 18.4 Merge stdout and stderr by default

Merging destroys stream identity and changes ordering semantics. Rejected; this
amendment selects exactly one pipeline source.

### 18.5 Treat every environment-selected absolute path as trusted

Environment variables are mutable ambient strings and captured output is
untrusted child data. Rejected in favour of typed directory bindings.

### 18.6 Implement secure tempdirs through `mktemp`

That depends on shell availability, command variants, umask, quoting, output
capture, and manual cleanup. Rejected in favour of runner-owned filesystem
operations.

### 18.7 Keep failed tempdirs automatically

This is convenient for debugging but leaks potentially sensitive inputs and
makes cleanup non-deterministic. Rejected from the initial surface.

## 19. Open questions

- Should a future field expose a captured runtime binding directly as one argv
  element without routing through the child environment?
- Should environment overlays later accept a typed reference to an existing
  runtime binding rather than only compile-time strings?
- Should secure tempdirs support an explicit capability-scoped root selected by
  repository policy?
- Should a future debug mode retain a failed tempdir after an explicit consent
  gate and print its path only to a protected diagnostic sink?
- Should merged stdout and stderr become a separate pipeline source, or remain
  an explicit shell-only operation?

These questions do not alter the initial requirements for bounded capture,
typed directory authority, parent-environment isolation, or mandatory cleanup.

## 20. Recommendation

Amend RFC 0001 with action-local runtime bindings, bounded
stdout-to-environment capture, explicit standard-error pipelines,
environment-selected working directories, and runner-owned secure temporary
directories.

Together, these features replace several high-value shell idioms without
weakening the structured-command trust boundary. They also create a coherent
execution context for downstream Netsukefile migration: values may flow between
ordered commands, but they remain typed, scoped, bounded, and incapable of
silently granting authority.
