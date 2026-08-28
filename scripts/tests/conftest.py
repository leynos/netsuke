"""Provide shared dynamic-import fixtures for script test modules."""

from __future__ import annotations

import importlib
import importlib.util
import pathlib
import sys
import typing as typ

import pytest

if typ.TYPE_CHECKING:
    import types

SCRIPT_DIRECTORY = pathlib.Path(__file__).resolve().parents[1]


@pytest.fixture(name="rollout_modules")
def rollout_modules_fixture(
    monkeypatch: pytest.MonkeyPatch,
) -> tuple[types.ModuleType, types.ModuleType, types.ModuleType]:
    """Import spelling-rollout scripts through their runtime module paths."""
    monkeypatch.syspath_prepend(str(SCRIPT_DIRECTORY))
    names = ("typos_rollout_cache", "typos_rollout", "generate_typos_config")
    importlib.invalidate_caches()
    cache, rollout, generator = (importlib.import_module(name) for name in names)
    return cache, rollout, generator


def load_script_module(module_name: str, file_name: str) -> types.ModuleType:
    """Import one documentation-coverage module under its required name."""
    spec = importlib.util.spec_from_file_location(
        module_name, SCRIPT_DIRECTORY / file_name
    )
    if spec is None:
        message = "expected import setup to produce a module spec"
        raise AssertionError(message)
    if spec.loader is None:
        message = "expected module spec to provide a loader"
        raise AssertionError(message)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


@pytest.fixture(name="cargo")
def cargo_fixture() -> types.ModuleType:
    """Import the Cargo and Rustdoc adapter under its normal module name."""
    return load_script_module("doc_coverage_cargo", "doc_coverage_cargo.py")


@pytest.fixture(name="runner")
def runner_fixture(cargo: types.ModuleType) -> types.ModuleType:
    """Import the measurement coordinator under its normal module name."""
    return load_script_module("doc_coverage_runner", "doc_coverage_runner.py")


@pytest.fixture(name="script")
def script_fixture(runner: types.ModuleType) -> types.ModuleType:
    """Import ``doc-coverage.py`` under a loadable module name."""
    return load_script_module("doc_coverage_module", "doc-coverage.py")
