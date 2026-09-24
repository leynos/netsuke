//! The section 7 inventories the parse needs in literal form.
//!
//! Two of the three cannot be derived from the survey tables: section 7.8
//! states the rename exceptions in a sentence, and table 11 gives the count of
//! optioned helpers without ever listing them, their names appearing in three
//! separate places. Each is transcribed here and witnessed against the document
//! by the reader that consumes it, so a prose edit fails loudly instead of
//! drifting silently.
//!
//! The third is the set of disposition tables itself. The parse needs each
//! table's heading, disposition column, and namespace, and no row of the
//! document states them.

use super::Namespace;

/// A section 7 table whose rows carry a disposition.
pub(super) struct CandidateTable {
    /// Heading text as it appears in the document, without the hashes.
    pub(super) heading: &'static str,
    /// Zero-based index of the disposition column.
    pub(super) disposition_column: usize,
    /// Namespace its rows register into.
    pub(super) namespace: Namespace,
}

/// The seven disposition tables of RFC 0006 section 7, in document order.
///
/// The global-function table at 7.7 has one fewer leading column than the six
/// before it, because a global function has no surveyed Ansible spelling
/// distinct from its name.
pub(super) const CANDIDATE_TABLES: [CandidateTable; 7] = [
    CandidateTable {
        heading: "7.1. Core filters",
        disposition_column: 2,
        namespace: Namespace::Filter,
    },
    CandidateTable {
        heading: "7.2. Collection and mathematical filters",
        disposition_column: 2,
        namespace: Namespace::Filter,
    },
    CandidateTable {
        heading: "7.3. URL filters",
        disposition_column: 2,
        namespace: Namespace::Filter,
    },
    CandidateTable {
        heading: "7.4. Core tests",
        disposition_column: 2,
        namespace: Namespace::Test,
    },
    CandidateTable {
        heading: "7.5. Filesystem tests",
        disposition_column: 2,
        namespace: Namespace::Test,
    },
    CandidateTable {
        heading: "7.6. Collection tests",
        disposition_column: 2,
        namespace: Namespace::Test,
    },
    CandidateTable {
        heading: "7.7. Global functions",
        disposition_column: 1,
        namespace: Namespace::Function,
    },
];

/// An accepted helper registered under a name other than the surveyed one.
pub(super) struct Rename {
    /// The surveyed spelling, as it appears in section 7.
    pub(super) surveyed: &'static str,
    /// The registered Netsuke name.
    pub(super) registered: &'static str,
    /// Section that specifies the registered name, as `N.N`.
    pub(super) section: &'static str,
}

/// RFC 0006 section 7.8's rename exceptions, asserted to number three.
///
/// Section 7.8 states these in prose: "Ansible's `hash` becomes `text_hash`
/// (§11.1), its `quote` becomes `shell_quote` (§10.2), and its `win_splitdrive`
/// becomes `splitdrive(dialect='windows')` (§8.6)." The sections recorded here
/// are where each *registered* name is specified in section 8, which for `quote`
/// is not the section its survey row cites.
pub(super) const RENAMES: [Rename; 3] = [
    Rename {
        surveyed: "hash",
        registered: "text_hash",
        section: "8.9",
    },
    Rename {
        surveyed: "quote",
        registered: "shell_quote",
        section: "8.9",
    },
    Rename {
        surveyed: "win_splitdrive",
        registered: "splitdrive",
        section: "8.6",
    },
];

/// An existing helper that gains a behaviour-preserving option.
pub(super) struct Optioned {
    /// The existing Netsuke helper name.
    pub(super) name: &'static str,
    /// The surveyed section 7 row that names it, in either column.
    pub(super) evidence_row: &'static str,
    /// The namespace RFC 0006 section 3.2 places it in.
    ///
    /// The namespace is recorded rather than assumed. Two of the three are
    /// filters and one is a function, and a hardcoded default is not merely
    /// inaccurate for the odd one out: the coverage check compares a child
    /// registry's namespace column against the value derived here, so a wrong
    /// answer makes a *correct* child RFC fail. `glob` is the one that differs,
    /// which is exactly the case a default gets wrong.
    pub(super) namespace: Namespace,
}

/// The three existing helpers gaining an option, from table 11's count of 3.
///
/// Table 11 gives the count. The names come from three separate places:
/// `basename` and `dirname` are section 7 row names, and `glob` appears only in
/// the resolution cell of the `fileglob` row. The namespaces are section 3.2's
/// two lists: `basename` and `dirname` are under Filters, and `glob` is under
/// Functions. `glob` is the only one of the three that is not a filter, which is
/// what makes a default wrong here rather than merely redundant.
pub(super) const OPTIONED: [Optioned; 3] = [
    Optioned {
        name: "basename",
        evidence_row: "basename",
        namespace: Namespace::Filter,
    },
    Optioned {
        name: "dirname",
        evidence_row: "dirname",
        namespace: Namespace::Filter,
    },
    Optioned {
        name: "glob",
        evidence_row: "fileglob",
        namespace: Namespace::Function,
    },
];
