//! Construction of the run's [`StatusReporter`] from resolved output settings.

use crate::display_plan::{DisplayPlan, ReporterKind};
use crate::status::{
    AccessibleReporter, IndicatifReporter, SilentReporter, StatusReporter, VerboseTimingReporter,
};

/// Build the status reporter selected by `plan` and optionally wrap it for timing.
pub(super) fn make_reporter(plan: DisplayPlan, verbose: bool) -> Box<dyn StatusReporter> {
    let prefs = plan.prefs();
    let base: Box<dyn StatusReporter> = match plan.reporter() {
        ReporterKind::Silent => Box::new(SilentReporter),
        ReporterKind::Accessible => Box::new(AccessibleReporter::new(prefs)),
        ReporterKind::Standard => Box::new(IndicatifReporter::with_display_plan(
            prefs,
            plan.force_text_task_updates(),
            plan.live_progress(),
        )),
    };

    if verbose {
        Box::new(VerboseTimingReporter::new(base, prefs))
    } else {
        base
    }
}

/// Adapt the reporter's task-progress method into a mutable callback.
pub(super) fn on_task_progress_callback(
    reporter: &dyn StatusReporter,
) -> impl FnMut(u32, u32, &str) + '_ {
    move |current: u32, total: u32, description: &str| {
        reporter.report_task_progress(current, total, description);
    }
}
