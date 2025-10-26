//! Network helpers exposed to `MiniJinja` templates.
//!
//! Currently this module provides the `fetch` function that retrieves remote
//! resources with optional on-disk caching.

use std::{
    collections::BTreeSet,
    io::{self, Read},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use super::{NetworkConfig, StdlibConfig, value_from_bytes};
use anyhow::{bail, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::fs_utf8::Dir;
use minijinja::{
    Environment, Error, ErrorKind,
    value::{Kwargs, Value},
};
use sha2::{Digest, Sha256};
use url::Url;

/// Declarative allow- and deny-list policy for outbound network requests.
///
/// The policy validates URL schemes and hostnames before a request is
/// dispatched. By default only HTTPS requests are permitted; callers can
/// extend this to support other schemes or restrict hosts.
///
/// # Examples
///
/// ```rust
/// use netsuke::stdlib::NetworkPolicy;
/// use url::Url;
///
/// let policy = NetworkPolicy::default();
/// let url = Url::parse("https://example.com/data.txt").unwrap();
/// assert!(policy.evaluate(&url).is_ok());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkPolicy {
    allowed_schemes: BTreeSet<String>,
    allowed_hosts: Option<Vec<HostPattern>>,
    blocked_hosts: Vec<HostPattern>,
}

impl NetworkPolicy {
    /// Create a policy that allows only HTTPS requests.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use netsuke::stdlib::NetworkPolicy;
    /// use url::Url;
    ///
    /// let policy = NetworkPolicy::https_only();
    /// let allowed = Url::parse("https://example.com").unwrap();
    /// let denied = Url::parse("http://example.com").unwrap();
    /// assert!(policy.evaluate(&allowed).is_ok());
    /// assert!(policy.evaluate(&denied).is_err());
    /// ```
    #[must_use]
    pub fn https_only() -> Self {
        let mut schemes = BTreeSet::new();
        schemes.insert(String::from("https"));
        Self {
            allowed_schemes: schemes,
            allowed_hosts: None,
            blocked_hosts: Vec::new(),
        }
    }

    /// Permit an additional URL scheme such as `http`.
    ///
    /// The scheme is lower-cased internally. Invalid scheme characters cause an
    /// error, allowing CLI validation to surface clear diagnostics.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use netsuke::stdlib::NetworkPolicy;
    /// use url::Url;
    ///
    /// let policy = NetworkPolicy::default()
    ///     .allow_scheme("http")
    ///     .expect("add http scheme");
    /// let url = Url::parse("http://localhost").unwrap();
    /// assert!(policy.evaluate(&url).is_ok());
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when the scheme is empty or contains invalid
    /// characters.
    pub fn allow_scheme(mut self, scheme: impl AsRef<str>) -> anyhow::Result<Self> {
        let candidate = scheme.as_ref();
        ensure!(!candidate.is_empty(), "scheme must not be empty");
        ensure!(
            candidate
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')),
            "scheme '{candidate}' contains invalid characters",
        );
        self.allowed_schemes.insert(candidate.to_ascii_lowercase());
        Ok(self)
    }

    /// Restrict requests to hosts that match the supplied patterns.
    ///
    /// Host patterns accept either exact hostnames or `*.example.com`
    /// wildcards. Subsequent calls append to the allowlist.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use netsuke::stdlib::NetworkPolicy;
    /// use url::Url;
    ///
    /// let policy = NetworkPolicy::default()
    ///     .allow_hosts(["example.com", "*.example.org"])
    ///     .unwrap();
    /// let allowed = Url::parse("https://sub.example.org").unwrap();
    /// let denied = Url::parse("https://unauthorised.test").unwrap();
    /// assert!(policy.evaluate(&allowed).is_ok());
    /// assert!(policy.evaluate(&denied).is_err());
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when any host pattern is invalid or when the resulting
    /// allowlist would be empty.
    pub fn allow_hosts<I, S>(mut self, hosts: I) -> anyhow::Result<Self>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut patterns = self.allowed_hosts.take().unwrap_or_default();
        for host in hosts {
            patterns.push(HostPattern::parse(host.as_ref())?);
        }
        if patterns.is_empty() {
            bail!("host allowlist must contain at least one entry");
        }
        self.allowed_hosts = Some(patterns);
        Ok(self)
    }

    /// Block every host until an allowlist is provided.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use netsuke::stdlib::NetworkPolicy;
    /// use url::Url;
    ///
    /// let policy = NetworkPolicy::default()
    ///     .deny_all_hosts()
    ///     .allow_hosts(["example.com"])
    ///     .unwrap();
    /// let allowed = Url::parse("https://example.com").unwrap();
    /// let denied = Url::parse("https://other.test").unwrap();
    /// assert!(policy.evaluate(&allowed).is_ok());
    /// assert!(policy.evaluate(&denied).is_err());
    /// ```
    #[must_use]
    pub fn deny_all_hosts(mut self) -> Self {
        self.allowed_hosts = Some(Vec::new());
        self
    }

    /// Append a host pattern to the blocklist.
    ///
    /// Blocked hosts are denied even when the allowlist permits them.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use netsuke::stdlib::NetworkPolicy;
    /// use url::Url;
    ///
    /// let policy = NetworkPolicy::default()
    ///     .block_host("169.254.169.254")
    ///     .unwrap();
    /// let denied = Url::parse("https://169.254.169.254").unwrap();
    /// assert!(policy.evaluate(&denied).is_err());
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when the host pattern is invalid.
    pub fn block_host(mut self, host: impl AsRef<str>) -> anyhow::Result<Self> {
        let pattern = HostPattern::parse(host.as_ref())?;
        self.blocked_hosts.push(pattern);
        Ok(self)
    }

    /// Validate the supplied URL against the configured policy.
    ///
    /// Returns `Ok(())` when the scheme and host are permitted and `Err`
    /// detailing the violated rule otherwise.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use netsuke::stdlib::NetworkPolicy;
    /// use url::Url;
    ///
    /// let policy = NetworkPolicy::default();
    /// let valid = Url::parse("https://example.com").unwrap();
    /// let invalid = Url::parse("http://example.com").unwrap();
    /// assert!(policy.evaluate(&valid).is_ok());
    /// assert!(policy.evaluate(&invalid).is_err());
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when the scheme is disallowed, the host is missing or
    /// not on the allowlist, or the host matches a blocklist entry.
    pub fn evaluate(&self, url: &Url) -> Result<(), String> {
        let scheme = url.scheme();
        if !self.allowed_schemes.contains(scheme) {
            return Err(format!("scheme '{scheme}' is not permitted"));
        }

        let host = url
            .host_str()
            .ok_or_else(|| String::from("URL must include a host"))?;
        if self
            .allowed_hosts
            .as_ref()
            .is_some_and(|allowlist| !allowlist.iter().any(|pattern| pattern.matches(host)))
        {
            return Err(format!("host '{host}' is not allowlisted"));
        }

        if self
            .blocked_hosts
            .iter()
            .any(|pattern| pattern.matches(host))
        {
            return Err(format!("host '{host}' is blocked"));
        }

        Ok(())
    }
}

impl Default for NetworkPolicy {
    fn default() -> Self {
        Self::https_only()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HostPattern {
    matcher: HostMatcher,
}

impl HostPattern {
    fn parse(pattern: &str) -> anyhow::Result<Self> {
        let trimmed = pattern.trim();
        ensure!(!trimmed.is_empty(), "host pattern must not be empty");
        ensure!(
            !trimmed.contains("://"),
            "host pattern '{trimmed}' must not include a scheme",
        );
        ensure!(
            !trimmed.contains('/'),
            "host pattern '{trimmed}' must not contain '/'",
        );
        let matcher = if let Some(suffix) = trimmed.strip_prefix("*.") {
            ensure!(
                !suffix.is_empty(),
                "wildcard host pattern '{trimmed}' must include a suffix",
            );
            HostMatcher::Suffix(suffix.to_ascii_lowercase())
        } else {
            HostMatcher::Exact(trimmed.to_ascii_lowercase())
        };
        Ok(Self { matcher })
    }

    fn matches(&self, candidate: &str) -> bool {
        let host = candidate.to_ascii_lowercase();
        match &self.matcher {
            HostMatcher::Exact(expected) => host == *expected,
            HostMatcher::Suffix(suffix) => {
                if host == *suffix {
                    return true;
                }
                if let Some(prefix) = host.strip_suffix(suffix) {
                    return prefix.ends_with('.') && prefix.len() > 1;
                }
                false
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum HostMatcher {
    Exact(String),
    Suffix(String),
}

#[derive(Clone)]
struct FetchCache {
    root: Arc<Dir>,
    relative: Utf8PathBuf,
}

impl FetchCache {
    fn new(config: &NetworkConfig) -> Self {
        Self {
            root: Arc::clone(&config.cache_root),
            relative: config.cache_relative.clone(),
        }
    }

    fn open_dir(&self) -> Result<Dir, Error> {
        open_cache_dir(&self.root, &self.relative)
    }
}

#[derive(Clone)]
struct FetchContext {
    cache: FetchCache,
    policy: Arc<NetworkPolicy>,
}

impl FetchContext {
    fn new(config: NetworkConfig) -> Self {
        Self {
            cache: FetchCache::new(&config),
            policy: Arc::new(config.policy),
        }
    }

    fn open_cache_dir(&self) -> Result<Dir, Error> {
        self.cache.open_dir()
    }

    fn policy(&self) -> &NetworkPolicy {
        self.policy.as_ref()
    }
}

/// Register network functions into the template environment.
///
/// Currently registers the `fetch` function for retrieving remote resources
/// with optional on-disk caching.
///
/// # Parameters
///
/// * `env` - `MiniJinja` environment to register functions into.
/// * `impure` - Shared flag for tracking template impurity.
pub(crate) fn register_functions(
    env: &mut Environment<'_>,
    impure: Arc<AtomicBool>,
    config: NetworkConfig,
) {
    let context = Arc::new(FetchContext::new(config));
    env.add_function("fetch", move |url: String, kwargs: Kwargs| {
        fetch(&url, &kwargs, &impure, &context)
    });
}

fn fetch(
    url: &str,
    kwargs: &Kwargs,
    impure: &Arc<AtomicBool>,
    context: &FetchContext,
) -> Result<Value, Error> {
    let use_cache = kwargs.get::<Option<bool>>("cache")?.unwrap_or(false);
    kwargs.assert_all_used()?;

    let parsed = Url::parse(url).map_err(|err| {
        Error::new(
            ErrorKind::InvalidOperation,
            format!("fetch URL '{url}' is invalid: {err}"),
        )
    })?;

    if let Err(message) = context.policy().evaluate(&parsed) {
        return Err(Error::new(
            ErrorKind::InvalidOperation,
            format!("fetch disallowed for '{url}': {message}"),
        ));
    }

    let bytes = if use_cache {
        let dir = context.open_cache_dir()?;
        let key = cache_key(url);
        if let Some(cached) = read_cached(&dir, &key)? {
            impure.store(true, Ordering::Relaxed);
            cached
        } else {
            let data = fetch_remote(&parsed, impure)?;
            write_cache(&dir, &key, &data, impure)?;
            data
        }
    } else {
        fetch_remote(&parsed, impure)?
    };

    Ok(value_from_bytes(bytes))
}

fn fetch_remote(url: &Url, impure: &Arc<AtomicBool>) -> Result<Vec<u8>, Error> {
    impure.store(true, Ordering::Relaxed);
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(30))
        .timeout_write(Duration::from_secs(30))
        .timeout(Duration::from_secs(60))
        .build();
    let response = agent.get(url.as_str()).call().map_err(|err| {
        Error::new(
            ErrorKind::InvalidOperation,
            format!("fetch failed for '{}': {err}", url.as_str()),
        )
    })?;
    let mut reader = response.into_reader();
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).map_err(|err| {
        Error::new(
            ErrorKind::InvalidOperation,
            format!("failed to read response from '{}': {err}", url.as_str()),
        )
    })?;
    Ok(bytes)
}

fn open_cache_dir(root: &Dir, relative: &Utf8Path) -> Result<Dir, Error> {
    if let Err(err) = StdlibConfig::validate_cache_relative(relative) {
        return Err(Error::new(ErrorKind::InvalidOperation, err.to_string()));
    }

    root.create_dir_all(relative)
        .map_err(|err| io_error("create cache dir", relative, &err))?;
    root.open_dir(relative)
        .map_err(|err| io_error("open cache dir", relative, &err))
}

fn read_cached(dir: &Dir, name: &str) -> Result<Option<Vec<u8>>, Error> {
    match dir.open(name) {
        Ok(mut file) => {
            let mut buf = Vec::new();
            file.read_to_end(&mut buf).map_err(|err| {
                Error::new(
                    ErrorKind::InvalidOperation,
                    format!("failed to read cache entry '{name}': {err}"),
                )
            })?;
            Ok(Some(buf))
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(Error::new(
            ErrorKind::InvalidOperation,
            format!("failed to open cache entry '{name}': {err}"),
        )),
    }
}

fn write_cache(dir: &Dir, name: &str, data: &[u8], impure: &Arc<AtomicBool>) -> Result<(), Error> {
    impure.store(true, Ordering::Relaxed);
    dir.write(name, data).map_err(|err| {
        Error::new(
            ErrorKind::InvalidOperation,
            format!("failed to write cache entry '{name}': {err}"),
        )
    })
}

fn cache_key(url: &str) -> String {
    let digest = Sha256::digest(url.as_bytes());
    hex_string(&digest)
}

fn hex_string(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        let result = write!(out, "{byte:02x}");
        #[expect(
            clippy::expect_used,
            reason = "hex output to String should be infallible"
        )]
        result.expect("writing hex to String never fails");
    }
    out
}

fn io_error(action: &str, path: &Utf8Path, err: &io::Error) -> Error {
    Error::new(
        ErrorKind::InvalidOperation,
        format!("{action} for '{path}' failed: {err}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    use anyhow::{Context, Result, anyhow, ensure};
    use std::{
        fs,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    };

    use crate::stdlib::DEFAULT_FETCH_CACHE_DIR;
    use camino::{Utf8Path, Utf8PathBuf};
    use cap_std::{ambient_authority, fs_utf8::Dir};
    use minijinja::value::{Kwargs, Value};
    use rstest::{fixture, rstest};
    use tempfile::tempdir;

    type CacheWorkspace = (tempfile::TempDir, Arc<Dir>, Utf8PathBuf);

    #[fixture]
    fn cache_workspace() -> Result<CacheWorkspace> {
        let temp = tempdir().context("create temporary workspace")?;
        let root_path = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
            .map_err(|path| anyhow!("workspace path not valid UTF-8: {path:?}"))?;
        let dir = Dir::open_ambient_dir(&root_path, ambient_authority())
            .context("open workspace directory")?;
        Ok((temp, Arc::new(dir), root_path))
    }

    fn make_context(root: Arc<Dir>) -> Result<FetchContext> {
        let policy = NetworkPolicy::default().allow_scheme("http")?;
        let config = NetworkConfig {
            cache_root: root,
            cache_relative: Utf8PathBuf::from(DEFAULT_FETCH_CACHE_DIR),
            policy,
        };
        Ok(FetchContext::new(config))
    }

    /// Assert that `open_cache_dir` rejects the provided path with an invalid-operation error.
    fn assert_open_cache_dir_rejects(root: &Dir, path: &Utf8Path, description: &str) -> Result<()> {
        match open_cache_dir(root, path) {
            Ok(dir) => Err(anyhow!(
                "expected {description} to fail but received directory {dir:?}"
            )),
            Err(err) => {
                ensure!(
                    err.kind() == ErrorKind::InvalidOperation,
                    "unexpected error kind {kind:?}",
                    kind = err.kind()
                );
                Ok(())
            }
        }
    }

    #[rstest]
    fn cache_key_stable() {
        assert_eq!(
            cache_key("http://example.com"),
            cache_key("http://example.com"),
        );
    }

    #[rstest]
    fn network_policy_denies_http_scheme() {
        let policy = NetworkPolicy::default();
        let url = Url::parse("http://example.com").expect("parse url");
        assert!(policy.evaluate(&url).is_err());
    }

    #[rstest]
    fn network_policy_allowlist_blocks_other_hosts() -> Result<()> {
        let policy = NetworkPolicy::default()
            .allow_scheme("http")?
            .deny_all_hosts()
            .allow_hosts(["example.com"])?;
        let allowed = Url::parse("http://example.com").expect("parse allowed");
        let denied = Url::parse("http://other.test").expect("parse denied");
        ensure!(
            policy.evaluate(&allowed).is_ok(),
            "allowlisted host should be permitted",
        );
        ensure!(
            policy.evaluate(&denied).is_err(),
            "non-allowlisted host should be rejected",
        );
        Ok(())
    }

    #[rstest]
    fn hex_string_formats_bytes() {
        assert_eq!(hex_string(&[0x0f, 0xa0, 0x3c]), "0fa03c");
    }

    #[rstest]
    fn to_value_preserves_utf8() {
        let value = value_from_bytes(b"payload".to_vec());
        assert_eq!(value.as_str(), Some("payload"));
    }

    #[rstest]
    fn to_value_returns_bytes_for_invalid_utf8() {
        let value = value_from_bytes(vec![0xff, 0xfe, 0xfd]);
        assert_eq!(value.as_bytes(), Some(&[0xff, 0xfe, 0xfd][..]));
    }

    #[rstest]
    #[case("", "empty path")]
    #[case("/etc/netsuke-cache", "absolute path")]
    #[case("../escape", "parent path")]
    fn open_cache_dir_rejects_invalid_paths(
        cache_workspace: Result<CacheWorkspace>,
        #[case] path: &str,
        #[case] description: &str,
    ) -> Result<()> {
        let (_temp, root, _path) = cache_workspace?;
        assert_open_cache_dir_rejects(root.as_ref(), Utf8Path::new(path), description)
    }

    /// Write an entry to the cache directory and assert it exists within the workspace.
    fn assert_cache_entry_exists(
        dir: Dir,
        cache_relative: &Utf8Path,
        workspace: &Utf8Path,
        entry_name: &str,
    ) -> Result<()> {
        dir.write(entry_name, b"data")
            .context("write cache entry")?;
        drop(dir);
        let entry = workspace.join(cache_relative).join(entry_name);
        ensure!(
            fs::metadata(entry.as_std_path()).is_ok(),
            "entry {} should exist",
            entry
        );
        Ok(())
    }

    #[rstest]
    fn open_cache_dir_errors_for_file_path(cache_workspace: Result<CacheWorkspace>) -> Result<()> {
        let (_temp, root, path) = cache_workspace?;
        let file_path = path.join("file");
        fs::write(file_path.as_std_path(), b"data").context("write file placeholder")?;
        assert_open_cache_dir_rejects(root.as_ref(), file_path.as_path(), "file path")
    }

    #[rstest]
    fn open_cache_dir_creates_relative_directory(
        cache_workspace: Result<CacheWorkspace>,
    ) -> Result<()> {
        let (_temp, root, path) = cache_workspace?;
        let dir = open_cache_dir(&root, Utf8Path::new("cache"))?;
        assert_cache_entry_exists(dir, Utf8Path::new("cache"), path.as_path(), "entry")
    }

    #[rstest]
    fn fetch_rejects_template_cache_dir_argument(
        cache_workspace: Result<CacheWorkspace>,
    ) -> Result<()> {
        let (_temp, root, _path) = cache_workspace?;
        let context = make_context(root)?;
        let kwargs = [(String::from("cache_dir"), Value::from(".netsuke/cache"))]
            .into_iter()
            .collect::<Kwargs>();
        let impure = Arc::new(AtomicBool::new(false));
        let Err(err) = fetch("http://127.0.0.1:9", &kwargs, &impure, &context) else {
            return Err(anyhow!(
                "expected cache_dir keyword to fail but request succeeded"
            ));
        };
        ensure!(
            err.kind() == ErrorKind::TooManyArguments,
            "unexpected error kind {kind:?}",
            kind = err.kind()
        );
        ensure!(
            err.to_string().contains("cache_dir"),
            "error should mention unexpected cache_dir argument: {err}",
        );
        ensure!(
            !impure.load(Ordering::Relaxed),
            "rejecting cache_dir must not mark the template impure",
        );
        Ok(())
    }

    #[rstest]
    fn fetch_cache_opens_default_directory(cache_workspace: Result<CacheWorkspace>) -> Result<()> {
        let (_temp, root, path) = cache_workspace?;
        let context = make_context(root)?;
        let dir = context.open_cache_dir()?;
        assert_cache_entry_exists(
            dir,
            Utf8Path::new(DEFAULT_FETCH_CACHE_DIR),
            path.as_path(),
            "entry",
        )
    }

    #[rstest]
    fn fetch_rejects_disallowed_scheme(cache_workspace: Result<CacheWorkspace>) -> Result<()> {
        let (_temp, root, _path) = cache_workspace?;
        let config = NetworkConfig {
            cache_root: root,
            cache_relative: Utf8PathBuf::from(DEFAULT_FETCH_CACHE_DIR),
            policy: NetworkPolicy::default(),
        };
        let context = FetchContext::new(config);
        let kwargs = std::iter::empty::<(String, Value)>().collect::<Kwargs>();
        let impure = Arc::new(AtomicBool::new(false));
        let Err(err) = fetch("http://example.com", &kwargs, &impure, &context) else {
            return Err(anyhow!("expected fetch to reject http scheme"));
        };
        ensure!(
            err.to_string().contains("scheme 'http' is not permitted"),
            "error should mention disallowed scheme: {err}",
        );
        ensure!(
            !impure.load(Ordering::Relaxed),
            "policy rejection must not mark the template impure",
        );
        Ok(())
    }
}
