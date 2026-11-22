//! Response-file rendering for a preparation rule that outgrows the command line.
//!
//! A nested output can name a directory long enough that the encoded PowerShell
//! command exceeds Windows' command-line limit. The rule must then carry the
//! script through Ninja's response file instead of the inline
//! `-EncodedCommand` argument. These cases render the rule directly, so they
//! pin the generated text without creating such a directory on disk.

use super::super::write_rule;
use crate::ninja_gen::{InterpreterBinding, RecipeShell};
use camino::{Utf8Path, Utf8PathBuf};

/// Long enough that the encoded script passes any Windows command-line limit.
const OVERSIZED_DIRECTORY: usize = 40_000;

/// Render one preparation rule for `directory`.
fn rendered_rule(directory: &Utf8Path) -> String {
    let mut out = String::new();
    write_rule(
        &mut out,
        RecipeShell::PowerShell,
        InterpreterBinding::InspectionOnly,
        directory,
    )
    .expect("the fixture rule should render");
    out
}

/// Return a directory spelling whose encoded command cannot stay inline.
fn oversized_directory() -> Utf8PathBuf {
    Utf8PathBuf::from(format!("out/{}", "a".repeat(OVERSIZED_DIRECTORY)))
}

/// Move an oversized PowerShell rule onto Ninja's response file.
#[test]
fn an_oversized_directory_uses_a_response_file() {
    let rule = rendered_rule(&oversized_directory());
    assert!(
        rule.contains("  rspfile = $out.netsuke-"),
        "an oversized command must declare Ninja's response file:\n{rule}",
    );
    assert!(
        rule.contains("  rspfile_content = "),
        "the response file must carry the encoded script:\n{rule}",
    );
    let command = rule
        .lines()
        .find(|line| line.contains("  command = "))
        .expect("the rule must declare a command");
    assert!(
        command.contains("-File \"$rspfile\""),
        "the response form must run the script through -File: {command}",
    );
    assert!(
        !command.contains("-EncodedCommand"),
        "the inline encoded form does not fit and must not be emitted: {command}",
    );
}

/// Keep a short PowerShell rule inline.
#[test]
fn a_short_directory_stays_inline() {
    let rule = rendered_rule(Utf8Path::new("build/report"));
    assert!(
        !rule.contains("rspfile"),
        "a short command needs no response file:\n{rule}",
    );
    assert!(
        rule.contains("-EncodedCommand"),
        "a short command must keep the inline encoded form:\n{rule}",
    );
}
