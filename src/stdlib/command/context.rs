//! Context objects shared between command filters and executors.

use std::sync::Arc;

use minijinja::value::Value;

use super::config::{CommandConfig, CommandOptions, OutputMode};
use crate::localization::{self, keys};

/// Shared context available to a command helper while its filter runs.
#[derive(Clone)]
pub(super) struct CommandContext {
    /// Shared command configuration, including capture limits and
    /// capability-scoped filesystem handles.
    config: Arc<CommandConfig>,
    /// Filter options parsed from the template call site.
    options: CommandOptions,
}

impl CommandContext {
    /// Construct a context from shared configuration and parsed options.
    pub(super) const fn new(config: Arc<CommandConfig>, options: CommandOptions) -> Self {
        Self { config, options }
    }

    /// Return the requested stdout mode for execution.
    pub(super) const fn stdout_mode(&self) -> OutputMode {
        self.options.stdout_mode()
    }

    /// Return the shared command configuration.
    pub(super) fn config(&self) -> &CommandConfig {
        &self.config
    }

    /// Return a cloned handle to the shared command configuration.
    pub(super) fn config_handle(&self) -> Arc<CommandConfig> {
        Arc::clone(&self.config)
    }
}

/// Parsed arguments for a single `grep` filter invocation.
pub(super) struct GrepCall<'a> {
    /// The pattern to search for.
    pub(super) pattern: &'a str,
    /// Optional flags passed to the `grep` invocation.
    pub(super) flags: Option<Value>,
}

impl<'a> GrepCall<'a> {
    /// Construct a call from a pattern and optional flags.
    pub(super) const fn new(pattern: &'a str, flags: Option<Value>) -> Self {
        Self { pattern, flags }
    }
}

/// Identifies where a command string came from for error diagnostics.
#[derive(Clone, Copy)]
pub(super) struct CommandLocation<'a> {
    /// The template expression that invoked the command.
    pub(super) template: &'a str,
    /// The command string as authored in the template.
    pub(super) command: &'a str,
}

impl<'a> CommandLocation<'a> {
    /// Construct a location from the invoking template and command.
    pub(super) const fn new(template: &'a str, command: &'a str) -> Self {
        Self { template, command }
    }

    /// Render a human-readable description of the command location.
    pub(super) fn describe(self) -> String {
        localization::message(keys::COMMAND_LOCATION)
            .with_arg("command", self.command)
            .with_arg("template", self.template)
            .to_string()
    }
}
