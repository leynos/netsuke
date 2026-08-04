//! Shared helpers for static Makefile contract tests.
//!
//! Scope, deliberately narrow: reading a file from the repository root through
//! a capability-scoped handle, and parsing a Make rule into its prerequisites
//! and recipe. Nothing here runs Make, runs Cargo, or writes anything.
//!
//! Reuse policy: include this module from a `tests/*.rs` binary that asserts on
//! Makefile *text*. Do not grow it into a general test-utility bag — fixture
//! construction, process invocation, and environment control belong in the
//! `test_support` crate, which is versioned, linted, and documented as such.
//! A helper earns a place here only when more than one contract test needs the
//! same reading or parsing behaviour.
//!
//! Integration tests under `tests/` compile as independent crates, so there is
//! no library to share through. The module lives in a subdirectory, which Cargo
//! does not auto-discover as a test target, and each consumer includes it with
//! `#[path = "support/makefile.rs"] mod makefile;`.
//!
//! Every helper is exercised by this module's own unit tests, which run once
//! per including crate. That is what keeps a consumer using only part of the
//! surface from tripping `dead_code`.

use anyhow::{Context, Result};
use camino::Utf8Path;
use cap_std::{ambient_authority, fs_utf8::Dir};

/// Opens the repository root as a capability-scoped directory handle.
///
/// Every read goes through this handle, so a contract test cannot reach
/// outside the checkout.
///
/// # Errors
///
/// Returns an error when the manifest directory cannot be opened.
///
/// # Examples
///
/// ```no_run
/// let root = repo_root().expect("open the repository root");
/// let makefile = root.read_to_string("Makefile").expect("read the Makefile");
/// assert!(makefile.contains("test-nextest:"));
/// ```
pub fn repo_root() -> Result<Dir> {
    Dir::open_ambient_dir(env!("CARGO_MANIFEST_DIR"), ambient_authority())
        .context("open the repository root as a capability-scoped directory")
}

/// Reads `relative` from the repository root as a UTF-8 string.
///
/// # Errors
///
/// Returns an error naming `relative` when it cannot be read.
///
/// # Examples
///
/// ```no_run
/// use camino::Utf8Path;
///
/// let makefile = read_repo_file(Utf8Path::new("Makefile")).expect("read the Makefile");
/// assert!(makefile.contains("test-nextest:"));
/// ```
pub fn read_repo_file(relative: &Utf8Path) -> Result<String> {
    repo_root()?
        .read_to_string(relative)
        .with_context(|| format!("{relative} should be readable"))
}

/// Splits a Make rule line into its target and its prerequisites.
///
/// Returns `None` for anything that is not a rule header: recipe and
/// continuation lines (leading tab or space), comments, directives such as
/// `.PHONY`, and variable assignments, which `rest.starts_with('=')`
/// distinguishes from a rule by catching the `:=` form.
///
/// Trailing `## ` help comments are discarded so `help` annotations do not leak
/// into the prerequisite list.
///
/// # Examples
///
/// ```
/// let (target, prerequisites) =
///     parse_rule("alpha: beta gamma ## build everything").expect("a rule header");
/// assert_eq!(target, "alpha");
/// assert_eq!(prerequisites, ["beta", "gamma"]);
///
/// // A recipe line is not a rule header.
/// assert_eq!(parse_rule("\techo one"), None);
/// ```
pub fn parse_rule(line: &str) -> Option<(&str, Vec<&str>)> {
    if line.starts_with(['\t', ' ', '#', '.']) {
        return None;
    }
    let (target, rest) = line.split_once(':')?;
    if target.is_empty() || rest.starts_with('=') {
        return None;
    }
    let prerequisites = rest
        .split("##")
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .collect();
    Some((target.trim(), prerequisites))
}

/// Returns the prerequisites declared for `target`.
///
/// Yields `None` when `contents` declares no such target.
///
/// # Examples
///
/// ```
/// let makefile = "alpha: beta gamma\n\techo one\n";
/// assert_eq!(
///     target_prerequisites(makefile, "alpha"),
///     Some(vec!["beta".to_owned(), "gamma".to_owned()])
/// );
/// ```
pub fn target_prerequisites(contents: &str, target: &str) -> Option<Vec<String>> {
    contents.lines().find_map(|line| {
        let (name, prerequisites) = parse_rule(line)?;
        (name == target).then(|| prerequisites.into_iter().map(ToOwned::to_owned).collect())
    })
}

/// Returns the tab-indented recipe lines for `target`, joined by newlines.
///
/// A target with no recipe yields an empty string; an absent target yields
/// `None`. Blank lines inside a recipe are traversed but dropped, so a recipe
/// separated by a blank line is returned whole.
///
/// # Examples
///
/// ```
/// let makefile = "alpha: beta\n\techo one\n\n\techo two\n\nbeta:\n";
/// assert_eq!(
///     target_recipe(makefile, "alpha").as_deref(),
///     Some("\techo one\n\techo two")
/// );
/// ```
pub fn target_recipe(contents: &str, target: &str) -> Option<String> {
    let mut lines = contents
        .lines()
        .skip_while(|line| parse_rule(line).is_none_or(|(name, _)| name != target));
    lines.next()?;
    let recipe: Vec<&str> = lines
        .take_while(|line| line.starts_with('\t') || line.trim().is_empty())
        .filter(|line| line.starts_with('\t'))
        .collect();
    Some(recipe.join("\n"))
}

#[cfg(test)]
mod tests {
    //! Edge cases the shared parser must support.
    //!
    //! These also keep every helper used from each including crate, so a
    //! consumer needing only part of the surface does not trip `dead_code`.

    use super::{parse_rule, read_repo_file, repo_root, target_prerequisites, target_recipe};
    use camino::Utf8Path;

    const SAMPLE: &str = concat!(
        "# a comment\n",
        ".PHONY: alpha beta\n",
        "BUILD_JOBS ?=\n",
        "VAR := value\n",
        "alpha: beta gamma ## help text\n",
        "\techo one\n",
        "\n",
        "\techo two\n",
        "\n",
        "beta: ## no recipe\n",
        "\n",
        "gamma:\n",
        "\techo three\n",
    );

    #[test]
    fn parse_rule_accepts_a_rule_header_and_strips_help_comments() {
        assert_eq!(
            parse_rule("alpha: beta gamma ## help text"),
            Some(("alpha", vec!["beta", "gamma"]))
        );
        assert_eq!(parse_rule("gamma:"), Some(("gamma", vec![])));
    }

    #[test]
    fn parse_rule_rejects_everything_that_is_not_a_rule_header() {
        assert_eq!(parse_rule("\techo one"), None, "recipe line");
        assert_eq!(parse_rule("  continued"), None, "continuation line");
        assert_eq!(parse_rule("# a comment"), None, "comment");
        assert_eq!(parse_rule(".PHONY: alpha"), None, "directive");
        assert_eq!(parse_rule("VAR := value"), None, "assignment");
        assert_eq!(parse_rule("BUILD_JOBS ?="), None, "no colon");
        assert_eq!(parse_rule(": orphan"), None, "empty target");
        assert_eq!(parse_rule(""), None, "blank line");
    }

    #[test]
    fn target_prerequisites_reports_declared_dependencies() {
        assert_eq!(
            target_prerequisites(SAMPLE, "alpha"),
            Some(vec!["beta".to_owned(), "gamma".to_owned()])
        );
        assert_eq!(target_prerequisites(SAMPLE, "gamma"), Some(vec![]));
        assert_eq!(target_prerequisites(SAMPLE, "missing"), None);
    }

    #[test]
    fn target_recipe_spans_blank_lines_and_stops_at_the_next_rule() {
        assert_eq!(
            target_recipe(SAMPLE, "alpha").as_deref(),
            Some("\techo one\n\techo two"),
            "a blank line inside a recipe should not truncate it"
        );
        assert_eq!(
            target_recipe(SAMPLE, "gamma").as_deref(),
            Some("\techo three")
        );
    }

    #[test]
    fn target_recipe_distinguishes_an_empty_recipe_from_an_absent_target() {
        assert_eq!(
            target_recipe(SAMPLE, "beta").as_deref(),
            Some(""),
            "a target with no recipe yields an empty string"
        );
        assert_eq!(target_recipe(SAMPLE, "missing"), None);
    }

    #[test]
    fn target_recipe_ignores_a_variable_whose_name_matches_the_target() {
        let makefile = "VAR := not a rule\nVAR:\n\techo real\n";
        assert_eq!(
            target_recipe(makefile, "VAR").as_deref(),
            Some("\techo real")
        );
    }

    #[test]
    fn repo_root_reads_a_known_repository_file() -> anyhow::Result<()> {
        repo_root()?;
        let makefile = read_repo_file(Utf8Path::new("Makefile"))?;
        // `ensure!` rather than `assert!`: the workspace denies
        // `clippy::panic_in_result_fn`, so a Result-returning test reports a
        // failure by returning it.
        anyhow::ensure!(
            makefile.contains("test-nextest:"),
            "the Makefile should declare test-nextest"
        );
        Ok(())
    }
}
