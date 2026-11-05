//! Tests for trusted configured-shell definitions.
//!
//! The field suites pin the local validity rules for each declared field. The
//! composition suite pins the registry rules, and compares the production walk
//! with an independent quadratic oracle so a divergence between the two shows
//! up as a test failure rather than as an accepted registry.

use super::{ShellArguments, ShellDefinition, ShellExecutable};
use crate::shell_selection::{HostFamily, ShellName};

mod composition;
mod fields;
mod oracle;

/// Build a definition for `name`, using a fixed program and argument vector.
///
/// Composition and oracle tests vary the name alone, so every other field is
/// the smallest value that passes its own validation.
fn definition(name: &str) -> ShellDefinition {
    ShellDefinition {
        name: ShellName::parse(name).expect("the test names are valid"),
        executable: ShellExecutable::parse("dash", HostFamily::UnixLike)
            .expect("the fixture executable is valid"),
        args: ShellArguments::parse(&["-c".to_owned()]).expect("the fixture arguments are valid"),
    }
}

/// Build the definitions for a sequence of already-validated shell names.
fn definitions(names: &[&str]) -> Vec<ShellDefinition> {
    names.iter().copied().map(definition).collect()
}
