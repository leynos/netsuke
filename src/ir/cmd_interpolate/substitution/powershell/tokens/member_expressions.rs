//! Recognize PowerShell variable roots through member and index chains.

use super::{PowerShellSourceCharacter, index_expressions::PowerShellExpressionEnd};

impl PowerShellSourceCharacter<'_> {
    /// Recognize an unescaped variable through chained member and index accessors.
    pub(super) fn ends_with_unescaped_variable_expression(self) -> bool {
        let mut expression_end = self;
        loop {
            if expression_end.has_unescaped_variable_expression_root() {
                return true;
            }
            let Some(previous_expression_end) = expression_end.previous_member_expression() else {
                return false;
            };
            expression_end = previous_expression_end;
        }
    }

    /// Report whether this expression ends in an unescaped variable root.
    fn has_unescaped_variable_expression_root(self) -> bool {
        match self.expression_end_before_member_name() {
            Some(PowerShellExpressionEnd::VariableInIndex) => true,
            Some(PowerShellExpressionEnd::Character(name_end)) => {
                name_end.is_variable_name_character()
                    && name_end
                        .variable_name_start()
                        .is_unescaped_variable_name_start()
            }
            None => false,
        }
    }

    /// Find the expression immediately before this member name.
    fn previous_member_expression(self) -> Option<Self> {
        let PowerShellExpressionEnd::Character(name_end) =
            self.expression_end_before_member_name()?
        else {
            return None;
        };
        if !name_end.is_variable_name_character() {
            return None;
        }
        let name_start = name_end.variable_name_start();
        if name_start.is_unescaped_variable_name_start() {
            return None;
        }
        name_start
            .previous()
            .filter(|previous| previous.value() == '.')?
            .previous()
    }

    /// Find the first character of the variable name ending at this position.
    pub(super) fn variable_name_start(self) -> Self {
        let mut name_start = self;
        while let Some(previous) = name_start.previous() {
            if !previous.is_variable_name_character() {
                break;
            }
            name_start = previous;
        }
        name_start
    }
}
