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
    /// Active quoting delimiter, when inside a quoted run.
    quote: Option<char>,
    /// Whether the previous character escaped the current one.
    escaped: bool,
    /// Whether the current character sits at a shell word boundary.
    word_boundary: bool,
    /// Whether `&` directly followed a redirection operator.
    pending_redirection_ampersand: bool,
}

impl ShellScanState {
    /// Start a scan at a word boundary outside any quote.
    const fn new() -> Self {
        Self {
            quote: None,
            escaped: false,
            word_boundary: true,
            pending_redirection_ampersand: false,
        }
    }

    /// Consume an escaped character, leaving the escape state.
    const fn consume_escaped(&mut self) -> bool {
        if self.escaped {
            self.escaped = false;
            self.word_boundary = false;
            true
        } else {
            false
        }
    }

    /// Consume a character inside an active quote, or report none.
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

    /// Return whether `character` starts a comment at a word boundary.
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
