//! Read one step's body out of a workflow file.
//!
//! Included by path rather than added to `tests/common`, because that module
//! compiles into every workflow test crate and only some of them read step
//! bodies; an unused item there is a dead-code error under `-D warnings`.

/// Return the lines of one workflow step's body.
///
/// A step runs from its `- name:` marker to the next one at the same
/// indentation, so the body includes the step's `run` script and every input
/// it declares. Returns an empty vector when no such step exists, leaving the
/// caller to decide whether absence is a failure.
pub fn workflow_step_body<'a>(contents: &'a str, step_name: &str) -> Vec<&'a str> {
    let step = format!("- name: {step_name}");
    contents
        .lines()
        .skip_while(|line| !line.contains(&step))
        .take_while(|line| !line.contains("      - name: ") || line.contains(&step))
        .collect()
}
