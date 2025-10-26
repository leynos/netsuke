//! Extensions to [`Cli`](crate::cli::Cli) that depend on the full crate.

use crate::cli::Cli;
use crate::stdlib::NetworkPolicy;
use anyhow::Result;

impl Cli {
    /// Construct the network policy requested through CLI flags.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use netsuke::cli::{BuildArgs, Cli, Commands};
    ///
    /// let cli = Cli {
    ///     file: "Netsukefile".into(),
    ///     directory: None,
    ///     jobs: None,
    ///     verbose: false,
    ///     fetch_allow_scheme: vec!["http".into()],
    ///     fetch_allow_host: Vec::new(),
    ///     fetch_block_host: Vec::new(),
    ///     fetch_default_deny: false,
    ///     command: Some(Commands::Build(BuildArgs { emit: None, targets: vec![] })),
    /// };
    /// let policy = cli.network_policy().expect("policy");
    /// let url = url::Url::parse("http://localhost").unwrap();
    /// assert!(policy.evaluate(&url).is_ok());
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when any provided scheme or host pattern is invalid.
    pub fn network_policy(&self) -> Result<NetworkPolicy> {
        let mut policy = NetworkPolicy::default();
        for scheme in &self.fetch_allow_scheme {
            policy = policy.allow_scheme(scheme)?;
        }

        if self.fetch_default_deny {
            if self.fetch_allow_host.is_empty() {
                policy = policy.deny_all_hosts();
            } else {
                policy = policy.allow_hosts(&self.fetch_allow_host)?;
            }
        } else if !self.fetch_allow_host.is_empty() {
            policy = policy.allow_hosts(&self.fetch_allow_host)?;
        }

        for host in &self.fetch_block_host {
            policy = policy.block_host(host)?;
        }

        Ok(policy)
    }
}
