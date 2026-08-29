# Architecture decision record (ADR): Environment seam taxonomy

## Status

Accepted.

## Date

2026-08-06

## Context and problem statement

`clippy.toml` disallows `std::env::var`, `var_os`, `set_var`, `remove_var`,
`vars`, and `vars_os` across the workspace, per the testing mandate in
`AGENTS.md`: behaviour that depends on an environment variable should accept
that value as an argument rather than read the process directly. Several
callers have satisfied that mandate with different shapes — a bare closure
parameter, a mockable trait object, a shared `Arc`-wrapped reader — chosen
independently as each site migrated off the disallowed calls under issues 484,
488, and 493. Without a stated taxonomy, a future migration has no way to pick
the right shape for a new boundary, and a reviewer lacks a yardstick for
whether a proposed seam is over- or under-engineered for its call-site count.

Netsuke's design record (`docs/netsuke-design.md`) does not describe these
seams. This ADR fills that gap and gives `docs/developers-guide.md`'s
"Environment and template ports", "Environment lookup seams", and "Manifest
`env()` reader" sections a single decision record to point back to.

## Decision

Adopt three seam shapes, selected by how many call sites a boundary has and
whether it is expected to grow:

- **Narrow closure seams**, for a single variable read by a single caller.
   The module owns a private function that takes an
   `FnOnce(&str) -> Result<String, env::VarError>` (or the equivalent
   `OsString`-typed form) instead of calling `std::env::var` itself. Examples:
   the `resolve_with` variants in `output_mode.rs` and `output_prefs.rs`
   described earlier in the developer guide. A related but distinct pattern
   injects a resolved *value* rather than a closure: the `stdlib::path`
   home-directory resolver's `HomeDirectory` enum (`Ambient`/`Missing`/
   `Explicit`) lets a caller supply the home directory directly, so the
   process-reading `home_from_env` ladder in `src/stdlib/path/path_utils.rs`
   remains a directly annotated composition root rather than gaining its own
   `_with` closure parameter.
- **The `mockable::Env` trait**, for a boundary mocked across many tests or
   expected to grow further inputs. `resolve_ninja_program_utf8_with` in
   `src/runner/process/ninja_program.rs` takes `&impl Env`; production supplies
   `mockable::DefaultEnv`, and tests supply `mockable::MockEnv` for every
   resolution branch without mutating the process (#488).
   `stdlib::which::env::EnvSnapshot::capture_with_env` takes the same
   `&impl Env` and reads `PATH`, `PATHEXT`, and `NETSUKE_WHICH_WORKSPACE`
   through it, so one provider covers every ambient input the resolver has
   (#487).
- **`EnvReader` `Arc` closures**, for a boundary whose registration point
   requires `Send + Sync`. The manifest `env()` Jinja helper
   (`src/manifest/env_reader.rs`) reads through an injected `EnvReader`, a
   shared `Fn(&str) -> Result<String, EnvReadError>` (a manifest-owned error
   type distinguishing an absent variable from a non-UTF-8 one, so the helper
   does not expose the process adapter's `VarError`); `minijinja` requires
   registered functions to be `Send + Sync`, so the reader is captured as an
   `Arc` by the registered closure rather than borrowed (#484).

None of these shapes is a general-purpose environment service. Each is owned by
the module that reads its variable, stays private to it, and covers one
variable or one precedence ladder; see "Environment and template ports" in
`docs/developers-guide.md` for the composition rules that apply to all three,
and "Ownership and permitted call sites" under that guide's "Manifest `env()`
reader" section for the `EnvReader` shape specifically.

### `EnvSnapshot` ownership

`stdlib::which::env::EnvSnapshot::capture` is the resolver's single ambient
boundary: it captures `PATH`, `PATHEXT`, and the `NETSUKE_WHICH_WORKSPACE`
switch as *data*, in one place, rather than letting each downstream decision
read the process independently. Absence and malformed-UTF-8 outcomes are
stored, not resolved, at capture time.

Capture is also the only place the platform's `std::env::VarError` is spoken.
It translates the reading into the `WorkspaceSwitch` domain state (`Value`,
`Absent`, `NotUnicode`) and emits the non-UTF-8 warning there, so the policy
behind the boundary carries neither the platform error type nor a logging
dependency. `workspace_switch.rs` holds only the variable name and that state,
making it a leaf module: it is used by `env` and by `lookup::workspace`, and it
calls back into neither, so there is no environment-to-lookup cycle.

The which-resolver `CacheKey` incorporates every input the snapshot captured,
though not uniformly: `cwd` is stored directly as its own field, while
`stdlib::which::cache::env_fingerprint` hashes `raw_path`, `raw_pathext`, and
the `WorkspaceSwitch` state, which derives `Hash` precisely so it can be hashed
directly. Either way, two resolutions that differ only in one captured
environment input cannot share a cache entry.

### Explicit child-environment composition

Tests that need a controlled child-process environment configure the child
explicitly; nothing sanctioned mutates the parent test process's environment to
influence a spawned `netsuke` binary. `test_support::netsuke`'s
`run_netsuke_in_with_env` calls `env_clear()` on the constructed
`assert_cmd::Command`, forwards the host `PATH`, and then applies the caller's
`extra_env` pairs through `Command::env`. The BDD helper
`build_netsuke_command` follows the same pattern: it clears the inherited
environment and forwards only `PATH` and the scenario's tracked
`env_vars_forward` map, one `cmd.env(key, value)` call per entry. Since #493,
nothing reads `NETSUKE_NINJA` (or any other override) from the parent process
to populate a child; the value always travels explicitly through `extra_env` or
`env_vars_forward`.

Subprocess isolation constructed this way — explicit `Command::env` calls
against a cleared child environment — is the only sanctioned route for getting
an ambient-looking variable such as `NETSUKE_NINJA` or `PATH` to a spawned
process under test. Mutating the test process's own environment to achieve the
same effect is not an accepted alternative to any of the three seam shapes
above.

Composition does not stop at the child-process boundary: in-process callers
that need a deterministic Ninja executable use `runner::run_with_ninja_program`
to supply the already-resolved program path directly, bypassing `NETSUKE_NINJA`
resolution entirely rather than setting the variable for a child to read.

## Rationale

- **Proportionate abstraction.** A trait object for a single-variable,
  single-caller boundary would recreate the ambient coupling the seam exists to
  remove, just one layer down; a bare closure is cheaper to read and to test.
  `mockable::Env` earns its weight only when a boundary is exercised by many
  tests or is expected to grow (#488).
- **`Send + Sync` is a real constraint, not a preference.** `EnvReader`'s
  `Arc` wrapping is not a stylistic choice; `minijinja`'s function-registration
  API requires it, and a plain closure parameter cannot satisfy that bound when
  the closure must be captured by a registered, potentially cloned function
  (#484).
- **Data over decisions at the boundary.** Capturing `EnvSnapshot` once and
  deriving decisions downstream keeps the resolver testable without process
  mutation, and keeps `workspace_switch` a leaf module rather than a second
  place that reads the process.
- **Cache correctness follows from capture completeness.** Hashing every
  captured input, including the workspace switch, is what prevents a resolution
  made with the fallback enabled from answering a lookup made with it disabled.
- **No back door around subprocess isolation.** Explicit `Command::env`
  composition is auditable per test and cannot race with parallel test
  execution the way a shared-process mutation could.

## Consequences

- A contributor introducing a new environment-dependent boundary chooses
  among the three seam shapes by call-site count and `Send + Sync`
  requirements, rather than inventing a fourth shape or reaching for the
  heaviest option by default.
- `docs/developers-guide.md`'s "Environment and template ports", "Environment
  lookup seams", and "Manifest `env()` reader" sections, and this ADR must stay
  consistent; widen one only alongside the others when a boundary's shape
  changes.
- Reviewers can reject a new `mockable::Env`-shaped boundary for a
  single-variable, single-caller site, and a new closure-shaped boundary for a
  site that clearly needs `Send + Sync` registration or broad mocking.
- Any future ambient input added to the which resolver's boundary (a new
  environment variable, for example) must be folded into `EnvSnapshot` and into
  `env_fingerprint`, not read independently downstream, to preserve the
  no-cycle and cache-correctness properties this ADR records.

## Alternatives considered

- **A single shared `Env` trait for every boundary.** Rejected: forcing
  `mockable::Env` (or an equivalent trait object) on single-variable,
  single-caller sites such as `output_mode.rs`'s `resolve_with` would add
  indirection with no matching test-surface benefit, and would blur the "one
  variable or one precedence ladder" ownership rule this ADR reaffirms.
- **Reading the parent process's environment for child-process tests.**
  Rejected: mutating the test process to influence a spawned `netsuke` binary
  reintroduces the shared-mutable-state races that injected readers and
  child-process configuration exist to avoid, and it is exactly the pattern
  #493 removed from the BDD and integration test helpers.
  `.config/nextest.toml` runs no serialized environment group precisely because
  no sanctioned test still mutates the harness environment.

## Implementation references

- Workspace switch state:
  [`src/stdlib/which/workspace_switch.rs`](../src/stdlib/which/workspace_switch.rs)
- `EnvSnapshot`: [`src/stdlib/which/env.rs`](../src/stdlib/which/env.rs)
- Cache fingerprint: [`src/stdlib/which/cache.rs`](../src/stdlib/which/cache.rs)
- `mockable::Env` seam:
  [`src/runner/process/ninja_program.rs`](../src/runner/process/ninja_program.rs);
  `runner::run_with_ninja_program` in
  [`src/runner/mod.rs`](../src/runner/mod.rs) is the companion injected seam
  that lets callers select the resolved Ninja executable directly, without
  going through `NETSUKE_NINJA` resolution at all
- `EnvReader`: [`src/manifest/env_reader.rs`](../src/manifest/env_reader.rs)
  (manifest `env()` Jinja helper)
- Child-environment composition:
  [`test_support/src/netsuke.rs`](../test_support/src/netsuke.rs)
  (`run_netsuke_in_with_env`) and `tests/bdd/steps/manifest_command_helpers.rs`
  (`build_netsuke_command`)
- Policy narrative: "Environment and template ports" and "Injected and
  child-process environments" in
  [`docs/developers-guide.md`](developers-guide.md)

## Addendum

### 2026-08-26: EnvLock retirement

`EnvLock` is retired rather than hardened. Production signatures must inject
`mockable::Env`, with `mockable::DefaultEnv` at production boundaries and
`mockable::MockEnv` in tests. CWD callers must use the existing
working-directory seam, absolute paths, or the `-C/--directory` route.

Manifest parsing owns a separate base-directory seam: it passes the manifest
directory or workspace root to `glob_paths(pattern, base)` and internal
`expand_glob(pattern, base)`. Relative glob patterns, including parent-relative
ones, resolve from that injected root and retain their pattern-relative result
spelling; absolute patterns remain absolute. This path neither reads nor
mutates process-global working-directory state during expansion.

Migrations are tracked in issues #491, #492, and #493; removal is tracked in
issue #494. No new `EnvLock` callers or synchronization tests are permitted.

### 2026-08-25: BDD isolation routes

Issue #492 applied this taxonomy to `rstest-bdd`, whose steps execute in the
test-harness process. The suite now uses two routes:

- **Route A — isolated child.** An end-to-end scenario invokes `netsuke` with
  `assert_cmd`, clears the child environment, and supplies required values
  through `Command::env`.
- **Route B — injected environment.** A scenario calls an in-process library
  entry point with its injected environment and asserts values such as `Cli`,
  `Manifest`, `BuildGraph`, or rendered output.

The BDD suite no longer uses `EnvLock` or `CwdGuard` to coordinate
process-global environment or working-directory changes. Route B avoids CWD
changes by passing absolute paths or preserving `-C/--directory` for manifest
lookup and automatic project discovery. Explicit relative `--config` and
`NETSUKE_CONFIG` selectors remain independent of `-C/--directory` and resolve
from the process working directory. Absolute selectors remain unchanged.
