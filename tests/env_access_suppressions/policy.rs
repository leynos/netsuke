//! What counts as a suppression of the environment-access policy.
//!
//! [`scanner`](super::scanner) finds the `allow` attributes; this module
//! decides which carried names constitute an offence, given the path the
//! attribute sits in.

/// Lint names an `allow` attribute may not carry in a compiled source.
///
/// The list follows the lint hierarchy rather than spelling one name, because
/// allowing a parent of the policy lint silences it just as naming it does.
/// `disallowed_methods` is declared in Clippy's `style` group, and `clippy::all`
/// sits above that; both were measured to suppress the policy outright under
/// this repository's configuration, `-D warnings` included.
///
/// `warnings` is banned too, but *not* because it sits above them — it does not.
/// The `warnings` group is the set of lints currently at `warn`, and Cargo
/// passes `[workspace.lints]` as command-line denies, so the policy lint is at
/// `deny` and therefore *outside* the group: `#![allow(warnings)]` on its own
/// leaves the policy lint firing, measured at exit 101 both bare and under the
/// gate's flags. Three measurements say it still belongs in the set. It
/// silences every warn-level lint under the gate, a file with four diagnostics
/// compiling clean; it silences `unfulfilled_lint_expectations`, which is the
/// self-removal mechanism `clippy.toml` relies on when it says the backlog
/// "removes itself instead of rotting"; and it is one half of the only measured
/// way to defeat the gate's own flags, described below.
///
/// That combination is worth stating precisely, because neither half is an
/// evasion alone and the pair is. `#![warn(clippy::disallowed_methods)]` lowers
/// the policy lint from `deny` to `warn`, which *puts it into the `warnings`
/// group*; `#![allow(warnings)]` then suppresses it. Measured at exit 0 under
/// `RUSTFLAGS=-D warnings`, in either order. A `warn` of the policy lint alone
/// is re-promoted by `-D warnings` and exits 101, and the `allow` alone cannot
/// reach the lint; only together do they escape. The ban below is what closes
/// it — the scanner reports the `allow(warnings)` half — and it is the reason
/// this entry is load-bearing rather than decorative. Should the lint target
/// ever stop passing `-D warnings`, the `warn` family must be re-measured
/// before the ban list is trusted: the re-promotion is the only thing holding
/// that side of the pair.
///
/// The two guard lints are what make the seam taxonomy's `expect`-not-`allow`
/// rule enforceable, and they are cheap to protect: a scan that reads the
/// attributes reports an item-level `allow` of the policy lint wherever it
/// sits, but nothing else reports a crate that has silenced the reporter. See
/// "Enforcing the environment mandate" in the developers' guide.
///
/// That second job is why `clippy::restriction` is banned even though it cannot
/// reach the policy lint. It is the *group* of both guard lints — measured from
/// `cargo clippy -- -W help`, which lists `clippy::allow-attributes` and
/// `clippy::allow-attributes-without-reason` as its members — so one crate-level
/// `#![allow(clippy::restriction, reason = "...")]` silences them together, and
/// an item-level bare `allow` further down then passes unreported. Measured on a
/// file whose only offence is that item-level `allow`: exit 101 with no crate
/// attribute, exit 0 with one. The `allow_attributes` entry above does not do
/// this — it leaves both diagnostics firing, exit 101 — because
/// `allow_attributes` does not fire on the *inner* form, which is the whole
/// reason this module reads source text. `blanket_clippy_restriction_lints` is
/// denied in the workspace but does not cover the attribute route either: it
/// fires on a group-level `-W clippy::restriction`, and reports nothing for an
/// attribute naming the group, measured at zero diagnostics.
///
/// The criterion for membership is therefore "can suppress something this
/// module exists to protect", not "can suppress the policy lint", and the two
/// are different sets. Membership is decided by measurement, per name.
///
/// The last three entries close a second way in, measured rather than assumed.
/// Clippy keeps the old spelling of a renamed lint, and a renamed name still
/// selects the lint it was renamed to, so `clippy::disallowed_method` — an
/// alias of the policy lint — silences the policy exactly as the current name
/// does. The same is true of the bare `disallowed_methods`, the name the lint
/// carried in a set of toolchain versions and still accepts. Ordinarily this is
/// harmless, because the rename is reported and `renamed_and_removed_lints` is
/// denied, so an alias is an error rather than a suppression. Allowing that
/// lint as well hides the rename, and the alias then silences the policy in
/// silence: measured at exit 0 where the same file without the attribute exits
/// 101. Banning the enabler closes the whole class of alias evasions, since no
/// alias suppresses anything while the rename that names it is still reported;
/// banning each alias too keeps the pair honest if a future Clippy stops
/// reporting renames.
///
/// `unknown_lints` is deliberately *not* in the set, and the reason is worth
/// stating because it looks like it belongs. It hides the report that an
/// attribute names a lint that does not exist, which sounds like the rename
/// mechanism above. It was measured and it is not: a misspelled name is a no-op
/// whether or not the report is allowed, so suppressing `unknown_lints` cannot
/// silence the policy, and `#![allow(unknown_lints, disallowed_methods)]`
/// without the rename enabler still exits 101. Banning a name that cannot
/// suppress anything would be a rule the code cannot justify. The workspace
/// still denies `unknown_lints`, so a misspelled name remains an error at the
/// lint level, which is where that concern belongs.
///
/// The test is applied to the name, not to the category it belongs to. An
/// earlier reading of this paragraph took "cannot suppress anything" to excuse
/// every name that leaves the policy lint firing, which would also excuse
/// `clippy::allow_attributes` and `clippy::allow_attributes_without_reason`
/// above — both in the set, both unable to reach the policy lint. What
/// distinguishes `unknown_lints` is that no measurement shows it silencing
/// *anything*; the guard lints can be silenced, and `clippy::restriction` does
/// it. So each name here is measured against what it can actually reach.
const FORBIDDEN_ALLOW_LINTS: [&str; 10] = [
    "clippy::disallowed_methods",
    "clippy::style",
    "clippy::all",
    "clippy::restriction",
    "warnings",
    "clippy::allow_attributes",
    "clippy::allow_attributes_without_reason",
    "clippy::disallowed_method",
    "renamed_and_removed_lints",
    "disallowed_methods",
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

/// Return `name` with the spellings that do not change what it denotes removed.
///
/// Two spellings reach the compiler without reaching a reader's eye, and both
/// were measured to silence the policy outright while passing the scan:
/// `r#clippy::disallowed_methods`, where a raw identifier denotes whatever the
/// name without the prefix denotes, and `clippy :: disallowed_methods`, where
/// whitespace separates the segments of one path. Comparing the normalized name
/// rather than the written one is what makes the ban a statement about which
/// lints are named instead of about how they are spelled.
fn canonical_lint(name: &str) -> String {
    name.replace("r#", "")
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

/// Return the lint names an attribute body carries, excluding its reason.
///
/// Each name is normalized by [`canonical_lint`], so a spelling that means the
/// same lint is compared as that lint. The reason clause is recognized on the
/// written text, before normalization, since it is the `reason` token that
/// identifies it rather than any lint it names.
pub(super) fn named_lints(body: &str) -> Vec<String> {
    split_clauses(body)
        .into_iter()
        .map(|clause| clause.trim().to_owned())
        .filter(|clause| !clause.is_empty() && !is_reason_clause(clause))
        .map(|clause| canonical_lint(&clause))
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
