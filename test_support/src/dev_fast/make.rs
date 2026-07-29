//! A described Make run, so tests exercise the real recipes.
//!
//! Variable overrides and environment entries are separate because the two
//! reach a recipe by different routes, and the distinction is load-bearing: a
//! command-line variable outranks a `?=` default in the Makefile, whereas an
//! environment entry is the only channel for a setting a script reads directly
//! without the Makefile naming it.

use std::fmt::Display;

/// A Make target plus the overrides a case needs.
///
/// Built fluently, then handed to
/// [`Sandbox::run_make`](super::Sandbox::run_make):
///
/// ```rust,no_run
/// use test_support::dev_fast::MakeInvocation;
///
/// let invocation = MakeInvocation::new("bench-build")
///     .variable("CARGO", "/path/to/fake/cargo")
///     .environment("BENCH_ROOT", "/tmp/bench");
/// ```
#[derive(Clone, Debug)]
pub struct MakeInvocation {
    target: String,
    variables: Vec<(String, String)>,
    environment: Vec<(String, String)>,
}

impl MakeInvocation {
    /// A run of `target` with no overrides.
    pub fn new(target: &str) -> Self {
        Self {
            target: target.to_owned(),
            variables: Vec::new(),
            environment: Vec::new(),
        }
    }

    /// Override a Make variable on the command line, outranking any `?=`
    /// default the Makefile sets.
    #[must_use]
    pub fn variable(mut self, name: &str, value: impl Display) -> Self {
        self.variables.push((name.to_owned(), value.to_string()));
        self
    }

    /// Set an environment entry for the Make process, which recipes inherit.
    /// Use this for settings a script reads directly from the environment.
    #[must_use]
    pub fn environment(mut self, name: &str, value: impl Display) -> Self {
        self.environment.push((name.to_owned(), value.to_string()));
        self
    }

    pub(super) fn target(&self) -> &str {
        &self.target
    }

    /// Variable overrides rendered as `NAME=value` command-line arguments.
    pub(super) fn variable_arguments(&self) -> impl Iterator<Item = String> + '_ {
        self.variables
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
    }

    pub(super) fn environment_entries(&self) -> impl Iterator<Item = (&str, &str)> + '_ {
        self.environment
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
    }
}
