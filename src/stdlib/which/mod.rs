//! Cross-platform `which` filter and helper function for `MiniJinja`.
//!
//! Resolves executables deterministically across Unix and Windows,
//! honouring user-configurable options for returning every match, emitting
//! canonical paths, bypassing the resolver cache, and opt-in search of the
//! current working directory.

use std::{num::NonZeroUsize, sync::Arc};

use camino::{Utf8Path, Utf8PathBuf};
use minijinja::{
    Environment, Error,
    value::{Kwargs, Value},
};

mod cache;
mod env;
mod error;
mod lookup;
mod options;
pub(crate) use lookup::{WORKSPACE_SKIP_DIRS, WorkspaceSkipList};

pub(crate) use cache::WhichResolver;
pub(crate) use options::WhichOptions;

use error::args_error;

#[derive(Clone, Debug)]
pub(crate) struct WhichConfig {
    pub(crate) cwd_override: Option<Arc<Utf8PathBuf>>,
    pub(crate) workspace_skips: WorkspaceSkipList,
    pub(crate) cache_capacity: NonZeroUsize,
}

impl WhichConfig {
    pub(crate) const fn new(
        cwd_override: Option<Arc<Utf8PathBuf>>,
        workspace_skips: WorkspaceSkipList,
        cache_capacity: NonZeroUsize,
    ) -> Self {
        Self {
            cwd_override,
            workspace_skips,
            cache_capacity,
        }
    }
}

pub(crate) fn register(env: &mut Environment<'_>, config: WhichConfig) {
    let resolver = Arc::new(WhichResolver::new(
        config.cwd_override,
        config.workspace_skips,
        config.cache_capacity,
    ));
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

pub(super) fn format_path_for_output(path: &Utf8Path) -> String {
    #[cfg(windows)]
    {
        path.as_str().replace('\\', "/")
    }
    #[cfg(not(windows))]
    {
        path.as_str().to_owned()
    }
}
