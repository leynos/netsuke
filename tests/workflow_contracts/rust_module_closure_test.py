"""Drive the Rust module-closure rules over a synthetic crate.

The proof-scope contract passes over this repository's source whether or not
a given rule works, since the checked-in scope already covers what the rules
find. These tests build a small crate in which each rule, and each exclusion,
decides exactly one file, and pin the reached set, so that breaking any rule
changes the answer here.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import pytest
from rust_module_closure import kani_seeds, reachable_files
from rust_module_graph import ModuleGraphError, read_crate

if typ.TYPE_CHECKING:
    from pathlib import Path

#: One file per rule. The comment on each says why it is, or is not, reached.
SYNTHETIC_CRATE: dict[str, str] = {
    "src/lib.rs": """
pub mod harness;
pub mod model;
pub mod unrelated;
mod inherent;
mod macros;
mod argument_position;
mod sibling;
mod echo;
#[cfg(test)]
mod test_only;
""",
    # The seed: reaches `model` by a rooted path, `macros` by a call, and
    # `data/fixture.txt` by an include followed by a method call. The comment
    # and the inline test module name `unrelated` and must not reach it, and
    # the test module's unresolvable include must not be read at all.
    "src/harness.rs": """
// crate::unrelated::helper() is prose, not a reference.
#[kani::proof]
fn proof() {
    let value = crate::model::Model::new();
    checked!(value);
    let _ = include_str!("../data/fixture.txt").contains(")");
}

#[cfg(test)]
mod tests {
    use crate::unrelated::helper;
    const OUTSIDE: &str = include_str!(env!("NOT_A_LITERAL"));
}
""",
    # Reached by path; its compiled subtree comes with it, including a
    # `#[path]` child, but not its test-only child.
    "src/model.rs": """
pub struct Model;
pub mod nested;
#[path = "custom/located.rs"]
pub mod located;
#[cfg(test)]
mod model_tests;
""",
    "src/model/nested.rs": "pub fn nested() {}\n",
    "src/model/model_tests.rs": "use crate::unrelated::helper;\n",
    # Reached through `super::super::sibling` from inside the model subtree.
    "src/custom/located.rs": "pub fn located() { super::super::sibling::touch(); }\n",
    # Its `self::echo` names its own child, not the crate's `echo`.
    "src/sibling.rs": "pub mod echo;\npub fn touch() { self::echo::ring(); }\n",
    "src/sibling/echo.rs": "pub fn ring() {}\n",
    # Not reached: only a `self::` path inside `sibling` spells its name.
    "src/echo.rs": "pub fn ring() {}\n",
    # Reached because its `impl` header names `Model`, a type the closure defines.
    "src/inherent.rs": """
impl crate::model::Model {
    pub fn new() -> Self { Self }
}
""",
    # Reached because the harness invokes `checked!`.
    "src/macros.rs": "macro_rules! checked { ($value:expr) => {}; }\n",
    # Not reached: `impl` in argument position is a type, not an impl item.
    "src/argument_position.rs": "pub fn take(_: impl AsRef<crate::model::Model>) {}\n",
    # Not reached: nothing names it outside comments and test code.
    "src/unrelated.rs": "pub fn helper() {}\n",
    # Not reached, and compiled out under `cargo kani`.
    "src/test_only.rs": "use crate::harness;\n",
    "data/fixture.txt": "fixture\n",
}

EXPECTED_REACHED = {
    "src/lib.rs",
    "src/harness.rs",
    "src/model.rs",
    "src/model/nested.rs",
    "src/custom/located.rs",
    "src/sibling.rs",
    "src/sibling/echo.rs",
    "src/inherent.rs",
    "src/macros.rs",
    "data/fixture.txt",
}


def _write_crate(root: Path, files: dict[str, str]) -> Path:
    """Write ``files`` beneath ``root`` and return the crate root file."""
    for relative, text in files.items():
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")
    return root / "src" / "lib.rs"


def _reached(root: Path) -> set[str]:
    """Return the synthetic crate's harness closure, relative to ``root``."""
    crate = read_crate(root / "src" / "lib.rs")
    return {
        path.relative_to(root.resolve()).as_posix()
        for path in reachable_files(crate, kani_seeds(crate))
    }


def test_closure_follows_every_rule_and_no_further(tmp_path: Path) -> None:
    """Pin the reached set of a crate where each rule decides one file."""
    _write_crate(tmp_path, SYNTHETIC_CRATE)
    reached = _reached(tmp_path)
    assert reached == EXPECTED_REACHED, (
        f"unexpected closure: extra {sorted(reached - EXPECTED_REACHED)}, "
        f"missing {sorted(EXPECTED_REACHED - reached)}"
    )


def test_test_only_modules_are_marked(tmp_path: Path) -> None:
    """Mark a `#[cfg(test)]` module, and only that one, as test-only."""
    crate = read_crate(_write_crate(tmp_path, SYNTHETIC_CRATE))
    test_only = {path for path, module in crate.modules.items() if module.is_test_only}
    assert test_only == {("test_only",), ("model", "model_tests")}, (
        f"test-only modules: {test_only}"
    )


def test_seeds_include_cfg_kani_sites(tmp_path: Path) -> None:
    """Treat a `cfg(kani)` site as a seed even when no harness reaches it."""
    files = {
        **SYNTHETIC_CRATE,
        "src/unrelated.rs": "#[cfg(kani)]\npub fn helper() {}\n",
    }
    _write_crate(tmp_path, files)
    assert "src/unrelated.rs" in _reached(tmp_path), "a `cfg(kani)` site must seed"


def test_assembled_include_reaches_the_literal_directory(tmp_path: Path) -> None:
    """Reach the directory of an assembled include's literal prefix."""
    harness = SYNTHETIC_CRATE["src/harness.rs"].replace(
        'include_str!("../data/fixture.txt")',
        'include_str!(concat!("../data/", "fixture.txt"))',
    )
    _write_crate(tmp_path, {**SYNTHETIC_CRATE, "src/harness.rs": harness})
    reached = _reached(tmp_path)
    assert "data" in reached, "the literal prefix's directory must be reached"
    assert "data/fixture.txt" not in reached, "an assembled path names no one file"


@pytest.mark.parametrize(
    ("relative", "text", "message"),
    [
        ("src/unrelated.rs", "mod inline {\n    mod hidden;\n}\n", "nested in a block"),
        ("src/unrelated.rs", "mod absent;\n", "has no file"),
        (
            "src/harness.rs",
            '#[kani::proof]\nfn proof() { include_str!(env!("X")); }\n',
            "without a literal path",
        ),
    ],
)
def test_unsupported_layouts_are_refused(
    tmp_path: Path, relative: str, text: str, message: str
) -> None:
    """Refuse a layout the reader would otherwise resolve by guessing."""
    _write_crate(tmp_path, {**SYNTHETIC_CRATE, relative: text})
    with pytest.raises(ModuleGraphError, match=message):
        _reached(tmp_path)
