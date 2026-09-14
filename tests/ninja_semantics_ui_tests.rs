//! Compile-time tests for the generated-Ninja text-domain boundaries.
//!
//! `test_support::ninja_semantics` wraps the whole generated document and the
//! text a recipe must carry in separate borrowed newtypes, so a call site
//! cannot silently pass one where the other belongs. The two swaps below are
//! the ones a test is most likely to make by accident: searching an encoded
//! payload as if it were plaintext Ninja text, and decoding a whole document as
//! if it were a Base64 payload. Both must be rejected by the compiler.
//!
//! The `test_support` rlib is built by Cargo and the fixtures are compiled
//! directly with the workspace `rustc` against it, so no scratch project and no
//! toolchain-sensitive `.stderr` snapshot is needed.

#[path = "support/test_support_rlib.rs"]
mod test_support_rlib;

use rstest::{fixture, rstest};
use std::{io, process::Output};

/// One `test_support` build shared by the cases below.
///
/// Built once: the cases run in parallel, so independent builds would contend
/// on Cargo's target-directory lock and repeat completed work.
#[fixture]
#[once]
fn test_support_build() -> test_support_rlib::TestSupportRlib {
    #[expect(
        clippy::expect_used,
        reason = "a once fixture cannot return Result; a build failure must abort the suite here"
    )]
    let rlib = test_support_rlib::TestSupportRlib::build().expect("test_support should build");
    rlib
}

/// Verify each generated-Ninja text domain rejects the other's wrapper.
#[rstest]
#[case::needle_as_document(
    "tests/ui/ninja_semantics_needle_as_document_compile_fail.rs",
    "RecipeNeedle"
)]
#[case::document_as_needle(
    "tests/ui/ninja_semantics_document_as_needle_compile_fail.rs",
    "GeneratedNinja"
)]
fn text_domains_cannot_be_swapped(
    test_support_build: &test_support_rlib::TestSupportRlib,
    #[case] fixture: &str,
    #[case] rejected_domain: &str,
) -> io::Result<()> {
    let output = test_support_build.compile(fixture)?;
    assert_type_mismatch(&output, fixture, rejected_domain)
}

/// The correct document-and-needle pair compiles under the same harness.
///
/// This is the control for the rejections above: it fails if the `--extern` or
/// `-L dependency` wiring breaks, which would otherwise make those cases pass
/// for the wrong reason.
#[rstest]
fn document_and_needle_compile_together(
    test_support_build: &test_support_rlib::TestSupportRlib,
) -> io::Result<()> {
    let fixture = "tests/ui/ninja_semantics_boundaries_compile_pass.rs";
    let output = test_support_build.compile(fixture)?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "the control fixture should compile; the harness wiring is broken:\n{}",
            test_support_rlib::stderr(&output),
        )));
    }
    Ok(())
}

/// Assert `output` rejected `fixture` with a mismatch naming `rejected_domain`.
fn assert_type_mismatch(output: &Output, fixture: &str, rejected_domain: &str) -> io::Result<()> {
    if output.status.success() {
        return Err(io::Error::other(format!(
            "{fixture} should not compile: {rejected_domain} must not stand in for the other generated-Ninja text domain",
        )));
    }
    let stderr = test_support_rlib::stderr(output);
    if !stderr.contains("E0308") || !stderr.contains(rejected_domain) {
        return Err(io::Error::other(format!(
            "the rejection must be a text-domain type mismatch naming {rejected_domain}, not a harness fault:\n{stderr}",
        )));
    }
    Ok(())
}
