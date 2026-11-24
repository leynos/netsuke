//! Verify typed budget exhaustion survives each translation branch.
//!
//! A native callback reports its own exhausted counter as `OutOfFuel`, so the
//! expression, render, and macro-fallback branches must distinguish that error
//! from a bare engine fuel failure. Each case below registers a tiny callback
//! that returns typed collection exhaustion; the old mapping would have
//! replaced it with a fuel-stage exhaustion and lost both the counter and the
//! stage.

use super::invocation::make_macro_fn;
use super::{ExpressionEvaluation, evaluate_with_state, render_template_with_budget};
use crate::manifest::budget::{
    ManifestBudget, ManifestBudgetExhaustion, ManifestBudgetKind, ManifestBudgetLimits,
    ManifestBudgetStage, adapter::BudgetErrorExt,
};
use minijinja::{Environment, Error, ErrorKind, value::Value};
use rstest::{fixture, rstest};

/// Typed exhaustion describing an exhausted collection call allowance.
fn collection_exhaustion() -> ManifestBudgetExhaustion {
    ManifestBudgetExhaustion {
        kind: ManifestBudgetKind::CollectionCallItems,
        stage: ManifestBudgetStage::Collection,
        limit: 4,
    }
}

/// Register a global function that reports typed collection exhaustion.
fn add_exploding_function(env: &mut Environment<'static>) {
    env.add_function("explode", || -> Result<Value, Error> {
        Err(collection_exhaustion().into_error(ErrorKind::OutOfFuel))
    });
}

/// An environment whose `explode` function reports typed collection
/// exhaustion.
#[fixture]
fn exploding_environment() -> Environment<'static> {
    let mut env = Environment::new();
    add_exploding_function(&mut env);
    env
}

/// Fresh manifest accounting for one evaluation.
#[fixture]
fn budget() -> ManifestBudget {
    ManifestBudget::default()
}

/// Find the typed exhaustion beneath a `MiniJinja` error chain.
fn exhaustion_of(error: &Error) -> &ManifestBudgetExhaustion {
    let mut source = std::error::Error::source(error);
    while let Some(current) = source {
        if let Some(exhaustion) = current.downcast_ref::<ManifestBudgetExhaustion>() {
            return exhaustion;
        }
        source = current.source();
    }
    panic!("the error must carry typed budget exhaustion: {error}");
}

#[rstest]
fn expression_evaluation_keeps_typed_collection_exhaustion(
    exploding_environment: Environment<'static>,
    budget: ManifestBudget,
) {
    let error = evaluate_with_state(
        &exploding_environment,
        &budget,
        &ExpressionEvaluation {
            expression: "explode()",
            context: &Value::UNDEFINED,
            stage: ManifestBudgetStage::When,
        },
    )
    .expect_err("the callback must fail the expression");

    let exhaustion = exhaustion_of(&error);
    assert_eq!(
        exhaustion.stage,
        ManifestBudgetStage::Collection,
        "the expression branch must keep the callback's stage"
    );
    assert_eq!(
        exhaustion.kind,
        ManifestBudgetKind::CollectionCallItems,
        "the expression branch must keep the callback's counter"
    );
}

#[rstest]
fn template_render_keeps_typed_collection_exhaustion(
    exploding_environment: Environment<'static>,
    budget: ManifestBudget,
) {
    let error =
        render_template_with_budget(&exploding_environment, "{{ explode() }}", &(), &budget)
            .expect_err("the callback must fail the render");

    let exhaustion = exhaustion_of(&error);
    assert_eq!(
        exhaustion.stage,
        ManifestBudgetStage::Collection,
        "the render branch must keep the callback's stage"
    );
    assert_eq!(exhaustion.limit, 4);
}

#[rstest]
fn macro_fallback_keeps_typed_collection_exhaustion(
    mut exploding_environment: Environment<'static>,
    budget: ManifestBudget,
) {
    let env = &mut exploding_environment;
    env.add_template(
        "macro-template",
        "{% macro boom() %}{{ explode() }}{% endmacro %}",
    )
    .expect("the macro fixture template must compile");
    add_exploding_function(env);
    env.add_function(
        "boom",
        make_macro_fn("macro-template".to_owned(), "boom".to_owned(), budget),
    );

    let expression = env
        .compile_expression("boom()")
        .expect("the macro expression must compile");
    let error = expression
        .eval(())
        .expect_err("the macro body must fail through the callback");

    let exhaustion = exhaustion_of(&error);
    assert_eq!(
        exhaustion.stage,
        ManifestBudgetStage::Collection,
        "the macro fallback must keep the callback's stage"
    );
    assert_eq!(
        exhaustion.kind,
        ManifestBudgetKind::CollectionCallItems,
        "the macro fallback must keep the callback's counter"
    );
}

#[rstest]
fn bare_engine_fuel_exhaustion_still_maps_to_the_evaluation_stage(budget: ManifestBudget) {
    let mut env = Environment::new();
    env.add_function("burn", || -> Result<Value, Error> {
        Err(Error::new(ErrorKind::OutOfFuel, "no fuel"))
    });

    let error = evaluate_with_state(
        &env,
        &budget,
        &ExpressionEvaluation {
            expression: "burn()",
            context: &Value::UNDEFINED,
            stage: ManifestBudgetStage::When,
        },
    )
    .expect_err("the callback must fail the expression");

    let exhaustion = exhaustion_of(&error);
    assert_eq!(
        exhaustion.kind,
        ManifestBudgetKind::Fuel,
        "a bare engine failure must still report the fuel counter"
    );
    assert_eq!(
        exhaustion.stage,
        ManifestBudgetStage::When,
        "a bare engine failure must still report the evaluation stage"
    );
    assert_eq!(
        exhaustion.limit,
        ManifestBudgetLimits::default().evaluation_fuel,
        "a bare engine failure must report the per-evaluation reservation"
    );
}
