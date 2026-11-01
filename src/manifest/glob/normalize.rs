//! Separator and escape normalisation for glob patterns and matched paths.

use camino::Utf8Path;

/// Normalise forward slashes in a glob `pattern` to the platform separator.
///
/// Forward slashes are rewritten to [`std::path::MAIN_SEPARATOR`] so manifests
/// can use `/` portably. On Unix, backslash escapes are preserved for the
/// follow-up literal-escape pass rather than treated as separators here.
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

/// Push the normalized separator for a backslash escape without consuming its
/// following character; it only peeks at that character to choose the output.
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

/// Stub that pushes a literal backslash, keeping cfg alignment on other platforms.
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
    // normalization path uses the `replace` branch. Provide a stub to keep
    // cfg alignment in sync for future callers.
    out.push('\\');
}

/// Rewrite selected backslash escapes into bracket character classes.
///
/// The `glob` crate treats `\` literally on Unix, so an escaped metacharacter
/// such as `\*` is converted to the single-element class `[*]` to match the
/// literal character. Only escapes for `*`, `?`, `[`, `]`, `{`, and `}` are
/// rewritten; other escapes are left for the `glob` crate to interpret.
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

/// Rewrite recognised escaped metacharacters as bracket classes.
///
/// Recognised metacharacters are consumed and emitted as bracket classes.
/// Other characters preserve the backslash, leaving the following character
/// for the caller to process.
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
/// Report whether a character continues a wildcard token.
fn is_wildcard_continuation_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '-' || ch == '_'
}

/// Remove an injected base directory from a matched path, restoring the
/// pattern-relative spelling the caller supplied.
///
/// Only relative patterns with an injected base are affected: the base is
/// present exactly when the matcher ran against `base.join(pattern)`, so every
/// match starts with it and the lexical strip cannot drop real matches. The
/// fallback returns the path unchanged.
///
/// Separator normalisation is platform-aware, like the pattern side above. On
/// Windows a backslash is a directory separator, so it is rendered as `/` for
/// the portable spelling callers expect. On Unix a backslash is an ordinary
/// filename character: rewriting it would report a different file from the one
/// the capability metadata check selected, and a name such as
/// `part\..\other.txt` would become the traversal `part/../other.txt`. The
/// Unix branch therefore returns the checked spelling unchanged.
pub(super) fn strip_base(base: Option<&Utf8Path>, path: &Utf8Path) -> String {
    let relative = base
        .and_then(|dir| path.strip_prefix(dir).ok())
        .unwrap_or(path);
    // Format once, after any lexical base stripping, so a matched path does
    // not first allocate a normalized String only to allocate again to rebase.
    #[cfg(unix)]
    {
        relative.as_str().to_owned()
    }
    #[cfg(not(unix))]
    {
        relative.as_str().replace('\\', "/")
    }
}
