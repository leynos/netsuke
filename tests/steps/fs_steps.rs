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
    // FIFO creation is unprivileged and should succeed.
    mknodat(
        &handle,
        "pipe",
        FileType::Fifo,
        Mode::RUSR | Mode::WUSR,
        Dev::default(),
    )
    .expect("create fifo fixture");
    // Use existing device nodes to avoid requiring privileges.
    let block_path = Utf8PathBuf::from("/dev/loop0");
    let char_path = Utf8PathBuf::from("/dev/null");
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
        world.env_vars.entry(key.to_string()).or_insert(previous); // restored after scenario
    }
    let previous = set_var("WORKSPACE", root.as_std_path().as_os_str());
    world.env_vars.entry("WORKSPACE".into()).or_insert(previous);
    handle.create_dir(".missing").expect("create missing dir");
    let missing_root = root.join(".missing");
    for name in ["dir", "file", "symlink", "pipe", "block", "char", "device"] {
        let path = missing_root.join(name);
        assert!(
            !path.as_std_path().exists(),
            "missing fixture {path} unexpectedly exists"
        );
    }
    world.temp = Some(temp);
}
