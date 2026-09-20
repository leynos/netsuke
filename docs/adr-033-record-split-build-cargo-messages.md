# Architectural decision record (ADR) 033: Record split-build Cargo messages

## Status

Accepted.

## Date

2026-09-20

## Context and problem statement

`harness_compiles_under_a_split_build_dir` protects the direct-`rustc` UI
harness against an artefact-discovery regression. Cargo may place dependency
artefacts under `CARGO_BUILD_BUILD_DIR` while leaving the uplifted
`test_support` artefact under `CARGO_TARGET_DIR`. The harness must therefore
collect every dependency directory named by Cargo's `compiler-artifact`
messages and select the `test_support` artefact from the target directory.

The former test established that contract by running a private Cargo build and
then compiling a fixture. The private target and build directories were
necessary to avoid an `E0460` race with the shared `test_support` fixture, but
they also forced roughly 350 crates to compile from scratch on every run. The
build cost was unrelated to the parser contract and made the test consume a
large part of its per-test timeout. Issue 732 therefore separates the parser
regression from the expensive workspace build while retaining a small
end-to-end split-build boundary test.

## Decision drivers

- Preserve coverage of Cargo's split-layout `compiler-artifact` message
  interpretation.
- Keep the test deterministic and independent of the host's Cargo cache,
  compiler wrapper, and build parallelism.
- Remove the private nested build that races the shared uplifted artefacts and
  consumes the test's timeout budget.
- Keep the fixture faithful to the two paths the harness must distinguish:
  dependency directories in the split build directory and the `test_support`
  artefact in the target directory.
- Keep integration coverage for the actual direct-`rustc` argument assembly in
  the shared harness tests, rather than making the parser fixture responsible
  for Cargo's build process.

## Requirements

### Functional requirements

- The regression test must parse recorded Cargo JSON through the same
  `dependency_dirs_in_message` and `library_path_in_message` functions used by
  the live harness.
- The recorded messages must include at least one dependency artefact under a
  split build directory and a `test_support` artefact under an uplifted target
  directory.
- The test must assert both path properties so that collapsing the two roots or
  selecting the wrong artefact fails the test.
- A separate boundary test must build a two-crate temporary workspace under
  private split roots, parse its real Cargo messages, and compile a fixture
  through the direct-`rustc` response-file path.

### Technical requirements

- The parser regression test must not invoke Cargo, create temporary target or
  build directories, or depend on the ambient workspace artefacts.
- The boundary test must keep its target and build roots private so it cannot
  race the ambient workspace's uplifted artefacts.
- The fixture must use the same `compiler-artifact` message shape that Cargo
  emits, including loadable `.rmeta` or `.rlib` filenames.
- The live `TestSupportRlib::build` and `compile` path must remain covered by
  the UI harness tests that use the shared `#[once]` fixture.

## Options considered

### Option A: Keep the private live Cargo build

Retain the existing test and its isolated target and build directories. This
keeps end-to-end coverage of Cargo plus artefact collection, but pays the full
workspace compilation cost on every run and preserves the `E0460`-avoidance
complexity. It does not address the performance problem that prompted issue 732.

### Option B: Build a minimal fixture crate under the split layout

Replace the workspace build with a small temporary crate that has one or more
dependencies. This reduces the compile cost while retaining a live Cargo
message stream, but it still couples the test to Cargo execution and temporary
filesystem state. It also tests a much smaller artefact set than the real
`test_support` build, including fewer proc-macro and dynamic-library cases.

### Option C: Record representative Cargo JSON and test the parser, with a

small end-to-end boundary fixture

Store a minimal JSON-lines fixture containing dependency and `test_support`
`compiler-artifact` messages, then feed each line through the production
collection functions. This removes the unrelated build cost while retaining the
exact message-reading contract and the split-path distinction. It does not
prove that a particular Cargo release emits the same messages at the full
workspace scale. A separate two-crate fixture supplies that live Cargo and
direct-`rustc` boundary coverage without rebuilding the workspace.

| Topic                            | A: live workspace | B: minimal crate | C: recorded JSON |
| -------------------------------- | ----------------- | ---------------- | ---------------- |
| Parser coverage                  | Yes               | Yes              | Yes              |
| Live Cargo coverage              | Yes               | Yes              | Small fixture    |
| Rebuilds the workspace           | Every run         | No               | No               |
| Reproduces the split path layout | Yes               | Yes              | Yes              |
| Covers the real dependency scale | Yes               | No               | Representative   |
| Depends on host state            | High              | Medium           | None             |

_Table 1: Trade-offs between live and recorded split-build coverage._

## Decision outcome

Adopt Option C. Add
[`tests/ui/split_build_dir_cargo_messages.jsonl`](../tests/ui/split_build_dir_cargo_messages.jsonl)
with representative Cargo `compiler-artifact` messages. Make
`harness_compiles_under_a_split_build_dir` parse those lines with the shared
artefact functions, assert that dependency directories include the recorded
split build directory, and assert that the selected `test_support` artefact
stays under the recorded uplifted target directory. Keep
`split_build_fixture_compiles_through_the_direct_rustc_harness` as a separate
small end-to-end test: it builds a two-crate workspace under private split
roots, parses real Cargo JSON, and compiles through the response-file-backed
direct-`rustc` boundary.

The parser test no longer invokes `TestSupportRlib::build` or `compile`,
creates temporary Cargo roots, or belongs to the `nested-cargo-builds` test
group. The small boundary test deliberately invokes Cargo in private roots,
while the shared live UI tests continue to build `test_support` and pass the
resulting artefacts through the response-file-backed direct-`rustc` path.

## Rationale

The defect being pinned is in interpreting Cargo's messages: dependency
artefacts may not share the directory containing the final uplifted rlib. A
recorded fixture expresses that distinction directly and makes the assertion
independent of cache warmth, compiler wrappers, filesystem layout, and Cargo
parallelism. The fixture also makes the intended paths visible in review.

Keeping the parser assertion separate from the live compile boundary gives each
test a clear responsibility. The recorded split-layout test proves that the
collection logic reads Cargo's reported paths correctly, while the small
fixture proves that real Cargo output, dependency search paths, and
response-file arguments reach `rustc` together. Requiring the parser test to
build the entire workspace would couple a small regression to an expensive and
race-prone setup.

## Consequences

- The split-build regression test is fast, deterministic, and no longer
  consumes an isolated Cargo build or a Windows-specific timeout allowance.
- The small boundary fixture retains live Cargo and direct-`rustc` integration
  coverage at the cost of compiling a temporary two-crate workspace.
- The recorded fixture must be maintained if Cargo's JSON contract or the
  repository's split-directory layout changes.
- The test does not detect a future Cargo change that stops emitting the
  represented message shape; the pinned toolchain and the live shared harness
  remain the coverage for that end-to-end concern.
- The parser test no longer exercises the full dependency count or the full
  Windows process-spawn pressure of the former workspace build. The boundary
  fixture still exercises response-file invocation with real Cargo paths, while
  the shared direct-`rustc` harness and response-file tests cover the broader
  integration surface.

## Known risks and limitations

- A fixture can become stale if Cargo changes field names, artefact naming, or
  the relationship between build and target directories. Updating the fixture
  must preserve both path assertions and should be based on current Cargo
  output.
- The fixture contains representative paths rather than a complete workspace
  transcript, so it does not guarantee that every future compiler-artifact
  variant is accepted. New variants belong in focused parser tests when they
  become relevant.
- The two-crate boundary fixture does not reproduce the former workspace's
  dependency count or command-line pressure. The shared direct-`rustc` harness
  and response-file unit tests retain coverage for the broader integration
  surface.

## Implementation references

- Split-layout regression:
  [`tests/locale_stub_ui_tests.rs`](../tests/locale_stub_ui_tests.rs)
- Recorded Cargo messages:
  [`tests/ui/split_build_dir_cargo_messages.jsonl`](../tests/ui/split_build_dir_cargo_messages.jsonl)
- Cargo message parsing and direct-`rustc` preparation:
  [`tests/support/test_support_rlib.rs`](../tests/support/test_support_rlib.rs)
- Parser implementation:
  [`tests/support/cargo_artifacts.rs`](../tests/support/cargo_artifacts.rs)
- Response-file integration:
  [`tests/support/rustc_response_file.rs`](../tests/support/rustc_response_file.rs)

## Related decisions

- [ADR-028: Defer replacing the split-build-dir harness test][adr-028]
  records the superseded live-build decision and its measurements.
- [ADR-025: Local pull-request coverage ratcheting][adr-025] records the
  coverage-lane ownership that motivated the former nested-build serialisation.

[adr-025]: adr-025-main-owned-coverage-publication.md
[adr-028]: adr-028-defer-split-build-dir-harness-trim.md
