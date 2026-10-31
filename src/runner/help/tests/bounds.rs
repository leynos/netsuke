//! Bounded-output unit tests for the `netsuke help targets` text renderer.
//!
//! These tests pin the alignment cap: one manifest-controlled name must not
//! multiply padding across every other row, while names within the cap keep
//! the established aligned layout and JSON output stays verbatim.

use super::*;

/// Bytes allowed for localized headings, separators, and the blank line
/// between sections. The exact heading text depends on the active locale, so
/// the bound allows a generous constant that stays far below the amplification
/// this regression targets.
const LOCALIZED_OVERHEAD_BYTES: usize = 256;

/// Bytes allowed per row for the two localized markers and their separators.
/// A row can carry at most one default and one conditional marker.
const MARKER_ROW_BYTES: usize = 128;

/// Build a manifest whose only entries are the supplied build targets.
fn manifest_with_targets(targets: Vec<Target>) -> NetsukeManifest {
    NetsukeManifest {
        netsuke_version: Version::new(1, 0, 0),
        vars: crate::ast::Vars::default(),
        macros: Vec::new(),
        rules: Vec::new(),
        actions: Vec::new(),
        targets,
        defaults: Vec::new(),
    }
}

/// Derive the rendered-size bound from the row inputs and the alignment cap.
///
/// Every row contributes its name and description bytes, a two-space prefix,
/// a bounded padding field, the optional two-space description separator, an
/// optional marker field, and a newline. Localized heading and marker text is
/// covered by fixed allowances rather than the active locale.
fn text_output_upper_bound(entries: &[HelpEntry]) -> usize {
    let name_bytes: usize = entries
        .iter()
        .map(|entry| terminal_safe(&entry.name).len())
        .sum();
    let description_bytes: usize = entries
        .iter()
        .map(|entry| {
            entry
                .description
                .as_deref()
                .map_or(0, |description| terminal_safe(description).len())
        })
        .sum();
    LOCALIZED_OVERHEAD_BYTES
        + name_bytes
        + description_bytes
        + entries.len() * (MAX_HELP_ALIGNMENT_WIDTH + MARKER_ROW_BYTES + 5)
}

/// Assert that no rendered line carries trailing whitespace.
fn assert_no_trailing_whitespace(rendered: &str) -> Result<()> {
    anyhow::ensure!(
        rendered.lines().all(|line| !line.ends_with(' ')),
        "catalogue rows must not end in padding whitespace: {rendered:?}"
    );
    Ok(())
}

/// Render a section and return its lines, rejecting output above the bound.
fn render_bounded_lines(manifest: &NetsukeManifest) -> Result<(String, Vec<String>)> {
    let entries = build_catalogue(manifest);
    let rendered = render_text(&entries, theme_prefs(ThemePreference::Unicode));
    anyhow::ensure!(
        rendered.len() <= text_output_upper_bound(&entries),
        "catalogue length {} exceeds bounded allocation {} for {} rows",
        rendered.len(),
        text_output_upper_bound(&entries),
        entries.len()
    );
    assert_no_trailing_whitespace(&rendered)?;
    let lines = rendered.lines().map(str::to_owned).collect();
    Ok((rendered, lines))
}

/// Guard the padding amplification reported by the issue: one 4,096-character
/// name among 2,000 short rows, half without descriptions.
#[test]
fn text_catalogue_bounds_padding_with_one_very_long_name() -> Result<()> {
    let long_name = "L".repeat(4_096);
    let mut targets = vec![catalogue_target(
        vec![long_name.clone()],
        Some("long row description".to_owned()),
        false,
        false,
    )];
    targets.extend((0..2_000usize).map(|index| {
        catalogue_target(
            vec![format!("s{index}")],
            index
                .is_multiple_of(2)
                .then(|| format!("short row description {index}")),
            false,
            false,
        )
    }));
    let manifest = manifest_with_targets(targets);

    let (rendered, lines) = render_bounded_lines(&manifest)?;

    anyhow::ensure!(
        rendered.contains(&long_name),
        "the long name must render intact rather than truncated"
    );
    anyhow::ensure!(
        lines.iter().any(|line| line == "  s1"),
        "a short row without a description must end after its name: {rendered:?}"
    );
    Ok(())
}

/// Guard the same amplification when one target expands a name list into many
/// rows, exercising the shared `append_target_entries` path.
#[test]
fn text_catalogue_bounds_padding_for_one_target_name_list() -> Result<()> {
    let long_name = "L".repeat(4_096);
    let mut names = vec![long_name.clone()];
    names.extend((0..2_000).map(|index| format!("s{index}")));
    let manifest = manifest_with_targets(vec![catalogue_target(
        names,
        Some("shared description".to_owned()),
        false,
        false,
    )]);

    let (rendered, lines) = render_bounded_lines(&manifest)?;

    anyhow::ensure!(
        rendered.contains(&long_name),
        "the long name must render intact rather than truncated"
    );
    let short_description_columns: Vec<usize> = lines
        .iter()
        .filter(|line| !line.contains(&long_name))
        .filter_map(|line| line.find("shared description"))
        .collect();
    anyhow::ensure!(
        short_description_columns.len() == 2_000,
        "every short row should carry the shared description"
    );
    let aligned = short_description_columns.first().is_some_and(|first| {
        short_description_columns
            .iter()
            .all(|column| column == first)
    });
    anyhow::ensure!(
        aligned,
        "names within the cap must align to the widest qualifying name"
    );
    Ok(())
}

/// Rows that fit within the cap keep the established aligned layout, and rows
/// with no following column end immediately after the name.
#[test]
fn text_catalogue_keeps_alignment_within_the_cap() -> Result<()> {
    let manifest = manifest_with_targets(vec![
        catalogue_target(vec!["a".to_owned()], Some("first".to_owned()), false, false),
        catalogue_target(
            vec!["longer-name".to_owned()],
            Some("second".to_owned()),
            false,
            false,
        ),
        catalogue_target(vec!["mid".to_owned()], None, false, false),
    ]);

    let (_rendered, lines) = render_bounded_lines(&manifest)?;

    anyhow::ensure!(
        lines.iter().any(|line| line == "  a            first"),
        "the short row should align under the widest name: {lines:?}"
    );
    anyhow::ensure!(
        lines.iter().any(|line| line == "  longer-name  second"),
        "the widest row should use the two-space separator: {lines:?}"
    );
    anyhow::ensure!(
        lines.iter().any(|line| line == "  mid"),
        "a row with no following column must not be padded: {lines:?}"
    );
    Ok(())
}

/// A name above the cap receives no padding, while a name exactly at the cap
/// still sets the alignment width for the section.
#[test]
fn text_catalogue_does_not_pad_names_above_the_cap() -> Result<()> {
    let at_cap = "c".repeat(MAX_HELP_ALIGNMENT_WIDTH);
    let overlong = "o".repeat(MAX_HELP_ALIGNMENT_WIDTH + 1);
    let manifest = manifest_with_targets(vec![
        catalogue_target(
            vec![at_cap.clone()],
            Some("at the cap".to_owned()),
            false,
            false,
        ),
        catalogue_target(
            vec![overlong.clone()],
            Some("over the cap".to_owned()),
            false,
            false,
        ),
        catalogue_target(
            vec!["short".to_owned()],
            Some("short row".to_owned()),
            false,
            false,
        ),
    ]);

    let (_rendered, lines) = render_bounded_lines(&manifest)?;

    anyhow::ensure!(
        lines
            .iter()
            .any(|line| line == &format!("  {overlong}  over the cap")),
        "an overlong name must use only the two-space description separator: {lines:?}"
    );
    anyhow::ensure!(
        lines
            .iter()
            .any(|line| line == &format!("  {at_cap}  at the cap")),
        "a name exactly at the cap must set the alignment width: {lines:?}"
    );
    anyhow::ensure!(
        lines
            .iter()
            .any(|line| line == &format!("  short{}  short row", " ".repeat(35))),
        "a short row must align to the widest qualifying name: {lines:?}"
    );
    Ok(())
}

/// A row whose only following column is a marker is padded; a row with no
/// following column is not.
#[test]
fn text_catalogue_pads_marker_rows_only() -> Result<()> {
    let prefs = theme_prefs(ThemePreference::Unicode);
    let marker = default_marker(prefs);
    let mut manifest = manifest_with_targets(vec![
        catalogue_target(vec!["w".to_owned()], None, false, false),
        catalogue_target(
            vec!["wide-name".to_owned()],
            Some("described".to_owned()),
            false,
            false,
        ),
        catalogue_target(vec!["plain".to_owned()], None, false, false),
    ]);
    manifest.defaults.push("w".to_owned());

    let entries = build_catalogue(&manifest);
    let rendered = render_text(&entries, prefs);
    let padded_marker_row = format!("  w{} {marker}", " ".repeat("wide-name".len() - 1));

    anyhow::ensure!(
        rendered.lines().any(|line| line == padded_marker_row),
        "a marker-only row must be padded to the alignment width: {rendered:?}"
    );
    anyhow::ensure!(
        rendered.lines().any(|line| line == "  plain"),
        "a row without a description or marker must not be padded: {rendered:?}"
    );
    assert_no_trailing_whitespace(&rendered)
}

/// JSON output carries names and descriptions verbatim, independent of the
/// text renderer's alignment bound.
#[test]
fn json_catalogue_preserves_long_names() -> Result<()> {
    let long_name = "J".repeat(4_096);
    let manifest = manifest_with_targets(vec![catalogue_target(
        vec![long_name.clone()],
        Some("json description".to_owned()),
        false,
        false,
    )]);
    let rendered = render_json(&build_catalogue(&manifest), None)?;
    let document: serde_json::Value =
        serde_json::from_str(&rendered).context("parse rendered JSON catalogue")?;

    anyhow::ensure!(
        document
            .pointer("/result/targets/0/name")
            .and_then(serde_json::Value::as_str)
            == Some(long_name.as_str()),
        "JSON must retain the full name: {document}"
    );
    anyhow::ensure!(
        document
            .pointer("/result/targets/0/description")
            .and_then(serde_json::Value::as_str)
            == Some("json description"),
        "JSON must retain the description: {document}"
    );
    Ok(())
}
