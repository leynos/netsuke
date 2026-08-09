//! Injected configuration environment for in-process BDD CLI merges.

use mockable::MockEnv;
use std::collections::HashMap;
use std::ffi::OsString;

use clap::ArgMatches;
use netsuke::cli::Cli;
use ortho_config::OrthoResult;

use crate::bdd::fixtures::TestWorld;

fn environment_from_world(world: &TestWorld) -> MockEnv {
    let selector_values = world.env_vars_forward.borrow().clone();
    let values = selector_values
        .iter()
        .filter_map(|(key, raw_value)| {
            raw_value
                .to_str()
                .map(|text| (key.clone(), text.to_owned()))
        })
        .collect::<HashMap<_, _>>();
    let mut env = MockEnv::new();
    env.expect_os_string()
        .returning(move |key| selector_values.get(key).map(OsString::from));
    env.expect_all().return_const(values);
    env
}

/// Merge CLI configuration using only the environment recorded in `world`.
pub fn merge_with_world_env(
    world: &TestWorld,
    cli: &Cli,
    matches: &ArgMatches,
) -> OrthoResult<Cli> {
    netsuke::cli::merge_with_config_and_env(cli, matches, &environment_from_world(world))
}

#[cfg(test)]
mod tests {
    //! Covers preservation of raw non-UTF-8 configuration selectors.

    use super::*;

    #[cfg(unix)]
    #[test]
    fn environment_from_world_preserves_a_non_utf8_config_selector() {
        use mockable::Env;
        use std::os::unix::ffi::OsStringExt;

        let world = TestWorld::default();
        let selector = OsString::from_vec(b"/tmp/config-\xff.toml".to_vec());
        world.track_env_var("NETSUKE_CONFIG".to_owned(), Some(selector.clone()));

        let env = environment_from_world(&world);

        assert_eq!(env.os_string("NETSUKE_CONFIG"), Some(selector));
        assert!(env.all().is_empty());
    }
}
