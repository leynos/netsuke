//! Exercise relative-link extraction and repository-root containment.

use super::*;
use anyhow::ensure;
use proptest::prelude::*;
use rstest::rstest;

#[test]
fn extracts_relative_targets_with_original_source_lines() {
    let text = concat!(
        "[external](https://example.org) [anchor](#heading)\n",
        "[mail](mailto:user@example.org) [empty]()\n",
        "[local](../guide.md#section) [wrapped](\nchild.md)\n",
        "[title](other.md \"description\") [unfinished](missing\n",
    );
    let actual: Vec<_> = targets(text)
        .into_iter()
        .map(|target| (target.target, target.line))
        .collect();
    assert_eq!(
        actual,
        vec![
            ("../guide.md#section".into(), 3),
            ("child.md".into(), 3),
            ("other.md".into(), 5)
        ]
    );
}

#[rstest]
#[case("", false)]
#[case("#anchor", false)]
#[case("https://example.org", false)]
#[case("mailto:a@b", false)]
#[case("child.md#heading", true)]
fn classifies_relative_targets(#[case] target: &str, #[case] expected: bool) {
    assert_eq!(is_relative(target), expected);
}

#[test]
fn preserves_a_bare_fragment_when_resolving_directly() {
    assert_eq!(
        resolve("docs/source.md", "#anchor"),
        Some("docs/#anchor".into())
    );
}

#[test]
fn reports_missing_and_escaping_links_with_file_and_line() -> Result<()> {
    let temporary = tempfile::tempdir()?;
    let root = camino::Utf8Path::from_path(temporary.path()).context("UTF-8 fixture root")?;
    let dir = cap_std::fs_utf8::Dir::open_ambient_dir(root, cap_std::ambient_authority())?;
    dir.create_dir_all("docs/rfcs")?;
    dir.write("docs/rfcs/present.md", "present")?;
    dir.write(
        "docs/rfcs/source.md",
        "[valid](present.md)\n[missing](absent.md)\n[escape](../../../outside.md)\n",
    )?;
    let repo = Repo::fixture(root)?;
    let failures = dangling_in(&repo, "docs/rfcs/source.md")?;
    ensure!(
        failures.len() == 2,
        "fixture result differs from expected contract"
    );
    ensure!(
        failures
            .first()
            .context("missing-target diagnostic")?
            .contains("docs/rfcs/source.md:2 links to absent.md")
    );
    ensure!(
        failures
            .first()
            .context("missing-target diagnostic")?
            .contains("resolves to docs/rfcs/absent.md")
    );
    ensure!(
        failures
            .get(1)
            .context("above-root diagnostic")?
            .contains("docs/rfcs/source.md:3")
    );
    ensure!(
        failures
            .get(1)
            .context("above-root diagnostic")?
            .contains("climbs above the repository root")
    );
    ensure!(
        dangling(&repo)? == failures,
        "fixture result differs from expected contract"
    );
    let missing = dangling_in(&repo, "docs/rfcs/missing.md")
        .err()
        .context("missing read fails")?;
    ensure!(missing.to_string().contains("read docs/rfcs/missing.md"));
    Ok(())
}

/// Model traversal by counting depth first, then reducing matched pairs.
fn reference_path(directory: &[String], target: &[String]) -> Option<String> {
    let mut depth = directory.len();
    for segment in target {
        if segment == ".." {
            depth = depth.checked_sub(1)?;
        } else if segment != "." && !segment.is_empty() {
            depth += 1;
        }
    }
    let mut stack = directory.to_vec();
    for segment in target {
        if segment == ".." {
            stack.truncate(stack.len().saturating_sub(1));
        } else if segment != "." && !segment.is_empty() {
            stack.push(segment.clone());
        }
    }
    Some(stack.join("/"))
}

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(proptest::test_runner::FileFailurePersistence::Direct(
            "tests/rfc_stdlib_coverage/links_tests.proptest-regressions",
        ))),
        .. ProptestConfig::default()
    })]
    #[test]
    fn resolution_matches_repository_stack_model(
        directory in proptest::collection::vec("[a-z]{1,5}", 0..6),
        filename in "[a-z]{1,5}",
        target in proptest::collection::vec(
            prop_oneof![Just(String::new()), Just(".".into()), Just("..".into()), "[a-z]{1,5}"],
            1..12),
        fragment in proptest::option::of("[a-z]{1,5}"),
    ) {
        let file = directory.iter().cloned().chain([format!("{filename}.md")]).collect::<Vec<_>>().join("/");
        let raw = target.join("/");
        // A bare fragment is deliberately a direct-resolver special case.
        let suffix = fragment.map_or_else(String::new, |anchor| format!("#{anchor}"));
        let linked = format!("{raw}{suffix}");
        let expected = if raw.is_empty() && !suffix.is_empty() {
            reference_path(&directory, &[suffix])
        } else {
            reference_path(&directory, &target)
        };
        prop_assert_eq!(resolve(&file, &linked), expected);
    }
}
