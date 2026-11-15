//! Require every landed-work entry to name what landed it.
//!
//! The plan's landed-work step asks for one entry each, "with merge commit and
//! the preservation obligation it creates". An entry that names only its issue
//! still reads as complete, so a missing commit is invisible to every other
//! check: the obligations it cites are traced, its owner tasks exist, and the
//! section is populated. That is how twelve entries came to record no commit at
//! all. This rule makes the requirement observable. It checks only the two
//! facts the plan names — a pull request number and a commit — because which
//! commit is the *right* one is a provenance question no reader of one document
//! can settle, and a rule that guessed would be worse than none.

use anyhow::{Result, ensure};

use super::markdown::{bullet_entries, regex_values};

/// A pull-request reference, in the backticked form the inventory uses.
const PR_PATTERN: &str = r"\bPR `#[0-9]+`";

/// A commit reference: a bare short SHA in backticks.
///
/// Eight characters is the shortest prefix this repository uses in prose and
/// the form the plan permits for landed-work entries. A full 40-character SHA is
/// not matched, so an entry naming only the pin does not pass by accident.
const COMMIT_PATTERN: &str = r"`[0-9a-f]{8}`";

/// Require each landed-work bullet to name a pull request and a commit.
///
/// # Errors
///
/// Returns an error for the first bullet that omits either, naming the bullet
/// and which of the two is missing.
pub(super) fn validate_landed_work(landed: &str) -> Result<()> {
    for entry in bullet_entries(landed, opens_landed_work) {
        ensure!(
            !regex_values(PR_PATTERN, &entry).is_empty(),
            "landed-work entry should name the pull request that carried it: {entry}"
        );
        ensure!(
            !regex_values(COMMIT_PATTERN, &entry).is_empty(),
            "landed-work entry should name the commit that landed it: {entry}"
        );
    }
    Ok(())
}

/// Report whether a line opens a landed-work bullet.
///
/// Only a top-level bullet does. An indented bullet belongs to the entry above
/// it, and demanding that a sub-bullet carry its own commit would invent a
/// requirement the plan does not state.
fn opens_landed_work(line: &str) -> bool {
    line.starts_with("- ")
}

#[cfg(test)]
mod tests {
    //! Pin the omission the rule exists to catch and the shapes it must allow.

    use anyhow::{Result, ensure};

    use super::validate_landed_work;

    /// Return the refusal message for a landed-work section, or fail the test.
    fn refusal(landed: &str) -> String {
        match validate_landed_work(landed) {
            Ok(()) => String::from("accepted, but the section should be refused"),
            Err(error) => error.to_string(),
        }
    }

    #[test]
    fn accepts_an_entry_naming_a_pull_request_and_a_commit() -> Result<()> {
        validate_landed_work("- PR `#714` (`2c030fd1`) moved edges into an arena.\n")
    }

    /// The commit may sit on the continuation line of a wrapped bullet.
    #[test]
    fn accepts_a_wrapped_entry_whose_commit_is_on_the_continuation() -> Result<()> {
        validate_landed_work(concat!(
            "- `#652` / PR `#714` moved graph edges into a\n",
            "  canonical edge arena (`2c030fd1`).\n",
        ))
    }

    /// A sub-bullet inherits the requirement of the entry that opens it.
    #[test]
    fn accepts_an_indented_sub_bullet() -> Result<()> {
        validate_landed_work(concat!(
            "- `#652` / PR `#714` (`2c030fd1`) moved graph edges into an arena.\n",
            "  - Preserve alias identity (CO-14).\n",
        ))
    }

    /// An entry that names only its issue is the shape that went unnoticed.
    #[test]
    fn rejects_an_entry_without_a_commit() -> Result<()> {
        let message = refusal("- `#705` / PR `#735` closed the redirect boundary.\n");
        ensure!(
            message.contains("should name the commit that landed it"),
            "error should name the missing commit, got: {message}"
        );
        Ok(())
    }

    /// A commit without a pull request is equally unverifiable.
    #[test]
    fn rejects_an_entry_without_a_pull_request() -> Result<()> {
        let message = refusal("- `#705` (`024a9014`) closed the redirect boundary.\n");
        ensure!(
            message.contains("should name the pull request that carried it"),
            "error should name the missing pull request, got: {message}"
        );
        Ok(())
    }

    /// One entry's commit must not satisfy its neighbour's omission.
    #[test]
    fn rejects_a_second_entry_relying_on_the_first_entrys_commit() -> Result<()> {
        let message = refusal(concat!(
            "- `#652` / PR `#714` (`2c030fd1`) moved edges into an arena.\n",
            "- `#705` / PR `#735` closed the redirect boundary.\n",
        ));
        ensure!(
            message.contains("`#705`"),
            "error should name the entry that omits a commit, got: {message}"
        );
        Ok(())
    }
}
