//! Contract test for the `OrthoConfig` documentation metadata Netsuke emits.
//!
//! `cargo-orthohelp` consumes this serialised IR. The snapshot projects only
//! Netsuke's schema choices, keeping release-help compatibility visible without
//! coupling the application to unrelated upstream IR fields.

use insta::assert_yaml_snapshot;
use netsuke::cli::CliConfig;
use ortho_config::OrthoConfigDocs;
use serde::Serialize;

const APPEND_MERGE_FIELDS: [&str; 4] = [
    "fetch_allow_scheme",
    "fetch_allow_host",
    "fetch_block_host",
    "default_targets",
];

#[derive(Serialize)]
struct MetadataSnapshot {
    ir_version: String,
    precedence: Vec<String>,
    discovery: Option<DiscoverySnapshot>,
    fields: Vec<FieldSnapshot>,
    subcommand_count: usize,
}

#[derive(Serialize)]
struct DiscoverySnapshot {
    formats: Vec<String>,
    search_paths: Vec<String>,
    override_flag: Option<String>,
    override_env: Option<String>,
    xdg_compliant: bool,
}

#[derive(Serialize)]
struct FieldSnapshot {
    name: String,
    cli: Option<CliSourceSnapshot>,
    environment: Option<String>,
    file: Option<String>,
    merge_strategy: &'static str,
}

#[derive(Serialize)]
struct CliSourceSnapshot {
    long: Option<String>,
    short: Option<char>,
    multiple: bool,
    possible_values: Vec<String>,
}

fn merge_strategy(field_name: &str) -> &'static str {
    if APPEND_MERGE_FIELDS.contains(&field_name) {
        "append"
    } else {
        "replace"
    }
}

fn metadata_snapshot() -> MetadataSnapshot {
    let metadata = CliConfig::get_doc_metadata();
    let precedence = metadata
        .sections
        .precedence
        .as_ref()
        .map_or_else(Vec::new, |precedence| {
            precedence
                .order
                .iter()
                .map(|source| format!("{source:?}"))
                .collect()
        });
    let discovery = metadata
        .sections
        .discovery
        .as_ref()
        .map(|discovery| DiscoverySnapshot {
            formats: discovery
                .formats
                .iter()
                .map(|format| format!("{format:?}"))
                .collect(),
            search_paths: discovery
                .search_paths
                .iter()
                .map(|path| path.pattern.clone())
                .collect(),
            override_flag: discovery.override_flag_long.clone(),
            override_env: discovery.override_env.clone(),
            xdg_compliant: discovery.xdg_compliant,
        });
    let fields = metadata
        .fields
        .iter()
        .map(|field| FieldSnapshot {
            name: field.name.clone(),
            cli: field.cli.as_ref().map(|cli| CliSourceSnapshot {
                long: cli.long.clone(),
                short: cli.short,
                multiple: cli.multiple,
                possible_values: cli.possible_values.clone(),
            }),
            environment: field.env.as_ref().map(|env| env.var_name.clone()),
            file: field.file.as_ref().map(|file| file.key_path.clone()),
            merge_strategy: merge_strategy(&field.name),
        })
        .collect();

    MetadataSnapshot {
        ir_version: metadata.ir_version,
        precedence,
        discovery,
        fields,
        subcommand_count: metadata.subcommands.len(),
    }
}

#[test]
fn cli_config_documentation_metadata_is_stable() {
    assert_yaml_snapshot!(metadata_snapshot());
}
