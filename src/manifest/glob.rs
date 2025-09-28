use minijinja::{Error, ErrorKind};

fn process_glob_entry(
    entry: std::result::Result<std::path::PathBuf, glob::GlobError>,
    pattern: &str,
) -> std::result::Result<Option<String>, Error> {
    match entry {
        Ok(path) => {
            let meta = path.metadata().map_err(|e| {
                Error::new(
                    ErrorKind::InvalidOperation,
                    format!("glob failed for '{pattern}': {e}"),
                )
            })?;
            if !meta.is_file() {
                return Ok(None);
            }
            let s = path.to_str().ok_or_else(|| {
                Error::new(
                    ErrorKind::InvalidOperation,
                    format!("glob matched a non-UTF-8 path: {}", path.display()),
                )
            })?;
            Ok(Some(s.replace('\\', "/")))
        }
        Err(e) => Err(Error::new(
            ErrorKind::InvalidOperation,
            format!("glob failed for '{pattern}': {e}"),
        )),
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

fn is_opening_brace(ch: char, in_class: bool) -> bool {
    ch == '{' && !in_class
}

fn is_closing_brace(ch: char, in_class: bool) -> bool {
    ch == '}' && !in_class
}

fn is_unmatched_closing_brace(ch: char, in_class: bool, depth: i32) -> bool {
    ch == '}' && !in_class && depth == 0
}

fn is_class_start(ch: char, in_class: bool) -> bool {
    ch == '[' && !in_class
}

fn is_class_end(ch: char, in_class: bool) -> bool {
    ch == ']' && in_class
}

fn create_unmatched_brace_error(pattern: &str, brace_char: char, pos: usize) -> Error {
    Error::new(
        ErrorKind::SyntaxError,
        format!("invalid glob pattern '{pattern}': unmatched '{brace_char}' at position {pos}"),
    )
}

fn validate_final_depth(
    pattern: &str,
    depth: i32,
    last_open_brace_pos: Option<usize>,
) -> std::result::Result<(), Error> {
    if depth != 0 {
        let pos = last_open_brace_pos.unwrap_or(0);
        Err(create_unmatched_brace_error(pattern, '{', pos))
    } else {
        Ok(())
    }
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
    depth: i32,
    escaped: bool,
    in_class: bool,
    last_open_brace_pos: Option<usize>,
    escape_active: bool,
}

impl BraceValidator {
    fn new() -> Self {
        Self {
            depth: 0,
            escaped: false,
            in_class: false,
            last_open_brace_pos: None,
            escape_active: cfg!(unix),
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

        if self.handle_character_class_transitions(ch) {
            return Ok(());
        }

        self.handle_brace_matching(ch, pos, pattern)
    }

    fn handle_escape_sequence(&mut self, ch: char) -> bool {
        if self.escaped {
            self.escaped = false;
            return true;
        }

        if ch == '\\' && self.escape_active {
            self.escaped = true;
            return true;
        }

        false
    }

    fn handle_character_class_transitions(&mut self, ch: char) -> bool {
        if is_class_start(ch, self.in_class) {
            self.in_class = true;
            true
        } else if is_class_end(ch, self.in_class) {
            self.in_class = false;
            true
        } else {
            false
        }
    }

    fn handle_brace_matching(
        &mut self,
        ch: char,
        pos: usize,
        pattern: &str,
    ) -> std::result::Result<(), Error> {
        if is_unmatched_closing_brace(ch, self.in_class, self.depth) {
            return Err(create_unmatched_brace_error(pattern, '}', pos));
        }

        if is_opening_brace(ch, self.in_class) {
            self.depth += 1;
            self.last_open_brace_pos = Some(pos);
        } else if is_closing_brace(ch, self.in_class) {
            self.depth -= 1;
        }

        Ok(())
    }

    fn validate_final_state(&self, pattern: &str) -> std::result::Result<(), Error> {
        validate_final_depth(pattern, self.depth, self.last_open_brace_pos)
    }
}

fn validate_brace_matching(pattern: &str) -> std::result::Result<(), Error> {
    let mut validator = BraceValidator::new();

    for (i, ch) in pattern.char_indices() {
        validator.process_character(ch, i, pattern)?;
    }

    validator.validate_final_state(pattern)
}

#[cfg(unix)]
fn is_wildcard_continuation_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '-' || ch == '_'
}

pub(crate) fn glob_paths(pattern: &str) -> std::result::Result<Vec<String>, Error> {
    use glob::{MatchOptions, glob_with};

    let opts = MatchOptions {
        case_sensitive: true,
        require_literal_separator: true,
        require_literal_leading_dot: false,
    };

    validate_brace_matching(pattern)?;

    let mut normalized = normalize_separators(pattern);
    #[cfg(unix)]
    {
        normalized = force_literal_escapes(&normalized);
    }

    let entries = glob_with(&normalized, opts).map_err(|e| {
        Error::new(
            ErrorKind::SyntaxError,
            format!("invalid glob pattern '{pattern}': {e}"),
        )
    })?;
    let mut paths = Vec::new();
    for entry in entries {
        if let Some(p) = process_glob_entry(entry, pattern)? {
            paths.push(p);
        }
    }
    Ok(paths)
}
