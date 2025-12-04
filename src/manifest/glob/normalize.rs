//! Separator and escape normalisation for glob patterns.

pub(crate) fn normalize_separators(pattern: &str) -> String {
    let native = std::path::MAIN_SEPARATOR;
    #[cfg(unix)]
    {
        let mut out = String::with_capacity(pattern.len());
        let mut it = pattern.chars().peekable();
        while let Some(c) = it.next() {
            if c == '\\' {
                out.push(process_backslash(&mut it, native));
            } else if c == '/' {
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
const fn should_preserve_backslash_for_bracket(next: char) -> bool {
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
pub(super) fn force_literal_escapes(pattern: &str) -> String {
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
const fn get_escape_replacement(ch: char) -> &'static str {
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

#[cfg(unix)]
fn is_wildcard_continuation_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '-' || ch == '_'
}
