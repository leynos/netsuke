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
        layer.insert(
            "env_allow_var".to_owned(),
            json!(generated_variable_names(kind, scheme_indices, "allow")),
        );
        layer.insert(
            "env_block_var".to_owned(),
            json!(generated_variable_names(kind, scheme_indices, "block")),
        );
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

/// Map generated indices to layer-unique environment variable names.
fn generated_variable_names(kind: GeneratedLayer, indices: &[u8], policy: &str) -> Vec<String> {
    generated_schemes(kind, indices)
        .into_iter()
        .map(|scheme| format!("{scheme}-{policy}-var"))
        .collect()
}

proptest! {
    /// Whatever combination of file, environment, and CLI layers is generated,
    /// scalar keys resolve to the highest-precedence definer and list keys
    /// append in layer order.
    #[test]
    fn merge_precedence_holds_for_generated_layers(layers in generated_precedence_layers()) {
        let defaults = sanitize_value(&CliConfig::default()).expect("sanitizable defaults");
        let mut composer = MergeComposer::new();
        composer.push_defaults(defaults);

        for (kind, values) in layers.as_slices() {
            let Some(layer) =
                build_generated_layer(kind, values.jobs, values.json, &values.schemes)
            else {
                continue;
            };
            match kind {
                GeneratedLayer::File => composer.push_file(layer, None),
                GeneratedLayer::Environment => composer.push_environment(layer),
                GeneratedLayer::Cli => composer.push_cli(layer),
            }
        }

        let expected_jobs = layers
            .cli
            .jobs
            .or(layers.environment.jobs)
            .or(layers.file.jobs)
            .map(|jobs| usize::try_from(jobs).expect("generated jobs fit usize"));
        let merged = CliConfig::merge_from_layers(composer.layers())
            .expect("generated layers must merge cleanly");
        prop_assert_eq!(merged.jobs, expected_jobs);
        prop_assert_eq!(
            merged.json,
            layers.cli.json.or(layers.environment.json).or(layers.file.json).unwrap_or(false),
        );
        prop_assert_eq!(merged.fetch_allow_scheme, layers.expected_schemes());
        prop_assert_eq!(merged.env_allow_var, layers.expected_variable_names("allow"));
        prop_assert_eq!(merged.env_block_var, layers.expected_variable_names("block"));
    }
}

prop_compose! {
    /// Build the strategy the precedence property draws from.
    ///
    /// Kept out of the property so the non-vacuity test can drive the *same*
    /// generator rather than a lookalike that might reach different classes.
    fn generated_precedence_layers()(
        file_jobs in prop::option::of(1u64..=64),
        env_jobs in prop::option::of(1u64..=64),
        cli_jobs in prop::option::of(1u64..=64),
        file_json in prop::option::of(any::<bool>()),
        env_json in prop::option::of(any::<bool>()),
        cli_json in prop::option::of(any::<bool>()),
        file_schemes in prop::collection::vec(0u8..8, 0..8),
        env_schemes in prop::collection::vec(0u8..8, 0..8),
        cli_schemes in prop::collection::vec(0u8..8, 0..8),
    ) -> GeneratedLayers {
        GeneratedLayers {
            file: GeneratedLayerValues {
                jobs: file_jobs,
                json: file_json,
                schemes: file_schemes,
            },
            environment: GeneratedLayerValues {
                jobs: env_jobs,
                json: env_json,
                schemes: env_schemes,
            },
            cli: GeneratedLayerValues {
                jobs: cli_jobs,
                json: cli_json,
                schemes: cli_schemes,
            },
        }
    }
}

/// One layer's generated scalar and list values.
#[derive(Debug, Clone)]
struct GeneratedLayerValues {
    jobs: Option<u64>,
    json: Option<bool>,
    schemes: Vec<u8>,
}

/// All three layers' generated values for one case.
#[derive(Debug, Clone)]
struct GeneratedLayers {
    file: GeneratedLayerValues,
    environment: GeneratedLayerValues,
    cli: GeneratedLayerValues,
}

impl GeneratedLayers {
    /// Return the three layers paired with their identities, in precedence
    /// order.
    fn as_slices(&self) -> [(GeneratedLayer, &GeneratedLayerValues); 3] {
        [
            (GeneratedLayer::File, &self.file),
            (GeneratedLayer::Environment, &self.environment),
            (GeneratedLayer::Cli, &self.cli),
        ]
    }

    /// Return the schemes the merge should produce, in layer order.
    fn expected_schemes(&self) -> Vec<String> {
        self.as_slices()
            .into_iter()
            .flat_map(|(kind, values)| generated_schemes(kind, &values.schemes))
            .collect()
    }

    /// Return the variable names the merge should produce for `policy`, in
    /// layer order.
    fn expected_variable_names(&self, policy: &str) -> Vec<String> {
        self.as_slices()
            .into_iter()
            .flat_map(|(kind, values)| generated_variable_names(kind, &values.schemes, policy))
            .collect()
    }
}

/// Verify that the precedence contract is exercised on every layer subset.
///
/// The randomised property samples the layer combinations; this test enumerates
/// them. There are eight subsets of {file, environment, CLI}, and each one is
/// built from the *same* values so the expected winner is fixed by the subset
/// alone. Enumerating makes the non-vacuity argument structural rather than
/// statistical: every subset is guaranteed to be covered on every run, and the
/// all-three/disagreeing case — the only one that distinguishes "highest layer
/// wins" from "whichever layer was present won" — cannot silently drop out.
#[test]
fn every_layer_subset_resolves_to_its_highest_present_layer() {
    for subset in 0u8..8 {
        let file_present = subset & 0b001 != 0;
        let environment_present = subset & 0b010 != 0;
        let cli_present = subset & 0b100 != 0;

        let mut composer = MergeComposer::new();
        composer
            .push_defaults(sanitize_value(&CliConfig::default()).expect("sanitizable defaults"));
        // One layer per tier and a distinct value in each, so the merged scalar
        // names the winning tier unambiguously.
        if file_present {
            composer.push_file(json!({ "jobs": 2u64 }), None);
        }
        if environment_present {
            composer.push_environment(json!({ "jobs": 3u64 }));
        }
        if cli_present {
            composer.push_cli(json!({ "jobs": 4u64 }));
        }

        let expected = if cli_present {
            Some(4)
        } else if environment_present {
            Some(3)
        } else if file_present {
            Some(2)
        } else {
            None
        };
        let merged = CliConfig::merge_from_layers(composer.layers())
            .expect("an enumerated layer subset must merge cleanly");
        assert_eq!(
            merged.jobs, expected,
            "subset {subset:03b} (file={file_present} env={environment_present} \
             cli={cli_present}) should resolve to the highest present layer"
        );
    }
}
