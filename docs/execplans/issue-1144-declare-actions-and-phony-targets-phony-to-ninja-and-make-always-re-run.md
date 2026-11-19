# Issue 1144: Declare actions and phony targets phony to Ninja, and make always re-run

This ExecPlan is a living document. Keep `Progress`, `Surprises & discoveries`,
`Decision log`, and `Outcomes & retrospective` current as implementation
proceeds.

Status: IN PROGRESS

Issue: [#1144](https://github.com/leynos/netsuke/issues/1144)

## Purpose and big picture

Netsuke currently lowers every executable edge to one ordinary Ninja build
statement whose output is the entry's declared name. An action such as `test`
therefore produces a Ninja output literally named `test`. Ninja treats that
name as a file: once a directory or file called `test` exists in the project,
the action is considered up to date and never runs again. The same defect
affects a `phony: true` target. Separately, `always: true` only lowers to an
edge-level `restat = 1`, which re-stats outputs after a run but never makes the
edge dirty, so an `always` target whose output exists does not re-run.

The fix is confined to the Ninja backend:

- An executable `phony` edge lowers to a hidden, content-addressed executable
  output under the reserved `.netsuke/phony` namespace, named publicly by a
  native Ninja `phony` alias. A same-named file or directory can neither
  suppress the command nor be removed by `clean`.
- Every executable `phony` edge and every non-phony `always` edge gains one
  shared no-input `phony` node as a render-time implicit dependency, so the
  recipe is dirty on each invocation. Edge-level `restat = 1` stays on non-phony
  `always` edges so an unchanged output still prunes downstream work.

The IR, graph identities, help output, defaults selection, and determinism
contracts are unchanged. Behaviour is proved by repeated real-Ninja CLI tests.

## Constraints

- Change only the Ninja backend. Do not change `BuildEdge`, the graph, the
  manifest schema, or public APIs.
- Apply identical semantics on the string path (`generate_into_with_binding`)
  and the bundle path (`generate_bundle_inner`, including serial staging).
- Reserve `.netsuke/phony` for all five edge path fields, exactly as the serial
  and dyndep namespaces are reserved.
- Keep dependency-only lowering (the `build all: phony | deps` statement)
  untouched.
- Keep byte-identical output for graphs that need neither a hidden output nor a
  force node.
- Hidden outputs and the force node must not become default roots.
- Wrap prose at 80 columns; follow the documentation style guide.

## Evidence

Probed against the repository's Ninja 1.11.1 in a scratch directory:

- `build .netsuke/phony/force: phony` with no inputs, used as
  `| .netsuke/phony/force`, makes the consumer run on every invocation.
- `build public: phony .netsuke/phony/edge-x` runs the hidden recipe even when a
  directory named `public` already exists.
- `ninja -t clean` skips the phony alias, leaving a user's same-named file or
  directory in place, and removes the hidden output.
- Ninja creates the `.netsuke/phony/` directory for the hidden output.
- `restat = 1` on a forced edge still re-runs the recipe but prunes a
  downstream edge when the output is unchanged.

## Plan of work

1. Implement the lowering in `src/ninja_gen`:
   - Add `force_run.rs` owning the reserved force path, hidden-output
     derivation, graph-level force predicate and the force statement.
   - Expose the staging content-addressing helper for reuse.
   - Reserve `.netsuke/phony` in `dyndep::reject_reserved_paths`.
   - Classify edges in `edge_metadata.rs`.
   - Render the hidden statement and alias, and append the force dependency, in
     `display_edge.rs`; thread the new metadata through `mod.rs` and
     `dyndep/render.rs`.
   - Update unit tests, snapshots and property-test expectations.
2. Add `tests/phony_always_runtime_tests.rs` with Unix-only repeated real-Ninja
   CLI tests for actions, `phony` targets, `always` targets with `restat`
   pruning, and `clean` preservation.
3. Reconcile documentation: users' guide, READMEs, design document, a new ADR,
   the developers' guide and the roadmap, then run the gates.

## Progress

- [x] Reconnaissance of the Ninja emitter, IR, tests and docs.
- [x] Empirical validation of the Ninja force/alias/clean/restat behaviour.
- [x] ExecPlan written.
- [x] Backend lowering implemented.
- [x] Runtime end-to-end tests added.
- [x] Documentation and ADR updated.
- [x] First CodeRabbit review answered: inherited `restat` clearing,
      reserved-path
      normalization, default-target coverage, the stray force node, and the
      documentation wording findings.
- [x] Second CodeRabbit review answered: separator-independent namespace
      normalization, nofollow verification of the namespace, localized cleanup
      contexts, and two documentation corrections.
- [x] Third review's blocking finding fixed: a declared path that resolves to
      the force node through an absolute spelling, a `..` re-entry, or a
      symbolic-link alias now stops the reset instead of being deleted.
      Regression tests cover each spelling, and properties compute the
      expectation from the workspace rather than assuming an above-root path
      is safe.
- [x] Migration notice added to the v0.1.0 guide, `.netsuke/phony` added to its
      reserved-namespace paragraph, and the command-layer boundary recorded in
      the developers' guide and ADR-0084.
- [ ] Gates rerun on the fixed tree and the review cleared.

## Surprises & discoveries

- The caution text the issue says is in `docs/users-guide.md` under "Targets,
  inputs, and dependencies", and the known-limitation bullet in the README, do
  not exist at this revision. Nothing matching "same-named", "runs every time"
  or the phony limitation is present. The removal step is therefore a no-op;
  the replacement prose still needs writing so the new behaviour is documented.
- The roadmap task the issue cites is in `docs/roadmap-hexagonal-hardening.md`
  (26.2.2), not `docs/roadmap.md`.
- `BuildEdge::phony` is currently ignored by the emitter; the determinism
  generator already draws it and documents that the emitter ignores it.
- `tests/features/ninja.feature` asserts `build clean:` as a substring, which
  the new alias `build clean: phony .netsuke/phony/edge-…` still satisfies.
- The localized `ninja_gen.reserved_output_path` message named the serial
  dependency state, which is wrong for the phony namespace. The English
  catalogues (`en-US` source and `en-GB`) now say "generated build state"; the
  other 33 catalogues keep the earlier wording until a dedicated translation
  change, because inventing translations is forbidden. The ADR records that gap
  and the users' guide names all three namespaces precisely.
- A manifest that declares only `actions` still has to declare `targets`; its
  absence fails compilation with `missing field 'targets'`. The runtime-test
  fixture therefore writes `targets: []`.
- The per-edge `description` binding is derived from
  `BuildEdge::progress_label`, not from `Action::description`.
  `Action::description` names the shared rule, so a test that wants to prove
  the hidden statement keeps its progress output must set the edge field.
- Dependency-only aggregates render their dependencies after `|`:
  `build all: phony | dep`. The `phony dep` spelling used in early prose drafts
  is not what the emitter produces.
- ADR-0084 is the next free number. 0082 and 0083 are held by other open
  branches, so every local and remote ref was swept before allocating.
- A console `action` now carries `pool = console` on its hidden statement, not
  on the alias; the alias runs nothing. `test_support::ninja_edge` therefore
  follows a generated `phony` alias to the statement it relays to, so a BDD
  scenario written against the manifest name still asserts the binding that
  takes effect.
- An existing BDD scenario pinned `| implicit-deps || order-only-deps`. The
  force node is appended as the last implicit dependency, so the pinned text
  now names it. The assertion was updated rather than the lowering, because the
  ordering is the contract: implicit dependencies, then the force node, then the
  `||` marker.
- Path reservation had to compare normalized spellings. `./.netsuke/phony/force`
  and `out/../.netsuke/phony/force` reach the same Ninja node as the literal
  path, so the check folds `.`, empty, and `..` components before matching and
  runs over every edge field plus the default targets. A `..` that would climb
  above the root cannot resolve to a reserved path and is accepted.
- A stray path at `.netsuke/phony/force` makes the no-input phony node look
  present, and Ninja then treats every forced edge as clean. Generation cannot
  repair that without inspecting the filesystem, which the determinism claim
  forbids, so `netsuke build` removes the path before it invokes Ninja. The
  `generate` command stays pure and cannot clear it for a hand-run file.
- The hidden statement inherits the action rule's bindings, so an action that
  sets `restat = 1` would otherwise carry it onto a path that is never written,
  and Ninja would prune dependants whose work was never up to date. The hidden
  statement now clears the inherited value with an empty `restat =` binding,
  the same way the pool writer clears an inherited console pool.
- The Whitaker suite's `no_std_fs_operations` lint rejected a locally written
  execute-bit probe in `test_support::ninja`. The crate already owns that
  wrapper: `test_support::fs::is_executable_file` is the ambient filesystem
  boundary, so the resolver delegates to it on Unix instead of duplicating the
  mode check. This is the reuse case the boundary module exists for, and it
  avoided a new exemption rather than requiring one.
- The spelling gate reads unbackticked `restat` as a typo for `restart`, while
  backticked `restat` is left alone. One ADR sentence had used the word as
  prose. Marking the identifier as code fixed the gate without an entry in
  `typos.local.toml`, which is the right place only when an identifier must
  appear unquoted.
- The CodeScene delta gate, which is advisory rather than a required check,
  flagged three changed files. The two `Action` builders in
  `tests/ninja_phony_lowering_tests.rs` restated the same struct literal, so
  they now share one constructor and the file scores 10.00. The remaining
  findings were "String Heavy Function Arguments" in the Ninja-text lookup
  helpers and in the BDD steps, where the strings are the subject under test;
  both files carry a narrowly scoped, reasoned rule-set entry in
  `.codescene/code-health-rules.json`, and `cs check` reports 10.00 for each.
- The determinism strategy's `always_restat_seen` bit exists to prove the
  public `restat` override was exercised. A hidden phony statement never
  carries that override, so those edges no longer set the bit, while the raw
  `phony` flag count stays a raw count because a dependency-only flagged edge
  never takes the hidden branch either.
- The first gate batch was red for three mechanical reasons, all fixed before
  rerunning: `missing_const_for_fn` on `classify` and `edge_needs_force`, and
  the two BDD expectations above. A red batch whose failures are expectations
  is cheaper to diagnose from its log than from a rerun.
- Two determinism mutation patches pin a `src/ninja_gen/mod.rs` hunk that moves
  the `default` emission ahead of the build statements. Because the force
  statement now sits between `write_action_rules` and the edge loop, that hunk
  no longer applied and `every_patch_applies_cleanly` failed. The patches were
  regenerated from a mutated copy of the current file so the seeded fault still
  moves the same statement, then re-checked with `git apply --check`.
- Three Ninja snapshot tests and the conditional-dependency fixture's
  dependency-class helper had not yet run when the first batch stopped. The
  helper now follows a generated `phony` alias through
  `test_support::ninja_edge::edge_build_line`, and the snapshots were
  regenerated and reviewed hunk by hunk: each shows the force statement, the
  hidden executable statement with the force implicit dependency, and the
  public alias.
- The second review found that the normalized reserved path was rebuilt with
  `Utf8PathBuf::push`, which inserts the platform separator. On Windows that
  yields `.netsuke\phony\force`, which never matches the slash-separated
  namespaces, so reservation would silently stop working there. Normalization
  now joins the surviving `Normal` components with `/` and builds no path at
  all; an absolute or prefixed path returns `None`, and a literal backslash
  stays an ordinary character on Unix, which a `#[cfg(unix)]` test pins.
- `open_dir_nofollow` alone accepts a directory symbolic link with the pinned
  capability implementation, so opening `.netsuke/phony` that way would still
  have let a link redirect the recursive removal. Each namespace component is
  now inspected with `stat(..., FollowSymlinks::No)` first and refused unless
  it is a real directory, mirroring the glob walker's documented literal-prefix
  check. The `force` entry is removed through the verified handle, as a
  relative name that cannot climb out.
- The localization audit requires every declared key in every catalogue, so the
  two new cleanup contexts could not be English-only. Both were translated into
  all 35 catalogues, with the right-to-left marks preserved for `ar`, `fa` and
  `he`. The refusal itself uses `io::ErrorKind::NotADirectory`, whose standard
  library text supplies the reason without authoring an untranslated sentence.
- Two documentation claims did not survive review. The ADR's Option C said a
  same-named directory "remains up to date", which contradicts the force-node
  behaviour that makes the edge dirty; the objection to that option is `clean`,
  not freshness. The users' guide said `generate` leaves the working tree
  untouched, which contradicts the dyndep sidecar materialization and pruning
  documented elsewhere; the claim is now scoped to `.netsuke/phony/force`.
- The third review's blocking finding showed that lexical comparison over the
  graph's spelling cannot decide an alias. `/project/.netsuke/phony/force` and
  `../project/.netsuke/phony/force` (from `/project`) both fold to nothing and
  both name the node. A capability handle cannot help either: with the pinned
  `cap-primitives`, `Dir::canonicalize` resolves through `openat2` with
  `RESOLVE_BENEATH`, so any `..` that leaves the workspace root fails rather
  than returning to it. Resolution therefore needs the absolute working
  directory, which is why `reset_force_node` resolves the selected `-C` path
  once and canonicalizes the longest existing ancestor of each declared path
  against it. The read is confined to this comparison; the removal itself still
  goes through the verified namespace handles.

## Decision log

- Hidden executable output lives at `.netsuke/phony/edge-<sha256>`; the force
  node is `.netsuke/phony/force`. The `edge-` prefix keeps the force node from
  being a parent of any hidden output and makes collision impossible.
- The hidden identity hashes the explicit then implicit outputs, NUL-separated
  within a group and separated by `\u{1}` between groups, under the schema tag
  `netsuke-phony-v1`. Paths cannot contain control characters, so the encoding
  is unambiguous.
- The force statement is emitted once at the start of the build-statement
  section, and only when a rendered edge consumes it.
- The alias is `build <explicit> | <implicit>: phony <hidden>`; it deliberately
  carries no force dependency because the hidden output is already dirty.
- `restat` is suppressed on hidden statements and retained on public non-phony
  `always` statements. An inherited rule-level `restat = 1` is cleared with an
  empty binding on the hidden statement.
- Reserved namespaces are matched against a lexically normalized path, over
  every edge path field and the default targets. A path that climbs above its
  root with `..` is not reserved, and the check says so once instead of falling
  back to the raw spelling for that case.
- `netsuke build` clears a path left at `.netsuke/phony/force` before invoking
  Ninja. The generator stays a pure translation of the graph, and the removal
  belongs to the command that owns build state.
- The reservation rule is split by what each layer can know.
  `ninja_gen/reserved_paths/mod.rs` owns the namespace constants and the pure
  folding of a graph spelling. The runner owns working-directory resolution,
  because it already selects the directory Ninja runs in and can read the
  filesystem without making generation impure. A declared path the runner
  cannot judge either way is not refused; only a resolution that lands in a
  reserved namespace stops the reset.
- The alias refusal reuses `ninja_gen.reserved_output_path` rather than adding
  a third message, so users meet one explanation for one rule and no new
  translation sweep is needed.
- The normalized reserved-path comparison is a `/`-joined string of the
  surviving components, never a rebuilt `Utf8PathBuf`, so the match is
  independent of the platform separator. A path that is absolute, carries a
  prefix, or climbs above its root is not treated as reserved.
- The reset inspects `.netsuke` and `.netsuke/phony` with
  `stat(..., FollowSymlinks::No)` and refuses either component when it is a
  symbolic link or not a directory. The open that follows uses
  `cap_primitives::fs::open_dir_nofollow`, which refuses a link as the final
  component itself — `O_NOFOLLOW` on Unix, a reparse-point check on the opened
  handle on Windows — so a link swapped in after the inspection fails the open
  instead of yielding a handle that points elsewhere. Removal happens through
  that verified handle, so a link cannot redirect a recursive delete outside
  the namespace. A test pins the primitive's refusal, so a dependency change
  that dropped it fails rather than silently weakening the reset.
- Both cleanup contexts are localized Fluent keys carrying `$path`
  (`runner.io.phony.inspect` and `runner.io.phony.reset`), and the refusal uses
  `io::ErrorKind::NotADirectory` rather than authored English prose.

## Acceptance criteria

- An action runs on every request when a same-named file or directory exists,
  including after a previous successful run.
- A `phony: true` target behaves the same way.
- `always: true` re-runs a file-producing target whose output is up to date,
  while an unchanged output still prunes dependants.
- End-to-end tests with a real Ninja cover all three cases.
- The caution and known-limitation text is absent from the documentation and
  the new behaviour is documented with migration notes.

## Outcomes & retrospective

The lowering is implemented and proved end to end.

- `src/ninja_gen/force_run.rs` owns the reserved force node, the hidden-output
  identity, the graph-level force predicate and the force statement.
- `edge_metadata.rs` classifies each edge as dependency-only, executable, or
  executable phony, and carries the hidden output and force flag to the
  renderer.
- `display_edge.rs` renders a hidden statement plus a native `phony` alias for
  an executable phony edge, and appends `.netsuke/phony/force` to the implicit
  dependency list of every forced edge.
- `dyndep/render.rs` and `mod.rs` both call `write_force_statement`, so the
  bundle and string paths agree byte for byte.
- `.netsuke/phony` joins `.netsuke/serial` and `.netsuke/dyndep` as a reserved
  namespace for every edge path field.

Verification that has actually run: the generation-side suite in
`tests/ninja_phony_lowering_tests.rs`, the real-Ninja CLI suite in
`tests/phony_always_runtime_tests.rs` with `NETSUKE_REQUIRE_NINJA=1`, the
`dyndep::tests::paths` matrix, and `tests/ninja_gen_unit_tests.rs` all pass.

The commit gates were then run as one sequential batch on the final tree.
`make check-fmt`, `make lint`, `make typecheck`, `make doc-coverage`,
`make markdownlint`, `make nixie`, `make test-workflow-contracts` and
`NETSUKE_REQUIRE_NINJA=1 make test` each exited 0; the test gate ran all 6216
nextest cases with 9 environment-gated skips and passed every doctest target.
Doc-comment coverage measured 98% against the 80% threshold.

Earlier drafts recorded 6050, 6065, 6102 and 6107 cases. Each was a real
measurement of an earlier tree: the first before the runtime and generation
suites added for this change were in the same batch, the second before the
third review's alias tests existed, 6102 before the fourth review's
namespace-open and outside-link tests were added, and 6107 before the branch
was rebased onto `main`, whose own commits added cases. A count belongs to the
tree its batch ran on, so 6216 is the figure for the tree that ships.

The rebase needed one repair beyond conflict resolution.
`src/runner/dyndep/publication.rs` had gained a shared
`runner::path_helpers::open_effective_dir` on this branch while `main` kept a
local copy and, separately, replaced the publication struct's `Dir` field with
a lease. Resolving the overlap kept the shared helper but left the old
`cap_std::fs_utf8::Dir` import behind, which `-D warnings` rejected as unused.
The import is gone; the module has no remaining use for the type.

The first review then raised shape questions the generation tests could not
answer, and the fixes changed executable behaviour:

- A hidden statement now clears an inherited rule-level `restat`, so an
  `always`-style action cannot prune dependants through a path that is never
  written.
- Reserved-path matching normalizes spelling and covers the default targets, so
  a manifest cannot forge generated state through `./` or `..` spellings.
- `netsuke build` removes a stray path at the force node before Ninja runs, with
  a unit test for each path type and a real-Ninja test that pre-creates the
  node, builds twice, and requires the forced recipe to run both times.

The second review found two portability and safety gaps in that reset, and both
fixes are in the shipping code:

- Reserved-path matching no longer rebuilds a path, so the comparison holds on
  Windows, and a Unix filename containing literal backslashes stays a filename.
- The namespace is verified without following symbolic links before anything is
  removed, so a link planted at `.netsuke` or `.netsuke/phony` cannot redirect
  the recursive delete into the workspace.

Its remaining findings were documentation: Option C's freshness wording in
ADR-0084 and the scope of the `generate` no-mutation claim in the users' guide.

The third review found one blocking defect and several documentation gaps, and
all of it is addressed:

- The reset deleted the force node before checking whether the manifest had
  declared that exact path under another spelling. It now resolves every
  declared path against the canonical working directory and refuses the reset
  when one lands in a reserved namespace.
  `src/ninja_gen/reserved_paths/tests.rs` pins the pure folding, and the
  `src/runner/phony_state/tests` module covers an absolute alias, a `..`
  re-entry, a symbolic-link alias, a declared link whose target lies outside
  the workspace, an unrelated path, and properties that derive the expected
  answer from the workspace. One property states the converse explicitly:
  `../<workspace>/...` re-enters the workspace, so an above-root path is not
  automatically safe. A further test pins
  `cap_primitives::fs::open_dir_nofollow` refusing a link as the final
  component, which is the guarantee the reset's TOCTOU argument rests on.

- The v0.1.0 migration guide gained the phony and always notice and names
  `.netsuke/phony` in its reserved-namespace paragraph.
- The developers' guide records the `src/runner/phony_state` boundary, and
  ADR-0084 carries an amendment for the alias guard.

The fourth review confirmed the lowering and raised four more points, all
addressed: the two namespace opens are covered at both components and the
reset's doc comment now states the primitive's own refusal rather than claiming
the inspection alone protects the delete; a declared absolute link whose target
lies outside the workspace is refused by resolving the parent and keeping the
entry name; the runtime test file moved its manifests under `tests/data/` to
stay within the 400-line limit; and both migration guides qualify "no manifest
needs an edit" for a manifest that already declares a path inside
`.netsuke/phony`.

Retrospective: the largest avoidable detour was diagnosing two failing text
assertions whose premises were wrong rather than whose implementation was. One
set `Action::description` and expected an edge-level binding; the other
asserted a `phony dep` spelling the emitter never produced. Both were fixed by
correcting the expectation, and both are now recorded under Surprises so a
follow-on reader does not re-derive them. The second lesson is that a review
comment about Ninja semantics deserves an experiment before an edit: the stray
force node was confirmed on the real binary before the reset command was
chosen, which is why its documented behaviour matches what Ninja does rather
than what the finding assumed.
