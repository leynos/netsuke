//! Extensions to [`Cli`](crate::cli::Cli) that depend on the full crate.

use crate::cli::Cli;
use crate::stdlib::NetworkPolicy;
use anyhow::Result;

impl Cli {
    /// Construct the manifest resource ceilings requested through configuration.
    ///
    /// # Errors
    ///
    /// Returns an error when any merged ceiling is zero.
    pub fn manifest_budget_limits(&self) -> Result<crate::manifest::ManifestBudgetLimits> {
        crate::manifest::ManifestBudgetLimits {
            evaluation_fuel: self.manifest_evaluation_fuel,
            manifest_fuel: self.manifest_fuel,
            rendered_value_bytes: self.manifest_rendered_value_bytes,
            rendered_manifest_bytes: self.manifest_rendered_manifest_bytes,
            source_bytes: self.manifest_source_bytes,
            foreach_cardinality: self.manifest_foreach_cardinality,
            expanded_entries: self.manifest_expanded_entries,
        }
        .validate()
    }

    /// Construct the network policy requested through CLI flags.
    ///
    /// Allowlist entries only constrain outbound hosts after
    /// `--fetch-default-deny` is provided. Without default-deny all hosts remain
    /// permitted even when allowlist flags are present.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use netsuke::cli::Cli;
    /// use netsuke::host_pattern::HostPattern;
    ///
    /// let cli = Cli {
    ///     fetch_allow_scheme: vec!["http".into()],
    ///     locale: None,
    ///     ..Cli::default()
    /// };
    /// let policy = cli.network_policy().expect("policy");
    /// let url = url::Url::parse("http://localhost").expect("parse URL");
    /// assert!(policy.evaluate(&url).is_ok());
    /// ```
    ///
    /// ```rust
    /// use netsuke::cli::Cli;
    /// use netsuke::host_pattern::HostPattern;
    ///
    /// let cli = Cli {
    ///     fetch_allow_scheme: vec![String::from("http?")],
    ///     fetch_allow_host: vec![HostPattern::parse("example.com").expect("parse host")],
    ///     locale: None,
    ///     ..Cli::default()
    /// };
    /// let err = cli
    ///     .network_policy()
    ///     .expect_err("network_policy should reject invalid scheme");
    /// assert!(err.to_string().contains("invalid characters"));
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
            policy = policy.deny_all_hosts();
            if !self.fetch_allow_host.is_empty() {
                policy = policy.allow_host_patterns(self.fetch_allow_host.clone())?;
            }
        }

        for host in &self.fetch_block_host {
            policy = policy.block_host_pattern(host.clone());
        }

        Ok(policy)
    }
}
