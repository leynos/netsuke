//! Contract test: no compiled source suppresses the environment-access policy.
//!
//! `clippy.toml` disallows the process-environment entry points, and the
//! workspace denies `clippy::disallowed_methods`, but the two together do not
//! close the door. An inner attribute — an `allow` of the policy lint, at the
//! top of a file — switches the lint off for everything below it and passes
//! `make lint` with every other contract green: `clippy.toml` still lists the
//! methods, the workspace still denies the lint, and the lint target still runs
//! across the workspace. None of them observes that a source opted out, because
//! each asserts a true statement about something else. A green tick means the
//! gate ran, not that it was allowed to see anything.
//!
//! Clippy cannot close this itself: `clippy::allow_attributes` does not fire on
//! inner attributes, so the seam taxonomy's "an `#[expect]` carrying a reason,
//! never an `allow`" rule has no mechanical enforcement there. The assertion is
//! therefore about source text, which is normally the wrong shape for a
//! contract — but an attribute *is* source text, there is no execution to
//! model, and "this file does not opt out" is exactly a statement about what
//! the file says.
//!
//! The scan recognizes an attribute only where a line begins with one, so prose
//! that quotes the attribute — this module's own documentation, a mutation
//! record, a snapshot of a diagnostic — is not a finding. It reads an attribute
//! to its matching parenthesis, so one `rustfmt` has wrapped across several
//! lines is read whole rather than truncated.
//!
//! See `docs/adr-008-environment-seam-taxonomy.md` for the taxonomy, and the
//! developers' guide for the sanctioned forms and the scoped exemption below.

use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use cap_std::{ambient_authority, fs_utf8::Dir};

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
const FORBIDDEN_ALLOW_LINTS: [&str; 6] = [
    "clippy::disallowed_methods",
    "clippy::style",
    "clippy::all",
    "warnings",
    "clippy::allow_attributes",
    "clippy::allow_attributes_without_reason",
];

/// Roots, relative to the workspace root, whose Rust sources the crate compiles.
const COMPILED_SOURCE_ROOTS: [&str; 3] = ["src", "build_l10n_audit", "test_support/src"];

/// Compiled sources that sit outside every [`COMPILED_SOURCE_ROOTS`] root.
///
/// `build.rs` is compiled and linted by `cargo clippy --all-targets`, and it is
/// where a build script's own environment reads live, so an inner attribute
/// there silences the policy for the build script exactly as it would in a
/// library source. It is listed separately because the walk is over directories
/// and this is a file.
const STANDALONE_COMPILED_SOURCES: [&str; 1] = ["build.rs"];

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

/// Return the byte offset of the `(` when `line` begins an `allow` attribute.
///
/// The anchor is what keeps this off prose. A whole-text search also matches a
/// citation — this module's documentation, a mutation record that quotes the
/// attribute it exists to prohibit — and reports an innocent source as
/// suppressing the policy. Matching only at the start of a line also excludes
/// `#[expect(...)]`, `.expect(...)` method calls, and commented-out attributes.
///
/// Requiring the `(` on the same line is what `rustfmt` guarantees: it
/// normalizes `# ! [allow` to `#![allow` and keeps the parenthesis with the
/// path, and `make check-fmt` rejects a source that has not been through it. A
/// contributor therefore cannot evade the scan by spacing the attribute out,
/// because the spelling that would evade it does not survive the format gate.
fn attribute_open_paren(line: &str) -> Option<usize> {
    let trimmed = line.trim_start();
    let (rest, marker) = match trimmed.strip_prefix("#![allow") {
        Some(rest) => (rest, "#![allow"),
        None => (trimmed.strip_prefix("#[allow")?, "#[allow"),
    };
    let after_marker = rest.trim_start();
    after_marker.strip_prefix('(')?;
    let indent = line.len() - trimmed.len();
    Some(indent + marker.len() + (rest.len() - after_marker.len()))
}

/// Read the parenthesized body of the attribute whose `(` sits at `open`.
///
/// Scanning tracks parenthesis depth so an attribute `rustfmt` has wrapped
/// across several lines is read whole. Parentheses inside a double-quoted
/// string are stepped over, and a backslash escape is honoured, so a `)` inside
/// a `reason` string does not end the scan early.
///
/// Returns `None` when the parenthesis is never closed, which leaves the
/// malformed attribute unread rather than reporting the remainder of the file
/// as its body.
fn read_attribute_body(source: &str, open: usize) -> Option<String> {
    let mut depth = 0_usize;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, byte) in source.as_bytes().iter().enumerate().skip(open) {
        if in_string {
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'(' => depth += 1,
            b')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return source.get(open + 1..offset).map(str::to_owned);
                }
            }
            _ => {}
        }
    }
    None
}

/// Return the body of every `allow` attribute in `source`.
///
/// The line offset is tracked by summing the lengths of the lines before it, so
/// the offset handed to [`read_attribute_body`] indexes the whole source rather
/// than the line. `split_inclusive` keeps the newline, which is what makes the
/// running total exact.
fn allow_attribute_bodies(source: &str) -> Vec<String> {
    let mut bodies = Vec::new();
    let mut line_start = 0_usize;
    for line in source.split_inclusive('\n') {
        if let Some(open) = attribute_open_paren(line) {
            bodies.extend(read_attribute_body(source, line_start + open));
        }
        line_start += line.len();
    }
    bodies
}

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
fn named_lints(body: &str) -> Vec<String> {
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
fn is_offence(path: &str, lint: &str) -> bool {
    if !FORBIDDEN_ALLOW_LINTS.contains(&lint) {
        return false;
    }
    !SCOPED_ALLOWLIST
        .iter()
        .any(|(allowed_path, allowed_lints)| *allowed_path == path && allowed_lints.contains(&lint))
}

/// Return every suppression of the policy `source` contains, as `(path, lint)`.
fn scan_source(path: &str, source: &str) -> Vec<(String, String)> {
    let mut findings = Vec::new();
    for body in allow_attribute_bodies(source) {
        for lint in named_lints(&body) {
            if is_offence(path, &lint) {
                findings.push((path.to_owned(), lint));
            }
        }
    }
    findings
}

/// Append every `.rs` source beneath `directory`, with its contents, in order.
fn collect_rust_sources(
    root: &Dir,
    directory: &str,
    sources: &mut Vec<(String, String)>,
) -> Result<()> {
    for entry_result in root
        .read_dir(directory)
        .with_context(|| format!("read `{directory}`"))?
    {
        let entry = entry_result.with_context(|| format!("read an entry of `{directory}`"))?;
        let name = entry
            .file_name()
            .with_context(|| format!("read an entry name in `{directory}`"))?;
        let path = format!("{directory}/{name}");
        let file_type = entry
            .file_type()
            .with_context(|| format!("read the file type of `{path}`"))?;
        if file_type.is_dir() {
            collect_rust_sources(root, &path, sources)?;
        } else if is_rust_source(&name) {
            let contents = root
                .read_to_string(&path)
                .with_context(|| format!("read {path}"))?;
            sources.push((path, contents));
        }
    }
    Ok(())
}

/// Return whether an entry name is a Rust source.
fn is_rust_source(name: &str) -> bool {
    Utf8Path::new(name).extension().is_some_and(|it| it == "rs")
}

/// Read every compiled source the scan governs, with its workspace-relative path.
fn compiled_sources() -> Result<Vec<(String, String)>> {
    let crate_root = Dir::open_ambient_dir(env!("CARGO_MANIFEST_DIR"), ambient_authority())
        .context("open the workspace root")?;
    let mut sources = Vec::new();
    for root_path in COMPILED_SOURCE_ROOTS {
        collect_rust_sources(&crate_root, root_path, &mut sources)
            .with_context(|| format!("walk the `{root_path}` source root"))?;
    }
    for path in STANDALONE_COMPILED_SOURCES {
        let contents = crate_root
            .read_to_string(path)
            .with_context(|| format!("read {path}"))?;
        sources.push((path.to_owned(), contents));
    }
    Ok(sources)
}

/// Render every finding as one failure message, so one run names them all.
///
/// Collecting before asserting means a contributor sees every offending file in
/// one run rather than fixing them one gate at a time. The message is assembled
/// from a joined list rather than pushed into a growing `String`, because the
/// helper sits outside a `#[test]` body, where neither `expect` nor the
/// `std::fmt::Write` result may be discarded.
fn build_error_message(findings: &[(String, String)]) -> String {
    let listed = findings
        .iter()
        .map(|(path, lint)| format!("{path}: {lint}"))
        .collect::<Vec<_>>()
        .join("\n- ");
    format!(
        "the environment-access policy is suppressed in compiled sources; \
         remove the `allow` and state the site as `#[expect(..., reason = \"...\")]`, \
         or route the access through a seam:\n- {listed}"
    )
}

/// Fail if any compiled source suppresses the environment-access policy.
#[test]
fn compiled_sources_never_suppress_the_environment_policy() -> Result<()> {
    let sources = compiled_sources()?;
    // A sweep that silently matched nothing would make the assertion vacuous.
    ensure!(
        !sources.is_empty(),
        "the compiled source roots should contain at least one Rust source"
    );
    let findings: Vec<(String, String)> = sources
        .iter()
        .flat_map(|(path, contents)| scan_source(path, contents))
        .collect();

    ensure!(findings.is_empty(), "{}", build_error_message(&findings));
    Ok(())
}

/// An inner `allow` of the policy lint is the evasion this contract exists for.
#[test]
fn inner_allow_of_the_policy_lint_is_reported() -> Result<()> {
    let source = "#![allow(clippy::disallowed_methods, reason = \"escape hatch probe\")]\n";
    let findings = scan_source("src/lib.rs", source);

    ensure!(
        findings
            == [(
                String::from("src/lib.rs"),
                String::from("clippy::disallowed_methods")
            )],
        "expected the inner allow to be reported, got {findings:?}"
    );
    Ok(())
}

/// An item-level `allow` of the blanket spelling is reported too.
#[test]
fn item_allow_of_warnings_is_reported() -> Result<()> {
    let source = "#[allow(warnings, reason = \"escape hatch probe\")]\nfn probe() {}\n";
    let findings = scan_source("src/lib.rs", source);

    ensure!(
        findings == [(String::from("src/lib.rs"), String::from("warnings"))],
        "expected the item-level allow to be reported, got {findings:?}"
    );
    Ok(())
}

/// The group above the policy lint silences it just as naming it does.
#[test]
fn allow_of_the_enclosing_group_is_reported() -> Result<()> {
    let source = "#![allow(clippy::style, reason = \"escape hatch probe\")]\n";
    let findings = scan_source("src/lib.rs", source);

    ensure!(
        findings == [(String::from("src/lib.rs"), String::from("clippy::style"))],
        "expected the enclosing group to be reported, got {findings:?}"
    );
    Ok(())
}

/// A wrapped attribute is read whole, as `rustfmt` writes a long one.
#[test]
fn wrapped_attribute_is_read_whole() -> Result<()> {
    let source =
        "#![allow(\n    clippy::disallowed_methods,\n    reason = \"escape hatch probe\"\n)]\n";
    let findings = scan_source("src/lib.rs", source);

    ensure!(
        findings
            == [(
                String::from("src/lib.rs"),
                String::from("clippy::disallowed_methods")
            )],
        "expected the wrapped allow to be reported, got {findings:?}"
    );
    Ok(())
}

/// A `)` inside the reason string does not end the attribute early.
#[test]
fn parenthesis_inside_a_reason_does_not_end_the_attribute() -> Result<()> {
    let source = "#[allow(warnings, reason = \"closing ) paren\")]\nfn probe() {}\n";
    let findings = scan_source("src/lib.rs", source);

    ensure!(
        findings == [(String::from("src/lib.rs"), String::from("warnings"))],
        "expected the allow to be read to its matching parenthesis, got {findings:?}"
    );
    Ok(())
}

/// Suppression in general is not the offence: an unrelated lint still passes.
#[test]
fn unrelated_allow_is_not_reported() -> Result<()> {
    let source = "#![allow(dead_code, reason = \"shared test-support module\")]\n";
    let findings = scan_source("test_support/src/lib.rs", source);

    ensure!(
        findings.is_empty(),
        "expected an unrelated allow to pass, got {findings:?}"
    );
    Ok(())
}

/// The exemption covers the derive-isolation modules and only those.
#[test]
fn guard_lint_exemption_is_scoped_to_the_isolation_modules() -> Result<()> {
    let source = "#![allow(\n    clippy::allow_attributes,\n    clippy::allow_attributes_without_reason,\n    unused_assignments\n)]\n";
    let findings = scan_source("src/runner/error.rs", source);

    ensure!(
        findings.is_empty(),
        "expected the documented exemption to pass, got {findings:?}"
    );
    Ok(())
}

/// The same attribute elsewhere still names the policy-carrying guard lints.
#[test]
fn guard_lint_exemption_does_not_cover_other_paths() -> Result<()> {
    let source = "#![allow(\n    clippy::allow_attributes,\n    clippy::allow_attributes_without_reason,\n    unused_assignments\n)]\n";
    let findings = scan_source("src/elsewhere.rs", source);

    ensure!(
        findings
            == [
                (
                    String::from("src/elsewhere.rs"),
                    String::from("clippy::allow_attributes")
                ),
                (
                    String::from("src/elsewhere.rs"),
                    String::from("clippy::allow_attributes_without_reason")
                )
            ],
        "expected the guard lints to be reported off the exempt paths, got {findings:?}"
    );
    Ok(())
}
