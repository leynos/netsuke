//! Integration coverage for early JSON preference resolution.

use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use metrics_util::debugging::DebuggingRecorder;
use netsuke::cli_localization;
use rstest::rstest;
use std::cell::Cell;
use std::{collections::HashMap, ffi::OsString, sync::Arc};
use tempfile::tempdir;

#[test]
fn resolve_merged_json_honours_injected_env() -> Result<()> {
    let temp_dir = tempdir().context("create temporary config directory")?;
    let config_path = temp_dir.path().join("netsuke.toml");
    let config_dir = Dir::open_ambient_dir(temp_dir.path(), ambient_authority())
        .context("open temporary config directory")?;
    config_dir
        .write("netsuke.toml", b"json = false\n")
        .context("write netsuke.toml")?;

    let localizer = Arc::from(cli_localization::build_localizer(None));
    let config_arg = config_path.to_string_lossy().into_owned();
    let (cli, matches) = netsuke::cli::parse_with_localizer_from(
        ["netsuke", "--config", &config_arg],
        &localizer,
        clap::ColorChoice::Auto,
    )
    .context("parse CLI args for injected JSON env")?;
    let env = TestEnv::default().with_var("NETSUKE_JSON", "1");

    ensure!(
        netsuke::cli::resolve_merged_json_with_env(&cli, &matches, &env)?,
        "injected NETSUKE_JSON should override file config",
    );

    Ok(())
}

/// Standalone merging discovers configuration without recording telemetry.
///
/// The application composition root owns discovery timing and metrics. This
/// public merge API must remain deterministic for library callers that inject
/// their environment without also opting into an observability dependency.
#[test]
fn standalone_merge_does_not_record_discovery_telemetry() -> Result<()> {
    let temp_dir = tempdir().context("create temporary config directory")?;
    let config_path = temp_dir.path().join("netsuke.toml");
    let config_dir = Dir::open_ambient_dir(temp_dir.path(), ambient_authority())
        .context("open temporary config directory")?;
    config_dir
        .write("netsuke.toml", b"jobs = 13\n")
        .context("write config")?;

    let localizer = Arc::from(cli_localization::build_localizer(None));
    let config_arg = config_path.to_string_lossy().into_owned();
    let (cli, matches) = netsuke::cli::parse_with_localizer_from(
        ["netsuke", "--config", config_arg.as_str()],
        &localizer,
        clap::ColorChoice::Auto,
    )
    .context("parse CLI args for standalone merge")?;
    let env = TestEnv::default();
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    let merged = metrics::with_local_recorder(&recorder, || {
        netsuke::cli::merge_with_config_and_env(&cli, &matches, &env)
    })?;

    ensure!(
        merged.jobs == Some(13),
        "standalone merge should retain the discovered config value"
    );
    ensure!(
        snapshotter.snapshot().into_vec().is_empty(),
        "standalone merge must not emit discovery telemetry"
    );
    Ok(())
}

/// Cached layers keep the file values found during diagnostic resolution.
#[test]
fn cached_merge_does_not_reload_discovered_config() -> Result<()> {
    let temp_dir = tempdir().context("create temporary config directory")?;
    let config_path = temp_dir.path().join("netsuke.toml");
    let config_dir = Dir::open_ambient_dir(temp_dir.path(), ambient_authority())
        .context("open temporary config directory")?;
    config_dir
        .write("netsuke.toml", b"jobs = 13\n")
        .context("write initial config")?;

    let localizer = Arc::from(cli_localization::build_localizer(None));
    let config_arg = config_path.to_string_lossy().into_owned();
    let (cli, matches) = netsuke::cli::parse_with_localizer_from(
        ["netsuke", "--config", config_arg.as_str()],
        &localizer,
        clap::ColorChoice::Auto,
    )
    .context("parse CLI args for cached merge")?;
    let env = TestEnv::default();
    let (json, outcome) =
        netsuke::cli::resolve_json_and_layers_outcome_with_env(&cli, &matches, &env);
    ensure!(!json?, "initial configuration should not enable JSON mode");

    config_dir
        .write("netsuke.toml", b"jobs = 29\n")
        .context("change config after discovery")?;
    let merged =
        netsuke::cli::merge_with_cached_file_layers(&cli, &matches, &env, outcome.into_layers())?;

    ensure!(
        merged.jobs == Some(13),
        "cached merge should retain the initially discovered file value"
    );
    Ok(())
}

/// The diagnostic pass and cached merge share one `NETSUKE_CONFIG` lookup.
///
/// Discovery happens once during diagnostic resolution; the subsequent merge
/// consumes the retained layers instead of re-reading the selector. This is
/// the single-lookup guarantee that avoids double environment I/O on startup.
#[test]
fn diag_then_cached_merge_reads_config_selector_once() -> Result<()> {
    let temp_dir = tempdir().context("create temporary config directory")?;
    let config_path = temp_dir.path().join("netsuke.toml");
    let config_dir = Dir::open_ambient_dir(temp_dir.path(), ambient_authority())
        .context("open temporary config directory")?;
    config_dir
        .write("netsuke.toml", b"jobs = 13\njson = true\n")
        .context("write config")?;

    let localizer = Arc::from(cli_localization::build_localizer(None));
    let (cli, matches) =
        netsuke::cli::parse_with_localizer_from(["netsuke"], &localizer, clap::ColorChoice::Auto)
            .context("parse CLI")?;
    let selector = config_path.as_os_str().to_owned();
    let env = CountingConfigEnv::default().with_var("NETSUKE_CONFIG", selector);

    let (json, outcome) =
        netsuke::cli::resolve_json_and_layers_outcome_with_env(&cli, &matches, &env);
    ensure!(
        json?,
        "the discovered file layer should enable JSON diagnostics"
    );
    let merged =
        netsuke::cli::merge_with_cached_file_layers(&cli, &matches, &env, outcome.into_layers())
            .context("merge the cached layers")?;

    ensure!(
        merged.jobs == Some(13) && merged.json,
        "the merge should consume the discovered config layer"
    );
    ensure!(
        env.config_lookups() == 1,
        "diagnostic resolution and cached merge must share one NETSUKE_CONFIG lookup, found {}",
        env.config_lookups()
    );
    Ok(())
}

#[derive(Default)]
struct TestEnv {
    values: HashMap<&'static str, OsString>,
}

impl netsuke::cli::ConfigEnvProvider for TestEnv {
    fn get(&self, key: &str) -> Option<OsString> {
        self.values.get(key).cloned()
    }

    /// Present the same snapshot the diagnostic pass saw.
    ///
    /// A provider whose `get` and `entries` disagree makes the early resolver
    /// and the full merge observe different environments, which would hide a
    /// disagreement between the two passes rather than test it.
    fn entries(&self) -> Vec<(OsString, OsString)> {
        self.values
            .iter()
            .map(|(key, value)| (OsString::from(*key), value.clone()))
            .collect()
    }
}

/// [`ConfigEnvProvider`] that counts `NETSUKE_CONFIG` selector lookups.
///
/// This lets the cached hand-off regression assert that diagnostic resolution
/// and the full merge observe exactly one selector read, so startup does not
/// pay a second environment lookup or file load.
#[derive(Default)]
struct CountingConfigEnv {
    values: HashMap<&'static str, OsString>,
    config_lookups: Cell<usize>,
}

impl CountingConfigEnv {
    fn with_var(mut self, name: &'static str, value: impl Into<OsString>) -> Self {
        self.values.insert(name, value.into());
        self
    }

    const fn config_lookups(&self) -> usize {
        self.config_lookups.get()
    }
}

impl netsuke::cli::ConfigEnvProvider for CountingConfigEnv {
    fn get(&self, key: &str) -> Option<OsString> {
        if key == "NETSUKE_CONFIG" {
            self.config_lookups.set(self.config_lookups.get() + 1);
        }
        self.values.get(key).cloned()
    }

    fn entries(&self) -> Vec<(OsString, OsString)> {
        Vec::new()
    }
}

impl TestEnv {
    fn with_var(mut self, name: &'static str, value: impl Into<OsString>) -> Self {
        self.values.insert(name, value.into());
        self
    }
}

/// Parse a CLI that selects `toml_content` as its explicit configuration file.
///
/// The returned [`tempfile::TempDir`] owns the file; keep it alive until the
/// diagnostic pass has discovered the layers.
fn parse_with_file(
    toml_content: &str,
) -> Result<(netsuke::cli::Cli, clap::ArgMatches, tempfile::TempDir)> {
    let temp_dir = tempdir().context("create temporary config directory")?;
    let config_path = temp_dir.path().join("netsuke.toml");
    let config_dir = Dir::open_ambient_dir(temp_dir.path(), ambient_authority())
        .context("open temporary config directory")?;
    config_dir
        .write("netsuke.toml", toml_content.as_bytes())
        .context("write netsuke.toml")?;

    let localizer = Arc::from(cli_localization::build_localizer(None));
    let config_arg = config_path.to_string_lossy().into_owned();
    let (cli, matches) = netsuke::cli::parse_with_localizer_from(
        ["netsuke", "--config", &config_arg],
        &localizer,
        clap::ColorChoice::Auto,
    )
    .context("parse CLI args for JSON boolean agreement")?;
    Ok((cli, matches, temp_dir))
}

/// Both passes accept exactly the four strict Boolean spellings.
#[rstest]
#[case::true_spelling("true", true)]
#[case::one("1", true)]
#[case::false_spelling("false", false)]
#[case::zero("0", false)]
fn diagnostic_and_merge_passes_agree_on_json_boolean_forms(
    #[case] raw: &str,
    #[case] expected: bool,
) -> Result<()> {
    let (cli, matches, _config) = parse_with_file("json = false\n")?;
    let env = TestEnv::default().with_var("NETSUKE_JSON", raw);

    let (json, outcome) =
        netsuke::cli::resolve_json_and_layers_outcome_with_env(&cli, &matches, &env);
    ensure!(
        json? == expected,
        "the diagnostic pass should read NETSUKE_JSON={raw:?} as {expected}",
    );

    let merged =
        netsuke::cli::merge_with_cached_file_layers(&cli, &matches, &env, outcome.into_layers())
            .context("merge the cached layers")?;
    ensure!(
        merged.json == expected,
        "the full merge should read NETSUKE_JSON={raw:?} as {expected}, got {}",
        merged.json,
    );
    Ok(())
}

/// Both passes reject the same non-Boolean spellings.
#[rstest]
#[case::capitalized_true("True")]
#[case::upper_true("TRUE")]
#[case::yes("yes")]
#[case::two("2")]
#[case::empty("")]
#[case::surrounded_by_spaces(" 1 ")]
#[case::trailing_space("1 ")]
fn diagnostic_and_merge_passes_reject_the_same_spellings(#[case] raw: &str) -> Result<()> {
    let (cli, matches, _config) = parse_with_file("json = false\n")?;
    let env = TestEnv::default().with_var("NETSUKE_JSON", raw);

    let (json, outcome) =
        netsuke::cli::resolve_json_and_layers_outcome_with_env(&cli, &matches, &env);
    let diagnostic_error = json.expect_err("the diagnostic pass should reject the spelling");
    ensure!(
        diagnostic_error
            .to_string()
            .contains("must be true, false, 1, or 0"),
        "the diagnostic rejection should name the accepted spellings, got: {diagnostic_error}",
    );

    let merge_error =
        netsuke::cli::merge_with_cached_file_layers(&cli, &matches, &env, outcome.into_layers())
            .expect_err("the full merge should reject the same spelling");
    ensure!(
        merge_error.to_string().contains("invalid type"),
        "the full merge should fail Boolean deserialization, got: {merge_error}",
    );
    Ok(())
}
