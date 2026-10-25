//! Guard the revision-pinned inventory's links to the roadmap and obligations.
//!
//! These tests check document structure only. They deliberately do not read
//! Rust source, so later implementation work can change code without making
//! this historical inventory contract stale.

use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use cap_std::{ambient_authority, fs_utf8::Dir};

#[path = "hexagonal_inventory_contract/markdown.rs"]
mod markdown;
#[path = "hexagonal_inventory_contract/roadmap.rs"]
mod roadmap;
#[path = "hexagonal_inventory_contract/validation.rs"]
mod validation;

use roadmap::{dependents_of, roadmap_tasks};
use validation::validate_inventory;

const INVENTORY_PATH: &str = "docs/hexagonal-hardening-inventory.md";
const ROADMAP_PATH: &str = "docs/roadmap-hexagonal-hardening.md";

/// Open the checkout root as a capability-scoped directory.
///
/// # Errors
///
/// Returns an error when the checkout root cannot be opened.
fn repo_root() -> Result<Dir> {
    let path = Utf8Path::new(env!("CARGO_MANIFEST_DIR"));
    Dir::open_ambient_dir(path, ambient_authority())
        .with_context(|| format!("open repository root {path}"))
}

/// Confirm the real inventory is structurally consistent with the roadmap.
#[test]
fn real_inventory_is_consistent() -> Result<()> {
    let root = repo_root()?;
    let inventory = root
        .read_to_string(INVENTORY_PATH)
        .with_context(|| format!("read {INVENTORY_PATH}"))?;
    let roadmap = root
        .read_to_string(ROADMAP_PATH)
        .with_context(|| format!("read {ROADMAP_PATH}"))?;
    let tasks = roadmap_tasks(&roadmap);
    ensure!(
        dependents_of(&tasks, "26.1.1").len() == 25,
        "the pinned roadmap dependency closure should contain 25 tasks"
    );
    validate_inventory(&inventory, &roadmap)
}

#[cfg(test)]
mod tests {
    //! Exercise the document parsers and integrity checks with small fixtures.

    use std::collections::BTreeSet;

    use anyhow::{Result, bail, ensure};
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::*;
    use crate::markdown::{pins, register_ids, section, task_ids};

    const PIN: &str = "0123456789abcdef0123456789abcdef01234567";
    const SIMPLE_ROADMAP: &str = concat!(
        "- [ ] 26.1.1. Root task.\n",
        "  - Dependencies: none.\n",
        "- [ ] 26.1.2. Child task.\n",
        "  - Dependencies: 26.1.1.\n",
    );
    const TRANSITIVE_ROADMAP: &str = concat!(
        "- [ ] 26.1.1. Root task.\n",
        "  - Dependencies: none.\n",
        "- [ ] 26.1.2. Child task.\n",
        "  - Dependencies: 26.1.1; may proceed independently of 28.2 implementation.\n",
        "- [x] 26.1.3. Grandchild task.\n",
        "  - Dependencies: 26.1.2 and 29.2.1/29.2.2 policy.\n",
        "    Continued prose includes 27.1.1.\n",
        "  - Acceptance: this is not a dependency; 27.3.2.\n",
    );
    const VALID_SKELETON: &str = concat!(
        "# Hexagonal hardening model and consumer inventory\n",
        "\nPin: 0123456789abcdef0123456789abcdef01234567\n\n",
        "## Scope and definitions\n\nDefinitions pending.\n\n",
        "## Method\n\nMethod pending.\n\n",
        "## Register\n\n",
        "### Type rows\n\n",
        "| ID | Item | Visibility | Derives | Location |\n",
        "| -- | -- | -- | -- | -- |\n\n",
        "### Site rows\n\n",
        "| ID | Site | Kind | Location | Production callers | Planned change | Obligations |\n",
        "| -- | -- | -- | -- | -- | -- | -- |\n\n",
        "## Landed work\n\nPending EP-M3.\n\n",
        "## Findings\n\nPending EP-M3.\n\n",
        "## Compatibility obligations\n\nPending EP-M3.\n\n",
        "## Trace matrix\n\n",
        "### Changes\n\n",
        "| Task | Register items | Findings | Obligations |\n",
        "| -- | -- | -- | -- |\n\n",
        "### Consumes\n\n",
        "| Task | Register items | Findings | Obligations |\n",
        "| -- | -- | -- | -- |\n\n",
        "### Excluded\n\n",
        "- 26.1.2: pending EP-M3.\n\n",
        "### Unchanged\n\nNone yet.\n\n",
        "## Consuming this inventory\n\nPending.\n\n",
        "## Refreshing this inventory\n\nPending.\n"
    );

    /// Return a fixture error when the inventory violates the contract.
    fn expect_validation_error(inventory: &str, roadmap: &str, expected: &str) -> Result<()> {
        match validate_inventory(inventory, roadmap) {
            Ok(()) => bail!("fixture should have failed validation for {expected}"),
            Err(error) => ensure!(
                format!("{error:#}").contains(expected),
                "expected error containing {expected:?}, found {error:#}"
            ),
        }
        Ok(())
    }

    /// Add one dependent task that reaches the root through another task.
    fn transitive_skeleton() -> String {
        VALID_SKELETON.replace(
            "- 26.1.2: pending EP-M3.\n",
            "- 26.1.2: pending EP-M3.\n- 26.1.3: pending EP-M3.\n",
        )
    }

    #[rstest]
    #[case::plain_heading("## Purpose\nbody\n\n## Next\nother\n", "Purpose", "body\n\n")]
    #[case::numbered_heading("## 4. Register\nrows\n## 5. Next\n", "Register", "rows\n")]
    fn section_stops_at_the_next_second_level_heading(
        #[case] source: &str,
        #[case] heading: &str,
        #[case] expected: &str,
    ) -> Result<()> {
        ensure!(
            section(source, heading) == Some(expected),
            "section body differs"
        );
        Ok(())
    }

    #[rstest]
    #[case::lowercase_sha(format!("{PIN} {PIN}g {} {}", "A".repeat(40), "a".repeat(39)), 1)]
    #[case::no_sha("no commit here".to_owned(), 0)]
    fn pins_collect_only_lowercase_forty_character_tokens(
        #[case] source: String,
        #[case] expected: usize,
    ) -> Result<()> {
        ensure!(
            pins(&source).len() == expected,
            "unexpected pin-token count"
        );
        Ok(())
    }

    #[rstest]
    #[case::version_and_two_part_label("0.1.0 and 28.2", &[])]
    #[case::slash_joined("29.2.1/29.2.2", &["29.2.1", "29.2.2"])]
    fn task_ids_ignore_versions_and_two_part_labels(
        #[case] source: &str,
        #[case] expected: &[&str],
    ) {
        let actual = task_ids(source);
        let expected_set: BTreeSet<String> =
            expected.iter().map(|value| (*value).to_owned()).collect();
        assert_eq!(actual, expected_set);
    }

    #[test]
    fn roadmap_parser_reads_dependencies_and_transitive_continuations() {
        let tasks = roadmap_tasks(TRANSITIVE_ROADMAP);
        assert_eq!(
            tasks.get("26.1.3"),
            Some(&BTreeSet::from([
                "26.1.2".to_owned(),
                "29.2.1".to_owned(),
                "29.2.2".to_owned(),
                "27.1.1".to_owned(),
            ]))
        );
        assert_eq!(
            dependents_of(&tasks, "26.1.1"),
            BTreeSet::from(["26.1.2".to_owned(), "26.1.3".to_owned()])
        );
    }

    #[test]
    fn register_ids_read_first_cells_from_register_tables() {
        let source = concat!(
            "## Register\n",
            "| ID | Item |\n| -- | -- |\n",
            "| A1/ast::Rule | Rule |\n\n",
            "## Trace matrix\n",
            "| Task | Register items |\n| -- | -- |\n| 26.1.2 | A1/* |\n"
        );
        assert_eq!(register_ids(source), ["A1/ast::Rule"]);
    }

    #[test]
    fn valid_skeleton_covers_every_dependent_task_as_pending() -> Result<()> {
        validate_inventory(VALID_SKELETON, SIMPLE_ROADMAP)
    }

    #[test]
    fn area_and_site_wildcards_cover_register_identifiers() -> Result<()> {
        let inventory = VALID_SKELETON
            .replace(
                "| -- | -- | -- | -- | -- |\n\n### Site rows",
                "| -- | -- | -- | -- | -- |\n| A1/ast::Rule | Rule | -- | -- | -- |\n\n### Site rows",
            )
            .replace(
                "| -- | -- | -- | -- | -- | -- | -- |\n\n## Landed work",
                "| -- | -- | -- | -- | -- | -- | -- |\n| A1/ast::Rule@Recipe::new#constructor | -- | constructor | -- | -- | -- | -- |\n\n## Landed work",
            )
            .replace(
                "| -- | -- | -- | -- |\n\n### Consumes",
                "| 26.1.2 | A1/* A1/ast::Rule@* | -- | -- |\n\n### Consumes",
            );
        validate_inventory(&inventory, SIMPLE_ROADMAP)
    }

    #[rstest]
    #[case::area_only("A1")]
    #[case::unknown_area("A8/ast::Rule")]
    #[case::unsupported_site_kind("A1/ast::Rule@Recipe::new#builder")]
    fn rejects_malformed_register_identifiers(#[case] identifier: &str) -> Result<()> {
        let inventory = VALID_SKELETON.replace(
            "| -- | -- | -- | -- | -- |\n\n### Site rows",
            &format!(
                "| -- | -- | -- | -- | -- |\n| {identifier} | Rule | -- | -- | -- |\n\n### Site rows"
            ),
        );
        expect_validation_error(&inventory, SIMPLE_ROADMAP, "is malformed")?;
        Ok(())
    }

    #[test]
    fn rejects_two_distinct_commit_pins() -> Result<()> {
        let inventory = VALID_SKELETON.replace(
            "## Scope and definitions",
            "abcdef0123456789abcdef0123456789abcdef01\n\n## Scope and definitions",
        );
        expect_validation_error(&inventory, SIMPLE_ROADMAP, "only its declared")?;
        Ok(())
    }

    #[test]
    fn rejects_an_inventory_without_a_pin_line() -> Result<()> {
        let inventory = VALID_SKELETON.replace(&format!("Pin: {PIN}\n"), "");
        expect_validation_error(&inventory, SIMPLE_ROADMAP, "exactly one `Pin:` line")?;
        Ok(())
    }

    #[test]
    fn rejects_duplicate_pin_lines_even_when_the_hash_is_the_same() -> Result<()> {
        let inventory = VALID_SKELETON.replace(
            &format!("Pin: {PIN}\n"),
            &format!("Pin: {PIN}\nPin: {PIN}\n"),
        );
        expect_validation_error(&inventory, SIMPLE_ROADMAP, "exactly one `Pin:` line")?;
        Ok(())
    }

    #[test]
    fn rejects_a_trace_row_for_an_unknown_roadmap_task() -> Result<()> {
        let inventory = VALID_SKELETON.replace(
            "| Task | Register items | Findings | Obligations |\n| -- | -- | -- | -- |\n\n### Consumes",
            "| Task | Register items | Findings | Obligations |\n| -- | -- | -- | -- |\n| 29.9.9 | -- | -- | -- |\n\n### Consumes",
        );
        expect_validation_error(&inventory, SIMPLE_ROADMAP, "absent from the roadmap")?;
        Ok(())
    }

    #[test]
    fn rejects_a_missing_transitive_dependent_even_if_named_elsewhere() -> Result<()> {
        let inventory = transitive_skeleton()
            .replace("- 26.1.3: pending EP-M3.\n", "")
            .replace(
                "## Scope and definitions",
                "## Purpose\n26.1.3 appears only outside the trace matrix.\n\n## Scope and definitions",
            );
        expect_validation_error(&inventory, TRANSITIVE_ROADMAP, "missing: {\"26.1.3\"}")?;
        Ok(())
    }

    #[test]
    fn rejects_duplicate_register_identifiers() -> Result<()> {
        let inventory = VALID_SKELETON.replace(
            "| -- | -- | -- | -- | -- |\n\n### Site rows",
            "| -- | -- | -- | -- | -- |\n| A1/ast::Rule | Rule | -- | -- | -- |\n| A1/ast::Rule | Rule | -- | -- | -- |\n\n### Site rows",
        );
        expect_validation_error(&inventory, SIMPLE_ROADMAP, "should be unique")?;
        Ok(())
    }

    #[test]
    fn rejects_an_obligation_without_a_trace_reference() -> Result<()> {
        let inventory = VALID_SKELETON.replace(
            "## Compatibility obligations\n\nPending EP-M3.",
            "## Compatibility obligations\n\n- CO-1: Preserve an obligation.",
        );
        expect_validation_error(&inventory, SIMPLE_ROADMAP, "CO-1 has no trace-matrix")?;
        Ok(())
    }

    #[test]
    fn rejects_a_finding_without_an_owner() -> Result<()> {
        let inventory = VALID_SKELETON.replace(
            "## Findings\n\nPending EP-M3.",
            "## Findings\n\n- OBS-1: An observed finding has no owner.",
        );
        expect_validation_error(&inventory, SIMPLE_ROADMAP, "owning roadmap task")?;
        Ok(())
    }

    #[test]
    fn rejects_a_register_item_without_a_trace_or_unchanged_entry() -> Result<()> {
        let inventory = VALID_SKELETON.replace(
            "| -- | -- | -- | -- | -- |\n\n### Site rows",
            "| -- | -- | -- | -- | -- |\n| A1/ast::Rule | Rule | -- | -- | -- |\n\n### Site rows",
        );
        expect_validation_error(
            &inventory,
            SIMPLE_ROADMAP,
            "neither traced nor listed unchanged",
        )?;
        Ok(())
    }
}
