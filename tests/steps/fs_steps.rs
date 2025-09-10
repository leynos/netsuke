//! Steps for preparing file-system fixtures used in Jinja tests.

use crate::CliWorld;
use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use cucumber::given;
use rustix::fs::{Dev, FileType, Mode, mknodat};
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
    mknodat(
        &handle,
        "pipe",
        FileType::Fifo,
        Mode::RUSR | Mode::WUSR,
        Dev::default(),
    )
    .expect("create fifo fixture");
    mknodat(
        &handle,
        "block",
        FileType::BlockDevice,
        Mode::RUSR | Mode::WUSR,
        Dev::default(),
    )
    .expect("create block device fixture");
    mknodat(
        &handle,
        "char",
        FileType::CharacterDevice,
        Mode::RUSR | Mode::WUSR,
        Dev::default(),
    )
    .expect("create char device fixture");
    let entries = [
        ("DIR_PATH", root.join("dir")),
        ("FILE_PATH", root.join("file")),
        ("SYMLINK_PATH", root.join("link")),
        ("PIPE_PATH", root.join("pipe")),
        ("BLOCK_DEVICE_PATH", root.join("block")),
        ("CHAR_DEVICE_PATH", root.join("char")),
        ("DEVICE_PATH", root.join("char")),
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
