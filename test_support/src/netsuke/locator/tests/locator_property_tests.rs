//! Property tests for generated Netsuke executable locator layouts.
//!
//! The table tests in `locator.rs` pin Cargo's named layouts and exhaust every
//! three-candidate presence mask. These properties complement them by stating
//! the same candidate ordering and selection invariants over arbitrary valid
//! UTF-8 root components, profiles, target triples, and target directories.

use super::super::{candidate_paths, netsuke_executable_from};
use super::{binary_name, env_with_target_dir, touch, utf8_root};
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;

/// Generate a valid UTF-8 path component for a temporary-root child.
fn root_component() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_-]{0,8}"
}

/// Generate a valid Cargo profile component distinct from `deps`.
fn profile_component() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_-]{0,8}".prop_filter("profile component must not be `deps`", |component| {
        component != "deps"
    })
}

/// Generate a valid target-triple component distinct from `deps`.
fn target_triple() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_-]{0,8}".prop_filter("target triple must not be `deps`", |component| {
        component != "deps"
    })
}

/// Generate an optional valid UTF-8 `CARGO_TARGET_DIR` component.
fn target_dir_component() -> impl Strategy<Value = Option<String>> {
    proptest::option::of(root_component())
}

/// Build an absolute UTF-8 root under a newly allocated temporary directory.
fn generated_root(
    root_component: String,
) -> Result<(tempfile::TempDir, camino::Utf8PathBuf), TestCaseError> {
    let temp = tempfile::tempdir().map_err(|error| TestCaseError::fail(error.to_string()))?;
    let root = utf8_root(&temp).map_err(|error| TestCaseError::fail(error.to_string()))?;
    Ok((temp, root.join(root_component)))
}

proptest! {
    /// Keep candidate contents and order stable for every generated layout.
    #[test]
    fn candidate_paths_match_the_documented_order(
        root_component in root_component(),
        profile in profile_component(),
        triple in target_triple(),
        target_dir_component in target_dir_component(),
    ) {
        let (_temp, root) = generated_root(root_component)?;
        let exe_dir = root.join("build").join(&triple).join(&profile);
        let target_dir = target_dir_component
            .as_deref()
            .map(|component| root.join(component));
        let env = env_with_target_dir(target_dir.as_deref());
        let binary = binary_name();

        let candidates = candidate_paths(&env, &exe_dir, &binary);
        let mut expected = vec![exe_dir.join(&binary)];
        if let Some(target_root) = &target_dir {
            expected.push(target_root.join(&profile).join(&binary));
            expected.push(target_root.join(&triple).join(&profile).join(&binary));
        }

        prop_assert_eq!(&candidates, &expected);
        if target_dir.is_none() {
            prop_assert_eq!(candidates.len(), 1);
        }
    }

    /// Resolve the first staged candidate and report every missing path.
    #[test]
    fn executable_lookup_honours_generated_candidate_order(
        root_component in root_component(),
        profile in profile_component(),
        triple in target_triple(),
        target_dir_component in target_dir_component(),
        presence in 0u8..8,
    ) {
        let (_temp, root) = generated_root(root_component)?;
        let exe_dir = root.join("build").join(&triple).join(&profile);
        let executable = exe_dir.join("deps").join("test-exe");
        touch(&executable).map_err(|error| TestCaseError::fail(error.to_string()))?;

        let target_dir = target_dir_component
            .as_deref()
            .map(|component| root.join(component));
        let env = env_with_target_dir(target_dir.as_deref());
        let binary = binary_name();
        let candidates = candidate_paths(&env, &exe_dir, &binary);
        for (slot, candidate) in candidates.iter().enumerate() {
            if presence & (1 << slot) != 0 {
                touch(candidate).map_err(|error| TestCaseError::fail(error.to_string()))?;
            }
        }

        let located = netsuke_executable_from(&env, &executable);
        let first_present = candidates
            .iter()
            .enumerate()
            .find(|(slot, _)| presence & (1 << slot) != 0);
        if let Some((_, expected)) = first_present {
            let resolved = located.map_err(|error| TestCaseError::fail(error.to_string()))?;
            prop_assert_eq!(resolved.as_path(), expected.as_path());
        } else {
            let error = located
                .err()
                .ok_or_else(|| TestCaseError::fail("missing candidates should fail"))?;
            let message = error.to_string();
            for candidate in candidates {
                prop_assert!(
                    message.contains(candidate.as_str()),
                    "missing-candidate diagnostic should list {candidate}; got: {message}"
                );
            }
        }
    }
}
