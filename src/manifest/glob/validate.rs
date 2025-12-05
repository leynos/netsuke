//! Brace and character-class validation for glob patterns.
use super::GlobPattern;
use super::errors::{GlobErrorContext, GlobErrorType, create_unmatched_brace_error};
use minijinja::Error;

/// Context for a character being processed by the validator.
#[derive(Debug, Clone, Copy)]
pub(super) struct CharContext {
    pub ch: char,
    pub position: usize,
}

/// Tracks brace depth and escape semantics while parsing a pattern.
#[derive(Debug, Clone)]
pub(super) struct BraceValidationState {
    pub depth: i32,
    pub in_class: bool,
    pub last_open_pos: Option<usize>,
}

/// Stateful brace validator that understands character classes and escapes.
#[derive(Debug)]
pub(super) struct BraceValidator {
    pub(super) state: BraceValidationState,
    pub(super) escaped: bool,
}

impl BraceValidator {
    pub(super) const ESCAPE_ACTIVE: bool = cfg!(unix);

    pub(super) const fn new() -> Self {
        Self {
            state: BraceValidationState {
                depth: 0,
                in_class: false,
                last_open_pos: None,
            },
            escaped: false,
        }
    }

    pub(super) fn process_character(
        &mut self,
        ch: char,
        pos: usize,
        pattern: &GlobPattern,
    ) -> std::result::Result<(), Error> {
        let context = CharContext { ch, position: pos };

        if self.handle_escape_sequence(context.ch) {
            return Ok(());
        }

        self.handle_character_class(context.ch);

        self.handle_braces(&context, pattern)
    }

    #[expect(
        clippy::missing_const_for_fn,
        reason = "escape handling depends on runtime state; const adds no value"
    )]
    pub(super) fn handle_escape_sequence(&mut self, ch: char) -> bool {
        if self.escaped {
            self.escaped = false;
            return true;
        }

        if ch == '\\' && Self::ESCAPE_ACTIVE {
            self.escaped = true;
            return true;
        }

        false
    }

    #[expect(
        clippy::missing_const_for_fn,
        reason = "validator mutates runtime state; const adds no benefit"
    )]
    pub(super) fn handle_character_class(&mut self, ch: char) {
        match ch {
            '[' if !self.state.in_class => self.state.in_class = true,
            ']' if self.state.in_class => self.state.in_class = false,
            _ => {}
        }
    }

    pub(super) fn handle_braces(
        &mut self,
        context: &CharContext,
        pattern: &GlobPattern,
    ) -> std::result::Result<(), Error> {
        if self.state.in_class {
            return Ok(());
        }

        match context.ch {
            '}' if self.state.depth == 0 => Err(create_unmatched_brace_error(&GlobErrorContext {
                pattern: pattern.raw.clone(),
                error_char: context.ch,
                position: context.position,
                error_type: GlobErrorType::UnmatchedBrace,
            })),
            '{' => {
                self.state.depth += 1;
                self.state.last_open_pos = Some(context.position);
                Ok(())
            }
            '}' => {
                self.state.depth -= 1;
                Ok(())
            }
            _ => Ok(()),
        }
    }

    pub(super) fn validate_final_state(
        &self,
        pattern: &GlobPattern,
    ) -> std::result::Result<(), Error> {
        if self.state.depth != 0 {
            let pos = self.state.last_open_pos.unwrap_or(0);
            Err(create_unmatched_brace_error(&GlobErrorContext {
                pattern: pattern.raw.clone(),
                error_char: '{',
                position: pos,
                error_type: GlobErrorType::UnmatchedBrace,
            }))
        } else {
            Ok(())
        }
    }
}

pub(super) fn validate_brace_matching(pattern: &GlobPattern) -> std::result::Result<(), Error> {
    let mut validator = BraceValidator::new();

    for (i, ch) in pattern.raw.char_indices() {
        validator.process_character(ch, i, pattern)?;
    }

    validator.validate_final_state(pattern)
}
