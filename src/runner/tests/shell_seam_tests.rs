//! The runner's resolved interpreter reaches the recipe-text filters.
//!
//! `StdlibConfig::with_recipe_shell` stores a dialect, and
//! `stdlib::recipe_text` reads it — but neither half shows that the runner
//! ever passes the resolved interpreter *down*. A build that resolves
//! `NETSUKE_WINDOWS_SHELL=bash` and then quotes its recipes for PowerShell
//! emits syntactically valid text for the wrong shell, which is the
//! silent-corruption path this milestone exists to close.
//!
//! These cases drive the build-manifest loader with an explicitly chosen
//! interpreter and assert on what the filters rendered. The distinguishing
//! evidence is that *changing the shell changes the output*: a case fixed on
//! one dialect would pass with the plumbing deleted, because that dialect is
//! what the default already produces.
//!
//! They live in the crate rather than in `tests/` because the seam is
//! crate-private — `ManifestLoadInputs` is `pub(crate)` — and because
//! `RecipeShell::host_default` returns the *host* interpreter, which on Linux
//! is always `Posix`, so an external test could not vary the shell at all.

use super::*;

/// Render `template` as a target description through the build-manifest loader.
///
/// Uses [`generation::load_manifest_for_build_with_limits`], the same entry
/// point `load_manifest_with_stage_reporting` calls, so the path from resolved
/// interpreter to rendered text is the runner's real one.
fn render_description_with_shell(shell: RecipeShell, template: &str) -> Result<String> {
    let manifest = format!(
        concat!(
            "netsuke_version: \"1.0.0\"\n",
            "targets:\n",
            "  - name: seam\n",
            "    description: \"{}\"\n",
            "    command: echo seam\n",
        ),
        template
    );
    let (_temp, manifest_path) = write_manifest(&manifest)?;
    let inputs = generation::ManifestLoadInputs::from_cli(&Cli::default(), shell)?;
    let loaded =
        generation::load_manifest_for_build_with_limits(&manifest_path, &inputs, None)?;
    Ok(loaded
        .targets
        .first()
        .and_then(|target| target.description.clone())
        .unwrap_or_default())
}

/// A template whose two filters quote differently per dialect.
///
/// `a b` needs quoting under both dialects but with different characters, and
/// a single quote inside the word is what separates them sharply: `sh` closes,
/// escapes and reopens the quote, while PowerShell doubles it.
const DIALECT_SENSITIVE_TEMPLATE: &str = "{{ 'a b' | shell_quote(dialect='sh') }}";

/// The interpreter the runner resolved reaches the filters.
///
/// `sh` is requested explicitly, so this also shows `dialect='sh'` is accepted
/// on the build surface. Passing `PowerShell` must produce a *different*
/// rendering for the same template, which is the part a deleted plumbing
/// cannot satisfy.
#[rstest]
#[case(RecipeShell::Posix)]
#[case(RecipeShell::Bash)]
fn build_loader_quotes_for_the_resolved_posix_shell(#[case] shell: RecipeShell) -> Result<()> {
    let rendered = render_description_with_shell(shell, DIALECT_SENSITIVE_TEMPLATE)?;
    ensure!(
        rendered == "a\x27 b\x27",
        "expected `sh` quoting for {shell:?}, got {rendered:?}"
    );
    Ok(())
}

/// `Posix` and `Bash` both quote as `sh`, so the two render identically.
///
/// This is the three-to-two surjection `RecipeShell::dialect` documents,
/// observed through the runner rather than asserted on the type.
#[rstest]
fn bash_and_posix_render_identically_through_the_loader() -> Result<()> {
    let posix = render_description_with_shell(RecipeShell::Posix, DIALECT_SENSITIVE_TEMPLATE)?;
    let bash = render_description_with_shell(RecipeShell::Bash, DIALECT_SENSITIVE_TEMPLATE)?;
    ensure!(
        posix == bash,
        "Posix rendered {posix:?} but Bash rendered {bash:?}"
    );
    Ok(())
}

/// An omitted `dialect` follows the interpreter the loader was given.
///
/// This is the load-bearing case. It is the only assertion here that fails if
/// the runner stops passing its resolved shell down while the config seam
/// survives, because every other case names its dialect explicitly and so
/// renders the same text regardless of the default. On this host the two
/// loaders must disagree — `Posix` quotes as `sh`, `PowerShell` does not — so
/// reverted plumbing shows up as two identical renderings.
#[rstest]
fn omitted_dialect_follows_the_loader_shell() -> Result<()> {
    let template = "{{ 'a b' | shell_quote }}";
    let posix = render_description_with_shell(RecipeShell::Posix, template)?;
    let windows = render_description_with_shell(RecipeShell::PowerShell, template)?;
    ensure!(
        posix != windows,
        "the default dialect did not follow the loader; both rendered {posix:?}"
    );
    ensure!(
        posix == "a\x27 b\x27",
        "expected the `sh` default to suffix-quote, got {posix:?}"
    );
    Ok(())
}

/// `shell_join` reaches the runner seam too, not only `shell_quote`.
#[rstest]
fn shell_join_is_registered_on_the_build_surface() -> Result<()> {
    let rendered = render_description_with_shell(
        RecipeShell::Posix,
        "{{ ['-C', 'target-cpu=native'] | shell_join(dialect='sh') }}",
    )?;
    ensure!(
        rendered == "-C target-cpu\x27=native\x27",
        "expected joined `sh` words, got {rendered:?}"
    );
    Ok(())
}

/// A bad dialect is reported with its machine-readable code, at this seam too.
///
/// The code lives in the catalogue text, so its presence here shows the
/// diagnostics the filters raise survive the loader's error wrapping.
#[rstest]
fn an_unknown_dialect_is_rejected_with_its_code() -> Result<()> {
    let error = render_description_with_shell(
        RecipeShell::Posix,
        "{{ 'a b' | shell_quote(dialect='cmd') }}",
    )
    .expect_err("an unknown dialect must fail the load");
    let rendered = format!("{error:#}");
    ensure!(
        rendered.contains("netsuke::jinja::shell::args"),
        "expected the shell args code in: {rendered}"
    );
    Ok(())
}
