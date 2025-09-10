//! Steps for preparing file-system fixtures used in Jinja tests.

use crate::CliWorld;
use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use cucumber::given;
#[cfg(unix)]
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
    #[cfg(unix)]
    {
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
    }
    #[cfg(unix)]
    let block = root.join("block");
    #[cfg(not(unix))]
    let block = Utf8PathBuf::from("NUL");
    #[cfg(unix)]
    let chardev = root.join("char");
    #[cfg(not(unix))]
    let chardev = Utf8PathBuf::from("NUL");
    #[cfg(unix)]
    let device = chardev.clone();
    #[cfg(not(unix))]
    let device = Utf8PathBuf::from("NUL");
    let dir = root.join("dir");
    let file = root.join("file");
    let link = root.join("link");
    let pipe = root.join("pipe");
    let entries = [
        ("DIR_PATH", dir),
        ("FILE_PATH", file),
        ("SYMLINK_PATH", link),
        ("PIPE_PATH", pipe),
        ("BLOCK_DEVICE_PATH", block),
        ("CHAR_DEVICE_PATH", chardev),
        ("DEVICE_PATH", device),
    ];
    for (key, path) in entries {
        let previous = set_var(key, path.as_std_path().as_os_str());
        world.env_vars.entry(key.to_string()).or_insert(previous);
    }
    world.temp = Some(temp);
}
