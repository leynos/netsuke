//! Using `Captured::state()` strictly within the owning `Captured`'s
//! lifetime compiles cleanly.

use minijinja::Environment;

fn main() {
    let mut env = Environment::new();
    env.add_template("greeting", "{{ 1 }}").expect("template");
    let template = env.get_template("greeting").expect("template");
    let captured = template.render_captured(()).expect("render");
    let state = captured.state();
    let _ = state.lookup("missing");
}
