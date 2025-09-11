//! Steps for preparing file-system fixtures used in Jinja tests.

use crate::CliWorld;
use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use cucumber::given;
use rustix::fs::{Dev, FileType, Mode, mknodat};
use rustix::io::Errno;
use test_support::env::set_var;

#[given("a file-type test workspace")]
fn file_type_workspace(world: &mut CliWorld) {
    let temp = tempfile::tempdir().expect("create tempdir for file-type workspace");
    let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).expect("utf8");
    let handle = Dir::open_ambient_dir(&root, ambient_authority())
        .expect("open ambient dir for file-type workspace");
    handle.create_dir("dir").expect("create dir fixture");
    handle.write("file", b"data").expect("write file fixture");
    handle
        .symlink("file", "link")
        .expect("create symlink fixture");
    // FIFO creation is unprivileged and should succeed.
    mknodat(
        &handle,
        "pipe",
        FileType::Fifo,
        Mode::RUSR | Mode::WUSR,
        Dev::default(),
    )
    .expect("create fifo fixture");
    // Creating device nodes is privileged; fall back to /dev where needed.
    let block_path = match mknodat(
        &handle,
        "block",
        FileType::BlockDevice,
        Mode::RUSR | Mode::WUSR,
        Dev::default(),
    ) {
        Ok(()) => root.join("block"),
        Err(e) if e == Errno::PERM || e == Errno::ACCESS => Utf8PathBuf::from("/dev/loop0"),
        Err(e) => panic!("create block device fixture: {e}"),
    };
    let char_path = match mknodat(
        &handle,
        "char",
        FileType::CharacterDevice,
        Mode::RUSR | Mode::WUSR,
        Dev::default(),
    ) {
        Ok(()) => root.join("char"),
        Err(e) if e == Errno::PERM || e == Errno::ACCESS => Utf8PathBuf::from("/dev/null"),
        Err(e) => panic!("create char device fixture: {e}"),
    };
    let entries = [
        ("DIR_PATH", root.join("dir")),
        ("FILE_PATH", root.join("file")),
        ("SYMLINK_PATH", root.join("link")),
        ("PIPE_PATH", root.join("pipe")),
        ("BLOCK_DEVICE_PATH", block_path),
        ("CHAR_DEVICE_PATH", char_path.clone()),
        ("DEVICE_PATH", char_path),
    ];
    for (key, path) in entries {
        let previous = set_var(key, path.as_std_path().as_os_str());
        world.env_vars.entry(key.to_string()).or_insert(previous);
    }
    world.temp = Some(temp);
}

#[given(expr = "the environment variable {string} is set to a missing path")]
fn set_missing_env_path(world: &mut CliWorld, key: String) {
    let temp = world.temp.as_ref().expect("file-type workspace tempdir");
    let path = Utf8PathBuf::from_path_buf(temp.path().join("__missing__").join(key.to_lowercase()))
        .expect("utf8 missing path");
    let previous = set_var(&key, path.as_std_path().as_os_str());
    world.env_vars.entry(key).or_insert(previous);
}
