//! Preserve expression values and engine state for shared fuel accounting.

use crate::manifest::{
    budget::{ManifestBudget, ManifestBudgetStage},
    budget_adapter::BudgetErrorExt,
};
use minijinja::{Environment, Error, ErrorKind, Value};

/// Borrow the expression inputs shared by state-preserving evaluation callers.
pub(crate) struct ExpressionEvaluation<'a> {
    /// Supplies expression syntax already validated by the caller.
    pub(crate) expression: &'a str,
    /// Borrows expression-local values without copying the context.
    pub(crate) context: &'a Value,
    /// Identifies the fixed evaluation stage for budget accounting.
    pub(crate) stage: ManifestBudgetStage,
}

/// Evaluate validated expression syntax without rendering its result to text.
///
/// Callers first compile expression syntax so delimiters cannot inject template
/// statements. A template assignment exposes the engine state that the direct
/// expression API discards, retaining value types and refunding unused fuel.
pub(crate) fn evaluate_with_state(
    env: &Environment,
    budget: &ManifestBudget,
    request: &ExpressionEvaluation<'_>,
) -> Result<Value, Error> {
    const PREFIX: &str = "{% set __netsuke_expression_result = (";
    const SUFFIX: &str = ") %}";
    budget
        .charge_source(PREFIX.len() + SUFFIX.len(), ManifestBudgetStage::Source)
        .map_err(|exhaustion| exhaustion.into_error(ErrorKind::InvalidOperation))?;
    let fuel = budget
        .reserve_fuel(request.stage)
        .map_err(|exhaustion| exhaustion.into_error(ErrorKind::OutOfFuel))?;
    let mut bounded_env = env.clone();
    bounded_env.set_fuel(Some(fuel));
    let source = [PREFIX, request.expression, SUFFIX].concat();
    let template = bounded_env.template_from_named_str("<expression>", &source)?;
    let result = template.render_captured_to(request.context, std::io::sink());
    match result {
        Ok(captured) => {
            let evaluated = captured.state();
            if let Some((_, unused)) = evaluated.fuel_levels() {
                budget.refund_unused_fuel(unused);
            }
            Ok(evaluated
                .lookup("__netsuke_expression_result")
                .unwrap_or(Value::UNDEFINED))
        }
        Err(error) if error.kind() == ErrorKind::OutOfFuel => Err(budget
            .fuel_exhaustion(request.stage)
            .into_error(ErrorKind::OutOfFuel)),
        Err(error) => Err(error),
    }
}
