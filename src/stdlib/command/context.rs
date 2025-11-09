//! Context objects shared between command filters and executors.

use std::sync::Arc;

use minijinja::value::Value;

use super::config::{CommandConfig, CommandOptions, OutputMode};

#[derive(Clone)]
pub(super) struct CommandContext {
    config: Arc<CommandConfig>,
    options: CommandOptions,
}

impl CommandContext {
    pub(super) const fn new(config: Arc<CommandConfig>, options: CommandOptions) -> Self {
        Self { config, options }
    }

    pub(super) const fn stdout_mode(&self) -> OutputMode {
        self.options.stdout_mode()
    }

    pub(super) fn config(&self) -> &CommandConfig {
        &self.config
    }

    pub(super) fn config_handle(&self) -> Arc<CommandConfig> {
        Arc::clone(&self.config)
    }
}

pub(super) struct GrepCall<'a> {
    pub(super) pattern: &'a str,
    pub(super) flags: Option<Value>,
}

impl<'a> GrepCall<'a> {
    pub(super) const fn new(pattern: &'a str, flags: Option<Value>) -> Self {
        Self { pattern, flags }
    }
}

#[derive(Clone, Copy)]
pub(super) struct CommandLocation<'a> {
    pub(super) template: &'a str,
    pub(super) command: &'a str,
}

impl<'a> CommandLocation<'a> {
    pub(super) const fn new(template: &'a str, command: &'a str) -> Self {
        Self { template, command }
    }

    pub(super) fn describe(self) -> String {
        format!("command '{}' in template '{}'", self.command, self.template)
    }
}
