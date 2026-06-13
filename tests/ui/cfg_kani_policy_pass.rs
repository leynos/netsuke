//! Trybuild pass case for repository-level Kani cfg policy.

const CARGO_TOML: &str = include_str!("../../Cargo.toml");
const MAKEFILE: &str = include_str!("../../Makefile");

fn main() {
    assert!(
        CARGO_TOML.contains("[package.metadata.kani.flags]"),
        "Cargo metadata must keep Kani flags visible",
    );
    assert!(
        CARGO_TOML.contains("unexpected_cfgs")
            && CARGO_TOML.contains(r#"check-cfg = ["cfg(kani)"]"#),
        "Cargo lints must declare cfg(kani) as an expected cfg",
    );
    assert!(
        MAKEFILE.contains("kani-ir: kani-full"),
        "Makefile must keep the IR Kani suite alias",
    );
}
