//! `NinjaValue` must not cross the consuming escape boundary a second time.

#[derive(Debug)]
pub enum NinjaGenError {
    UnsafeNinjaValue,
}

// Compile the private escape boundary in isolation to reject a second escape.
#[path = "../../src/ninja_gen/escape.rs"]
mod ninja_gen_escape;

fn main() {
    let shell_text = ninja_gen_escape::ShellText::new("$value".into());
    let escaped = ninja_gen_escape::escape_ninja_value(&shell_text)
        .expect("first conversion should be valid");
    let _ = ninja_gen_escape::escape_ninja_value(escaped);
}
