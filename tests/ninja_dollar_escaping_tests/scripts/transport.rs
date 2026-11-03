//! Byte-preservation cases for the POSIX `printf %b` script transport.
//!
//! Each case runs a generated Ninja target under real Ninja and asserts that
//! script data reaches the final interpreter unchanged, and that no injected
//! redirect ran. Everything here is Unix-only because the generated wrapper
//! is a POSIX shell command.

// Every case in this module is Unix-only, so the parent imports are too.
// Leaving this ungated makes the import unused on Windows, where `-D warnings`
// turns that into a build failure.
#[cfg(unix)]
use super::*;
#[cfg(unix)]
use netsuke::manifest::{EnvAccessPolicy, EnvReadError, EnvReader};
#[cfg(unix)]
use std::fmt::Display;
#[cfg(unix)]
use std::sync::Arc;

/// The literal byte sequence the reported trigger carries.
///
/// It begins with a backslash so `printf %b` reads `\047` as an octal
/// apostrophe; the remainder turns the reconstructed byte into a command
/// boundary and a redirect to `injected`. Only the Unix execution cases below
/// consume it, so it is gated with them rather than left dead on Windows.
#[cfg(unix)]
const BACKSLASH_PAYLOAD: &str = r"\047;>injected;#";

/// Generate and execute a one-target manifest whose script writes `out`.
#[cfg(unix)]
fn run_transported_script(
    manifest: &netsuke::ast::NetsukeManifest,
    input: Option<(&str, &str)>,
) -> Result<(Vec<u8>, NinjaWorkspace)> {
    let ninja = generate_posix(&BuildGraph::from_manifest_for_shell(
        manifest,
        RecipeShell::Posix,
    )?)?;
    let workspace = run_ninja_workspace(&ninja, &[], input)?;
    let bytes = test_support::fs::read(workspace.path.join("out"))
        .context("read byte-exact script output")?;
    Ok((bytes, workspace))
}

/// The script content a transport assertion covers.
#[cfg(unix)]
#[derive(Clone, Copy)]
enum TransportSubject {
    /// Bytes written by a script literal.
    ScriptBytes,
    /// A path inserted through a marker.
    InputPath,
    /// A `shell_quote`d `env()` fallback.
    EnvFallback,
    /// A quoted-heredoc body.
    HeredocBytes,
}

#[cfg(unix)]
impl TransportSubject {
    /// Return the wording used in assertion diagnostics.
    const fn label(self) -> &'static str {
        match self {
            Self::ScriptBytes => "script bytes",
            Self::InputPath => "input path",
            Self::EnvFallback => "env fallback",
            Self::HeredocBytes => "heredoc bytes",
        }
    }
}

/// Assert one quoted-heredoc body crosses the transport byte for byte.
#[cfg(unix)]
fn assert_quoted_heredoc_preserves(body: impl Display) -> Result<()> {
    let source = format!(
        "netsuke_version: '1.0.0'\ntargets:\n  - name: out\n    script: |\n      cat > {{{{ outs }}}} <<'EOF'\n      {body}\n      EOF\n"
    );
    let manifest = manifest::from_str(&source)?;
    let (actual, workspace) = run_transported_script(&manifest, None)?;
    let expected = format!("{body}\n");
    assert_transport_preserved(
        &actual,
        &workspace,
        expected.as_bytes(),
        TransportSubject::HeredocBytes,
    )
}

/// Assert the transport kept `expected` and executed no injected redirect.
#[cfg(unix)]
fn assert_transport_preserved(
    actual: &[u8],
    workspace: &NinjaWorkspace,
    expected: &[u8],
    subject: TransportSubject,
) -> Result<()> {
    ensure!(
        actual == expected,
        "{} changed in transport: expected {:?}, got {:?}",
        subject.label(),
        String::from_utf8_lossy(expected),
        String::from_utf8_lossy(actual)
    );
    ensure!(
        !test_support::fs::exists(workspace.path.join("injected")),
        "{} executed the injected redirect",
        subject.label()
    );
    Ok(())
}

/// Verify a script literal survives `printf %b` without becoming shell syntax.
#[cfg(unix)]
#[rstest]
fn script_source_backslash_payload_is_not_reinterpreted() -> Result<()> {
    let source = format!(
        "netsuke_version: '1.0.0'\ntargets:\n  - name: out\n    script: |\n      printf '%s' '{BACKSLASH_PAYLOAD}' > {{{{ outs }}}}\n"
    );
    let manifest = manifest::from_str(&source)?;
    let (actual, workspace) = run_transported_script(&manifest, None)?;
    assert_transport_preserved(
        &actual,
        &workspace,
        BACKSLASH_PAYLOAD.as_bytes(),
        TransportSubject::ScriptBytes,
    )
}

/// Verify an input path survives each marker quoting context.
#[cfg(unix)]
#[rstest]
#[case::single_quoted("printf '%s' '{{ ins }}' > {{ outs }}")]
#[case::unquoted("printf '%s' {{ ins }} > {{ outs }}")]
#[case::double_quoted("printf '%s' \"{{ ins }}\" > {{ outs }}")]
fn input_path_backslash_payload_is_not_reinterpreted(#[case] script_line: &str) -> Result<()> {
    let source = format!(
        "netsuke_version: '1.0.0'\ntargets:\n  - name: out\n    sources: '{BACKSLASH_PAYLOAD}'\n    script: |\n      {script_line}\n"
    );
    let manifest = manifest::from_str(&source)?;
    let (actual, workspace) =
        run_transported_script(&manifest, Some((BACKSLASH_PAYLOAD, "input-contents")))?;
    assert_transport_preserved(
        &actual,
        &workspace,
        BACKSLASH_PAYLOAD.as_bytes(),
        TransportSubject::InputPath,
    )
}

/// Verify a `shell_quote`d `env()` fallback reaches the final interpreter intact.
#[cfg(unix)]
#[rstest]
fn env_fallback_through_shell_quote_is_not_reinterpreted() -> Result<()> {
    // The fallback is a Jinja string literal, so it needs one extra layer of
    // backslash escaping before `shell_quote` sees the reported payload. This
    // is template-language escaping, not part of the script transport.
    let jinja_payload = BACKSLASH_PAYLOAD.replace('\\', "\\\\");
    let source = format!(
        "netsuke_version: '1.0.0'\ntargets:\n  - name: out\n    script: |\n      printf '%s\\n' {{{{ env('NETSUKE_ISSUE_967_UNSET', default='{jinja_payload}') | shell_quote }}}} > {{{{ outs }}}}\n"
    );
    let reader: EnvReader = Arc::new(|_| Err(EnvReadError::NotPresent));
    let manifest =
        manifest::from_str_with_env_and_policy(&source, &reader, &EnvAccessPolicy::default())?;
    let (actual, workspace) = run_transported_script(&manifest, None)?;
    let expected = format!("{BACKSLASH_PAYLOAD}\n");
    assert_transport_preserved(
        &actual,
        &workspace,
        expected.as_bytes(),
        TransportSubject::EnvFallback,
    )
}

/// Return printable ASCII with every Jinja delimiter pair removed.
#[cfg(unix)]
fn printable_ascii_without_jinja_delimiters() -> String {
    let mut text = (0x20u8..=0x7e).map(char::from).collect::<String>();
    for delimiter in ["{{", "}}", "{%", "%}", "{#", "#}"] {
        while let Some(index) = text.find(delimiter) {
            text.replace_range(index..index + delimiter.len(), "");
        }
    }
    text
}

/// Verify a quoted heredoc carries every backslash sequence byte for byte.
#[cfg(unix)]
#[rstest]
#[case::double_backslash(r"\\")]
#[case::literal_backslash_n(r"\n")]
#[case::literal_backslash_t(r"\t")]
#[case::literal_backslash_c(r"\c")]
#[case::literal_backslash_zero(r"\0")]
#[case::octal(r"\0101")]
#[case::hexadecimal(r"\x41")]
#[case::octal_apostrophe(r"\047")]
#[case::unicode(concat!(r"\", "u0041"))]
#[case::trailing_backslash("trailing\\")]
#[case::before_apostrophe(r"\'")]
#[case::before_double_quote(r#"\""#)]
#[case::before_backtick(r"\`")]
#[case::before_dollar(r"\$")]
fn quoted_heredoc_preserves_backslash_sequences(#[case] body: &str) -> Result<()> {
    assert_quoted_heredoc_preserves(body)
}

/// Verify the printable ASCII range survives the quoted-heredoc transport.
#[cfg(unix)]
#[rstest]
fn quoted_heredoc_preserves_printable_ascii() -> Result<()> {
    assert_quoted_heredoc_preserves(printable_ascii_without_jinja_delimiters())
}
