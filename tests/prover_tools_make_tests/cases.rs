//! The per-target expectations the delegated prover-tool cases are built from.
//!
//! Each delegated Make target sends one tool and one flag variable, and the
//! cases need all of it: the recipe name, the argv the tool should receive, and
//! the secret the diagnostics must redact. Keeping the set here leaves the case
//! list to state what it asserts, which is what keeps either file inside the
//! size cap.

/// One delegated prover-tool target and what its invocation should look like.
#[derive(Copy, Clone, Debug)]
pub(super) struct ProverMakeCase {
    /// The Make target under test.
    pub(super) target: &'static str,
    /// The argv the fake tool should receive, in order.
    pub(super) expected_args: &'static [&'static str],
    /// The Make variable that carries this target's flags.
    pub(super) secret_flags_variable: &'static str,
    /// The flag value the diagnostics must not echo.
    pub(super) secret_flags_value: &'static str,
}

impl ProverMakeCase {
    /// Build the Kani installation case.
    ///
    /// Use `ProverMakeCase::install_kani()` to select the `install-kani` recipe.
    pub(super) const fn install_kani() -> Self {
        Self {
            target: "install-kani",
            expected_args: &["kani", "install", "--api-token=kani-install-secret"],
            secret_flags_variable: "KANI_INSTALL_FLAGS",
            secret_flags_value: "--api-token=kani-install-secret",
        }
    }

    /// Build the Verus installation case.
    ///
    /// Use `ProverMakeCase::install_verus()` to select the `install-verus` recipe.
    pub(super) const fn install_verus() -> Self {
        Self {
            target: "install-verus",
            expected_args: &["verus", "install", "--token=verus-install-secret"],
            secret_flags_variable: "VERUS_INSTALL_FLAGS",
            secret_flags_value: "--token=verus-install-secret",
        }
    }

    /// Build the Verus proof-execution case.
    ///
    /// Use `ProverMakeCase::verus()` to select the `verus` recipe.
    pub(super) const fn verus() -> Self {
        Self {
            target: "verus",
            expected_args: &["verus", "run", "--proof-token=verus-run-secret"],
            secret_flags_variable: "VERUS_FLAGS",
            secret_flags_value: "--proof-token=verus-run-secret",
        }
    }
}
