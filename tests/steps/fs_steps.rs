//! Steps for preparing file-system fixtures used in Jinja tests.

use crate::CliWorld;
use anyhow::{Context, Result, anyhow, bail, ensure};
use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs::FileTypeExt as CapFileTypeExt, fs_utf8::Dir};
use cucumber::given;
use rustix::fs::{Dev, FileType as RxFileType, Mode, mknodat};
use std::os::unix::fs::FileTypeExt;
use test_support::env::set_var;

fn setup_workspace() -> Result<(tempfile::TempDir, Utf8PathBuf, Dir)> {
    let temp = tempfile::tempdir().context("create tempdir for file-type workspace")?;
    let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
        .map_err(|path| anyhow!("workspace path is not valid UTF-8: {path:?}"))?;
    let handle = Dir::open_ambient_dir(&root, ambient_authority())
        .context("open ambient dir for file-type workspace")?;
    Ok((temp, root, handle))
}

fn create_basic_fixtures(handle: &Dir) -> Result<()> {
    handle.create_dir("dir").context("create dir fixture")?;
    handle
        .write("file", b"data")
        .context("write file fixture")?;
    handle
        .symlink("file", "link")
        .context("create symlink fixture")?;
    // FIFO creation is unprivileged and should succeed.
    mknodat(
        handle,
        "pipe",
        RxFileType::Fifo,
        Mode::RUSR | Mode::WUSR,
        Dev::default(),
    )
    .map_err(|err| anyhow!("create fifo fixture: {err}"))?;
    Ok(())
}

/// Configuration for a device node fixture and its fallback path.
#[derive(Copy, Clone)]
struct DeviceConfig<'a> {
    file_type: RxFileType,
    fallback_path: &'a str,
}

fn find_block_device_fallback() -> Result<Utf8PathBuf> {
    let entries = std::fs::read_dir("/dev").context("read /dev directory")?;
    for entry_result in entries {
        let entry = entry_result.context("read /dev entry")?;
        let file_type = entry.file_type().context("fetch /dev entry file type")?;
        if file_type.is_block_device() {
            let path = entry.path();
            let utf8 = Utf8PathBuf::from_path_buf(path)
                .map_err(|p| anyhow!("block device path is not valid UTF-8: {p:?}"))?;
            return Ok(utf8);
        }
    }
    bail!("no block device found in /dev");
}

fn create_device_fixtures() -> Result<(Utf8PathBuf, Utf8PathBuf)> {
    let block_path = find_block_device_fallback()?;
    let char_path = create_device_with_fallback(DeviceConfig {
        file_type: RxFileType::CharacterDevice,
        fallback_path: "/dev/null",
    })?;
    Ok((block_path, char_path))
}

fn create_device_with_fallback(config: DeviceConfig<'_>) -> Result<Utf8PathBuf> {
    let dev = Dir::open_ambient_dir("/dev", ambient_authority()).context("open /dev")?;
    let entries = dev.entries().context("read /dev entries")?;
    for entry_result in entries {
        let entry = entry_result.context("read /dev entry")?;
        let file_type = entry.file_type().context("fetch /dev entry file type")?;
        let matches = match config.file_type {
            RxFileType::BlockDevice => file_type.is_block_device(),
            RxFileType::CharacterDevice => file_type.is_char_device(),
            _ => false,
        };
        if matches {
            let name = entry.file_name().context("fetch /dev entry name")?;
            return Ok(Utf8PathBuf::from(format!("/dev/{name}")));
        }
    }
    let fallback = Utf8PathBuf::from(config.fallback_path);
    ensure!(
        fallback.as_std_path().exists(),
        "no {:?} found in /dev and fallback {} is missing",
        config.file_type,
        config.fallback_path,
    );
    Ok(fallback)
}

fn setup_environment_variables(
    world: &mut CliWorld,
    root: &Utf8PathBuf,
    device_paths: &(Utf8PathBuf, Utf8PathBuf),
) {
    let (block_path, char_path) = device_paths;
    let entries = [
        ("DIR_PATH", root.join("dir")),
        ("FILE_PATH", root.join("file")),
        ("SYMLINK_PATH", root.join("link")),
        ("PIPE_PATH", root.join("pipe")),
        ("BLOCK_DEVICE_PATH", block_path.clone()),
        ("CHAR_DEVICE_PATH", char_path.clone()),
        ("DEVICE_PATH", char_path.clone()),
    ];
    for (key, path) in entries {
        let previous = set_var(key, path.as_std_path().as_os_str());
        world.env_vars.entry(key.to_owned()).or_insert(previous);
    }
    let previous = set_var("WORKSPACE", root.as_std_path().as_os_str());
    world.env_vars.entry("WORKSPACE".into()).or_insert(previous);
}

fn verify_missing_fixtures(handle: &Dir, root: &Utf8PathBuf) -> Result<()> {
    handle
        .create_dir(".missing")
        .context("create missing dir")?;
    let missing_root = root.join(".missing");
    for name in ["dir", "file", "symlink", "pipe", "block", "char", "device"] {
        let path = missing_root.join(name);
        ensure!(
            !path.as_std_path().exists(),
            "missing fixture {path} unexpectedly exists",
        );
    }
    Ok(())
}

#[given("a file-type test workspace")]
fn file_type_workspace(world: &mut CliWorld) -> Result<()> {
    let (temp, root, handle) = setup_workspace()?;
    create_basic_fixtures(&handle)?;
    let device_paths = create_device_fixtures()?;
    setup_environment_variables(world, &root, &device_paths);
    verify_missing_fixtures(&handle, &root)?;
    world.temp = Some(temp);
    Ok(())
}
