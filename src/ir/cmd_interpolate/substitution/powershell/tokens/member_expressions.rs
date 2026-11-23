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

impl PowerShellSourceCharacter<'_> {
    /// Report whether a hash follows a static member access such as `[Type]::Name`.
    ///
    /// PowerShell reads `[string]::Empty#` as a comment, but a bare word such as
    /// `foo::bar#` stays one argument token.
    pub(super) fn hash_follows_static_member_access(self) -> bool {
        self.static_member_operand_end()
            .is_some_and(Self::is_static_access_operand)
    }

    /// Find the operand character before a `::` that follows a member name.
    fn static_member_operand_end(self) -> Option<Self> {
        let member_start = self.member_name_start()?;
        let second_colon = member_start.previous()?;
        if second_colon.value() != ':' {
            return None;
        }
        let first_colon = second_colon.previous()?;
        if first_colon.value() != ':' {
            return None;
        }
        first_colon.previous()
    }

    /// Move to the first character of the member name ending just before this hash.
    fn member_name_start(self) -> Option<Self> {
        let mut character = self.previous()?;
        if !character.is_member_name_character() {
            return None;
        }
        while let Some(previous) = character.previous() {
            if !previous.is_member_name_character() {
                break;
            }
            character = previous;
        }
        Some(character)
    }

    /// Report whether a character can end an operand of a static member access.
    fn is_static_access_operand(self) -> bool {
        match self.value() {
            ']' | ')' => true,
            _ => self.is_variable_operand(),
        }
    }

    /// Report whether this character ends a variable used before `::`.
    fn is_variable_operand(self) -> bool {
        if !self.is_member_name_character() {
            return false;
        }
        let mut character = self;
        while let Some(previous) = character.previous() {
            if !previous.is_member_name_character() {
                break;
            }
            character = previous;
        }
        character
            .previous()
            .is_some_and(|dollar| dollar.value() == '$' && !dollar.is_escaped_by_backtick())
    }

    /// Report whether this character can occur in a member or type name.
    ///
    /// Unlike [`Self::is_variable_name_character`], this excludes the colon so a
    /// static-access `::` is not mistaken for part of the member name.
    fn is_member_name_character(self) -> bool {
        let character = self.value();
        character.is_alphanumeric() || matches!(character, '_' | '?' | '^')
    }
}
