//! OBL-NO-ESCAPE and OBL-CONTEXT: the quoter's bytes survive the real pipeline.
//!
//! Everywhere else in these suites the filter is called directly. These two
//! obligations go through the whole pipeline instead — a manifest on disk, the
//! real loader, the real renderer — because the hazard they name lives in the
//! layers around the filter rather than in it: an auto-escaping environment
//! would rewrite `&` to `&amp;`, and a template whose interpolation sits inside
//! `"..."` hands the shell the quoter's own quote characters as *data*.
//! [`an_auto_escaping_environment_rewrites_the_same_template`] and
//! [`the_two_interpolation_positions_differ`] are the controls that show the
//! assertions can fail.
use super::property_support::{SH, quote_value, render_with};
// The interpreter lookups are reachable only from the two `#[cfg(unix)]` cases
// at the foot of this module, so an unconditional import is an unused-import
// error on Windows, where `-D warnings` is a merge gate.
#[cfg(unix)]
use super::property_support::{posix_shell, run_posix_shell};
use anyhow::{Context, Result, ensure};
use minijinja::{AutoEscape, Environment, context, value::Value};
use netsuke::manifest::{
    self, EnvAccessPolicy, EnvReader, ManifestBudgetLimits, ManifestEnvironment,
};
use netsuke::stdlib::{NetworkPolicy, StdlibConfig};
use rstest::rstest;

// ---------------------------------------------------------------------------
// OBL-NO-ESCAPE
// ---------------------------------------------------------------------------

/// Write a one-target manifest binding `value` as the template variable
/// `seam_value`, and return the workspace holding it.
///
/// The value travels in a YAML block scalar rather than being spliced into the
/// template source or quoted as a YAML double-quoted scalar. Both of those
/// alternatives reintroduce the escaping problem this file is about: a value
/// containing `"` or `\` has to be re-escaped for YAML, and a value containing
/// `{{` would be taken as template syntax. A block scalar is literal, so the
/// subject reaches the filter byte for byte.
fn workspace_with_bound_value(value: &str, body: &str) -> Result<tempfile::TempDir> {
    let workspace = tempfile::tempdir().context("create manifest workspace")?;
    let manifest_path = workspace.path().join("Netsukefile");
    let manifest = format!(
        "netsuke_version: \"1.0.0\"\nvars:\n  seam_value: |-\n    {value}\n{body}",
        value = value.replace('\n', "\n    "),
    );
    test_support::fs::write(&manifest_path, manifest).context("write manifest")?;
    Ok(workspace)
}

/// The span the manifest loader rendered for `{{ seam_value | shell_quote }}`.
fn rendered_description(value: &str) -> Result<String> {
    let workspace = workspace_with_bound_value(
        value,
        concat!(
            "targets:\n",
            "  - name: seam\n",
            "    description: \"{{ seam_value | shell_quote(dialect='sh') }}\"\n",
            "    command: echo seam\n",
        ),
    )?;
    let manifest_path = workspace.path().join("Netsukefile");
    let reader: EnvReader = netsuke::manifest::process_env_reader();
    let environment = ManifestEnvironment::new(&reader, EnvAccessPolicy::default());
    let loaded = manifest::from_path_with_policy_and_environment_and_limits(
        &manifest_path,
        NetworkPolicy::default(),
        &environment,
        ManifestBudgetLimits::default(),
        netsuke::recipe_shell::RecipeShell::Posix,
        None,
    )?;
    Ok(loaded
        .targets
        .first()
        .and_then(|target| target.description.clone())
        .unwrap_or_default())
}

/// The quoter's bytes survive the real manifest loader unchanged.
///
/// The comparison is against `shell_quote`'s own output for the same input, so
/// the assertion fails if either the filter or the loader changes. An escaped
/// `&amp;` inside a recipe would be a silent corruption of exactly the text
/// this feature exists to protect.
#[rstest]
#[case("&")]
#[case("<")]
#[case(">")]
#[case("\"")]
#[case("'")]
#[case("a & b")]
fn quoted_output_survives_manifest_rendering_verbatim(#[case] value: &str) -> Result<()> {
    let expected = quote_value(value, SH)?;
    let observed = rendered_description(value)?;
    ensure!(
        observed == expected,
        "the loader rendered {observed:?} but the quoter produced {expected:?}"
    );
    for escaped in ["&amp;", "&lt;", "&gt;", "&quot;", "&#x27;"] {
        ensure!(
            !observed.contains(escaped),
            "the loader HTML-escaped the quoter's output: {observed}"
        );
    }
    Ok(())
}

/// An auto-escaping environment *does* rewrite the same template.
///
/// This is the negative control that proves the assertion above can detect
/// escaping. The manifest pipeline renders through an unnamed template, so
/// `MiniJinja`'s default callback leaves `<` alone; forcing the callback on shows
/// what the assertion would have caught, and confirms the escape route exists
/// rather than having been removed in some future version.
#[test]
fn an_auto_escaping_environment_rewrites_the_same_template() -> Result<()> {
    let value = "<a & b>";
    let template = "{{ value | shell_quote(dialect='sh') }}";

    let plain = render_with(template, SH, &Value::from(value))?;
    ensure!(
        plain.contains('<'),
        "without auto-escaping the raw character should survive: {plain}"
    );

    let config = StdlibConfig::from_current_dir()?;
    let mut escaping = Environment::new();
    netsuke::stdlib::register_with_config(&mut escaping, config)?;
    escaping.set_auto_escape_callback(|_| AutoEscape::Html);
    let escaped = escaping.render_str(template, context! { value => value })?;
    ensure!(
        escaped != plain,
        "forcing auto-escaping changed nothing, so the control proves nothing"
    );
    ensure!(
        escaped.contains("&lt;") && escaped.contains("&amp;"),
        "the escaping environment should rewrite the significant characters: {escaped}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// OBL-CONTEXT
// ---------------------------------------------------------------------------

/// The generated Ninja `command =` line for a one-target manifest.
///
/// `value` is bound as the template variable `seam_value`; `template` is the
/// command text, which interpolates it.
fn generated_command(value: &str, template: &str) -> Result<String> {
    let workspace = workspace_with_bound_value(
        value,
        &format!(
            concat!(
                "targets:\n",
                "  - name: out\n",
                "    command: \"{template}\"\n",
                "    description: seam\n",
            ),
            template = template.replace('\\', "\\\\").replace('"', "\\\""),
        ),
    )?;
    let manifest_path = workspace.path().join("Netsukefile");
    let run = test_support::netsuke::run_netsuke_in(
        workspace.path(),
        &[
            "--file",
            manifest_path
                .to_str()
                .context("manifest path should be UTF-8")?,
            "generate",
            "--output",
            "out.ninja",
        ],
    )?;
    ensure!(run.success, "generation failed: {}", run.stderr);
    let ninja = test_support::fs::read_to_string(workspace.path().join("out.ninja"))
        .context("read generated Ninja")?;
    ninja
        .lines()
        .find_map(|line| line.strip_prefix("  command = ").map(str::to_owned))
        .context("generated Ninja should carry a command binding")
}

/// The value the OBL-CONTEXT cases quote: a space and an `=` inside one word.
const CONTEXT_VALUE: &str = "RUSTFLAGS=-D warnings";

/// A filter in unquoted argv position produces text the shell reads correctly.
#[cfg(unix)]
#[rstest]
fn a_filter_in_unquoted_position_yields_one_argument() -> Result<()> {
    let command = generated_command(
        CONTEXT_VALUE,
        "printf '%s\\n' {{ seam_value | shell_quote(dialect='sh') }}",
    )?;

    let shell = posix_shell().context("a POSIX shell is required for this case")?;
    let decoded = run_posix_shell(&shell, &command)?;
    ensure!(
        decoded == format!("{CONTEXT_VALUE}\n"),
        "the shell read {decoded:?} from {command:?}, expected one argument"
    );
    ensure!(
        !command.contains("\"RUSTFLAGS"),
        "the interpolated word must not be enclosed in shell double quotes: {command}"
    );
    Ok(())
}

/// The same filter inside `"..."` is a manifest defect, pinned as such.
///
/// The first draft of the plan's acceptance transcript placed the interpolation
/// inside a pair of double quotes. There the quoter's own quote characters are
/// *data*, and the shell hands the recipe a corrupted argument — the failure
/// mode this obligation exists to name. The case is here so that a future
/// change which silently "fixes" it has to confront the documented answer.
#[cfg(unix)]
#[rstest]
fn a_filter_inside_double_quotes_corrupts_the_argument() -> Result<()> {
    let command = generated_command(
        CONTEXT_VALUE,
        "printf '%s\\n' \"{{ seam_value | shell_quote(dialect='sh') }}\"",
    )?;

    let shell = posix_shell().context("a POSIX shell is required for this case")?;
    let decoded = run_posix_shell(&shell, &command)?;
    ensure!(
        decoded != format!("{CONTEXT_VALUE}\n"),
        "the double-quoted position unexpectedly produced the right argument: {command}"
    );
    ensure!(
        decoded.contains('\'') || decoded.contains('"'),
        "the corruption should be the quoter's literal quote characters: {decoded:?}"
    );
    Ok(())
}

/// The two positions must not produce the same command.
///
/// Both cases above ask the same question of two templates. If the two ever
/// rendered identically, one of the two would be measuring nothing.
#[test]
fn the_two_interpolation_positions_differ() -> Result<()> {
    let unquoted = generated_command(
        CONTEXT_VALUE,
        "printf '%s\\n' {{ seam_value | shell_quote(dialect='sh') }}",
    )?;
    let quoted = generated_command(
        CONTEXT_VALUE,
        "printf '%s\\n' \"{{ seam_value | shell_quote(dialect='sh') }}\"",
    )?;
    ensure!(
        unquoted != quoted,
        "the two positions produced the same command, so the pair proves nothing"
    );
    Ok(())
}
