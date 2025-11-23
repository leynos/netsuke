use std::{
    ffi::OsString,
    num::NonZeroUsize,
    sync::{Arc, Mutex, MutexGuard},
};

use camino::Utf8PathBuf;
use lru::LruCache;
use minijinja::Error;

use super::{
    env::EnvSnapshot,
    lookup::{is_executable, lookup},
    options::{CwdMode, WhichOptions},
};

pub(super) const CACHE_CAPACITY: usize = 64;

#[derive(Clone, Debug)]
pub(crate) struct WhichResolver {
    cache: Arc<Mutex<LruCache<CacheKey, CacheEntry>>>,
    cwd_override: Option<Arc<Utf8PathBuf>>,
}

impl WhichResolver {
    pub(crate) fn new(cwd_override: Option<Arc<Utf8PathBuf>>) -> Self {
        #[expect(
            clippy::unwrap_used,
            reason = "cache capacity constant is greater than zero"
        )]
        let capacity = NonZeroUsize::new(CACHE_CAPACITY).unwrap();
        Self {
            cache: Arc::new(Mutex::new(LruCache::new(capacity))),
            cwd_override,
        }
    }

    pub(crate) fn resolve(
        &self,
        command: &str,
        options: &WhichOptions,
    ) -> Result<Vec<Utf8PathBuf>, Error> {
        let env = EnvSnapshot::capture(self.cwd_override.as_deref().map(Utf8PathBuf::as_path))?;
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

    fn lock_cache(&self) -> MutexGuard<'_, LruCache<CacheKey, CacheEntry>> {
        match self.cache.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
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
