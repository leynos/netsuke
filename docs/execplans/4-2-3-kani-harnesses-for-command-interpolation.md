# Add Kani harnesses for command interpolation (roadmap 4.2.3)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: IN PROGRESS

Revision 2.22. See `Revision note` at the foot of this document.

## Purpose / big picture

Netsuke turns a YAML manifest into a Ninja build file. Along the way it must
turn manifest placeholders into internal `INS_TOKEN` and `OUTS_TOKEN` markers,
then lower those markers to a concrete shell command with the target's input
and output paths. Literal `$in` and `$out` remain shell variables. This
substitution is security-sensitive: it decides which parts of an author's
command text are rewritten and which are left alone, and it is the last place
that rejects a command whose shell syntax has been damaged by the substitution.

The command-interpolation property suite now supplements the unit and
integration examples with adversarial templates containing `$`, backticks,
quotes, backslashes, and spaces. Its dense and sparse strategies cover up to
256 characters and eight placeholders, while the Kani harnesses exhaustively
cover the two feasible per-position marker decisions.

After this change a maintainer will run one command:

```bash
make kani-ir
```

and see the bounded model checker Kani prove — exhaustively, over every input
its bound admits — the two feasible placeholder kernels:

1. Literal `$in` and `$out` are left unchanged as shell variables;
2. the internal marker tokens match exact text, including when adjacent to
   identifier characters.

The production scanner and guard exceed the five-minute resource cap even at
small string bounds. Independent Proptest properties therefore cover templates
up to 256 characters with at most eight placeholders: scanner agreement, POSIX
protected-placeholder rejection, odd-backtick rejection, and guard placement on
the substituted command.

"Exhaustively, within a bound" is the important phrase. Kani does not sample.
For every input inside the bound it either proves the property or produces a
concrete counterexample. The shell-variable proof quantifies over every symbolic
`$` position in its bounded window; the marker proof drives both real marker
constants through the production recognizer.

This plan adds no independent user-visible behaviour. During the final rebase,
the target branch's security change began rejecting POSIX placeholders inside
backticks rather than silently preserving them; this plan retains that stronger
contract and updates its Proptest oracle accordingly. PowerShell keeps its
native backtick-escape semantics. The maintainer-facing change remains two more
verified harnesses, guide inventory rows, and a mutation-evidence contract that
cannot silently rot.

## Context and orientation

Assume you have never opened this repository before. This section gives you
everything you need.

### What Netsuke is

Netsuke is a Rust command-line build tool. It reads a manifest (a
`Netsukefile`, written in YAML with Jinja templating), lowers it into an
internal build graph, and writes a `build.ninja` file that the Ninja build tool
executes. The pipeline has four stages:

1. `src/manifest/` — parse the YAML, expand `foreach` and `when`, render Jinja.
2. `src/ir/` — lower the parsed manifest into an *intermediate representation*
   (IR): a backend-agnostic build graph. "IR" means exactly that graph.
3. `src/ninja_gen.rs` and siblings — serialize the IR into Ninja syntax.
4. `src/runner/` — invoke Ninja and report progress.

This plan touches stage 2, including the command-interpolation module and its
test-only sibling modules.

### The file this plan is about

`src/ir/cmd_interpolate/mod.rs`, `substitution.rs`, and `verification.rs`. Read
the module and its siblings before doing anything else.

The Proptest entry point is `src/ir/cmd_interpolate_property_tests.rs`; its
test-only sibling `src/ir/cmd_interpolate_property_support.rs` owns shared
generators, specifications, and assertions. Neither property-test support nor
the Kani verification module is a production dependency.

- `CommandBindings` in `mod.rs` holds the selected shell and shell-quoted input
  and output path lists for one recipe. Its `new` constructor quotes each
  `Utf8PathBuf` for that shell and joins with spaces.
- `substitute(template: &str, bindings: &CommandBindings)` in `mod.rs`
  collects the template into a `Vec<char>` and delegates the one-pass scan to
  `SubstitutionTraversal` in `substitution.rs`. The traversal tracks the
  selected shell's backtick semantics and asks `find_substitution` whether a
  placeholder starts at each position.
- `find_substitution(chars: &[char], pos) -> Option<(Placeholder, usize)>` in
  `mod.rs` is the per-position decision. It takes a `&[char]` slice and
  **allocates nothing**. It recognizes only the two internal markers `INS_TOKEN`
  (`__NETSUKE_INS_PLACEHOLDER__`) and `OUTS_TOKEN`
  (`__NETSUKE_OUTS_PLACEHOLDER__`), which `src/manifest/render.rs` emits for
  `{{ ins }}` and `{{ outs }}`. Literal `$in` and `$out` are not Netsuke
  placeholders.
- `find_substitution` first checks that the current character is `_`, preserving
  the invariant that both machine-generated markers begin with an underscore,
  then tries the two exact marker matches.
- `try_match_token(chars, pos, token: &str, replacement)`
  matches the internal markers. It applies **no** boundary rule and requires
  **no** `$` prefix, so `x__NETSUKE_INS_PLACEHOLDER__` *is* rewritten when the
  marker begins at the underscore. It takes the token as a parameter, and
  `find_substitution` reaches it only after its underscore-prefix guard.
- `has_unmatched_backticks(s)` returns true when the backtick count is odd for
  POSIX-compatible command routes.
- `interpolate_command_with_bindings(template, bindings)` is
  the production entry point. It substitutes first, then applies the selected
  shell's guard to the result. POSIX-compatible routes reject an odd
  `has_unmatched_backticks(&interpolated)` count or
  `shlex::split(&interpolated).is_none()`; PowerShell retains its native
  backtick-escape semantics. The order matters: the guard runs on the
  substituted string.
- `interpolate_command_with_shell` is `#[cfg(test)]` only and supplies the
  explicit shell used by structural tests. It is **not** available under
  `cfg(kani)`. Harnesses use `interpolate_command_with_bindings` or call the
  private matchers directly.

The only production caller is `register_action` in
`src/ir/from_manifest_support.rs` lines 26-80.

### Behaviour you must know before writing any assertion

These follow from the rules above. Each was hand-derived and each must appear
as a characterization test in EP-M1. If any turns out to be false, stop: your
model of the code is wrong.

| Input                                    | Output                | Why                                                               |
| ---------------------------------------- | --------------------- | ----------------------------------------------------------------- |
| `cp $in $out`                            | unchanged             | literal shell variables are not Netsuke placeholders              |
| `$input`                                 | unchanged             | no internal marker begins at the dollar sign                      |
| `echo __NETSUKE_INS_PLACEHOLDER__`       | `echo <ins>`          | the exact input marker is lowered                                 |
| `x__NETSUKE_INS_PLACEHOLDER__`           | `x<ins>`              | markers have no identifier-boundary rule                          |
| `` echo `__NETSUKE_INS_PLACEHOLDER__` `` | rejected on POSIX     | a marker inside a protected backtick region cannot safely lower   |
| `` echo `__NETSUKE_INS_PLACEHOLDER__ ``  | rejected on POSIX     | the substituted command has an odd backtick count                 |
| `` echo `__NETSUKE_INS_PLACEHOLDER__` `` | lowered on PowerShell | PowerShell treats backticks as native escapes, not protected text |

### What Kani is and how this repository already uses it

Kani is a bounded model checker for Rust. Annotate a function with
`#[kani::proof]`, build symbolic inputs with `kani::any()`, constrain them with
`kani::assume(...)`, and assert properties. Kani compiles to a logical formula
and asks a SAT solver whether any admissible input violates an assertion.
"Bounded" means loops unroll a fixed number of times (`#[kani::unwind(N)]`) and
data structures get fixed sizes.

Conventions established by roadmap items 4.1.1, 4.1.2, 4.2.1, and 4.2.2, which
you must follow:

- **Version pinning.** `tools/kani/VERSION` contains `0.67.0`. Install with
  `make install-kani`. Never install an unpinned Kani.
- **Where harnesses live.** The production module declares them at its foot:

  ```rust
  #[cfg(kani)]
  mod verification;
  ```

  with bodies in the sibling `*_verification.rs`. See `src/ir/cycle.rs` lines
  380-382 and `src/ir/from_manifest.rs` lines 164-166. The command
  interpolation harnesses live in `src/ir/cmd_interpolate/verification.rs`. The
  split exists because AGENTS.md caps any code file at 400 lines. This plan
  cites Markdown documents by section heading rather than line number, because
  `main` moves those files faster than the numbers can be kept true; line
  numbers are used only for code. The command-interpolation implementation is
  split between `mod.rs` and `substitution.rs`. Verify any anchor by heading or
  symbol name before trusting a number. The repository's sibling-module
  convention for shared production helpers is `<module>_support.rs`; see
  `src/ir/cycle_support.rs`, which holds the `canonicalize_cycle_by` kernel that
  `src/ir/cycle.rs` re-exports.
- **`cfg(kani)` is a build configuration, not a Cargo feature.** It is declared
  in `Cargo.toml` line 254 under `[lints.rust] unexpected_cfgs`. There is no
  `kani` entry in `Cargo.toml`.
- **Default unwind** is set in `Cargo.toml` lines 71-72:
  `[package.metadata.kani.flags] default-unwind = "6"`. The value must be a
  string; the integer form is rejected.
- **Per-harness attributes.** Existing harnesses use `#[kani::solver(kissat)]`
  and `#[kani::unwind(N)]`. Add both to every new harness; kissat is
  substantially faster than the default solver on this workload.
- **Harnesses drive production code.** ADR-004 forbids re-implementing the
  logic under proof inside a harness. A harness-local *oracle* for a
  side-condition is a different thing and is permitted where this plan says so
  explicitly; see `Decision log`.
- **Every harness needs a mutation patch** under `docs/verification/mutations/`,
  named after the harness path with `::` replaced by `__`. Each seeds one
  realistic fault that the harness must reject.
- **Resource capping.** Roadmap 4.2.2 established that uncapped local Kani runs
  OOM-killed developer machines. Every Kani command runs inside the
  `timeout`-plus-`systemd-run` wrapper in `Concrete steps`, with the Kani
  `LD_LIBRARY_PATH` set. Without that `LD_LIBRARY_PATH`, `cargo kani` and Cargo
  build scripts fail to load `libLLVM` with an opaque linker error. This is the
  single most likely place to get stuck.

The pre-existing inventory is thirteen harnesses across
`src/ir/from_manifest_verification.rs` (four) and
`src/ir/cycle_verification.rs` (nine), documented in the "Kani harness
inventory" table of `docs/developers-guide.md`. Before this work there were
twelve mutation patches: the adapter harness
`canonicalize_path_wrapper_matches_u8_kernel_for_two_nodes` has none.

### Continuous integration and its budget

`.github/workflows/ci.yml` defines a `kani-smoke` job, pull requests only,
capped at `timeout-minutes: 20`. Measured on five recent runs the job takes
4:18 to 4:43 total: roughly 55 seconds of setup and version check, 191 to 227
seconds for `make kani-ir`, and 25 seconds of post-steps. Of that
`make kani-ir` time, cross-referencing the per-harness verification times
recorded in the 4.2.2 execplan (4.6s at N=2 up to 11.8s at N=4), only about 45
to 75 seconds is solving; the remaining three minutes is fixed crate
compilation and goto-program instrumentation.

So the real budget for new solving is generous in absolute terms but the cap is
hard, and GitHub's runners are slower per core than the development machine. See
`Tolerances`.

The job's cache key is `hashFiles('tools/kani/VERSION', 'Makefile')` with no
`restore-keys`. Editing the Makefile therefore forces a cold Kani install on
exactly the pull request that is already near budget. Do not edit the Makefile
in this work unless a tolerance forces it.

This is no longer hypothetical: `main` has since edited the Makefile for
unrelated reasons (a `bench-config-load` target and Markdown-discovery
exclusions), so the Kani cache is already cold and the next run of this job
will pay the full install cost. Treat the first measured timing as pessimistic
and re-measure on a second run before concluding anything about the budget.
`main` has also added a `build-test-windows` job that gates merges; it is
separate from `kani-smoke` and does not share its budget, but it lengthens
overall pull-request turnaround. Adding `restore-keys` to the Kani cache
remains the cheapest fix and is listed as a contingency lever, not part of this
work.

### Terms used in this plan

- **Placeholder.** One of the internal `INS_TOKEN` or `OUTS_TOKEN` markers.
- **Shell variable.** Literal `$in` and `$out` text, which remains unchanged.
- **Marker form.** The two internal tokens, which match exact text without an
  identifier-boundary rule.
- **Backtick region.** Characters between an odd-numbered backtick and the next
  one, as tracked by `in_backticks`.
- **Guard.** The rejection condition in `interpolate_command_with_bindings`.
- **Oracle.** A small function written inside a harness that computes a side
  condition independently of the production code, so an assertion can compare
  against something other than the code under test.
- **Non-vacuity.** Evidence that a passing verification could have failed: a
  mutation patch plus reachable-witness covers.

## Signposts: documentation and skills

Read these before starting. They are not optional background.

Repository documents:

- `AGENTS.md` — mandatory gates, file-size cap, commit style, spelling policy.
- `docs/adr-004-bound-kani-ir-harnesses-to-small-n.md` — the governing decision
  on Kani bounds and production-owned kernels.
- `docs/developers-guide.md`, sections "Command and recipe lowering" and
  "Formal-verification tooling".
- `docs/formal-verification-methods-in-netsuke.md` — the design rationale.
- `docs/execplans/4-2-2-kani-harnesses-for-cycle-canonicalization.md` — the
  closest comparable job. Its Decision log records the resource-cap wrapper and
  the `LD_LIBRARY_PATH` workaround. Read those two entries at minimum.
- `docs/documentation-style-guide.md` — en-GB-oxendict spelling, sentence-case
  headings, 80-column prose, ADR format.
- `docs/rust-testing-with-rstest-fixtures.md` and
  `docs/rust-doctest-dry-guide.md` — test-authoring conventions.
- `docs/reliable-testing-in-rust-via-dependency-injection.md` — why the
  production seam in EP-M1 is shaped the way it is.

Skills to load:

- `rust-router` first, then `kani` for harness authoring and `proptest` for the
  EP-M5 hand-off.
- `rust-unit-testing` for the characterization cases (rstest, googletest,
  pretty_assertions).
- `rust-verification` if you need to re-argue a method choice.
- `leta` for navigating symbols rather than grepping.
- `hexagonal-architecture` only to check the boundary claim in
  `Interfaces and dependencies`; this task does not adopt the pattern.

## Conformance basis

There is no Terms of Reference document in this repository. Upstream artefacts:

- `docs/roadmap.md`, item 4.2.3 and its four sub-items, referred to as
  `RM-4.2.3.a` through `RM-4.2.3.d`.
- `docs/formal-verification-methods-in-netsuke.md`, section "Kani for command
  interpolation" — the design basis, `FV-CMD`.
- `docs/adr-004-bound-kani-ir-harnesses-to-small-n.md` — `ADR-004`.

Trace links:

```plaintext
RM-4.2.3.a -> FV-CMD -> EP-M2 -> ir::cmd_interpolate::verification::shell_variable_prefix_does_not_match
RM-4.2.3.a -> FV-CMD -> EP-M2 -> ir::cmd_interpolate::verification::marker_token_match_is_exact
RM-4.2.3.b -> FV-CMD -> EP-M5 -> ir::cmd_interpolate::property_tests::scanner_agrees_with_independent_specification
RM-4.2.3.c -> FV-CMD -> EP-M5 -> ir::cmd_interpolate::property_tests::substituted_odd_backticks_are_rejected
RM-4.2.3.d -> FV-CMD -> EP-M5 -> ir::cmd_interpolate::property_tests::guard_uses_the_substituted_command
ADR-004    ->           EP-M0 -> docs/adr-004-bound-kani-ir-harnesses-to-small-n.md (4.2.3 extension)
```

Roadmap item 4.4.1 declares `Requires 4.2.3`. This plan must therefore leave
the placeholder contract legible in `docs/developers-guide.md`, even though
writing the user-facing README section is 4.4.1's job.

## Constraints

1. **No user-visible behaviour change.** `interpolate_command_with_bindings`
   must accept and reject exactly the same commands and produce exactly the
   same strings. No pre-existing test may be edited to accommodate the change.
   If one needs editing, that is evidence of a behaviour change: stop.
2. **No public API widening.** `netsuke::ir`'s public surface must not grow.
   `ADR-004` Option D rejected verification ports on that surface. New items are
   `pub(super)` at widest.
3. **No new dependencies.** `Cargo.toml` must not gain an entry.
4. **No capacity limits in production.** Do not introduce a fixed-capacity
   buffer, a maximum placeholder count, or any other new bound into the
   production substitution path. Today it is unbounded; it stays unbounded.
   Revision 1 of this plan proposed such a buffer and it was rejected — see
   `Decision log`.
5. **No unstable Kani features.** No `-Z` flags. `KANI_FLAGS` stays empty by
   default.
6. **Kani stays out of the ordinary gates.** `make test`, `make lint`,
   `make check-fmt`, and `make all` must not invoke Kani.
7. **CI budget.** The `kani-smoke` job must finish inside `timeout-minutes: 20`.
   Measure it; do not assume it.
8. **File-size limit.** No code file may exceed 400 lines.
9. **Spelling and style.** en-GB-oxendict spelling everywhere; 80-column prose.
10. **Pinned toolchain.** Kani stays at 0.67.0.
11. **Resource capping.** Every Kani invocation uses the wrapper in
    `Concrete steps`, with the Kani `LD_LIBRARY_PATH`.
12. **Do not edit the Makefile or the CI workflow** unless a tolerance forces
    it, because the CI cache key hashes the Makefile and has no `restore-keys`.

## Tolerances (exception triggers)

Stop and escalate. Do not work around these.

- **Scope.** More than 16 files changed, or more than 1200 net lines added
  across code, tests, and documentation.
- **Interface.** Any change to a `pub` item in `netsuke::ir`, or any new item
  visible outside `src/ir/`.
- **Dependencies.** Any new entry in `Cargo.toml`.
- **Unstable features.** Any need for `-Z stubbing`, `-Z bounded-arbitrary`,
  `-Z function-contracts`, or any other `-Z` flag.
- **Local verification budget.** If capped `make kani-ir` exceeds **8 minutes**
  locally, stop and report the per-harness timings. Eight, not fourteen: the
  measured CI baseline is 3:45 of which about a minute is solving, GitHub
  runners are roughly 1.3 to 1.8 times slower per core, and the
  runner-to-runner variance is around 30 per cent. Fourteen minutes locally
  would be a coin flip against the 20-minute cap.
- **Measured CI budget.** After the first pull-request run that includes new
  harnesses, read the actual step duration rather than guessing:

  ```bash
  env -u GH_TOKEN gh api "repos/leynos/netsuke/actions/runs/<run-id>/jobs" \
    --jq '.jobs[] | select(.name=="kani-smoke") | .steps[]
          | {name, started_at, completed_at}'
  ```

  If the `Run Kani harnesses` step exceeds **12 minutes**, stop and escalate.
- **Memory.** If any harness is OOM-killed at the 8 GiB cap, record the bound
  at which it died and stop. Do not raise the cap.
- **Bound shortfall.** Mechanical rule: if EP-M0 cannot verify the
  `shell_variable_prefix_does_not_match` shape at a window of at least 8
  characters within 6 minutes, **commit nothing to the working branch** and
  report the measurement table. Do not proceed to EP-M1 on judgement.
- **Iterations.** If a harness still fails after 3 fix attempts, stop and report
  the counterexample rather than weakening the property.
- **Behaviour.** If any pre-existing test requires editing, stop.
- **Ambiguity.** If the marker-only substitution semantics turn out to be
  genuinely ambiguous, record both readings and ask.

## Risks

- **Symbolic `char` is the wrong encoding and will blow the budget.**
  Severity: high. Likelihood: high if not designed against. Kani's
  `Arbitrary for char` generates a `u32` and constrains it away from surrogates
  and out-of-range values: 32 symbolic bits and a validity invariant per
  symbol. Roadmap 4.2.2 already found symbolic `char` and `String` construction
  too expensive and replaced it with symbolic selectors over concrete values.
  Mitigation: this plan **mandates** symbolic `u8`, constrained by
  `kani::assume` to membership in a small set of concrete ASCII byte values,
  then widened with `char::from(b)` — a zero-extension, not a multiplexer. No
  harness may call `kani::any::<char>()` or `kani::any::<[char; N]>()`.

- **Symbolic UTF-8 encoding and decoding is a hidden dominant cost.**
  Severity: high. Likelihood: high if not designed against. `substitute` takes
  `&str` and converts it to a character slice before constructing a
  `SubstitutionTraversal`. Feeding it a symbolic string means symbolically
  encoding UTF-8 in the harness and symbolically decoding it in production,
  with four-way length branching per character. Mitigation: the Kani harnesses
  call the allocation-free `find_substitution` matcher directly. The residual
  properties exercise the real `SubstitutionTraversal` through `substitute`.
  This is a cost decision, not a style one.

- **Anything touching `shlex::split` symbolically is expensive.**
  Severity: high. Likelihood: high. `interpolate_command_with_bindings` calls
  it unconditionally, so no alphabet restriction avoids it — restriction makes
  `shlex` *total*, not absent, and CBMC still executes its state machine and
  builds a symbolic `Vec<String>`. `ADR-004` records a `Utf8PathBuf` proof
  hitting the 8 GiB cap at N=3. Mitigation: only two harnesses touch
  `interpolate_command_with_bindings`, and both target a window of 6 to 8
  characters — enough to exhibit the specific fault each is designed to catch
  (a backtick plus a marker plus a binding-introduced backtick fits in six).
  Longer inputs are handed to Proptest against the real crate in EP-M5.
  Revision 1's INV-GUARD-A/B split is dropped; it paid full price for no saving.

- **The roadmap's 256-character, 8-placeholder bound is not reachable by
  Kani.** Severity: medium. Likelihood: high. Mitigation: the shell-variable
  proof needs only an eight-character window with a symbolic position, while
  the marker proof uses a 32-character window containing the real constants.
  The full scanner and guard retain the genuine resource bound, so EP-M5 hands
  their residual range to Proptest, exactly as `ADR-004` did for roadmap
  4.2.1's unreachable 10-node bound.

- **Marker-form placeholders fall outside every plausible small alphabet.**
  Severity: medium. Likelihood: high. The two real markers are 27 characters
  long and contain uppercase letters, so a small alphabet cannot express them.
  Mitigation: `marker_token_match_is_exact` drives both concrete constants
  through `find_substitution` in a 32-character symbolic window. The proof
  retains the exact-match and boundary-asymmetry checks without introducing a
  proof-only short-token path.

- **Property generators must retain adversarial coverage.** Severity: high.
  Likelihood: medium. The command-interpolation property suite now generates
  `$`, backticks, quotes, backslashes, spaces, dense eight-placeholder
  templates, and sparse templates up to 256 characters. Future simplification
  of those strategies could silently remove coverage for marker pass-through or
  protected contexts. Mitigation: retain the hand-derived characterization
  table, adversarial generators, and residual Proptest properties alongside the
  Kani harnesses.

- **Mutation patches rot silently and nothing detects it.**
  Severity: medium. Likelihood: certain over time. Nothing in the Makefile, the
  workflows, or the test suite applies the patches under
  `docs/verification/mutations/`. The pre-existing inventory had twelve patches
  for thirteen harnesses. Mitigation: EP-M2 adds a contract test that runs
  `git apply --check` over every patch and asserts every `#[kani::proof]`
  harness has a patch or an explicitly justified exemption. This is cheap, runs
  in `make test`, and is the highest-value single addition in this plan.

- **Literal shell variables must remain untouched.** Severity: medium.
  Likelihood: medium. Mitigation: the shell-variable harness and the
  `dollar_prefixed_shell_variables_are_preserved` property cover literal `$in`,
  `$out`, `$ins`, and `$outs`. Changing this behaviour is a behaviour change
  and out of scope here (Constraint 1).

- **`ADR-004` numbering collision.** Severity: low. Likelihood: certain.
  Three files already share the `adr-004` prefix; this is a known accepted
  collision. Extend the existing Kani ADR rather than minting a number.

## Verification plan

### Axioms (assumed, not proved)

- **AXIOM-KANI.** Kani 0.67.0 with kissat is sound for the bounded fragment it
  explores. Kani's own correctness is out of scope.
- **AXIOM-SHLEX.** `shlex::split` (2.0.1) is total from `&str` to
  `Option<Vec<String>>`. Reading its source, `split` returns `None` for exactly
  three conditions: an unterminated `"`, an unterminated `'`, and a trailing
  `\`. A `#` starts a comment and does not error. Its internals are out of
  scope; only Netsuke's use of it is verified.
- **AXIOM-QUOTE.** `shell_quote`'s `Sh` dialect produces a word that a POSIX
  shell expands back to the original path. Out of scope. Note that it emits `'`
  and `\'`, so a path containing a backtick becomes a quoted word that still
  contains a literal backtick character.
- **AXIOM-WINDOW.** The shell-variable harness quantifies over a symbolic `$`
  position across an eight-character array. This covers every bounded position,
  including both truncated ends, and the marker harness separately supplies a
  32-character array containing each real marker.

### Obligations

______________________________________________________________________

**OBL-SHELL** — *literal shell-variable prefixes never select a marker.*

Statement: for every character array `chars` over ALPHABET-T and every offset
`pos` within it, when `chars[pos] == '$'`, `find_substitution(chars, pos)`
returns `None`. This covers literal `$in`, `$out`, `$ins`, and `$outs`
prefixes, including truncated starts.

- **Method:** bounded model check (Kani), driving production
  `find_substitution` directly.
- **Rationale:** this is `RM-4.2.3.a`'s marker-only contract. It is a
  per-position property over all strings in the bounded alphabet, and the
  failure mode is accidentally treating shell variables as Netsuke markers —
  exactly what exhaustive exploration catches and sampling can miss.
- **Domain:** `[u8; 8]`, each byte `kani::assume`d to be a member of

  ```rust
  const ALPHABET_T: [u8; 10] = *b"$inout_`a ";
  ```

  then widened by `char::from`. The letters `i`, `n`, `o`, `u`, `t` build the
  placeholders; `a` represents "identifier character not in any placeholder";
  the space represents "non-identifier, non-special"; `_` is the
  non-alphanumeric identifier character; the backtick is present for the
  string-level harnesses that share the alphabet. `pos` is a symbolic `usize`
  with `kani::assume(pos < 8)`. Eight characters with a symbolic offset covers
  every window position under AXIOM-WINDOW, including both truncated ends, so
  the result holds for strings of any length.
- **Oracle:** the shell-variable prefix is identified by direct indexing in
  the harness, not by calling production matching code.
- **Artefact:** `src/ir/cmd_interpolate/verification.rs`, harness
  `shell_variable_prefix_does_not_match`.
- **Evidence:** capped `make kani-ir` reports `SUCCESS`.
- **Non-vacuity:**
  - *Covers.* `kani::cover!` that input and output shell-variable prefixes pass
    through, including a start-position case. Any reported `UNSATISFIABLE`
    means the domain is wrong — treat it as a failure.
  - *Mutation.* The harness is validated by the live matcher assertion and the
    Kani mutation-evidence contract; the superseded sigil mutation patch is no
    longer applicable to marker-only lowering.

______________________________________________________________________

**OBL-MARKER** — *marker placeholders match on exact text and deliberately
ignore token boundaries.*

Statement: for every array `chars` containing either internal marker and every
offset `pos`, `find_substitution(chars, pos)` returns the matching placeholder
and exact marker length if and only if the marker text starts at `pos`; and no
property of `chars[pos - 1]` or the character after the marker affects the
result.

- **Method:** bounded model check (Kani), driving production
  `find_substitution` with the real `INS_TOKEN` and `OUTS_TOKEN` marker forms.
- **Rationale:** exercising the production decision keeps the marker
  fallback reachable through its underscore-prefix guard while retaining the
  exact-match and boundary-asymmetry checks. A mutation in the fallback loop
  must therefore fail through the same entry point used by production.
- **Domain:** a 32-character symbolic window containing each real marker, with
  symbolic positions and explicit prefix, suffix, near-miss, and truncation
  cases. The window is large enough for the marker loop's 27-character span.
- **Artefact:** `src/ir/cmd_interpolate/verification.rs`, harness
  `marker_token_match_is_exact`.
- **Evidence:** capped `make kani-ir` reports `SUCCESS`.
- **Non-vacuity:**
  - *Covers.* A match preceded by an identifier character still returns `Some`
    (this is the asymmetry, and if it is `UNSATISFIABLE` the harness proves
    nothing about it); a match at `pos == 0`; a near-miss differing in exactly
    one character returns `None`; a match truncated by the array end returns
    `None`.
  - *Mutation.* `...__marker_token_match_is_exact.patch` changes
    `chars.get(pos + i)` to `chars.get(pos)` in `src/ir/cmd_interpolate/mod.rs`.
- **Residual gap:** longer surrounding command text remains covered by the
  property tests; both checked-in marker constants are driven through
  `find_substitution` by this harness.

______________________________________________________________________

**OBL-SPEC** — *the scanner agrees with a declarative specification, so
backtick regions are preserved.*

Statement: for every generated template and short `ins`/`outs` binding,
`substitute(template, &bindings)` agrees with
`spec_substitute(template, &bindings.ins, &bindings.outs)`, where
`spec_substitute` is a declarative oracle defined in the property tests.

- **Method:** Proptest differential testing against a harness-local oracle.
- **Rationale:** this is `RM-4.2.3.b` and more. Stating "backtick regions are
  preserved" directly requires mapping input offsets to output offsets, which
  is awkward and easy to state vacuously. Differential agreement with an
  independently written specification pins the *whole* scanning semantics —
  backtick exclusion, index advance, and the interaction between them — in one
  property, and it is the strongest non-vacuous statement available for this
  code. It is the technique Kani's own documentation demonstrates
  (`assert!(meets_specification(input, output))`).
- **The oracle must be structurally different from production**, or it inherits
  production's bugs and passes vacuously. Specifically it must:
  - recompute backtick parity from scratch at each index by counting backticks
    in `chars[0 .. i]`, rather than carrying a running flag;
  - decide each index independently by asking "is this index the start of a
    match that is not covered by an earlier match?", computing coverage by a
    forward pass over decisions rather than by an index-advance loop;
  - be written from the contract in `Context and orientation`, not by
    paraphrasing `substitute`.
  It is permitted to call production `find_substitution`, because that function
  is separately pinned by `OBL-SHELL` and `OBL-MARKER`; what it must not reuse
  is `substitute`'s control flow. Quadratic cost is acceptable and expected.
- **Domain:** generated dense and sparse templates, including the eight-
  placeholder and 256-character cases; `ins` and `outs` are raw binding strings
  generated by the property-test support module.
- **Artefact:** `src/ir/cmd_interpolate_property_tests.rs`, property
  `scanner_agrees_with_independent_specification`.
- **Evidence:** the scanner property and its mutation evidence pass under
  `make test`.
- **Non-vacuity:**
  - *Covers.* Some input has a non-empty backtick region **containing a
    marker**;
    some input has a *closed* backtick region followed by a substituted
    placeholder (catches a stuck `in_backticks` flag); some input produces at
    least two substitutions.
  - *Mutation.*
    `...__scanner_agrees_with_independent_specification.patch` removes the
    `if self.in_backticks { ... }` arm in `src/ir/cmd_interpolate/substitution.rs`.
  - *Second mutation, applied during EP-M3 as a one-off check and recorded in
    `Artefacts and notes` rather than committed:* change `i += skip` to
    `i += 1` at line 256. The oracle must reject this. If it does not, the
    oracle is paraphrasing production and must be rewritten.

______________________________________________________________________

**OBL-ODD** — *a command whose substituted form has an odd backtick count is
rejected.*

Statement: for every generated template and binding pair, let
`s = substitute(template, &bindings)`. If the number of backticks in `s` is
odd, then `interpolate_command_with_bindings(template, bindings)` returns
`Err(IrGenError::InvalidCommand { command, .. })` with `command == s`.

- **Method:** Proptest over the residual template and binding range.
- **Rationale:** this is `RM-4.2.3.c`. The load-bearing word is *substituted*:
  the count is taken after substitution, so a binding that introduces a
  backtick must also trigger rejection.
- **The backtick count in the assertion must be computed by a harness-local
  fold, not by calling `has_unmatched_backticks`.** If the harness calls the
  production predicate, the mutation below flips both sides of the implication
  and the harness passes — fully vacuous. This is an authorized oracle.
- **Domain:** generated interpolation templates with residual bindings of
  length at most 2 over

  ```rust
  const ALPHABET_B: [u8; 2] = *b"a`";
  ```

- **Constructing the bindings.** The planned Kani harness is not retained after
  the M0 cap measurement. The residual Proptest module is a child of the
  interpolation module, so it can construct `CommandBindings` with arbitrary
  raw text without widening the production API or retaining a proof-only
  constructor.
- **Artefact:** `src/ir/cmd_interpolate_property_tests.rs`, property
  `substituted_odd_backticks_are_rejected`.
- **Evidence:** the odd-backtick property and its mutation evidence pass under
  `make test`.
- **Non-vacuity:**
  - *Covers.* Some input where the template's backtick count is **even** but
    the bindings make the substituted count odd — this is the case that
    distinguishes checking the result from checking the template, and if it is
    `UNSATISFIABLE` the harness is not testing what it claims. Also: some input
    where the template itself is odd; and some input that is accepted.
  - *Mutation.*
    `...__substituted_odd_backticks_are_rejected.patch` changes
    `rem_euclid(2) != 0` to `== 0` in `src/ir/cmd_interpolate/mod.rs`.

______________________________________________________________________

**OBL-GUARD** — *the guard is applied to the substituted command, not the
template.*

Statement: for every generated template and binding pair, let
`s = substitute(template, &bindings)`. Then
`interpolate_command_with_bindings(template, bindings)` returns `Ok(s)` if and
only if the harness-computed backtick count of `s` is even **and**
`shlex::split(&s).is_some()`.

This is the reformulation of `RM-4.2.3.d`. The roadmap's literal wording —
"successful results satisfy the current `shlex` guard" — restates the branch
condition at line 107 and cannot fail for any implementation whatsoever, which
the ExecPlan discipline treats as a verification failure rather than a pass.
The reformulation adds three things that *can* fail: the "only if" direction
(nothing the guard accepts is rejected), the identity `result == s`, and guard
*placement*, since `s` is recomputed by the harness from the substituted form.

Be honest about what this does **not** prove: the biconditional is insensitive
to the *meaning* of `has_unmatched_backticks`. Applying `OBL-ODD`'s mutation
flips both sides and this harness still passes. `OBL-ODD` pins the meaning and
`OBL-GUARD` pins the placement; neither alone is sufficient, and the plan must
say so in the developers' guide.

- **Method:** Proptest against the real `shlex` crate.
- **Domain:** generated interpolation templates over the residual alphabet,
  extended with `'` (single quote), and bindings as `OBL-ODD`. Six characters
  is enough to exhibit the placement fault: a backtick, a marker, and a
  binding-introduced backtick fit.
- **Do not assert the `snippet` field.** `snippet` is
  `interpolated.chars().take(160).collect()`; asserting it drags a
  160-iteration loop into the hardest formula in the suite and forces
  `#[kani::unwind(161)]`, which Kani applies to *every* loop in the harness
  including `shlex`'s. The `snippet` construction is covered by the existing
  unit tests.
- **Artefact:** `src/ir/cmd_interpolate_property_tests.rs`, property
  `guard_uses_the_substituted_command`.
- **Evidence:** the guard-placement property and its mutation evidence pass
  under `make test`.
- **Non-vacuity:**
  - *Covers.* Some input reaches `Ok`; some reaches `Err`; and — the important
    one — some input where `shlex::split` returns `None` while the backtick
    count is **even**. Without that witness the `shlex` conjunct is inert and
    the harness collapses into `OBL-ODD`. The single quote is in the alphabet
    precisely to make it reachable.
  - *Mutation.* `...__guard_uses_the_substituted_command.patch` changes
    `has_unmatched_backticks(&interpolated)` to
    `has_unmatched_backticks(template)` in `src/ir/cmd_interpolate/mod.rs`.

______________________________________________________________________

**OBL-EQUIV** — *the `SubstitutionTraversal` module split does not change
behaviour.*

Statement: the refactored `substitute` produces, for every template and
bindings, the string the pre-refactor implementation produced.

- **Method:** characterization tests plus a differential Proptest against a
  temporary oracle. Not model checking.
- **Rationale:** this is refactoring equivalence over a pre-existing
  implementation, not a new invariant. The existing suite is demonstrably too
  weak to detect a regression (see `Risks`), so "no test edits" is necessary
  but not sufficient.
- **Procedure:**
  1. Add the characterization table from `Context and orientation` as
     `#[rstest]` cases, with hand-computed expected strings. Run before
     touching production; they must pass.
  2. Copy the current `substitute` body verbatim into a `#[cfg(test)]` oracle
     named `substitute_before_refactor`.
  3. Add a Proptest that generates templates over the adversarial alphabet
     `` $ ` i n o u t _ a ' \ space `` up to 64 characters and asserts
     `substitute == substitute_before_refactor`.
  4. Perform the refactor.
  5. Delete the oracle **in the same commit that closes EP-M1**. It is a
     temporary differential fixture, not a compatibility shim; it has no
     consumer and must not survive the milestone.
- **Artefact:** `src/ir/cmd_interpolate_tests.rs` (new sibling; see
  `Interfaces and dependencies` for why the tests move out) and
  `src/ir/cmd_interpolate_property_tests.rs`.
- **Evidence:** `make test` passes at every step; `git diff` shows no
  pre-existing test assertion modified; a `netsuke build` over a manifest in
  `tests/` produces a byte-identical `build.ninja` before and after.
- **Non-vacuity:** each characterization case is asserted against a
  hand-computed string, never against a call to the code under test. If you
  cannot derive the expected output by hand from the rules above, the case is
  wrong. The Proptest generator must report case classification showing it
  actually produces `$`, backticks, and quotes; a generator that never emits
  them is a verification failure, not a pass.

______________________________________________________________________

**OBL-PATCHES** — *mutation evidence stays valid.*

Statement: every file in `docs/verification/mutations/` applies cleanly to the
current tree, and every `#[kani::proof]` harness in `src/` has a corresponding
patch or an explicitly justified exemption.

- **Method:** a contract test in `make test`.
- **Rationale:** the repository's entire non-vacuity discipline rests on these
  patches and nothing validates them. They are handwritten unified diffs, so
  any edit near a patched hunk breaks `git apply` silently. There are already
  twelve patches for thirteen harnesses.
- **Artefact:** `tests/kani_mutation_evidence_tests.rs`, following the style of
  the existing `tests/kani_cfg_ui_tests.rs` contract test.
- **Evidence:** the test fails if a patch is deleted or a harness is added
  without one. Verify by temporarily renaming a patch and observing the failure.
- **Exemption list.** The test carries a `MUTATION_EXEMPT` constant with one
  entry: `canonicalize_path_wrapper_matches_u8_kernel_for_two_nodes`, reason
  "adapter harness; the `u8` kernel harnesses carry the mutation evidence for
  the algorithm it adapts". Do not add 4.2.3 harnesses to that list. Whether
  the adapter harness should gain a patch is a question for the maintainer;
  raise it, do not silently fix roadmap 4.2.2's work here.

### Residual gaps (to be recorded in `ADR-004` and the retrospective)

- Strings longer than the EP-M0 bound are covered by Proptest, not proof, for
  `OBL-SPEC`, `OBL-ODD`, and `OBL-GUARD`. `OBL-SHELL` has no such gap for its
  per-position shell-variable check.
- The marker harness drives both 27-character `INS_TOKEN` and `OUTS_TOKEN`
  constants through `find_substitution`; longer surrounding commands remain a
  property-test concern.
- `shell_quote`'s output is assumed (AXIOM-QUOTE). The guard harnesses take
  arbitrary bindings, which is stronger than only-quoted bindings, but the
  quoting function itself is unverified.
- Non-ASCII input is outside every alphabet. `substitute` treats any
  non-matching `char` identically, so the space representative covers it by
  argument rather than by proof.
- The path from `BuildGraph::from_manifest` down to this code is covered by
  existing unit and behavioural tests, consistent with `ADR-004`'s existing
  limitation for `find_duplicates`.

## Plan of work

### Stage A — understand and measure (no production changes)

Read `src/ir/cmd_interpolate/mod.rs`, `substitution.rs`, and `verification.rs`
end to end, then `src/ir/cycle_verification.rs` and `src/ir/cycle_support.rs`
for the shape you are copying, then `ADR-004`, then the two Decision log
entries in the 4.2.2 execplan covering the resource cap and `LD_LIBRARY_PATH`.

Run EP-M0. Write no harness until you have its measurement table.

### Stage B — red

1. **Characterization (EP-M1).** Add the `#[rstest]` cases from the table in
   `Context and orientation`. They must pass against the *current*
   implementation. Any case that fails is a discovery: record it in
   `Surprises & discoveries` and escalate, because the behaviour you were about
   to freeze is not what you thought.
2. **Harness compilation failure (EP-M2).** Write
   `src/ir/cmd_interpolate/verification.rs` with
   `shell_variable_prefix_does_not_match` and declare the module. Run the capped
   `make kani-ir`. Expect a compile error naming the missing items. That is
   the red evidence.

### Stage C — implementation and verification together

EP-M1, then EP-M2, EP-M3, EP-M4 in order. Per harness the loop is:

1. Write the harness with its covers.
2. Run it capped. Record wall-clock time and confirm every cover is
   `SATISFIED`.
3. If it fails, read the counterexample. Fix the harness if the harness is
   wrong; escalate if production is wrong, because that is a behaviour change.
4. Write the mutation patch, apply it, re-run, confirm the harness fails **and
   that it fails on the intended check name**, revert, re-run, confirm success.
5. Commit.

Do not batch the mutation patches to the end. A patch written after four
harnesses exist cannot tell you which harness was vacuous.

### Stage D — hand-off, documentation, wider validation

EP-M5 extends Proptest to the residual range. EP-M6 updates documentation, the
ADR, and the roadmap, then runs the full gate set and a CodeRabbit review.

Each stage ends with `make check-fmt`, `make lint`, `make test`. Do not enter
the next stage on a failing gate.

## Milestones and plateaus

### EP-M0 — feasibility spike (prototyping)

- **Outcome:** a measurement table in `Artefacts and notes` and a bound
  decision. Nothing else is committed.
- **Method:** on a scratch branch, write one throwaway harness that builds a
  symbolic `[u8; N]` over ALPHABET-T, widens with `char::from`, and calls
  `find_substitution` at a symbolic offset with a trivial assertion. Measure at
  `N = 8`, then **double and bisect** rather than walking a ladder: growth here
  is exponential, so `N = 8, 16, 12` answers the question in three runs where
  revision 1's six-point ladder burned roughly forty minutes. Then repeat for
  the string-level shape (calling the `SubstitutionTraversal`-backed
  `substitute`) and, separately, one probe that calls
  `interpolate_command_with_bindings` at `M = 6` to settle the `shlex` cost.
- **Also evaluate, and record the answer:** whether `cargo kani --jobs 4`
  is available and accepted in Kani 0.67.0 and whether it requires
  `--output-format=terse`. Check `cargo kani --help` before assuming. Harness
  solving is single-threaded per harness and runners have four cores, so this
  is potentially a two-to-four-times saving on the solve phase for no proof
  weakening. Do not change the Makefile to adopt it (Constraint 12); record it
  as the first contingency lever in `Risks`.
- **Acceptance evidence:** a table with columns `shape`, `N`, `wall-clock`,
  `peak RSS`, `verdict`, plus a stated `M` for the string-level and guard
  harnesses.
- **Conformance check and mechanical stop:** if the `find_substitution` shape
  does not verify at `N = 8` within 6 minutes, commit nothing and report; that
  shortfall would undermine the plan's premise rather than merely narrow its
  reach, and it is not covered by the maintainer's pre-acceptance. A shortfall
  in the *string-level* bound is different: the maintainer accepted it in
  advance on 2026-08-24, so record the achieved bound and the residual gap in
  `Decision log`, draft the `ADR-004` extension, and continue to EP-M1 without
  setting the plan to `BLOCKED`.
- **Recovery:** delete the scratch branch.
- **Compatibility decision:** none; no interface exists yet.

### EP-M1 — production seam, behaviour unchanged

- **Outcome:** `substitute` delegates to `SubstitutionTraversal` in
  `src/ir/cmd_interpolate/substitution.rs`; the unit tests live in
  `src/ir/cmd_interpolate_tests.rs`; the adversarial Proptest generator exists;
  `make test` passes with no pre-existing test edited.
- **Requirements:** discharges `OBL-EQUIV`; enables every other obligation.
- **Acceptance evidence:** characterization cases pass before and after; the
  temporary differential oracle passes and is deleted in the closing commit;
  `netsuke build` produces a byte-identical `build.ninja` across the refactor
  commit; `make check-fmt`, `make lint`, `make test` green.
- **Conformance check:** no public API change; no new dependency; no capacity
  limit introduced (Constraint 4); every touched file under 400 lines.
- **Recovery:** one commit; `git revert` restores it, and the characterization
  tests remain valid against the restored implementation.
- **Compatibility decision:** none. `substitute` keeps its signature and all
  callers are updated in the same commit. The `substitute_before_refactor`
  oracle is a test fixture with a stated deletion point, not a shim.

### EP-M2 — shell-variable and marker properties proved; patch gate added

- **Outcome:** `shell_variable_prefix_does_not_match` and
  `marker_token_match_is_exact` verify; `tests/kani_mutation_evidence_tests.rs`
  exists and passes.
- **Requirements:** discharges `RM-4.2.3.a`, `OBL-SHELL`, `OBL-MARKER`,
  `OBL-PATCHES`.
- **Acceptance evidence:** capped `make kani-ir` reports `SUCCESS` for both new
  harnesses and all thirteen pre-existing ones, with every cover `SATISFIED`;
  each mutation patch makes its own harness fail on the expected check;
  `make test` includes the new contract test and passes.
- **Conformance check:** suite wall-clock recorded against the 8-minute
  tolerance; AXIOM-WINDOW argument written into the harness module doc comment.
- **Recovery:** harnesses are `cfg(kani)`-gated and additive.

### EP-M3 — scanner specification and backtick rejection hand-off

- **Outcome:** the M0 resource cap precludes the proposed scanner Kani
  harnesses. EP-M5 supplies the independent scanner and odd-backtick properties
  over the required residual range.
- **Requirements:** discharges `RM-4.2.3.b`, `RM-4.2.3.c`, `OBL-SPEC`,
  and `OBL-ODD` through the Proptest hand-off.
- **Acceptance evidence:** the scanner and odd-backtick property mutations
  produce named counterexamples; `make test` passes after restoration.
- **Conformance check:** the independent scanner oracle is structurally
  different from production scanning and is documented in `ADR-004`.

### EP-M4 — guard placement hand-off

- **Outcome:** the guard's scanner dependency makes its Kani probe infeasible
  under the M0 cap. EP-M5 checks the real `shlex` guard on substituted command
  text throughout the residual range.
- **Requirements:** discharges `RM-4.2.3.d` and `OBL-GUARD` through the
  Proptest hand-off.
- **Acceptance evidence:** the guard-placement mutation produces a named
  counterexample; `make test` passes after restoration.
- **Conformance check:** confirm the reformulation of `RM-4.2.3.d` is recorded
  in `Decision log` with maintainer acceptance, so a reviewer can see why the
  property does not merely restate the guard branch.

### EP-M5 — Proptest hand-off for the residual range

- **Outcome:** `src/ir/cmd_interpolate_property_tests.rs` covers the range Kani
  could not reach: templates to 256 characters with up to 8 placeholders over
  the adversarial alphabet, raw adversarial bindings, and the real `shlex`
  guard.
- **Requirements:** covers the shortfall between the achieved bounds and
  `RM-4.2.3.a`'s stated 256/8, so the roadmap's bound is met by *some* method
  everywhere.
- **Acceptance evidence:** `make test` passes; each new property fails under
  the corresponding EP-M2 to EP-M4 mutation patch (reuse them — the cheapest
  possible non-vacuity check); Proptest case-classification output shows the
  generators actually reach 8 placeholders and non-empty backtick regions. A
  generator that never produces the interesting shape is a failure, not a pass.

### EP-M6 — documentation, ADR, roadmap, final validation

- **Deliverables:**
  - `docs/developers-guide.md`: one inventory row per new harness in the table
    in the "Kani harness inventory" section, matching the existing columns;
    extend the prose above it to describe the `SubstitutionTraversal` seam and
    `cfg(kani)` bindings constructor, as the paragraph introducing that table
    already describes the cycle kernel; add a subsection under "Command and
    recipe lowering" stating the placeholder
    contract now proved — supported markers, literal shell-variable pass-through,
    the marker-form asymmetry, backtick-region
    exclusion, and guard placement, including the explicit note that `OBL-ODD`
    and `OBL-GUARD` are complementary. This is the source material for roadmap
    4.4.1.
  - `docs/formal-verification-methods-in-netsuke.md`: update the "Kani for
    command interpolation" section to
    state the bounds actually achieved and the AXIOM-WINDOW argument rather than
    the aspirational 256/8, and name the Proptest hand-off; update the harness
    count in the continuous-integration paragraph that currently says
    "13 harnesses".
  - `docs/adr-004-bound-kani-ir-harnesses-to-small-n.md`: extend Status, Date,
    Decision outcome, and Known risks with the 4.2.3 bound decision, the
    window-completeness argument, the harness-local oracle policy, and the
    residual gaps. Add this execplan to Related documents.
  - `docs/roadmap.md`, item 4.2.3: change `- [ ] 4.2.3.` and all four
    sub-bullets to `- [x]`, then append evidence bullets in the style of the
    completed 4.2.1 and 4.2.2 entries.
  - `docs/users-guide.md`: **no change**, and say so here rather than leaving
    it unstated. This work adds no user-visible behaviour and no configuration.
- **Acceptance evidence:** `make check-fmt`, `make lint`, `make test`,
  `make markdownlint`, `make nixie`, and capped `make kani-ir` all pass;
  `coderabbit review --agent` reports no outstanding concerns; the measured CI
  `Run Kani harnesses` step is within the 12-minute tolerance.
- **Conformance check:** every trace link resolves to a named, passing artefact;
  every residual gap is in `ADR-004`; set status to `COMPLETE` only after that
  reconciliation.
- **Remaining gaps:** roadmap 4.4.1 remains open by design.

## Interfaces and dependencies

No new libraries. Existing ones: `shlex` 2.0.1 (guard, AXIOM-SHLEX),
`shell-quote` 0.7.2 (`Sh`, AXIOM-QUOTE), `rstest` 0.26.1, `googletest` 0.14.3,
`pretty_assertions` 1.4.1, `proptest` 1.11.0, and Kani 0.67.0 external to Cargo.

The current implementation is split between `src/ir/cmd_interpolate/mod.rs` and
its private `substitution.rs` sibling:

`substitute` converts the template to characters, constructs a
`SubstitutionTraversal`, and advances it through the one-pass placeholder and
backtick state.

The Kani harnesses drive the allocation-free matchers directly, while the
residual Proptest properties exercise `SubstitutionTraversal` through
`substitute`.

The module split also moves the existing test body into
`src/ir/cmd_interpolate_tests.rs`, wired with
`#[cfg(test)] #[path = "cmd_interpolate_tests.rs"] mod tests;` — the same
pattern `src/ir/cycle.rs` uses. The characterization cases go there.

Revision 1 of this plan proposed a `PlanBuffer`/`ScanOutcome`/`Substitution`
kernel. It is **withdrawn**; see `Decision log`.

### Architectural note: where the boundary sits

Be precise about what this seam is. `SubstitutionTraversal` versus `substitute`
is a **technical** boundary — traversal state versus the string-level entry
point — introduced to keep one-pass shell handling private. It is not a domain
boundary, and the plan should not dress it as one.

There *is* a real conceptual line in this file, and the harnesses respect it:
placeholder policy (which sequences are placeholders, where boundaries fall) is
pure and depends on nothing outside the character slice, while the acceptance
guard and path quoting encode knowledge of POSIX shell syntax. Backtick
handling straddles it — regions are a shell concept but region *exclusion* is
pure scanning — which is why `OBL-SPEC` proves the scanner and `OBL-ODD` and
`OBL-GUARD` prove the guard separately.

Do not introduce traits, ports, adapters, or dependency injection here. The
`hexagonal-architecture` guidance applies where infrastructure is likely to
change and multiple delivery mechanisms exist; neither is true. `ADR-004`
Option D already rejected verification ports on this surface.

## Concrete steps

Work in
`/home/leynos/.lody/repos/github---leynos---netsuke/worktrees/ac36bbe2-72c0-44e8-8d59-aaf79d5e8205`
on branch `4-2-3-kani-harnesses-for-command-interpolation`.

### One-time setup

```bash
make install-kani
make kani-check
```

Expect `prover-tools:` diagnostic lines on standard error and exit zero.

### Running Kani (always capped, always with `LD_LIBRARY_PATH`)

Never run Kani bare. The wrapper below is the one roadmap 4.2.2 settled on
after uncapped runs OOM-killed the machine. The `LD_LIBRARY_PATH` is not
optional: without it `cargo kani` and Cargo build scripts fail to load
`libLLVM` with an opaque linker error, and that is the most common way to lose
an hour on this task.

```bash
timeout --kill-after=20s 5m \
  systemd-run \
    --user \
    --scope \
    --expand-environment=no \
    -p CPUQuota=200% \
    -p MemoryMax=8G \
    -p MemorySwapMax=0 \
    -p TasksMax=96 \
    -p IOWeight=20 \
    /usr/bin/nice -n 15 \
    env LD_LIBRARY_PATH="$HOME/.kani/kani-0.67.0/toolchain/lib:$HOME/.kani/kani-0.67.0/lib" \
    make kani-ir \
  2>&1 | tee /tmp/kani-netsuke-4-2-3-kani-harnesses-for-command-interpolation.out
```

To iterate on a single harness, add
`KANI_FLAGS="--harness shell_variable_prefix_does_not_match"` to the `make`
invocation. `KANI_FLAGS` is empty by default (Makefile line 17) and `kani-full`
passes it straight through, so this needs no Makefile edit.

Expected shape of success:

```plaintext
Checking harness ir::cmd_interpolate::verification::shell_variable_prefix_does_not_match...
VERIFICATION:- SUCCESSFUL
Verification Time: 41.2s
```

Expected shape with a mutation applied — this is what you want to see during
the non-vacuity check, and you must confirm the named check is the intended one:

```plaintext
 Failed Checks: literal shell-variable prefix was treated as a marker
 File: src/ir/cmd_interpolate/verification.rs, line 63, in ...
VERIFICATION:- FAILED
```

If a `kani::cover!` is reported `UNSATISFIABLE`, the harness's domain never
reaches that case. Treat it as a failure and fix the domain.

### Ordinary gates

Sequentially, never in parallel — the build cache is shared.

```bash
make check-fmt 2>&1 | tee /tmp/check-fmt-netsuke-4-2-3-kani-harnesses-for-command-interpolation.out
make lint      2>&1 | tee /tmp/lint-netsuke-4-2-3-kani-harnesses-for-command-interpolation.out
make test      2>&1 | tee /tmp/test-netsuke-4-2-3-kani-harnesses-for-command-interpolation.out
```

Documentation gates, after any Markdown edit:

```bash
make markdownlint 2>&1 | tee /tmp/markdownlint-netsuke-4-2-3-kani-harnesses-for-command-interpolation.out
make nixie        2>&1 | tee /tmp/nixie-netsuke-4-2-3-kani-harnesses-for-command-interpolation.out
```

`make fmt` reformats Markdown repository-wide. If you run it, inspect
`git diff` and revert reflow noise in untouched files before committing.

Expect lint friction in harness code: `unwrap` and `expect` are denied, and
`allow_attributes_without_reason` is on. Build symbolic arrays with explicit
indexing and `match`, not `unwrap`.

### Applying and reverting a mutation patch

```bash
git apply docs/verification/mutations/ir__cmd_interpolate__verification__marker_token_match_is_exact.patch
# run the capped single-harness command; expect VERIFICATION:- FAILED
git apply -R docs/verification/mutations/ir__cmd_interpolate__verification__marker_token_match_is_exact.patch
# re-run; expect VERIFICATION:- SUCCESSFUL
```

Read an existing patch such as
`docs/verification/mutations/ir__cycle__verification__self_dependency_reports_cycle.patch`
for the exact unified-diff format before writing a new one.

### Commits and review

Commit after each milestone and after each harness within EP-M2 to EP-M4.
AGENTS.md style: imperative subject of at most 50 characters, blank line, body
wrapped at 72 columns. Only commit when `make check-fmt`, `make lint`, and
`make test` pass.

At each milestone boundary, after the deterministic gates pass:

```bash
coderabbit review --agent
```

Clear every concern before the next milestone. CodeRabbit is not a substitute
for the deterministic gates; run those first.

## Validation and acceptance

### Red-green-refactor evidence to record

EP-M1 — **Red:** characterization cases added and passing (they pin current
behaviour); the EP-M2 harness file references the private matchers in
`cmd_interpolate/mod.rs` before they exist and the capped `make kani-ir` fails
to compile. Record the message. **Green:** the seam is added, `make test`
passes with no pre-existing test edited. Record the nextest summary line.
**Refactor:** all three gates pass and `build.ninja` is byte-identical.

EP-M2 to EP-M4, per harness — **Red:** the capped single-harness run with the
mutation applied reports `VERIFICATION:- FAILED` naming the intended check.
Record the check name and counterexample summary. **Green:** the same run with
the mutation reverted reports `VERIFICATION:- SUCCESSFUL` with zero failed
checks. Record the verification time. **Cover:** every `kani::cover!` reported
`SATISFIED`.

### Acceptance, phrased as behaviour

1. Run the capped `make kani-ir`. Expect fifteen harnesses (thirteen
   pre-existing plus two new), zero failed checks, every cover satisfied, and
   `VERIFICATION:- SUCCESSFUL` overall.
2. Apply any new mutation patch, re-run its harness, expect
   `VERIFICATION:- FAILED`; revert and expect success. The harnesses are
   load-bearing.
3. Temporarily rename any file in `docs/verification/mutations/` and run
   `make test`. Expect `tests/kani_mutation_evidence_tests.rs` to fail. Restore
   it.
4. Run `make test`. Expect a pass, and expect `git diff` against the merge base
   to show no modification to any pre-existing test assertion.
5. Run `netsuke build` against a manifest in `tests/`. Expect a byte-identical
   `build.ninja` compared with the pre-change build.
6. Open `docs/developers-guide.md` and find a table row for each new harness,
   naming its module, property, and unwind bound, plus a prose subsection
   stating the placeholder contract.
7. Open `docs/roadmap.md` at item 4.2.3 and see `- [x]` on the item and all
   four sub-items with evidence bullets.

### Quality criteria

- **Tests:** `make test` passes; no pre-existing test edited.
- **Verification:** `OBL-SHELL` and `OBL-MARKER` are discharged by Kani with
  mutation evidence and satisfied covers. `OBL-SPEC`, `OBL-ODD`, and
  `OBL-GUARD` are covered by independent Proptest properties over the
  256-character, eight-placeholder residual range; `OBL-EQUIV` is covered by
  characterization plus differential Proptest; `OBL-PATCHES` is covered by the
  contract test.
- **Lint and format:** `make check-fmt`, `make lint`, `make markdownlint`,
  `make nixie` all pass.
- **Performance:** capped `make kani-ir` under 8 minutes locally; the measured
  CI `Run Kani harnesses` step under 12 minutes.
- **Security:** no new dependency; no change to which commands are accepted or
  rejected. The work strengthens evidence for an existing security-sensitive
  boundary rather than moving it.

## Idempotence and recovery

Every step is re-runnable. `make install-kani` is idempotent. Kani runs are
read-only with respect to the working tree. Mutation patches apply and revert
with `git apply` and `git apply -R`; if a revert fails the tree is dirty, and
`git checkout -- src/` restores it since patches only touch production source.

The riskiest step is EP-M1's seam. It is a single commit, `git revert` undoes
it cleanly, and the characterization tests remain valid against the restored
implementation.

Note the ordering deliberately avoids a negative-value plateau: EP-M1's
production change is three lines plus a `cfg(kani)` constructor, so even if the
work is abandoned immediately afterwards the repository carries a stronger test
suite and a trivial refactor rather than churn without benefit. Revision 1's
ordering did not have this property.

Nothing writes outside the repository except scratch logs under `/tmp`.

## Artefacts and notes

This section is populated during implementation. It must contain, by EP-M6:

### EP-M0 measurements (2026-08-30)

All probes used the required five-minute, 2-CPU, 8-GiB `systemd-run` wrapper.
The wrapper does not expose a peak-RSS measurement, so that column is recorded
as unavailable rather than inferred from host-wide memory. No probe was
OOM-killed.

| Shape                                     | N or M | Wall-clock | Peak RSS    | Verdict              |
| ----------------------------------------- | ------ | ---------- | ----------- | -------------------- |
| `find_substitution` sigil window          | 8      | 35.261s    | unavailable | verified             |
| `find_substitution` sigil window          | 16     | 27.877s    | unavailable | verified             |
| `find_substitution` sigil window          | 12     | 39.759s    | unavailable | verified             |
| `SubstitutionTraversal` scanner           | 8      | 5m cap     | unavailable | timed out            |
| `SubstitutionTraversal` scanner           | 6      | 5m cap     | unavailable | timed out            |
| `interpolate_command_with_bindings` guard | 6      | not run    | n/a         | dominated by scanner |

`cargo kani --help` confirmed that 0.67.0 advertises `--jobs 4`; the M2 probe
then established that it requires `--output-format terse`. The available
optimization is therefore parallel harness scheduling without a Makefile
change, using that flag pair only for the capped validation run.

The sigil proof's successful N=16 run is enough to support the planned
window-completeness argument. The real scanner does not fit the cap even at
M=6, so scanner and guard harnesses cannot be admitted at the plan's target
bound. The temporary feasibility seam and harness module were removed before
EP-M1; no production code changed in this milestone. Logs are retained under
`/tmp/kani-m0-*-netsuke-4-2-3-kani-harnesses-for-command-interpolation.out`.

### EP-M1 refactor evidence (2026-08-30)

The temporary verbatim `substitute_before_refactor` oracle agreed with the
refactored implementation over 256 adversarial templates before removal. The
retained targeted nextest run passed all 20 command-interpolation tests:
`/tmp/m1-targeted-final-netsuke-4-2-3-kani-harnesses-for-command-interpolation.out`.

### EP-M2 through EP-M6 evidence (2026-08-30)

The final sigil proof took 66.686 seconds and satisfied all five covers; the
marker proof took 22.644 seconds and satisfied all four. Their mutation patches
failed their named assertions, then passed after restoration. Each of the three
Proptest mutation patches produced its intended counterexample before
restoration.

The final capped, four-worker run completed all 15 harnesses with zero
failures. The sigil harness took 101.287 seconds in that parallel run and the
marker harness took 31.502 seconds. `make check-fmt`, `make lint`, `make test`
(2,389 passed, 3 skipped, plus doctests), `make markdownlint`, and `make nixie`
all passed. CodeRabbit returned zero findings at M2, M5, and M6. The M6 Kani
log is
`/tmp/kani-m6-cleanup-full-netsuke-4-2-3-kani-harnesses-for-command-interpolation.out`.

`docs/users-guide.md` is intentionally unchanged: this task adds no
user-visible behaviour or configuration. Hosted `kani-smoke` timing remains
outside this local completion boundary and must be reported separately by CI.

## Progress

- [x] (2026-08-24) Reconnaissance: `src/ir/cmd_interpolate.rs`,
  `src/ir/cycle_verification.rs`, `src/ir/cycle_support.rs`, `ADR-004`, the
  developers' guide formal-verification sections, the roadmap, the CI workflow,
  and the 4.2.2 execplan's operational lessons.
- [x] (2026-08-24) Confirmed Kani 0.67.0 pin; `bounded_any` and stubbing are
  experimental `-Z` features and therefore out of tolerance.
- [x] (2026-08-24) Measured the CI baseline: `kani-smoke` 4:18-4:43 total, of
  which `make kani-ir` is 191-227s and roughly 45-75s is solving.
- [x] (2026-08-24) Drafted revision 1.
- [x] (2026-08-24) Six-lens design review; revision 2 rewritten in response.
- [x] (2026-08-24) Rebased onto `origin/main` at `7e5c2679` ("Add target
  descriptions and netsuke help targets"). No conflicts; line references
  refreshed. See `Decision log` for the relevance assessment.
- [x] (2026-08-24) Rebased again onto `origin/main` at `aa93ef8b`, across nine
  commits. No conflicts. `src/ir/cmd_interpolate.rs` is untouched, the harness
  count is still thirteen, and the patch count is still twelve, so issue #585's
  facts and every obligation in this plan stand. See `Decision log`.
- [x] (2026-08-24) Maintainer ratified all three decisions flagged as requiring
  acceptance: the `RM-4.2.3.d` reformulation, harness-local oracles, and a
  string-level bound below the roadmap's stated 256/8.
- [x] (2026-08-30) Plan approved by the maintainer as a whole, authorizing
  implementation to begin at EP-M0.
- [x] (2026-08-30) EP-M0 feasibility spike and bound decision. The sigil
  window verified at N=8, N=12, and N=16; the full scanner timed out at M=8 and
  M=6. See `Artefacts and notes` and `Decision log`.
- [x] (2026-08-30) EP-M1 production seam; behaviour unchanged. Added the
  `substitute_chars` technical seam, characterization cases, and an adversarial
  Proptest; removed the temporary differential oracle.
- [x] (2026-08-30) EP-M2 sigil and marker harnesses; mutation-patch contract
  test. The final individual proofs took 66.686s and 22.644s respectively; all
  five sigil covers and four marker covers were satisfied. The full suite
  completed 15 harnesses with zero failures under the cap using four workers
  and terse output. The contract test now enforces 14 patches for 15 proofs,
  with the documented cycle-kernel exemption, and refreshed six stale patches.
- [x] (2026-08-30) EP-M3 scanner and backtick obligation hand-off. The M0
  five-minute cap precluded scanner proofs at M=6 and M=8, so no Kani harness
  was added; M5 supplies the agreed residual-range checks instead.
- [x] (2026-08-30) EP-M4 guard-placement hand-off. The guard necessarily calls
  the scanner, so its Kani probe is dominated by the M0 scanner shortfall; M5
  exercises the substituted command and real `shlex` guard over the full range.
- [x] (2026-08-30) EP-M5 Proptest hand-off. Independent scanner and guard
  oracles cover templates up to 256 characters assembled from zero to eight
  placeholders. Dedicated properties force eight-placeholder cases and verify
  odd-backtick rejection and substituted-command guard placement. The three
  added mutation patches each produced a counterexample before restoration.
- [x] (2026-08-30) EP-M6 documentation, ADR extension, and roadmap close-out.
  All deterministic gates and the capped 15-harness Kani run passed; CodeRabbit
  returned zero findings.
- [x] (2026-09-02) Rebase alignment and validation: origin/main now uses
  marker-only lowering, so literal `$in` and `$out` remain shell variables. The
  active documentation and harness references are updated. After the rebase,
  `make check-fmt`, `make test`, `make typecheck`, `make lint`, doc-coverage
  (99.13%), `make markdownlint`, and `make nixie` all passed. Focused capped
  Kani runs passed: `shell_variable_prefix_does_not_match` satisfied 3/3 covers
  in 24.92348s, and `marker_token_match_is_exact` satisfied 4/4 covers in
  97.312706s. The full `make kani-ir` compiled and passed the
  command-interpolation proofs, then reached the existing five-minute cap at
  `ir::from_manifest::verification::missing_rule_shape_is_rejected` (exit 124);
  the full suite is not recorded as passed.

## Surprises & discoveries

- Observation: `substitute` rewrites `$in` inside `$$in`, producing `$<ins>`.
  Evidence: `has_valid_word_boundaries` in `src/ir/cmd_interpolate/mod.rs` with
  `is_identifier_char` (lines 126-128); `$` is not an identifier character.
  Impact: `$$` is Ninja's escape for a literal dollar, so an author writing
  `$$in` to mean the literal text `$in` gets a substitution. This may be
  contractual or a latent defect. EP-M1 pins it with a characterization test;
  the maintainer must classify it before EP-M2 encodes it in a proof. Changing
  it is out of scope (Constraint 1).

- Observation: marker-form placeholders are matched with no boundary rule and
  no `$` prefix, so `x__NETSUKE_INS_PLACEHOLDER__` is rewritten. Evidence:
  `try_match_token` (lines 213-233), called unconditionally from
  `find_substitution`'s `or_else` arm (lines 207-210). Impact: defensible,
  since the markers are machine-generated, but it is an asymmetry the harnesses
  must encode deliberately. It is also why `OBL-MARKER` exists: without it, no
  harness can reach that arm at all.

- Observation: nothing in the repository validates the mutation patches, and
  there are twelve patches for thirteen harnesses. Evidence: no reference to
  `docs/verification/mutations` in the Makefile, the workflows, or any test;
  `ls docs/verification/mutations | wc -l` is 12 while `grep -c 'kani::proof'`
  over the two verification modules totals 13. Impact: the repository's
  non-vacuity discipline is convention-only. `OBL- PATCHES` closes this. The
  missing patch belongs to roadmap 4.2.2's adapter harness; raise it rather
  than fixing it here. Tracked as issue #585.

- Observation:
  `docs/execplans/4-2-1-kani-harnesses-for-manifest-to-ir-safety-checks.md`
  carries `Status: READY FOR REVIEW` although its own Progress and Outcomes
  sections, the roadmap checkmarks, and the committed harnesses all show the
  work complete. Impact: none here, but do not treat execplan status fields as
  a source of truth. The field is stale in that file and uses at least five
  different vocabularies across the directory. Tracked as issue #586.

- Observation: the fallback marker matcher remains reachable when Kani explores
  a sigil candidate, and `try_match_token` counts a 27-character marker with
  `str::chars`. Evidence: the M0 scanner logs required loop unrolling through
  iteration 28 in `src/ir/cmd_interpolate/mod.rs`. Impact: every retained Kani
  harness must use an unwind at least 32; the default unwind of 6 is unsound
  for this path.

- Observation: Kani 0.67.0 accepts `--jobs 4` only with
  `--output-format terse`; the former alone fails before verification begins.
  Evidence: the M2 capped full-suite probe reported `Conflicting options`.
  Impact: use both flags for a parallel full-suite run; leave the repository's
  default `KANI_FLAGS` empty.

## Decision log

Three entries below were marked **requires maintainer acceptance**, because
they deviate from the roadmap's literal wording or from a constraint this plan
otherwise imposes. **All three were ratified by the maintainer on 2026-08-24**
and are now marked **accepted**. No decision in this log is outstanding.

- **Decision:** Withdraw revision 1's `PlanBuffer` / `ScanOutcome` /
  `Substitution` kernel and replace it with a three-line `substitute_chars`
  seam. **Rationale:** the design review found the kernel disproportionate and
  actively harmful. It would have introduced a fixed capacity into a currently
  unbounded production path, and every resolution of the overflow case is bad:
  truncation corrupts silently, erroring breaks previously valid manifests,
  growing defeats the purpose, and chunking loses `in_backticks` state at chunk
  edges — in code that decides how shell commands are built. It was also
  unnecessary: `find_substitution` already takes `&[char]` and allocates
  nothing, so the properties needing the largest domain need no refactor at
  all, and the cited precedent `rotate_cycle_by` allocates a `Vec` and still
  verifies at N=4. The real cost driver is symbolic UTF-8 across the `&str`
  boundary, which the seam addresses directly. Constraint 4 now forbids the
  original approach. **Date/Author:** 2026-08-24, planning agent.

- **Decision:** Reformulate `RM-4.2.3.d` as the guard-placement biconditional
  `OBL-GUARD` rather than asserting `shlex::split(result).is_some()` on the
  `Ok` branch. **Accepted by the maintainer, 2026-08-24.** **Rationale:** the
  literal reading restates the branch condition at
  `src/ir/cmd_interpolate.rs:107` and cannot fail for any implementation. The
  ExecPlan discipline treats assuming the conclusion as a verification failure.
  The biconditional adds the "only if" direction, the result identity, and
  guard placement, each with a real failure mode demonstrated by the mutation
  patch. Its limits are stated honestly in `OBL-GUARD`. **Date/Author:**
  2026-08-24, planning agent.

- **Decision:** Permit harness-local oracles for side conditions, and a
  harness-local declarative specification for `OBL-SPEC`. **Accepted by the
  maintainer, 2026-08-24.** **Rationale:** `ADR-004` Option C forbids a
  harness-side model *replacing* a production path, and Option D rejects public
  verification ports. Neither forbids an independently written oracle used as
  the right-hand side of an assertion, which is the standard
  bounded-model-checking idiom and the only way several of these obligations
  can fail at all. Without it, `OBL-ODD` would compare
  `has_unmatched_backticks` against itself and its mutation would flip both
  sides. `OBL-SPEC`'s oracle carries transcription risk, which is why the plan
  mandates a structurally different formulation and an explicit independence
  check against a second mutation. **Date/Author:** 2026-08-24, planning agent.

- **Decision:** Treat the roadmap's "256-character commands with at most 8
  placeholders" as a target measured against in EP-M0, and expect to meet it for
  `RM-4.2.3.a` by a window-completeness argument rather than by a large bound.
  **Accepted by the maintainer, 2026-08-24**, including in advance the case
  where EP-M0 shows the string-level bound falling short. EP-M0 therefore does
  not set the plan to `BLOCKED` on a shortfall; it records the achieved bound,
  the residual gap, and the Proptest hand-off, and continues. The mechanical
  stop in `Tolerances` still applies if the sigil shape cannot reach a window
  of 8 characters, because that would undermine the plan's premise rather than
  merely narrow its reach. **Rationale:** `ADR-004` records the identical
  collision for roadmap 4.2.1, where a stated 10-node bound proved unreachable
  and was resolved by small bounds plus a Proptest hand-off. For the sigil
  contract the bound largely dissolves under AXIOM-WINDOW; for the string-level
  and guard harnesses it does not, and EP-M5 covers the remainder.
  **Date/Author:** 2026-08-24, planning agent.

- **Decision:** Mandate symbolic `u8` constrained to concrete ASCII values and
  widened with `char::from`, never `kani::any::<char>()`. **Rationale:**
  `Arbitrary for char` costs 32 symbolic bits plus a validity invariant per
  symbol. Roadmap 4.2.2 already abandoned symbolic `char` and `String`
  construction for the same reason. A `u8` index into a constant table was also
  considered and rejected: it uses fewer input bits but reintroduces a ten-way
  multiplexer per symbol. **Date/Author:** 2026-08-24, planning agent.

- **Decision:** Do not retain a `cfg(kani)` `CommandBindings::from_parts`
  constructor. **Rationale:** the M0 cap decision hands scanner and guard
  properties to the child Proptest module, which can construct raw bindings
  without changing the production API. Removing the unused proof-only
  constructor keeps the Kani build warning-free and avoids a seam with no
  surviving proof consumer. **Date/Author:** 2026-08-30, implementation agent.

- **Decision:** Add `tests/kani_mutation_evidence_tests.rs`, a contract test
  that `git apply --check`s every mutation patch and asserts harness parity.
  **Rationale:** three independent review lenses identified the absence of any
  gate over `docs/verification/mutations/` as the highest-value cheap addition.
  The discipline is what makes every Kani result in this repository
  non-vacuous, and it currently rests on convention alone. **Date/Author:**
  2026-08-24, planning agent.

- **Decision:** Do not use `ortho_config`, and make no change to
  `docs/users-guide.md`. **Rationale:** the work introduces no configuration
  and no user-visible behaviour. The user-facing placeholder contract is
  roadmap 4.4.1, which `Requires 4.2.3`; `docs/developers-guide.md` is the
  correct destination under AGENTS.md and gives 4.4.1 its source material.
  **Date/Author:** 2026-08-24, planning agent.

- **Decision:** Carry this plan forward unchanged in substance across the
  rebase onto `origin/main` at `7e5c2679`. **Rationale:** that commit adds
  target descriptions and `netsuke help targets`. Its only change under
  `src/ir/` is a comment in `src/ir/from_manifest.rs` recording that target
  descriptions are discovery metadata and never take part in recipe resolution
  — which reinforces rather than disturbs the boundary this plan verifies. It
  touches no file this plan modifies, adds no shared helper or pattern relevant
  to command interpolation, and leaves `src/ir/cmd_interpolate.rs`,
  `src/ir/cycle*.rs`, the Makefile, and the CI workflow untouched. Its
  additions to `docs/developers-guide.md` sit outside the formal-verification
  sections. The only consequence is line-number drift in citations, refreshed
  in this revision. **Date/Author:** 2026-08-24, planning agent.

- **Decision:** Carry this plan forward unchanged in substance across the
  second rebase, onto `origin/main` at `aa93ef8b`. **Rationale:** nine commits
  landed, including serial dependency ordering (roadmap 3.14.3),
  configuration-load caching and observability, a Windows CI job, and a large
  restructuring of `docs/developers-guide.md`. A semantic diff over `src/ir`
  reports a new `DependencyOrder` enum, one added `BuildEdge` field, and
  mechanical updates to four test helpers; `cmd_interpolate` does not appear at
  all. `src/ir/cmd_interpolate.rs`, `src/ir/cmd_interpolate_property_tests.rs`,
  `src/ir/from_manifest_support.rs`, and `docs/verification/` are
  byte-identical, the harness count is still thirteen, the patch count is still
  twelve, and the `kani-smoke` job keeps its shape, its cache key, and its
  20-minute cap. Two operational notes are folded into `Risks` below. Citations
  were converted from line numbers to section headings for Markdown documents,
  because two consecutive rebases have now moved them and the numbers cannot be
  kept true. **Date/Author:** 2026-08-24, planning agent.

- **Decision:** Extend `docs/adr-004-bound-kani-ir-harnesses-to-small-n.md`
  rather than minting a new ADR number. **Rationale:** the same decision — how
  far to bound Kani harnesses and where to hand off — applied to a third
  subject. Roadmap 4.2.2 set the precedent, and three files already share the
  `adr-004` prefix. **Date/Author:** 2026-08-24, planning agent.

- **Decision:** Continue after the EP-M0 string-level shortfall with only the
  small, allocation-free sigil and marker proofs; discharge scanner, backtick,
  and guard behaviour through the mandated Proptest hand-off rather than add
  harnesses that exceed the five-minute cap. **Accepted in advance by the
  maintainer on 2026-08-24; measured 2026-08-30.** **Rationale:**
  `find_substitution` verified at N=8, N=12, and N=16, so the pre-accepted
  window argument remains valid. In contrast, the production scanner timed out
  at M=8 and again at M=6 under the mandatory resource cap. The guard
  necessarily invokes that scanner first, so a guard probe would not establish
  a feasible independent bound. The full domain therefore belongs to EP-M5's
  256-character, eight-placeholder Proptest coverage. This decision preserves
  the plan's stated safety boundary without weakening the resource cap or
  changing production behaviour. **Date/Author:** 2026-08-30, implementation
  agent.

- **Decision:** Guard the marker fallback in `find_substitution` by its `_`
  prefix before asking `try_match_token` to count either 27-character marker.
  **Rationale:** neither marker can match at any other character, so this
  preserves every output while avoiding an unrelated long-token loop in the
  sigil proof's rejected cases. The dedicated marker proof still drives the
  full matcher directly. Existing characterization and property tests remain
  the behavioural regression contract. **Date/Author:** 2026-08-30,
  implementation agent.

## Rebase integration note

2026-08-30 — rebased onto `origin/main` at `95d90181` (including `#565` and
`#597`). The rebase found that `#565` had converted `cmd_interpolate` into a
module directory and introduced a POSIX security boundary that rejects a
recognized placeholder inside backticks. Keeping the old one-pass scanner or
its preservation-oriented Proptest oracle would have both regressed that
security fix and made the branch fail to compile.

- **Decision:** retain the target branch's fallible `SubstitutionTraversal` and
  place the two Kani proofs in `src/ir/cmd_interpolate/verification.rs`. The
  proofs still drive the private production matchers directly, so their stated
  sigil and marker boundary contracts are unchanged.
- **Decision:** retain the target branch's dynamic Kani mutation-evidence
  discovery rather than restore this plan's superseded static inventory. Extend
  it narrowly for the three named Proptest mutation patches, which have no Kani
  harness because they cover the scanner and command guard outside the feasible
  Kani resource bound.
- **Finding:** all five interpolation mutation patches were stale because the
  source moved from `src/ir/cmd_interpolate.rs` to
  `src/ir/cmd_interpolate/{mod,substitution}.rs`. They were regenerated against
  the refactored production code and `git apply --check` now accepts each one.
- **Finding:** the independent scanner oracle must return either substituted
  text or the original template as an invalid-command outcome. This preserves
  independent agreement coverage while accurately modelling the target branch's
  protected-placeholder rejection.
- **Progress:** replay completed through all ten commits. The focused
  `cargo check --tests`, all five mutation-patch applicability checks, and the
  complete deterministic suite passed: `make check-fmt`, `make test` (2,647
  passed, 3 skipped, plus doctests), `make typecheck`, `make lint`,
  `make markdownlint`, and `make nixie`. The capped `make kani-ir` run verified
  all 15 harnesses with zero failures. Final `coderabbit review --agent`
  completed with zero concerns.

2026-09-01 — rebasing onto `origin/main` at `22cf1091` incorporates the target
branch's `RecipeShell` separation. POSIX routes preserve the protected-region
security boundary, while PowerShell treats backticks as native escapes and does
not apply POSIX `shlex` validation.

- **Decision:** retain the target branch's `RecipeShell`-aware scanner and
  command guard. The residual scanner and guard properties now construct raw
  `CommandBindings` with `RecipeShell::Posix`, which keeps their independent
  specification aligned with the POSIX-only backtick contract while preserving
  the target's PowerShell apostrophe property.
- **Finding:** Weave structurally auto-merged the residual Proptest commit but
  duplicated its module header and left a stray delimiter. The rebase's
  per-commit `cargo check --tests` checkpoint detected the malformed module
  immediately; the repair retained the target shell-aware test seam and the
  branch's properties.
- **Finding:** the scanner and guard mutation patches moved with their
  production seams. They were regenerated to disable protected-region handling
  in `SubstitutionTraversal` and to validate the original template instead of
  the substituted command, respectively. All five interpolation patches now pass
  `git apply --check`.
- **Progress:** the rebase is awaiting the prescribed deterministic and
  Markdown gates. Once those pass, update this plan to `COMPLETE`, amend the
  replayed integration commit, and publish the rebased branch with lease
  protection.
- **Progress:** the post-rebase deterministic gates passed on 2026-09-01:
  `make check-fmt`, `make test` (2,735 nextest tests plus doctests),
  `make typecheck`, `make lint`, `make doc-coverage` (99.10%),
  `make markdownlint`, and `make nixie`. CodeRabbit review remains pending and
  is deliberately requested only after this clean gate result.
- **Progress:** `coderabbit review --agent` was unavailable after the clean
  gate run: the first request closed its WebSocket and two later requests hung
  at `connecting_to_review_service` (the last for 20 minutes). None returned a
  rate-limit response or a review verdict. This is a hosted-service limitation,
  not a source concern; no further review retry is pending. The implementation
  is complete subject to final commit, lease-protected publication, and hosted
  CI triggered by that publication.

## Outcomes & retrospective

This work adds two exhaustive Kani kernel proofs without changing command
interpolation behaviour. The historical eight-character sigil proof is
superseded by the target branch's marker-only lowering; the post-rebase
shell-variable and marker proofs establish the active boundary-independent
matching decisions. The full scanner and guard could not meet the cap even at
six characters, so the independent Proptest properties cover the 256/8 residual
range instead. `ADR-004` records that boundary, the local-oracle policy, and
its limits.

The developers' guide now records the established `$$in` and marker-asymmetry
behaviour. `docs/users-guide.md` remains unchanged because that contract is not
a user-visible behaviour change. The unpatched cycle-wrapper harness remains
the separately tracked roadmap 4.2.2 issue #585; it is not absorbed here.

## Revision note

**Revision 2 (2026-08-24).** Rewritten after a six-lens design review.

What changed and why:

- The `PlanBuffer` kernel is withdrawn and replaced by a three-line
  `substitute_chars` seam. The review found it disproportionate, found that it
  would introduce a capacity bound into unbounded security-sensitive production
  code with no good overflow resolution, and found its premise false —
  `find_substitution` already takes `&[char]` and allocates nothing. Constraint
  4 now forbids the original approach.
- Revision 1's `INV-PLAN-SOUND` mutation patch was wrong in sign: the proposed
  `pos + len + 1` → `pos + len` change makes *nothing* match, so soundness
  would have held vacuously. Two reviewers found this independently.
  `OBL-SIGIL` now uses a biconditional with a correct control, and the plan
  warns explicitly about the two different `len` conventions in the file.
- Revision 1's alphabet made `try_match_token` structurally unreachable, so a
  mutation deleting the entire marker arm would have survived every harness.
  `OBL-MARKER` is new and exploits the existing `token: &str` parameter.
- `OBL-ODD` and `OBL-GUARD` now mandate harness-local oracles; without them the
  assertions compared production predicates against themselves and the
  mutations flipped both sides.
- The `INV-GUARD-A`/`-B` split is dropped: `shlex::split` is called
  unconditionally, so restricting the alphabet makes it total, not absent, and
  the split paid full price for no saving.
- `OBL-SPEC` replaces a directly stated backtick-preservation property with
  differential agreement against a declarative specification — stronger, and
  free of output-offset arithmetic. It carries an explicit oracle-independence
  check.
- `OBL-PATCHES` is new: nothing in the repository validates the mutation
  patches, and there are already twelve for thirteen harnesses.
- Encoding is now mandated as symbolic `u8` widened by `char::from`, after the
  review established that `Arbitrary for char` and symbolic UTF-8 across the
  `&str` boundary were the dominant hidden costs.
- Budgets are now measured rather than guessed: the CI baseline is 4:18-4:43
  with about a minute of solving, the local tolerance drops from 14 to 8
  minutes, and a measured CI-step tolerance is added.
- Operational gaps closed: the `LD_LIBRARY_PATH` requirement and the `timeout`
  half of the resource cap, both learned on roadmap 4.2.2 and both missing from
  revision 1; the missing `Artefacts and notes` section; the 400-line pressure
  on `cmd_interpolate.rs` itself; lint friction in harness code; a mechanical
  rather than judgement-based EP-M0 stop; and realistic scope tolerances.
- EP-M1 is reordered so no milestone has negative value if the work stops, and
  a temporary differential oracle guards the refactor because the existing
  suite provably cannot detect a boundary-rule regression.

Effect on remaining work: the production change shrinks from a new type family
to three lines, the harness count drops from six to five, one contract test is
added, and three decisions now require explicit maintainer acceptance before
their milestones start.

**Revision 2.1 (2026-08-24).** Rebased onto `origin/main` at `7e5c2679`. No
conflicts and no substantive change: that commit adds target descriptions and
`netsuke help targets`, touches nothing this plan modifies, and its single
`src/ir/` edit is a comment reinforcing that target descriptions stay out of
recipe resolution. Line references into `docs/roadmap.md`,
`docs/developers-guide.md`, `Cargo.toml`, and `src/ir/from_manifest.rs` were
refreshed, and a note now warns that such numbers drift — verify an anchor by
heading or symbol name before trusting it. Remaining work is unaffected.

**Revision 2.2 (2026-08-24).** The maintainer ratified all three decisions
flagged as requiring acceptance: the `RM-4.2.3.d` reformulation as a
guard-placement biconditional, harness-local oracles including the declarative
specification for `OBL-SPEC`, and a string-level bound below the roadmap's
stated 256 characters and 8 placeholders. The third was accepted in advance, so
EP-M0 no longer sets the plan to `BLOCKED` when the string-level bound falls
short — it records the achieved bound, the residual gap, and the Proptest
hand-off and continues. The mechanical stop for a sigil-shape shortfall below
an 8-character window is unchanged, because that would undermine the plan's
premise rather than narrow its reach. Planning dates were also corrected from
2026-08-17 to 2026-08-24; the earlier value was wrong and two commit author
dates still carry it. No obligation, milestone, or artefact changed. The plan
remains `DRAFT` pending approval to begin implementation.

**Revision 2.3 (2026-08-24).** Rebased onto `origin/main` at `aa93ef8b`, across
nine commits, with no conflicts and no change of substance. A semantic diff over
`src/ir` shows a new `DependencyOrder` enum and one added `BuildEdge` field
from roadmap 3.14.3; `cmd_interpolate` is untouched, the harness count is still
thirteen, and the patch count is still twelve, so every obligation and both
follow-up issues stand as written. Two operational notes were added to `Risks`:
`main` has edited the Makefile, which invalidates the Kani cache because the CI
cache key hashes it and has no `restore-keys`, so the next timing will be
pessimistic; and a merge-gating `build-test-windows` job now exists alongside
`kani-smoke`. Citations to Markdown documents were converted from line numbers
to section headings, since two consecutive rebases have moved them and the
numbers cannot be kept true; line numbers are retained only for code, where
`src/ir/cmd_interpolate.rs` has been stable throughout. The plan remains
`DRAFT` pending approval to begin implementation.

**Revision 2.4 (2026-08-30).** The maintainer approved implementation and EP-M0
completed under the required capped wrapper. `find_substitution` verified at
N=8, N=12, and N=16, while the real scanner timed out at both M=8 and M=6. The
plan therefore remains in progress with the pre-accepted small-proof plus
Proptest hand-off; its exact measurements and the no-guard rationale are in
`Artefacts and notes` and the `Decision log`.

**Revision 2.5 (2026-08-30).** EP-M2 added the two allocation-free
`cmd_interpolate` proofs, their mutation patches, and the mutation-evidence
contract test. Both harnesses passed individually with every cover reachable;
their mutations failed the named assertions and the restored proofs passed. The
complete suite first exceeded the five-minute sequential wrapper, but completed
all 15 harnesses under the same cap with Kani's supported
`--jobs 4 --output-format terse` pair. Six earlier mutation patches had rotted
after support-module extraction and were refreshed so the new contract test
validates the whole checked-in inventory.

**Revision 2.6 (2026-08-30).** The pre-accepted EP-M0 hand-off was completed:
the scanner, odd-backtick, and guard-placement obligations are now Proptest
properties over templates no longer than 256 characters containing at most
eight generated placeholders. Their independent scanner and substituted-command
guard oracles rejected all three corresponding mutation patches. EP-M3 and
EP-M4 therefore have no Kani artefacts by design; the bounded proofs remain
limited to the feasible sigil and marker kernels.

**Revision 2.7 (2026-08-30).** EP-M6 completed the documentation, ADR, and
roadmap close-out. The unused `cfg(kani)` raw-bindings constructor was removed
after the M0 hand-off left it without a proof consumer. Final deterministic
gates passed, the capped parallel Kani suite verified 15 of 15 harnesses with
zero failures, and CodeRabbit returned zero findings. The plan is complete;
hosted CI remains a separate pending state.

**Revision 2.9 (2026-09-01).** The current rebase targets `origin/main` at
`22cf1091`, which makes recipe shell selection explicit. The plan now records
the POSIX-only protected-placeholder invariant, preserves PowerShell's native
backtick escapes, and records the checkpoint-detected Weave auto-merge repair.
It remains in progress pending fresh deterministic and Markdown gate evidence.

**Revision 2.10 (2026-09-01).** Record the successful deterministic gate run
for the fully repaired rebase. The plan remains in progress only for the
required post-gate CodeRabbit review and publication steps.

**Revision 2.11 (2026-09-01).** Complete the implementation after recording the
final deterministic gate evidence and three unavailable CodeRabbit review
attempts. The review service yielded no verdict or concern, and no rate-limit
response required a delayed retry. Publication and hosted CI remain separate
external states.

**Revision 2.12 (2026-09-02).** Reopen the plan for the review-repair round.
Documentation now tracks the implemented `cmd_interpolate` module layout,
shell-aware `SubstitutionTraversal`, and Kani mutation exclusions. The ADR date
format is corrected. Code and test repairs, deterministic gates, and the
requested CodeRabbit re-review remain pending; no new validation result is
recorded here.

**Revision 2.13 (2026-09-02).** Complete the review-repair round after all
required local validation passed: `make check-fmt`, `make test`,
`make typecheck`, `make lint`, `make doc-coverage`, `make markdownlint`, and
`make nixie`. The full `make kani-ir` run passed 15/15 after 1,200 seconds, with
`LD_LIBRARY_PATH` supplied for the existing pinned Kani toolchain. The
targeted marker proof passed all four covers in 885.665 seconds, and the sigil
proof passed all five covers in 47.017 seconds. A CodeRabbit re-review will be
requested next; its outcome is not yet recorded.

**Revision 2.14 (2026-09-02).** Reopen the plan for one post-review code-health
repair. CodeScene confirmed that the deliberate string-heavy property-oracle
inputs are non-actionable. The duplicate PowerShell command/script test setup
was fixed with a shared test-only assertion while retaining separate
entry-point tests. Deterministic gates and the queued CodeRabbit review must be
reconfirmed; no outcomes are recorded yet.

**Revision 2.15 (2026-09-02).** The post-CodeScene gate run found Whitaker's
`no_expect_outside_tests` violation on the extracted helper. The helper and its
two test entry points now return and propagate `Result<(), IrGenError>` rather
than calling `expect`. Revalidation remains pending.

**Revision 2.16 (2026-09-02).** The Result-returning helper then triggered
Clippy's `panic-in-result-fn` check because it asserted. The helper now only
prepares and interpolates, returning `Result<String, IrGenError>`; separately
named test entry points assert the expected values. Revalidation remains
pending, and the plan remains `IN PROGRESS`.

**Revision 2.17 (2026-09-02).** The `panic-in-result-fn` check also applies to
Result-returning tests. The shared helper remains fallible and assertion-free,
while the two recognized void `#[test]` functions now call it with test-boundary
`expect` and assert the outputs. Revalidation remains pending, and the plan
remains `IN PROGRESS`.

**Revision 2.18 (2026-09-02).** Complete the post-CodeScene repair after the
green gate run: `make check-fmt`, `make test` (2,737 tests, 3 skipped, plus 32
doctests with 6 ignored), `make typecheck`, `make lint`, doc coverage at 99.10%,
`make markdownlint`, and `make nixie` all passed. The prior full
`make kani-ir` result remains applicable: 15/15 proofs passed. CodeRabbit
review remains queued.

**Revision 2.19 (2026-09-02).** During the rebase onto `origin/main`, the
target branch's recipe-placeholder lowering became marker-only. Literal `$in`
and `$out` are shell variables and must pass through unchanged; only the
internal `INS_TOKEN` and `OUTS_TOKEN` markers are recognized by
`find_substitution`. This supersedes the earlier sigil-placeholder contract
while preserving the historical observations and measurements above.

- **Decision:** retain the target branch's marker-only lowering and its
  underscore-prefix guard. The shell-variable harness is named
  `shell_variable_prefix_does_not_match`; `marker_token_match_is_exact` drives
  both real marker constants through `find_substitution`.
- **Decision:** remove superseded sigil harness and mutation-patch references
  from the active plan. The marker mutation reference remains
  `docs/verification/mutations/ir__cmd_interpolate__verification__marker_token_match_is_exact.patch`;
  the live mutation-evidence contract remains the authority for current patch
  applicability.
- **Progress:** updated the active contract, harness inventory, obligation
  names, milestone references, and verification commands in this plan, plus the
  corresponding developers' guide, formal-verification methods, and ADR. The
  post-rebase review remains pending.
- **Validation:** after the rebase, `make check-fmt`, `make test`,
  `make typecheck`, `make lint`, doc-coverage (99.13%), `make markdownlint`, and
  `make nixie` passed. Focused capped Kani runs passed
  `shell_variable_prefix_does_not_match` with 3/3 covers in 24.92348s and
  `marker_token_match_is_exact` with 4/4 covers in 97.312706s. Full
  `make kani-ir` compiled and passed the command-interpolation proofs before
  reaching the existing five-minute cap at
  `ir::from_manifest::verification::missing_rule_shape_is_rejected` (exit 124);
  it is not recorded as a full-suite pass.

**Revision 2.20 (2026-09-02).** A post-rebase review confirmed the active
marker-only contract and identified follow-up coverage and documentation work.
The developers' guide now requires a caption for the Kani harness inventory;
the standalone compile-time fixture, test refinements, and supplemental-patch
checks remain part of this repair round. The focused absent-path and
normalizer-failure properties passed, as did the 34 command-interpolation tests
and all five Kani `cfg` UI tests. On this Linux workspace,
`make SHELL=bash test` passed with 2,782 tests and 3 skipped; the full gate set
also passed: `make check-fmt`, `make lint`, `make typecheck`, `make test`, doc
coverage at 99.13%, `make markdownlint`, and `make nixie`. Windows-host
execution remains pending locally because this workspace is Linux. The hosted
`build-test-windows` Test step passed `make SHELL=bash test` in source run
`33664620259` for commit `d9495016`. The full `make kani-ir` run is
deliberately not claimed because the capped run timed out in the unrelated
manifest harness recorded in Revision 2.19. CodeRabbit was queued through
`comenq` as queue id `2ebbb791`, with an approximate posting delay of 3h15m, so
no fresh CodeRabbit verdict exists yet. The plan remains `IN PROGRESS` pending
the CodeRabbit verdict.

**Revision 2.21 (2026-09-02).** Inspection confirmed that
`src/ir/cmd_interpolate_property_support.rs` is private test support under
`cfg(test)`. Its generated templates, bindings, and fragments must remain
unconstrained `String`/`&str` values so tests cover placeholders, backticks,
quotes, empty values, and malformed values. A file-scoped CodeScene suppression
was added for the String Heavy Function Arguments metric rather than
introducing newtypes or refactoring the test seam. Validation completed: the
JSON scope assertion passed;
`cargo test --lib ir::cmd_interpolate::property_tests::scanner_agrees_with_independent_specification`
passed (1/1); and `make check-fmt`, `make markdownlint`, and `make nixie`
passed.

**Revision 2.22 (2026-09-02).** The minimal compile-fixture refactor now
centralizes the duplicated cfg(kani) compile-pass success and stderr assertion
in `assert_kani_fixture_compiles` within `tests/kani_cfg_ui_tests.rs`. The two
named contract tests and all failure tests, helpers, and fixtures remain
unchanged. Validation passed with `cargo test --test kani_cfg_ui_tests` (5
passed), `make check-fmt`, and `make lint`. CodeRabbit remains queued with no
fresh verdict, so the plan remains `IN PROGRESS`.
