# Add trusted configured shell definitions (11.2.2)

This ExecPlan is a living document. Keep `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` current at each milestone and
whenever evidence changes the design.

Status: COMPLETE

The maintainer approved implementation on 2026-11-01. All three milestones are
implemented, gated, and reviewed, and roadmap 11.2.2 is checked. Publication
and the hosted review cycle are owned by `pr-babysitting`; this plan records
the implementation evidence, not the hosted outcome.

## Purpose and observable outcome

Operators can declare additional structured-command shells in trusted system
and user configuration. Loading configuration accepts structurally valid
entries, appends entries in layer order, and rejects reserved or repeated
names. Project files and explicit configuration selectors cannot grant this
interpreter authority. Errors identify the relevant configuration key before
manifest compilation starts.

This task delivers configuration primitives only. A configured shell need not
exist on the current machine to pass configuration validation. Executable
resolution belongs to 11.2.3; structured-command integration waits for 12.3.1.
No new command execution, shell selector flag, manifest field, or project trust
switch is delivered here.

## Constraints

- Preserve ADR-019, including its corrective addenda, and RFC 0011 section 5.
  Reuse `ShellName` and `BuiltInShell`; do not create another name grammar or
  built-in registry.
- Keep configuration, source provenance, filesystem discovery, and environment
  reading outside execution-domain types. A future resolver consumes validated
  definitions; runners must never consult `CliConfig`.
- Accept definitions only from system/user operator discovery roots and their
  inherited occurrences. An arbitrary explicit file, `--config`, or
  `NETSUKE_CONFIG` does not establish this authority. A project occurrence
  never becomes trusted through path equality with another occurrence.
- Reject unauthorized `shells` keys before generic merging, including empty or
  malformed values. Do not silently discard a request and report success.
  Existing fetch-policy trust flags grant no shell authority.
- Definitions have exactly `name`, `executable`, and `args`. Field validation
  happens during deserialization; merged collisions happen in `PostMergeHook`.
- Do not add an aggregate registry-count or registry-byte cap. ADR-019's first
  correction removed that requirement; the argument-size bound is per entry.
- Never mutate the test harness environment. Inject discovery inputs and use
  child-process environment configuration for end-to-end cases.
- Preserve the pinned Polonius Alpha and next-generation trait solver posture;
  add no compiler-mode switches or defensive borrow-checker workarounds.
- Run gates sequentially with shared Cargo caches and compiler admission.
  Stop on full storage; report unhealthy admission without bypassing it.

## Tolerances (exception triggers)

Changes may touch the configuration/discovery/merge modules, their build-time
source slice, shell-name imports, tests, proof files, and the documentation
listed below. No runtime execution or structured-command AST work is allowed.
No new production dependency, public API compatibility layer, global discovery
precedence change, or project-definition trust mechanism is authorized.

Pause and record a proposed deviation if source authority cannot be retained
through the pinned OrthoConfig interface without changing unrelated policy, if
the executable grammar needs a new platform policy, or if the proof cannot be
connected to the production contract. Seek approval for that deviation; do not
weaken the trust boundary, omit evidence, or invent an admission flag. Each
code file remains at most 400 lines. Split by feature responsibility when
needed, documenting ownership and permitted callers before extraction.

## Risks

The existing `FileScope::Operator` means ordinary configuration precedence, not
independently established shell authority. Treating it as sufficient would
trust arbitrary explicit files. Discovery must retain the kind of root that
loaded each occurrence before flattening inheritance.

The same file can occur under both an operator root and a project root. Global
path deduplication can erase the distinction. Preserve occurrence-level labels
and test both orderings. A project occurrence containing definitions fails even
if the same file was also loaded through a trusted root.

OrthoConfig merge deserializes values after layering. A typed field alone may
not detect an invalid entry hidden by later replacement. Validate every trusted
occurrence's entries before merging, using the same deserializer, then validate
merged collisions through the hook. Keep raw layer values until error
collection and JSON preference resolution are complete.

Host path syntax differs. A relative executable must not slip through as a bare
name because a separator has different meaning on the validating host. Define
and test the lexical policy before adding filesystem probes; none belong here.

A proof of a model does not prove Serde, OrthoConfig, filesystem provenance, or
the executable's source-evaluation behaviour. Keep these residual boundaries
explicit and exercise the actual adapters in tests.

## Progress

- [x] 2026-10-20: Rename the working branch and inspect the clean baseline.
- [x] 2026-10-20: Load requested skills and gather two Wyvern reconnaissance
  reports through context pack `pk_3mn3mkwi`.
- [x] 2026-10-20: Draft contracts, proof sketch, milestones, and test strategy.
- [x] 2026-10-20: Complete six-perspective community-of-experts review and
  reconcile architecture, contract, and operations findings.
- [x] 2026-10-20: Pass plan-only formatting, format checks, Markdown/spelling,
  diagram validation, and whitespace checks before publication.
- [x] 2026-11-01: Obtain explicit maintainer approval and rebase onto
  `origin/main` (`da06ffb11`) before implementation.
- [x] 2026-11-01: Re-rebase onto the advanced `origin/main` (`5f0a4d123`) after
  PR #958 landed, and revalidate the full gate set on the rebased candidate.
- [x] M1: Establish definition authority and reject unauthorized sources.
  - [x] 2026-11-01: Confirm the Red stage on the prebuilt binary: a project
    `[[shells]]` entry reaches `manifest_not_found`, while a malformed `jobs`
    value in the same file fails configuration merging, proving the fixture
    works and only enforcement is missing.
  - [x] 2026-11-01: Add root-authority classification, the restricted candidate
    walk, the environment-ingress rejection, and the classifier,
    differential, and end-to-end rejection tests.
  - [x] 2026-11-01: Confirm M1 locally: the focused library suite passes
    (17 of 17: six classifier cases, four properties with their witness, four
    differential cases, and the two pre-existing project-key tests) and the
    end-to-end rejection suite passes (7 of 7), after `make fmt`.
  - [x] 2026-11-01: Repair the M1 Clippy findings the library-only abort
    masked, extracting `ProjectKey` into its own module and grouping the
    discovery merge inputs into `RootTrust` and `ProjectScope`.
  - [x] 2026-11-01: Record `project_key.rs` in the Kani proof scope and group
    the two authority suites into an `authority/` directory module after
    `make test-workflow-contracts` rejected both the uncovered proof input and
    the unrecorded `authority` prefix group.
  - [x] 2026-11-01: Run the M1 gate set at `0df3dff18`. Eight of the eleven
    gates passed: `check-fmt` (6 s), `markdownlint` (34 s), `nixie` (3 s),
    `doc-coverage` (980 s, 98.23 %), `typecheck` (307 s), `test` (1086 s,
    4964 passed), `test-workflow-contracts` (346 s), and `lint-clippy`
    (980 s). `make lint-whitaker` then failed on seven
    `no_std_fs_operations` findings in `tests/cli_tests/shells_authority.rs`,
    so `lint-python` and `github-actions-lint` did not run. The authority
    suite is rewritten for M2, so the capability-handle repair lands with the
    M2 candidate instead of as a separate commit.
  - [x] 2026-11-01: Fold the M1 milestone CLI review into the M2 review.
    M2 replaces the placeholder authority assertion, the schema-less
    rejection, and the whole authority suite, so a review of M1 alone would
    have assessed code that no longer exists.
- [x] M2: Add validated definitions, append merging, and collision proof.
  - [x] 2026-11-01: Add the `shells` field with `skip_cli`, append merging, and
    an empty default, so neither the command line nor the environment can
    declare a definition.
  - [x] 2026-11-01: Add the `src/cli/config/shells/` schema: `ShellDefinition`,
    `ShellExecutable`, `ShellArguments`, the name/executable/argument
    deserializers, and the bounded diagnostic preview.
  - [x] 2026-11-01: Validate each trusted occurrence field by field during
    discovery, so a fault names `shells[index].name|executable|args` before any
    typed merge runs.
  - [x] 2026-11-01: Validate the merged registry in `PostMergeHook`, rejecting
    built-in and repeated names before manifest compilation.
  - [x] 2026-11-01: Extend the build-script slice and its contract test with
    the new modules. The slice now mirrors `src/lib.rs`'s public module tree
    (`pub mod cli`, `pub mod shell_selection`) instead of carrying a
    `#[expect(dead_code)]`: with the tree public, every re-export is reachable
    and the expectation no longer fires.
  - [x] 2026-11-01: Add the Verus refinement proof at
    `verus/configured_shells/main.rs` and the quadratic Rust oracle beside the
    composition tests. `make verus VERUS_FLAGS='--proof-file
    verus/configured_shells/main.rs'` reports `9 verified, 0 errors`.
  - [x] 2026-11-01: Restate the M1 property
        `any_shells_declaration_fails_closed`
    as `untrusted_shells_declaration_fails_closed`. The M1 form asserted that
    *any* `shells` declaration fails, which M2 deliberately relaxes for an
    operator root, so the property now compares the resolver's errors against
    the C1 authority of the declaring occurrences and carries its own witness.
  - [x] 2026-11-05: Run the M2 gate set against the M2 working tree, whose
    `HEAD` was still `0df3dff18` (the M1 repair commit). The gated tree was
    committed afterwards as `343d10337`, which the later rebase replayed as
    `1139f31df`; `0df3dff18` itself is reachable only from the local recovery
    ref `refs/recovery/11-2-2-old-head-11410e7a3`, so a fresh clone cannot
    resolve it. All ten gates passed:
    `check-fmt` (9 s), `lint` (427 s), `doc-coverage` (373 s), `test` (371 s),
    `test-workflow-contracts` (244 s), `test-package-verify` (213 s),
    `test-documentation-contracts` (55 s), `verus` (6 s), `markdownlint`
    (73 s), and `nixie` (5 s). An earlier batch failed
    `test-workflow-contracts` because the new modules were absent from
    `tools/kani/proof-scope.toml`; recording them fixed it, and the full set
    was rerun rather than resuming from the failed gate.
  - [x] 2026-11-05: Run the combined M1–M2 milestone CLI review over
    `e5d57c58..HEAD` (39 files, exactly the plan, M1, and M2 commits). Two
    minor findings were valid and are repaired: the `DriveRelative` message
    cited a rooted path as the drive-relative example, and the `arguments`
    deserializer appended a bounded preview of the offending element to the
    diagnostic, contradicting C5 and the type's own documentation.
- [x] M3: Complete behavioural evidence, documentation, and delivery.
  - [x] 2026-11-05: Add the `configured_shells` feature with four scenarios:
    operator layers append in order, a project file cannot define a shell, an
    explicit `--config` selection does not grant authority, and layered names
    must be unique. Each rejection asserts that no manifest is compiled.
  - [x] 2026-11-05: Add the `tests/cli_tests/shells_startup/` module, which
    drives the real binary through `assert_cmd`: a rejection stops before
    compilation, the JSON preference survives a discovery-time rejection,
    environment aliases are denied, diagnostics stay bounded, and a trusted
    definition neither requires its program to exist nor blocks a build. The
    module is split into `mod.rs`, `diagnostics.rs`, and `success.rs` so each
    file stays inside the 400-line ceiling, with the `insta` snapshots moved
    beside `mod.rs` because `insta` resolves them relative to the source file.
  - [x] 2026-11-05: Add unit coverage for the diagnostic quarantine predicate,
    including the aggregate-nested form of the error and both JSON-preference
    orderings.
  - [x] 2026-11-05: Document the operator contract in the users' guide, the
    configuration sample, the developers' guide, the design document, the
    OrthoConfig guide, and the repository layout, with the users' guide example
    registered as an executable documentation contract.
  - [x] 2026-11-05: Run the full gate set at `f03faf7b0`. All ten gates passed
    in one batch: `check-fmt` (4 s), `lint` (103 s), `doc-coverage` (165 s),
    `test` (59 s), `test-workflow-contracts` (213 s), `test-package-verify`
    (63 s), `test-documentation-contracts` (2 s), `verus` (2 s, 9 verified and
    0 errors), `markdownlint` (27 s), and `nixie` (3 s); the batch finished
    `failed=0`. An earlier batch failed `markdownlint` on an -ise spelling in
    this plan, so the word was corrected and the whole set was rerun rather
    than resumed.
  - [x] 2026-11-05: Run the M3 milestone CLI review with CodeRabbit CLI 0.9.2
    over `1139f31df..f03faf7b0` (`--committed --fresh --base-commit`), which
    reviewed exactly the 24 changed files and completed in 443 s with exit 0.
    Three findings were valid and are repaired: the M2 gate entry named a
    commit that only a local recovery ref reaches, the decision-log entry
    named the wrong file for the `[[shells]]` fence, and two near-identical
    quarantine tests were not parameterized.
  - [x] 2026-11-05: Rebase the branch onto `origin/main` at `ce15e026`, the
    crates.io Trusted Publishing merge. All 15 commits replayed with an
    identical patch (`range-diff` reports `=` for every pair), and the only
    file both sides touched was `docs/developers-guide.md`, where the
    publishing section and the configured-shells section are disjoint. That
    upstream work adds no shell-selection helper, pattern, or convention this
    plan should adopt, so no design or decision entry changes; the rebase is
    recorded here instead.
  - [x] 2026-11-05: Run the full gate set at the rebased candidate
    `205873fc5`, adding `typecheck` to the batch because the rebase hook names
    it. All eleven gates passed in one batch: `check-fmt` (3 s), `lint`
    (208 s), `typecheck` (44 s), `doc-coverage` (77 s, 98.32 %), `test`
    (187 s), `test-workflow-contracts` (258 s), `test-package-verify` (29 s),
    `test-documentation-contracts` (21 s), `verus` (2 s, 9 verified and 0
    errors), `markdownlint` (35 s), and `nixie` (1 s); the batch finished
    `failed=0`. Every verdict recorded before the rebase is superseded by this
    run, because a rebase creates a new candidate.
  - [x] 2026-11-05: Publish the repaired candidate at `bef3ec159` and hand
    delivery to `pr-babysitting`. Hosted evaluation then found the Windows
    unused-import failure and the CodeScene findings listed in the 2026-11-09
    entries below, so the published candidate was repaired and re-gated rather
    than accepted as delivered.
  - [x] 2026-11-09: Repair the delivery findings raised against the published
    candidate `bef3ec159`. Hosted Windows CI failed there with
    `error: unused import: merge_in_child`, because that import was reachable
    only from `#[cfg(unix)]` tests, and CodeScene scored four files below
    10.00. `tests/cli_tests/shells_authority.rs` becomes
    `shells_authority/mod.rs` plus a `support.rs` fixture, with the probe
    imports, `operator_rejection`, and that fixture's `tempdir` import gated
    on `unix`. The host path rules move out of
    `src/cli/config/shells/executable.rs` into `spelling.rs`, registered in
    `build.rs` and `tools/kani/proof-scope.toml`. The startup fixtures
    consolidate into `shells_startup/support.rs`, and a `ProjectFixture` in
    `src/cli/discovery/layers/tests/authority/cases.rs` replaces the repeated
    temporary-project setup. All nine touched or new files score 10.00 under
    `cs check`, and both `cargo check --all-targets --all-features` and
    `make typecheck` exit 0 on Linux. The first gate batch then failed `test`
    on `build_script_rerun_directives_match_the_compiled_module_slice`, a
    second registry of the build-script module slice; adding `spelling.rs` to
    `BUILD_SLICE_RERUN_PATHS` in `tests/build_module_slice_ui_tests.rs` fixed
    it, and the whole set was rerun rather than resumed.
  - [x] 2026-11-09: Run the full gate set on the repaired candidate, whose
    code commits are `893e70029`, `ca04b9ad1`, and `6449d02de`. All eleven
    gates passed in one batch: `check-fmt` (5 s), `lint` (304 s),
    `typecheck` (40 s), `doc-coverage` (317 s, 98.32 %), `test` (444 s),
    `test-workflow-contracts` (273 s), `test-package-verify` (108 s),
    `test-documentation-contracts` (2 s), `verus` (4 s, 9 verified and 0
    errors), `markdownlint` (30 s), and `nixie` (3 s); the batch finished
    `failed=0`.
  - [x] 2026-11-09: Rebase the branch onto `origin/main` at `8dfbab416`, the
    nightly-2026-11-08 toolchain bump. Twenty of the 21 replayed commits
    produce an identical patch (`range-diff` reports `=` for every pair). The
    one conflict is in `tools/kani/proof-scope.toml`: upstream renamed
    `src/recipe_shell.rs` into `src/power_shell_location/mod.rs` and
    `src/recipe_shell/mod.rs`, and this branch inserts `shell_selection`
    entries in the same place. The resolution keeps upstream's two renamed
    entries and the four shell-selection entries, and the file still parses as
    TOML with 189 sources. Upstream work adds no configured-shell convention
    to adopt: RFC 0036 shapes shell parsing and transformations rather than
    configuration authority, and the Windows recipe-interpreter binding is an
    independent change that also demands an absolute executable path.
  - [x] 2026-11-09: Run the full gate set on the rebased candidate, whose
    replayed series ends at `f480699be`. All eleven gates passed in one batch:
    `check-fmt` (3 s), `lint` (4454 s), `typecheck` (495 s), `doc-coverage`
    (1738 s, 98.14 %), `test` (849 s), `test-workflow-contracts` (280 s),
    `test-package-verify` (216 s), `test-documentation-contracts` (48 s),
    `verus` (3 s, 9 verified and 0 errors), `markdownlint` (30 s), and `nixie`
    (4 s); the batch finished `failed=0`. The durations are dominated by the
    nightly-2026-11-08 bump, which invalidated every cached dependency, plus a
    shared compile-slot queue of over 100 waiting requests; no gate waited on
    an unhealthy admission service. Every verdict recorded before this rebase
    is superseded by this run, because a rebase creates a new candidate.
  - [x] 2026-11-09: Repair the first hosted failure the rebased candidate
    `ba61a02cd` produced. The Windows lane failed
    `cli_examples::configured_shell_example_is_accepted_from_an_operator_root`
    because the marked users'-guide example spelled `executable = "/bin/dash"`,
    which Windows rejects as rooted without a drive. The example and the
    commented sample in `docs/sample-netsuke.toml` now use the bare program name
    `dash`, the users' guide states that an absolute path must name a Windows
    drive or UNC share, and
    `cli_examples::configured_shell_example_spells_an_executable_every_host_accepts`
    asserts the documented spelling parses under both host families, so a Linux
    gate now covers the class. The same run's `build-test` and `lint-windows`
    jobs failed earlier, in `Install Whitaker`, on an HTTP 504 fetching
    `dylint-link-x86_64-unknown-linux-gnu-v6.3.0.tgz` from the rolling release;
    that is a transient archive fault and is rerun, not repaired.
  - [x] 2026-11-14: Rebase the 23-commit series from the previous base
    `8dfbab416` onto `origin/main` at `750f56a22`, the publishing-merge
    advance, replaying with `--merge --no-fork-point --no-update-refs
    --no-autostash --reapply-cherry-picks --keep-empty --empty=stop` and
    `zdiff3`. Tracked `.gitattributes` opts `shells/spelling.rs` into the Weave
    driver, so every `rebase` and `--continue` ran with the documented
    command-scoped bypass, `merge.weave.driver='git merge-file --zdiff3
    --marker-size=%L %A %O %B'` plus `merge.weave.recursive=text`; the tracked
    attribute file was left untouched. `range-diff` reports `=` for 20 of the
    23 pairs. The three that differ each keep both sides' intent:
    `tests/cli_tests/merge_probe.rs` adopts upstream's new
    `parse_with_localizer_from(args, &localizer, clap::ColorChoice::Auto)`
    signature; `src/cli/config/mod.rs` keeps the branch's file, whose
    `impl Default for CliConfig` block has moved to `defaults.rs`, while
    upstream's only change there was deleting one blank line inside that block;
    and the documentation commit keeps upstream's reordered
    `EXPECTED_EXAMPLE_IDS` list and repository-layout tree
    (`terminal_rendering/`, `terminal_rendering_tests.rs`) alongside this
    branch's `guide-configured-shell-definition` identifier and renamed
    snapshot directory. No conflict marker survives and `git diff --check`
    is clean.
  - [x] 2026-11-14: Repair the findings the ExecPlan and proof assessments
    raised against `93af3ad3f`. Six changes, each with its own red evidence:
    `is_unc_share` now requires a server and a share name, so `\\`, `//`,
    `\\server`, and `\\server\` are rejected as rooted without a drive while
    `\\server\share` and `//server/share/shell.exe` stay accepted; every
    `shells` occurrence diagnostic, including the unauthorized-key error, now
    ends `; declared in "<path>"` with the path rendered through the same
    bounded, escaped preview as a rejected value; composition errors name
    `shells[index].name` and `shells[first].name` rather than the bare indices;
    the oracle gained a layered property (zero to eight layers of zero to
    sixteen definitions), a regrouping property, and a six-class witness test;
    and the argument budget gained 4095/4096/4097 byte cases built from
    three-byte characters plus a shared-budget case.
  - [x] 2026-11-14: Run the four mutation controls the verification plan
    requires, restoring every mutation before continuing. Removing duplicate
    rejection from `check_definition` fails
    `a_duplicate_message_names_both_positions`, both
    `repeated_names_are_rejected` cases, `every_layer_class_reaches_a_verdict`,
    `production_composition_agrees_with_the_oracle`, and
    `layered_registries_agree_with_the_oracle`; removing
    `!appears_before(names, processed - 1)` from `walk_prefix` fails
    verification at `walk_prefix_is_accept_prefix` with 8 verified and 1 error.
    Removing the built-in lookup fails `built_in_names_are_rejected`,
    `a_reserved_repeat_reports_the_reserved_rule`, and the same three oracle
    tests; removing `!reserved(names[processed - 1])` fails verification at
    `walk_prefix_is_accept_prefix` and `non_vacuity_witnesses` with 7 verified
    and 2 errors. The restored model reports 9 verified and 0 errors.
  - [x] 2026-11-14: Clear two rounds of Clippy findings the repair introduced,
    each caught by the `lint` gate rather than by the focused test runs.
    `clippy::missing_const_for_fn` and `clippy::option_if_let_else` rejected
    `OccurrenceReport::new` and its `decorate` `match`, which became
    `const fn new` and `self.source.map_or_else(...)`. The repository also
    denies `clippy::integer_division` and
    `clippy::integer_division_remainder_used`, which rejected the byte-budget
    fixtures' `bytes / 3`, `bytes % 3`, and `MAX_ARGUMENT_BYTES / 2`. Both
    helpers now avoid arithmetic on the budget: `multibyte_text` appends whole
    three-byte characters while they still fit and pads with one-byte
    characters, and the shared-budget case uses a one-byte first element so
    the second element alone states the remainder. `make lint-clippy` then
    exits 0.
  - [x] 2026-11-14: Repair one stale assertion the `test` gate caught after the
    composition messages were reworded. The BDD step
    `loading_fails_at_duplicate_name` in
    `tests/bdd/steps/configured_shells.rs` still demanded the phrase
    `already names`, which the new message replaced with `already declares`,
    so `configured_shells_layered_names_must_be_unique` failed with the
    correct diagnostic in hand. The step now asserts `already declares`; all
    four `configured_shells` scenarios pass.
  - [x] 2026-11-14: Pass the whole eleven-gate set against the repaired tree.
    The run tagged `final16` executed every gate sequentially and finished
    `failed=0`: `check-fmt` 4s, `lint` 83s, `typecheck` 9s, `doc-coverage`
    36s, `test` 92s, `test-workflow-contracts` 636s,
    `test-package-verify` 155s, `test-documentation-contracts` 21s, `verus`
    2s, `markdownlint` 39s, `nixie` 1s. `make verus` reports "9 verified, 0
    errors"; doc-comment coverage is 98.26% against the 80% threshold. The
    gates read the tree that commit `2cdbb176a` records. The Rust, YAML,
    Python, and proof content those gates passed is byte-identical to that
    commit. The later commits in this file add Progress prose, which is
    Markdown the run did not read, and comments in a committed proptest seed
    file, which proptest ignores; the Markdown gates were re-run over that
    prose rather than assumed to cover it.
  - [x] 2026-11-14: Drive the production merge from the generated layers,
    because both hosted assessments asked for it. The properties above flatten
    the layers themselves, so a merge that behaved differently at a layer
    boundary would have passed them. `production_merge_agrees_with_the_oracle`
    in `src/cli/config/shells/tests/oracle.rs` now builds one configuration
    layer per generated layer and calls `CliConfig::merge_from_layers`, the
    entry point configuration loading uses. It requires the verdict to match
    the oracle, an accepted registry to survive with every definition in merge
    order and its program and arguments intact, and a regrouping of the same
    names at drawn boundaries not to change the verdict. The negative control
    is a duplicate-rejection mutation: making `check_definition` accept an
    occupied name fails this property with "merge accepted a rejected
    registry: [["fish", "fish"]]", which is the one-layer duplicate class;
    the byte-identical restored file returns it to green.
  - [x] 2026-11-14: Clear one more Clippy finding the `lint` gate caught in the
    new property's helper. `expected_definitions` rebound `name` inside
    `.map(|name| ...)`, and the repository denies `clippy::shadow_reuse`, so the
    owned binding is now `owned`. The gate aborted at `lint` (42 s) with exit 2;
    `make lint-clippy` and the focused oracle run (7 of 7) pass afterwards.
  - [x] 2026-11-14: Pass the whole eleven-gate set against the merge-property
    tree. The run tagged `final18` executed every gate sequentially and
    finished `failed=0` in 859 s: `check-fmt` 3s, `lint` 107s, `typecheck`
    28s, `doc-coverage` 15s, `test` 109s, `test-workflow-contracts` 534s,
    `test-package-verify` 13s, `test-documentation-contracts` 4s, `verus` 2s,
    `markdownlint` 43s, `nixie` 1s. `make verus` reports "9 verified, 0
    errors"; doc-comment coverage is 98.26% against the 80% threshold. The
    gates read the working tree that commit `92d9fb110` records, byte for byte,
    including this plan file. The content written after that run is recorded in
    the bullets below, which name the run that covers it.
  - [x] 2026-11-14: Close the two gaps the second assessment round named, in
    both assessment threads. `production_merge_agrees_with_the_oracle` now
    compares the merged ordered `(name, executable, args)` definitions for the
    regrouped layers as well as for the generated ones, so a regrouped merge
    that accepted the registry and then reordered or rewrote it no longer
    passes on its verdict alone, and
    `every_layer_class_reaches_a_verdict` merges each fixed witness through
    `CliConfig::merge_from_layers` as well as validating it directly, so the
    empty-layer and cross-layer-duplicate classes reach the production merge on
    every run. Both comparisons live in one `merge_violation` query so the
    property and the witnesses cannot drift apart. Two controls: removing
    duplicate rejection now fails the witness test as well as the property with
    "merge accepted a rejected registry: [["fish", "fish"]]", and reversing the
    order the helper pushes generated layers fails the property with "merge
    changed the registry: [["fish"], ["dash"]]", naming the definitions it got
    in the reverse of the order the layers declare, which proves the payload
    comparison is live. Both
    mutations were reverted from byte-identical copies (sha256 verified). The
    control's small seed is committed with an annotation saying which control
    produced it.
  - [x] 2026-11-15: Repair the two Windows-only failures the hosted
    `Windows / build-test-windows` job reported for `65f2ccca8`, which no Linux
    gate can observe. `a_rejection_names_the_declaring_file` compared the
    diagnostic with the path the harness built, but discovery records the
    canonical spelling, which on Windows is the long `runneradmin` form of a
    directory the harness created as `RUNNER~1`, and the preview is bounded at
    64 characters, so the temporary path was quoted as a truncated escaped
    prefix. The test now reads the quoted preview out of the diagnostic and
    requires it to be a faithful escaped prefix of the `dunce::canonicalize`d
    path that still reaches the configuration directory holding the file.
    `the_declaring_path_is_bounded_and_escaped` could not create its
    control-character directory at all, because Windows rejects a control
    character in a file name with os error 123; escaping is now stated by four
    `preview` unit tests beside the renderer, which run on every platform, and
    the integration case keeps only the overlong directory that every supported
    filesystem can hold.
  - [x] 2026-11-15: Pass the whole eleven-gate set against this tree. The run
    tagged `final21` executed every gate sequentially and finished `failed=0`
    in 853 s: `check-fmt` 4s, `lint` 47s, `typecheck` 9s, `doc-coverage` 18s,
    `test` 67s, `test-workflow-contracts` 618s, `test-package-verify` 36s,
    `test-documentation-contracts` 1s, `verus` 2s, `markdownlint` 51s,
    `nixie` 0s. `make verus` reports "9 verified, 0 errors" and doc-comment
    coverage is 98.26%. An earlier run tagged `final20` aborted at `lint`
    (15 s) because the new preview reader sliced a `&str` by index, which this
    repository denies as `clippy::string_slice`; `split_once` now supplies both
    halves, and the `preview` fixture's fixed slice became `strip_suffix`.
  - [x] 2026-11-15: Repair the two snapshot tests the next hosted Windows run
    reported, which the earlier failure had hidden. nextest cancels a run at
    its first failing test, so `json_diagnostic_format_is_stable` and
    `text_diagnostic_format_is_stable` had never executed on either host. Both
    pinned the declaring-file preview verbatim, and on Windows that preview is
    the escaped, canonical long form of a temporary path cut at the
    64-character bound, so the pinned text could not match. `normalised` now
    replaces the whole preview with `[DECLARING_FILE]` in both the text and the
    JSON escaping, which still pins the shape of the clause, and the contents
    are pinned by `a_rejection_names_the_declaring_file`. Both snapshots were
    updated to the marker.
  - [x] 2026-11-15: Pass the whole eleven-gate set against the snapshot repair.
    The run tagged `final23` finished `failed=0` in 835 s: `check-fmt` 5s,
    `lint` 63s, `typecheck` 6s, `doc-coverage` 55s, `test` 68s,
    `test-workflow-contracts` 594s, `test-package-verify` 6s,
    `test-documentation-contracts` 0s, `verus` 1s, `markdownlint` 33s,
    `nixie` 1s. The run tagged `final22` before it aborted at `markdownlint`
    (19 s) because the spelling gate rejects `normalising` in favour of the
    en-GB-oxendict `normalizing`; that word was the only finding, and the
    re-run above covers the corrected prose.
  - [x] 2026-11-15: Clear the hosted CodeScene finding for `b531c57c8`. The
    advisory rule "String Heavy Function Arguments" rejected
    `tests/cli_tests/shells_startup/support.rs` at 47.62% of its function
    arguments being strings against a 39% threshold, which failed the quality
    gate. The new redaction helper carried three `&str` parameters, so a
    `DeclaringClause` value now owns one format's opening and closing literals
    and redacts through a single borrowed argument, and `normalised` takes the
    child's `&Output` like the sibling helpers instead of an extracted string.
    `cs delta` now reports the module at Code Health 10.00 with the rule no
    longer above its threshold, and the startup suite still passes 21 of 21.
  - [x] 2026-11-15: Pass the whole eleven-gate set against the CodeScene repair
    at `080159e2e`. The run tagged `final24` finished `failed=0` in 933 s:
    `check-fmt` 3s, `lint` 81s, `typecheck` 14s, `doc-coverage` 83s, `test`
    89s, `test-workflow-contracts` 586s, `test-package-verify` 41s,
    `test-documentation-contracts` 1s, `verus` 2s, `markdownlint` 33s,
    `nixie` 0s. `make verus` reports "9 verified, 0 errors" and doc-comment
    coverage is 98.26%.
  - [x] 2026-11-15: Answer the hosted User-Facing Documentation pre-merge
    warning, which passed the users' guide but found no beta5 signpost for the
    feature. The v0.1.0 migration guide gains an Optional row in "Impact at a
    glance", a beta5 section that links to the users' guide section and states
    that existing manifests and configurations need no change, and a
    migration-checklist item; the CHANGELOG records the feature under
    "Unreleased / Added". `make check-fmt`, `make lint`, `make typecheck`,
    `make markdownlint`, `make nixie`, `make test-documentation-contracts`,
    and `make test` all pass against the repair.
  - [x] 2026-11-15: Repair the three findings from the first completed hosted
    review, which had been rate limited for the previous heads. The review
    asked for the proof claim in the developers' guide and repository layout to
    name the Verus model and label the oracle as bounded test evidence, for
    this plan's RFC 0011 link to use the real `rfc-0011-…` filename, and for
    the declaring-file assertion to require the directory only when it falls
    inside the preview's quoting budget. All three were verified against the
    current tree and repaired.
  - [x] 2026-11-15: Repair the two pre-merge errors and the
    developer-documentation warning raised against `69355e214`. The existence
    query for an unresolvable project file now goes through the
    `PathNormalizer` seam, which gained `exists` returning `io::Result<bool>`,
    and a failed query is classified as present so operator authority is
    withheld rather than granted on an absent-looking path. Shell authority
    became its own named step, `shells_authority_errors`, whose errors the
    collecting loop joins into `ResolvedFileLayers.errors`.
    `shell_definitions.rs` now yields typed `OccurrenceDiagnostic` values and
    renders them in one place, the shell-key deferral rule moved beside the
    checks that mint those keys, and `ShellExecutable`'s `Deserialize`
    documents its compile-time host dependency. New tests cover every shape
    branch at unit and pipeline level, and removing the unknown-field check
    was observed to fail exactly its two pipeline cases. The
    formal-verification document, developers' guide, and repository layout now
    record the third kernel, the `discovery/layers/` pass, and the executable
    spelling rules.

## Surprises & discoveries

The baseline is `37fc364a3caf6845dc4550666dea779d8b9328e2`; roadmap 11.2.1 is
checked complete and the selection vocabulary is present. Its older ExecPlan
header still describes unfinished PR delivery; use current source and roadmap
as prerequisite evidence rather than editing that unrelated history here.

`src/cli/discovery/project_policy.rs` already preserves project occurrences, but
`scope_selected_primary_layer` labels an arbitrary non-project explicit chain
as `Operator`. That label cannot implement this feature's authority rule.
CodeGraph was unavailable in the planning session; targeted source inspection
and shared context-pack references supplied the repository evidence instead.

The repository already declares `googletest`, `pretty_assertions`, `rstest`,
`insta`, and property-test tooling. Reuse those dependencies. The Verus runner
already supports a selected proof file through `VERUS_FLAGS`.

The plan and its external-interface evidence cite OrthoConfig 0.9.0, but
`Cargo.toml` pins 0.10.0. Inspecting the 0.10.0 registry source confirms that
`ConfigDiscovery::candidates()` returns bare paths only: the per-candidate
`Candidate.scopes` field is `pub(super)`, so no public scope label exists. Root
authority must therefore be re-derived by normalized path comparison against
the project file, as planned, and cannot simply read a candidate's scope.
Nothing in 0.10.0 blocks the design; the walk, `load_config_file_as_chain`, and
the public `LayerDiscoveryOutcome` fields all exist as the plan assumes.

Red evidence needed no source change. The prebuilt `target/debug/netsuke`
accepts a project `.netsuke.toml` carrying `[[shells]]` and proceeds to
`netsuke::runner::manifest_not_found`, while `jobs = "notanumber"` in the same
file fails configuration merging. The fixture therefore reaches discovery, and
the only missing piece is enforcement.

`compose_layers()` cannot carry the provenance C1 needs: it returns a flattened
layer list with no record of which candidate root supplied it. The restricted
walk reads the same public `candidates()` order, loads each chain with
`load_config_file_as_chain()`, and reproduces the old error handling exactly,
because all automatic candidates are optional (`required_bound` is zero unless
`add_required_path` is called, which Netsuke never does). Dropping the builder's
`.env_var(CONFIG_ENV_VAR)` is behaviour-preserving: a set `NETSUKE_CONFIG`
routes to the explicit branch before automatic discovery runs, so the automatic
path never consumed the selector. Only OrthoConfig's internal discovery
telemetry is lost, and Netsuke does not consume it. The builder still reports
the environment source because candidate generation reads `HOME` and the XDG
variables from it.

A project file that exists but cannot be path-normalized cannot be shown to be
distinct from a candidate, so `ProjectKey` records `unresolved` and withholds
operator authority for that scan. A missing project file cannot appear as an
occurrence, so it keeps operator authority. Preserving exactly one
`normalized_path_key` call keeps
`project_layer_scan_normalizes_only_the_project_key` and the replay
environment-read assertions meaningful.

`test_support::fs::create_dir` is not recursive, so a fixture that creates an
injected XDG tree must use `create_dir_all` for any intermediate directory it
has not already made. The differential test that writes an ordinary operator
configuration hit exactly this and failed with a bare
`No such file or directory` before the fix, which is why the success path is
exercised rather than assumed.

The first `lint-clippy` run stopped on the library's five findings, so Clippy
never compiled the test targets and six further findings in
`src/cli/discovery/layers/tests/` and `tests/cli_tests/` stayed hidden until
the gate was re-run after that repair. A green Clippy result therefore only
covers the targets Clippy reached, and any batch that aborted must be repeated
in full rather than resumed from the first passing phase.

The branch never edits `typos.toml`. The earlier spelling gate regenerated it
in place; the rebase onto the current `origin/main` (`5f0a4d123`) delivered
exactly that regenerated content from upstream, so the working-tree difference
disappeared and no separate refresh pull request is needed.

The M1 gate batch turned red at `make test-workflow-contracts`, not at any Rust
gate. Two repository contracts read the source tree as data: the Kani
proof-scope contract requires every file the `#[kani::proof]` harnesses reach
to appear in `tools/kani/proof-scope.toml`, and the module-layout contract
refuses sibling modules that share a first prefix. Adding `project_key.rs` and
the `authority_tests.rs`/`authority_properties.rs` pair therefore failed both,
even though `make test` was green and the code ran correctly. The repair records
`project_key.rs` in the proof scope and groups the two authority suites into
`src/cli/discovery/layers/tests/authority/`, the layout the
`src/manifest/tests/*/` modules already use. Neither failure is visible from
Cargo, so a green `make test` is not evidence that the source-tree contracts
hold.

The build-script module slice is registered in two places, and only one of them
is a directive `build.rs` emits. `tests/build_module_slice_ui_tests.rs` also
pins the slice as a literal list, so adding a module to `emit_rerun_directives`
alone leaves `make test` red on
`build_script_rerun_directives_match_the_compiled_module_slice` while every
other gate stays green. Search for an existing sibling path, such as
`shells/preview.rs`, to find every registry the new module has to join.

The build-script slice analyses visibility as a binary crate does, and a
`pub use` re-export inside a private module is unreachable there even though
the library crate consumes it. Declaring the slice's `cli` and
`shell_selection` modules `pub`, exactly as `src/lib.rs` does, keeps rustc's
unused-item analysis about the slice's real reachability. The consequence is
that the vocabulary's `#[expect(dead_code)]` became an unfulfilled expectation,
which `-D warnings` turns into a hard error, so the slice now carries no lint
suppression at all.

`prop::sample::select` takes `impl Into<Cow<'static, [T]>>`, which a
`const NAME_POOL: [&str; 7]` does not satisfy. The oracle declares the pool as
`&[&str]` instead. The failure surfaced only when `make typecheck` compiled the
build script's `cfg(test)` modules, because the slice compiles
`src/cli/config/shells/tests/` too.

An import that only a `#[cfg(unix)]` test reaches is invisible to every Linux
gate and fatal to the Windows job, which compiles with `-D warnings`. The
`shells` authority suite imported the merge probe at module scope and used it
only inside unix-gated tests, so the branch passed the local lint, typecheck,
and test gates while hosted CI stopped at
`error: unused import: merge_in_child`. Gate the import together with the
helper that reaches it. A Windows cross-check cannot settle this on this host:
`cargo check --target x86_64-pc-windows-msvc` stops in `ring`'s build script
with `GNU compiler is not supported for this target`, because `ureq` reaches
`ring` through `netsuke-build`, a dev-dependency of `test_support`, and the
target needs an MSVC C toolchain that is not installed. Cfg-gated imports
therefore have to be audited by inspection locally and confirmed by hosted CI.

A documented example that the suite executes is loaded on every host, so every
value it spells has to be host-neutral. The configured-shell example named
`executable = "/bin/dash"`, which a Unix-like host accepts as absolute and
Windows rejects as rooted without a drive, so the identical text passed Linux
and failed the Windows lane at
`cli_examples::configured_shell_example_is_accepted_from_an_operator_root` with
`Validation failed for 'shells[0].executable': a Windows shell executable must
name a drive or a UNC share`.
This is a second instance of the same blind spot as the unix-gated import
above: a defect that only the Windows job can observe. The repair is the
portable spelling, not a host gate on the test, and the example's spelling is
now asserted against both host families so a Linux gate catches the class. Note
also that the job stopped at its first failure, leaving 1429 of 4723 tests
unrun, so a green rerun is the only evidence for the rest.

A generated property can agree with its oracle on every sample and still be
nearly vacuous for one of the rules it is meant to cover. The first version of
the layered generator drew each name uniformly from the seven-name pool, so a
registry of up to 128 names almost always contained a reserved spelling. Both
the walk and the oracle then rejected the sample for the reserved rule, and the
duplicate rule was never the deciding difference. The duplicate mutation
control passed that property for exactly that reason while failing the
single-sequence one, whose five-name sequences sometimes avoid the reserved
names. Weighting unreserved names three to one against reserved spellings made
duplicate-only registries common, and the same mutation then failed the layered
property too. Non-vacuity has to be demonstrated against the mutation rather
than argued from a generator's bounds.

`OrthoError::Validation` carries only a key and a message, so the declaring
file cannot be reported as structured data. The source therefore appears inside
the message text, rendered through the same bounded and escaped preview as a
rejected value, and the JSON diagnostic's `source` field stays `null`. That is
the honest shape the dependency allows; a consumer that needs the path
structurally would have to parse it back out.

Verifying that a model mutation fails is only half of the control. Removing the
duplicate clause from `walk_prefix` leaves the reserved clause in place, so the
run still discharges eight obligations and reports 8 verified with 1 error. The
verified and error counts, not merely a non-zero exit, are what distinguish
"the mutation was caught by the intended obligation" from "the run happened to
fail".

Two more Windows-only failures came from tests that asserted about a path
rather than about behaviour. One compared the diagnostic with the path the
harness had built, but discovery records the canonical spelling, and on Windows
that is the long `runneradmin` form of a directory the harness created as
`RUNNER~1`; the bound then cut the temporary path at 64 characters, so the full
path was never quoted on that host. The other wrote a directory name containing
a control character to exercise escaping, which Windows refuses outright with
os error 123. Both were host assumptions rather than confirmed behaviour. The
assertion now reads the quoted preview back out of the diagnostic and checks it
is a faithful escaped prefix of the canonical path, and escaping is proven
beside the renderer where no filesystem is involved. A test that builds its own
expectation has to build it the way production does, canonical spelling and
bound included, or it is testing the harness instead.

The next hosted Windows run then failed two more tests the first had hidden:
nextest cancels a run at its first failing test, so the two snapshot tests had
never executed on either host. They pinned the declaring-file preview verbatim,
which on Windows is the escaped and truncated canonical path, so the pinned
text matched only the machine that produced it. The snapshot now pins the shape
of the clause and leaves the contents to the dedicated assertion. A snapshot of
host-supplied text is a record of one machine, and a suite that cancels at the
first failure can hide a whole class of such tests behind one earlier red one.

## Decision log

- 2026-10-20: Keep the plan approval-gated and the roadmap entry unchecked.
- 2026-10-20: Introduce shell-specific authority at discovery roots rather than
  reinterpret `FileScope::Operator` and alter existing policy semantics.
- 2026-10-20: Fail unauthorized definitions with a configuration-key error.
  Silent quarantine would hide an operator's mistaken assumption of trust.
- 2026-10-20: Use Verus selectively for the unbounded collision-validation
  kernel, with an independent specification and executable oracle tests. Do not
  formalize diagnostics, deserialization plumbing, or filesystem trust. The
  Wyvern reports favoured tests alone for provenance; this plan agrees and
  limits deductive proof to the sequence contract rather than that adapter.
- 2026-10-20: Pandalump/Wafflecat identified first-file discovery and lost
  candidate provenance; preserve discovery precedence, use explicit inheritance
  in layered examples, and deny ambiguous project/operator roots.
- 2026-10-20: Telefono/Dinolump clarified host path syntax, overlap cases, and
  focused gate commands. Buzzy Bee/Doggylump added normalized environment
  ingress, bounded escaped diagnostics, late JSON preference evidence, and
  set-based production validation. These review findings are incorporated.
- 2026-11-01: Record the OrthoConfig version as a deviation, not a defect. The
  plan cites 0.9.0 while `Cargo.toml` pins 0.10.0; the newer API still exposes
  `candidates()`, `load_config_file_as_chain`, and the public
  `LayerDiscoveryOutcome` fields the design relies on. No milestone or contract
  changes. Root authority remains a Netsuke-owned normalized comparison because
  `Candidate.scopes` is not publicly reachable.
- 2026-11-01: Replace Netsuke's `compose_layers()` call with a restricted
  `ConfigDiscovery::candidates()` walk. The dependency's composition discards
  candidate provenance, and C1 cannot be reconstructed from the flattened list.
  The walk keeps the dependency's candidate generation, order, first-success
  traversal, and optional-error handling; differential tests compare both
  outcomes for missing, malformed, inherited, and successful configurations.
- 2026-11-01: Enforce C1 at the single `retain_layers_and_resolve_json` choke
  point, on key presence before typed merging, so empty and malformed
  declarations are rejected rather than silently ignored. During M1 every
  declaration is rejected, including a trusted one, because no schema exists to
  validate it yet.
- 2026-11-01: Treat an unresolvable existing project file as ambiguous and fail
  closed, rather than assuming operator ownership of every occurrence. A
  missing project file stays trusted because it cannot occur as a layer.
- 2026-11-01: Treat the regenerated `typos.toml` as upstream content rather
  than a branch change. The rebase onto `5f0a4d123` supplied the exact file the
  pinned builder produces, so the branch carries no spelling-config diff and
  needs no stacked base pull request.
- 2026-11-01: Split `ProjectKey` into distinct identification and authority
  concerns. `matches` and `matches_path` stay literal so the project-scope
  second pass and deduplication keep their existing outcome even when
  normalization fails, while the new `is_ambiguous()` feeds only the two
  authority call sites. Conflating the two would have silently changed
  deduplication on a normalization failure, which
  `normalization_failure_does_not_fail_discovery` pins.
- 2026-11-01: Group the discovery merge inputs into `RootTrust` and
  `ProjectScope`, and move `ProjectKey` into
  `src/cli/discovery/layers/project_key.rs`. The repository's four-argument
  limit and its 400-line file boundary both bind here; grouping related
  evidence and splitting by responsibility are the documented remedies, and
  neither weakens the authority rule the parameters carry.
- 2026-11-01: Discharge the C1 property obligation against the classifier
  directly, not only through the written cases. The generator covers every
  (root trust, occurrence location, shells declared) combination, a separate
  property asserts rejection happens exactly when a declaration is present, and
  a fixed witness proves the generator reaches an operator-authorized
  occurrence so the properties cannot pass vacuously.
- 2026-11-01: Make the rejection property authority-relative once the schema
  exists. M2 admits a well-formed declaration from an operator root, so the
  property now expects rejection exactly when an untrusted occurrence declares
  `shells`. The superseded M1 entry above described the schema-less stage.
- 2026-11-01: Declare the build slice's `cli` and `shell_selection` modules
  `pub` rather than suppress the resulting reachability warnings. The slice is
  a binary crate, so a re-export inside a private module is unreachable and
  reports as an unused import; mirroring `src/lib.rs`'s public tree removes the
  warning and the need for any `#[expect]` in the slice.
- 2026-11-05: Fix the two valid M1–M2 review findings inside the M3 candidate
  rather than as separate commits. Any repair after a green batch owes a fresh
  full batch, and M3 needs one regardless, so folding them in avoids a third
  gate run whose only subject would be two small edits.
- 2026-11-05: Observe the first M3 scenario in-process rather than through the
  binary. No public interface exposes the resolved `shells` registry, and the
  C4 invocation prefix has no observable effect until command execution exists,
  so the scenario merges the operator chain through the same public OrthoConfig
  entry points the application uses and asserts on the typed definitions. The
  remaining scenarios assert the observable rejection through the real binary.
- 2026-11-05: Use a sentinel output file, not `generate --output FILE`, to
  prove that a rejection precedes manifest compilation. A successful run writes
  the sentinel and a rejected one does not, so the assertion does not depend on
  a Ninja render succeeding for an unrelated reason.
- 2026-11-05: Pin the text and JSON diagnostic shapes with `insta` snapshots
  and nothing else. C5 names those two formats as the observable surface, so
  they are the only outputs whose exact spelling is contractual; every other
  test asserts typed values or collections, which leaves the snapshots narrow.
  The snapshot normalizes the temporary workspace, the tracing timestamp, and
  the crate version, because none of those is part of the diagnostic.
- 2026-11-05: Keep the `guide-configured-shell-definition` marker on the
  `[[shells]]` fence in `docs/users-guide.md`, and leave the Rust `append` and
  `PostMergeHook` example added to `docs/ortho-config-users-guide.md` without a
  `tested-example` marker. The executable-documentation loader reads
  `README.md`, `docs/users-guide.md`, and
  `docs/stdlib-yaml-and-jinja-guide.md`; a marker in a fourth file would
  register an identifier no loader can find.
- 2026-11-05: Omit `shells` from release help. The field has no command-line
  surface, so a generated help entry would describe a flag that does not exist.
- 2026-11-05: Record the new modules in `tools/kani/proof-scope.toml`. The
  workflow contract requires the declared paths to cover the harness closure
  exactly, so an unlisted module fails the contract rather than silently
  skipping a proof input.
- 2026-11-09: Split the host path-spelling predicates into
  `src/cli/config/shells/spelling.rs` rather than shortening the type that
  calls them. CodeScene measures string-heavy functions per file, and the
  predicates are a coherent unit: pure text inspection shared by both host
  families, holding no state in common with the `ShellExecutable` value they
  validate.
- 2026-11-14: Require a complete UNC share, not two separators. Windows treats
  `\\` as unresolved, and `\\server` names no share, so accepting either would
  claim an absolute program reference that no path resolution can honour. They
  now report the same `RootedWithoutDrive` fault as `/opt/tool`, which is the
  closest true statement about a rooted spelling with no drive.
- 2026-11-14: Report the declaring file inside the diagnostic message rather
  than as structured source metadata. `OrthoError::Validation` has no source
  field, and inventing one would mean changing the shared validation helper and
  the JSON rendering for a single key. The preview goes through the existing
  bounded, escaped renderer, so a path with a control character or an extreme
  length is as safe as a rejected value.
- 2026-11-14: Bundle the declaring path with the collected errors in an
  `OccurrenceReport` instead of adding a fifth parameter to every check. The
  repository's four-argument limit binds here, and a type that owns both pieces
  also makes it impossible to push a diagnostic that silently omits the source.
- 2026-11-14: Name `shells[index].name` in composition errors. The merged
  registry has no single declaring file, so the source clause cannot apply, but
  C5 requires the field path, and `shells[1]` alone leaves the operator to
  guess which field collided.
- 2026-11-14: Weight the layered generator towards unreserved names three to
  one. A uniform draw masks the duplicate rule behind the reserved rule in
  large registries, which the duplicate mutation control demonstrated. The
  weights are a coverage choice, not a production limit.
- 2026-11-14: Commit `proptest-regressions/cli/config/shells/tests/oracle.txt`.
  It was written while the mutation controls were live, so it pins no real
  defect, but its four seeds are the minimal witnesses (`["dash", "dash"]`,
  `[["fish", "fish"]]`, `["sh"]`, `[["sh"]]`) and replaying them first makes
  both rules' smallest counterexamples deterministic on every run. The
  repository already tracks nine such files.
- 2026-11-15: Compare a declaring-path diagnostic with the canonical spelling of
  the path it quotes, and prove escaping beside the renderer. Asserting the raw
  harness path passed only because a Linux temporary path is short and already
  canonical, and no filesystem-hosted test can carry a control character on
  Windows. The integration case therefore keeps the overlong directory, which
  every supported filesystem can create, and reads the quoted preview back out
  of the diagnostic instead of assuming the whole path is shown.
- 2026-11-15: Redact the declaring-file preview in the snapshot tests instead of
  normalizing its path. The path's canonical spelling, separators, and length
  differ by host, and the renderer bounds it, so no substitution of the
  workspace root can match. The clause's shape is what a format snapshot should
  pin, and its contents are pinned by the dedicated assertion.
- 2026-11-14: Check the correspondence against the production merge, not only
  against `validate_composition`. Every earlier property flattened the layers
  in the test before calling the validator, so the layer boundary was the
  test's own assumption rather than something the samples exercised: a merge
  that dropped, reordered, replaced, or deduplicated definitions across files
  would have agreed with the oracle on every sample. The property now builds
  real configuration layers, merges them with `CliConfig::merge_from_layers`,
  and compares both the verdict and, on acceptance, the ordered definitions and
  their payloads against the oracle. The defaults layer is composed in because
  `merge_from_layers` needs the unrelated keys, which also proves the merge
  still completes with an empty `shells` registry. This trades the module's
  original "validator only" scope for adapter evidence; it does not weaken the
  independence of the oracle, which still states the rule pairwise and shares
  no code with either the validator or the merge.
- 2026-11-14: Check the merged definitions, not only the verdict, for the
  regrouped layers, and merge the fixed witnesses too. A verdict-only
  comparison answers "would this be accepted?" and says nothing about what was
  accepted, so a merge that accepted a regrouped registry and then reordered or
  rewrote it would pass. The same argument applies to the class witnesses: they
  named the outcomes on the validator alone, which left the production merge
  dependent on the generator drawing those classes. Both comparisons are one
  `merge_violation` query returning the first disagreement, which keeps the
  property and the witnesses stating the same rule and gives each failure a
  diagnostic that names the layers.
- 2026-11-15: Give each snapshotted format a `DeclaringClause` value rather than
  passing its two delimiters to a free helper. The slice of the startup support
  module that holds test plumbing had reached CodeScene's string-argument
  threshold, and the threshold measures the module rather than the call: two
  more `&str` parameters in one new helper was enough to fail it. The type also
  states why the pair exists, which a bare pair of literals did not. Taking
  `&Output` in `normalised` follows the same reasoning: the helper now derives
  the text it normalizes from the child's own output, as `stderr_of` and
  `json_failure` already do, instead of requiring every caller to extract it.
- 2026-11-15: Treat the missing beta5 signpost as a documentation gap rather
  than an out-of-scope suggestion. The migration guide is where a beta-series
  operator learns that a key now fails closed, and the authority rule is the
  one thing about this feature that can break an existing checkout; leaving it
  only in the users' guide assumes the operator reads that guide before the
  migration notes. The Optional row, the beta5 section, and the checklist item
  state the same rule in the three places the guide already uses for a
  voluntary change, and the CHANGELOG entry keeps the feature discoverable.
- 2026-11-15: State the Verus proof's scope as the model walk and the oracle as
  bounded evidence. `verus/configured_shells/main.rs` proves `walk` against the
  pairwise specification; it does not verify the production traversal or the
  production `HashMap` seen-set, and an oracle over generated sequences is test
  evidence, not a proof of correspondence. The earlier wording claimed the
  production function and said the oracle "closed" the gap, which overstated
  what the artefacts establish and would have invited a reviewer to trust a
  guarantee the proof does not carry. Naming the model and the evidence class
  keeps the claim checkable against the files.
- 2026-11-15: Bound the declaring-file assertion by the preview budget. The
  diagnostic quotes at most 64 characters of the canonical path, so a deep
  temporary root — on Windows especially — can push the configuration directory
  outside the preview while the diagnostic is still correct. The test now
  requires the directory only when the canonical path reaches it within that
  budget, and the budget is a named constant beside the assertion that mirrors
  `src/cli/config/shells/preview.rs`.
- 2026-11-15: Extend `PathNormalizer` with `exists` instead of adding a second
  filesystem seam. `ProjectKey::classify` needs to know whether an unresolvable
  project file is present, and the trait is already documented as the one place
  discovery reaches the filesystem; a second seam would have been a parallel
  boundary with its own test doubles. `Path::exists` reports `false` for every
  I/O error, so a failed query is classified as *present* here: concluding
  absence from an unreadable directory entry is exactly the trust upgrade the
  ambiguity flag exists to prevent. Two tests pin the classification, one with
  a project file on disk and one without, so neither error-as-absent nor
  error-as-present can be reintroduced.
- 2026-11-15: Return errors from `shells_authority_errors` rather than pushing
  into `ResolvedFileLayers` from inside the check. The collecting loop already
  owns accumulation for every other quarantine step, and authority answers a
  different question from precedence, so the step now reads as one query whose
  result the caller joins. The error list is the only output, which also lets a
  future caller report authority faults without allocating a resolver.
- 2026-11-15: Separate occurrence validation from message decoration with a
  typed `OccurrenceDiagnostic`. The previous report struct built strings while
  it validated, which made the rule text and the declaring-file clause
  inseparable in tests and left the clause one edit away from being forgotten
  by a new check. A check now states a key and a rule, and one renderer adds
  the bounded declaring-file preview, so unit tests assert exact keys and rules
  without a temporary file.
- 2026-11-15: Keep the shell-key deferral rule in
  `src/cli/discovery/shell_definitions.rs` rather than
  `src/cli/config/shells/`. The build script compiles the schema module as part
  of its own slice for manual-page and completion generation, where no caller
  of that rule exists; placing it there triggers `-D dead-code` in that slice,
  whose unused-item analysis is deliberately kept meaningful. The discovery
  module mints every `shells` key, so owning the grammar there keeps one owner
  and lets `cli::diag` import it instead of restating it.
- 2026-11-15: Document `ShellExecutable`'s compile-time host dependency instead
  of injecting a host family into deserialization. Serde's `Deserialize` has no
  place to carry a context, and the whole configuration tree would need a
  threaded host to change that; the impl now states the dependency, the module
  doc names the exception, and the developers' guide records the operator-
  visible consequence, which is that a spelling for the other host family is
  rejected while the file is read. `ShellExecutable::parse` remains the
  injectable entry point.
- 2026-11-15: Leave `VERUS_FLAGS`'s default on the canonical-value kernel.
  `make verus` is a maintainer target with a tested command shape, and the
  approved scope selected the new kernel through `VERUS_FLAGS` rather than
  moving the default. The documentation now states which kernel the default
  proves and how to select the others, and every gate run for this plan passes
  both `--proof-file` values so the new kernel is proved as well.

## Outcomes & retrospective

Roadmap 11.2.2 is implemented. An operator can declare additional
structured-command shells in trusted system and user configuration, and no
untrusted source can: project files, `--config`, and `NETSUKE_CONFIG` are
rejected at discovery time, before generic merging, so a rejected request never
becomes a silent success. Accepted definitions append in layer order, and
reserved built-in names and duplicates are rejected in `PostMergeHook` with
both colliding positions named. Diagnostics stay bounded: a rejection names the
configuration key but never echoes an argument vector, an offending value in
full, or an ambient environment value.

The evidence is a Verus proof over the composition rule's model walk, an
independent quadratic Rust oracle that exercises the production function over
generated sequences, four `rstest-bdd` scenarios, and an `assert_cmd` startup
suite that drives the real binary. The full gate set passed at the rebased
candidate `205873fc5`, and the milestone CLI review over `1139f31df..f03faf7b0`
covered all 24 changed files and left no valid concern unresolved.

Two lessons are worth carrying forward. First, a gate batch binds to a
candidate, not to a branch: the rebase onto `ce15e026` invalidated every
earlier verdict, so the whole set was rerun rather than reasoned about. Second,
a plan edit is not a docs-only delta, because
`tests/execplan_status_contract_tests.rs` reads every plan from disk; the
plan's own prose therefore has to clear the same gates as the code it describes.

The two hosted assessments then found four gaps the local evidence had missed:
an incomplete UNC share was accepted as absolute, a rejection named the field
but not the file that declared it, the composition error named indices rather
than `shells[i].name`, and the layered obligation had no generated test at all.
All four are repaired here. The third lesson is the sharpest: the layered
property, once written, agreed with its oracle so consistently that it passed a
deliberately broken build, because a uniform name pool put a reserved spelling
into almost every large registry. Only the mutation control exposed that, and
the property was rewritten before it was trusted.

Follow-up review found one more of the same kind. The layered properties
flattened the layers themselves before calling the validator, so every sample
agreed with the oracle about a sequence the test had already assembled, and the
production merge was never exercised. `production_merge_agrees_with_the_oracle`
now drives `CliConfig::merge_from_layers` with one configuration layer per
generated layer, which is what makes the evidence adapter evidence rather than
model evidence. The general lesson is that a property is only as strong as the
production entry point it reaches: a test that pre-processes its input and then
calls an inner function proves a statement about the inner function alone.

A further round found the same weakness one level down. The regrouped layers
reached the production merge, but only their verdict was compared, so a merge
that accepted a regrouped registry and then reordered or rewrote it was
unobserved; and the fixed class witnesses still stopped at the validator. Both
now travel through `CliConfig::merge_from_layers` and are compared as ordered
definitions. A passing verdict is not passing evidence: what a merge accepts is
as much a part of its contract as whether it accepts.

The hosted Windows job then found a different kind of gap: two startup tests
asserted something true only of the host that ran them. One built its expected
path from the harness's temporary directory, which Windows spells in short form
while discovery records the canonical long form, and whose length the 64-column
bound then truncated; the other needed a directory name containing a control
character, which Windows will not create. Both are repaired by asserting the
property rather than the spelling — the preview must be a faithful escaped
prefix of the canonical path — and by proving escaping beside the renderer.
Assertions about paths and other host-supplied text have to be stated in the
form production uses, or a green Linux run says nothing about another host.

Executable resolution is 11.2.3 and structured-command integration is 12.3.1;
neither is claimed here. The wider issue #593 stays open.

## Conformance basis

The repository-relative documents below are read at baseline commit
`37fc364a3caf6845dc4550666dea779d8b9328e2`; recheck changes before execution.
The terms of reference identify themselves as draft v0.2, last revised
2026-09-25. The design has no independent version identifier, so the baseline
commit identifies its revision.

- [Terms of reference](../terms-of-reference.md), goals and safe-failure
  constraints, and [Netsuke design](../netsuke-design.md), configuration and
  execution boundaries: M1–M3 preserve finite manifest authority.
- [Roadmap 11.2.2](../roadmap.md#112-resolve-trusted-shells-without-expanding-manifest-authority):
  M1 establishes source authority, M2 establishes validation and merge rules,
  and M3 supplies user evidence and documentation. 11.2.3 and 11.3 remain out
  of scope; the wider issue is #593, which this task does not close.
- [ADR-019](../adrs/adr-0019-select-allow-listed-structured-command-shells.md),
  including all addenda, and
  [RFC 0011](../rfcs/rfc-0011-allow-listed-structured-command-shells.md)
  sections 5 and 11: contracts C1–C5 below.
- [RFC 0001](../rfcs/rfc-0001-structured-command-blocks-and-argv-templates.md),
  shell mode and execution boundaries: preserve existing command behaviour
  throughout.
- [ADR-008](../adrs/adr-0008-environment-seam-taxonomy.md): injected
  environment and child-process isolation for M1 and M3.

Read [documentation contents](../contents.md) and
[repository layout](../repository-layout.md) first. Use the `codegraph-mcp`,
`hexagonal-architecture`, and `rust-router` skills; use `leta` when semantic
navigation is available, `rust-verification` and `verus` for M2, and
`firecrawl-mcp` for unresolved external-interface questions. The `execplans`
skill governs approval, progress, and deviation recording.

Testing references:

- [rstest fixtures](../rust-testing-with-rstest-fixtures.md).
- [Doctest guidance](../rust-doctest-dry-guide.md).
- [Dependency injection](../reliable-testing-in-rust-via-dependency-injection.md).
- [rstest-bdd](../rstest-bdd-users-guide.md).
- [Formal verification](../formal-verification-methods-in-netsuke.md).

Use the actual repository toolchain and test macros when an embedded upstream
guide describes an older baseline.

## Context and intended boundaries

`src/shell_selection/` owns `ShellName`, `BuiltInShell`, `HostFamily`, and
`ShellSelection`. The `name` module is also consumed by a restricted build-time
source slice. Preserve that import boundary when exposing the existing name
validator to configuration; inspect `build.rs` and the module-slice tests.

`src/cli/config/mod.rs` owns `CliConfig`, its defaults, append annotations, and
`PostMergeHook`. Add a feature-owned `src/cli/config/shells/` module for the
configuration value, local validation, and collision validation. Keep its API
limited to configuration loading and the future shell-registry composition
root. No general interpreter service or execution-domain port is needed.

`src/cli/discovery/layers/mod.rs` discovers root chains;
`src/cli/discovery/project_policy.rs` preserves authority before merging and
retains errors while resolving output preferences. `src/cli/merge/mod.rs` and
`src/cli/merge/input.rs` assemble file, environment, and CLI layers. Audit each
route, including explicit selection and cached discovery replay. Existing
policy helpers are reuse precedents, not interchangeable trust grants.

Before adding any helper, search these modules for equivalent functionality.
Record ownership, permitted callers, and composition rules in
`docs/netsuke-design.md` and `docs/repository-layout.md`. No adapter calls a
future execution adapter; configuration yields validated data only.

## Behavioural contracts and abstract specification

C1, source authority: each file occurrence has a root category, an inheritance
chain, a value, and a diagnostic location. Only roots positively identified as
system or user operator sources grant definition authority to their inherited
occurrences. Project and arbitrary explicit roots do not. Unknown provenance
fails closed. `shells` is configuration-file-only; ambient `NETSUKE_SHELLS` and
generated CLI arguments must not inject it. Reject environment ingress by the
normalized top-level key, covering case/whitespace normalization and `__`
nested components, not merely the exact raw spelling. Existing selector flags
merely choose files. No independent trust mechanism is introduced.

C2, local validity: deserialize a definition with exactly three required
fields. Reuse `ShellName`'s ASCII grammar `[a-z][a-z0-9_-]{0,62}` and its
rejection of `true`, `false`, `yes`, `no`, `y`, `n`, `on`, and `off`.
`executable` is a non-empty UTF-8 string without NUL, either a bare executable
name or an absolute path. Reject relative paths containing `/` or `\`, and
reject `.` and `..` as executable names. On Windows, reject drive-relative and
rooted-without-drive paths; test complete drive and UNC paths. Apply the host's
absolute-path semantics without checking existence. A Windows drive path
containing backslashes is rejected on Unix rather than reinterpreted as a bare
name. `C:tool` is a bare name on Unix, but a rejected drive-relative path on
Windows. `/opt/tool` is absolute on Unix and is rejected as
rooted-without-drive on Windows; `C:\tools\shell.exe` and
`\\server\share\shell.exe` are absolute on Windows. A relative `bin/tool` or
`bin\tool` is rejected on either host. Preserve accepted spelling without
expansion, normalization, trimming, or executable probing.

`args` has 1–16 elements, no NUL in any element, and a sum of UTF-8 byte
lengths at most 4096. Empty individual arguments are permitted. The sum
excludes serialization syntax and process terminators. Use checked accumulation
or remaining-budget subtraction. Reject missing fields, wrong field types, and
unknown fields. Do not infer a source switch or reject an arbitrary
structurally valid prefix merely because it differs from a known shell.

C3, composition: let D be the concatenation, in merge order, of the definition
sequences from authorized occurrences. Empty layers contribute the empty
sequence. This means loaded occurrences, not all candidate files: preserve
first-available-file discovery and use `extends` for layered operator input.
Successful post-merge validation returns D unchanged exactly when no name is
built in and no two positions have equal names. Built-ins are `sh`, `bash`,
`pwsh`, and `powershell`. Identical repeated definitions still collide; there
is no replacement, keyed merge, or deduplication. An empty higher layer does
not clear a lower layer. Invalid input never exposes a partial registry.

C4, operator policy: for executable E and fixed prefix A, the future invocation
is E with A followed by exactly one rendered source argument S. The operator
must choose A such that this invokes E's source evaluator. Structural checking
cannot establish that semantic precondition for arbitrary E. Document a
violation as a trusted operator policy error; do not claim this phase detects
it, or execute E to inspect it. Named selection does not sanitize shell source.

C5, diagnostics and timing: malformed trusted fields, unauthorized sources,
built-in collisions, and duplicate names fail configuration loading before
manifest compilation. Errors identify `shells[index].name`,
`shells[index].executable`, or `shells[index].args` as appropriate, plus a safe
name/source preview when available. Bound and escape control characters in raw
invalid names, executable strings, and source paths before rendering; reuse
`BoundedConfigPath` conventions rather than relying on successful name parsing.
Unauthorized-key errors identify `shells` and the source. Do not print the full
argument vector or environment values. Preserve text/JSON output preference
even when a later layer selects JSON.

## Verification plan and proof sketch

Define the specification before choosing the collision algorithm. Treat a name
as a finite byte sequence, reserved names as a fixed set B, and D as an
arbitrary finite sequence of names. Define Accept(D) independently by the
pairwise condition: every name is outside B, and for all indices i < j in D,
D[i] differs from D[j]. The expected successful output is D itself. There are
no uniqueness or non-reservation preconditions; those are conclusions on
success, not premises.

The proposed executable validator walks left to right, rejects membership in B
or in the previously seen names, and otherwise records the current name. Use
set membership in production, with expected O(n) or O(n log n) work and O(n)
auxiliary storage; the quadratic oracle is test-only. Do not introduce an
aggregate count cap or a benchmark project for this startup validation. The
important representation relationship is that the seen set equals the names in
the already accepted prefix. Proposed semantic lemmas are: extending a valid
prefix succeeds exactly for a non-reserved fresh name; failure has a
reserved-name or earlier-equal-name witness; the complete walk accepts exactly
Accept(D); and success preserves sequence order and payloads. These statements
are hypotheses about proof structure, not a prescribed list of solver helpers.

M2 adds `verus/configured_shells/main.rs` proving the walk against the pairwise
specification over arbitrary sequence lengths. Use byte-sequence names or a
proved faithful equality abstraction, not an unexplained finite integer hash.
Connect the model to the production function by documented branch/state
correspondence and an independent quadratic Rust oracle comparing production
results on actual `ShellName` values. State explicitly that this is a model
proof plus tested correspondence, unless the implementation establishes a
stronger verified executable bridge. Do not describe it as proof of all Rust
configuration loading. Use no `assume`, `admit`, external proof body, or axiom
introduced to obtain the result. Defer triggers, collection algebra, and
auxiliary induction details until the real verification conditions exist.

The non-trivial external assumptions are correct byte/string equality, standard
collection membership, the pinned OrthoConfig append contract, Serde's typed
input contract, and the discovery adapter's source labels. Verus relies on its
standard library and solver soundness. Filesystem ownership and arbitrary
executable semantics are outside the proof. Test the repository's use of each
interface rather than attempting to verify third-party internals.

C1 uses `rstest` source matrices and discovery integration tests, with
`proptest` sequences of root-labelled occurrences, including shared parents and
reversed root order. Include overlapping HOME/XDG/project roots, candidate
deduplication, normalized aliases, and path-normalization failure; ambiguity
must never authorize a definition. Verify unauthorized presence always fails,
no file path alone elevates authority, and unrelated configuration retains its
existing precedence. The restricted walk is additionally compared against
OrthoConfig's `compose_layers()` as a differential oracle for missing,
malformed, inherited, and successful configurations; both traversals must
observe the same layers in the same order or fail with the same text. The proof
does not certify these I/O decisions.

C2 uses parameterized boundary tests: name lengths 0/1/63/64, invalid ASCII and
Unicode, all reserved Boolean words, empty/NUL executables, bare/absolute and
relative path families, 0/1/16/17 arguments, NUL arguments, empty elements, and
4095/4096/4097 UTF-8 bytes including multibyte characters. Test every supported
configuration format through the real loader where practical. Missing and extra
fields must have explicit cases. Conventional tests suffice here. The byte
cases are built from three-byte characters with a one-byte remainder, so a
limit applied to `chars().count()` rather than to `len()` is rejected at 4096,
and a separate case proves the budget is shared across elements rather than
applied to each one. The Windows path cases cover the complete UNC share in
both separator spellings and reject `\\`, `//`, `\\server`, and `\\server\` as
rooted without a drive.

C3 combines direct post-merge tests, real OrthoConfig layer tests, the
unbounded Verus model, and generated oracle comparisons. Generate zero to eight
layers and zero to sixteen definitions per layer, deliberately including empty,
unique, reserved, identical-duplicate, and cross-layer-duplicate classes; these
are test bounds, not production limits. Acceptance under regrouping of
unchanged ordered layers must match; order changes may change the first error
but never validity. Record class coverage and avoid filtering away collisions.

Class coverage is recorded twice. `every_layer_class_reaches_a_verdict` states
one named witness per class — empty layers only, unique names across layers, a
reserved spelling, an identical duplicate, a duplicate split across layers, and
an empty layer that cannot clear an earlier duplicate — and asserts both the
production verdict and the oracle verdict for it. `generated_layers` reaches
those classes without filtering, but drawing each name uniformly would let the
reserved rule mask the duplicate rule in large registries, so the element
strategy weights unreserved names three to one. The test
`the_generator_pool_reaches_every_outcome` asserts that both halves of the pool
are drawn from names the pool really contains, so the weights cannot drift away
from the pool.

The correspondence is checked against the production merge, not only against
the validator. `production_merge_agrees_with_the_oracle` turns every generated
layer into a configuration layer and calls `CliConfig::merge_from_layers`, so
the sampled layers travel through the schema's append merge and post-merge
hook. A merge that reordered, replaced, deduplicated, or dropped definitions at
a layer boundary would fail the payload comparison. The same two checks apply
to a regrouping of the same names at drawn boundaries: the verdict must be
unchanged and, when the registry is accepted, the merged definitions must still
equal the ordered `(name, executable, args)` triples the names require, because
a regrouped merge that accepted the registry and then reordered or rewrote it
would otherwise pass on its verdict alone. The class witnesses above stay in
force and now also travel through the merge, because they state the outcomes
the generated samples must be able to reach and a sample is not guaranteed to
draw the empty-layer or cross-layer-duplicate class on every run.

Non-vacuity evidence includes an empty successful registry, two distinct
configured names succeeding, a built-in rejection, and a duplicate introduced
only in the last layer. Temporarily remove duplicate rejection and separately
reserved-name rejection: the focused tests must fail and the corresponding
model mutation must fail verification. Restore mutations before any commit. No
incorrect program or intentionally false theorem remains in the tree. The
recorded controls are: removing duplicate rejection from `check_definition`
fails `a_duplicate_message_names_both_positions`, both
`repeated_names_are_rejected` cases, `every_layer_class_reaches_a_verdict`,
`production_composition_agrees_with_the_oracle`,
`layered_registries_agree_with_the_oracle`, and
`production_merge_agrees_with_the_oracle`, the last with "merge accepted a
rejected registry: [["fish", "fish"]]"; removing `!appears_before` from
`walk_prefix` fails verification with 8 verified and 1 error; removing the
built-in lookup fails `built_in_names_are_rejected`,
`a_reserved_repeat_reports_the_reserved_rule`, and the same three oracle tests;
removing `!reserved` fails verification with 7 verified and 2 errors. The
payload comparison has its own control, because no production mutation makes a
merge reorder an accepted registry: reversing the order in which the test
helper pushes the generated layers makes
`production_merge_agrees_with_the_oracle` fail with "merge changed the
registry: [[\"fish\"], [\"dash\"]]", naming the merged definitions in the
reverse of the declared order. That sample is the smallest two-layer registry
whose merge order can differ. Every mutation was reverted from a byte-identical
copy, and the restored model reports 9 verified and 0 errors. The seed that
control produced is committed, so the smallest order-sensitive registry is
replayed on every run.

C4 is an explicit operator assumption, demonstrated by documented `dash`
`args = ["-c"]` examples; no proof or test claims arbitrary evaluator-prefix
correctness. C5 uses text/JSON diagnostic snapshots with stable temporary-path
redaction and child-process tests demonstrating failure before a manifest or
Ninja side effect. `insta` is justified for these two observable formats. Use
`googletest` matchers for typed outcomes and `pretty_assertions` for exact
collections; preserve meaningful failure messages.

## Implementation milestones

### M1: Preserve shell-definition source authority

After approval, first add failing tests for a project `shells` key, an
arbitrary explicit file, a project parent loaded through `extends`, and an
environment injection attempt. Confirm failure reflects missing enforcement,
not a broken fixture. Introduce the narrow root authority metadata and enforce
C1 before flattening into merge layers. Preserve all existing fetch,
environment-grant, budget, discovery-cache, and JSON preference behaviour.

With pinned OrthoConfig 0.9.0, use public `ConfigDiscovery::candidates()` and
`load_config_file_as_chain()` for a narrow Netsuke-owned automatic candidate
walk: the automatic path must have neither an explicit selector nor injected
explicit paths. Classify each candidate root before loading its inherited
chain. Match project roots using the existing normalized comparison; only the
remaining system/XDG/home candidates from this restricted builder receive
operator authority. If project and operator roots overlap, classify the
ambiguous occurrence as untrusted. Preserve the first usable primary chain,
optional-error handling, candidate order, and Netsuke's existing separate
project pass. Differential tests must compare old/new discovery outcomes for
missing, malformed, inherited, and successful ordinary configurations. Do not
reimplement candidate-path generation. `clear_project_roots()` alone is not an
operator-only discovery mechanism: the dependency restores a default current
directory when the builder has no project roots. Keep explicit selection on its
separate untrusted path. Cache the tagged result rather than reconstruct trust
from flattened paths during replay.

A coherent M1 plateau rejects unsupported unauthorized declarations while no
configured-shell schema is exposed yet. Add no permissive intermediate path.
Use existing injected discovery seams. Record the exact root classifier and
handling of inherited occurrences in the design document. Regression tests for
ordinary config discovery and existing trust policies must still pass. Run the
milestone gates below and commit the complete authority change. Remaining gap:
trusted definition schema and collision validation are M2.

### M2: Validate and compose trusted definitions

Add failing field-deserialization and merged-collision tests, then implement
C2/C3 in `src/cli/config/shells/`. Add `shells` with empty default and append
merge to `CliConfig`; suppress CLI and environment ingress through the pinned
derive's supported controls or the existing explicit layer allow-list. Check
the generated help/completion/config metadata and restricted build slice. Do
not add a field to runtime `Cli` merely to carry unused execution state.

Validate each trusted occurrence with the same field deserializer before
composition. Invoke the merged validator from `PostMergeHook`; retain useful
field/index errors through OrthoConfig conversion. Make it impossible for the
normal startup path to reach manifest loading after any definition error.
Perform the proof-sketch review before settling the validator's state shape;
then implement and verify the proof and oracle correspondence described above.

Acceptance requires all boundary and real append-merge tests passing, the
mutation controls detecting the intended faults, no definitions accepted from
unauthorized ingress, and the proof command reporting verified obligations
without failures. The coherent plateau loads validated data without resolving
or executing shells. Commit only after all milestone gates and the proof pass.
Remaining gap: full user-workflow evidence and documentation closure are M3.

### M3: Demonstrate and document the configuration contract

Add `tests/features/configured_shells.feature` and bind it using the existing
`rstest-bdd` test layout. Include these acceptance scenarios; extend with the
invalid-field and collision matrix without duplicating unit-test mechanics:

```gherkin
Feature: Trusted configured shell definitions
  Scenario: Operator layers append different definitions
    Given a system configuration defining shell "dash"
    And the selected user configuration extends that system configuration
    And that user configuration defines shell "custom"
    When Netsuke loads its configuration
    Then both definitions retain their fixed arguments in layer order

  Scenario: A project cannot define an interpreter
    Given a project configuration containing a shell definition
    When Netsuke loads its configuration
    Then loading fails at configuration key "shells"
    And no manifest is compiled

  Scenario: Explicit selection does not grant authority
    Given an otherwise untrusted file defining shell "custom"
    When that file is selected with --config
    Then loading fails at configuration key "shells"

  Scenario: Layered names must be unique
    Given two trusted layers defining shell "dash"
    When Netsuke loads its configuration
    Then loading fails at the duplicate shell name
```

Add subprocess startup tests using isolated config roots and child-only
`Command::env`/`env_clear`, including `NETSUKE_CONFIG`, `--config`, inherited
project definitions, malformed data, duplicate trusted definitions, and both
text and JSON errors. Include an earlier malformed or unauthorized `shells`
value followed by a later `json = true`: assert parseable JSON, the relevant
key, nonzero exit, and an untouched manifest/Ninja sentinel. Cover normalized
environment aliases and malformed nested environment values as denied ingress.
Assert oversized/control-character inputs yield bounded escaped diagnostics
without argument contents or ambient values. The declaring path's escaping is
proven by the `preview` unit tests beside the renderer, because Windows cannot
create a directory whose name contains a control character; the startup case
covers the overlong directory every supported filesystem can hold, and requires
the quoted preview to be a faithful escaped prefix of the canonical path that
reaches the configuration directory. Use a valid harmless manifest and
controlled Ninja stub for success; use an observable sentinel to show invalid
config prevents later work. Do not depend on an installed `dash`, `bash`, or
PowerShell.

Add a commented `[[shells]]` example to `docs/sample-netsuke.toml`, including
the complete-prefix operator obligation, limits, trust scope, and absent
executable acceptance. Update `docs/ortho-config-users-guide.md` with append
order, empty-array behaviour, collision rejection, and post-merge hooks. Update
`docs/users-guide.md` to explain authorized sources and actionable failures
while making clear that named-command execution is a later task. Update
`docs/netsuke-design.md`, `docs/repository-layout.md`, and
`docs/developers-guide.md` with ownership, source-label handling, proof scope,
proof/runtime correspondence, and test commands. Record substantive deviations
in an ADR and reference it from the design only after approval.

Any new fenced example in the users' guide needs a stable `tested-example`
marker, `EXPECTED_EXAMPLE_IDS` registration, and semantic validation. Apply the
same existing documentation-contract discipline to edited registered examples.
Run the final gates, inspect the diff for unrelated formatter output, and
commit. Only after feature completion and evidence reconciliation mark roadmap
11.2.2 `[x]` (done). Never mark the whole phase or issue #593 complete. Update
this plan's progress and outcomes; reserve COMPLETE for delivered and reviewed
work.

## Commands and acceptance evidence

Run from the repository root. Before each milestone, verify the branch, clean
state, and current dependency baseline. Use the `scrutineer` agent as the sole
full-gate runner for this worktree. Run the focused newly added test before and
after production changes, recording the actual nextest test identifier in
Progress. The current Makefile has no test-name filter variable. For focused
Linux runs, use its underlying command with the same flags (drop only the mold
flag on other hosts), replacing the filter with the recorded test name:

```bash
RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }-D warnings -Zthreads=8 -Clink-arg=-fuse-ld=mold" \
  cargo nextest run --workspace --all-targets --all-features --no-tests fail -E 'test(configured_shell)'
```

Preserve compiler wrappers and the shared cache. A red run must fail for the
intended missing rule.

After each major implementation milestone, run these commands sequentially,
with Bash `set -o pipefail` and a separate `/tmp` tee log per command:

```bash
make fmt
make check-fmt
make lint
make doc-coverage
make test
make markdownlint
make nixie
```

For M2 and subsequent changes affecting the collision contract, additionally
run the selected proof explicitly; do not rely on the runner's default file:

```bash
make verus VERUS_FLAGS='--proof-file verus/configured_shells/main.rs'
```

If the pinned verifier is absent, install using `make install-verus`,
preserving its pinned version and checksum. A skipped proof is not acceptance.
Review assumptions and correspondence evidence alongside the verifier's success
report.

For M3 documentation examples, additionally run:

```bash
make test-documentation-contracts
make test-workflow-contracts
```

If workflow contracts or coverage validators are changed incidentally, apply
AGENTS.md's corresponding extra gates; do not broaden scope merely to obtain a
green run. For the plan-only PR, run `make fmt`, `make check-fmt`,
`make markdownlint`, and `make nixie`, plus `git diff --check`; no Rust
behaviour or proof is changed by publishing this document. Record actual
outcomes, not assumed success. Full code gates apply to every implementation
commit.

At each plateau check C1–C5 against the assigned evidence, confirm no
unapproved surface or dependency was added, and refresh traceability and risks.
Commit only passing changes. Diagnose a failed gate from its existing log;
rerun after a fix, not while an earlier gate is queued. Never place build
targets in `/tmp`.

## Recovery and delivery

All steps are repeatable with isolated fixtures and unchanged shared Cargo
state. If a milestone fails, keep the preceding passing commit as rollback
point and correct the uncommitted milestone. Do not reset other work, change
trust semantics to bypass a test, or force-push divergent history.

For plan delivery, push the requested branch over SSH after verifying the
`leynos` identity, refresh
`origin/11-2-2-trusted-configured-shell-definitions`, and set it as this
branch's upstream. Create a draft PR titled
`Plan: Trusted configured shell definitions (11.2.2)`. Link this file with a
branch-based GitHub URL and end the description with `## References` containing
<https://lody.ai/leynos/sessions/4d87d518-d8e7-424d-a9e7-9031fd2c62d8>. Read
the published body back and verify draft status, branch, and commit parity.
Then request plan approval and stop before implementation.

## External-interface evidence

Firecrawl checked the versioned
[OrthoConfig 0.9 PostMergeHook documentation](https://docs.rs/ortho_config/0.9.0/ortho_config/trait.PostMergeHook.html)
on 2026-10-20: the hook runs after declarative merging and returns
`OrthoResult<()>`. It does not supply a substitute for root authority metadata.
The manifest now pins 0.10.0; the 0.10.0 registry source was inspected on
2026-11-01 and keeps that hook contract, while `candidates()` returns only
paths, so root authority stays a Netsuke-owned classification. The checked
[rust-prover-tools interface](https://github.com/leynos/rust-prover-tools)
accepts `verus run --proof-file`; the repository's pinned runner and Makefile
remain authoritative during execution. No external claim replaces an
integration test against the pinned dependency.

## Revision note

2026-11-01: Recorded maintainer approval, set `Status: APPROVED`, and rebased
the branch onto `origin/main` (`da06ffb11`) before implementation. The rebase
replayed the plan commit unchanged and introduced no source change.

2026-11-01: Rebasing again onto `5f0a4d123` (PR #958) recorded the OrthoConfig
0.10.0 deviation, the missing public candidate-scope label, and the resolution
of the regenerated `typos.toml` as upstream content.

2026-10-20: Initial approval-gated draft, based on two Wyvern reconnaissance
reports, current source contracts, and external-interface checks. Reconciled
all six expert perspectives: source-root ambiguity, unchanged discovery order,
host path cases, normalized environment ingress, failure diagnostics, and
proof/test correspondence. Plan-only formatting, Markdown/spelling, diagram,
and whitespace checks passed; feature implementation still awaits approval.
