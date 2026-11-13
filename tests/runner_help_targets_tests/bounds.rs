//! Bounded-output scenarios for the `netsuke help targets` catalogue.
//!
//! The text catalogue aligns descriptions to a name column. These scenarios
//! pin the rule that one manifest-controlled name cannot widen that column for
//! every other row, so the rendered output stays linear in the manifest's own
//! text plus a fixed per-row allowance.

use super::*;
use std::fmt::Write as _;

/// Display columns one name may contribute to catalogue alignment, mirroring
/// `MAX_HELP_ALIGNMENT_WIDTH` in `src/runner/help/mod.rs`.
const HELP_ALIGNMENT_WIDTH: usize = 40;
/// Fixed allowance for localized headings and inter-section spacing.
const HELP_FIXED_OVERHEAD_BYTES: usize = 256;
/// Per-row allowance for the two-space prefix, separators, newline, and
/// localized markers.
const HELP_ROW_OVERHEAD_BYTES: usize = 133;
/// Description carried by the single long-named target.
const LONG_ROW_DESCRIPTION: &str = "Long row description";

/// Derive the maximum catalogue length from the rows a fixture generates.
fn help_catalogue_upper_bound(
    long_name: &str,
    long_description: &str,
    short_names: &[String],
    short_descriptions: &[Option<String>],
) -> usize {
    let name_bytes = long_name.len() + short_names.iter().map(String::len).sum::<usize>();
    let description_bytes = long_description.len()
        + short_descriptions
            .iter()
            .flatten()
            .map(String::len)
            .sum::<usize>();
    let rows = 1 + short_names.len();
    HELP_FIXED_OVERHEAD_BYTES
        + name_bytes
        + description_bytes
        + rows * (HELP_ALIGNMENT_WIDTH + HELP_ROW_OVERHEAD_BYTES)
}

/// Generated long-name fixture: the manifest on disk and the row inputs used
/// to derive the expected output bound.
struct LongNameManifest {
    /// Path to the written manifest.
    manifest_path: Utf8PathBuf,
    /// The single 4,096-character target name.
    long_name: String,
    /// The many short target names.
    short_names: Vec<String>,
    /// Descriptions for alternate short names; `None` rows have no
    /// description column.
    short_descriptions: Vec<Option<String>>,
}

/// Write a manifest combining one very long name with many short names, half
/// of which carry descriptions, and return it with the generated row inputs.
fn write_long_name_manifest(temp_path: &Utf8Path) -> Result<LongNameManifest> {
    let long_name = "L".repeat(4_096);
    let short_names: Vec<String> = (0..2_000).map(|index| format!("row{index:04}")).collect();
    let short_descriptions: Vec<Option<String>> = (0..short_names.len())
        .map(|index| {
            index
                .is_multiple_of(2)
                .then(|| format!("Short row {index}"))
        })
        .collect();

    let mut manifest = String::from("netsuke_version: \"1.0.0\"\ntargets:\n");
    writeln!(manifest, "  - name: \"{long_name}\"")?;
    writeln!(manifest, "    description: {LONG_ROW_DESCRIPTION}")?;
    writeln!(manifest, "    command: touch long-row-ran")?;
    for (name, description) in short_names.iter().zip(&short_descriptions) {
        writeln!(manifest, "  - name: {name}")?;
        if let Some(description_text) = description {
            writeln!(manifest, "    description: {description_text}")?;
        }
        writeln!(manifest, "    command: touch {name}-ran")?;
    }

    let manifest_path = temp_path.join("Netsukefile");
    let workspace = Dir::open_ambient_dir(temp_path, ambient_authority())
        .context("open long-name help-targets fixture directory")?;
    workspace
        .write("Netsukefile", &manifest)
        .with_context(|| format!("write manifest to {}", manifest_path.as_str()))?;
    Ok(LongNameManifest {
        manifest_path,
        long_name,
        short_names,
        short_descriptions,
    })
}

/// Guard the reported amplification end to end: one 4,096-character target
/// name among 2,000 short targets must not multiply padding across the
/// catalogue, and no recipe may run.
#[rstest]
fn help_targets_bounds_alignment_padding_for_one_long_name() -> Result<()> {
    let temp = tempfile::tempdir().context("create long-name help-targets workspace")?;
    let temp_path = Utf8Path::from_path(temp.path()).context("temporary path should be UTF-8")?;
    let fixture = write_long_name_manifest(temp_path)?;
    let workspace = Dir::open_ambient_dir(temp_path, ambient_authority())
        .context("open long-name help-targets fixture directory")?;
    let output = assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .current_dir(temp_path)
        .env("NETSUKE_NINJA", "/definitely-not-a-ninja-binary")
        .arg("--file")
        .arg(&fixture.manifest_path)
        .arg("help")
        .arg("targets")
        .output()
        .context("run help targets against the long-name manifest")?;
    ensure!(
        output.status.success(),
        "help targets should succeed without starting Ninja; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).context("stdout should be valid UTF-8")?;
    let bound = help_catalogue_upper_bound(
        &fixture.long_name,
        LONG_ROW_DESCRIPTION,
        &fixture.short_names,
        &fixture.short_descriptions,
    );
    ensure!(
        stdout.len() <= bound,
        "catalogue length {} exceeds bounded allocation {bound} for {} rows",
        stdout.len(),
        fixture.short_names.len() + 1
    );
    ensure!(
        stdout.contains(&fixture.long_name),
        "the long name must render intact rather than truncated"
    );
    ensure!(
        stdout.lines().all(|line| !line.ends_with(' ')),
        "catalogue rows must not end in padding whitespace"
    );
    ensure!(
        stdout.lines().any(|line| line == "  row0001"),
        "a row without a description must end after its name"
    );
    let short_columns: Vec<usize> = stdout
        .lines()
        .filter(|line| !line.contains(&fixture.long_name))
        .filter_map(|line| line.find("Short row"))
        .collect();
    ensure!(
        short_columns.len() == 1_000,
        "every short row with a description should be present"
    );
    let aligned = short_columns
        .first()
        .is_some_and(|first| short_columns.iter().all(|column| column == first));
    ensure!(
        aligned,
        "short rows must align to the widest qualifying name"
    );
    ensure!(
        workspace.open("long-row-ran").is_err()
            && workspace.open("row0000-ran").is_err()
            && workspace.open(".netsuke").is_err(),
        "help targets must not execute recipes or create build output"
    );
    Ok(())
}
