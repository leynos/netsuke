//! Attempting to hold a `&State` beyond the owning `Captured`'s lifetime
//! must be rejected by the borrow checker.

use minijinja::Environment;

fn main() {
    let mut env = Environment::new();
    env.add_template("greeting", "{{ 1 }}").expect("template");
    let template = env.get_template("greeting").expect("template");
    let state_ref = {
        let captured = template.render_captured(()).expect("render");
        captured.state()
    };
    let _ = state_ref;
}
