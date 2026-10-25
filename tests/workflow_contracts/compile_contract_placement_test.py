"""Keep compile-time contracts in Cargo's graph, outside timed test bodies.

Compile-time contracts used to run inside Nextest tests: each one checked a
library with a nested ``cargo check``, read artefact paths out of Cargo's JSON
stream, and handed them to ``rustc`` through ``--extern``. Cargo's package
lock, dependency rebuilds, and compiler-admission waits were therefore charged
to a test's time allowance. ADR-0059 moved every such check into the build
phase: positive public-API checks compile as ``public_api_fixture/``, and the
compile-fail checks are rustdoc ``compile_fail`` examples on the items they
protect. This contract keeps that placement from eroding.

Run via ``make test-workflow-contracts``.
"""

import re
import tomllib

import pytest
from nextest_rust_test_discovery import rust_test_sources
from workflow_loading import REPO_ROOT

FIXTURE = REPO_ROOT / "public_api_fixture"

#: Tokens that only a test resolving Cargo's artefacts for a hand-driven
#: compiler needs, and the reason each one must not return.
ARTEFACT_RESOLUTION_TOKENS: dict[str, str] = {
    '"--extern"': (
        "handing a Cargo artefact to rustc puts compiler work back inside the "
        "test's time allowance"
    ),
    "--message-format=json": (
        "reading artefact paths from Cargo's JSON stream belongs to the retired "
        "direct-rustc harness"
    ),
}

#: The fixture modules and the public boundary each one type-checks. The lint
#: module is feature-gated because `netsuke::lint` is.
FIXTURE_MODULES: dict[str, str] = {
    "cli_configuration": "the cached configuration flow",
    "command_env": "the explicit Ninja environment API",
    "command_list": "the command-list AST",
    "config_cached_discovery": "the cached discovery observer API",
    "ir_gen_error": "the IR-generation error",
    "legacy_recipe_telemetry": "the legacy-recipe metric names",
    "lint": "the lint rule and analysis API",
    "manifest_environment": "the manifest environment loaders",
    "policy_from_str": "Clap-independent policy parsing",
    "verbose_timing_reporter": "the verbose timing reporter constructors",
}

#: Compile-fail examples that replaced direct-rustc fixtures, keyed by the
#: source that must carry them.
COMPILE_FAIL_CONTROLS: dict[str, tuple[str, ...]] = {
    "test_support/src/locale_stubs.rs": (
        "```compile_fail,E0599",
        "let _ = StubEnv::default();",
    ),
    "test_support/src/ninja_semantics.rs": (
        "```compile_fail,E0308",
        "let _ = GeneratedNinja::new(needle);",
        "let _ = RecipeNeedle::new(document);",
    ),
}


@pytest.mark.parametrize("token", sorted(ARTEFACT_RESOLUTION_TOKENS))
def test_no_test_resolves_cargo_artefacts_for_a_compiler(token: str) -> None:
    """Refuse a test that wires Cargo's artefacts into a compiler it starts."""
    offenders = sorted(
        str(path.relative_to(REPO_ROOT))
        for path, source in rust_test_sources().items()
        if token in source
    )
    assert not offenders, (
        f"{ARTEFACT_RESOLUTION_TOKENS[token]}; found {token} in {offenders}"
    )


def test_public_api_fixture_sees_only_the_netsuke_library() -> None:
    """Keep the fixture an external embedder with no other dependency."""
    root = tomllib.loads((REPO_ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    fixture = tomllib.loads((FIXTURE / "Cargo.toml").read_text(encoding="utf-8"))
    assert "public_api_fixture" in root["workspace"]["members"], (
        "the fixture must stay a workspace member, or no ordinary build "
        "type-checks the public API"
    )
    assert fixture["package"]["publish"] is False, "the fixture must never publish"
    assert fixture["dependencies"] == {"netsuke-build": {"path": ".."}}, (
        "the fixture must depend on netsuke alone, so each module sees only the "
        "public API; a second dependency would let `policy_from_str` name Clap"
    )
    assert fixture["features"] == {"lint": ["netsuke-build/lint"]}, (
        "the fixture forwards only the feature that changes the public surface"
    )
    assert "dev-dependencies" not in fixture, (
        "the fixture compiles no tests, so it needs no development dependencies"
    )


@pytest.mark.parametrize("module", sorted(FIXTURE_MODULES))
def test_public_api_fixture_compiles_each_boundary(module: str) -> None:
    """Keep every public boundary declared in the fixture's library root."""
    library = (FIXTURE / "src" / "lib.rs").read_text(encoding="utf-8")
    assert re.search(rf"^pub mod {module};$", library, re.MULTILINE), (
        f"public_api_fixture must declare `{module}`, or nothing type-checks "
        f"{FIXTURE_MODULES[module]} from outside the crate"
    )
    assert (FIXTURE / "src" / f"{module}.rs").is_file(), (
        f"public_api_fixture/src/{module}.rs must exist"
    )


def test_public_api_fixture_gates_the_lint_module_on_its_feature() -> None:
    """Compile the lint boundary exactly when `netsuke::lint` exists."""
    library = (FIXTURE / "src" / "lib.rs").read_text(encoding="utf-8")
    assert '#[cfg(feature = "lint")]\npub mod lint;' in library, (
        "the lint module must follow the lint feature, or a default-features "
        "build fails on a module the library does not export"
    )


@pytest.mark.parametrize("relative", sorted(COMPILE_FAIL_CONTROLS))
def test_support_compile_fail_controls_remain_doctests(relative: str) -> None:
    """Keep the support-crate rejections as executed compile-fail examples."""
    source = (REPO_ROOT / relative).read_text(encoding="utf-8")
    missing = [
        marker for marker in COMPILE_FAIL_CONTROLS[relative] if marker not in source
    ]
    assert not missing, (
        f"{relative} lost compile-fail controls {missing}; without them the "
        "rejection is a doc-comment promise rather than a checked contract"
    )
