//! Shared helpers for the linter's unit tests.
//!
//! Every rule test starts from manifest text, so a [`Fixture`] runs the real
//! pipeline — parse, expand, render, lower — and then lints the result. A rule
//! test therefore exercises the same artefacts the command does.
//!
//! A fixture names the recipe shell its text is written in. The host default is
//! PowerShell on Windows and POSIX elsewhere, so a body holding shell syntax
//! only one dialect parses would otherwise be a different test on each
//! platform.
//!
//! The methods are fallible and the assertions are macros. A fixture that
//! cannot be built is a broken test rather than a verdict, so it propagates;
//! the macros expand in the calling test, which keeps a failure's line number
//! pointing at the case that failed rather than at this file.

use anyhow::{Context, Result};

use crate::ir::BuildGraph;
use crate::lint::finding::Finding;
use crate::lint::policy::Policy;
use crate::lint::{Category, Outcome, Request, analyse};
use crate::manifest;
use crate::recipe_shell::RecipeShell;

/// Manifest text under test, with the interpreter its recipes are lowered for.
///
/// The two travel together: a body is only meaningful once a dialect has read
/// it, and lowering for the wrong one fails closed rather than falling back.
/// Holding them in one value keeps the choice visible at the call site instead
/// of leaving it to a default the reader has to look up.
///
/// # Examples
///
/// ```rust,ignore
/// let findings = Fixture::for_shell(manifest, RecipeShell::Posix)
///     .only("manual-ninja-escape")?;
/// assert_eq!(findings.len(), 1);
/// ```
pub struct Fixture<'a> {
    /// Manifest source the rule under test reads.
    yaml: &'a str,
    /// Interpreter that receives completed recipe text.
    shell: RecipeShell,
}

impl<'a> Fixture<'a> {
    /// Hold `yaml` for lowering under the host's default recipe shell.
    ///
    /// Use this when the rule under test is indifferent to the dialect.
    #[must_use]
    pub fn new(yaml: &'a str) -> Self {
        Self {
            yaml,
            shell: RecipeShell::host_default(),
        }
    }

    /// Hold `yaml` for lowering under `shell`.
    ///
    /// Use this when the recipe text is itself the subject, as it is for a rule
    /// that scans shell syntax or for a fixture containing syntax only one
    /// dialect accepts.
    #[must_use]
    pub fn for_shell(yaml: &'a str, shell: RecipeShell) -> Self {
        Self { yaml, shell }
    }

    /// Lint the fixture under `policy`.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixture does not parse, lower, or index.
    pub fn lint(&self, policy: &Policy) -> Result<Outcome> {
        let parsed = manifest::from_str(self.yaml).context("fixture manifest should parse")?;
        let graph = BuildGraph::from_manifest_for_shell(&parsed, self.shell)
            .context("fixture manifest should lower")?;
        analyse(
            Request {
                source: self.yaml.to_owned(),
                manifest: &parsed,
                graph: &graph,
            },
            policy,
        )
        .map_err(|failure| anyhow::anyhow!("fixture manifest should index: {}", failure.message))
    }

    /// Lint the fixture with only `rule` enabled.
    ///
    /// # Errors
    ///
    /// Returns an error when `rule` is not registered or the fixture does not
    /// compile.
    pub fn only(&self, rule: &str) -> Result<Vec<Finding>> {
        let policy = Policy::resolve(&rule_policy(rule))
            .map_err(|error| anyhow::anyhow!("selectors should resolve: {}", error.message()))?;
        Ok(self.lint(&policy)?.findings)
    }

    /// Report the messages `rule` produced.
    ///
    /// # Errors
    ///
    /// Returns an error when `rule` is not registered or the fixture does not
    /// compile.
    pub fn messages(&self, rule: &str) -> Result<Vec<String>> {
        Ok(self
            .only(rule)?
            .iter()
            .map(Finding::display_message)
            .collect())
    }

    /// Report the source text each finding of `rule` points at.
    ///
    /// # Errors
    ///
    /// Returns an error when `rule` is not registered or the fixture does not
    /// compile.
    pub fn spans(&self, rule: &str) -> Result<Vec<String>> {
        Ok(self
            .only(rule)?
            .iter()
            .map(|finding| {
                finding.span().map_or_else(
                    || "<no span>".to_owned(),
                    |span| {
                        self.yaml
                            .get(span.start..span.end)
                            .unwrap_or_default()
                            .to_owned()
                    },
                )
            })
            .collect())
    }

    /// Report the rules that fired under the registry defaults.
    ///
    /// A rule that reasons about other rules' findings — `unused-suppression`
    /// — must be measured with the default policy rather than in isolation.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixture does not compile.
    pub fn rules_fired(&self) -> Result<Vec<&'static str>> {
        Ok(self
            .lint(&Policy::defaults())?
            .findings
            .iter()
            .map(|finding| finding.meta.name)
            .collect())
    }
}

/// Selectors that silence every category and then enable only `rule`.
///
/// Isolating one rule means disabling every category before re-enabling it, so
/// a finding cannot arrive from a neighbour.
fn rule_policy(rule: &str) -> Vec<String> {
    let mut selectors: Vec<String> = Category::ALL
        .into_iter()
        .map(|category| format!("{}=off", category.as_str()))
        .collect();
    selectors.push(format!("{rule}=warning"));
    selectors
}

/// Lint a fixture under the registry defaults, failing the calling test when
/// the fixture itself is broken.
#[macro_export]
macro_rules! lint_fixture {
    ($yaml:expr $(,)?) => {
        $crate::lint::test_support::Fixture::new($yaml)
            .lint(&$crate::lint::policy::Policy::defaults())
            .expect("fixture should compile")
    };
}

/// Lint a fixture under an explicit policy.
#[macro_export]
macro_rules! lint_fixture_with {
    ($yaml:expr, $policy:expr $(,)?) => {
        $crate::lint::test_support::Fixture::new($yaml)
            .lint($policy)
            .expect("fixture should compile")
    };
}

/// Report the messages one rule produced for a fixture.
#[macro_export]
macro_rules! lint_messages {
    ($yaml:expr, $rule:expr $(,)?) => {
        $crate::lint::test_support::Fixture::new($yaml)
            .messages($rule)
            .expect("fixture should compile")
    };
}

/// Report the source text each of one rule's findings points at.
#[macro_export]
macro_rules! lint_spans {
    ($yaml:expr, $rule:expr $(,)?) => {
        $crate::lint::test_support::Fixture::new($yaml)
            .spans($rule)
            .expect("fixture should compile")
    };
}

/// Report how many findings one rule produced for a fixture.
#[macro_export]
macro_rules! lint_count {
    ($yaml:expr, $rule:expr $(,)?) => {
        $crate::lint::test_support::Fixture::new($yaml)
            .only($rule)
            .expect("fixture should compile")
            .len()
    };
}

/// Report the rules that fired for a fixture under the registry defaults.
#[macro_export]
macro_rules! lint_rules_fired {
    ($yaml:expr $(,)?) => {
        $crate::lint::test_support::Fixture::new($yaml)
            .rules_fired()
            .expect("fixture should compile")
    };
}

/// Assert that one rule fires exactly `$count` times for a fixture.
#[macro_export]
macro_rules! assert_lint_fires {
    ($yaml:expr, $rule:expr, $count:expr $(,)?) => {{
        let messages = $crate::lint_messages!($yaml, $rule);
        assert_eq!(
            messages.len(),
            $count,
            "`{}` should fire {} time(s); reported {messages:?}",
            $rule,
            $count
        );
    }};
}

/// Assert that one rule reports nothing for a fixture.
#[macro_export]
macro_rules! assert_lint_silent {
    ($yaml:expr, $rule:expr $(,)?) => {{
        let messages = $crate::lint_messages!($yaml, $rule);
        assert!(
            messages.is_empty(),
            "`{}` should report nothing; reported {messages:?}",
            $rule
        );
    }};
}

/// Assert that one rule fires under the registry defaults.
#[macro_export]
macro_rules! assert_lint_fires_by_default {
    ($yaml:expr, $rule:expr $(,)?) => {{
        let fired = $crate::lint_rules_fired!($yaml);
        assert!(
            fired.contains(&$rule),
            "`{}` should fire under the defaults; fired {fired:?}",
            $rule
        );
    }};
}

/// Assert that one rule stays silent under the registry defaults.
#[macro_export]
macro_rules! assert_lint_silent_by_default {
    ($yaml:expr, $rule:expr $(,)?) => {{
        let fired = $crate::lint_rules_fired!($yaml);
        assert!(
            !fired.contains(&$rule),
            "`{}` should stay silent under the defaults; fired {fired:?}",
            $rule
        );
    }};
}
