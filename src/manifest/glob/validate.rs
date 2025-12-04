//! Brace and character-class validation for glob patterns.
use super::errors::{GlobErrorContext, GlobErrorType, create_unmatched_brace_error};
use super::GlobPattern;
use minijinja::Error;

/// Context for a character being processed by the validator.
#[derive(Debug, Clone, Copy)]
pub(super) struct CharContext {
    pub ch: char,
    pub position: usize,
    pub in_class: bool,
    pub escaped: bool,
}

/// Tracks brace depth and escape semantics while parsing a pattern.
#[derive(Debug, Clone)]
pub(super) struct BraceValidationState {
    pub depth: i32,
    pub in_class: bool,
    pub last_open_pos: Option<usize>,
    pub escape_active: bool,
}

/// Stateful brace validator that understands character classes and escapes.
#[derive(Debug)]
pub(super) struct BraceValidator {
    pub(super) state: BraceValidationState,
    pub(super) escaped: bool,
}

impl BraceValidator {
    pub(super) const fn new() -> Self {
        Self {
            state: BraceValidationState {
                depth: 0,
                in_class: false,
                last_open_pos: None,
                escape_active: cfg!(unix),
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
        let context = CharContext {
            ch,
            position: pos,
            in_class: self.state.in_class,
            escaped: self.escaped,
        };

        if let Some(result) = self.handle_escape_sequence(&context) {
            return result;
        }

        self.handle_character_class(&context);

        self.handle_braces(&context, pattern)
    }

    pub(super) fn handle_escape_sequence(
        &mut self,
        context: &CharContext,
    ) -> Option<std::result::Result<(), Error>> {
        if context.escaped {
            self.escaped = false;
            return Some(Ok(()));
        }

        if context.ch == char::from(0x5c) && self.state.escape_active {
            self.escaped = true;
            return Some(Ok(()));
        }

        None
    }

    #[expect(
        clippy::missing_const_for_fn,
        reason = "validator mutates runtime state; const adds no benefit"
    )]
    pub(super) fn handle_character_class(&mut self, context: &CharContext) {
        match context.ch {
            '[' if !context.in_class => self.state.in_class = true,
            ']' if context.in_class => self.state.in_class = false,
            _ => {}
        }
    }

    pub(super) fn handle_braces(
        &mut self,
        context: &CharContext,
        pattern: &GlobPattern,
    ) -> std::result::Result<(), Error> {
        if context.in_class {
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
