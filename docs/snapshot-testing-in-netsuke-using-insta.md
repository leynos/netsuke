# Snapshot Testing IR and Ninja Outputs in Netsuke

Snapshot testing with the `insta` crate provides a powerful way to ensure
Netsuke’s intermediate representations and generated Ninja build files remain
correct over time. According to the Netsuke design, the Intermediate
Representation (IR) is a backend-agnostic build graph, and the Ninja file
generation is a separate stage built on that IR. Leverage this separation by
writing **separate snapshot tests** for IR and for Ninja output. This guide
covers setting up `insta`, organizing test modules and snapshot files, ensuring
deterministic outputs, running the tests, and integrating them into a GitHub
Actions CI workflow.

## Setting Up Insta for Snapshot Testing

First, add `insta` as a development dependency in your **Cargo.toml**:

```toml
[dev-dependencies]
insta = "1"
```

The `insta` crate provides macros like `assert_snapshot!` (for plain text or
`Debug` snapshots) and `assert_yaml_snapshot!`/`assert_json_snapshot!` (for
structured snapshots). Use these macros in tests, and install the companion CLI
tool `cargo-insta` for reviewing or updating snapshots (useful in CI and local
development).

**Project Structure:** Organize the tests in the `tests/` directory, using one
module for IR snapshots and another for Ninja snapshots. Each module has its
own snapshot output directory for clarity. A possible layout:

```text
netsuke/
├─ Cargo.toml
├─ src/
│   ├─ ast.rs            (Manifest AST structures)
│   ├─ ir.rs             (BuildGraph IR structures)
│   ├─ ninja_gen.rs      (Ninja file generation logic)
│   └─ ...               (other source files)
└─ tests/
    ├─ ir_snapshot_tests.rs
    ├─ ninja_snapshot_tests.rs
    └─ snapshots/
        ├─ ir/           (snapshot files for IR tests)
        └─ ninja/        (snapshot files for Ninja tests)
```

By default, `insta` creates a `tests/snapshots` directory and stores snapshot
data in files named after the test modules. This configuration separates IR and
Ninja snapshots into subfolders, keeping the expected outputs organized and
aligning with Netsuke’s design separation of IR from code generation.

## Writing Snapshot Tests for IR Outputs

A dedicated test module (e.g. **tests/ir_snapshot_tests.rs**) contains IR
snapshot tests. Each test feeds a Netsuke manifest (the input build
specification) into the compiler’s IR generation stage and captures the
resulting IR in a stable, human-readable form. According to the design, the IR
(BuildGraph) is intended to be independent of any particular backend, so it is
verified in isolation here.

**Example IR Snapshot Test:**

```rust
use insta::assert_snapshot;
use insta::Settings;
use netsuke::NetsukeManifest;      // assumed struct for parsed manifest
use netsuke::ir::BuildGraph;       // assumed IR data structure

#[test]
fn simple_manifest_ir_snapshot() {
    // Example Netsuke manifest in YAML (string literal for test)
    let manifest_yaml = r#"
        netsuke_version: "0.1"
        rules:
          - name: "compile"
            command: "gcc -c $in -o $out"
        targets:
          - file: "hello.o"
            deps: ["hello.c"]
            rule: "compile"
    "#;

    // 1. Parse manifest YAML into AST/manifest struct
    let manifest = NetsukeManifest::from_yaml_str(manifest_yaml)
        .expect("Manifest parsed");

    // 2. Generate the IR (BuildGraph) from the manifest
    let build_graph = BuildGraph::from_manifest(&manifest)
        .expect("IR generation succeeded");

    // 3. Convert IR to a deterministic string representation
    // For example, use Debug trait or implement a custom Display/serialization
    let ir_pretty = format!("{:#?}", build_graph);

    // 4. Assert snapshot, storing output in tests/snapshots/ir/
    Settings::new()
        .set_snapshot_path("tests/snapshots/ir")
        .bind(|| {
            assert_snapshot!("simple_manifest_ir", ir_pretty);
        });
}
```

This test involves:

- Construct a **deterministic** input (a small manifest with a known rule and
  target).

- Run the IR generation (`BuildGraph::from_manifest`). This function should
  produce the intermediate build graph.

- Format the IR consistently for comparison. Pretty-printed debug output
  (`{:#?}`) can be used, but for more complex structures implement `Display` or
  use `assert_yaml_snapshot!` to serialize the IR to YAML/JSON for clarity.

- Use `Settings::new().set_snapshot_path("tests/snapshots/ir")` to direct the
  snapshot file to the IR snapshot directory. Call `assert_snapshot!` with a
  snapshot name (`"simple_manifest_ir"`) and the IR output string. On first
  run, `insta` will record this output as the reference snapshot. **Determinism
  in IR Output:** To ensure consistent snapshots, the IR output must be
  **deterministic**. This means that given the same manifest input, the IR’s
  printed form should not vary between test runs or across machines. Pay
  attention to ordering and ephemeral data:

- **Ordering:** If `BuildGraph` contains collections (e.g. sets of targets
  or rules), iterate or sort them in a fixed order before printing. Using
  `BTreeMap` or sorting vectors of targets by name can help. This avoids
  nondeterministic ordering from hash maps.

- **Stable Identifiers:** If IR includes IDs or memory addresses, prefer stable
  identifiers. For example, when generating rule IDs, assign them in insertion
  order so they are consistent, or omit details that can change.

- **No timestamps or environment-specific data:** The IR should not include
  timestamps, random values, or absolute file system paths. If such data is
  unavoidable, use `insta` redactions or post-process the output to replace
  them with placeholders (e.g., `<CURRENT_DIR>`).

By making the IR snapshot output stable, the snapshot tests will reliably catch
regressions. If the IR generation logic changes intentionally (e.g., new fields
added), the snapshot will change predictably, prompting a review.

## Writing Snapshot Tests for Ninja File Output

Next, create **tests/ninja_snapshot_tests.rs** to verify Ninja build file
generation separately. This stage takes the IR (BuildGraph) and produces a
Ninja build script (usually the contents of a `build.ninja` file). Because
Netsuke’s design cleanly separates IR building from code generation, it is
possible to use the same manifest (or multiple manifest scenarios) to test the
Ninja output specifically.

**Example Ninja Snapshot Test:**

```rust
use insta::assert_snapshot;
use insta::Settings;
use netsuke::NetsukeManifest;
use netsuke::ir::BuildGraph;
use netsuke::ninja_gen;          // module for Ninja file generation

#[test]
fn simple_manifest_ninja_snapshot() {
    // (Re-use the same manifest YAML as before)
    let manifest_yaml = r#"
        netsuke_version: "0.1"
        rules:
          - name: "compile"
            command: "gcc -c $in -o $out"
        targets:
          - file: "hello.o"
            deps: ["hello.c"]
            rule: "compile"
    "#;
    let manifest = NetsukeManifest::from_yaml_str(manifest_yaml).expect("Manifest parsed");
    let build_graph = BuildGraph::from_manifest(&manifest).expect("IR generation succeeded");

    // Generate Ninja file content from the IR
    let ninja_file = ninja_gen::generate_ninja(&build_graph)
        .expect("Ninja file generation succeeded");

    // The output is a multi-line Ninja build script (as a String)
    // Ensure the output is deterministic
    // (e.g., consistent ordering of rules/targets)
    Settings::new()
        .set_snapshot_path("tests/snapshots/ninja")
        .bind(|| {
            assert_snapshot!("simple_manifest_ninja", ninja_file);
        });
}
```

Key points for Ninja snapshot tests:

- Use a known manifest input and first derive the IR. An IR can also be
  constructed directly for tests, but using the manifest→IR pipeline ensures
  realistic coverage.

- Call the Ninja generation function (e.g. `ninja_gen::generate_ninja`), which
  produces the Ninja file contents as a `String`. This function traverses the
  IR and outputs rules and build statements in Ninja syntax.

- As with IR, **determinism is crucial**. The Ninja output should list rules,
  targets, and dependencies in a consistent order. For example, if the IR does
  not preserve order, targets may need to be sorted by name or hashing and
  deduplication must avoid randomness. The design’s approach of consolidating
  rules by a hash of their properties should still produce the same ordering
  given the same input, as long as iteration over hashmaps is avoided or
  stabilized.

- Use `Settings::set_snapshot_path` to store these snapshots in a separate
  `tests/snapshots/ninja` directory. The snapshot name
  `"simple_manifest_ninja"` identifies this particular scenario.

With this setup, IR tests and Ninja tests have distinct snapshot files. For
example, after the first test run (see next section), expected snapshot files
include `tests/snapshots/ir/simple_manifest_ir.snap` and
`tests/snapshots/ ninja/simple_manifest_ninja.snap` (or combined snapshot files
per test module). These snapshot files contain the expected IR debug output and
Ninja file text respectively.

## Running and Updating Snapshot Tests

To execute the snapshot tests, run `cargo test`. All tests (including our new
snapshot tests) will run. On the first run (or whenever a snapshot differs from
expectations), test failures will indicate snapshot changes.

**Example:**

```bash
$ cargo test
running 2 tests
test simple_manifest_ir_snapshot ... FAILED
test simple_manifest_ninja_snapshot ... FAILED

---- simple_manifest_ir_snapshot stdout ----
snapshot assertion for `simple_manifest_ir` failed in "tests/ir_snapshot_tests.rs":
   Snapshot not found - stored new snapshot
---- Snapshot Differences ----
… (insta will indicate that a new snapshot was created) …

---- simple_manifest_ninja_snapshot stdout ----
snapshot assertion for `simple_manifest_ninja` failed in "tests/ninja_snapshot_tests.rs":
   Snapshot not found - stored new snapshot
…
```

On the first run, `insta` (with default `INSTA_UPDATE=auto`) writes new
snapshot files and marks the tests as failed for review. `.snap` files (or
`.snap.new` if not auto-approved) appear in the `tests/snapshots/`
subdirectories.

**Reviewing and Accepting Snapshots:** Use the `cargo-insta` CLI to review and
accept these new snapshots:

- Run `cargo insta review` to interactively inspect differences. This displays
  a diff between old and new snapshot contents for each test. Since this is the
  first run, it only shows the new content.

- Accept the new snapshots using the review interface. `cargo-insta` then moves
  the `.snap.new` files to replace the old snapshots or create the `.snap`
  files if they did not exist.

- As an alternative, when confident in the outputs, run `cargo insta accept
  --all` to accept all changes in one go.

Once accepted, re-run `cargo test` – it should pass because the recorded
snapshots now match the output. Commit the new/updated `.snap` files to version
control. **Always include the snapshot files** so that CI can validate against
them.

**Deterministic Failures:** If a snapshot test fails unexpectedly in the
future, it means the IR or Ninja output changed. This could reveal a regression
or a legitimate update:

- For an intended change (e.g., the IR structure or Ninja output format was
  updated as part of a feature), review and accept the new snapshots, then
  include the updated `.snap` files in the commit.

- If it is unintended, investigate the differences. Snapshot diffs make the
  change clear (e.g., a rule name, dependency order, etc.) and help pinpoint
  the issue.

## Integrating Snapshot Tests into GitHub Actions CI

Automating snapshot tests in CI ensures that changes to Netsuke do not
introduce regressions without notice. Use GitHub Actions to run `cargo test`
(which includes the snapshot tests) on every push or pull request. Here’s how
to set it up:

**1. CI Workflow Setup:** In the repository (e.g.,
`.github/workflows/ test.yml`), use a Rust toolchain action and run tests. For
example:

```yaml
name: Rust CI
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Install Rust toolchain
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          components: ["rustfmt", "clippy"]  # add components as needed
          override: true

      - name: Cache Cargo registry
        uses: actions/cache@v3
        with:
          path: ~/.cargo/registry
          key: cargo-registry-${{ hashFiles('**/Cargo.lock') }}

      - name: Cache Cargo build
        uses: actions/cache@v3
        with:
          path: target
          key: cargo-build-${{ hashFiles('**/Cargo.lock') }}-$(rustc --version)

      - name: Install cargo-insta (snapshot review tool)
        run: cargo install cargo-insta

      - name: Run tests (including snapshot tests)
        env:
          INSTA_UPDATE: no   # Ensure tests fail on any snapshot mismatch (no auto-update in CI)
        run: cargo test --all --all-features
```

**Notes:**

- Setting `INSTA_UPDATE: no` in CI disables automatic snapshot creation or
  updating. If a snapshot is missing or differs, the tests **fail**. The
  default `auto` mode already treats CI specially (it will not auto-accept in
  CI), but setting `no` is an explicit safeguard.

- Install `cargo-insta` mainly for completeness – running `cargo test` does not
  strictly require the CLI tool, but its presence enables `cargo insta`
  subcommands in CI if needed (for example, to print a summary or ensure no
  unused snapshots with `cargo insta test --unreferenced=reject`).

- The caches for Cargo help speed up CI. Ensure you include the snapshot files
  in the repository so that tests can find the expected outputs.

**2. Handling Snapshot Changes in CI:** In a typical workflow, CI will run
tests and either pass or fail:

- If all snapshots match, CI passes. No action needed.

- If a snapshot test does not pass (indicating changes in the IR or Ninja
  output), the CI job will not succeed. Developers should pull the changes
  locally, run `cargo insta review`, accept the new snapshot if it is intended,
  and commit the updated snapshot file. **Do not automatically accept snapshots
  in CI** – reviewing changes is essential to catch unintended alterations.

The CI process can be enhanced to make snapshot reviews easier:

- Use `actions/upload-artifact` to upload the `.snap.new` files or diff results
  when tests fail so they can be downloaded from the CI logs for inspection.

- Or run `cargo insta test --diff` in CI to print diffs to the log for quick
  viewing of what changed (the `INSTA_OUTPUT` env var can control diff vs
  summary output).

However, the simplest approach is to let `cargo test` report failures and use
those as a signal to update snapshots locally.

## Conclusion

Introducing snapshot tests for both the IR and the Ninja output adheres to
Netsuke’s design principles and increases confidence in each stage of the build
process. The IR tests verify that the manifest-to-IR transformation produces
the expected build graph structure independently of any output format. The
Ninja snapshot tests then verify that the IR-to-Ninja translation is correct.
Both sets of tests use deterministic outputs to ensure consistent, meaningful
snapshots.

With the `insta` crate, adding new test cases is straightforward – simply
create a manifest (or multiple variants) and assert that the IR or Ninja output
matches the snapshot. The snapshot files serve as living documentation of the
expected build graph and build script for given scenarios. Integrated into
GitHub Actions, this testing framework will catch regressions early: any change
in Netsuke’s IR logic or code generation will surface as a snapshot diff,
prompting careful review.

This structured snapshot testing approach enables confident evolution of the
Netsuke project while preserving the correctness of its core compilation
pipeline.

**Sources:**

- Netsuke Design/Roadmap – separation of IR and Ninja generation

- Insta crate documentation – usage of snapshot assertions and CI integration
  guidelines
