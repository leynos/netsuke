//! Contracts for manifest loads under the deny-by-default test surface.
//!
//! The unit rows in `restricted_helpers` observe the surface directly; this
//! module observes the same surface where a manifest author meets it, through
//! `from_str_with_options`. Three behaviours are pinned here.
//!
//! `env()` reads only the surface's own reader. With none supplied it answers
//! every lookup as absent without consulting the process, which is the property
//! that keeps a test load reproducible on a machine whose environment differs
//! from the author's.
//!
//! `now()` is available exactly when the caller injected a clock. The ambient
//! clock refuses, because an unrepeatable timestamp would make a test result
//! depend on when it ran; an explicitly injected clock — including
//! `system_clock()`, which is an injection even though it reads the host — is
//! admitted.
//!
//! A refusal raised inside `when`, directly or through a macro, fails the load
//! rather than marking the entry conditional. That classification belongs to
//! manifest query loads alone, so the test surface must not inherit it.

use std::error::Error as _;

use anyhow::{Context as _, Result, ensure};
use metrics_util::{MetricKind, debugging::DebuggingRecorder};
use minijinja::Error as MiniJinjaError;
use mockable::{DefaultEnv, Env};
use time::macros::datetime;

use crate::ast::Recipe;
use crate::manifest::{
    EnvAccessPolicy, EnvReadError, EnvReader, ManifestBudgetLimits, ManifestEnvironment,
    ManifestLoadOptions, ManifestName, TestSurface, from_str_with_options, process_env_reader,
};
use crate::stdlib::{StdlibConfig, fixed_clock, system_clock};

/// The instant the fixed-clock rows pin.
const FIXED_INSTANT: time::OffsetDateTime = datetime!(2026-06-08 12:00:00 UTC);

/// Open the stdlib configuration the test workspace supplies.
fn config() -> StdlibConfig {
    StdlibConfig::from_current_dir().expect("the test workspace must open")
}

/// Build a one-target manifest whose command interpolates `expression`.
fn command_manifest(expression: &str) -> String {
    format!(
        concat!(
            "netsuke_version: \"1.0.0\"\n",
            "targets:\n",
            "  - name: probe\n",
            "    command: \"echo {}\"\n",
        ),
        expression
    )
}

/// Load `yaml` under the surface `surface` describes.
fn load(yaml: &str, surface: TestSurface) -> Result<crate::ast::NetsukeManifest> {
    from_str_with_options(
        yaml,
        &ManifestName::new("Netsukefile"),
        ManifestLoadOptions::test(surface),
    )
}

/// Build the build-surface options the telemetry rows compare against.
fn full_options(reader: &EnvReader) -> ManifestLoadOptions<'_> {
    ManifestLoadOptions::full(
        config(),
        ManifestEnvironment::new(reader, EnvAccessPolicy::default()),
    )
}

/// Load `yaml` through the build registration.
fn load_full(yaml: &str) -> Result<crate::ast::NetsukeManifest> {
    let reader = process_env_reader();
    from_str_with_options(
        yaml,
        &ManifestName::new("Netsukefile"),
        full_options(&reader),
    )
}

/// Build a manifest that filters every foreach target.
fn filtered_manifest() -> String {
    concat!(
        "netsuke_version: \"1.0.0\"\n",
        "targets:\n",
        "  - name: \"out-{{ item }}\"\n",
        "    command: \"echo {{ item }}\"\n",
        "    foreach:\n",
        "      - alpha\n",
        "      - beta\n",
        "    when: \"false\"\n",
    )
    .to_owned()
}

/// Run `run` under a local recorder and return its counters' names.
fn recorded_counters<T>(run: impl FnOnce() -> T) -> (T, Vec<String>) {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let outcome = metrics::with_local_recorder(&recorder, run);
    let names = snapshotter
        .snapshot()
        .into_vec()
        .into_iter()
        .filter(|(key, ..)| key.kind() == MetricKind::Counter)
        .map(|(key, ..)| key.key().name().to_owned())
        .collect();
    (outcome, names)
}

/// The counters a test load must not contribute to.
const TRACKED_COUNTERS: [&str; 4] = [
    "netsuke_manifest_filtered_targets_total",
    "netsuke_manifest_filtered_actions_total",
    "netsuke_manifest_omitted_filtered_entries_total",
    "netsuke_manifest_budget_exhausted_total",
];

/// Return the tracked counters present in `names`.
fn tracked(names: &[String]) -> Vec<&str> {
    names
        .iter()
        .map(String::as_str)
        .filter(|name| TRACKED_COUNTERS.contains(name))
        .collect()
}

/// A test load expands and filters without emitting expansion telemetry.
#[test]
fn test_load_emits_no_expansion_telemetry() -> Result<()> {
    let yaml = filtered_manifest();
    let (test_result, test_counters) =
        recorded_counters(|| load(&yaml, TestSurface::new(config())));
    ensure!(
        test_result?.targets.is_empty(),
        "the probe is vacuous unless the load actually filters a target"
    );
    ensure!(
        tracked(&test_counters).is_empty(),
        "a test load recorded expansion telemetry: {test_counters:?}"
    );

    let (full_result, full_counters) = recorded_counters(|| load_full(&yaml));
    ensure!(
        full_result?.targets.is_empty(),
        "the probe must filter in both modes"
    );
    ensure!(
        tracked(&full_counters).contains(&"netsuke_manifest_filtered_targets_total"),
        "the build load recorded no expansion telemetry, so the comparison is vacuous: \
         {full_counters:?}"
    );
    Ok(())
}

/// A test load that exhausts its budget records no budget telemetry.
#[test]
fn test_load_emits_no_budget_telemetry_when_exhausted() -> Result<()> {
    let yaml = command_manifest("{{ 1 }}");
    let limits = ManifestBudgetLimits {
        source_bytes: 8,
        ..ManifestBudgetLimits::default()
    };
    let (test_result, test_counters) = recorded_counters(|| {
        from_str_with_options(
            &yaml,
            &ManifestName::new("Netsukefile"),
            ManifestLoadOptions::test(TestSurface::new(config())).with_budget_limits(limits),
        )
    });
    ensure!(
        test_result.is_err(),
        "the probe is vacuous unless the budget exhausts"
    );
    ensure!(
        tracked(&test_counters).is_empty(),
        "an exhausted test load recorded telemetry: {test_counters:?}"
    );

    let reader = process_env_reader();
    let (full_result, full_counters) = recorded_counters(|| {
        from_str_with_options(
            &yaml,
            &ManifestName::new("Netsukefile"),
            full_options(&reader).with_budget_limits(limits),
        )
    });
    ensure!(
        full_result.is_err(),
        "the budget must exhaust in both modes"
    );
    ensure!(
        tracked(&full_counters).contains(&"netsuke_manifest_budget_exhausted_total"),
        "the build load recorded no budget telemetry, so the comparison is vacuous: \
         {full_counters:?}"
    );
    Ok(())
}

/// Return the single rendered command of a loaded probe manifest.
fn rendered_command(manifest: &crate::ast::NetsukeManifest) -> Result<&str> {
    let target = manifest
        .targets
        .first()
        .context("the probe has one target")?;
    match &target.recipe {
        Recipe::Command { command, .. } => command
            .as_single()
            .context("the probe command must render to one string"),
        other => anyhow::bail!("the probe target is not a command: {other:?}"),
    }
}

/// Return the operation named by a test-mode refusal in an error chain.
fn refusal_operation(error: &anyhow::Error) -> Option<&str> {
    error.chain().find_map(|cause| {
        let jinja = cause.downcast_ref::<MiniJinjaError>()?;
        jinja
            .source()?
            .downcast_ref::<crate::stdlib::TestModeRefusal>()
            .map(crate::stdlib::TestModeRefusal::operation)
    })
}

/// A test load with no reader treats the process environment as absent.
#[test]
fn test_load_without_a_reader_reads_no_process_environment() -> Result<()> {
    ensure!(
        DefaultEnv.raw("PATH").is_ok(),
        "the probe is vacuous unless PATH is set in the test process"
    );

    let manifest = load(
        &command_manifest("{{ env('PATH', default='unset') }}"),
        TestSurface::new(config()),
    )?;

    ensure!(
        rendered_command(&manifest)? == "echo unset",
        "the surface consulted the process environment: {}",
        rendered_command(&manifest)?
    );
    Ok(())
}

/// A surface reader supplies the value `env()` returns.
#[test]
fn test_load_reads_the_supplied_surface_reader() -> Result<()> {
    let reader: EnvReader = std::sync::Arc::new(|name: &str| {
        if name == "PROFILE" {
            Ok("from-reader".to_owned())
        } else {
            Err(EnvReadError::NotPresent)
        }
    });

    let manifest = load(
        &command_manifest("{{ env('PROFILE', default='unset') }}"),
        TestSurface::new(config()).with_env_reader(reader),
    )?;

    ensure!(
        rendered_command(&manifest)? == "echo from-reader",
        "the surface ignored its reader: {}",
        rendered_command(&manifest)?
    );
    Ok(())
}

/// An injected fixed clock is what `now()` reports.
#[test]
fn test_load_renders_now_from_an_injected_fixed_clock() -> Result<()> {
    let manifest = load(
        &command_manifest("{{ now() }}"),
        TestSurface::new(config().with_clock(fixed_clock(FIXED_INSTANT))),
    )?;

    ensure!(
        rendered_command(&manifest)? == "echo 2026-06-08T12:00:00Z",
        "the injected clock was not used: {}",
        rendered_command(&manifest)?
    );
    Ok(())
}

/// Explicitly injecting the system clock keeps `now()` real.
#[test]
fn test_load_admits_now_when_the_system_clock_is_injected() -> Result<()> {
    let manifest = load(
        &command_manifest("{{ now() }}"),
        TestSurface::new(config().with_clock(system_clock())),
    )?;

    let rendered = rendered_command(&manifest)?
        .strip_prefix("echo ")
        .context("the probe rendered a timestamp")?;
    time::OffsetDateTime::parse(rendered, &time::format_description::well_known::Rfc3339)
        .with_context(|| {
            format!("the injected system clock did not render an instant: {rendered}")
        })?;
    Ok(())
}

/// The ambient clock refuses, naming `now` in a typed refusal.
#[test]
fn test_load_refuses_now_with_the_default_clock() -> Result<()> {
    let error = load(&command_manifest("{{ now() }}"), TestSurface::new(config()))
        .expect_err("the ambient clock must refuse under test");

    ensure!(
        refusal_operation(&error) == Some("now"),
        "the ambient clock did not produce a typed `now` refusal: {error:#}"
    );
    Ok(())
}

/// Build a manifest whose `when` calls a refused helper directly.
fn when_manifest(condition: &str, macros: &str) -> String {
    format!(
        concat!(
            "netsuke_version: \"1.0.0\"\n",
            "{macros}",
            "targets:\n",
            "  - name: probe\n",
            "    command: \"echo probe\"\n",
            "    when: \"{condition}\"\n",
        ),
        macros = macros,
        condition = condition
    )
}

/// A refusal in `when` fails the load instead of marking it conditional.
#[test]
fn a_refusal_inside_when_fails_the_load() -> Result<()> {
    let yaml = when_manifest("command_available('rustc')", "");
    let error = load(&yaml, TestSurface::new(config()))
        .expect_err("a refused helper in `when` must fail the load");

    ensure!(
        refusal_operation(&error) == Some("command_available"),
        "the `when` failure was not a typed refusal: {error:#}"
    );
    Ok(())
}

/// A refusal reached through a macro called from `when` also fails the load.
#[test]
fn a_refusal_inside_a_macro_called_from_when_fails_the_load() -> Result<()> {
    let macros = concat!(
        "macros:\n",
        "  - signature: \"leaky()\"\n",
        "    body: \"{{ fetch('https://example.invalid/probe') }}\"\n",
    );
    let yaml = when_manifest("leaky()", macros);
    let error = load(&yaml, TestSurface::new(config()))
        .expect_err("a refused helper inside a macro must fail the load");

    ensure!(
        refusal_operation(&error) == Some("fetch"),
        "the macro failure was not a typed refusal: {error:#}"
    );
    Ok(())
}
