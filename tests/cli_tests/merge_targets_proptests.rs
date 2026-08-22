//! Property tests for list-append and scalar merge ordering of the build-target
//! and display-policy fields.
//!
//! The example merge tests pin one fixed composition per story. These
//! properties hold across generated inputs for the fields the ladder tests do
//! not enumerate exhaustively:
//!
//! - `default_targets` appends in discovery order (file → environment → CLI),
//!   mirroring the ladder tests for scalar fields.
//! - Scalar merge ordering (defaults → file → environment → CLI) holds for
//!   generated locale and policy-enum values.
//!
//! The explicit-CLI `build <targets...>` replacement asymmetry lives in the
//! command layer of `netsuke::cli::merge` and is asserted end to end in
//! [`super::merge`] where a parsed command is available; a pure
//! `MergeComposer` composition cannot observe it.
//!
//! No `#[derive(Arbitrary)]` is used anywhere; every strategy is hand-written
//! and stays free of any second environment selector.

use netsuke::cli::CliConfig;
use ortho_config::{MergeComposer, sanitize_value};
use proptest::prelude::*;
use serde_json::{Value, json};

/// Values a generated merge layer may carry for the fields under test.
///
/// A field left `None` is omitted from the layer, matching a layer the
/// discovery pass simply did not produce.
struct LayerValues {
    default_targets: Option<Vec<String>>,
    jobs: Option<u64>,
    locale: Option<String>,
    emoji: Option<&'static str>,
    color: Option<&'static str>,
}

impl LayerValues {
    /// An empty layer carrying no generated values.
    const fn empty() -> Self {
        Self {
            default_targets: None,
            jobs: None,
            locale: None,
            emoji: None,
            color: None,
        }
    }
}

/// A generated target name: lower-case letters with an optional numeric
/// suffix. No second environment selector participates anywhere in these
/// strategies.
fn target_strategy() -> impl Strategy<Value = String> {
    "[a-z]{1,12}(-[a-z0-9]{1,4})?"
}

/// A locale tag in the `xx-YY` shape Netsuke accepts.
fn locale_strategy() -> impl Strategy<Value = String> {
    prop::sample::select(&["en-US", "es-ES", "de-DE", "fr-FR", "it-IT", "ja-JP"])
        .prop_map(str::to_owned)
}

/// A policy-enum string accepted by the display-policy parsers.
fn policy_strategy() -> impl Strategy<Value = &'static str> {
    prop::sample::select(&["auto", "always", "never"])
}

/// Build the JSON layer for `values`.
fn build_target_layer(values: &LayerValues) -> Value {
    let mut layer = serde_json::Map::new();
    if let Some(targets) = &values.default_targets {
        layer.insert("default_targets".to_owned(), json!(targets));
    }
    if let Some(jobs) = values.jobs {
        layer.insert("jobs".to_owned(), json!(jobs));
    }
    if let Some(locale) = &values.locale {
        layer.insert("locale".to_owned(), json!(locale));
    }
    if let Some(emoji) = values.emoji {
        layer.insert("emoji".to_owned(), json!(emoji));
    }
    if let Some(color) = values.color {
        layer.insert("color".to_owned(), json!(color));
    }
    Value::Object(layer)
}

/// Merge generated layers through `MergeComposer`.
///
/// # Errors
///
/// Propagates an `OrthoError` when the generated layers do not merge cleanly,
/// which a well-formed strategy should never trigger.
fn merge_generated(
    file_layer: Value,
    env_layer: Value,
    cli_layer: Value,
    defaults: Value,
) -> anyhow::Result<CliConfig> {
    let mut composer = MergeComposer::new();
    composer.push_defaults(defaults);
    composer.push_file(file_layer, None);
    composer.push_environment(env_layer);
    composer.push_cli(cli_layer);
    Ok(CliConfig::merge_from_layers(composer.layers())?)
}

proptest! {
    /// The `default_targets` alias appends through the whole ladder: the merged
    /// vector is the concatenation of the file, environment, and CLI layers in
    /// that order, regardless of how long or empty each generated layer is.
    #[test]
    fn default_targets_append_in_layer_order(
        file_targets in prop::collection::vec(target_strategy(), 0..5),
        env_targets in prop::collection::vec(target_strategy(), 0..5),
        cli_targets in prop::collection::vec(target_strategy(), 0..5),
    ) {
        let defaults = sanitize_value(&CliConfig::default()).expect("sanitizable defaults");
        let file_layer = build_target_layer(&LayerValues {
            default_targets: Some(file_targets.clone()),
            ..LayerValues::empty()
        });
        let env_layer = build_target_layer(&LayerValues {
            default_targets: Some(env_targets.clone()),
            ..LayerValues::empty()
        });
        let cli_layer = build_target_layer(&LayerValues {
            default_targets: Some(cli_targets.clone()),
            ..LayerValues::empty()
        });
        let merged = merge_generated(file_layer, env_layer, cli_layer, defaults)
            .expect("generated layers must merge cleanly");

        // The append strategy concatenates default_targets across file, env,
        // and CLI layers in discovery order (defaults contributes nothing).
        let expected_appended = file_targets
            .iter()
            .chain(env_targets.iter())
            .chain(cli_targets.iter())
            .cloned()
            .collect::<Vec<_>>();
        prop_assert_eq!(&merged.default_targets, &expected_appended);
        // The compatibility alias never leaks into the explicit build targets:
        // with no explicit build command, the merged cmds.build.targets stays
        // empty.
        prop_assert!(merged.cmds.build.targets.is_empty());
    }

    /// Scalar merge ordering holds for generated locale and policy-enum values:
    /// the highest populated layer wins, and each field is resolved
    /// independently.
    #[test]
    fn scalar_merge_ordering_holds_for_locale_and_policies(
        file_locale in prop::option::of(locale_strategy()),
        env_locale in prop::option::of(locale_strategy()),
        cli_locale in prop::option::of(locale_strategy()),
        file_emoji in prop::option::of(policy_strategy()),
        env_emoji in prop::option::of(policy_strategy()),
        cli_emoji in prop::option::of(policy_strategy()),
        file_color in prop::option::of(policy_strategy()),
        env_color in prop::option::of(policy_strategy()),
        cli_color in prop::option::of(policy_strategy()),
        file_jobs in prop::option::of(1u64..=64),
        env_jobs in prop::option::of(1u64..=64),
        cli_jobs in prop::option::of(1u64..=64),
    ) {
        let defaults = sanitize_value(&CliConfig::default()).expect("sanitizable defaults");
        let file_layer = build_target_layer(&LayerValues {
            default_targets: None,
            jobs: file_jobs,
            locale: file_locale.clone(),
            emoji: file_emoji,
            color: file_color,
        });
        let env_layer = build_target_layer(&LayerValues {
            default_targets: None,
            jobs: env_jobs,
            locale: env_locale.clone(),
            emoji: env_emoji,
            color: env_color,
        });
        let cli_layer = build_target_layer(&LayerValues {
            default_targets: None,
            jobs: cli_jobs,
            locale: cli_locale.clone(),
            emoji: cli_emoji,
            color: cli_color,
        });
        let merged = merge_generated(file_layer, env_layer, cli_layer, defaults)
            .expect("generated layers must merge cleanly");

        let expected_locale = cli_locale.or(env_locale).or(file_locale);
        let expected_emoji = cli_emoji.or(env_emoji).or(file_emoji).unwrap_or("auto");
        let expected_color = cli_color.or(env_color).or(file_color).unwrap_or("auto");
        let expected_jobs = cli_jobs.or(env_jobs).or(file_jobs);
        let emoji_text = merged.emoji.to_string();
        let color_text = merged.color.to_string();
        prop_assert_eq!(merged.locale.as_deref(), expected_locale.as_deref());
        prop_assert_eq!(emoji_text.as_str(), expected_emoji);
        prop_assert_eq!(color_text.as_str(), expected_color);
        prop_assert_eq!(
            merged.jobs,
            expected_jobs.map(|jobs| usize::try_from(jobs).expect("generated jobs fit usize"))
        );
    }
}
