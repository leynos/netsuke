//! Cross-platform `which` filter and helper function for `MiniJinja`.
//!
//! Resolves executables deterministically across Unix and Windows,
//! honouring user-configurable options for returning every match, emitting
//! canonical paths, bypassing the resolver cache, and opt-in search of the
//! current working directory.

use std::{
    ffi::OsString,
    fmt, fs,
    num::NonZeroUsize,
    sync::{Arc, Mutex},
};

#[cfg(windows)]
use std::ffi::OsStr;

use camino::{Utf8Path, Utf8PathBuf};
use indexmap::IndexSet;
use lru::LruCache;
use minijinja::{
    Environment, Error, ErrorKind,
    value::{Kwargs, Value},
};

const CACHE_CAPACITY: usize = 64;

pub(crate) fn register(env: &mut Environment<'_>) {
    let resolver = Arc::new(WhichResolver::new());
    {
        let filter_resolver = Arc::clone(&resolver);
        env.add_filter("which", move |value: Value, kwargs: Kwargs| {
            resolve_with(&filter_resolver, &value, &kwargs).and_then(|output| {
                kwargs.assert_all_used()?;
                Ok(output)
            })
        });
    }
    {
        let function_resolver = Arc::clone(&resolver);
        env.add_function("which", move |value: Value, kwargs: Kwargs| {
            resolve_with(&function_resolver, &value, &kwargs).and_then(|output| {
                kwargs.assert_all_used()?;
                Ok(output)
            })
        });
    }
}

fn resolve_with(
    resolver: &WhichResolver,
    command: &Value,
    kwargs: &Kwargs,
) -> Result<Value, Error> {
    let name = command
        .as_str()
        .map(str::trim)
        .filter(|candidate| !candidate.is_empty())
        .ok_or_else(|| args_error("which requires a non-empty string"))?;
    let options = WhichOptions::from_kwargs(kwargs)?;
    let matches = resolver.resolve(name, &options)?;
    Ok(render_value(&matches, &options))
}

fn render_value(matches: &[Utf8PathBuf], options: &WhichOptions) -> Value {
    if options.all {
        let rendered: Vec<String> = matches
            .iter()
            .map(|path| format_path_for_output(path))
            .collect();
        Value::from_serialize(rendered)
    } else {
        let first = matches
            .first()
            .map_or_else(String::new, |path| format_path_for_output(path));
        Value::from(first)
    }
}

fn format_path_for_output(path: &Utf8Path) -> String {
    #[cfg(windows)]
    {
        path.as_str().replace('\\', "/")
    }
    #[cfg(not(windows))]
    {
        path.as_str().to_owned()
    }
}

#[derive(Clone, Debug)]
struct WhichResolver {
    cache: Arc<Mutex<LruCache<CacheKey, CacheEntry>>>,
}

impl WhichResolver {
    fn new() -> Self {
        #[expect(
            clippy::unwrap_used,
            reason = "cache capacity constant is greater than zero"
        )]
        let capacity = NonZeroUsize::new(CACHE_CAPACITY).unwrap();
        Self {
            cache: Arc::new(Mutex::new(LruCache::new(capacity))),
        }
    }

    fn resolve(&self, command: &str, options: &WhichOptions) -> Result<Vec<Utf8PathBuf>, Error> {
        let env = EnvSnapshot::capture()?;
        let key = CacheKey::new(command, &env, options);
        if !options.fresh
            && let Some(cached) = self.try_cache(&key)
        {
            return Ok(cached);
        }
        let matches = lookup(command, &env, options)?;
        self.store(key, matches.clone());
        Ok(matches)
    }

    fn try_cache(&self, key: &CacheKey) -> Option<Vec<Utf8PathBuf>> {
        let mut guard = self.lock_cache();
        match guard.get(key) {
            Some(entry) if entry.is_valid() => Some(entry.matches.clone()),
            Some(_) => {
                guard.pop(key);
                None
            }
            None => None,
        }
    }

    fn store(&self, key: CacheKey, matches: Vec<Utf8PathBuf>) {
        let mut guard = self.lock_cache();
        guard.put(key, CacheEntry { matches });
    }

    fn lock_cache(&self) -> std::sync::MutexGuard<'_, LruCache<CacheKey, CacheEntry>> {
        match self.cache.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

fn push_matches(
    matches: &mut Vec<Utf8PathBuf>,
    candidates: Vec<Utf8PathBuf>,
    collect_all: bool,
) -> bool {
    for candidate in candidates {
        if !is_executable(&candidate) {
            continue;
        }
        matches.push(candidate);
        if !collect_all {
            return true;
        }
    }
    false
}

fn lookup(
    command: &str,
    env: &EnvSnapshot,
    options: &WhichOptions,
) -> Result<Vec<Utf8PathBuf>, Error> {
    if is_direct_path(command) {
        return resolve_direct(command, env, options);
    }

    let dirs = env.resolved_dirs(options.cwd_mode);
    let mut matches = Vec::new();

    #[cfg(windows)]
    let suffixes = env.pathext();

    for dir in &dirs {
        #[cfg(windows)]
        let candidates = candidate_paths(dir, command, suffixes);
        #[cfg(not(windows))]
        let candidates = vec![dir.join(command)];

        if push_matches(&mut matches, candidates, options.all) {
            break;
        }
    }

    if matches.is_empty() {
        return Err(not_found_error(command, &dirs, options.cwd_mode));
    }

    if options.canonical {
        canonicalise(matches)
    } else {
        Ok(matches)
    }
}

#[derive(Clone, Debug)]
struct CacheEntry {
    matches: Vec<Utf8PathBuf>,
}

impl CacheEntry {
    fn is_valid(&self) -> bool {
        self.matches
            .iter()
            .all(|path| is_executable(path.as_path()))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct CacheKey {
    command: String,
    path: Option<OsString>,
    pathext: Option<OsString>,
    cwd: Utf8PathBuf,
    options: CacheKeyOptions,
}

impl CacheKey {
    fn new(command: &str, env: &EnvSnapshot, options: &WhichOptions) -> Self {
        Self {
            command: command.to_owned(),
            path: env.raw_path.clone(),
            pathext: env.raw_pathext.clone(),
            cwd: env.cwd.clone(),
            options: CacheKeyOptions::from(options),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct CacheKeyOptions {
    all: bool,
    canonical: bool,
    cwd_mode: CwdMode,
}

impl From<&WhichOptions> for CacheKeyOptions {
    fn from(value: &WhichOptions) -> Self {
        Self {
            all: value.all,
            canonical: value.canonical,
            cwd_mode: value.cwd_mode,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum CwdMode {
    Auto,
    Always,
    Never,
}

impl CwdMode {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "auto" => Some(Self::Auto),
            "always" => Some(Self::Always),
            "never" => Some(Self::Never),
            _ => None,
        }
    }
}

impl Default for CwdMode {
    fn default() -> Self {
        Self::Auto
    }
}

#[derive(Clone, Debug, Default)]
struct WhichOptions {
    all: bool,
    canonical: bool,
    fresh: bool,
    cwd_mode: CwdMode,
}

impl WhichOptions {
    fn from_kwargs(kwargs: &Kwargs) -> Result<Self, Error> {
        let all = kwargs.get::<Option<bool>>("all")?.unwrap_or(false);
        let canonical = kwargs.get::<Option<bool>>("canonical")?.unwrap_or(false);
        let fresh = kwargs.get::<Option<bool>>("fresh")?.unwrap_or(false);
        let cwd_mode = kwargs
            .get::<Option<String>>("cwd_mode")?
            .map(|mode| {
                let lower = mode.to_ascii_lowercase();
                CwdMode::parse(&lower).ok_or_else(|| {
                    args_error(format!(
                        "cwd_mode must be 'auto', 'always', or 'never', got '{mode}'"
                    ))
                })
            })
            .transpose()?;
        Ok(Self {
            all,
            canonical,
            fresh,
            cwd_mode: cwd_mode.unwrap_or_default(),
        })
    }
}

struct EnvSnapshot {
    cwd: Utf8PathBuf,
    raw_path: Option<OsString>,
    raw_pathext: Option<OsString>,
    entries: Vec<PathEntry>,
    #[cfg(windows)]
    pathext: Vec<String>,
}

impl EnvSnapshot {
    fn capture() -> Result<Self, Error> {
        let cwd = current_dir_utf8()?;
        let raw_path = std::env::var_os("PATH");
        let entries = parse_path_entries(raw_path.clone(), &cwd)?;
        #[cfg(windows)]
        let raw_pathext = std::env::var_os("PATHEXT");
        #[cfg(windows)]
        let pathext = parse_pathext(raw_pathext.as_deref());
        #[cfg(not(windows))]
        let raw_pathext = None;
        Ok(Self {
            cwd,
            raw_path,
            raw_pathext,
            entries,
            #[cfg(windows)]
            pathext,
        })
    }

    fn resolved_dirs(&self, mode: CwdMode) -> Vec<Utf8PathBuf> {
        let mut dirs = Vec::new();
        if matches!(mode, CwdMode::Always) {
            dirs.push(self.cwd.clone());
        }
        for entry in &self.entries {
            match entry {
                PathEntry::Dir(path) => dirs.push(path.clone()),
                PathEntry::CurrentDir if matches!(mode, CwdMode::Always | CwdMode::Auto) => {
                    dirs.push(self.cwd.clone());
                }
                PathEntry::CurrentDir => {}
            }
        }
        dirs
    }

    #[cfg(windows)]
    fn pathext(&self) -> &[String] {
        &self.pathext
    }
}

#[derive(Clone, Debug)]
enum PathEntry {
    Dir(Utf8PathBuf),
    CurrentDir,
}

fn parse_path_entries(raw: Option<OsString>, cwd: &Utf8Path) -> Result<Vec<PathEntry>, Error> {
    let mut entries = Vec::new();
    let Some(raw_value) = raw else {
        return Ok(entries);
    };
    for (index, component) in std::env::split_paths(&raw_value).enumerate() {
        if component.as_os_str().is_empty() {
            entries.push(PathEntry::CurrentDir);
            continue;
        }
        let utf8 = Utf8PathBuf::from_path_buf(component).map_err(|_| {
            args_error(format!(
                "PATH entry #{index} contains non-UTF-8 characters; Netsuke requires UTF-8 paths"
            ))
        })?;
        let resolved = if utf8.is_absolute() {
            utf8
        } else {
            cwd.join(utf8)
        };
        entries.push(PathEntry::Dir(resolved));
    }
    Ok(entries)
}

#[cfg(windows)]
const DEFAULT_PATHEXT: &[&str] = &[
    ".com", ".exe", ".bat", ".cmd", ".vbs", ".vbe", ".js", ".jse", ".wsf", ".wsh", ".msc",
];

#[cfg(windows)]
fn parse_pathext(raw: Option<&OsStr>) -> Vec<String> {
    let mut dedup = IndexSet::new();
    let source = raw
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| DEFAULT_PATHEXT.join(";"));
    for segment in source.split(';') {
        let trimmed = segment.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut normalised = trimmed.to_ascii_lowercase();
        if !normalised.starts_with('.') {
            normalised.insert(0, '.');
        }
        dedup.insert(normalised);
    }
    if dedup.is_empty() {
        DEFAULT_PATHEXT.iter().map(|ext| ext.to_string()).collect()
    } else {
        dedup.into_iter().collect()
    }
}

fn current_dir_utf8() -> Result<Utf8PathBuf, Error> {
    let cwd = std::env::current_dir().map_err(|err| {
        Error::new(
            ErrorKind::InvalidOperation,
            format!("failed to resolve current directory: {err}"),
        )
    })?;
    Utf8PathBuf::from_path_buf(cwd).map_err(|_| {
        Error::new(
            ErrorKind::InvalidOperation,
            "current directory contains non-UTF-8 components",
        )
    })
}

fn is_direct_path(command: &str) -> bool {
    #[cfg(windows)]
    {
        command.contains(['\\', '/', ':'])
    }
    #[cfg(not(windows))]
    {
        command.contains('/')
    }
}

fn resolve_direct(
    command: &str,
    env: &EnvSnapshot,
    options: &WhichOptions,
) -> Result<Vec<Utf8PathBuf>, Error> {
    let raw = Utf8Path::new(command);
    let resolved = if raw.is_absolute() {
        raw.to_path_buf()
    } else {
        env.cwd.join(raw)
    };
    if !is_executable(&resolved) {
        return Err(direct_not_found(command, &resolved));
    }
    let output = if options.canonical {
        canonicalise(vec![resolved])?
    } else {
        vec![resolved]
    };
    Ok(output)
}

fn direct_not_found(command: &str, path: &Utf8Path) -> Error {
    Error::new(
        ErrorKind::InvalidOperation,
        format!(
            "[netsuke::jinja::which::not_found] command '{command}' at '{path}' is missing or not executable"
        ),
    )
}

fn is_executable(path: &Utf8Path) -> bool {
    fs::metadata(path.as_std_path())
        .is_ok_and(|metadata| metadata.is_file() && has_execute_permission(&metadata))
}

#[cfg(unix)]
fn has_execute_permission(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn has_execute_permission(metadata: &fs::Metadata) -> bool {
    metadata.is_file()
}

fn canonicalise(paths: Vec<Utf8PathBuf>) -> Result<Vec<Utf8PathBuf>, Error> {
    let mut unique = IndexSet::new();
    let mut resolved = Vec::new();
    for path in paths {
        let canonical = fs::canonicalize(path.as_std_path()).map_err(|err| {
            Error::new(
                ErrorKind::InvalidOperation,
                format!("failed to canonicalise '{path}': {err}"),
            )
        })?;
        let utf8 = Utf8PathBuf::from_path_buf(canonical).map_err(|_| {
            Error::new(
                ErrorKind::InvalidOperation,
                "canonical path contains non-UTF-8 characters",
            )
        })?;
        if unique.insert(utf8.clone()) {
            resolved.push(utf8);
        }
    }
    Ok(resolved)
}

fn not_found_error(command: &str, dirs: &[Utf8PathBuf], mode: CwdMode) -> Error {
    let count = dirs.len();
    let preview = path_preview(dirs);
    let mut message = format!(
        "[netsuke::jinja::which::not_found] command '{command}' not found after checking {count} PATH entries. Preview: {preview}"
    );
    if let Some(hint) = hint_for_mode(mode) {
        message.push_str(". ");
        message.push_str(hint);
    }
    Error::new(ErrorKind::InvalidOperation, message)
}

fn path_preview(dirs: &[Utf8PathBuf]) -> String {
    const LIMIT: usize = 4;
    if dirs.is_empty() {
        return "<empty>".to_owned();
    }
    let mut parts: Vec<_> = dirs
        .iter()
        .take(LIMIT)
        .map(|dir| format_path_for_output(dir))
        .collect();
    if dirs.len() > LIMIT {
        parts.push("…".into());
    }
    parts.join(", ")
}

const fn hint_for_mode(mode: CwdMode) -> Option<&'static str> {
    #[cfg(windows)]
    {
        match mode {
            CwdMode::Always => None,
            _ => Some("Set cwd_mode=\"always\" to include the current directory."),
        }
    }
    #[cfg(not(windows))]
    {
        match mode {
            CwdMode::Never => Some(
                "Empty PATH segments are ignored; use cwd_mode=\"auto\" to include the working directory.",
            ),
            _ => None,
        }
    }
}

fn args_error(message: impl fmt::Display) -> Error {
    Error::new(
        ErrorKind::InvalidOperation,
        format!("[netsuke::jinja::which::args] {message}"),
    )
}

#[cfg(windows)]
fn candidate_paths(dir: &Utf8Path, command: &str, pathext: &[String]) -> Vec<Utf8PathBuf> {
    let mut paths = Vec::new();
    let base = dir.join(command);
    if Utf8Path::new(command).extension().is_some() {
        paths.push(base);
        return paths;
    }
    for ext in pathext {
        let mut candidate = base.as_str().to_owned();
        candidate.push_str(ext);
        paths.push(Utf8PathBuf::from(candidate));
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{Context, Result, anyhow, bail, ensure};
    use rstest::rstest;
    use tempfile::TempDir;
    use test_support::{
        env::{VarGuard, set_var},
        env_lock::EnvLock,
    };

    #[cfg(windows)]
    use std::ffi::OsStr;

    struct EnvFixture {
        _lock: EnvLock,
        _temp: TempDir,
        dirs: Vec<Utf8PathBuf>,
        _path_guard: VarGuard,
        #[cfg(windows)]
        pathext_guard: VarGuard,
    }

    impl EnvFixture {
        fn new(dir_names: &[&str]) -> Result<Self> {
            let lock = EnvLock::acquire();
            let temp = TempDir::new().context("create which test workspace")?;
            let mut dirs = Vec::new();
            for name in dir_names {
                let dir = temp.path().join(name);
                std::fs::create_dir_all(&dir)
                    .with_context(|| format!("create fixture dir {name}"))?;
                let utf = Utf8PathBuf::from_path_buf(dir)
                    .map_err(|_| anyhow!("fixture path for {name} is not UTF-8"))?;
                dirs.push(utf);
            }
            let joined = std::env::join_paths(dirs.iter().map(|dir| dir.as_std_path()))
                .context("join PATH entries")?;
            let path_guard = VarGuard::set("PATH", joined.as_os_str());
            #[cfg(windows)]
            let pathext_guard = VarGuard::set("PATHEXT", OsStr::new(".cmd;.exe"));
            Ok(Self {
                _lock: lock,
                _temp: temp,
                dirs,
                _path_guard: path_guard,
                #[cfg(windows)]
                pathext_guard,
            })
        }

        fn dir(&self, index: usize) -> &Utf8Path {
            self.dirs.get(index).map_or_else(
                || panic!("dir index {index} is out of bounds"),
                Utf8PathBuf::as_path,
            )
        }

        fn write_tool(&self, dir_index: usize, name: &str) -> Result<Utf8PathBuf> {
            let mut path = self
                .dirs
                .get(dir_index)
                .cloned()
                .ok_or_else(|| anyhow!("dir index {dir_index} is out of bounds"))?;
            let filename = tool_name(name);
            path.push(Utf8Path::new(&filename));
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent.as_std_path())
                    .with_context(|| format!("ensure parent for {path:?}"))?;
            }
            std::fs::write(path.as_std_path(), script_contents())
                .with_context(|| format!("write tool {path:?}"))?;
            mark_executable(&path)?;
            Ok(path)
        }

        fn remove_path(path: &Utf8Path) -> Result<()> {
            std::fs::remove_file(path.as_std_path()).with_context(|| format!("remove {path:?}"))
        }
    }

    #[cfg(windows)]
    fn tool_name(base: &str) -> String {
        format!("{base}.cmd")
    }

    #[cfg(not(windows))]
    fn tool_name(base: &str) -> String {
        base.to_owned()
    }

    fn script_contents() -> &'static [u8] {
        #[cfg(windows)]
        {
            b"@echo off\r\n"
        }
        #[cfg(not(windows))]
        {
            b"#!/bin/sh\nexit 0\n"
        }
    }

    #[cfg(unix)]
    fn mark_executable(path: &Utf8Path) -> Result<()> {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(path.as_std_path())
            .with_context(|| format!("stat {path:?}"))?
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(path.as_std_path(), perms)
            .with_context(|| format!("set perms {path:?}"))
    }

    #[cfg(not(unix))]
    fn mark_executable(_path: &Utf8Path) -> Result<()> {
        Ok(())
    }

    fn resolver() -> WhichResolver {
        WhichResolver::new()
    }

    fn render(
        resolver: &WhichResolver,
        name: &str,
        options: &WhichOptions,
    ) -> Result<Vec<Utf8PathBuf>> {
        resolver
            .resolve(name, options)
            .map_err(|err| anyhow!(err.to_string()))
    }

    #[rstest]
    fn resolves_first_path_entry() -> Result<()> {
        let fixture = EnvFixture::new(&["bin_a", "bin_b"])?;
        fixture.write_tool(1, "tool")?;
        let resolver = resolver();
        let output = render(&resolver, "tool", &WhichOptions::default())?;
        let expected = fixture.dir(1).join(Utf8Path::new(&tool_name("tool")));
        ensure!(
            output.first() == Some(&expected),
            "expected first match {expected:?} but got {output:?}"
        );
        Ok(())
    }

    #[rstest]
    fn returns_all_matches_in_path_order() -> Result<()> {
        let fixture = EnvFixture::new(&["first", "second"])?;
        fixture.write_tool(0, "tool")?;
        fixture.write_tool(1, "tool")?;
        let options = WhichOptions {
            all: true,
            ..WhichOptions::default()
        };
        let resolver = resolver();
        let output = render(&resolver, "tool", &options)?;
        ensure!(output.len() == 2, "expected two matches but got {output:?}");
        let first = fixture.dir(0).join(Utf8Path::new(&tool_name("tool")));
        let second = fixture.dir(1).join(Utf8Path::new(&tool_name("tool")));
        ensure!(
            output.first() == Some(&first),
            "incorrect first match: {output:?}"
        );
        ensure!(
            output.get(1) == Some(&second),
            "incorrect second match: {output:?}"
        );
        Ok(())
    }

    #[rstest]
    fn canonical_removes_duplicates() -> Result<()> {
        let fixture = EnvFixture::new(&["bin", "bin_dot"])?;
        fixture.write_tool(0, "dedupe")?;
        let mut options = WhichOptions {
            all: true,
            ..WhichOptions::default()
        };
        // Without canonical flag, both entries appear because PATH includes two references.
        let alias = fixture.dir(0).join(".");
        let joined = std::env::join_paths([fixture.dir(0).as_std_path(), alias.as_std_path()])
            .expect("join paths");
        let _ = set_var("PATH", joined.as_os_str());
        let resolver = resolver();
        let duplicates = render(&resolver, "dedupe", &options)?;
        ensure!(
            duplicates.len() == 2,
            "expected duplicates to include two entries"
        );
        options.canonical = true;
        options.fresh = true;
        let canonical = render(&resolver, "dedupe", &options)?;
        ensure!(canonical.len() == 1, "canonical result should dedupe");
        Ok(())
    }

    #[rstest]
    fn fresh_rechecks_existing_entries() -> Result<()> {
        let fixture = EnvFixture::new(&["primary", "secondary"])?;
        fixture.write_tool(1, "swap")?;
        let standard = WhichOptions::default();
        let resolver = resolver();
        let initial = render(&resolver, "swap", &standard)?;
        let secondary = fixture.dir(1).join(Utf8Path::new(&tool_name("swap")));
        ensure!(
            initial.first() == Some(&secondary),
            "initial lookup should prefer the secondary PATH entry"
        );
        fixture.write_tool(0, "swap")?;
        let cached = render(&resolver, "swap", &standard)?;
        ensure!(
            cached.first() == Some(&secondary),
            "cached value should remain"
        );
        let fresh = WhichOptions {
            fresh: true,
            ..WhichOptions::default()
        };
        let refreshed = render(&resolver, "swap", &fresh)?;
        let primary = fixture.dir(0).join(Utf8Path::new(&tool_name("swap")));
        ensure!(
            refreshed.first() == Some(&primary),
            "fresh lookup should return the primary path"
        );
        let latest = render(&resolver, "swap", &standard)?;
        ensure!(
            latest.first() == Some(&primary),
            "cache should now track the updated executable"
        );
        Ok(())
    }

    #[rstest]
    fn cache_invalidates_deleted_entries() -> Result<()> {
        let fixture = EnvFixture::new(&["solo"])?;
        let path = fixture.write_tool(0, "gone")?;
        let resolver = resolver();
        let hits = render(&resolver, "gone", &WhichOptions::default())?;
        ensure!(hits.first() == Some(&path), "expected cached path match");
        EnvFixture::remove_path(&path)?;
        let err = match resolver.resolve("gone", &WhichOptions::default()) {
            Ok(value) => bail!(
                "expected missing executable error but resolver returned {value:?}"
            ),
            Err(err) => err,
        };
        ensure!(
            err.to_string().contains("not found"),
            "missing executable should surface not_found error"
        );
        Ok(())
    }
}
