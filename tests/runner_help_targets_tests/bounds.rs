//! Bounded-output scenarios for the `netsuke help targets` catalogue.
//!
//! The text catalogue aligns descriptions to a name column. These scenarios
//! pin the rule that one manifest-controlled name cannot widen that column for
//! every other row, and that a description shared by a target's name list is
//! repeated in the output once per name rather than being bounded by the
//! manifest that states it once.

use super::*;
use netsuke::locale_resolution::{LocaleEnvProvider, SystemEnv};
use std::fmt::Write as _;
use std::io::Read as _;

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

/// Build a `netsuke help targets` command whose child environment this test
/// alone decides.
///
/// A standard command is returned rather than an `assert_cmd` wrapper so a
/// test can also spawn it with piped streams and inspect the child directly.
///
/// An inherited `NETSUKE_JSON` would route the run through the JSON renderer,
/// and `NETSUKE_FILE` or `NETSUKE_CONFIG` would select another manifest or
/// user configuration, so either mismatch would describe the ambient shell
/// rather than the catalogue. Clearing first makes each assertion a statement
/// about this commit. The locale is pinned on the command line for the same
/// reason, and `PATH` plus `SystemRoot` are forwarded because they are
/// platform plumbing: the child still has to start and to locate executables.
/// Both are read through the environment seam rather than `std::env`.
///
/// Clearing removes `HOME`, `XDG_CONFIG_HOME`, `APPDATA` and `LOCALAPPDATA`, so
/// no per-user configuration survives. Unix discovery still falls back to
/// `/etc/xdg` when `XDG_CONFIG_DIRS` is unset, so that variable names an absent
/// directory inside the temporary workspace rather than the host's system
/// configuration.
fn help_targets_command(temp_path: &Utf8Path, manifest_path: &Utf8Path) -> std::process::Command {
    let isolated_config_dirs = temp_path.join(".netsuke-absent-xdg-config-dirs");
    let mut command = std::process::Command::new(assert_cmd::cargo::cargo_bin("netsuke"));
    command
        .current_dir(temp_path)
        .env_clear()
        .env("NETSUKE_NINJA", "/definitely-not-a-ninja-binary")
        .env("XDG_CONFIG_DIRS", isolated_config_dirs.as_std_path())
        .args(["--locale", "en-US", "--file"])
        .arg(manifest_path)
        .args(["help", "targets"]);
    if let Some(path) = SystemEnv.var("PATH") {
        command.env("PATH", path);
    }
    if cfg!(windows)
        && let Some(system_root) = SystemEnv.var("SystemRoot")
    {
        command.env("SystemRoot", system_root);
    }
    command
}

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
    let mut command = help_targets_command(temp_path, &fixture.manifest_path);
    let output = command
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

/// Bytes in the description shared by every name in the amplification fixture.
const SHARED_DESCRIPTION_BYTES: usize = 8 * 1024;
/// Names that share that one description.
const SHARED_DESCRIPTION_NAMES: usize = 500;

/// Write a manifest whose one target declares every name in its name list.
///
/// The manifest states the description once and lists the names, while the
/// catalogue repeats that description once per name. The rendered output is
/// therefore many times the size of the manifest that defines it, and stays
/// inside the manifest budgets, which charge the description once.
fn write_shared_description_manifest(temp_path: &Utf8Path) -> Result<(Utf8PathBuf, String)> {
    let description = "D".repeat(SHARED_DESCRIPTION_BYTES);
    let mut manifest = String::from("netsuke_version: \"1.0.0\"\ntargets:\n  - name:\n");
    for index in 0..SHARED_DESCRIPTION_NAMES {
        writeln!(manifest, "      - shared{index:04}")?;
    }
    writeln!(manifest, "    description: {description}")?;
    writeln!(manifest, "    command: touch shared-ran")?;

    let manifest_path = temp_path.join("Netsukefile");
    let workspace = Dir::open_ambient_dir(temp_path, ambient_authority())
        .context("open shared-description help-targets fixture directory")?;
    workspace
        .write("Netsukefile", &manifest)
        .with_context(|| format!("write manifest to {}", manifest_path.as_str()))?;
    Ok((manifest_path, description))
}

/// Guard the accumulated-catalogue amplification end to end: one target whose
/// name list shares a long description must render that description once per
/// name, with no recipe executed and no row left carrying trailing padding.
#[rstest]
fn help_targets_renders_one_shared_description_per_name() -> Result<()> {
    let temp = tempfile::tempdir().context("create shared-description workspace")?;
    let temp_path = Utf8Path::from_path(temp.path()).context("temporary path should be UTF-8")?;
    let (manifest_path, description) = write_shared_description_manifest(temp_path)?;
    let workspace = Dir::open_ambient_dir(temp_path, ambient_authority())
        .context("open shared-description fixture directory")?;
    let mut command = help_targets_command(temp_path, &manifest_path);
    let output = command
        .output()
        .context("run help targets against the shared-description manifest")?;
    ensure!(
        output.status.success(),
        "help targets should succeed without starting Ninja; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).context("stdout should be valid UTF-8")?;
    ensure!(
        stdout.matches(description.as_str()).count() == SHARED_DESCRIPTION_NAMES,
        "every name should render the shared description once"
    );
    ensure!(
        stdout.lines().all(|line| !line.ends_with(' ')),
        "catalogue rows must not end in padding whitespace"
    );
    ensure!(
        workspace.open("shared-ran").is_err() && workspace.open(".netsuke").is_err(),
        "help targets must not execute recipes or create build output"
    );
    Ok(())
}

/// Bytes read before the reader closes, far below the catalogue size.
const EARLY_CLOSE_BYTES: u64 = 64;

/// Guard the closed-pipe path end to end: a reader that stops early must not
/// turn the run into an error.
///
/// The long-name fixture's catalogue is far larger than a pipe buffer, so the
/// child is still writing when the reader is dropped. The resulting broken
/// pipe must count as a completed write, and the process must still exit
/// successfully, which is what keeps `netsuke help targets | head` clean.
#[rstest]
fn help_targets_survives_an_early_closing_reader() -> Result<()> {
    let temp = tempfile::tempdir().context("create early-close workspace")?;
    let temp_path = Utf8Path::from_path(temp.path()).context("temporary path should be UTF-8")?;
    let fixture = write_long_name_manifest(temp_path)?;
    let mut command = help_targets_command(temp_path, &fixture.manifest_path);
    let mut child = command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("spawn help targets with a piped catalogue")?;
    let stdout = child
        .stdout
        .take()
        .context("the catalogue should be captured through a pipe")?;
    let mut reader = std::io::BufReader::new(stdout);
    let mut head = Vec::new();
    reader
        .by_ref()
        .take(EARLY_CLOSE_BYTES)
        .read_to_end(&mut head)
        .context("read the start of the catalogue")?;
    ensure!(
        !head.is_empty(),
        "the catalogue should start before the reader closes"
    );
    drop(reader);

    let output = child
        .wait_with_output()
        .context("wait for help targets after the reader closed")?;

    ensure!(
        output.status.success(),
        "a closed reader must not fail the run; status {:?}, stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}
