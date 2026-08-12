//! Public-CLI end-to-end tests for serial dependency sidecar publication.

use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use tempfile::TempDir;

const SERIAL_MANIFEST: &str = concat!(
    "netsuke_version: '1.0.0'\n",
    "targets:\n",
    "  - name: check-fmt\n",
    "    command: test ! -e check-fmt && echo one >> order.log && touch check-fmt\n",
    "  - name: lint\n",
    "    command: test -e check-fmt && echo two >> order.log && touch lint\n",
    "  - name: test\n",
    "    command: test -e lint && echo three >> order.log && touch test\n",
    "  - name: all\n",
    "    dependency_order: serial\n",
    "    deps: [check-fmt, lint, test]\n",
    "    command: test -e test && echo all >> order.log && touch all\n",
    "defaults: [all]\n",
);

const FAILING_MANIFEST: &str = concat!(
    "netsuke_version: '1.0.0'\n",
    "targets:\n",
    "  - name: first\n",
    "    command: exit 1\n",
    "  - name: later\n",
    "    command: touch later-marker && touch later\n",
    "  - name: all\n",
    "    dependency_order: serial\n",
    "    deps: [first, later]\n",
    "    command: touch all-marker && touch all\n",
    "defaults: [all]\n",
);

fn cli_workspace(manifest: &str) -> Result<(TempDir, Dir)> {
    let directory = tempfile::tempdir()?;
    let root_path = Utf8PathBuf::from_path_buf(directory.path().to_path_buf()).map_err(|path| {
        anyhow::anyhow!("temporary CLI workspace is not UTF-8: {}", path.display())
    })?;
    let root = Dir::open_ambient_dir(&root_path, ambient_authority())
        .context("open temporary CLI workspace")?;
    root.write("Netsukefile", manifest)
        .context("write serial CLI manifest")?;
    Ok((directory, root))
}

fn run_netsuke_build(directory: &TempDir) -> Result<std::process::Output> {
    assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .current_dir(directory.path())
        .args(["-j", "3", "build"])
        .output()
        .context("run netsuke CLI serial build")
}

#[test]
fn build_materializes_sidecars_and_runs_serial_dependencies() -> Result<()> {
    let (directory, root) = cli_workspace(SERIAL_MANIFEST)?;
    let output = run_netsuke_build(&directory)?;
    ensure!(
        output.status.success(),
        "netsuke CLI build failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let dyndep = root
        .open_dir(".netsuke/dyndep")
        .context("CLI build must materialize the dyndep directory")?;
    let sidecar_count = dyndep
        .entries()
        .context("enumerate CLI-materialized dyndep sidecars")?
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().ok())
        .filter(|name| {
            Utf8Path::new(name)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("dd"))
        })
        .count();
    ensure!(sidecar_count > 0, "CLI build must materialize .dd sidecars");
    let order = root
        .read_to_string("order.log")
        .context("read CLI order log")?;
    ensure!(
        order.lines().collect::<Vec<_>>() == ["one", "two", "three", "all"],
        "CLI serial dependencies ran out of order: {order:?}"
    );
    Ok(())
}

#[test]
fn build_short_circuits_after_first_serial_failure() -> Result<()> {
    let (directory, root) = cli_workspace(FAILING_MANIFEST)?;
    let output = run_netsuke_build(&directory)?;
    ensure!(!output.status.success(), "netsuke CLI build should fail");
    ensure!(
        root.open("later-marker").is_err(),
        "later serial dependency ran"
    );
    ensure!(
        root.open("all-marker").is_err(),
        "aggregate target ran after failure"
    );
    Ok(())
}
