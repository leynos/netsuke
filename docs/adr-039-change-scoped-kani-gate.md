# Architecture decision record (ADR) 039: Run the Kani proofs on a pull request only when their inputs change

## Status

Accepted.

## Date

2026-09-23

## Context and problem statement

The `kani-smoke` job in `.github/workflows/ci.yml` runs `make kani-ir`, the 15
bounded Kani harnesses in `src/ir/from_manifest_verification.rs`,
`src/ir/cycle_verification.rs` and `src/ir/cmd_interpolate/verification.rs` (see
[ADR-004](adr-004-bound-kani-ir-harnesses-to-small-n.md)). Until this decision
it ran them on every pull request. From 1 to 21 September 2026 that was 534
runs and 3,696 runner minutes, about seven minutes a run, and most of those
pull requests changed nothing a proof reads: documentation, workflows for other
jobs, the command-line interface, the runner.

`kani-smoke` is a required check on the repository ruleset. GitHub treats a
required check that never reports as pending, so a pull request that skips the
job never becomes mergeable. The usual ways of skipping work, a `paths` filter
on the trigger or an `if:` on the job, therefore cannot be used.

The proofs must stay trustworthy. A pull request that could change a proof's
outcome must run it before it merges, and anything the pull-request decision
misses must still be verified soon after.

## Decision drivers

- `kani-smoke` must report on every pull request, since it is required.
- A pull request touching any proof input must run every harness.
- The set of proof inputs must be derived from the source, not written down
  once and trusted, and adding a harness anywhere must fail a contract until
  the set covers it.
- `main` must be verified in full on every push and at least nightly.
- The mechanism must be reusable by other repositories adopting Kani.

## Options considered

- **Keep running every harness on every pull request.** Correct, and costs
  about seven minutes on every run of every pull request.
- **A `paths` filter on the trigger, or a job-level `if:`.** Refused: the
  required check would never report on a skipped pull request.
- **A path-filter action inside the job.** Works, but adds a third-party
  action to a required job and moves the path set into workflow YAML where no
  contract can derive it from the source.
- **An in-repository decision step reading a derived, contract-held scope.**
  Chosen.

## Decision

`kani-smoke` always runs and always reports. Its steps are, in order: the
checkout (with `fetch-depth: 2`), `astral-sh/setup-uv`, and **Decide Kani proof
scope**, which runs `uv run --script scripts/kani_proof_scope.py` with
`INPUT_EVENT_NAME` set from `github.event_name`. The script writes
`run-proofs=true` or `run-proofs=false` to the step outputs, and every later
step carries `if: steps.scope.outputs.run-proofs == 'true'`. When the proofs
are skipped the job ends green, and the script says why in the job summary and
as a `::notice::` annotation.

**What runs in full, and when.**

- Every push to `main` runs every harness, whatever it changed.
- A nightly `schedule` trigger at 04:41 UTC runs every harness, clear of the
  03:05 UTC mutation-testing run. `build-test` and `windows` carry
  `if: github.event_name != 'schedule'`, so the nightly run is the Kani job
  alone.
- A manual `workflow_dispatch` runs every harness.
- On a pull request the change set is the diff between the merge commit
  `actions/checkout` checks out and its first parent, the base branch tip, read
  with `--no-renames` so that a file moved out of the scope counts under both
  paths. When HEAD is not a two-parent merge, or git fails, the change set is
  unreadable and every harness runs. The proofs run when any changed path falls
  in the scope, and skip otherwise.
- A scope file that is missing or malformed fails the step, rather than
  deciding from it.

**The scope.** `tools/kani/proof-scope.toml` holds two lists. An entry ending in
`/` covers a directory; any other entry names one file.

- `sources` is the module closure of the harnesses. The contract
  `tests/workflow_contracts/kani_proof_scope_test.py` recomputes it from the
  Rust source on every run, reading the library's module tree
  (`tests/workflow_contracts/rust_module_graph.py`) and closing over it
  (`tests/workflow_contracts/rust_module_closure.py`). It seeds from every
  compiled file whose code names `kani` (a `#[kani::proof]` harness, a
  `cfg(kani)` site or a `kani::` call) or declares a `#[global_allocator]`, and
  follows:

  - `crate::`, `super::`, `self::` and `$crate::` paths, and `{…}` use groups,
    to the module named and every compiled module beneath it;
  - bare `child::` paths to a module declared in the same file;
  - `name!` invocations to every file defining `macro_rules! name`;
  - `impl` items whose header names a type or trait the closure defines, since
    coherence lets an impl live anywhere in the crate;
  - `include_str!`, `include_bytes!` and `include!` to the file a literal names,
    or to the directory of a `concat!`'s leading literal;
  - and, at the end, each reached module's declaring ancestors, whose `mod`
    attributes decide whether and how it compiles.

  Every rule over-approximates. `#[cfg(test)]` modules and inline
  `#[cfg(test)] mod … { … }` bodies are left out because `cargo kani` compiles
  without `cfg(test)`; a contract refuses `--tests` in the Makefile's Kani
  invocation and a `tests` key in `[package.metadata.kani.flags]`, which is the
  change that would make that unsound. A `mod name;` nested in an inline module
  body, a missing module file, and an include whose path is not a literal are
  refused, so a layout the reader does not model fails the contract instead of
  shrinking the scope.

- `infrastructure` names what builds and runs the proofs, which no source
  closure can find: `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`,
  `build.rs`, `.cargo/`, `tools/kani/`, `.github/actions/kani-cache/`, the
  `Makefile`, `.github/workflows/ci.yml`, and the decision script itself. The
  contract requires each of them.

The contract fails when the closure reaches a path the scope does not cover,
when any `#[kani::proof]` file anywhere in the repository is outside the scope,
when a required infrastructure entry is dropped, and when a `sources` entry
covers a compiled file the closure does not reach or covers nothing it reaches.
The last two keep the scope from quietly widening to the whole crate, which
would satisfy every sufficiency check and run the proofs on every pull request
again. At adoption the closure is 43 paths: `src/ir/`, `src/ast/`,
`src/ninja_gen/` and six of its sibling `src/ninja_gen_*.rs` modules, the
localization modules and `locales/`, `src/hasher.rs`, `src/hex.rs`,
`src/recipe_shell.rs`, and the crate root.

## Consequences

- A pull request that touches no proof input finishes `kani-smoke` in the time
  it takes to check out, install uv and run the script, and reports success. Of
  the 300 most recent merges to `main` before adoption, 129 (43%) touched no
  path in the scope.
- `Cargo.lock`, `Cargo.toml`, `ci.yml` and the `Makefile` are in the scope as
  whole files, so a dependency bump or a CI change runs the proofs even when it
  cannot affect them. That is the conservative side of the trade and the
  largest source of full runs.
- The pull-request decision covers what a proof verifies, not every way the
  crate can fail to compile under Kani's bundled toolchain
  (`nightly-2025-11-21` for Kani 0.67.0). A pull request outside the scope that
  uses a language feature newer than that toolchain would still pass
  `build-test` and skip the proofs, and the failure would appear on the push to
  `main` that follows it. The push-to-`main` and nightly runs exist to bound
  that window.
- Adding a harness, or a `cfg(kani)` site, outside the current closure fails
  the contract and prints the paths to add. Moving code so that a harness no
  longer reaches a file fails it too, and prints the entry to remove.

## Reuse in other repositories

The mechanism is repository-independent apart from the scope file. A step by
step adoption guide lives outside this repository, in the estate's
`kani-change-scoped-gate.md` note; the parts to copy are
`scripts/kani_proof_scope.py`, the two module readers and their tests, the
scope contract with its `REQUIRED_INFRASTRUCTURE` list rewritten for the
adopting repository, and the job shape above.
