//! Keep Netsuke's reserved generated state clear of manifest paths.
//!
//! Two jobs share this module because they share one rule: the namespaces
//! `.netsuke/serial`, `.netsuke/dyndep`, and `.netsuke/phony` belong to
//! Netsuke, not to a manifest. Generation rejects the spellings it can judge
//! lexically; the command layer judges the rest.
//!
//! The first job is the build's refusal. Before it publishes a dyndep sidecar,
//! prunes an obsolete one, or removes the force node, the build resolves every
//! declared path and the selected `--file` manifest against the working
//! directory, so an absolute path, a `..` path that re-enters the workspace, a
//! symbolic-link alias, or a dangling alias cannot name reserved state. The
//! check runs whether or not that state exists: a first build must refuse as
//! firmly as a later one.
//!
//! The second job is the force-node reset. Ninja decides whether an input-less
//! `phony` output is dirty by whether that path exists, so while
//! `.netsuke/phony/force` is absent every edge depending on it rebuilds. A
//! stray file, directory, or link at that path would silently turn `always` and
//! executable `phony` work off, and generation cannot repair it without
//! reading the filesystem. The build therefore clears the node before Ninja
//! starts, through handles opened without following links.
//!
//! The reset takes the refusal's own proof value, so it cannot run without the
//! check that keeps declared outputs and the selected manifest safe.
//!
//! Each question lives in its own unit: [`workspace`] resolves the working
//! directory once, [`policy`] owns the refusal and its proof, [`resolve`]
//! answers whether one spelling reaches reserved state, and [`reset`] owns the
//! namespace handles and the removal.

pub(in crate::runner) use self::policy::refuse_reserved_declarations;
pub(in crate::runner) use self::reset::{ResetOutcome, reset_force_node};
pub(in crate::runner) use self::workspace::Workspace;

mod policy;
mod reset;
mod resolve;
mod workspace;

#[cfg(test)]
mod tests;
