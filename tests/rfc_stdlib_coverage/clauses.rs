//! The cross-cutting contract clauses and their discharge in child RFCs.
//!
//! RFC 0006 section 6 is normative and says so: "A child RFC that does not
//! satisfy every clause below for every helper it adds is not complete." The
//! clause list is therefore derived from the document rather than transcribed —
//! each `### 6.N.` subsection is a clause — and each child RFC must discharge
//! every one of them.
//!
//! The discharge is recorded in a child's section 5 as a two-column table,
//! `Clause | Discharge`, whose clause column holds the clause id in backticks.
//! The set of ids must equal section 6's subsection ids exactly. That is the
//! smallest contract that makes "discharges every clause" checkable without
//! pretending a test can read prose, and it is deliberately not a subsection
//! apiece: the eleven clause subsections record the group's own contract, and
//! the table records how each clause is met.

use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};

use super::{RFC_0006, Repo, Section, backticked, strip_backticks};

/// The heading of a child RFC's clause-discharge subsection.
///
/// Unnumbered on purpose. The table resolves all eleven clause subsections, and
/// numbering it `5.6` collided with the clause's own subsection title, "Type and
/// error contract" — the same title RFC 0006 clause 6.6 carries — leaving a
/// reader to reconcile a subsection titled one way with a table row reading
/// `6.6`. It closes section 5 as `### Clause discharge`.
const CLAUSES_HEADING: &str = "### Clause discharge";

/// The same heading as `tables()` reports it, without its `###`.
///
/// The parser resolves the table by the heading directly above it rather than
/// taking the subsection's first table, so a child that places a table under
/// one of the eleven clause subsections cannot have it mistaken for the
/// discharge table.
const TABLE_HEADING: &str = "Clause discharge";

/// RFC 0006 section 6's clause ids, in document order.
pub(super) fn clause_ids(repo: &Repo) -> Result<Vec<String>> {
    let text = repo.read(RFC_0006)?;
    let document = Section::whole(&text);
    let section = document
        .subsection("## 6. Cross-cutting contract")
        .context("RFC 0006 has no section 6")?;
    let mut ids = Vec::new();
    for line in &section.lines {
        let Some(heading) = line.strip_prefix("### ") else {
            continue;
        };
        let id = heading.split('.').take(2).collect::<Vec<_>>().join(".");
        if id.starts_with("6.") && !ids.contains(&id) {
            ids.push(id);
        }
    }
    ensure!(
        !ids.is_empty(),
        "RFC 0006 section 6 has no numbered subsections, so it states no clauses"
    );
    Ok(ids)
}

/// The clause ids a child RFC discharges.
pub(super) fn discharged(repo: &Repo, file: &str) -> Result<BTreeSet<String>> {
    let text = repo.read(file)?;
    let document = Section::whole(&text);
    let section = document
        .subsection(CLAUSES_HEADING)
        .with_context(|| format!("{file} has no clause-discharge table at {CLAUSES_HEADING}"))?;
    // Matched on the heading text rather than by taking the first table found:
    // the subsection is located by its own heading, and pairing the table with
    // the heading directly above it is what distinguishes it from any table a
    // child places under one of the eleven clause subsections above.
    let Some((_, rows)) = section
        .tables()
        .into_iter()
        .find(|(heading, rows)| !rows.is_empty() && heading == TABLE_HEADING)
    else {
        return Err(anyhow::anyhow!(
            "the clause-discharge subsection of {file} contains no table"
        ));
    };
    let mut ids = BTreeSet::new();
    for row in &rows {
        let cell = row.cell(0, "clause")?;
        let id = strip_backticks(cell).trim().to_owned();
        ensure!(
            id.starts_with("6."),
            "{file}:{} names clause {id:?}, which is not a section 6 clause",
            row.line
        );
        ensure!(
            !strip_backticks(row.cell(1, "discharge")?).trim().is_empty(),
            "{file}:{} discharges {id} with an empty cell",
            row.line
        );
        // Two rows for one clause would leave the *other* ten clauses covered by
        // a set that looks complete, so the duplicate is rejected rather than
        // absorbed. `clause_ids` already de-duplicates on the same rule.
        ensure!(
            ids.insert(id.clone()),
            "{file}:{} discharges clause {id} a second time",
            row.line
        );
    }
    Ok(ids)
}

/// The heading of a child RFC's section 5.
///
/// The child RFCs carry the same section 5 title RFC 0006's own capability-group
/// sections use, because a child RFC restates one group's section 5 in full.
const SECTION_5_HEADING: &str = "## 5. Cross-cutting contract conformance";

/// Locate a child RFC's section 5, and check each of its subsections.
///
/// Split from [`check_subsections`] so the caller does not have to hold the
/// document open: the text is read here, scanned, and dropped.
pub(super) fn check_section_five(
    repo: &Repo,
    file: &str,
    owned: &BTreeSet<String>,
) -> Result<BTreeSet<String>> {
    let text = repo.read(file)?;
    let document = Section::whole(&text);
    let section = document
        .subsection(SECTION_5_HEADING)
        .with_context(|| format!("{file} has no section 5 at {SECTION_5_HEADING}"))?;
    check_subsections(file, &section, owned)
}

/// The subsection heading that closes section 5.
///
/// [`CLAUSES_HEADING`] carries its `### ` because a caller locates the
/// subsection by full heading text; the scans below compare against heading text
/// alone, so the marker is trimmed once here rather than at four call sites.
const CLAUSES_TITLE: &str = "Clause discharge";

/// The exact words `D6` permits in place of a group-specific consequence.
///
/// The plan's anti-vacuity rule admits one escape: a subsection that has nothing
/// group-specific to add says so, naming the clause it defers to. That sentence
/// is fixed, so it is matched as a fixed string. Only the prefix is compared,
/// because the clause number it names depends on which clause the subsection
/// discharges.
const NO_ADDITIONAL_OBLIGATION: &str = "No additional obligation beyond RFC 0006 section 6.";

/// The claim that a helper exists because Ansible has one.
///
/// This is literally the acceptance criterion the task exists to enforce: RFC
/// 0006 is a survey of Ansible's standard library, and a child RFC that
/// justifies a helper by appealing to Ansible rather than to Netsuke's own
/// contract has not made a decision, only deferred one. It is a substring
/// search, so leaving it to review would be indefensible.
///
/// Each phrase is matched at a leading word boundary: `Unlike Ansible`
/// contains `like Ansible` and says the opposite of what this check looks for,
/// so an unbounded search flags a sentence that argues *against* the appeal it
/// is meant to catch.
///
/// A leading boundary alone does not clear `such as Ansible`, because `as` is
/// its own word there — the phrase is the tail of the compound preposition —
/// and the sentence introduces an example rather than a justification. That
/// shape is excluded by name in [`deference_phrase`]. A trailing boundary is
/// deliberately *not* required: `as Ansible does` is the shape the check exists
/// to catch, and it is this plan's own recorded seeded fault, so tightening the
/// right-hand side would delete a control that has been proven to fire.
const DEFERENCE_PHRASES: [&str; 2] = ["as Ansible", "like Ansible"];

/// The exemplar idiom that contains `as Ansible` without deferring to it.
///
/// `such as Ansible`, `as with Ansible`, and `as in Ansible` name a source of
/// examples, not a reason to have a helper. Matched immediately before the
/// phrase so the exclusion cannot swallow a genuine appeal that happens to
/// follow one of these words elsewhere in the sentence.
const EXEMPLAR_PREFIXES: [&str; 3] = ["such ", "with ", "in "];

/// Whether `body` justifies a helper by appeal to Ansible.
///
/// Returns the phrase that matched, so the diagnostic can quote it.
fn deference_phrase(body: &str) -> Option<&'static str> {
    DEFERENCE_PHRASES.into_iter().find(|phrase| {
        body.match_indices(phrase).any(|(at, _)| {
            let Some(prefix) = body.get(..at) else {
                return false;
            };
            // A boundary is the start of the text, or a character that cannot
            // continue a word. `is_alphanumeric` alone would treat the `_` in
            // `unlike_ansible` as a break, and the underscore is a word
            // character in every identifier this corpus uses.
            let bounded = !prefix
                .chars()
                .next_back()
                .is_some_and(|ch| ch.is_alphanumeric() || ch == '_');
            let exemplar = EXEMPLAR_PREFIXES
                .iter()
                .any(|prefix_word| prefix.ends_with(prefix_word));
            bounded && !exemplar
        })
    })
}

/// Check one child RFC's section 5 against the `CONF-1` obligations.
///
/// Three shapes of vacuity are mechanical, and each is caught here. The first is
/// the empty stub: a subsection whose body is present but says nothing. The
/// second is the generic discharge, which discusses the clause but never names a
/// helper the RFC owns — the anti-vacuity rule's own test, since a consequence
/// specific to a group of helpers has to name one of them. The third is the
/// appeal to Ansible, which is the survey's subject rather than its conclusion.
///
/// `owned` is the set the child's registry claims, so a subsection may name any
/// helper it registers and no other. A subsection that names a helper outside
/// that set is the generic discharge wearing a plausible name.
pub(super) fn check_subsections(
    file: &str,
    section_five: &Section<'_>,
    owned: &BTreeSet<String>,
) -> Result<BTreeSet<String>> {
    let mut ids = BTreeSet::new();
    for found in section_five.subsections() {
        // The discharge table closes section 5 and is graded by `discharged`,
        // which reads its own content. Excluding it here is what keeps the two
        // checks from disagreeing about one subsection.
        if found.heading == CLAUSES_TITLE {
            continue;
        }
        let Some(id) = clause_id_of(&found.heading) else {
            continue;
        };
        ensure!(
            ids.insert(id.clone()),
            "{file}:{} is a second section 5 subsection for clause {id}",
            found.line
        );
        let body = found.body.join("\n");
        ensure!(
            !body.trim().is_empty(),
            "{file}:{} is subsection {} of section 5 with an empty body",
            found.line,
            found.heading
        );
        let escape = body.contains(NO_ADDITIONAL_OBLIGATION);
        let named = names_an_owned_helper(&body, owned);
        ensure!(
            escape || named,
            "{file}:{} is subsection {}, whose body names none of the helpers this RFC owns \
             and does not read {:?}. A reviewer cannot tell it apart from a restatement of \
             the clause it discharges",
            found.line,
            found.heading,
            NO_ADDITIONAL_OBLIGATION
        );
        if let Some(phrase) = deference_phrase(&body) {
            return Err(anyhow::anyhow!(
                "{file}:{} is subsection {}, which justifies a helper by appealing to Ansible \
                 ({phrase:?}). RFC 0006 surveys Ansible; it does not adopt its choices",
                found.line,
                found.heading
            ));
        }
    }
    Ok(ids)
}

/// The clause id a section 5 subsection title names, if it names one.
///
/// The id is the leading `N.M` token, and requiring that shape is what keeps the
/// scan off a subsection that is not a clause: `5.1. Registry` resolves the
/// clause its title names, whereas a subsection titled `Worked example` resolves
/// nothing and is body, not a clause discharge.
fn clause_id_of(heading: &str) -> Option<String> {
    let id = heading.split(' ').next()?.trim_end_matches('.');
    let mut parts = id.split('.');
    let (major, minor) = (parts.next()?, parts.next()?);
    (parts.next().is_none() && major.chars().all(|ch| ch.is_ascii_digit()))
        .then(|| format!("6.{minor}"))
        .filter(|_| minor.chars().all(|ch| ch.is_ascii_digit()))
}

/// Whether `body` names at least one helper in `owned`.
///
/// A backticked span rather than a bare word: the registry's helper names are
/// written as code throughout the corpus, and a bare-word scan would read the
/// English words `size`, `hash`, `shell`, and `contents` as helper names in any
/// sentence that happened to use them. Requiring the code span is the whole-token
/// test — a word is either entirely a name or it is not a token this matches.
fn names_an_owned_helper(body: &str, owned: &BTreeSet<String>) -> bool {
    backticked(body)
        .iter()
        .any(|name| owned.contains(name.trim()))
}

#[cfg(test)]
mod deference_tests {
    //! Unit tests for [`deference_phrase`](super::deference_phrase).
    //!
    //! The predicate is the mechanical half of this plan's own acceptance
    //! criterion, and it is a lexical search over prose: the two false shapes
    //! below are sentences a child RFC would plausibly write, and both read as
    //! the reverse of the appeal it exists to catch.

    use super::deference_phrase;

    /// An appeal to Ansible is caught, and quoted back in the diagnostic.
    ///
    /// `as Ansible does` is the recorded seeded fault for `CONF-1`, so it is
    /// pinned here: a change that stopped catching it would leave that control
    /// green and empty.
    #[test]
    fn an_appeal_is_flagged() {
        assert_eq!(deference_phrase("as Ansible does"), Some("as Ansible"));
        assert_eq!(deference_phrase("as Ansible"), Some("as Ansible"));
        assert_eq!(
            deference_phrase("like Ansible's fileglob"),
            Some("like Ansible")
        );
        assert_eq!(
            deference_phrase("This is like Ansible"),
            Some("like Ansible")
        );
    }

    /// A sentence that argues *against* the appeal is not an appeal.
    ///
    /// `Unlike Ansible` contains `like Ansible`, and flagging it would fail a
    /// subsection that has made exactly the decision this check demands.
    #[test]
    fn the_reverse_of_an_appeal_is_not_flagged() {
        assert_eq!(deference_phrase("Unlike Ansible, Netsuke has none"), None);
        assert_eq!(deference_phrase("unlike_ansible"), None);
        assert_eq!(deference_phrase("unlikeAnsible"), None);
    }

    /// Naming Ansible as a source of examples is not deference to it.
    ///
    /// `such as Ansible` introduces an instance, and its `as` is its own word,
    /// so the leading boundary alone does not clear it.
    #[test]
    fn an_example_list_is_not_flagged() {
        assert_eq!(deference_phrase("such as Ansible does today"), None);
        assert_eq!(deference_phrase("helpers such as Ansible has"), None);
        assert_eq!(deference_phrase("as with Ansible"), None);
    }

    /// A sentence mentioning Ansible for another reason is not flagged.
    #[test]
    fn an_unrelated_mention_is_not_flagged() {
        assert_eq!(deference_phrase("because Ansible has one"), None);
        assert_eq!(deference_phrase("Ansible"), None);
        assert_eq!(deference_phrase(""), None);
    }
}
