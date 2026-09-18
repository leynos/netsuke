//! What counts as a suppression of the environment-access policy.
//!
//! [`scanner`](super::scanner) finds the `allow` attributes; this module
//! decides which of the names they carry are an offence, given the path the
//! attribute sits in.

/// Lint names an `allow` attribute may not carry in a compiled source.
///
/// The list follows the lint hierarchy rather than spelling one name, because
/// allowing a parent of the policy lint silences it just as naming it does.
/// `disallowed_methods` is declared in Clippy's `style` group, and `clippy::all`
/// sits above that; both were measured to suppress the policy outright under
/// this repository's configuration. `warnings` is the level above them and the
/// blanket spelling a reader reaches for first.
///
/// The two guard lints are what make the seam taxonomy's `expect`-not-`allow`
/// rule enforceable, and they are cheap to protect: a scan that reads the
/// attributes reports an item-level `allow` of the policy lint wherever it
/// sits, but nothing else reports a crate that has silenced the reporter. See
/// "Enforcing the environment mandate" in the developers' guide.
///
/// The last two entries close a second way in, measured rather than assumed.
/// Clippy keeps the old spelling of a renamed lint, and a renamed name still
/// selects the lint it was renamed to, so `clippy::disallowed_method` — an
/// alias of the policy lint — silences the policy exactly as the current name
/// does. Ordinarily that is harmless, because the rename is reported and
/// `renamed_and_removed_lints` is denied, so the alias is an error rather than
/// a suppression. Allowing that lint as well hides the rename, and the alias
/// then silences the policy in silence: measured at exit 0 where the same file
/// without the attribute exits 101. Banning the enabler closes the whole class
/// of alias evasions, since no alias suppresses anything while the rename that
/// names it is still reported; banning the alias too keeps the pair honest if
/// a future Clippy stops reporting renames. Neither name is in the scoped
/// exemption, which covers only the two guard lints.
const FORBIDDEN_ALLOW_LINTS: [&str; 8] = [
    "clippy::disallowed_methods",
    "clippy::style",
    "clippy::all",
    "warnings",
    "clippy::allow_attributes",
    "clippy::allow_attributes_without_reason",
    "clippy::disallowed_method",
    "renamed_and_removed_lints",
];

/// Paths permitted to suppress the two guard lints, and which of those they may.
///
/// This is a scoped exemption, not a general one: a file listed here may still
/// not suppress the policy lint itself, its group, or `warnings`. The three
/// files are the derive-isolation modules documented in the developers' guide.
/// Each isolates `thiserror`/`miette` derive expansions, where
/// `unused_assignments` fires on some Rust versions and not others. `#[expect]`
/// fails when the lint does not fire, and `unfulfilled_lint_expectations`
/// cannot itself be expected, so the module must carry an `allow` — which the
/// guard lints then reject, leaving the module no way to state the suppression
/// that the guard lints themselves require it to state.
///
/// A future reader who removes the workaround should delete the entry for that
/// file here at the same time, or this exemption outlives its reason.
/// See <https://github.com/rust-lang/rust/issues/130021>.
const SCOPED_ALLOWLIST: [(&str, [&str; 2]); 3] = [
    (
        "src/runner/error.rs",
        [
            "clippy::allow_attributes",
            "clippy::allow_attributes_without_reason",
        ],
    ),
    (
        "src/manifest/diagnostics/mod.rs",
        [
            "clippy::allow_attributes",
            "clippy::allow_attributes_without_reason",
        ],
    ),
    (
        "src/manifest/diagnostics/yaml.rs",
        [
            "clippy::allow_attributes",
            "clippy::allow_attributes_without_reason",
        ],
    ),
];

/// Return whether a clause of an attribute body is its `reason = "..."` argument.
fn is_reason_clause(clause: &str) -> bool {
    clause
        .strip_prefix("reason")
        .is_some_and(|rest| rest.trim_start().starts_with('='))
}

/// Split an attribute body on the commas that separate its clauses.
///
/// Only commas at the top level separate clauses: one inside a `reason`
/// string, or inside a nested group, is part of the clause being read.
fn split_clauses(body: &str) -> Vec<String> {
    let mut clauses = Vec::new();
    let mut current = String::new();
    let mut depth = 0_usize;
    let mut in_string = false;
    let mut escaped = false;
    for character in body.chars() {
        if in_string {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                in_string = false;
            }
            current.push(character);
            continue;
        }
        match character {
            '"' => {
                in_string = true;
                current.push(character);
            }
            '(' => {
                depth += 1;
                current.push(character);
            }
            ')' => {
                depth = depth.saturating_sub(1);
                current.push(character);
            }
            ',' if depth == 0 => clauses.push(std::mem::take(&mut current)),
            _ => current.push(character),
        }
    }
    clauses.push(current);
    clauses
}

/// Return the lint names an attribute body carries, excluding its reason.
pub(super) fn named_lints(body: &str) -> Vec<String> {
    split_clauses(body)
        .into_iter()
        .map(|clause| clause.trim().to_owned())
        .filter(|clause| !clause.is_empty() && !is_reason_clause(clause))
        .collect()
}

/// Return whether `path` suppressing `lint` is an offence.
///
/// A path on the scoped allowlist is excused the two guard lints it names and
/// nothing else: an `allow` of the policy lint, of its group, or of `warnings`
/// is a finding wherever it appears, exemption or not.
pub(super) fn is_offence(path: &str, lint: &str) -> bool {
    if !FORBIDDEN_ALLOW_LINTS.contains(&lint) {
        return false;
    }
    !SCOPED_ALLOWLIST
        .iter()
        .any(|(allowed_path, allowed_lints)| *allowed_path == path && allowed_lints.contains(&lint))
}
