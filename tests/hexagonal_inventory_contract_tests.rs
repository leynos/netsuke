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
#[cfg(test)]
#[path = "hexagonal_inventory_contract/tests.rs"]
mod tests;
#[path = "hexagonal_inventory_contract/validation.rs"]
mod validation;

use roadmap::{dependents_of, roadmap_tasks};
use validation::{validate_inventory, validate_populated};

const INVENTORY_PATH: &str = "docs/hexagonal-hardening-inventory.md";
const ROADMAP_PATH: &str = "docs/roadmap-hexagonal-hardening.md";

/// Number of roadmap tasks that transitively depend on 26.1.1.
///
/// This guards the roadmap parser against silently dropping dependents, which
/// would leave trace coverage with nothing to check and let it pass vacuously.
/// The roadmap is read live, not at the inventory's pin, so adding a dependent
/// task requires extending the trace matrix and this count together.
const EXPECTED_DEPENDENT_COUNT: usize = 25;

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
    let dependents = dependents_of(&tasks, "26.1.1");
    ensure!(
        dependents.len() == EXPECTED_DEPENDENT_COUNT,
        concat!(
            "roadmap should list {} tasks depending on 26.1.1, found {}; ",
            "update the inventory trace matrix and this count together"
        ),
        EXPECTED_DEPENDENT_COUNT,
        dependents.len()
    );
    validate_populated(&inventory)?;
    validate_inventory(&inventory, &roadmap)
}
