//! Supply the shared inventory fixtures used by the contract tests.

use anyhow::{Result, bail, ensure};

use super::validation::validate_inventory;

/// A syntactically valid 40-character lowercase hash for pin tests.
pub(super) const PIN: &str = "0123456789abcdef0123456789abcdef01234567";

/// A two-task roadmap with one direct dependent.
pub(super) const SIMPLE_ROADMAP: &str = concat!(
    "- [ ] 26.1.1. Root task.\n",
    "  - Dependencies: none.\n",
    "- [ ] 26.1.2. Child task.\n",
    "  - Dependencies: 26.1.1.\n",
);

/// A three-task roadmap exercising transitive dependents and prose.
pub(super) const TRANSITIVE_ROADMAP: &str = concat!(
    "- [ ] 26.1.1. Root task.\n",
    "  - Dependencies: none.\n",
    "- [ ] 26.1.2. Child task.\n",
    "  - Dependencies: 26.1.1; may proceed independently of 28.2 implementation.\n",
    "- [x] 26.1.3. Grandchild task.\n",
    "  - Dependencies: 26.1.2 and 29.2.1/29.2.2 policy.\n",
    "    Continued prose includes 27.1.1.\n",
    "  - Acceptance: this is not a dependency; 27.3.2.\n",
);

/// A roadmap whose heading and prose change the dependency set.
pub(super) const HEADING_BLEED_ROADMAP: &str = concat!(
    "- [ ] 26.3.2. Refactor the boundary.\n",
    "  - Dependencies: 26.3.1 and 26.2.3.\n",
    "## 27. Application and diagnostic ownership\n",
    "\n",
    "Entry: accepted RFC 0026 scope and the current inventory from 26.1.1.\n",
    "- [ ] 27.1.1. Own the diagnostics.\n",
    "  - Dependencies: 26.3.2.\n",
);

/// A minimal inventory that satisfies every structural check it exercises.
///
/// Each section carries either a populated table or an explicit pending note,
/// so replacing one section in a test changes only that section's meaning.
pub(super) const VALID_SKELETON: &str = concat!(
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
pub(super) fn expect_validation_error(
    inventory: &str,
    roadmap: &str,
    expected: &str,
) -> Result<()> {
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
pub(super) fn transitive_skeleton() -> String {
    VALID_SKELETON.replace(
        "- 26.1.2: pending EP-M3.\n",
        "- 26.1.2: pending EP-M3.\n- 26.1.3: pending EP-M3.\n",
    )
}

/// Add one register row and replace the skeleton's `Unchanged` body.
pub(super) fn inventory_with_register_item_and_unchanged(unchanged: &str) -> String {
    VALID_SKELETON
        .replace(
            "| -- | -- | -- | -- | -- |\n\n### Site rows",
            "| -- | -- | -- | -- | -- |\n| A1/ast::Rule | Rule | -- | -- | -- |\n\n### Site rows",
        )
        .replace(
            "### Unchanged\n\nNone yet.\n",
            &format!("### Unchanged\n\n{unchanged}"),
        )
}
