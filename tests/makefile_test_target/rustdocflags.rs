//! Contract model for Makefile recipes that set `RUSTDOCFLAGS`.
//!
//! The repository contract names its caller-overridable value
//! `RUSTDOC_FLAGS`, but Cargo accepts only `RUSTDOCFLAGS`. Each covered recipe
//! must therefore use the Make substitution `$(RUSTDOC_FLAGS)` when assigning
//! Cargo's environment variable. Unlike `rustflags.rs`, this module asserts on
//! the extracted Make token directly: a `sh -c` expansion would not model Make
//! substitution. A completeness check makes new Rustdoc-flag recipes opt into
//! this contract deliberately.

use super::{read_repo_file, target_recipe};
use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use std::collections::BTreeSet;

/// The prefix introducing a quoted `RUSTDOCFLAGS` assignment in a recipe.
const RUSTDOCFLAGS_PREFIX: &str = "RUSTDOCFLAGS=\"";
/// The Make substitution that supplies Cargo's supported environment variable.
const RUSTDOC_FLAGS_TOKEN: &str = "$(RUSTDOC_FLAGS)";

/// A recipe line that assigns `RUSTDOCFLAGS`, and the contract it must meet.
#[derive(Clone, Copy, Debug)]
struct RustdocflagsCase {
    /// The Make target owning the recipe.
    target: &'static str,
    /// Substring selecting the recipe line.
    line_marker: &'static str,
}

impl RustdocflagsCase {
    /// Describe the documentation-test recipe.
    const fn doctest() -> Self {
        Self {
            target: "doctest",
            line_marker: "--doc",
        }
    }

    /// Describe the Rustdoc invocation in the Clippy lint target.
    const fn lint_rustdoc() -> Self {
        Self {
            target: "lint-clippy",
            line_marker: "doc --workspace",
        }
    }

    /// Describe the aggregate documentation-coverage recipe.
    const fn doc_coverage() -> Self {
        Self {
            target: "doc-coverage",
            line_marker: "$(UV_ENV)",
        }
    }
}

/// Enumerate every `RUSTDOCFLAGS`-setting recipe under contract.
const RUSTDOCFLAGS_CASES: [RustdocflagsCase; 3] = [
    RustdocflagsCase::doctest(),
    RustdocflagsCase::lint_rustdoc(),
    RustdocflagsCase::doc_coverage(),
];

/// Extract the double-quoted `RUSTDOCFLAGS` assignment from a recipe line.
fn rustdocflags_assignment(line: &str) -> Option<&str> {
    let start = line.find(RUSTDOCFLAGS_PREFIX)? + RUSTDOCFLAGS_PREFIX.len();
    let rest = line.get(start..)?;
    let end = rest.find('\"')?;
    rest.get(..end)
}

/// Return the recipe line `case` selects.
fn recipe_line(makefile: &str, case: RustdocflagsCase) -> Result<String> {
    let recipe = target_recipe(makefile, case.target)
        .with_context(|| format!("Makefile should declare a {} target", case.target))?;
    recipe
        .lines()
        .find(|line| line.contains(RUSTDOCFLAGS_PREFIX) && line.contains(case.line_marker))
        .map(str::trim)
        .map(ToOwned::to_owned)
        .with_context(|| {
            format!(
                "{} should set RUSTDOCFLAGS on a line matching {:?}",
                case.target, case.line_marker
            )
        })
}

/// Assert that `case` supplies Cargo's Rustdoc flags through Make substitution.
fn assert_make_substitution(makefile: &str, case: RustdocflagsCase) -> Result<()> {
    let line = recipe_line(makefile, case)?;
    let assignment = rustdocflags_assignment(&line).with_context(|| {
        format!(
            "{} should assign a double-quoted RUSTDOCFLAGS value",
            case.target
        )
    })?;
    ensure!(
        assignment == RUSTDOC_FLAGS_TOKEN,
        "{} should assign RUSTDOCFLAGS from {RUSTDOC_FLAGS_TOKEN:?}, found {assignment:?}",
        case.target
    );
    Ok(())
}

#[test]
fn unit_extracts_the_rustdocflags_assignment_from_a_recipe_line() {
    assert_eq!(
        rustdocflags_assignment(r#"\tRUSTDOCFLAGS="$(RUSTDOC_FLAGS)" $(CARGO) x"#),
        Some(RUSTDOC_FLAGS_TOKEN)
    );
    assert_eq!(rustdocflags_assignment("\tcargo build"), None);
}

#[test]
fn behavioural_rustdocflags_recipes_use_the_repository_contract() -> Result<()> {
    let makefile = read_repo_file(Utf8Path::new("Makefile"))?;
    for case in RUSTDOCFLAGS_CASES {
        assert_make_substitution(&makefile, case)?;
    }
    Ok(())
}

#[test]
fn behavioural_rustdocflags_default_preserves_rustdoc_warning_denial() -> Result<()> {
    let makefile = read_repo_file(Utf8Path::new("Makefile"))?;
    let default = makefile
        .lines()
        .find(|line| line.starts_with("RUSTDOC_FLAGS ?="))
        .context("Makefile should declare the RUSTDOC_FLAGS default")?;
    ensure!(
        default.contains("--cfg docsrs"),
        "RUSTDOC_FLAGS should preserve the docsrs configuration: {default:?}"
    );
    ensure!(
        default.contains("-D warnings"),
        "RUSTDOC_FLAGS should deny Rustdoc warnings: {default:?}"
    );
    Ok(())
}

#[test]
fn behavioural_doctest_sets_cargos_rustdocflags_variable() -> Result<()> {
    let makefile = read_repo_file(Utf8Path::new("Makefile"))?;
    let recipe = target_recipe(&makefile, "doctest").context("Makefile should declare doctest")?;
    ensure!(
        recipe.contains(RUSTDOCFLAGS_PREFIX),
        "doctest should set Cargo's RUSTDOCFLAGS variable"
    );
    Ok(())
}

#[test]
fn behavioural_makefile_never_exports_or_shell_expands_rustdoc_flags() -> Result<()> {
    let makefile = read_repo_file(Utf8Path::new("Makefile"))?;
    for (line_number, line) in makefile.lines().enumerate() {
        let is_recipe = line.starts_with('\t');
        ensure!(
            !line.trim_start().starts_with("export RUSTDOC_FLAGS"),
            "Makefile line {} must not export unsupported RUSTDOC_FLAGS: {line:?}",
            line_number + 1
        );
        ensure!(
            !(is_recipe && line.contains("$${RUSTDOC_FLAGS}")),
            "Makefile recipe line {} must not shell-expand RUSTDOC_FLAGS: {line:?}",
            line_number + 1
        );
        ensure!(
            !(is_recipe && line.contains("RUSTDOC_FLAGS=") && line.contains("$(CARGO)")),
            "Makefile recipe line {} must not pass unsupported RUSTDOC_FLAGS to Cargo: {line:?}",
            line_number + 1
        );
    }
    Ok(())
}

#[test]
fn behavioural_every_rustdocflags_recipe_line_is_under_contract() -> Result<()> {
    let makefile = read_repo_file(Utf8Path::new("Makefile"))?;
    let declared: BTreeSet<String> = makefile
        .lines()
        .filter(|line| line.starts_with('\t') && line.contains(RUSTDOCFLAGS_PREFIX))
        .map(|line| line.trim().to_owned())
        .collect();
    let covered: BTreeSet<String> = RUSTDOCFLAGS_CASES
        .iter()
        .map(|case| recipe_line(&makefile, *case))
        .collect::<Result<_>>()?;

    let uncovered: Vec<&String> = declared.difference(&covered).collect();
    ensure!(
        uncovered.is_empty(),
        "every recipe setting RUSTDOCFLAGS needs a RustdocflagsCase; uncovered: {uncovered:#?}"
    );
    ensure!(
        covered.len() == RUSTDOCFLAGS_CASES.len(),
        "each RustdocflagsCase should select a distinct recipe line, {} cases selected {} lines",
        RUSTDOCFLAGS_CASES.len(),
        covered.len()
    );
    Ok(())
}
