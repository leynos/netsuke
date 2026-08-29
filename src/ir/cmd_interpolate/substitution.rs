//! Hold the state required for one-pass recipe placeholder substitution.

use super::{CommandBindings, IrGenError, find_substitution, invalid_command_error};
use crate::ninja_gen::RecipeShell;

/// Track one-pass substitution while retaining the original error template.
///
/// This helper is private to `cmd_interpolate`: it groups immutable template
/// context with mutable output state so per-character processing avoids a wide
/// parameter list without re-scanning the template.
pub(super) struct SubstitutionTraversal<'template, 'bindings> {
    /// Preserve the source text for interpolation diagnostics.
    template: &'template str,
    /// Retain the single character buffer traversed by the pass.
    chars: &'template [char],
    /// Reuse shell-specific path substitutions prepared for the enclosing action.
    bindings: &'bindings CommandBindings,
    /// Accumulate substituted output without a second traversal.
    output: String,
    /// Record whether the current position is protected by backticks.
    in_backticks: bool,
}

impl<'template, 'bindings> SubstitutionTraversal<'template, 'bindings> {
    /// Initialise a traversal for one template and its placeholder bindings.
    pub(super) fn new(
        template: &'template str,
        chars: &'template [char],
        bindings: &'bindings CommandBindings,
    ) -> Self {
        Self {
            template,
            chars,
            bindings,
            output: String::with_capacity(template.len()),
            in_backticks: false,
        }
    }

    /// Append the substitution or source character beginning at `pos`.
    pub(super) fn append_substitution_at_position(
        &mut self,
        pos: usize,
    ) -> Result<usize, IrGenError> {
        let ch = *self
            .chars
            .get(pos)
            .ok_or_else(|| invalid_command_error(self.template.to_owned()))?;
        if self.bindings.shell != RecipeShell::PowerShell && ch == '`' {
            self.in_backticks ^= true;
            self.output.push(ch);
            return Ok(pos + 1);
        }

        let substitution =
            find_substitution(self.chars, pos, &self.bindings.ins, &self.bindings.outs);
        if self.in_backticks {
            return self.append_protected_character(pos, ch, substitution);
        }
        Ok(self.append_unprotected_character(pos, ch, substitution))
    }

    /// Append a character protected by backticks or reject its placeholder.
    fn append_protected_character(
        &mut self,
        pos: usize,
        ch: char,
        substitution: Option<(&str, usize)>,
    ) -> Result<usize, IrGenError> {
        if substitution.is_some() {
            return Err(invalid_command_error(self.template.to_owned()));
        }
        self.output.push(ch);
        Ok(pos + 1)
    }

    /// Append an unprotected replacement or the original character.
    fn append_unprotected_character(
        &mut self,
        pos: usize,
        ch: char,
        substitution: Option<(&str, usize)>,
    ) -> usize {
        let Some((replacement, skip)) = substitution else {
            self.output.push(ch);
            return pos + 1;
        };
        self.output.push_str(replacement);
        pos + skip
    }

    /// Finish the traversal and return the generated shell text.
    pub(super) fn finish(self) -> String {
        self.output
    }
}
