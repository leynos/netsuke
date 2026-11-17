//! Property regressions for the `netsuke help targets` text layout.
//!
//! The alignment cap, the trailing-whitespace rule, and the padded description
//! column are invariants over arbitrary names, descriptions, and marker
//! combinations. The fixed-input regressions in `bounds.rs` pin specific
//! boundaries; these cases check the same rules across the input space, where
//! a reader cannot audit them from a small table.

use super::bounds::{manifest_with_targets, text_output_upper_bound};
use super::*;

/// One generated row: name, description, conditional flag, and default flag.
type Row = (String, Option<String>, bool, bool);

/// Generate catalogue rows spanning short, capped, and over-long names.
///
/// Names are drawn from an alphabet that excludes `~`, which every description
/// starts with, so a row's description column can be found unambiguously.
/// Descriptions end in a non-space character because the renderer emits them
/// verbatim: a description that ends in whitespace would put that whitespace at
/// the end of the line, so the no-trailing-padding invariant is stated over
/// content that does not itself end in a space.
fn catalogue_rows() -> impl Strategy<Value = Vec<Row>> {
    proptest::collection::vec(
        (
            "[A-Za-z0-9_.-]{1,60}",
            prop_oneof![Just(None), "~[ -z]{0,119}[!-z]".prop_map(Some)],
            any::<bool>(),
            any::<bool>(),
        ),
        1..8,
    )
}

/// Prefix each generated name with its row index.
///
/// The index makes names unique and prefix-free, so a rendered line identifies
/// exactly one row even when two rows share a description.
fn indexed_name(index: usize, name: &str) -> String {
    format!("r{index}-{name}")
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 64,
        ..ProptestConfig::default()
    })]

    /// Every rendered catalogue stays bounded, renders names intact, carries
    /// no trailing padding, and aligns descriptions to the capped width.
    #[test]
    fn text_catalogue_holds_its_layout_invariants(rows in catalogue_rows()) {
        let targets = rows
            .iter()
            .enumerate()
            .map(|(index, (name, description, conditional, _))| {
                catalogue_target(
                    vec![indexed_name(index, name)],
                    description.clone(),
                    false,
                    *conditional,
                )
            })
            .collect();
        let mut manifest = manifest_with_targets(targets);
        manifest.defaults = rows
            .iter()
            .enumerate()
            .filter(|(_, (_, _, _, is_default))| *is_default)
            .map(|(index, (name, _, _, _))| indexed_name(index, name))
            .collect();

        let entries = build_catalogue(&manifest);
        let rendered = render_text_string(&entries, theme_prefs(ThemePreference::Unicode))
            .expect("the text catalogue should render into a string");
        prop_assert!(
            rendered.len() <= text_output_upper_bound(&entries),
            "catalogue length {} exceeds the bounded allocation {}",
            rendered.len(),
            text_output_upper_bound(&entries)
        );
        for line in rendered.lines() {
            prop_assert!(!line.ends_with(' '), "row must not end in padding: {line:?}");
        }

        let width = entries
            .iter()
            .map(|entry| UnicodeWidthStr::width(terminal_safe(&entry.name).as_ref()))
            .filter(|name_width| *name_width <= MAX_HELP_ALIGNMENT_WIDTH)
            .max()
            .unwrap_or(0);
        for entry in &entries {
            let name = terminal_safe(&entry.name);
            prop_assert!(
                rendered.contains(name.as_ref()),
                "name must render intact: {name:?}"
            );
            let Some(description) = entry
                .description
                .as_deref()
                .filter(|text| !text.is_empty())
            else {
                continue;
            };
            let prefix = format!("  {name}");
            let line = rendered
                .lines()
                .find(|line| line.starts_with(&prefix))
                .expect("every name should have a rendered row");
            let name_width = UnicodeWidthStr::width(name.as_ref());
            let expected = if name_width <= MAX_HELP_ALIGNMENT_WIDTH {
                width
            } else {
                name_width
            } + 4;
            prop_assert_eq!(
                line.find(description),
                Some(expected),
                "description column for {:?}",
                name
            );
        }
    }
}
