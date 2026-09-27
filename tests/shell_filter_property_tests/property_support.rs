//! Shared harness for the `shell_quote` and `shell_join` property suites.
//!
//! Split out of the parent to keep it under `AGENTS.md`'s 400-line cap. What is
//! shared is deliberately *only* the harness: the dialect names, the generator,
//! the interpreter lookups, and the encoders and decoders the sibling suites
//! measure against. Each obligation lives in the sibling that states it, so a
//! suite can be read without the others.
//!
//! The generated domain is a deliberate alphabet rather than `any::<String>()`.
//! Uniform bytes are almost all inert — a random string of printable ASCII is
//! mostly alphanumerics — so the interesting inputs, the ones made only of
//! metacharacters, would be vanishingly rare and a property would pass on a
//! sample that never exercised quoting at all.

use anyhow::{Context, Result, bail, ensure};
use camino::Utf8PathBuf;
use minijinja::{Environment, context, value::Value};
use netsuke::stdlib::StdlibConfig;
use proptest::prelude::*;
use std::process::Command;

/// The dialect names the filters accept, spelled as call sites must spell them.
pub(super) const SH: &str = "sh";
/// The PowerShell dialect name.
pub(super) const POWERSHELL: &str = "powershell";

/// Characters the shell treats as special, plus enough letters to build words.
///
/// Every entry earns its place: the quotes and backslash are the escaping
/// cases, the expansion characters (`$`, backtick) are the ones a wrong encoder
/// silently expands, the glob and grouping characters (`*`, `?`, `[`, `{`) are
/// the ones that change the *shape* of an argument vector, and the separators
/// (`;`, `&`, `|`, newline-adjacent control characters) are the ones that split
/// one word into several. Tab is here because it is both a shell separator and
/// a control character the encoder must carry literally.
pub(super) const ALPHABET: &[char] = &[
    'a', 'b', 'z', 'A', 'Z', '0', '9', '_', '-', '.', '/', ',', ' ', '\t', '\'', '"', '$', '`',
    '\\', '*', '?', ';', '&', '|', '<', '>', '(', ')', '[', ']', '{', '}', '#', '~', '!', '=', ':',
    '@', '%', '^', '+', 'é', '中', '\u{80}', '\u{a0}',
];

/// The expansion-and-quote witness the plan names as its worst case.
///
/// `shell_quote` must not leave the expansion live, and must not leave the
/// quote unbalanced. Kept as a constant so the corpus-span check below can
/// require it, rather than hoping a random draw produces it.
pub(super) const EXPANSION_WITNESS: &str = "$HOME 'x'";

/// A string drawn from [`ALPHABET`], admissible as a single-line recipe word.
///
/// Lengths run to twenty-four characters, which is long enough to reach several
/// quoting transitions — the encoder alternates in and out of quotes per run.
///
/// The empty word and [`EXPANSION_WITNESS`] are seeded into one arm each. Both
/// are reachable by chance — an empty draw is one of the twenty-five lengths,
/// and the witness is one of astronomically many draws — but a corpus-span
/// check that requires them would then fail on an unlucky seed roughly one run
/// in fourteen, which is a flaky control rather than a control.
pub(super) fn word() -> impl Strategy<Value = String> {
    prop_oneof![
        1 => Just(String::new()),
        1 => Just(EXPANSION_WITNESS.to_owned()),
        8 => prop::collection::vec(prop::sample::select(ALPHABET), 0..24).prop_map(|chars| {
            chars
                .into_iter()
                .filter(|character| !matches!(character, '\n' | '\r' | '\0'))
                .collect()
        }),
    ]
}

/// A string that is never empty, for cases where the empty word is not the
/// subject under test and would only add a degenerate case.
pub(super) fn non_empty_word() -> impl Strategy<Value = String> {
    word().prop_filter("the word must not be empty", |value| !value.is_empty())
}

/// Locate `sh`, or `None` when the host has no POSIX shell.
///
/// Homebrew's macOS `sh` lives outside the default `PATH` some CI runners set,
/// so the well-known absolute paths are tried after the `PATH` lookup.
pub(super) fn posix_shell() -> Option<Utf8PathBuf> {
    for candidate in ["/bin/sh", "/usr/bin/sh", "/usr/local/bin/sh"] {
        let path = Utf8PathBuf::from(candidate);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

/// Run `script` under a real POSIX shell and return its stdout.
pub(super) fn run_posix_shell(shell: &Utf8PathBuf, script: &str) -> Result<String> {
    let output = Command::new(shell.as_str())
        .arg("-c")
        .arg(script)
        .output()
        .with_context(|| format!("run {shell} -c {script}"))?;
    ensure!(
        output.status.success(),
        "{shell} exited with {} for {script}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).context("shell stdout should be valid UTF-8")
}

/// The rendered text of `template` under a stdlib environment with `dialect`.
///
/// The environment is registered through `stdlib::register_with_config`, so the
/// filters under test are the ones a manifest actually receives rather than a
/// re-registration of the same closures.
///
/// `subject` is bound to the template variables `value` and `values` rather
/// than spliced into the source. Binding under both names lets one helper serve
/// the scalar and the list filter; interpolating a Rust `{:?}` rendering would
/// work for the common case and break on exactly the inputs this file exists to
/// exercise — a word containing `\u{80}` or a backslash is not a valid Jinja
/// string escape, and the resulting diagnostic would be an escaping bug in the
/// test harness masquerading as one in the filter.
pub(super) fn render_with(template: &str, dialect: &str, subject: &Value) -> Result<String> {
    let base = StdlibConfig::from_current_dir()?;
    let config = match dialect {
        SH => base.with_recipe_shell(netsuke::recipe_shell::RecipeShell::Posix),
        POWERSHELL => base.with_recipe_shell(netsuke::recipe_shell::RecipeShell::PowerShell),
        other => bail!("unknown test dialect {other}"),
    };
    let mut env = Environment::new();
    netsuke::stdlib::register_with_config(&mut env, config)?;
    // The `?` converts `minijinja::Error` into the `anyhow::Error` this helper
    // returns, so it is load-bearing rather than a `needless_question_mark`.
    Ok(env.render_str(template, context! { value => subject, values => subject })?)
}

/// Render `{{ value | shell_quote }}` with an omitted dialect, for `dialect`.
pub(super) fn quote_value(value: &str, dialect: &str) -> Result<String> {
    render_with("{{ value | shell_quote }}", dialect, &Value::from(value))
}

/// Decode one PowerShell single-quoted literal written by the encoder.
///
/// This is a *decoder*, not a restatement of the encoder: it strips the
/// enclosing quotes and collapses each doubled quote. Writing it the other way
/// round — calling the encoder and comparing — would prove only that the
/// function is a function.
///
/// # Errors
///
/// Returns an error if `text` is not a single-quoted PowerShell literal.
pub(super) fn decode_power_shell_literal(text: &str) -> Result<String> {
    let inner = text
        .strip_prefix('\'')
        .and_then(|rest| rest.strip_suffix('\''))
        .with_context(|| format!("not a PowerShell single-quoted literal: {text:?}"))?;
    Ok(inner.replace("''", "'"))
}

/// The bytes a POSIX shell reads back, given the encoder's output.
///
/// `printf %s` writes its argument without a trailing newline, so the child's
/// stdout is the decoded word and nothing else. Passing the encoded text as the
/// script's argument rather than splicing it into the script keeps the harness
/// honest: the shell parses the word from argument position, which is exactly
/// where a recipe's word sits.
pub(super) fn decode_through_posix_shell(shell: &Utf8PathBuf, encoded: &str) -> Result<String> {
    let script = format!(r"printf %s {encoded}");
    run_posix_shell(shell, &script)
}

/// Split `text` as a POSIX shell would, using the same lexer the IR uses.
pub(super) fn split(text: &str) -> Result<Vec<String>> {
    shlex::split(text).with_context(|| format!("shlex could not split {text:?}"))
}

/// The POSIX encoder's output for `value`, read out of the filter itself.
pub(super) fn encoded_sh(value: &str) -> Result<String> {
    quote_value(value, SH)
}
