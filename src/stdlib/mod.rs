//! Standard library registration for `MiniJinja` templates.
//!
//! The module wires the platform-aware file tests, the path manipulation
//! filters, and the collection filters into a single entrypoint so template
//! authors can rely on consistent behaviour across projects. Tests such as
//! `dir`, `file`, and `symlink` inspect metadata without following symlinks,
//! while filters expose conveniences like `basename`, `with_suffix`,
//! `realpath`, content hashing, and collection utilities including
//! `flatten`, `group_by`, and `uniq`.

mod collections;
mod path;

use camino::Utf8Path;
use cap_std::fs;
#[cfg(unix)]
use cap_std::fs::FileTypeExt;
use minijinja::{Environment, Error, value::Value};

type FileTest = (&'static str, fn(fs::FileType) -> bool);

/// Register standard library helpers with the `MiniJinja` environment.
///
/// # Examples
/// ```
/// use minijinja::{context, Environment};
/// use netsuke::stdlib;
///
/// let mut env = Environment::new();
/// stdlib::register(&mut env);
/// env.add_template("t", "{{ path | basename }}").expect("add template");
/// let tmpl = env.get_template("t").expect("get template");
/// let rendered = tmpl
///     .render(context!(path => "foo/bar.txt"))
///     .expect("render");
/// assert_eq!(rendered, "bar.txt");
/// ```
pub fn register(env: &mut Environment<'_>) {
    register_file_tests(env);
    path::register_filters(env);
    collections::register_filters(env);
}

fn register_file_tests(env: &mut Environment<'_>) {
    const TESTS: &[FileTest] = &[
        ("dir", is_dir),
        ("file", is_file),
        ("symlink", is_symlink),
        ("pipe", is_fifo),
        ("block_device", is_block_device),
        ("char_device", is_char_device),
        ("device", is_device),
    ];

    for &(name, pred) in TESTS {
        env.add_test(name, move |val: Value| -> Result<bool, Error> {
            if let Some(s) = val.as_str() {
                return path::file_type_matches(Utf8Path::new(s), pred);
            }
            Ok(false)
        });
    }
}

fn is_dir(ft: fs::FileType) -> bool {
    ft.is_dir()
}

fn is_file(ft: fs::FileType) -> bool {
    ft.is_file()
}

fn is_symlink(ft: fs::FileType) -> bool {
    ft.is_symlink()
}

#[cfg(unix)]
fn is_fifo(ft: fs::FileType) -> bool {
    ft.is_fifo()
}

#[cfg(not(unix))]
fn is_fifo(_ft: fs::FileType) -> bool {
    false
}

#[cfg(unix)]
fn is_block_device(ft: fs::FileType) -> bool {
    ft.is_block_device()
}

#[cfg(not(unix))]
fn is_block_device(_ft: fs::FileType) -> bool {
    false
}

#[cfg(unix)]
fn is_char_device(ft: fs::FileType) -> bool {
    ft.is_char_device()
}

#[cfg(not(unix))]
fn is_char_device(_ft: fs::FileType) -> bool {
    false
}

#[cfg(unix)]
fn is_device(ft: fs::FileType) -> bool {
    is_block_device(ft) || is_char_device(ft)
}

#[cfg(not(unix))]
fn is_device(_ft: fs::FileType) -> bool {
    false
}
