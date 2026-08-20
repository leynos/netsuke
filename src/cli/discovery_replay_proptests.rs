//! Property tests for repeated deferred-diagnostics replay.
//!
//! The bounded diagnostics retained during one discovery pass are replayed
//! only after tracing is configured. The fixed case tests prove a single
//! replay is side-effect-free; these properties extend that to an arbitrary
//! number of replays, so repeated `emit_diagnostics` calls cannot drift, drain
//! events, or re-read the environment.

use super::layer_tests::{CountingEnv, LayerScenario, replay_events, scenario_cli};
use super::*;
use proptest::prelude::*;
use tempfile::tempdir;

proptest! {
    /// A discovery outcome replays identically any number of times without
    /// touching the environment again.
    #[test]
    fn replay_is_repeatable_and_environment_free(extra_replays in 0..32usize) {
        let temp = tempdir().expect("create temp dir");
        test_support::fs::write(temp.path().join(".netsuke.toml"), "jobs = 7\n")
            .expect("write project config");
        let cli = scenario_cli(LayerScenario::Discovery, &temp).expect("build scenario cli");
        let env = CountingEnv::default();
        let discovered = collect_diag_file_layers_with_env(&cli, &env);
        let discovery_lookups = env.get_calls();
        prop_assert!(
            discovery_lookups > 0,
            "discovery should read the injected environment"
        );

        let first = replay_events(&discovered).expect("replay retained events");
        prop_assert!(
            first.iter().any(|event| event.contains("using config discovery")),
            "the discovery branch should be replayed, got {first:?}"
        );
        for _ in 0..extra_replays {
            prop_assert_eq!(
                &replay_events(&discovered).expect("replay again"),
                &first,
                "repeated replay must emit identical events"
            );
        }
        prop_assert_eq!(
            env.get_calls(),
            discovery_lookups,
            "replay must not repeat environment access"
        );
    }
}
