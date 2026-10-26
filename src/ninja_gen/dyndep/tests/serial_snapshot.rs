//! Snapshot the complete emitted bundle for a multi-stage serial edge.
//!
//! The fixture keeps several dependencies and a default target in view so a
//! reviewed snapshot records relay edges, sidecar production, gate chaining,
//! and sidecar contents together. Hash placeholders preserve each distinct
//! identity and every reference to it while keeping the snapshot portable.

use super::{graph_with_edge, serial_edge};
use crate::ninja_gen::dyndep::generate_bundle_for_shell;
use crate::ninja_gen::{GeneratedNinja, RecipeShell};
use crate::snapshot_test_support::snapshot_settings;
use anyhow::Result;
use camino::Utf8PathBuf;
use insta::assert_snapshot;
use std::fmt::Write as _;

/// Render the complete bundle with stable path separators and hash names.
///
/// # Errors
///
/// Return an error if formatting the rendered bundle fails.
fn rendered_serial_bundle(bundle: &GeneratedNinja) -> Result<String> {
    let mut rendered = format!("=== build.ninja ===\n{}", bundle.build_file());
    for (index, sidecar) in bundle.dyndep_files().iter().enumerate() {
        write!(
            rendered,
            "\n=== sidecar {index}: {} ===\n",
            sidecar.relative_path()
        )?;
        rendered.push_str(sidecar.content());
    }

    let platform_normalized = rendered.replace('\\', "/");
    let serial_normalized =
        replace_namespace_hashes(&platform_normalized, ".netsuke/serial/", '/', "serial")?;
    replace_namespace_hashes(&serial_normalized, ".netsuke/dyndep/", '.', "dyndep")
}

/// Replace generated digests by first-seen placeholders within one namespace.
///
/// The same digest always receives the same placeholder, while distinct
/// digests keep distinct names in their original encounter order. This keeps
/// cross-references visible and leaves statement and sidecar ordering intact.
///
/// # Errors
///
/// Return an error if formatting a normalized identity fails.
fn replace_namespace_hashes(
    text: &str,
    prefix: &str,
    delimiter: char,
    label: &str,
) -> Result<String> {
    let mut output = String::with_capacity(text.len());
    let mut identities = Vec::<String>::new();
    let mut remaining = text;

    while let Some(prefix_start) = remaining.find(prefix) {
        let Some(before_prefix) = remaining.get(..prefix_start) else {
            output.push_str(remaining);
            break;
        };
        let Some(with_prefix) = remaining.get(prefix_start..) else {
            output.push_str(remaining);
            break;
        };
        let Some(after_prefix) = with_prefix.strip_prefix(prefix) else {
            output.push_str(remaining);
            break;
        };
        output.push_str(before_prefix);
        output.push_str(prefix);
        remaining = after_prefix;

        let Some(digest) = remaining.get(..64).filter(|digest| {
            digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        }) else {
            continue;
        };
        let Some(suffix) = remaining.get(64..) else {
            continue;
        };
        if !suffix.starts_with(delimiter) {
            continue;
        }

        let ordinal = identities
            .iter()
            .position(|seen| seen == digest)
            .map_or_else(
                || {
                    identities.push(digest.to_owned());
                    identities.len()
                },
                |index| index + 1,
            );
        write!(output, "<{label}-{ordinal:02}>")?;
        remaining = suffix;
    }

    output.push_str(remaining);
    Ok(output)
}

/// Assert a serial bundle snapshot that includes its default target.
#[test]
fn serial_bundle_with_relays_matches_snapshot() -> Result<()> {
    let mut graph = graph_with_edge(serial_edge("all", &["check-fmt", "lint", "test"]));
    graph.default_targets = vec![Utf8PathBuf::from("all")];
    // Fix the recipe shell so the snapshot compares the same bundle on every host.
    let bundle = generate_bundle_for_shell(&graph, RecipeShell::Posix)?;
    let rendered = rendered_serial_bundle(&bundle)?;

    snapshot_settings("ninja_dyndep").bind(|| {
        assert_snapshot!("serial_bundle_with_relays", rendered);
    });
    Ok(())
}
