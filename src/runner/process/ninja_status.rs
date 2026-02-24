//! Ninja status-line parsing for task progress updates.

/// Parsed task progress from a Ninja status line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct NinjaTaskProgress {
    current: u32,
    total: u32,
    description: String,
}

impl NinjaTaskProgress {
    /// Build a parsed status update.
    pub(super) const fn new(current: u32, total: u32, description: String) -> Self {
        Self {
            current,
            total,
            description,
        }
    }

    /// Return the completed task count.
    pub(super) const fn current(&self) -> u32 {
        self.current
    }

    /// Return the total task count.
    pub(super) const fn total(&self) -> u32 {
        self.total
    }

    /// Return the trailing human-readable status text.
    pub(super) fn description(&self) -> &str {
        &self.description
    }
}

/// Tracks task updates and filters regressive or inconsistent lines.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct NinjaTaskProgressTracker {
    total: Option<u32>,
    last_current: u32,
}

impl NinjaTaskProgressTracker {
    /// Accept a new update when it is consistent and monotonic.
    pub(super) const fn accept(&mut self, update: &NinjaTaskProgress) -> bool {
        if update.total() == 0 || update.current() == 0 || update.current() > update.total() {
            return false;
        }
        match self.total {
            Some(total) if total != update.total() || update.current() < self.last_current => false,
            _ => {
                self.total = Some(update.total());
                self.last_current = update.current();
                true
            }
        }
    }
}

/// Parse a Ninja status line in the form `[current/total] description`.
pub(super) fn parse_ninja_status_line(line: &str) -> Option<NinjaTaskProgress> {
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix('[')?;
    let (current_raw, remaining) = rest.split_once('/')?;
    let (total_raw, description_raw) = remaining.split_once(']')?;
    if current_raw.is_empty()
        || total_raw.is_empty()
        || !current_raw.bytes().all(|byte| byte.is_ascii_digit())
        || !total_raw.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let current = current_raw.parse::<u32>().ok()?;
    let total = total_raw.parse::<u32>().ok()?;
    let description = description_raw
        .trim_start()
        .trim_end_matches('\r')
        .to_owned();
    Some(NinjaTaskProgress::new(current, total, description))
}

#[cfg(test)]
mod tests {
    use super::{NinjaTaskProgressTracker, parse_ninja_status_line};
    use rstest::rstest;

    #[rstest]
    #[case("[1/3] cc -c src/a.c", Some((1, 3, "cc -c src/a.c")))]
    #[case("  [2/3] cc -c src/b.c", Some((2, 3, "cc -c src/b.c")))]
    #[case("[3/3]\r", Some((3, 3, "")))]
    #[case("no prefix", None)]
    #[case("[/3] invalid", None)]
    #[case("[2/] invalid", None)]
    #[case("[two/3] invalid", None)]
    #[case("[2/three] invalid", None)]
    #[case("[4/3] invalid", Some((4, 3, "invalid")))]
    fn parse_ninja_status_line_parses_expected(
        #[case] line: &str,
        #[case] expected: Option<(u32, u32, &str)>,
    ) {
        let parsed = parse_ninja_status_line(line);
        let actual = parsed.map(|progress| {
            (
                progress.current(),
                progress.total(),
                progress.description().to_owned(),
            )
        });
        let expected_owned =
            expected.map(|(current, total, description)| (current, total, description.to_owned()));
        assert_eq!(actual, expected_owned);
    }

    #[rstest]
    #[case(vec!["[1/3] a", "[2/3] b", "[3/3] c"], vec![true, true, true])]
    #[case(vec!["[2/3] b", "[1/3] a"], vec![true, false])]
    #[case(vec!["[1/3] a", "[2/4] b"], vec![true, false])]
    #[case(vec!["[1/3] a", "[1/3] a"], vec![true, true])]
    #[case(vec!["[0/3] a"], vec![false])]
    #[case(vec!["[1/0] a"], vec![false])]
    fn tracker_accepts_only_monotonic_updates(
        #[case] lines: Vec<&str>,
        #[case] expected: Vec<bool>,
    ) {
        let mut tracker = NinjaTaskProgressTracker::default();
        let actual: Vec<bool> = lines
            .into_iter()
            .map(|line| {
                parse_ninja_status_line(line).is_some_and(|progress| tracker.accept(&progress))
            })
            .collect();
        assert_eq!(actual, expected);
    }
}
