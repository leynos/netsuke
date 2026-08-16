//! Construction of the run's [`StatusReporter`] from resolved output settings.

use crate::output_mode::OutputMode;
use crate::output_prefs::OutputPrefs;
use crate::status::{
    AccessibleReporter, IndicatifReporter, SilentReporter, StatusReporter, VerboseTimingReporter,
};

/// Build the appropriate [`StatusReporter`] for the resolved output mode,
/// progress preference, verbose preference, and output preferences.
#[derive(Debug, Clone, Copy)]
pub(super) struct ReporterOptions {
    pub(super) mode: OutputMode,
    pub(super) progress_enabled: bool,
    pub(super) verbose: bool,
    pub(super) prefs: OutputPrefs,
    pub(super) stdout_is_tty: bool,
}

pub(super) fn make_reporter(options: ReporterOptions) -> Box<dyn StatusReporter> {
    let base: Box<dyn StatusReporter> = if options.progress_enabled {
        let force_text_task_updates =
            should_force_text_task_updates(options.mode, options.stdout_is_tty);
        match options.mode {
            OutputMode::Accessible => Box::new(AccessibleReporter::new(options.prefs)),
            OutputMode::Standard => Box::new(IndicatifReporter::with_force_text_task_updates(
                options.prefs,
                force_text_task_updates,
            )),
        }
    } else {
        Box::new(SilentReporter)
    };

    if options.verbose {
        Box::new(VerboseTimingReporter::new(base, options.prefs))
    } else {
        base
    }
}

const fn should_force_text_task_updates(mode: OutputMode, stdout_is_tty: bool) -> bool {
    mode.is_accessible() || !stdout_is_tty
}

#[cfg(test)]
mod tests {
    //! Unit tests for the forced-text-update predicate.

    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(OutputMode::Standard, true, false)]
    #[case(OutputMode::Standard, false, true)]
    #[case(OutputMode::Accessible, true, true)]
    #[case(OutputMode::Accessible, false, true)]
    fn force_text_task_updates_when_required(
        #[case] mode: OutputMode,
        #[case] stdout_is_tty: bool,
        #[case] expected: bool,
    ) {
        assert_eq!(
            should_force_text_task_updates(mode, stdout_is_tty),
            expected
        );
    }
}
