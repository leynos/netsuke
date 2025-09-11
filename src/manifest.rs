//! Manifest loading helpers.
//!
//! This module parses a `Netsukefile` without relying on a global Jinja
//! preprocessing pass. The YAML is parsed first and Jinja expressions are
//! evaluated only within string values or the `foreach` and `when` keys. It
//! exposes `env()` to read environment variables and `glob()` to expand
//! filesystem patterns during template evaluation. Both helpers fail fast when
//! inputs are missing or patterns are invalid.

use crate::ast::{NetsukeManifest, Recipe, StringOrList, Target, Vars};
use anyhow::{Context, Result};
use miette::{Diagnostic, NamedSource, SourceSpan};
use minijinja::{Environment, Error, ErrorKind, UndefinedBehavior, context, value::Value};
use serde_yml::{Error as YamlError, Location};
use serde_yml::{Mapping as YamlMapping, Value as YamlValue};
use std::{fs, path::Path};
use thiserror::Error;

mod hints;
use hints::YAML_HINTS;

// Compute a narrow highlight span from a location.
fn to_span(src: &str, loc: Location) -> SourceSpan {
    let at = loc.index();
    let bytes = src.as_bytes();
    let (start, end) = match bytes.get(at) {
        Some(&b) if b != b'\n' => (at, at + 1),
        _ => {
            // Fallback: highlight the previous byte on the same line when possible.
            let start = if at > 0 && bytes.get(at - 1).is_some_and(|p| *p != b'\n') {
                at - 1
            } else {
                at
            };
            (start, at)
        }
    };
    let len = end.saturating_sub(start);
    #[allow(clippy::useless_conversion, reason = "future-proof span length type")]
    SourceSpan::new(start.into(), len.into())
}

#[derive(Debug, Error, Diagnostic)]
#[error("{message}")]
#[diagnostic(code(netsuke::yaml::parse))]
pub struct YamlDiagnostic {
    #[source_code]
    src: NamedSource<String>,
    #[label("parse error here")]
    span: Option<SourceSpan>,
    #[help]
    help: Option<String>,
    #[source]
    source: YamlError,
    message: String,
}

fn has_tab_indent(src: &str, loc: Option<Location>) -> bool {
    let Some(loc) = loc else { return false };
    let line_idx = loc.line().saturating_sub(1);
    let line = src.lines().nth(line_idx).unwrap_or("");
    // Inspect only leading whitespace on the error line to avoid false positives
    // from tabs elsewhere in the file.
    line.chars()
        .take_while(|c| c.is_whitespace())
        .any(|c| c == '\t')
}

fn hint_for(err_str: &str, src: &str, loc: Option<Location>) -> Option<String> {
    if has_tab_indent(src, loc) {
        return Some("Use spaces for indentation; tabs are invalid in YAML.".into());
    }
    let lower = err_str.to_lowercase();
    YAML_HINTS
        .iter()
        .find(|(needle, _)| lower.contains(*needle))
        .map(|(_, hint)| (*hint).into())
}

#[derive(Debug, Error, Diagnostic)]
pub enum ManifestError {
    #[error("manifest parse error")]
    #[diagnostic(code(netsuke::manifest::parse))]
    Parse {
        #[source]
        #[diagnostic_source]
        source: YamlDiagnostic,
    },
}

fn map_yaml_error(err: YamlError, src: &str, name: &str) -> YamlDiagnostic {
    let loc = err.location();
    let (line, col, span) = loc.map_or((1, 1, None), |l| {
        (l.line(), l.column(), Some(to_span(src, l)))
    });
    let err_str = err.to_string();
    let hint = hint_for(&err_str, src, loc);
    let mut message = format!("YAML parse error at line {line}, column {col}: {err_str}");
    if let Some(ref h) = hint {
        message.push_str("\nhelp: ");
        message.push_str(h);
    }

    YamlDiagnostic {
        src: NamedSource::new(name, src.to_string()),
        span,
        help: hint,
        source: err,
        message,
    }
}

/// Resolve the value of an environment variable for the `env()` Jinja helper.
///
/// Returns the variable's value or a structured error that mirrors Jinja's
/// failure modes, ensuring templates halt when a variable is missing or not
/// valid UTF-8.
///
/// # Examples
///
/// The [`EnvLock`](test_support::env_lock::EnvLock) guard serialises access to
/// the process environment so tests do not interfere with each other.
///
/// ```rust,ignore
/// use test_support::env_lock::EnvLock;
/// let _guard = EnvLock::acquire();
/// std::env::set_var("FOO", "bar");
/// assert_eq!(env("FOO").unwrap(), "bar");
/// std::env::remove_var("FOO");
/// ```
fn env_var(name: &str) -> std::result::Result<String, Error> {
    match std::env::var(name) {
        Ok(val) => Ok(val),
        Err(std::env::VarError::NotPresent) => Err(Error::new(
            ErrorKind::UndefinedError,
            format!("environment variable '{name}' is not set"),
        )),
        Err(std::env::VarError::NotUnicode(_)) => Err(Error::new(
            ErrorKind::InvalidOperation,
            format!("environment variable '{name}' is set but contains invalid UTF-8"),
        )),
    }
}

/// Process a single glob entry and normalize its output.
///
/// Returns the entry path when it points to a file, skipping directories.
/// Requires matched paths to be valid UTF-8; output is normalized to use
/// forward slashes.
///
/// # Examples
///
/// ```rust,ignore
/// use glob::glob;
/// let entry = glob("Cargo.toml").unwrap().next().unwrap();
/// let path = process_glob_entry(entry, "Cargo.toml")
///     .unwrap()
///     .unwrap();
/// assert!(path.ends_with("Cargo.toml"));
/// ```
fn process_glob_entry(
    entry: std::result::Result<std::path::PathBuf, glob::GlobError>,
    pattern: &str,
) -> std::result::Result<Option<String>, Error> {
    match entry {
        Ok(path) => {
            // Query metadata early to surface filesystem errors promptly.
            let meta = path.metadata().map_err(|e| {
                Error::new(
                    ErrorKind::InvalidOperation,
                    format!("glob failed for '{pattern}': {e}"),
                )
            })?;
            if !meta.is_file() {
                return Ok(None);
            }
            // Reject non-UTF-8 paths to avoid lossy round-trips and ensure
            // manifests remain deterministic.
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

/// Expand a glob pattern into a list of matching paths with deterministic ordering.
///
/// Results are returned in lexicographic order to keep builds deterministic.
/// Invalid patterns or filesystem errors surface as Jinja evaluation errors so
/// manifests fail fast when input is incorrect.
///
/// # Errors
///
/// Returns an error if the glob pattern is invalid, a directory cannot be
/// read, or a matched path is not valid UTF-8.
/// Matching is case-sensitive on all platforms, wildcards do not cross path
/// separators (use `**` to span directories), and leading-dot entries are
/// matched by wildcards.
/// Convert `/` and `\` to the host separator while preserving escapes for
/// glob metacharacters on Unix.
fn normalize_separators(pattern: &str) -> String {
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
    // Keep escapes for bracket-like characters so they remain literal.
    matches!(next, '[' | ']' | '{' | '}')
}

#[cfg(unix)]
fn should_preserve_backslash_for_wildcard(
    it: &mut std::iter::Peekable<std::str::Chars<'_>>,
) -> bool {
    match it.peek().copied() {
        Some('*' | '?') => {
            // Only treat as escape when the wildcard continues a word.
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
        None => '\\', // Trailing backslash has nothing to escape; keep literal.
    }
}

/// Convert escaped glob metacharacters into bracket classes so they remain
/// literals when passed to `glob_with`.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(force_literal_escapes(r"\*foo"), "[*]foo");
/// ```
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

/// Check if character opens a brace outside character class.
fn is_opening_brace(ch: char, in_class: bool) -> bool {
    ch == '{' && !in_class
}

/// Check if character closes a brace outside character class.
fn is_closing_brace(ch: char, in_class: bool) -> bool {
    ch == '}' && !in_class
}

/// Check if character is an unmatched closing brace.
fn is_unmatched_closing_brace(ch: char, in_class: bool, depth: i32) -> bool {
    ch == '}' && !in_class && depth == 0
}

/// Check if character starts a character class.
fn is_class_start(ch: char, in_class: bool) -> bool {
    ch == '[' && !in_class
}

/// Check if character ends a character class.
fn is_class_end(ch: char, in_class: bool) -> bool {
    ch == ']' && in_class
}

/// Create error for unmatched brace with position.
fn create_unmatched_brace_error(pattern: &str, brace_char: char, pos: usize) -> Error {
    Error::new(
        ErrorKind::SyntaxError,
        format!("invalid glob pattern '{pattern}': unmatched '{brace_char}' at position {pos}"),
    )
}

/// Validate final brace depth and create error if unmatched.
fn validate_final_depth(
    pattern: &str,
    depth: i32,
    last_open_brace_pos: Option<usize>,
) -> Result<(), Error> {
    if depth != 0 {
        let pos = last_open_brace_pos.unwrap_or(0);
        Err(create_unmatched_brace_error(pattern, '{', pos))
    } else {
        Ok(())
    }
}

/// Handle an escape sequence, pushing the appropriate replacement into `out`.
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

/// Return the bracket-class replacement for recognised escapes.
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

/// Track brace state while validating glob patterns.
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

    fn process_character(&mut self, ch: char, pos: usize, pattern: &str) -> Result<(), Error> {
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

    fn handle_brace_matching(&mut self, ch: char, pos: usize, pattern: &str) -> Result<(), Error> {
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

    fn validate_final_state(&self, pattern: &str) -> Result<(), Error> {
        validate_final_depth(pattern, self.depth, self.last_open_brace_pos)
    }
}

/// Validate that braces in a glob pattern are balanced.
///
/// Escaped braces are ignored when tracking depth on Unix, and braces inside
/// character classes `[]` are treated as literals. Returns a syntax error when
/// opening and closing braces do not match, including the position of the
/// offending character for easier debugging.
///
/// # Examples
///
/// ```ignore
/// assert!(validate_brace_matching("foo{bar}").is_ok());
/// assert!(validate_brace_matching("{foo").is_err());
/// ```
fn validate_brace_matching(pattern: &str) -> std::result::Result<(), Error> {
    let mut validator = BraceValidator::new();

    for (i, ch) in pattern.char_indices() {
        validator.process_character(ch, i, pattern)?;
    }

    validator.validate_final_state(pattern)
}

// Determine whether a wildcard continues an identifier when normalising
// separators. Only used on Unix hosts where '\\' is not a path separator.
#[cfg(unix)]
fn is_wildcard_continuation_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '-' || ch == '_'
}

fn glob_paths(pattern: &str) -> std::result::Result<Vec<String>, Error> {
    use glob::{MatchOptions, glob_with};

    // Enforce shell-like semantics:
    // - patterns are case-sensitive,
    // - wildcards do not cross path separators,
    // - dotfiles are matched by default.
    let opts = MatchOptions {
        case_sensitive: true,
        require_literal_separator: true,
        require_literal_leading_dot: false,
    };

    // Validate braces on the original input so diagnostics mirror the user's
    // pattern verbatim. The validator ignores braces inside character classes.
    validate_brace_matching(pattern)?;

    // Normalize separators so `/` and `\\` behave the same on all platforms.
    let mut normalized = normalize_separators(pattern);
    // Force escaped meta to be treated literally by `glob_with` via bracket
    // classes. Only apply on Unix hosts where '\\' is not a path separator.
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

/// Parse a manifest string using Jinja for value templating.
///
/// The input YAML must be valid on its own. Jinja expressions are evaluated
/// only inside recognised string fields and the `foreach` and `when` keys.
///
/// # Errors
///
/// Returns an error if YAML parsing or Jinja evaluation fails.
fn from_str_named(yaml: &str, name: &str) -> Result<NetsukeManifest> {
    let mut doc: YamlValue =
        serde_yml::from_str(yaml).map_err(|e| map_yaml_error(e, yaml, name))?;

    let mut jinja = Environment::new();
    jinja.set_undefined_behavior(UndefinedBehavior::Strict);
    // Expose custom helpers to templates.
    jinja.add_function("env", |name: String| env_var(&name));
    jinja.add_function("glob", |pattern: String| glob_paths(&pattern));

    if let Some(vars) = doc.get("vars").and_then(|v| v.as_mapping()).cloned() {
        for (k, v) in vars {
            let key = k
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("non-string key in vars mapping: {k:?}"))?
                .to_string();
            jinja.add_global(key, Value::from_serialize(v));
        }
    }

    expand_foreach(&mut doc, &jinja)?;

    let manifest: NetsukeManifest =
        serde_yml::from_value(doc).map_err(|e| ManifestError::Parse {
            source: map_yaml_error(e, yaml, name),
        })?;

    render_manifest(manifest, &jinja)
}

/// Parse a manifest string using Jinja for value templating.
///
/// The input YAML must be valid on its own. Jinja expressions are evaluated
/// only inside recognised string fields and the `foreach` and `when` keys.
///
/// # Errors
///
/// Returns an error if YAML parsing or Jinja evaluation fails.
pub fn from_str(yaml: &str) -> Result<NetsukeManifest> {
    from_str_named(yaml, "Netsukefile")
}

/// Expand `foreach` entries within the raw YAML document.
fn expand_foreach(doc: &mut YamlValue, env: &Environment) -> Result<()> {
    let Some(targets) = doc.get_mut("targets").and_then(|v| v.as_sequence_mut()) else {
        return Ok(());
    };

    let mut expanded = Vec::new();
    for target in std::mem::take(targets) {
        match target {
            YamlValue::Mapping(map) => expanded.extend(expand_target(map, env)?),
            other => expanded.push(other),
        }
    }

    *targets = expanded;
    Ok(())
}

fn expand_target(map: YamlMapping, env: &Environment) -> Result<Vec<YamlValue>> {
    let foreach_key = YamlValue::String("foreach".into());
    if let Some(expr_val) = map.get(&foreach_key) {
        let values = parse_foreach_values(expr_val, env)?;
        let mut items = Vec::new();
        for (index, item) in values.into_iter().enumerate() {
            let mut clone = map.clone();
            clone.remove(&foreach_key);
            if !when_allows(&mut clone, env, &item, index)? {
                continue;
            }
            inject_iteration_vars(&mut clone, &item, index)?;
            items.push(YamlValue::Mapping(clone));
        }
        Ok(items)
    } else {
        Ok(vec![YamlValue::Mapping(map)])
    }
}

fn parse_foreach_values(expr_val: &YamlValue, env: &Environment) -> Result<Vec<Value>> {
    if let Some(seq) = expr_val.as_sequence() {
        return Ok(seq.iter().cloned().map(Value::from_serialize).collect());
    }
    let expr = as_str(expr_val, "foreach")?;
    let seq = eval_expression(env, "foreach", expr, context! {})?;
    let iter = seq
        .try_iter()
        .context("foreach expression did not yield an iterable")?;
    Ok(iter.collect())
}

fn when_allows(
    map: &mut YamlMapping,
    env: &Environment,
    item: &Value,
    index: usize,
) -> Result<bool> {
    let when_key = YamlValue::String("when".into());
    if let Some(when_val) = map.remove(&when_key) {
        let expr = as_str(&when_val, "when")?;
        let result = eval_expression(env, "when", expr, context! { item, index })?;
        Ok(result.is_true())
    } else {
        Ok(true)
    }
}

fn inject_iteration_vars(map: &mut YamlMapping, item: &Value, index: usize) -> Result<()> {
    let vars_key = YamlValue::String("vars".into());
    let mut vars = match map.remove(&vars_key) {
        None => YamlMapping::new(),
        Some(YamlValue::Mapping(m)) => m,
        Some(other) => {
            return Err(anyhow::anyhow!(
                "target.vars must be a mapping, got: {other:?}"
            ));
        }
    };
    vars.insert(
        YamlValue::String("item".into()),
        serde_yml::to_value(item).context("serialise item")?,
    );
    vars.insert(
        YamlValue::String("index".into()),
        YamlValue::Number(u64::try_from(index).expect("index overflow").into()),
    );
    map.insert(vars_key, YamlValue::Mapping(vars));
    Ok(())
}

fn as_str<'a>(value: &'a YamlValue, field: &str) -> Result<&'a str> {
    value
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("{field} must be a string expression"))
}

fn eval_expression(env: &Environment, name: &str, expr: &str, ctx: Value) -> Result<Value> {
    env.compile_expression(expr)
        .with_context(|| format!("{name} expression parse error"))?
        .eval(ctx)
        .with_context(|| format!("{name} evaluation error"))
}

/// Render a Jinja template and label any error with the given context.
fn render_str_with(
    env: &Environment,
    tpl: &str,
    ctx: &impl serde::Serialize,
    what: impl FnOnce() -> String,
) -> Result<String> {
    env.render_str(tpl, ctx).with_context(what)
}

/// Render all templated strings in the manifest.
fn render_manifest(mut manifest: NetsukeManifest, env: &Environment) -> Result<NetsukeManifest> {
    for action in &mut manifest.actions {
        render_target(action, env)?;
    }
    for target in &mut manifest.targets {
        render_target(target, env)?;
    }
    for rule in &mut manifest.rules {
        render_rule(rule, env)?;
    }
    Ok(manifest)
}

fn render_rule(rule: &mut crate::ast::Rule, env: &Environment) -> Result<()> {
    if let Some(desc) = &mut rule.description {
        *desc = render_str_with(env, desc, &context! {}, || "render rule description".into())?;
    }
    render_string_or_list(&mut rule.deps, env, &Vars::new())?;
    match &mut rule.recipe {
        Recipe::Command { command } => {
            *command =
                render_str_with(env, command, &context! {}, || "render rule command".into())?;
        }
        Recipe::Script { script } => {
            *script = render_str_with(env, script, &context! {}, || "render rule script".into())?;
        }
        Recipe::Rule { rule: r } => render_string_or_list(r, env, &Vars::new())?,
    }
    Ok(())
}

fn render_target(target: &mut Target, env: &Environment) -> Result<()> {
    render_vars(&mut target.vars, env)?;
    render_string_or_list(&mut target.name, env, &target.vars)?;
    render_string_or_list(&mut target.sources, env, &target.vars)?;
    render_string_or_list(&mut target.deps, env, &target.vars)?;
    render_string_or_list(&mut target.order_only_deps, env, &target.vars)?;
    match &mut target.recipe {
        Recipe::Command { command } => {
            *command = render_str_with(env, command, &target.vars, || {
                "render target command".into()
            })?;
        }
        Recipe::Script { script } => {
            *script = render_str_with(env, script, &target.vars, || "render target script".into())?;
        }
        Recipe::Rule { rule } => render_string_or_list(rule, env, &target.vars)?,
    }
    Ok(())
}

fn render_vars(vars: &mut Vars, env: &Environment) -> Result<()> {
    let snapshot = vars.clone();
    for (key, value) in vars.iter_mut() {
        if let YamlValue::String(s) = value {
            *s = render_str_with(env, s, &snapshot, || format!("render var '{key}'"))?;
        }
    }
    Ok(())
}

fn render_string_or_list(value: &mut StringOrList, env: &Environment, ctx: &Vars) -> Result<()> {
    match value {
        StringOrList::String(s) => {
            *s = render_str_with(env, s, ctx, || "render string value".into())?;
        }
        StringOrList::List(list) => {
            for item in list {
                *item = render_str_with(env, item, ctx, || "render list value".into())?;
            }
        }
        StringOrList::Empty => {}
    }
    Ok(())
}

/// Load a [`NetsukeManifest`] from the given file path.
///
/// # Errors
///
/// Returns an error if the file cannot be read or the YAML fails to parse.
pub fn from_path(path: impl AsRef<Path>) -> Result<NetsukeManifest> {
    let path_ref = path.as_ref();
    let data = fs::read_to_string(path_ref)
        .with_context(|| format!("failed to read {}", path_ref.display()))?;
    from_str_named(&data, &path_ref.display().to_string())
}

#[cfg(test)]
mod tests;
