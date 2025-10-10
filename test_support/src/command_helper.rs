use std::{ffi::OsString, process::Command};

use camino::Utf8PathBuf;
use cap_std::fs_utf8::Dir;

const UPPERCASE_SOURCE: &str = concat!(
    "use std::io::{self, Read};\n",
    "fn main() {\n",
    "    let mut input = String::new();\n",
    "    io::stdin().read_to_string(&mut input).expect(\"stdin\");\n",
    "    print!(\"{}\", input.to_uppercase());\n",
    "}\n",
);

pub fn compile_uppercase_helper(dir: &Dir, root: &Utf8PathBuf, name: &str) -> Utf8PathBuf {
    dir.write(&format!("{name}.rs"), UPPERCASE_SOURCE.as_bytes())
        .expect("write helper source");

    let src_path = root.join(format!("{name}.rs"));
    let exe_path = root.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| OsString::from("rustc"));
    let status = Command::new(&rustc)
        .arg(src_path.as_std_path())
        .arg("-o")
        .arg(exe_path.as_std_path())
        .status()
        .expect("compile helper");

    assert!(status.success(), "failed to compile helper: {status:?}");
    exe_path
}
