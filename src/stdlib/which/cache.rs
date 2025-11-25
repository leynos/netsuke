//! LRU-backed cache for the `which` resolver to avoid repeat filesystem scans.

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    num::NonZeroUsize,
    sync::{Arc, Mutex, MutexGuard},
};

use camino::Utf8PathBuf;
use lru::LruCache;
use minijinja::{Error, ErrorKind};

use super::{env::EnvSnapshot, lookup::lookup, options::WhichOptions};

#[derive(Clone, Debug)]
pub(crate) struct WhichResolver {
    cache: Arc<Mutex<LruCache<CacheKey, CacheEntry>>>,
    cwd_override: Option<Arc<Utf8PathBuf>>,
}

impl WhichResolver {
    pub(crate) fn new(
        cwd_override: Option<Arc<Utf8PathBuf>>,
        cache_capacity: NonZeroUsize,
    ) -> Result<Self, Error> {
        if cache_capacity.get() == 0 {
            // Defensive guard: unreachable with NonZeroUsize but keeps the error surface stable.
            return Err(Error::new(
                ErrorKind::InvalidOperation,
                "which cache capacity must be positive",
            ));
        }

        Ok(Self {
            cache: Arc::new(Mutex::new(LruCache::new(cache_capacity))),
            cwd_override,
        })
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
        guard.get(key).map(|entry| entry.matches.clone())
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

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct CacheKey {
    command: String,
    env_fingerprint: u64,
    cwd: Utf8PathBuf,
    options: WhichOptions,
}

impl CacheKey {
    fn new(command: &str, env: &EnvSnapshot, options: &WhichOptions) -> Self {
        Self {
            command: command.to_owned(),
            env_fingerprint: env_fingerprint(env),
            cwd: env.cwd.clone(),
            options: options.cache_key_view(),
        }
    }
}

fn env_fingerprint(env: &EnvSnapshot) -> u64 {
    let mut hasher = DefaultHasher::new();
    env.raw_path.hash(&mut hasher);
    env.raw_pathext.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;
    use rstest::rstest;
    use std::num::NonZeroUsize;

    fn cache_key_for(command: &str) -> CacheKey {
        CacheKey {
            command: command.to_owned(),
            env_fingerprint: 1,
            cwd: Utf8PathBuf::from("/"),
            options: WhichOptions::default(),
        }
    }

    #[rstest]
    fn cache_capacity_bounds_entries() {
        let resolver =
            WhichResolver::new(None, NonZeroUsize::new(1).expect("non-zero cache capacity"))
                .expect("construct resolver");

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
}
