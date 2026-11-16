//! Property tests for user-scope candidate derivation and selection.
//!
//! The fixed cases in the parent module pin the named contract scenarios.
//! These properties extend them over the environment states a table can only
//! sample: which user bases are set, empty, relative, or unset, and which
//! candidate files exist or fail to load.

use super::{Case, canonical, write_config};
use crate::cli::discovery::discovery_env_source;
use crate::cli::discovery::layers::config_discovery;
use crate::cli::discovery::paths::{SanitisedUserBases, user_scope_candidates};
use crate::cli::test_support::TestEnv;
use ortho_config::{MapEnv, SharedEnvSource};
use proptest::prelude::*;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The user base variables consulted before the home fallbacks, in preference
/// order, paired with the directory each generated case gives that base.
const BASE_ORDER: [(&str, &str); 3] = [
    ("XDG_CONFIG_HOME", "xdg-config-home"),
    ("APPDATA", "appdata"),
    ("LOCALAPPDATA", "localappdata"),
];

/// Relative paths a base directory contributes, nested file first.
const BASE_FILES: [&str; 2] = ["netsuke/config.toml", ".netsuke.toml"];

/// Relative paths the home directory contributes, in preference order.
const HOME_FILES: [&str; 3] = [
    ".config/netsuke/config.toml",
    ".config/.netsuke.toml",
    ".netsuke.toml",
];

/// Content that cannot be parsed, so its candidate cannot load.
const MALFORMED: &str = "emoji = \"always\n";

/// How one user base variable is set for a generated case.
#[derive(Clone, Copy, Debug)]
enum BaseChoice {
    /// The variable is absent from the environment.
    Unset,
    /// The variable is present but empty, which discovery treats as unset.
    Empty,
    /// The variable names a path relative to the working directory, which
    /// discovery also treats as unset.
    Relative,
    /// The variable names its own base directory.
    Set,
}

/// Generate one base choice, giving all four states equal weight.
fn base_choice() -> impl Strategy<Value = BaseChoice> {
    prop_oneof![
        Just(BaseChoice::Unset),
        Just(BaseChoice::Empty),
        Just(BaseChoice::Relative),
        Just(BaseChoice::Set),
    ]
}

/// Write `choice` for `key` into `environment`, resolving `Set` beneath `base`.
fn insert_base(environment: &mut MapEnv, key: &str, choice: BaseChoice, base: &Path) {
    match choice {
        BaseChoice::Unset => {}
        BaseChoice::Empty => {
            environment.insert(key, OsString::new());
        }
        BaseChoice::Relative => {
            environment.insert(key, OsString::from("relative-base"));
        }
        BaseChoice::Set => {
            environment.insert(key, base.as_os_str());
        }
    }
}

/// Return the candidate paths beneath `base`, relative to it.
fn files_under(candidates: &[PathBuf], base: &Path) -> Vec<PathBuf> {
    candidates
        .iter()
        .filter_map(|candidate| candidate.strip_prefix(base).ok())
        .map(Path::to_path_buf)
        .collect()
}

/// Render `files` as owned paths for comparison with a discovered list.
fn expected_files(files: &[&str]) -> Vec<PathBuf> {
    files.iter().map(PathBuf::from).collect()
}

/// Return the first and last candidate index beneath `base`.
fn span_under(candidates: &[PathBuf], base: &Path) -> Option<(usize, usize)> {
    let indices = candidates
        .iter()
        .enumerate()
        .filter(|(_, candidate)| candidate.starts_with(base))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    indices
        .first()
        .zip(indices.last())
        .map(|(first, last)| (*first, *last))
}

proptest! {
    /// Every candidate is absolute and belongs to a user base, no candidate
    /// comes from the system scope, the bases keep their documented order, and
    /// each base offers its nested file before its dotfile.
    #[test]
    fn user_candidates_stay_inside_user_bases_in_order(
        xdg_config_home in base_choice(),
        appdata in base_choice(),
        localappdata in base_choice(),
        home in base_choice(),
        userprofile in base_choice(),
    ) {
        let temp = tempfile::tempdir().expect("create temp dir");
        let root = temp.path();
        let mut environment = MapEnv::new();
        let mut ordered_bases = Vec::new();
        for ((key, directory), choice) in BASE_ORDER.iter().zip([
            xdg_config_home,
            appdata,
            localappdata,
        ]) {
            let base = root.join(directory);
            insert_base(&mut environment, key, choice, &base);
            ordered_bases.push((base, choice));
        }
        let home_base = root.join("home");
        let profile_base = root.join("profile");
        insert_base(&mut environment, "HOME", home, &home_base);
        insert_base(&mut environment, "USERPROFILE", userprofile, &profile_base);
        let system = root.join("system");
        environment.insert("XDG_CONFIG_DIRS", system.as_os_str());

        let candidates = user_scope_candidates(&environment);

        for candidate in &candidates {
            prop_assert!(
                candidate.is_absolute(),
                "a candidate must not resolve against the working directory: {:?}",
                candidate
            );
            prop_assert!(
                !candidate.starts_with(&system),
                "a candidate must not come from the system scope: {:?}",
                candidate
            );
        }

        let mut present_bases = Vec::new();
        for (base, choice) in &ordered_bases {
            match choice {
                BaseChoice::Set => {
                    let files = files_under(&candidates, base);
                    prop_assert_eq!(
                        &files,
                        &expected_files(&BASE_FILES),
                        "a set base contributes its nested file before its dotfile"
                    );
                    present_bases.push(base.clone());
                }
                BaseChoice::Empty | BaseChoice::Relative | BaseChoice::Unset => {
                    let files = files_under(&candidates, base);
                    prop_assert!(
                        files.is_empty(),
                        "an empty, relative, or unset base contributes nothing, got {:?}",
                        files
                    );
                }
            }
        }

        let resolved_home = match (home, userprofile) {
            (BaseChoice::Set, _) => Some(home_base),
            (_, BaseChoice::Set) => Some(profile_base),
            _ => None,
        };
        // With no resolved home the only candidates are the base ones, and the
        // count below rejects any candidate no base accounts for.
        if let Some(base) = &resolved_home {
            prop_assert_eq!(
                &files_under(&candidates, base),
                &expected_files(&HOME_FILES),
                "the home directory contributes its documented files in order"
            );
            present_bases.push(base.clone());
        }

        let accounted: usize = present_bases
            .iter()
            .map(|base| files_under(&candidates, base).len())
            .sum();
        prop_assert_eq!(
            candidates.len(),
            accounted,
            "every candidate belongs to exactly one user base: {:?}",
            candidates
        );

        let later_bases = present_bases.iter().skip(1);
        for (earlier, later) in present_bases.iter().zip(later_bases) {
            prop_assert!(
                matches!(
                    (
                        span_under(&candidates, earlier),
                        span_under(&candidates, later)
                    ),
                    (Some((_, end)), Some((start, _))) if end < start
                ),
                "each base must finish before the next begins: {:?}",
                candidates
            );
        }
    }

    /// No base value the user pass rejects reaches the fallback scan.
    ///
    /// The dependency filters only empty values, so without the sanitised view
    /// a relative base would still become a working-directory relative
    /// candidate. Every generated state must therefore yield absolute
    /// candidates only, while the system candidate keeps its place.
    #[test]
    fn sanitised_fallback_admits_only_absolute_bases(
        xdg_config_home in base_choice(),
        appdata in base_choice(),
        localappdata in base_choice(),
        home in base_choice(),
        userprofile in base_choice(),
    ) {
        let temp = tempfile::tempdir().expect("create temp dir");
        let root = temp.path();
        let mut environment = MapEnv::new();
        for ((key, directory), choice) in BASE_ORDER.iter().zip([
            xdg_config_home,
            appdata,
            localappdata,
        ]) {
            insert_base(&mut environment, key, choice, &root.join(directory));
        }
        insert_base(&mut environment, "HOME", home, &root.join("home"));
        insert_base(&mut environment, "USERPROFILE", userprofile, &root.join("profile"));
        let system = root.join("system");
        environment.insert("XDG_CONFIG_DIRS", system.as_os_str());

        let sanitised: SharedEnvSource = Arc::new(SanitisedUserBases::new(Arc::new(environment)));
        let candidates = config_discovery(None, sanitised).candidates();

        for candidate in &candidates {
            prop_assert!(
                candidate.is_absolute(),
                "a rejected base must not yield a fallback candidate: {:?}",
                candidate
            );
        }
        prop_assert!(
            candidates.contains(&system.join("netsuke/config.toml")),
            "the system candidate should survive every base state: {:?}",
            candidates
        );
    }

    /// Discovery loads exactly the first existing user candidate, ignores every
    /// later one, and fails closed when that candidate cannot be loaded.
    #[test]
    fn discovery_selects_the_first_existing_user_candidate(
        presence in proptest::array::uniform9(any::<bool>()),
        corrupt_first in any::<bool>(),
    ) {
        let case = Case::new().expect("build user-scope case");
        let mut environment = TestEnv::default().with_var("HOME", case.home().as_os_str());
        for (key, directory) in BASE_ORDER {
            let base = case.base(directory).expect("create user base");
            environment = environment.with_var(key, base.into_os_string());
        }

        let candidates = user_scope_candidates(discovery_env_source(&environment).as_ref());
        prop_assert!(
            candidates.len() >= presence.len(),
            "four set bases should yield at least {} candidates: {:?}",
            presence.len(),
            candidates
        );
        let mut present = Vec::new();
        for (candidate, is_present) in candidates.iter().zip(presence.iter()) {
            if *is_present {
                write_config(candidate, "jobs = 4\n").expect("write user candidate");
                present.push(candidate.clone());
            }
        }

        let first_present = present.first();
        if let (true, Some(first)) = (corrupt_first, first_present) {
            write_config(first, MALFORMED).expect("corrupt the first candidate");
        }
        let scoped = case.scoped_environment(environment);
        let (paths, error) = case.discover(&scoped);

        match (first_present, corrupt_first) {
            (Some(_), true) => {
                prop_assert!(
                    error.is_some(),
                    "a candidate that exists but cannot load must fail discovery: {:?}",
                    paths
                );
                prop_assert!(
                    paths.is_empty(),
                    "a failed user pass must not fall through to another layer: {:?}",
                    paths
                );
            }
            (Some(first), false) => {
                prop_assert!(
                    error.is_none(),
                    "discovery should succeed: {:?}",
                    error
                );
                prop_assert_eq!(
                    &paths,
                    &vec![canonical(first).expect("canonicalise the expected layer")]
                );
            }
            (None, _) => {
                prop_assert!(
                    error.is_none(),
                    "absent user candidates must not fail discovery: {:?}",
                    error
                );
                prop_assert_eq!(
                    &paths,
                    &vec![canonical(&case.system_config()).expect("canonicalise system layer")]
                );
            }
        }
    }
}
