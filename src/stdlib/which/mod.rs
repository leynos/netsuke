//! Cross-platform `which` filter and helper function for `MiniJinja`.
//!
//! Resolves executables deterministically across Unix and Windows,
//! honouring user-configurable options for returning every match, emitting
//! canonical paths, bypassing the resolver cache, and opt-in search of the
//! current working directory.

use std::{ffi::OsString, num::NonZeroUsize, sync::Arc};

use camino::{Utf8Path, Utf8PathBuf};
use minijinja::{
    Environment, Error,
    value::{Kwargs, Value},
};

mod cache;
mod env;
mod lookup;
mod options;
mod resolve_error;
pub(crate) use lookup::{WORKSPACE_SKIP_DIRS, WorkspaceSkipList};

pub(crate) use cache::WhichResolver;
pub(crate) use options::WhichOptions;

use crate::localization::{self, keys};
use resolve_error::ResolveError;

#[derive(Clone, Debug)]
pub(crate) struct WhichConfig {
    pub(crate) cwd_override: Option<Arc<Utf8PathBuf>>,
    pub(crate) path_override: Option<OsString>,
    pub(crate) workspace_skips: WorkspaceSkipList,
    pub(crate) cache_capacity: NonZeroUsize,
}

impl WhichConfig {
    pub(crate) const fn new(
        cwd_override: Option<Arc<Utf8PathBuf>>,
        path_override: Option<OsString>,
        workspace_skips: WorkspaceSkipList,
        cache_capacity: NonZeroUsize,
    ) -> Self {
        Self {
            cwd_override,
            path_override,
            workspace_skips,
            cache_capacity,
        }
    }
}

pub(crate) fn register(env: &mut Environment<'_>, config: WhichConfig) {
    let resolver = Arc::new(WhichResolver::new(
        config.cwd_override,
        config.path_override,
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
    {
        let predicate_resolver = Arc::clone(&resolver);
        env.add_function("command_available", move |value: Value, kwargs: Kwargs| {
            command_available_with(&predicate_resolver, &value, &kwargs).and_then(|output| {
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
        .ok_or_else(|| {
            Error::from(ResolveError::args(localization::message(
                keys::STDLIB_WHICH_COMMAND_EMPTY,
            )))
        })?;
    let options = WhichOptions::from_kwargs(kwargs).map_err(Error::from)?;
    let matches = resolver.resolve(name, &options).map_err(Error::from)?;
    Ok(render_value(&matches, &options))
}

fn command_available_with(
    resolver: &WhichResolver,
    command: &Value,
    kwargs: &Kwargs,
) -> Result<Value, Error> {
    let name = command
        .as_str()
        .map(str::trim)
        .filter(|candidate| !candidate.is_empty())
        .ok_or_else(|| {
            Error::from(ResolveError::args(localization::message(
                keys::STDLIB_WHICH_COMMAND_EMPTY,
            )))
        })?;
    let options = WhichOptions::from_kwargs(kwargs).map_err(Error::from)?;
    kwargs.assert_all_used()?;
    is_command_available(resolver.resolve(name, &options))
        .map(Value::from)
        .map_err(Error::from)
}

pub(super) fn is_command_available(
    result: Result<Vec<Utf8PathBuf>, ResolveError>,
) -> Result<bool, ResolveError> {
    match result {
        Ok(matches) => Ok(!matches.is_empty()),
        Err(ResolveError::NotFound { .. } | ResolveError::DirectNotFound { .. }) => Ok(false),
        Err(err) => Err(err),
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    fn command_available_helper_returns_true_for_matches() {
        let result = is_command_available(Ok(vec![Utf8PathBuf::from("/bin/tool")]));
        assert!(result.expect("availability result"));
    }

    #[rstest]
    fn command_available_helper_returns_false_for_search_misses() {
        let result = is_command_available(Err(ResolveError::NotFound {
            command: "tool".to_owned(),
            dirs: Vec::new(),
            cwd_mode: options::CwdMode::Never,
        }));
        assert!(!result.expect("availability result"));
    }

    #[rstest]
    fn command_available_helper_returns_false_for_direct_misses() {
        let result = is_command_available(Err(ResolveError::DirectNotFound {
            command: "./tool".to_owned(),
            path: Utf8PathBuf::from("/workspace/tool"),
        }));
        assert!(!result.expect("availability result"));
    }

    #[rstest]
    fn command_available_helper_propagates_argument_errors() {
        let result = is_command_available(Err(ResolveError::args("bad option")));
        assert!(matches!(result, Err(ResolveError::Args { .. })));
    }
}
