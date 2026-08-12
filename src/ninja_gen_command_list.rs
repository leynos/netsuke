//! Shell-safe rendering for ordered Ninja command-list entries.

use sha2::{Digest, Sha256};

/// Prefix used to carry bounded list-entry failure attribution through Ninja.
pub(crate) const COMMAND_LIST_FAILURE_PREFIX: &str = "netsuke command-list failure: action ";

/// A command-list entry cannot preserve the ordered execution contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommandListEntryError {
    /// An entry starts more than one background job.
    MultipleBackgroundJobs,
    /// An `exec` occurs in a shell structure the list wrapper cannot supervise.
    UnsupportedExec,
}

/// Return the unsupported boundary, if any, for one command-list entry.
pub(crate) fn command_list_entry_error(command: &str) -> Option<CommandListEntryError> {
    if background_operator_count(command) > 1 {
        Some(CommandListEntryError::MultipleBackgroundJobs)
    } else if exec_boundary(command) == ExecBoundary::Unsupported {
        Some(CommandListEntryError::UnsupportedExec)
    } else {
        None
    }
}

/// Render one entry so it fails atomically without exposing command content.
pub(crate) fn command_list_entry(command: &str, action_id: &str, entry_index: usize) -> String {
    let identity = action_identity(action_id);
    let context = format!("{COMMAND_LIST_FAILURE_PREFIX}{identity}, entry {entry_index}");
    let (evaluator, exec_succeeded) = command_evaluator(command);
    format!(
        concat!(
            "{{ _netsuke_background_before=$${{!:-}}; _netsuke_exec_succeeded=0; ",
            "trap '_netsuke_command_status=$$?; printf \"%s\\n\" \"{}\" >&2; ",
            "trap - EXIT; exit \"$$_netsuke_command_status\"' EXIT; ",
            "if {}; then _netsuke_command_status=0;{} else _netsuke_command_status=$$?; fi; ",
            "_netsuke_background_after=$${{!:-}}; ",
            "if [ -n \"$$_netsuke_background_after\" ] && ",
            "[ \"$$_netsuke_background_after\" != \"$$_netsuke_background_before\" ]; then ",
            "if wait \"$$_netsuke_background_after\"; then :; ",
            "else _netsuke_background_status=$$?; ",
            "if [ \"$$_netsuke_command_status\" -eq 0 ]; then ",
            "_netsuke_command_status=$$_netsuke_background_status; fi; fi; fi; ",
            "if [ \"$$_netsuke_command_status\" -eq 0 ]; then trap - EXIT; ",
            "if [ \"$$_netsuke_exec_succeeded\" -eq 1 ]; then exit 0; else :; fi; ",
            "else trap - EXIT; printf '%s\\n' '{}' >&2; ",
            "exit \"$$_netsuke_command_status\"; fi; }}"
        ),
        context, evaluator, exec_succeeded, context,
    )
}

/// Evaluate a supported direct `exec` in a retaining subshell.
///
/// A direct `exec` replaces its subshell, allowing the brace group to observe
/// its status. A successful replacement then exits the command chain without
/// emitting a marker, as an in-shell `exec` would.
fn command_evaluator(command: &str) -> (String, &'static str) {
    let quoted = shell_single_quote(command);
    if exec_boundary(command) == ExecBoundary::Direct {
        (format!("(eval {quoted})"), " _netsuke_exec_succeeded=1;")
    } else {
        (format!("eval {quoted}"), "")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExecBoundary {
    None,
    Direct,
    Unsupported,
}

/// Classify `exec` only when it begins a simple command after assignments.
fn exec_boundary(command: &str) -> ExecBoundary {
    let Some(words) = shlex::split(command) else {
        return ExecBoundary::None;
    };
    let Some(first_non_assignment) = words.iter().find(|word| !is_assignment(word)) else {
        return ExecBoundary::None;
    };
    if first_non_assignment == "exec" {
        ExecBoundary::Direct
    } else if is_unsupported_exec_structure(first_non_assignment, &words) {
        ExecBoundary::Unsupported
    } else {
        ExecBoundary::None
    }
}

/// Whether a shell structure can replace the wrapper before it reports failure.
fn is_unsupported_exec_structure(first_word: &str, words: &[String]) -> bool {
    is_exec_wrapper(first_word) && words.iter().any(|word| word == "exec")
}

/// Whether `word` can invoke `exec` outside the direct supported boundary.
fn is_exec_wrapper(word: &str) -> bool {
    matches!(word, "if" | "command")
}

/// Whether `word` is a valid POSIX shell assignment word.
fn is_assignment(word: &str) -> bool {
    let Some((name, _)) = word.split_once('=') else {
        return false;
    };
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|first| first == '_' || first.is_ascii_alphabetic())
        && chars.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

/// Count unquoted background operators without mistaking `&&` for one.
fn background_operator_count(command: &str) -> usize {
    let mut state = ShellScanState::new();
    let mut count = 0;
    let mut characters = command.chars().peekable();
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
}

impl ShellScanState {
    const fn new() -> Self {
        Self {
            quote: None,
            escaped: false,
            word_boundary: true,
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
                self.word_boundary = true;
                0
            }
            '&' => {
                self.word_boundary = true;
                1
            }
            ';' | '|' | '(' | ')' => {
                self.word_boundary = true;
                0
            }
            whitespace if whitespace.is_whitespace() => {
                self.word_boundary = true;
                0
            }
            _ => {
                self.word_boundary = false;
                0
            }
        }
    }
}

/// Return a fixed-width fingerprint for an action identifier.
///
/// IR-generated identifiers are already hashes, but hashing again prevents a
/// programmatically supplied identifier from disclosing arbitrary content.
fn action_identity(action_id: &str) -> String {
    let digest = Sha256::digest(action_id.as_bytes());
    let mut identity = String::with_capacity(digest.len() * 2);
    for byte in digest {
        identity.push(hex_digit(byte >> 4));
        identity.push(hex_digit(byte & 0x0f));
    }
    identity
}

const fn hex_digit(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        _ => (b'a' + (nibble - 10)) as char,
    }
}

/// Quote `value` as one literal POSIX shell argument.
///
/// The command-list renderer passes each entry to `eval` so an inline comment
/// or trailing control operator cannot consume the brace-group terminator.
fn shell_single_quote(value: &str) -> String {
    let escaped = value.replace('\'', r"'\''");
    format!("'{escaped}'")
}
