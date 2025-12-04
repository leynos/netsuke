//! Brace and character-class validation for glob patterns.
use super::errors::{GlobErrorContext, GlobErrorType, create_unmatched_brace_error};
use minijinja::Error;

struct ValidationState {
    depth: i32,
    in_class: bool,
    last_open_pos: Option<usize>,
    #[cfg(unix)]
    escaped: bool,
}

impl ValidationState {
    const fn new() -> Self {
        Self {
            depth: 0,
            in_class: false,
            last_open_pos: None,
            #[cfg(unix)]
            escaped: false,
        }
    }

    #[cfg(unix)]
    #[expect(
        clippy::missing_const_for_fn,
        reason = "mutating runtime state; const would not improve clarity"
    )]
    fn process_escape(&mut self, ch: char) -> bool {
        if self.escaped {
            self.escaped = false;
            return true;
        }
        if ch == '\\' {
            self.escaped = true;
            return true;
        }
        false
    }

    #[cfg(not(unix))]
    fn process_escape(&mut self, _ch: char) -> bool {
        false
    }

    #[expect(
        clippy::missing_const_for_fn,
        reason = "mutating runtime state; const would not improve clarity"
    )]
    fn process_character_class(&mut self, ch: char) -> bool {
        match (self.in_class, ch) {
            (true, ']') => {
                self.in_class = false;
                true
            }
            (true, _) => true,
            (false, '[') => {
                self.in_class = true;
                true
            }
            _ => false,
        }
    }

    fn process_brace(
        &mut self,
        ch: char,
        pos: usize,
        pattern: &str,
    ) -> std::result::Result<(), Error> {
        match ch {
            '{' => {
                self.depth += 1;
                self.last_open_pos = Some(pos);
                Ok(())
            }
            '}' if self.depth == 0 => Err(create_unmatched_brace_error(&GlobErrorContext {
                pattern: pattern.to_owned(),
                error_char: ch,
                position: pos,
                error_type: GlobErrorType::UnmatchedBrace,
            })),
            '}' => {
                self.depth -= 1;
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn validate_final_state(&self, pattern: &str) -> std::result::Result<(), Error> {
        if self.depth == 0 {
            return Ok(());
        }
        let pos = self.last_open_pos.unwrap_or(0);
        Err(create_unmatched_brace_error(&GlobErrorContext {
            pattern: pattern.to_owned(),
            error_char: '{',
            position: pos,
            error_type: GlobErrorType::UnmatchedBrace,
        }))
    }
}

pub(super) fn validate_brace_matching(pattern: &str) -> std::result::Result<(), Error> {
    let mut state = ValidationState::new();

    for (pos, ch) in pattern.char_indices() {
        if state.process_escape(ch) {
            continue;
        }
        if state.process_character_class(ch) {
            continue;
        }
        state.process_brace(ch, pos, pattern)?;
    }

    state.validate_final_state(pattern)
}
