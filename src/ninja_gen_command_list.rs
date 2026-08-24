//! Shell-safe rendering for ordered Ninja command-list entries.

use sha2::{Digest, Sha256};

use crate::hex::to_lower_hex;

#[path = "ninja_gen_command_list_scanner.rs"]
mod scanner;

use scanner::background_operator_count;

/// Prefix used to carry bounded list-entry failure attribution through Ninja.
pub(crate) const COMMAND_LIST_FAILURE_PREFIX: &str = "netsuke command-list failure: action ";

/// A command-list entry cannot preserve the ordered execution contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommandListEntryError {
    /// An entry starts multiple background jobs.
    MultipleBackgroundJobs,
    /// An `exec` occurs in a shell structure the list wrapper cannot supervise.
    UnsupportedExec,
    /// An `eval` payload cannot be analysed for attributable background jobs.
    UnanalyzableEval,
    /// An entry cannot be represented safely in one Ninja command binding.
    NinjaControlCharacter,
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

/// Signals that static inspection cannot account for an `eval` payload.
#[derive(Clone, Copy)]
struct UnanalyzableEval;

/// Return the unsupported boundary, if any, for one command-list entry.
pub(super) fn command_list_entry_error(
    command: CommandListEntry<'_>,
) -> Option<CommandListEntryError> {
    // Manifest validation normally rejects syntax that `shlex` cannot parse,
    // but programmatic IR can bypass it. Preserve the direct scan on parse
    // failure: it can still prove multiple direct background jobs, while
    // nested `eval` and `exec` analysis remain unavailable.
    let direct_background_jobs = background_operator_count(command);
    if command.has_ninja_control_character() {
        Some(CommandListEntryError::NinjaControlCharacter)
    } else if let Some(words) = ShellWords::parse(command) {
        let Ok(nested_jobs) = words.background_job_count() else {
            return Some(CommandListEntryError::UnanalyzableEval);
        };
        if direct_background_jobs
            .checked_add(nested_jobs)
            .is_none_or(|background_jobs| background_jobs > 1)
        {
            Some(CommandListEntryError::MultipleBackgroundJobs)
        } else if exec_boundary(command) == ExecBoundary::Unsupported {
            Some(CommandListEntryError::UnsupportedExec)
        } else {
            None
        }
    } else if direct_background_jobs > 1 {
        Some(CommandListEntryError::MultipleBackgroundJobs)
    } else if exec_boundary(command) == ExecBoundary::Unsupported {
        Some(CommandListEntryError::UnsupportedExec)
    } else {
        None
    }
}

/// Render one entry so it fails atomically without exposing command content.
///
/// Brace groups deliberately run in the current shell, so the EXIT trap must
/// be cleared on both the success and failure paths before leaving the group.
/// `$!` records only the latest background PID: validation rejects entries
/// with multiple or dynamically generated background jobs before rendering.
/// The `_netsuke_*` variables are reserved because user assignments to them
/// can corrupt status propagation or failure attribution. Finally, a direct
/// successful `exec` sets `_netsuke_exec_succeeded=1` and exits with status
/// zero, preserving process replacement by preventing later entries from
/// running.
pub(super) fn command_list_entry(
    command: CommandListEntry<'_>,
    action_id: ActionId<'_>,
    entry_index: usize,
) -> String {
    let identity = action_identity(action_id);
    let context = format!("{COMMAND_LIST_FAILURE_PREFIX}{identity}, entry {entry_index}");
    let evaluator = command_evaluator(command);
    format!(
        concat!(
            "{{ _netsuke_background_before=${{!:-}}; _netsuke_exec_succeeded=0; ",
            "trap '_netsuke_command_status=$?; printf \"%s\\n\" \"{}\" >&2; ",
            "trap - EXIT; exit \"$_netsuke_command_status\"' EXIT; ",
            "if {}; then _netsuke_command_status=0;{} else _netsuke_command_status=$?; fi; ",
            "_netsuke_background_after=${{!:-}}; ",
            "if [ -n \"$_netsuke_background_after\" ] && ",
            "[ \"$_netsuke_background_after\" != \"$_netsuke_background_before\" ]; then ",
            "if wait \"$_netsuke_background_after\"; then :; ",
            "else _netsuke_background_status=$?; ",
            "if [ \"$_netsuke_command_status\" -eq 0 ]; then ",
            "_netsuke_command_status=$_netsuke_background_status; fi; fi; fi; ",
            "if [ \"$_netsuke_command_status\" -eq 0 ]; then trap - EXIT; ",
            "if [ \"$_netsuke_exec_succeeded\" -eq 1 ]; then exit 0; else :; fi; ",
            "else trap - EXIT; printf '%s\\n' '{}' >&2; ",
            "exit \"$_netsuke_command_status\"; fi; }}"
        ),
        context, evaluator.shell_expression, evaluator.exec_success_fragment, context,
    )
}

/// Evaluate a supported direct `exec` in a retaining subshell.
///
/// A direct `exec` replaces its subshell, allowing the brace group to observe
/// its status. A successful replacement then exits the command chain without
/// emitting a marker, as an in-shell `exec` would.
struct CommandEvaluator {
    /// Shell expression that evaluates one list entry.
    shell_expression: String,
    /// Fragment that records a successful retaining-subshell `exec`.
    exec_success_fragment: &'static str,
}

/// Build the evaluator for one entry, retaining subprocess replacement on a
/// direct `exec`.
fn command_evaluator(command: CommandListEntry<'_>) -> CommandEvaluator {
    let quoted = shell_single_quote(command);
    if exec_boundary(command) == ExecBoundary::Direct {
        CommandEvaluator {
            shell_expression: format!("(eval {quoted})"),
            exec_success_fragment: " _netsuke_exec_succeeded=1;",
        }
    } else {
        CommandEvaluator {
            shell_expression: format!("eval {quoted}"),
            exec_success_fragment: "",
        }
    }
}

/// How an `exec` keyword sits within a command-list entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExecBoundary {
    /// The entry does not contain `exec` in a shell command position.
    None,
    /// `exec` is the entry's first simple command after leading assignments.
    Direct,
    /// `exec` occurs in a later or wrapped command position the wrapper cannot supervise.
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
        let direct_index = self.first_non_assignment_index();
        self.0
            .iter()
            .map(|word| ShellWord(word))
            .enumerate()
            .find_map(|(index, word)| self.exec_boundary_at(index, word, direct_index))
            .unwrap_or(ExecBoundary::None)
    }

    /// Classify `exec` at `index`: `Direct` when it is the first simple command,
    /// `Unsupported` when a wrapper position hides it, else `None`.
    fn exec_boundary_at(
        &self,
        index: usize,
        word: ShellWord<'_>,
        direct_index: Option<usize>,
    ) -> Option<ExecBoundary> {
        if !word.is_exec() {
            return None;
        }
        if Some(index) == direct_index {
            return Some(ExecBoundary::Direct);
        }
        (self.is_command_word(index) || self.is_exec_wrapper(index))
            .then_some(ExecBoundary::Unsupported)
    }

    /// Return the index of the first word that is not a leading assignment.
    fn first_non_assignment_index(&self) -> Option<usize> {
        self.0
            .iter()
            .position(|word| !ShellWord(word).is_assignment())
    }

    /// Return whether this word begins a simple shell command.
    fn is_command_word(&self, index: usize) -> bool {
        let Some(words_before) = self.0.get(..index) else {
            return false;
        };
        let preceding_word = words_before
            .iter()
            .rev()
            .find(|word| !ShellWord(word).is_assignment());
        preceding_word.is_none_or(|word| ShellWord(word).ends_command())
    }

    /// Return whether `command exec` wraps a process-replacing built-in.
    fn is_exec_wrapper(&self, index: usize) -> bool {
        let is_wrapper = index
            .checked_sub(1)
            .and_then(|previous_index| self.0.get(previous_index))
            .is_some_and(|word| ShellWord(word).is_exec_wrapper());
        is_wrapper
            && index
                .checked_sub(1)
                .is_some_and(|previous| self.is_command_word(previous))
    }

    /// Count background jobs launched by the entry, including static nested
    /// `eval` payloads. An error means an `eval` payload cannot be analysed
    /// without potentially hiding background jobs.
    fn background_job_count(&self) -> Result<usize, UnanalyzableEval> {
        self.background_job_count_at_depth(0)
    }

    /// Count background jobs reachable at `depth`, recursing into static `eval` payloads.
    ///
    /// # Errors
    ///
    /// Returns [`UnanalyzableEval`] when nested evaluation cannot be analysed
    /// or when the accumulated job count overflows.
    fn background_job_count_at_depth(&self, depth: usize) -> Result<usize, UnanalyzableEval> {
        self.0
            .iter()
            .map(|word| ShellWord(word))
            .enumerate()
            .filter(|(index, word)| word.is_eval() && self.is_command_word(*index))
            .try_fold(0_usize, |count, (index, _)| {
                count
                    .checked_add(self.background_jobs_from_eval(index, depth)?)
                    .ok_or(UnanalyzableEval)
            })
    }

    /// Count background jobs in the static payload of the `eval` at `index`.
    ///
    /// # Errors
    ///
    /// Returns [`UnanalyzableEval`] when the nesting depth limit is reached,
    /// the payload contains dynamic expansion, shell parsing fails, or the
    /// resulting job count overflows.
    fn background_jobs_from_eval(
        &self,
        index: usize,
        depth: usize,
    ) -> Result<usize, UnanalyzableEval> {
        const MAX_EVAL_NESTING: usize = 16;
        if depth == MAX_EVAL_NESTING {
            return Err(UnanalyzableEval);
        }
        let source = self.eval_source(index);
        if source.is_empty() {
            return Ok(0);
        }
        if ShellWord(&source).has_dynamic_expansion() {
            return Err(UnanalyzableEval);
        }
        let nested = CommandListEntry(&source);
        background_operator_count(nested)
            .checked_add(
                Self::parse(nested)
                    .ok_or(UnanalyzableEval)?
                    .background_job_count_at_depth(depth + 1)?,
            )
            .ok_or(UnanalyzableEval)
    }

    /// Reconstruct the static words that the `eval` command will evaluate.
    fn eval_source(&self, index: usize) -> String {
        index
            .checked_add(1)
            .and_then(|first_argument| self.0.get(first_argument..))
            .unwrap_or_default()
            .iter()
            .take_while(|word| !ShellWord(word).is_list_operator())
            .cloned()
            .collect::<Vec<_>>()
            .join(" ")
    }
}

impl ShellWord<'_> {
    /// Whether this word is `exec`.
    fn is_exec(self) -> bool {
        self.0 == "exec"
    }

    /// Whether this word invokes `eval` as a simple shell command.
    fn is_eval(self) -> bool {
        self.0 == "eval"
    }

    /// Whether this word can invoke `exec` outside the direct supported boundary.
    fn is_exec_wrapper(self) -> bool {
        matches!(self.0, "if" | "command")
    }

    /// Whether this word ends one simple command and starts another.
    fn ends_command(self) -> bool {
        matches!(
            self.0,
            "&&" | "||"
                | "|"
                | "&"
                | "("
                | "{"
                | "if"
                | "then"
                | "do"
                | "else"
                | "elif"
                | "while"
                | "until"
        ) || self.0.ends_with(';')
            || self.0.ends_with(')')
    }

    /// Whether this word terminates an `eval` command's argument sequence.
    fn is_list_operator(self) -> bool {
        matches!(self.0, "&&" | "||" | "|" | "&" | ";") || self.0.ends_with(';')
    }

    /// Whether this shell source can expand into arbitrary syntax at runtime.
    fn has_dynamic_expansion(self) -> bool {
        self.0
            .chars()
            .any(|character| matches!(character, '$' | '`' | '*' | '?' | '['))
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

impl CommandListEntry<'_> {
    /// Whether this entry contains a control character Ninja cannot retain in
    /// one `command =` binding.
    fn has_ninja_control_character(self) -> bool {
        self.0.chars().any(char::is_control)
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
    // `shell_quote::QuoteRefExt::quoted(Sh)` produces minimally quoted
    // fragments, while the `eval` wrapper requires this canonical enclosing
    // form to preserve its generated Ninja text and delimiter contract.
    let escaped = command.0.replace('\'', r"'\''");
    format!("'{escaped}'")
}

#[cfg(test)]
#[path = "ninja_gen_command_list_tests.rs"]
mod tests;
