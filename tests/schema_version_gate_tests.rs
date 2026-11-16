//! Integration tests for reader-side schema version admission.

use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use cap_std::{ambient_authority, fs_utf8::Dir};
use netsuke::ast::NetsukeManifest;
use netsuke::manifest::{self, EnvReader, ManifestLoadStage};
use netsuke::stdlib::NetworkPolicy;
use proptest::prelude::*;
use rstest::rstest;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

const SCHEMA_DIAGNOSTIC: &str = "netsuke::manifest::schema_version";
const SENTINEL: &str = "NETSUKE_SENTINEL";

struct LoadProbe {
    result: Result<NetsukeManifest>,
    stages: Vec<ManifestLoadStage>,
    environment_reads: Arc<AtomicUsize>,
}

#[derive(Clone, Debug)]
enum SchemaDeclaration {
    Missing,
    NonString,
    Invalid,
    Unsupported(u64),
}

/// Generate a rejected schema declaration for property tests.
fn rejected_declaration() -> impl Strategy<Value = SchemaDeclaration> {
    prop_oneof![
        Just(SchemaDeclaration::Missing),
        Just(SchemaDeclaration::NonString),
        Just(SchemaDeclaration::Invalid),
        (1_u64..=16).prop_map(SchemaDeclaration::Unsupported),
    ]
}

/// Generate a bounded set of unknown manifest fields.
fn extra_unknown_keys() -> impl Strategy<Value = Vec<String>> {
    prop::collection::btree_set("x_[a-z][a-z0-9_]{0,10}", 0..=3)
        .prop_map(|keys| keys.into_iter().collect())
}

/// Render the YAML field for one schema declaration.
fn schema_field(declaration: &SchemaDeclaration) -> &'static str {
    match declaration {
        SchemaDeclaration::Missing | SchemaDeclaration::Unsupported(_) => "",
        SchemaDeclaration::NonString => "netsuke_version: 7\n",
        SchemaDeclaration::Invalid => "netsuke_version: 'not-a-version'\n",
    }
}

/// Build a manifest with a schema declaration and optional unknown fields.
fn manifest_with_declaration(declaration: &SchemaDeclaration, unknown_keys: &[String]) -> String {
    let schema = match declaration {
        SchemaDeclaration::Unsupported(minor) => {
            format!("netsuke_version: '1.{minor}.0'\n")
        }
        _ => schema_field(declaration).to_owned(),
    };
    let mut extras = String::new();
    for key in unknown_keys {
        extras.push_str(key);
        extras.push_str(": true\n");
    }
    format!(
        concat!(
            "{schema}{extras}",
            "targets:\n",
            "  - name: 'out-{{{{ item }}}}.txt'\n",
            "    foreach: \"[env('{sentinel}')]\"\n",
            "    command: \"echo {{{{ env('{sentinel}') }}}}\"\n",
        ),
        schema = schema,
        extras = extras,
        sentinel = SENTINEL,
    )
}

/// Load YAML in a temporary workspace and record pre-evaluation activity.
///
/// # Errors
///
/// Returns an error if the workspace cannot be created, opened or populated.
fn load_path(yaml: &str) -> Result<LoadProbe> {
    let workspace = tempfile::tempdir().context("create schema-version workspace")?;
    let workspace_path = Utf8Path::from_path(workspace.path())
        .context("schema-version workspace path should be UTF-8")?
        .to_owned();
    let workspace_dir = Dir::open_ambient_dir(&workspace_path, ambient_authority())
        .context("open schema-version workspace")?;
    workspace_dir
        .write("Netsukefile", yaml)
        .context("write schema-version manifest")?;
    let path = workspace_path.join("Netsukefile");

    let environment_reads = Arc::new(AtomicUsize::new(0));
    let observed_reads = Arc::clone(&environment_reads);
    let env_reader: EnvReader = Arc::new(move |_| {
        observed_reads.fetch_add(1, Ordering::SeqCst);
        Ok(String::from("probe"))
    });
    let mut stages = Vec::new();
    let result = manifest::from_path_with_policy_and_env(
        &path,
        NetworkPolicy::default(),
        &env_reader,
        Some(&mut |stage| stages.push(stage)),
    );

    Ok(LoadProbe {
        result,
        stages,
        environment_reads,
    })
}

/// Assert that schema rejection stops before template evaluation.
///
/// # Errors
///
/// Returns an error if loading succeeds, evaluation proceeds, or the stable
/// schema diagnostic is absent.
fn assert_rejected_before_evaluation(probe: LoadProbe) -> Result<()> {
    let error = probe
        .result
        .err()
        .context("unsupported schema declaration should fail to load")?;
    ensure!(
        probe.stages
            == vec![
                ManifestLoadStage::ManifestIngestion,
                ManifestLoadStage::InitialYamlParsing,
            ],
        "schema rejection should stop after YAML parsing: {:?}",
        probe.stages
    );
    ensure!(
        probe.environment_reads.load(Ordering::SeqCst) == 0,
        "schema rejection must not evaluate env()"
    );
    let diagnostic = format!("{error:#}");
    ensure!(
        diagnostic.contains(SCHEMA_DIAGNOSTIC),
        "schema rejection should carry its stable code: {diagnostic}"
    );
    Ok(())
}

#[rstest]
fn unsupported_version_is_rejected_before_template_evaluation() -> Result<()> {
    let yaml = manifest_with_declaration(&SchemaDeclaration::Unsupported(1), &[]);
    assert_rejected_before_evaluation(load_path(&yaml)?)
}

#[rstest]
fn missing_version_is_rejected_before_template_evaluation() -> Result<()> {
    let yaml = manifest_with_declaration(&SchemaDeclaration::Missing, &[]);
    assert_rejected_before_evaluation(load_path(&yaml)?)
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 512, ..ProptestConfig::default() })]

    #[test]
    fn every_unsupported_mapping_stops_before_template_evaluation(
        declaration in rejected_declaration(),
        unknown_keys in extra_unknown_keys(),
    ) {
        let yaml = manifest_with_declaration(&declaration, &unknown_keys);
        let probe = load_path(&yaml).expect("write and load generated manifest");
        let error = probe.result.err();
        prop_assert!(error.is_some(), "rejected declaration loaded successfully");
        let diagnostic = format!("{:#}", error.expect("error was checked above"));
        prop_assert!(diagnostic.contains(SCHEMA_DIAGNOSTIC), "{diagnostic}");
        prop_assert_eq!(
            probe.stages,
            vec![
                ManifestLoadStage::ManifestIngestion,
                ManifestLoadStage::InitialYamlParsing,
            ],
        );
        prop_assert_eq!(probe.environment_reads.load(Ordering::SeqCst), 0);
    }
}

#[rstest]
fn supported_version_reaches_the_template_evaluation_probe() -> Result<()> {
    let yaml = concat!(
        "netsuke_version: '1.0.0'\n",
        "targets:\n",
        "  - name: 'out-{{ item }}.txt'\n",
        "    foreach: \"[env('NETSUKE_SENTINEL')]\"\n",
        "    command: \"echo {{ env('NETSUKE_SENTINEL') }}\"\n",
    );
    let probe = load_path(yaml)?;
    ensure!(probe.result.is_ok(), "supported schema should load");
    ensure!(
        probe.environment_reads.load(Ordering::SeqCst) > 0,
        "the positive control should call the injected environment reader"
    );
    ensure!(
        probe.stages.contains(&ManifestLoadStage::TemplateExpansion),
        "the positive control should reach template expansion"
    );
    Ok(())
}

#[rstest]
fn merged_schema_version_is_admitted_before_template_evaluation() -> Result<()> {
    let yaml = concat!(
        "schema: &schema\n",
        "  netsuke_version: '1.1.0'\n",
        "<<: *schema\n",
        "targets:\n",
        "  - name: 'out-{{ item }}.txt'\n",
        "    foreach: \"[env('NETSUKE_SENTINEL')]\"\n",
        "    command: \"echo {{ env('NETSUKE_SENTINEL') }}\"\n",
    );
    let probe = load_path(yaml)?;
    let error = probe
        .result
        .as_ref()
        .err()
        .context("merged unsupported schema version should fail")?;
    ensure!(
        format!("{error:#}").contains("1.1"),
        "the merged declaration should identify schema 1.1: {error:#}"
    );
    assert_rejected_before_evaluation(probe)
}
