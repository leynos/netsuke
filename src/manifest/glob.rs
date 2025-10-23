//! Utilities for normalising and validating manifest glob patterns.
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs::Dir};
use minijinja::{Error, ErrorKind};

/// Represents a character being processed with its context
#[derive(Debug, Clone, Copy)]
pub struct CharContext {
    pub ch: char,
    pub position: usize,
    pub in_class: bool,
    pub escaped: bool,
}

/// Configuration for brace validation
#[derive(Debug, Clone)]
pub struct BraceValidationState {
    pub depth: i32,
    pub in_class: bool,
    pub last_open_pos: Option<usize>,
    pub escape_active: bool,
}

/// Represents a glob pattern with processing state
#[derive(Debug, Clone)]
pub struct GlobPattern {
    pub raw: String,
    pub normalized: Option<String>,
}

/// Error context for glob operations
#[derive(Debug)]
pub struct GlobErrorContext {
    pub pattern: String,
    pub error_char: char,
    pub position: usize,
    pub error_type: GlobErrorType,
}

#[derive(Debug)]
pub enum GlobErrorType {
    UnmatchedBrace,
    InvalidPattern,
    IoError,
}

pub type GlobEntryResult = std::result::Result<std::path::PathBuf, glob::GlobError>;

fn create_glob_error(context: &GlobErrorContext, details: Option<String>) -> Error {
    match context.error_type {
        GlobErrorType::UnmatchedBrace => Error::new(
            ErrorKind::SyntaxError,
            format!(
                "invalid glob pattern '{}': unmatched '{}' at position {}",
                context.pattern, context.error_char, context.position
            ),
        ),
        GlobErrorType::InvalidPattern => {
            let detail = details.unwrap_or_else(|| "unknown pattern error".to_owned());
            Error::new(
                ErrorKind::SyntaxError,
                format!("invalid glob pattern '{}': {detail}", context.pattern),
            )
        }
        GlobErrorType::IoError => {
            let detail = details.unwrap_or_else(|| "unknown IO error".to_owned());
            let message = if detail.starts_with("glob ") {
                detail
            } else {
                format!("glob failed for '{}': {detail}", context.pattern)
            };
            Error::new(ErrorKind::InvalidOperation, message)
        }
    }
}

fn create_unmatched_brace_error(context: &GlobErrorContext) -> Error {
    create_glob_error(context, None)
}

fn process_glob_entry(
    entry: GlobEntryResult,
    pattern: GlobPattern,
    root: &Dir,
) -> std::result::Result<Option<String>, Error> {
    match entry {
        Ok(path) => {
            let utf_path = Utf8PathBuf::try_from(path).map_err(|_| {
                create_glob_error(
                    &GlobErrorContext {
                        pattern: pattern.raw.clone(),
                        error_char: char::from(0),
                        position: pattern.raw.len(),
                        error_type: GlobErrorType::IoError,
                    },
                    Some("glob matched a non-UTF-8 path".to_owned()),
                )
            })?;
            let metadata = fetch_metadata(root, &utf_path).map_err(|err| {
                create_glob_error(
                    &GlobErrorContext {
                        pattern: pattern.raw.clone(),
                        error_char: char::from(0),
                        position: pattern.raw.len(),
                        error_type: GlobErrorType::IoError,
                    },
                    Some(err.to_string()),
                )
            })?;
            if !metadata.is_file() {
                return Ok(None);
            }
            Ok(Some(utf_path.as_str().replace(char::from(0x5c), "/")))
        }
        Err(e) => Err(create_glob_error(
            &GlobErrorContext {
                pattern: pattern.raw,
                error_char: char::from(0),
                position: 0,
                error_type: GlobErrorType::IoError,
            },
            Some(e.to_string()),
        )),
    }
}

fn fetch_metadata(root: &Dir, path: &Utf8Path) -> std::io::Result<cap_std::fs::Metadata> {
    if path.is_absolute() {
        let stripped = path
            .as_str()
            .trim_start_matches(|c| c == char::from(0x2f) || c == char::from(0x5c));
        if stripped.is_empty() {
            root.metadata(Utf8Path::new("."))
        } else {
            root.metadata(stripped)
        }
    } else {
        root.metadata(path)
    }
}

fn open_root_dir(pattern: &GlobPattern) -> std::io::Result<Dir> {
    let candidate = pattern
        .normalized
        .as_deref()
        .unwrap_or(pattern.raw.as_str());
    let path = Utf8Path::new(candidate);
    if path.is_absolute() {
        Dir::open_ambient_dir("/", ambient_authority())
    } else {
        Dir::open_ambient_dir(".", ambient_authority())
    }
}

pub(crate) fn normalize_separators(pattern: &str) -> String {
    let native = std::path::MAIN_SEPARATOR;
    #[cfg(unix)]
    {
        let mut out = String::with_capacity(pattern.len());
        let mut it = pattern.chars().peekable();
        while let Some(c) = it.next() {
            if c == '\\' {
                out.push(process_backslash(&mut it, native));
            } else if c == '/' || c == '\\' {
                out.push(native);
            } else {
                out.push(c);
            }
        }
        out
    }
    #[cfg(not(unix))]
    {
        pattern.replace('/', &native.to_string())
    }
}

#[cfg(unix)]
fn should_preserve_backslash_for_bracket(next: char) -> bool {
    matches!(next, '[' | ']' | '{' | '}')
}

#[cfg(unix)]
fn should_preserve_backslash_for_wildcard(
    it: &mut std::iter::Peekable<std::str::Chars<'_>>,
) -> bool {
    match it.peek().copied() {
        Some('*' | '?') => {
            let mut lookahead = it.clone();
            lookahead.next();
            match lookahead.peek() {
                None => true,
                Some(&ch) => is_wildcard_continuation_char(ch),
            }
        }
        _ => false,
    }
}

#[cfg(unix)]
fn process_backslash(it: &mut std::iter::Peekable<std::str::Chars<'_>>, native: char) -> char {
    match it.peek().copied() {
        Some(ch) if should_preserve_backslash_for_bracket(ch) => '\\',
        Some(_) if should_preserve_backslash_for_wildcard(it) => '\\',
        Some(_) => native,
        None => '\\',
    }
}

#[cfg(unix)]
fn force_literal_escapes(pattern: &str) -> String {
    let mut out = String::with_capacity(pattern.len());
    let mut it = pattern.chars().peekable();
    let mut in_class = false;
    while let Some(c) = it.next() {
        match c {
            '[' if !in_class => {
                in_class = true;
                out.push(c);
            }
            ']' if in_class => {
                in_class = false;
                out.push(c);
            }
            '\\' if !in_class => process_escape_sequence(&mut it, &mut out),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(unix)]
fn process_escape_sequence(it: &mut std::iter::Peekable<std::str::Chars<'_>>, out: &mut String) {
    if let Some(&next) = it.peek() {
        let repl = get_escape_replacement(next);
        if repl == "\\" {
            out.push('\\');
        } else {
            it.next();
            out.push_str(repl);
        }
    } else {
        out.push('\\');
    }
}

#[cfg(unix)]
fn get_escape_replacement(ch: char) -> &'static str {
    match ch {
        '*' => "[*]",
        '?' => "[?]",
        '[' => "[[]",
        ']' => "[]]",
        '{' => "[{]",
        '}' => "[}]",
        _ => "\\",
    }
}

struct BraceValidator {
    state: BraceValidationState,
    escaped: bool,
}

impl BraceValidator {
    fn new() -> Self {
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

    fn process_character(
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

    fn handle_escape_sequence(
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

    fn handle_character_class(&mut self, context: &CharContext) {
        match context.ch {
            '[' if !context.in_class => self.state.in_class = true,
            ']' if context.in_class => self.state.in_class = false,
            _ => {}
        }
    }

    fn handle_braces(
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

    fn validate_final_state(&self, pattern: &GlobPattern) -> std::result::Result<(), Error> {
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

fn validate_brace_matching(pattern: &GlobPattern) -> std::result::Result<(), Error> {
    let mut validator = BraceValidator::new();

    for (i, ch) in pattern.raw.char_indices() {
        validator.process_character(ch, i, pattern)?;
    }

    validator.validate_final_state(pattern)
}

#[cfg(unix)]
fn is_wildcard_continuation_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '-' || ch == '_'
}

/// Expand a glob pattern and collect the matching UTF-8 file paths.
///
/// # Errors
///
/// Returns an error when the pattern is syntactically invalid, when
/// capability-restricted filesystem access fails, or when a match contains
/// non-UTF-8 data.
///
/// # Panics
///
/// Panics if pattern normalisation fails to record the derived pattern, which
/// indicates a logic error in the validator.
pub fn glob_paths(pattern: &str) -> std::result::Result<Vec<String>, Error> {
    use glob::{MatchOptions, glob_with};

    let opts = MatchOptions {
        case_sensitive: true,
        require_literal_separator: true,
        require_literal_leading_dot: false,
    };

    let mut pattern_state = GlobPattern {
        raw: pattern.to_owned(),
        normalized: None,
    };

    validate_brace_matching(&pattern_state)?;

    #[cfg(unix)]
    let mut normalized = normalize_separators(&pattern_state.raw);
    #[cfg(not(unix))]
    let normalized = normalize_separators(&pattern_state.raw);
    #[cfg(unix)]
    {
        normalized = force_literal_escapes(&normalized);
    }

    pattern_state.normalized = Some(normalized);
    let normalized_pattern = pattern_state.normalized.as_deref().unwrap_or_else(|| {
        debug_assert!(false, "normalized pattern must be present");
        pattern_state.raw.as_str()
    });

    let root = open_root_dir(&pattern_state).map_err(|e| {
        create_glob_error(
            &GlobErrorContext {
                pattern: pattern_state.raw.clone(),
                error_char: char::from(0),
                position: 0,
                error_type: GlobErrorType::IoError,
            },
            Some(e.to_string()),
        )
    })?;

    let entries = glob_with(normalized_pattern, opts).map_err(|e| {
        create_glob_error(
            &GlobErrorContext {
                pattern: pattern_state.raw.clone(),
                error_char: char::from(0),
                position: 0,
                error_type: GlobErrorType::InvalidPattern,
            },
            Some(e.to_string()),
        )
    })?;
    let mut paths = Vec::new();
    for entry in entries {
        if let Some(p) = process_glob_entry(entry, pattern_state.clone(), &root)? {
            paths.push(p);
        }
    }
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{Context, Result, anyhow, ensure};

    #[test]
    fn validate_brace_matching_accepts_balanced_braces() {
        let pattern = GlobPattern {
            raw: "{foo,bar}".into(),
            normalized: None,
        };
        assert!(validate_brace_matching(&pattern).is_ok());
    }

    #[test]
    fn validate_brace_matching_rejects_unmatched_closing() -> Result<()> {
        let pattern = GlobPattern {
            raw: "foo}".into(),
            normalized: None,
        };
        match validate_brace_matching(&pattern) {
            Ok(()) => Err(anyhow!(
                "validate_brace_matching should fail for pattern {:?}",
                pattern.raw
            )),
            Err(err) => {
                ensure!(
                    err.kind() == ErrorKind::SyntaxError,
                    "unexpected error kind {kind:?}",
                    kind = err.kind()
                );
                Ok(())
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn process_glob_entry_rejects_non_utf8_paths() -> Result<()> {
        use cap_std::{ambient_authority, fs::Dir};
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let root =
            Dir::open_ambient_dir("/", ambient_authority()).context("open ambient root dir")?;
        let path = std::path::PathBuf::from(OsString::from_vec(b"bad\xFF".to_vec()));
        let pattern = GlobPattern {
            raw: "pattern".into(),
            normalized: None,
        };
        match process_glob_entry(Ok(path), pattern, &root) {
            Ok(value) => Err(anyhow!("expected non-UTF-8 error but received {value:?}")),
            Err(err) => {
                ensure!(
                    err.kind() == ErrorKind::InvalidOperation,
                    "unexpected error kind {kind:?}",
                    kind = err.kind()
                );
                Ok(())
            }
        }
    }
}
