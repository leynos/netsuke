//! Steps for preparing file-system fixtures used in Jinja tests.

use crate::CliWorld;
use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use cucumber::given;
use rustix::fs::{Dev, FileType, Mode, mknodat};
use rustix::io::Errno;
use test_support::env::set_var;

fn setup_workspace() -> (tempfile::TempDir, Utf8PathBuf, Dir) {
    let temp = tempfile::tempdir().expect("create tempdir for file-type workspace");
    let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).expect("utf8");
    let handle = Dir::open_ambient_dir(&root, ambient_authority())
        .expect("open ambient dir for file-type workspace");
    (temp, root, handle)
}

fn create_basic_fixtures(handle: &Dir) {
    handle.create_dir("dir").expect("create dir fixture");
    handle.write("file", b"data").expect("write file fixture");
    handle
        .symlink("file", "link")
        .expect("create symlink fixture");
    // FIFO creation is unprivileged and should succeed.
    mknodat(
        handle,
        "pipe",
        FileType::Fifo,
        Mode::RUSR | Mode::WUSR,
        Dev::default(),
    )
    .expect("create fifo fixture");
}

/// Configuration for a device node fixture and its fallback path.
#[derive(Copy, Clone)]
struct DeviceConfig<'a> {
    name: &'a str,
    file_type: FileType,
    fallback_path: &'a str,
}

fn create_device_fixtures(handle: &Dir, root: &Utf8PathBuf) -> (Utf8PathBuf, Utf8PathBuf) {
    let block_path = create_device_with_fallback(
        handle,
        root,
        DeviceConfig {
            name: "block",
            file_type: FileType::BlockDevice,
            fallback_path: "/dev/loop0",
        },
    );
    let char_path = create_device_with_fallback(
        handle,
        root,
        DeviceConfig {
            name: "char",
            file_type: FileType::CharacterDevice,
            fallback_path: "/dev/null",
        },
    );
    (block_path, char_path)
}

fn create_device_with_fallback(
    handle: &Dir,
    root: &Utf8PathBuf,
    config: DeviceConfig<'_>,
) -> Utf8PathBuf {
    match mknodat(
        handle,
        config.name,
        config.file_type,
        Mode::RUSR | Mode::WUSR,
        Dev::default(),
    ) {
        Ok(()) => root.join(config.name),
        Err(e) if e == Errno::PERM || e == Errno::ACCESS => Utf8PathBuf::from(config.fallback_path),
        Err(e) => panic!("create {:?} fixture: {e}", config.file_type),
    }
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
        world.env_vars.entry(key.to_string()).or_insert(previous);
    }
    let previous = set_var("WORKSPACE", root.as_std_path().as_os_str());
    world.env_vars.entry("WORKSPACE".into()).or_insert(previous);
}

fn verify_missing_fixtures(handle: &Dir, root: &Utf8PathBuf) {
    handle.create_dir(".missing").expect("create missing dir");
    let missing_root = root.join(".missing");
    for name in ["dir", "file", "symlink", "pipe", "block", "char", "device"] {
        let path = missing_root.join(name);
        assert!(
            !path.as_std_path().exists(),
            "missing fixture {path} unexpectedly exists",
        );
    }
}

#[given("a file-type test workspace")]
fn file_type_workspace(world: &mut CliWorld) {
    let (temp, root, handle) = setup_workspace();
    create_basic_fixtures(&handle);
    let device_paths = create_device_fixtures(&handle, &root);
    setup_environment_variables(world, &root, &device_paths);
    verify_missing_fixtures(&handle, &root);
    world.temp = Some(temp);
}
