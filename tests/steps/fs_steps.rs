//! Steps for preparing file-system fixtures used in Jinja tests.

use crate::CliWorld;
use cucumber::given;
use nix::{sys::stat::Mode, unistd::mkfifo};
use std::{os::unix::fs as unix_fs, path::PathBuf};
use test_support::env::set_var;

#[given("a file-type test workspace")]
fn file_type_workspace(world: &mut CliWorld) {
    let temp = tempfile::tempdir().expect("tempdir");
    let dir = temp.path().join("dir");
    std::fs::create_dir(&dir).expect("dir");
    let file = temp.path().join("file");
    std::fs::write(&file, b"data").expect("file");
    let link = temp.path().join("link");
    unix_fs::symlink(&file, &link).expect("symlink");
    let pipe = temp.path().join("pipe");
    mkfifo(&pipe, Mode::S_IRUSR | Mode::S_IWUSR).expect("fifo");
    let entries = [
        ("DIR_PATH", dir),
        ("FILE_PATH", file),
        ("SYMLINK_PATH", link),
        ("PIPE_PATH", pipe),
        ("DEVICE_PATH", PathBuf::from("/dev/null")),
    ];
    for (key, path) in entries {
        let previous = set_var(key, path.as_os_str());
        world.env_vars.entry(key.to_string()).or_insert(previous);
    }
    world.temp = Some(temp);
}
