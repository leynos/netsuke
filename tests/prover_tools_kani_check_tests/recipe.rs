//! Assert that the `kani-check` recipe cannot swallow a failed write.
//!
//! The isolation case in `sandbox` runs the query with every writable path
//! removed, which proves a write cannot succeed there. It cannot show that the
//! recipe never tries: an attempt whose error is discarded (`2>/dev/null`,
//! `|| true`) leaves success behind, and a sandbox whose job is to block writes
//! has nothing to report. This module supplies the complementary contract. The
//! recipe contains no error suppression and no write-intent command at all, so
//! there is no attempt left for a failure to be silently swallowed by.
//!
//! `suppressed_write_attempts` is exercised with representative suppressions
//! and writes, so the check is known to fire rather than merely known to pass
//! on today's recipe.

use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use rstest::rstest;

use crate::makefile::{read_repo_file, target_recipe};

/// The Make target whose recipe must stay free of write attempts.
const KANI_CHECK_TARGET: &str = "kani-check";

/// Shell fragments that discard a command's failure.
///
/// `|| exit 0` is the subtle one: it does not merely ignore a failure, it
/// reports success in its place.
const ERROR_SUPPRESSIONS: [&str; 3] = ["|| true", "|| :", "|| exit 0"];

/// Commands that create, alter or delete something on the filesystem.
const WRITE_COMMANDS: [&str; 12] = [
    "mkdir", "touch", "cp", "mv", "rm", "rmdir", "ln", "tee", "dd", "truncate", "mktemp", "chmod",
];

/// Return every way `recipe` could attempt a write or discard a failure.
///
/// A redirect that does not duplicate a descriptor (`>&2`, `2>&1`) writes to a
/// path, so it counts as a write attempt however the caller means to use it:
/// the query's only output channel is its own standard streams.
///
/// The scan is literal rather than a shell parse, and quoted text is included.
/// A diagnostic that needed to print a `>` would therefore be reported too;
/// that is deliberate, because such a change deserves a review rather than a
/// silent pass.
fn suppressed_write_attempts(recipe: &str) -> Vec<String> {
    let mut findings: Vec<String> = ERROR_SUPPRESSIONS
        .iter()
        .filter(|fragment| recipe.contains(*fragment))
        .map(|fragment| format!("`{fragment}` discards a command's failure"))
        .collect();
    findings.extend(file_redirects(recipe));
    findings.extend(write_commands(recipe));
    findings
}

/// Return the redirections in `recipe` that name a filesystem path.
///
/// `>&2` and `2>&1` duplicate a descriptor and are left alone; everything else
/// following a `>` is a path the shell would create or truncate.
fn file_redirects(recipe: &str) -> Vec<String> {
    recipe
        .char_indices()
        .filter(|&(_, character)| character == '>')
        .filter_map(|(index, _)| {
            let rest = recipe.get(index + 1..)?;
            (!rest.trim_start().starts_with('&')).then(|| {
                let target = rest
                    .split(|c: char| c.is_whitespace() || c == ';')
                    .next()
                    .unwrap_or_default();
                format!("`> {target}` would write a file")
            })
        })
        .collect()
}

/// Return the write-intent commands appearing as whole words in `recipe`.
fn write_commands(recipe: &str) -> Vec<String> {
    recipe
        .split(|c: char| c.is_whitespace() || ";|()&".contains(c))
        .filter(|token| WRITE_COMMANDS.contains(token))
        .map(|token| format!("`{token}` would write to the filesystem"))
        .collect()
}

/// Check that the shipped `kani-check` recipe attempts no writes at all.
///
/// # Errors
///
/// Returns an error when the Makefile cannot be read, declares no `kani-check`
/// recipe, or the recipe contains a suppression or a write-intent command.
#[test]
fn kani_check_recipe_contains_no_write_attempts_or_suppressions() -> Result<()> {
    let makefile = read_repo_file(Utf8Path::new("Makefile"))?;
    let recipe = target_recipe(&makefile, KANI_CHECK_TARGET)
        .with_context(|| format!("the Makefile should declare a {KANI_CHECK_TARGET} recipe"))?;
    let findings = suppressed_write_attempts(&recipe);

    ensure!(
        findings.is_empty(),
        concat!(
            "the {target} recipe must not attempt a write or discard a failure; ",
            "the query is read-only, and an ignored write failure is invisible ",
            "to the sandbox that blocks it. Found: {findings:?}",
        ),
        target = KANI_CHECK_TARGET,
        findings = findings
    );
    Ok(())
}

#[rstest]
#[case::discarded_stderr("mkdir -p \"$cache\" 2>/dev/null", "`mkdir`")]
#[case::discarded_status("cat \"$pin\" || true", "`|| true`")]
#[case::false_success("cat \"$pin\" || exit 0", "`|| exit 0`")]
#[case::truncating_redirect(": > \"$path\"", "would write a file")]
#[case::appending_redirect("printf done >> \"$log\"", "would write a file")]
#[case::touch_stamp("touch \"$stamp\"", "`touch`")]
#[case::remove_lock("rm -f \"$lock\"", "`rm`")]
#[case::make_temp("cache=$(mktemp -d)", "`mktemp`")]
fn representative_suppressions_and_writes_are_reported(
    #[case] recipe: &str,
    #[case] expected: &str,
) {
    let findings = suppressed_write_attempts(recipe);

    assert!(
        findings.iter().any(|finding| finding.contains(expected)),
        "the check should report {expected} in {recipe:?}, got {findings:?}"
    );
}

#[rstest]
#[case::stderr_diagnostic("printf 'prover-tools: done\\n' >&2")]
#[case::captured_stderr("output=\"$(kani --version 2>&1)\"")]
#[case::descriptor_only("status=$?; printf '%s\\n' \"$status\" >&2")]
fn diagnostics_that_only_use_the_standard_streams_are_accepted(#[case] recipe: &str) {
    let findings = suppressed_write_attempts(recipe);

    assert!(
        findings.is_empty(),
        "the check should accept {recipe:?}, got {findings:?}"
    );
}
