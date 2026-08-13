//! Shell-safe rendering for ordered Ninja command-list entries.

use sha2::{Digest, Sha256};

use crate::hex::to_lower_hex;

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

/// One rendered shell command-list entry.
#[derive(Clone, Copy)]
pub(super) struct CommandListEntry<'a>(pub(super) &'a str);

/// An internal action identifier before it is converted to a safe fingerprint.
#[derive(Clone, Copy)]
pub(super) struct ActionId<'a>(pub(super) &'a str);

/// One shell word parsed from a command-list entry.
#[derive(Clone, Copy)]
struct ShellWord<'a>(&'a str);

/// The shell-word sequence parsed from one command-list entry.
struct ShellWords(Vec<String>);

/// Return the unsupported boundary, if any, for one command-list entry.
pub(super) fn command_list_entry_error(
    command: CommandListEntry<'_>,
) -> Option<CommandListEntryError> {
    if background_operator_count(command) > 1 {
        Some(CommandListEntryError::MultipleBackgroundJobs)
    } else if exec_boundary(command) == ExecBoundary::Unsupported {
        Some(CommandListEntryError::UnsupportedExec)
    } else {
        None
    }
}

/// Render one entry so it fails atomically without exposing command content.
pub(super) fn command_list_entry(
    command: CommandListEntry<'_>,
    action_id: ActionId<'_>,
    entry_index: usize,
) -> String {
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
fn command_evaluator(command: CommandListEntry<'_>) -> (String, &'static str) {
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
fn exec_boundary(command: CommandListEntry<'_>) -> ExecBoundary {
    ShellWords::parse(command).map_or(ExecBoundary::None, |words| words.exec_boundary())
}

impl ShellWords {
    /// Parse the shell words that make up one command-list entry.
    fn parse(command: CommandListEntry<'_>) -> Option<Self> {
        shlex::split(command.0).map(Self)
    }

    /// Classify `exec` only when it begins a simple command after assignments.
    fn exec_boundary(&self) -> ExecBoundary {
        let Some(first_non_assignment) = self.first_non_assignment() else {
            return ExecBoundary::None;
        };
        if first_non_assignment.is_exec() {
            ExecBoundary::Direct
        } else if first_non_assignment.is_exec_wrapper() && self.contains_exec() {
            ExecBoundary::Unsupported
        } else {
            ExecBoundary::None
        }
    }

    /// Return the first word that is not a leading assignment.
    fn first_non_assignment(&self) -> Option<ShellWord<'_>> {
        self.0
            .iter()
            .map(|word| ShellWord(word))
            .find(|word| !word.is_assignment())
    }

    /// Whether the parsed entry has an `exec` word anywhere in its structure.
    fn contains_exec(&self) -> bool {
        self.0.iter().any(|word| ShellWord(word).is_exec())
    }
}

impl ShellWord<'_> {
    /// Whether this word is `exec`.
    fn is_exec(self) -> bool {
        self.0 == "exec"
    }

    /// Whether this word can invoke `exec` outside the direct supported boundary.
    fn is_exec_wrapper(self) -> bool {
        matches!(self.0, "if" | "command")
    }

    /// Whether this word is a valid POSIX shell assignment word.
    fn is_assignment(self) -> bool {
        let Some((name, _)) = self.0.split_once('=') else {
            return false;
        };
        let mut chars = name.chars();
        chars
            .next()
            .is_some_and(|first| first == '_' || first.is_ascii_alphabetic())
            && chars.all(|character| character == '_' || character.is_ascii_alphanumeric())
    }
}

/// Count unquoted background operators without mistaking `&&` for one.
fn background_operator_count(command: CommandListEntry<'_>) -> usize {
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
fn action_identity(action_id: ActionId<'_>) -> String {
    to_lower_hex(&Sha256::digest(action_id.0.as_bytes()))
}

/// Quote `value` as one literal POSIX shell argument.
///
/// The command-list renderer passes each entry to `eval` so an inline comment
/// or trailing control operator cannot consume the brace-group terminator.
fn shell_single_quote(command: CommandListEntry<'_>) -> String {
    let escaped = command.0.replace('\'', r"'\''");
    format!("'{escaped}'")
}

#[cfg(test)]
mod tests {
    //! Unit tests for private command-list shell boundaries.

    use super::{
        ActionId, CommandListEntry, ExecBoundary, action_identity, background_operator_count,
        command_list_entry, exec_boundary, shell_single_quote,
    };

    #[test]
    fn classifies_direct_and_unsupported_exec_entries() {
        assert_eq!(
            exec_boundary(CommandListEntry("FOO=1 exec false")),
            ExecBoundary::Direct
        );
        assert_eq!(
            exec_boundary(CommandListEntry("if true; then exec false; fi")),
            ExecBoundary::Unsupported
        );
    }

    #[test]
    fn counts_only_unquoted_background_operators_before_comments() {
        assert_eq!(background_operator_count(CommandListEntry("sleep 1 &")), 1);
        assert_eq!(
            background_operator_count(CommandListEntry("sleep 1 & true &")),
            2
        );
        assert_eq!(
            background_operator_count(CommandListEntry("echo '&' # &")),
            0
        );
    }

    #[test]
    fn shell_quotes_each_entry_as_one_literal_argument() {
        assert_eq!(
            shell_single_quote(CommandListEntry("echo 'quoted'")),
            "'echo '\\''quoted'\\'''"
        );
    }

    #[test]
    fn rendered_entry_uses_a_hashed_action_identity_and_one_based_index() {
        let rendered = command_list_entry(CommandListEntry("false"), ActionId("example"), 3);
        let expected_identity = "50d858e0985ecc7f60418aaf0cc5ab587f42c2570a884095a9e8ccacd0f6545c";
        assert_eq!(action_identity(ActionId("example")), expected_identity);
        assert!(
            rendered.contains(&format!(
                "netsuke command-list failure: action {expected_identity}, entry 3"
            )),
            "entry must use the hashed identity and its one-based index: {rendered}"
        );
    }
}
