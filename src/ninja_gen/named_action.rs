//! Rendering of one IR action as a Ninja rule.
//!
//! The emitted rule text depends on both the action and the shell selected for
//! its legacy recipe, so the two travel together in [`NamedAction`]. Keeping the
//! rendering out of the parent module leaves the graph-level emission order in
//! `mod.rs` and the per-action text here.

use super::RecipeShell;
use super::command_list::command_list_shell_text;
use super::error::NinjaGenError;
use super::escape::{ShellText, escape_metadata_value};
use super::recipe_shell::escape_posix_script;
use crate::ast::{CommandIsolation, Recipe, StringOrList};
use crate::recipe_shell::BashExecutable;
use std::fmt::Write;

/// Wrapper struct to display a rule with its identifier.
pub(crate) struct NamedAction<'a> {
    /// The rule identifier bound to the action.
    pub(crate) id: &'a str,
    /// The IR action whose recipe and metadata are rendered.
    pub(crate) action: &'a crate::ir::Action,
    /// The explicit interpreter receiving this action's legacy recipe text.
    pub(crate) shell: RecipeShell,
}

impl NamedAction<'_> {
    /// Write the action's optional metadata bindings followed by a blank line.
    ///
    /// # Errors
    ///
    /// Returns [`NinjaGenError`] when a metadata binding cannot be escaped or
    /// the output writer fails.
    fn write_metadata<W: Write>(&self, f: &mut W) -> Result<(), NinjaGenError> {
        let description = escape_metadata_value(self.action.description.as_deref())?;
        let depfile = escape_metadata_value(self.action.depfile.as_deref())?;
        let deps_format = escape_metadata_value(self.action.deps_format.as_deref())?;
        let pool = escape_metadata_value(self.action.pool.as_deref())?;
        write_kv!(f, "description", &description);
        write_kv!(f, "depfile", &depfile);
        write_kv!(f, "deps", &deps_format);
        write_kv!(f, "pool", &pool);
        write_flag!(f, "restat", self.action.restat);
        writeln!(f)?;
        Ok(())
    }

    /// Panic in debug builds when `command` is not POSIX-shell parseable.
    pub(crate) fn assert_shell_command(command: &str) {
        // `shlex::split` approximates POSIX shell parsing; keep this debug-only
        // sanity guard to catch obviously malformed commands during development.
        debug_assert!(
            shlex::split(command).is_some(),
            "invalid command: {command}"
        );
    }

    /// Reject a recipe that references another rule.
    #[cold]
    #[expect(
        clippy::panic_in_result_fn,
        reason = "debug builds intentionally panic to expose rule recursion"
    )]
    #[expect(
        clippy::manual_assert,
        reason = "debug-only guard escalates to panic for visibility"
    )]
    fn reject_rule_recipe() -> Result<ShellText, NinjaGenError> {
        if cfg!(debug_assertions) {
            panic!("rules do not reference other rules");
        }
        Err(NinjaGenError::UnsafeNinjaValue)
    }

    /// Reject a command recipe that carries no entries.
    ///
    /// Deserialization rejects empty command recipes, so reaching here means an
    /// earlier stage constructed one directly. `Display::to_string` turns the
    /// returned error into a panic, so the fault still surfaces loudly without
    /// a hand-rolled debug-only panic.
    #[cold]
    const fn reject_empty_command_recipe() -> Result<ShellText, NinjaGenError> {
        Err(NinjaGenError::UnsafeNinjaValue)
    }

    /// Converts the action recipe into shell text before Ninja escaping.
    fn shell_text(&self) -> Result<ShellText, NinjaGenError> {
        let command = match &self.action.recipe {
            Recipe::Command {
                command: StringOrList::String(scalar_command),
                ..
            } => {
                if self.shell != RecipeShell::PowerShell {
                    Self::assert_shell_command(scalar_command);
                }
                ShellText::new(scalar_command.clone())
            }
            Recipe::Command {
                command: StringOrList::List(items),
                isolation,
            } => command_list_shell_text(self.id, items, *isolation, self.shell)?,
            Recipe::Command {
                command: StringOrList::Empty,
                ..
            } => return Self::reject_empty_command_recipe(),
            Recipe::Script { script } => self.script_shell_text(script),
            Recipe::Rule { .. } => return Self::reject_rule_recipe(),
        };
        Ok(command)
    }

    /// Wraps a multi-line script in a one-line shell command for Ninja.
    fn script_shell_text(&self, script: &str) -> ShellText {
        if self.shell == RecipeShell::PowerShell {
            return ShellText::new(script.to_owned());
        }
        // Ninja commands must be single-line. The wrapper reconstructs the
        // original script with `printf %b` piped into a fresh shell so expected
        // expansions still happen at execution time.
        //
        // Three parsers stand between the encoded text and the final
        // interpreter: the outer shell's double-quoted word, the inner shell's
        // single-quoted `%b` operand, and `printf %b` itself. `escape_posix_script`
        // encodes one layer per parser, composing from the innermost outwards.
        // It relies on this exact quoting structure; changing either the
        // wrapper or the layer order invalidates the contract.
        let escaped = escape_posix_script(script);
        let cmd = format!("/bin/sh -e -c \"printf %b '{escaped}' | /bin/sh -e\"");
        // Scripts are allowed to contain shell constructs such as heredocs and
        // comments that `shlex` cannot model, so only command recipes use the
        // debug parser guard.
        ShellText::new(cmd)
    }

    /// Writes this action's Ninja rule, escaping only the shell-text boundary.
    ///
    /// # Errors
    ///
    /// Returns [`NinjaGenError`] when the recipe, metadata, or output cannot be
    /// rendered safely.
    pub(crate) fn write_into<W: Write>(&self, output: &mut W) -> Result<(), NinjaGenError> {
        self.write_into_with_bash(output, None)
    }

    /// Write one action using the optional runner-selected Bash executable.
    ///
    /// # Errors
    ///
    /// Returns [`NinjaGenError`] when the recipe, metadata, or output cannot be
    /// rendered safely.
    pub(crate) fn write_into_with_bash<W: Write>(
        &self,
        output: &mut W,
        bash_executable: Option<&BashExecutable>,
    ) -> Result<(), NinjaGenError> {
        let shell_text = self.shell_text()?;
        let command = match (&self.action.recipe, self.shell) {
            (
                Recipe::Command {
                    command: StringOrList::List(_),
                    isolation: CommandIsolation::Isolated,
                },
                RecipeShell::PowerShell,
            ) => self.shell.isolated_command_list_value(&shell_text)?,
            _ => self
                .shell
                .command_value_with_bash(&shell_text, bash_executable)?,
        };
        writeln!(output, "rule {}", self.id)?;
        writeln!(output, "  command = {}", command.command())?;
        if let Some(content) = command.response_file_content() {
            writeln!(output, "  rspfile = $out.netsuke-{}.ps1", self.id)?;
            writeln!(output, "  rspfile_content = {content}")?;
        }
        self.write_metadata(output)
    }
}
