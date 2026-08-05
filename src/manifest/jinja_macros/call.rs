//! Invocation of `MiniJinja` values with explicit keyword arguments.

use minijinja::{
    Error, State,
    value::{Kwargs, Value},
};

/// Invoke a `MiniJinja` value with optional keyword arguments.
///
/// `MiniJinja` encodes keyword arguments by appending a [`Kwargs`] value to the
/// positional slice. This helper hides that convention so callers can pass the
/// keyword collection explicitly.
///
/// # Examples
///
/// ```rust,ignore
/// use minijinja::{Environment, value::{Kwargs, Value}};
/// use netsuke::manifest::jinja_macros::call_macro_value;
///
/// let mut env = Environment::new();
/// env.add_template(
///     "macro",
///     "{% macro greet(name='friend') %}hi {{ name }}{% endmacro %}",
/// )
/// .unwrap();
/// let template = env.get_template("macro").unwrap();
/// let captured = template.render_captured(()).unwrap();
/// let value = captured.state().lookup("greet").unwrap();
/// let kwargs = Kwargs::from_iter([(String::from("name"), Value::from("Ada"))]);
/// let rendered = call_macro_value(captured.state(), &value, &[], Some(kwargs)).unwrap();
/// assert_eq!(rendered.to_string(), "hi Ada");
/// ```
pub(crate) fn call_macro_value(
    state: &State,
    macro_value: &Value,
    positional: &[Value],
    kwargs: Option<Kwargs>,
) -> Result<Value, Error> {
    kwargs.map_or_else(
        || macro_value.call(state, positional),
        |macro_kwargs| {
            let mut call_args = Vec::with_capacity(positional.len() + 1);
            call_args.extend_from_slice(positional);
            call_args.push(Value::from(macro_kwargs));
            macro_value.call(state, call_args.as_slice())
        },
    )
}
