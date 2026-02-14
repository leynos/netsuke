//! Shared assertion helpers for content-based checks in BDD steps.
//!
//! Provides unified assertion functions for common patterns across step
//! definition files, particularly for checking that content contains expected
//! fragments.

use anyhow::{Context, Result, ensure};
use rstest_bdd::Slot;
pub use test_support::fluent::normalize_fluent_isolates;

/// Assert that optional content contains an expected fragment.
///
/// This unifies the pattern from `ninja.rs::assert_content_contains`.
///
/// # Arguments
///
/// * `content` - Optional content to check
/// * `fragment` - Expected substring to find
/// * `content_label` - Human-readable label for error messages
///
/// # Example
///
/// ```ignore
/// assert_optional_contains(
///     world.ninja_content.get(),
///     fragment.as_str(),
///     "ninja content",
/// )?;
/// ```
pub fn assert_optional_contains(
    content: Option<String>,
    fragment: &str,
    content_label: &str,
) -> Result<()> {
    let text = content.context(format!("{content_label} should be available"))?;
    let normalized_text = normalize_fluent_isolates(&text);
    let normalized_fragment = normalize_fluent_isolates(fragment);
    ensure!(
        normalized_text.contains(&normalized_fragment),
        "{content_label} should contain '{fragment}'"
    );
    Ok(())
}

/// Assert that content from a slot contains an expected fragment.
///
/// This unifies the pattern from `manifest_command.rs::assert_output_contains`.
///
/// # Arguments
///
/// * `slot` - Slot containing the content to check
/// * `fragment` - Expected substring to find
/// * `content_label` - Human-readable label for error messages
///
/// # Example
///
/// ```ignore
/// assert_slot_contains(
///     &world.command_stdout,
///     fragment.as_str(),
///     "stdout",
/// )?;
/// ```
pub fn assert_slot_contains(
    slot: &Slot<String>,
    fragment: &str,
    content_label: &str,
) -> Result<()> {
    let content = slot
        .get()
        .with_context(|| format!("no {content_label} captured"))?;
    let normalized_content = normalize_fluent_isolates(&content);
    let normalized_fragment = normalize_fluent_isolates(fragment);
    ensure!(
        normalized_content.contains(&normalized_fragment),
        "expected {content_label} to contain '{fragment}', got '{content}'"
    );
    Ok(())
}
