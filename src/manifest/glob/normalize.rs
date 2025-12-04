//! Separator and escape normalisation for glob patterns.

pub(crate) fn normalize_separators(pattern: &str) -> String {
    let native = std::path::MAIN_SEPARATOR;
    #[cfg(unix)]
    {
        let mut out = String::with_capacity(pattern.len());
        let mut it = pattern.chars().peekable();
        while let Some(c) = it.next() {
            if c == '\\' {
                push_normalized_backslash(&mut it, &mut out, native);
                continue;
            }

            if c == '/' {
                out.push(native);
                continue;
            }

            out.push(c);
        }
        out
    }
    #[cfg(not(unix))]
    {
        pattern.replace('/', &native.to_string())
    }
}

#[cfg(unix)]
fn push_normalized_backslash(
    it: &mut std::iter::Peekable<std::str::Chars<'_>>,
    out: &mut String,
    native: char,
) {
    let Some(next) = it.peek().copied() else {
        out.push('\\');
        return;
    };

    if matches!(next, '[' | ']' | '{' | '}') {
        out.push('\\');
        return;
    }

    if matches!(next, '*' | '?') {
        let mut lookahead = it.clone();
        lookahead.next();
        let should_preserve_wildcard = match lookahead.peek() {
            None => true,
            Some(&ch) => is_wildcard_continuation_char(ch),
        };
        if should_preserve_wildcard {
            out.push('\\');
            return;
        }
    }

    out.push(native);
}

#[cfg(not(unix))]
#[expect(
    dead_code,
    reason = "stub keeps cfg alignment; unused on non-Unix targets"
)]
fn push_normalized_backslash(
    _it: &mut std::iter::Peekable<std::str::Chars<'_>>,
    out: &mut String,
    _native: char,
) {
    // On non-Unix targets the function is never invoked because the
    // normalisation path uses the `replace` branch. Provide a stub to keep
    // cfg alignment in sync for future callers.
    out.push('\\');
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
            '\\' if !in_class => handle_escaped_char(&mut it, &mut out),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(unix)]
fn handle_escaped_char(it: &mut std::iter::Peekable<std::str::Chars<'_>>, out: &mut String) {
    let Some(next) = it.peek().copied() else {
        out.push('\\');
        return;
    };

    match next {
        '*' => {
            it.next();
            out.push_str("[*]");
        }
        '?' => {
            it.next();
            out.push_str("[?]");
        }
        '[' => {
            it.next();
            out.push_str("[[]");
        }
        ']' => {
            it.next();
            out.push_str("[]]");
        }
        '{' => {
            it.next();
            out.push_str("[{]");
        }
        '}' => {
            it.next();
            out.push_str("[}]");
        }
        _ => out.push('\\'),
    }
}

#[cfg(unix)]
fn is_wildcard_continuation_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '-' || ch == '_'
}
