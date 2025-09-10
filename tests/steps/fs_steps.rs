//! Steps for preparing file-system fixtures used in Jinja tests.

use crate::CliWorld;
use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use cucumber::given;
use rustix::fs::{Dev, FileType, Mode, mknodat};
use test_support::env::set_var;

#[given("a file-type test workspace")]
fn file_type_workspace(world: &mut CliWorld) {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).expect("utf8");
    let handle = Dir::open_ambient_dir(&root, ambient_authority()).expect("ambient");
    handle.create_dir("dir").expect("dir");
    handle.write("file", b"data").expect("file");
    handle.symlink("file", "link").expect("symlink");
    mknodat(
        &handle,
        "pipe",
        FileType::Fifo,
        Mode::RUSR | Mode::WUSR,
        Dev::default(),
    )
    .expect("fifo");
    mknodat(
        &handle,
        "block",
        FileType::BlockDevice,
        Mode::RUSR | Mode::WUSR,
        Dev::default(),
    )
    .expect("block");
    mknodat(
        &handle,
        "char",
        FileType::CharacterDevice,
        Mode::RUSR | Mode::WUSR,
        Dev::default(),
    )
    .expect("char");
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
