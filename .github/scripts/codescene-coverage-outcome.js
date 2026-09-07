/**
 * Determine the trusted coverage Check Run conclusion from stage outcomes.
 *
 * A skipped submission is neutral only when download and validation succeeded:
 * that represents an intentionally unavailable token. Every other skipped or
 * failed stage must fail closed so branch protection retains the coverage gate.
 *
 * @param {string} downloadOutcome The coverage artefact download outcome.
 * @param {string} validationOutcome The hostile artefact validation outcome.
 * @param {string} submissionOutcome The CodeScene submission outcome.
 * @returns {'success' | 'neutral' | 'failure'} The Check Run conclusion.
 */
function coverageConclusion(
  downloadOutcome,
  validationOutcome,
  submissionOutcome,
) {
  const prerequisitesSucceeded =
    downloadOutcome === 'success' && validationOutcome === 'success';

  if (submissionOutcome === 'success') {
    return 'success';
  }
  if (submissionOutcome === 'skipped' && prerequisitesSucceeded) {
    return 'neutral';
  }
  return 'failure';
}

module.exports = { coverageConclusion };
