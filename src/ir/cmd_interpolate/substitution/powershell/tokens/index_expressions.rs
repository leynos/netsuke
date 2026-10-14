//! Recognize variable roots through balanced PowerShell index expressions.

use super::PowerShellSourceCharacter;

/// Describe the character before an index or an unescaped variable in that index.
pub(super) enum PowerShellExpressionEnd<'source> {
    /// Continue scanning a member or variable name from this position.
    Character(PowerShellSourceCharacter<'source>),
    /// An unescaped variable appeared inside the index expression.
    VariableInIndex,
}

impl<'source> PowerShellSourceCharacter<'source> {
    /// Find the character before an index or detect a variable within that index.
    pub(super) fn expression_end_before_member_name(
        self,
    ) -> Option<PowerShellExpressionEnd<'source>> {
        if self.value() == ']' {
            self.expression_before_index()
        } else {
            Some(PowerShellExpressionEnd::Character(self))
        }
    }

    /// Scan a balanced index and report the expression that precedes it.
    fn expression_before_index(self) -> Option<PowerShellExpressionEnd<'source>> {
        let mut index_depth = 0usize;
        let mut has_unescaped_variable = false;
        let mut cursor = self;
        loop {
            match cursor.value() {
                ']' => index_depth += 1,
                '[' if index_depth == 1 => {
                    return expression_at_index_start(cursor, has_unescaped_variable);
                }
                '[' if index_depth > 1 => index_depth -= 1,
                '$' if index_depth > 0 && !cursor.is_escaped_by_backtick() => {
                    has_unescaped_variable = true;
                }
                _ => {}
            }
            cursor = cursor.previous()?;
        }
    }
}

/// Return the expression root found before the opening index bracket.
fn expression_at_index_start(
    opening_bracket: PowerShellSourceCharacter<'_>,
    has_unescaped_variable: bool,
) -> Option<PowerShellExpressionEnd<'_>> {
    if has_unescaped_variable {
        Some(PowerShellExpressionEnd::VariableInIndex)
    } else {
        opening_bracket
            .previous()
            .map(PowerShellExpressionEnd::Character)
    }
}
