//! Property tests for configuration layer precedence.
//!
//! The example merge tests pin one fixed composition; these properties hold for
//! any generated combination of file, environment, and CLI layers, so the
//! precedence and list-appending contract cannot regress on later layers.

use netsuke::cli::CliConfig;
use ortho_config::{MergeComposer, sanitize_value};
use proptest::prelude::*;
use serde_json::{Map, Value, json};

/// Which generated layer a configured key claims, for deterministic scheme names.
#[derive(Clone, Copy)]
enum GeneratedLayer {
    File,
    Environment,
    Cli,
}

impl GeneratedLayer {
    /// Layer-unique scheme prefix so generated values never collide across layers.
    const fn scheme_prefix(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Environment => "env",
            Self::Cli => "cli",
        }
    }
}

/// Build the JSON layer for `kind` from generated scalar and list values.
///
/// A layer with no generated keys is omitted, matching a layer that the
/// discovery pass simply did not produce.
fn build_generated_layer(
    kind: GeneratedLayer,
    jobs: Option<u64>,
    json_enabled: Option<bool>,
    scheme_indices: &[u8],
) -> Option<Value> {
    let mut layer = Map::new();
    if let Some(job_value) = jobs {
        layer.insert("jobs".to_owned(), json!(job_value));
    }
    if let Some(json_value) = json_enabled {
        layer.insert("json".to_owned(), json!(json_value));
    }
    let schemes = generated_schemes(kind, scheme_indices);
    if !schemes.is_empty() {
        layer.insert("fetch_allow_scheme".to_owned(), json!(schemes));
    }
    (!layer.is_empty()).then(|| Value::Object(layer))
}

/// Map generated indices to layer-unique scheme names.
///
/// Duplicates within a layer collapse, so the expected merged list stays
/// deterministic whichever append strategy the merge applies.
fn generated_schemes(kind: GeneratedLayer, indices: &[u8]) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    indices
        .iter()
        .filter(|index| seen.insert(**index))
        .map(|index| format!("{}-scheme{}", kind.scheme_prefix(), index))
        .collect()
}

proptest! {
    /// Whatever combination of file, environment, and CLI layers is generated,
    /// scalar keys resolve to the highest-precedence definer and list keys
    /// append in layer order.
    #[test]
    fn merge_precedence_holds_for_generated_layers(
        file_jobs in prop::option::of(1u64..=64),
        env_jobs in prop::option::of(1u64..=64),
        cli_jobs in prop::option::of(1u64..=64),
        file_json in prop::option::of(any::<bool>()),
        env_json in prop::option::of(any::<bool>()),
        cli_json in prop::option::of(any::<bool>()),
        file_schemes in prop::collection::vec(0u8..8, 0..8),
        env_schemes in prop::collection::vec(0u8..8, 0..8),
        cli_schemes in prop::collection::vec(0u8..8, 0..8),
    ) {
        let defaults = sanitize_value(&CliConfig::default()).expect("sanitizable defaults");
        let mut composer = MergeComposer::new();
        composer.push_defaults(defaults);

        let file_layer = build_generated_layer(
            GeneratedLayer::File,
            file_jobs,
            file_json,
            &file_schemes,
        );
        if let Some(layer) = &file_layer {
            composer.push_file(layer.clone(), None);
        }
        let env_layer = build_generated_layer(
            GeneratedLayer::Environment,
            env_jobs,
            env_json,
            &env_schemes,
        );
        if let Some(layer) = &env_layer {
            composer.push_environment(layer.clone());
        }
        let cli_layer = build_generated_layer(GeneratedLayer::Cli, cli_jobs, cli_json, &cli_schemes);
        if let Some(layer) = &cli_layer {
            composer.push_cli(layer.clone());
        }

        let expected_jobs = cli_jobs
            .or(env_jobs)
            .or(file_jobs)
            .map(|jobs| usize::try_from(jobs).expect("generated jobs fit usize"));
        let merged = CliConfig::merge_from_layers(composer.layers())
            .expect("generated layers must merge cleanly");
        prop_assert_eq!(merged.jobs, expected_jobs);
        prop_assert_eq!(merged.json, cli_json.or(env_json).or(file_json).unwrap_or(false));
        let expected_schemes = generated_schemes(GeneratedLayer::File, &file_schemes)
            .into_iter()
            .chain(generated_schemes(GeneratedLayer::Environment, &env_schemes))
            .chain(generated_schemes(GeneratedLayer::Cli, &cli_schemes))
            .collect::<Vec<_>>();
        prop_assert_eq!(merged.fetch_allow_scheme, expected_schemes);
    }
}
