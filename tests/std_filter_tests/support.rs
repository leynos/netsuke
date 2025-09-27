use std::cell::RefCell;

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use minijinja::{Environment, context};
use netsuke::stdlib;
use rstest::fixture;
use tempfile::tempdir;

pub(crate) use test_support::{EnvVarGuard, env_lock::EnvLock};

pub(crate) type Workspace = (tempfile::TempDir, Utf8PathBuf);

thread_local! {
    static TEMPLATE_STORAGE: RefCell<Vec<(Box<str>, Box<str>)>> = const { RefCell::new(Vec::new()) };
}

pub(crate) fn register_template(
    env: &mut Environment<'_>,
    name: impl Into<String>,
    source: impl Into<String>,
) {
    TEMPLATE_STORAGE.with(|storage| {
        let (name_ptr, source_ptr) = {
            let mut storage = storage.borrow_mut();
            storage.push((name.into().into_boxed_str(), source.into().into_boxed_str()));
            let (name, source) = storage.last().expect("template storage entry");
            (
                std::ptr::from_ref(name.as_ref()),
                std::ptr::from_ref(source.as_ref()),
            )
        };
        // SAFETY: the pointers originate from boxed strings stored in the
        // thread-local registry. They remain valid for the duration of the
        // process, so treating them as `'static` references is sound.
        unsafe {
            env.add_template(&*name_ptr, &*source_ptr)
                .expect("template");
        }
    });
}

pub(crate) fn stdlib_env() -> Environment<'static> {
    let mut env = Environment::new();
    stdlib::register(&mut env);
    env
}

#[fixture]
pub(crate) fn filter_workspace() -> Workspace {
    let temp = tempdir().expect("tempdir");
    let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).expect("utf8");
    let dir = Dir::open_ambient_dir(&root, ambient_authority()).expect("dir");
    dir.write("file", b"data").expect("file");
    #[cfg(unix)]
    dir.symlink("file", "link").expect("symlink");
    #[cfg(not(unix))]
    dir.write("link", b"data").expect("link copy");
    dir.write("lines.txt", b"one\ntwo\nthree\n").expect("lines");
    (temp, root)
}

pub(crate) struct HomeEnvGuard {
    _lock: EnvLock,
    _home: EnvVarGuard,
    _profile: EnvVarGuard,
    _drive: EnvVarGuard,
    _path: EnvVarGuard,
    _share: EnvVarGuard,
}

impl HomeEnvGuard {
    fn new(home: Option<&str>) -> Self {
        let lock = EnvLock::acquire();
        let home_guard = home.map_or_else(
            || EnvVarGuard::remove("HOME"),
            |value| EnvVarGuard::set("HOME", value),
        );
        let profile_guard = EnvVarGuard::remove("USERPROFILE");
        let drive_guard = EnvVarGuard::remove("HOMEDRIVE");
        let path_guard = EnvVarGuard::remove("HOMEPATH");
        let share_guard = EnvVarGuard::remove("HOMESHARE");
        Self {
            _lock: lock,
            _home: home_guard,
            _profile: profile_guard,
            _drive: drive_guard,
            _path: path_guard,
            _share: share_guard,
        }
    }

    pub(crate) fn home_only(root: &Utf8Path) -> Self {
        Self::new(Some(root.as_str()))
    }

    pub(crate) fn unset() -> Self {
        Self::new(None)
    }
}

pub(crate) fn render<'a>(
    env: &mut Environment<'a>,
    name: &'a str,
    template: &'a str,
    path: &Utf8PathBuf,
) -> String {
    env.add_template(name, template).expect("template");
    env.get_template(name)
        .expect("get template")
        .render(context!(path => path.as_str()))
        .expect("render")
}
