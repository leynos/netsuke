//! Brace and character-class validation for glob patterns.
use super::{GlobErrorContext, GlobErrorType, create_unmatched_brace_error};
use minijinja::Error;

struct BraceValidator {
    depth: i32,
    in_class: bool,
    last_open_pos: Option<usize>,
    escaped: bool,
}

impl BraceValidator {
    const fn new() -> Self {
        Self {
            depth: 0,
            in_class: false,
            last_open_pos: None,
            escaped: false,
        }
    }

    fn process_character(
        &mut self,
        ch: char,
        pos: usize,
        pattern: &str,
    ) -> std::result::Result<(), Error> {
        if self.handle_escape_sequence(ch) {
            return Ok(());
        }

        self.handle_character_class(ch);

        self.handle_braces(ch, pos, pattern)
    }

    #[expect(
        clippy::missing_const_for_fn,
        reason = "validator mutates runtime state; const adds no benefit"
    )]
    fn handle_escape_sequence(&mut self, ch: char) -> bool {
        if self.escaped {
            self.escaped = false;
            return true;
        }

        #[cfg(unix)]
        {
            if ch == '\\' {
                self.escaped = true;
                return true;
            }
        }
        #[cfg(not(unix))]
        {
            let _ = ch;
        }

        false
    }

    #[expect(
        clippy::missing_const_for_fn,
        reason = "validator mutates runtime state; const adds no benefit"
    )]
    fn handle_character_class(&mut self, ch: char) {
        match ch {
            '[' if !self.in_class => self.in_class = true,
            ']' if self.in_class => self.in_class = false,
            _ => {}
        }
    }

    fn handle_braces(
        &mut self,
        ch: char,
        pos: usize,
        pattern: &str,
    ) -> std::result::Result<(), Error> {
        if self.in_class {
            return Ok(());
        }

        match ch {
            '}' if self.depth == 0 => Err(create_unmatched_brace_error(&GlobErrorContext {
                pattern: pattern.to_owned(),
                error_char: ch,
                position: pos,
                error_type: GlobErrorType::UnmatchedBrace,
            })),
            '{' => {
                self.depth += 1;
                self.last_open_pos = Some(pos);
                Ok(())
            }
            '}' => {
                self.depth -= 1;
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn validate_final_state(&self, pattern: &str) -> std::result::Result<(), Error> {
        if self.depth != 0 {
            let pos = self.last_open_pos.unwrap_or(0);
            Err(create_unmatched_brace_error(&GlobErrorContext {
                pattern: pattern.to_owned(),
                error_char: '{',
                position: pos,
                error_type: GlobErrorType::UnmatchedBrace,
            }))
        } else {
            Ok(())
        }
    }
}

pub(super) fn validate_brace_matching(pattern: &str) -> std::result::Result<(), Error> {
    let mut validator = BraceValidator::new();

    for (i, ch) in pattern.char_indices() {
        validator.process_character(ch, i, pattern)?;
    }

    validator.validate_final_state(pattern)
}
