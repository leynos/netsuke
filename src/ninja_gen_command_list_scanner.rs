//! Lexical detection of command-list background operators.

use super::CommandListEntry;

/// Count unquoted background operators without mistaking `&&` for one.
pub(super) fn background_operator_count(command: CommandListEntry<'_>) -> usize {
    let mut state = ShellScanState::new();
    let mut count = 0;
    let mut characters = command.0.chars().peekable();
    while let Some(character) = characters.next() {
        if state.consume_escaped() {
            continue;
        }
        if state.consume_quoted(character) {
            continue;
        }
        if state.starts_comment(character) {
            break;
        }
        count += state.count_unquoted_background_operator(character, &mut characters);
    }
    count
}

/// Minimal shell scanner state used only to detect background operators.
struct ShellScanState {
    quote: Option<char>,
    escaped: bool,
    word_boundary: bool,
    pending_redirection_ampersand: bool,
}

impl ShellScanState {
    const fn new() -> Self {
        Self {
            quote: None,
            escaped: false,
            word_boundary: true,
            pending_redirection_ampersand: false,
        }
    }

    const fn consume_escaped(&mut self) -> bool {
        if self.escaped {
            self.escaped = false;
            self.word_boundary = false;
            true
        } else {
            false
        }
    }

    const fn consume_quoted(&mut self, character: char) -> bool {
        let Some(delimiter) = self.quote else {
            return false;
        };
        if character == delimiter {
            self.quote = None;
        } else if character == '\\' && delimiter == '"' {
            self.escaped = true;
        }
        self.word_boundary = false;
        true
    }

    const fn starts_comment(&self, character: char) -> bool {
        character == '#' && self.word_boundary
    }

    /// Count one unquoted background operator and advance this scanner state.
    fn count_unquoted_background_operator(
        &mut self,
        character: char,
        characters: &mut std::iter::Peekable<std::str::Chars<'_>>,
    ) -> usize {
        match character {
            '\\' => {
                self.escaped = true;
                0
            }
            '\'' | '"' => {
                self.quote = Some(character);
                self.word_boundary = false;
                0
            }
            '&' if characters.peek() == Some(&'&') => {
                characters.next();
                self.pending_redirection_ampersand = false;
                self.word_boundary = true;
                0
            }
            '&' if self.pending_redirection_ampersand => {
                self.pending_redirection_ampersand = false;
                self.word_boundary = false;
                0
            }
            '&' => {
                self.pending_redirection_ampersand = false;
                self.word_boundary = true;
                1
            }
            '<' | '>' => {
                self.pending_redirection_ampersand = true;
                self.word_boundary = true;
                0
            }
            ';' | '|' | '(' | ')' => {
                self.pending_redirection_ampersand = false;
                self.word_boundary = true;
                0
            }
            whitespace if whitespace.is_whitespace() => {
                self.pending_redirection_ampersand = false;
                self.word_boundary = true;
                0
            }
            _ => {
                self.pending_redirection_ampersand = false;
                self.word_boundary = false;
                0
            }
        }
    }
}
