//! LRU-backed cache for the `which` resolver to avoid repeat filesystem scans.
use super::{
    WhichConfig,
    env::EnvSnapshot,
    lookup::{WorkspaceSkipList, lookup},
    options::WhichOptions,
    resolve_error::ResolveError,
    telemetry::{
        cwd_mode_label, record_cache_outcome, record_resolution_error, record_resolution_found,
    },
};
use camino::Utf8PathBuf;
use lru::LruCache;
use std::{
    collections::hash_map::DefaultHasher,
    ffi::OsString,
    hash::{Hash, Hasher},
    sync::{Arc, Mutex, MutexGuard},
};
use tracing::field;
/// Shared resolver that caches `which` lookups under an LRU bound.
#[derive(Clone, Debug)]
pub(crate) struct WhichResolver {
    /// LRU mapping cache keys to their stored matches.
    cache: Arc<Mutex<LruCache<CacheKey, CacheEntry>>>,
    /// Override for the working directory lookups run from.
    cwd_override: Option<Arc<Utf8PathBuf>>,
    /// Override for the `PATH` value used during lookups.
    path_override: Option<OsString>,
    /// Override for the `PATHEXT` value used on Windows lookups.
    pathext_override: Option<OsString>,
    /// Directory basenames excluded from the workspace fallback search.
    workspace_skips: WorkspaceSkipList,
}
impl WhichResolver {
    /// Build a resolver from its configuration.
    ///
    /// Takes the whole [`WhichConfig`] rather than its fields individually:
    /// the overrides travel together, and threading each one as a separate
    /// argument grows the signature every time a new environment seam is
    /// added.
    pub(crate) fn new(config: WhichConfig) -> Self {
        let WhichConfig {
            cwd_override,
            path_override,
            pathext_override,
            workspace_skips,
            cache_capacity,
        } = config;
        Self {
            cache: Arc::new(Mutex::new(LruCache::new(cache_capacity))),
            cwd_override,
            path_override,
            pathext_override,
            workspace_skips,
        }
    }
    /// Resolve `command` to executable paths, consulting the cache unless `fresh`.
    /// Capture and lookup failures are recorded as metrics before being returned.
    ///
    /// The span and both counters carry the requested `cwd_mode`, so a
    /// resolution can be attributed to the search domain that produced it
    /// without recording the command or any path.
    pub(crate) fn resolve(
        &self,
        command: &str,
        options: &WhichOptions,
    ) -> Result<Vec<Utf8PathBuf>, ResolveError> {
        let cwd_mode = cwd_mode_label(options.cwd_mode);
        let span = tracing::trace_span!(
            "stdlib.which.resolve",
            cwd_mode,
            cache_outcome = field::Empty,
            result = field::Empty,
            error_category = field::Empty,
        );
        let _guard = span.enter();
        let env = match EnvSnapshot::capture_with_pathext(
            self.cwd_override.as_deref().map(Utf8PathBuf::as_path),
            self.path_override.as_deref(),
            self.pathext_override.as_deref(),
        ) {
            Ok(env) => env,
            Err(err) => {
                record_resolution_error(&span, cwd_mode, &err);
                return Err(err);
            }
        };
        let key = CacheKey::new(command, &env, options, &self.workspace_skips);
        if options.fresh {
            record_cache_outcome(&span, cwd_mode, "bypass");
        } else if let Some(cached) = self.try_cache(&key) {
            record_cache_outcome(&span, cwd_mode, "hit");
            record_resolution_found(&span, cwd_mode);
            return Ok(cached);
        } else {
            record_cache_outcome(&span, cwd_mode, "miss");
        }
        let matches = match lookup(command, &env, options, &self.workspace_skips) {
            Ok(matches) => matches,
            Err(err) => {
                record_resolution_error(&span, cwd_mode, &err);
                return Err(err);
            }
        };
        self.store(key, matches.clone());
        record_resolution_found(&span, cwd_mode);
        Ok(matches)
    }
    // POLONIUS-REFUSED(lock-boundary): the hit is cloned out of the LRU
    // because a reference cannot outlive the `MutexGuard`, and the resolver
    // is shared across template evaluation sites. Owned returns at this
    // boundary are an aliasing/synchronization constraint, not NLL residue.
    /// Return cached matches for a key, if any is present.
    fn try_cache(&self, key: &CacheKey) -> Option<Vec<Utf8PathBuf>> {
        let mut guard = self.lock_cache();
        guard.get(key).map(|entry| entry.matches.clone())
    }
    /// Insert matches under a key for later reuse.
    fn store(&self, key: CacheKey, matches: Vec<Utf8PathBuf>) {
        let mut guard = self.lock_cache();
        guard.put(key, CacheEntry { matches });
    }
    /// Lock the cache, recovering the guard from a poisoned mutex.
    fn lock_cache(&self) -> MutexGuard<'_, LruCache<CacheKey, CacheEntry>> {
        match self.cache.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}
/// Matches stored in the cache for one key.
#[derive(Clone, Debug)]
struct CacheEntry {
    /// The executable paths that resolved for the key.
    matches: Vec<Utf8PathBuf>,
}
/// Identity of one cached resolution.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct CacheKey {
    /// The command that was looked up.
    command: String,
    /// Hash of the environment inputs that affect resolution.
    env_fingerprint: u64,
    /// The working directory the lookup ran from.
    cwd: Utf8PathBuf,
    /// The options the lookup ran with, minus cache-irrelevant flags.
    options: WhichOptions,
    /// The skip list the lookup applied.
    workspace_skips: WorkspaceSkipList,
}
impl CacheKey {
    /// Build a cache key from the lookup inputs.
    fn new(
        command: &str,
        env: &EnvSnapshot,
        options: &WhichOptions,
        workspace_skips: &WorkspaceSkipList,
    ) -> Self {
        Self {
            command: command.to_owned(),
            env_fingerprint: env_fingerprint(env),
            cwd: env.cwd.clone(),
            options: options.cache_key_view(),
            workspace_skips: workspace_skips.clone(),
        }
    }
}
/// Hash the environment inputs that affect a resolution's outcome.
fn env_fingerprint(env: &EnvSnapshot) -> u64 {
    let mut hasher = DefaultHasher::new();
    env.raw_path.hash(&mut hasher);
    env.raw_pathext.hash(&mut hasher);
    // The workspace switch is an environment input like PATH: a fallback hit
    // cached while the switch was on must not answer a resolution made with
    // it off, and vice versa.
    env.workspace_switch().hash(&mut hasher);
    hasher.finish()
}
#[cfg(test)]
mod tests {
    //! Unit tests for the which resolver cache: key derivation, capacity
    //! bounds, and skip-list handling during resolution.
    use super::*;
    use crate::stdlib::which::options::CwdMode;
    use crate::stdlib::which::workspace_switch::WorkspaceSwitch;
    use anyhow::{Result, anyhow, ensure};
    use camino::Utf8PathBuf;
    use rstest::rstest;
    use std::num::NonZeroUsize;
    use tempfile::TempDir;
    fn cache_key_for(command: &str) -> CacheKey {
        CacheKey {
            command: command.to_owned(),
            env_fingerprint: 1,
            cwd: Utf8PathBuf::from("/"),
            options: WhichOptions::default(),
            workspace_skips: WorkspaceSkipList::default(),
        }
    }
    #[rstest]
    fn cache_capacity_bounds_entries() {
        let resolver = WhichResolver::new(WhichConfig::new(
            None,
            None,
            WorkspaceSkipList::default(),
            NonZeroUsize::new(1).expect("non-zero cache capacity"),
        ));
        let first_key = cache_key_for("first");
        let first_path = Utf8PathBuf::from("/bin/first");
        resolver.store(first_key.clone(), vec![first_path.clone()]);
        assert_eq!(
            resolver.try_cache(&first_key),
            Some(vec![first_path.clone()])
        );
        let second_key = cache_key_for("second");
        let second_path = Utf8PathBuf::from("/bin/second");
        resolver.store(second_key.clone(), vec![second_path.clone()]);
        assert!(resolver.try_cache(&first_key).is_none());
        assert_eq!(resolver.try_cache(&second_key), Some(vec![second_path]));
    }
    #[test]
    fn cache_key_differs_when_skip_lists_differ() -> Result<()> {
        let temp = TempDir::new()?;
        let cwd = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
            .map_err(|path| anyhow!("temp path should be utf8: {path:?}"))?;
        let env = EnvSnapshot::capture(Some(cwd.as_path()), Some(std::ffi::OsStr::new("")))?;
        let options = WhichOptions::default();
        let key_a = CacheKey::new(
            "tool",
            &env,
            &options,
            &WorkspaceSkipList::from_names(["target"]),
        );
        let key_b = CacheKey::new(
            "tool",
            &env,
            &options,
            &WorkspaceSkipList::from_names(["build"]),
        );
        ensure!(key_a != key_b, "skip lists must influence cache key");
        Ok(())
    }
    /// Differing workspace-switch readings must not share a cache entry.
    ///
    /// A fallback hit cached while `NETSUKE_WHICH_WORKSPACE` left the search
    /// enabled would otherwise answer a resolution made with it disabled —
    /// cross-toggle cache poisoning. The snapshots here differ only in the
    /// captured switch state, so key inequality proves the fingerprint
    /// covers it.
    #[test]
    fn cache_key_differs_when_workspace_switch_differs() -> Result<()> {
        let temp = TempDir::new()?;
        let cwd = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
            .map_err(|path| anyhow!("temp path should be utf8: {path:?}"))?;
        let base = EnvSnapshot::capture(Some(cwd.as_path()), Some(std::ffi::OsStr::new("")))?;
        let options = WhichOptions::default();
        let skips = WorkspaceSkipList::default();
        let enabled = base.clone().with_workspace_switch(WorkspaceSwitch::Absent);
        let disabled = base.with_workspace_switch(WorkspaceSwitch::Value(String::from("0")));
        ensure!(
            enabled.workspace_fallback_enabled() && !disabled.workspace_fallback_enabled(),
            "the fixtures should sit on opposite sides of the switch"
        );
        let key_enabled = CacheKey::new("tool", &enabled, &options, &skips);
        let key_disabled = CacheKey::new("tool", &disabled, &options, &skips);
        ensure!(
            key_enabled != key_disabled,
            "the workspace switch must influence the cache key"
        );
        Ok(())
    }
    #[test]
    fn cache_key_differs_when_cwd_mode_differs() -> Result<()> {
        let temp = TempDir::new()?;
        let cwd = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
            .map_err(|path| anyhow!("temp path should be utf8: {path:?}"))?;
        let env = EnvSnapshot::capture(Some(cwd.as_path()), Some(std::ffi::OsStr::new("")))?;
        let skips = WorkspaceSkipList::default();
        let keys = [
            CwdMode::Auto,
            CwdMode::Always,
            CwdMode::Never,
            CwdMode::WorkspaceRecursive,
        ]
        .map(|cwd_mode| {
            CacheKey::new(
                "tool",
                &env,
                &WhichOptions {
                    cwd_mode,
                    ..WhichOptions::default()
                },
                &skips,
            )
        });
        ensure!(
            keys.iter()
                .enumerate()
                .all(|(index, key)| keys.iter().skip(index + 1).all(|other| other != key)),
            "every distinct search domain must have a distinct cache key"
        );
        Ok(())
    }
    #[test]
    fn lookup_applies_skip_list_during_resolution() -> Result<()> {
        let temp = TempDir::new()?;
        let cwd = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
            .map_err(|path| anyhow!("temp path should be utf8: {path:?}"))?;
        let target = cwd.join("target");
        test_support::fs::create_dir_all(target.as_std_path())?;
        test_support::write_exec(target.as_std_path(), "tool")?;
        #[cfg(not(windows))]
        let env = super::super::env::mock_env_for_capture(
            Some(std::ffi::OsString::new()),
            Err(std::env::VarError::NotPresent),
        );
        #[cfg(windows)]
        let env = super::super::env::mock_env_for_capture(
            Some(std::ffi::OsString::new()),
            None,
            Err(std::env::VarError::NotPresent),
        );
        let snapshot = EnvSnapshot::capture_with_env(Some(cwd.as_path()), None, &env)?;
        let options = WhichOptions {
            cwd_mode: CwdMode::WorkspaceRecursive,
            ..WhichOptions::default()
        };
        let err = lookup("tool", &snapshot, &options, &WorkspaceSkipList::default())
            .expect_err("default skip should ignore target");
        ensure!(matches!(err, ResolveError::NotFound { .. }));
        let matches = lookup(
            "tool",
            &snapshot,
            &options,
            &WorkspaceSkipList::from_names([".git"]),
        )?;
        ensure!(
            matches == vec![target.join("tool")],
            "expected executable discovery when target not skipped"
        );
        Ok(())
    }
}
