"""Keep nested Cargo compiler diagnostics visible alongside artefact JSON."""

from workflow_loading import REPO_ROOT


def test_json_cargo_preparations_render_compiler_diagnostics() -> None:
    """Keep compiler failures visible while preserving JSON artefact output."""
    sources = (
        REPO_ROOT / "tests" / "support" / "test_support_rlib.rs",
        REPO_ROOT / "tests" / "command_env_ui_tests.rs",
        REPO_ROOT / "tests" / "build_module_slice_ui_tests.rs",
        REPO_ROOT / "tests" / "locale_stub_ui_tests.rs",
    )
    for source in sources:
        text = source.read_text(encoding="utf-8")
        assert '.arg("--message-format=json-render-diagnostics")' in text, (
            f"{source.relative_to(REPO_ROOT)} must render diagnostics to stderr "
            "while retaining Cargo artefact JSON on stdout"
        )
        assert '.arg("--message-format=json")' not in text, (
            "stderr-only error reporting must not discard JSON compiler messages"
        )
