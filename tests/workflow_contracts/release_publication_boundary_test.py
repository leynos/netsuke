"""Hold release-policy and adapter responsibilities at their boundaries.

Release sources outside the declared transport boundary are network-free.
Policy sources additionally decide only from in-memory values; file access,
serialization, response schemas, and provider-specific formats belong in the
named adapters and CLIs.

Run via ``make test-workflow-contracts``.
"""

import pytest
from release_publication_imports import (
    any_offender,
    imported_modules,
    network_offenders,
    policy_boundary_offenders,
)
from release_publication_modules import (
    ADAPTER_MODULES,
    NETWORK_FREE_MODULES,
    POLICY_MODULES,
    PROCESS_MODULES,
    SOURCE_MODULES,
    TRANSPORT_MODULES,
)
from release_publication_processes import (
    any_process_offender,
    process_offenders,
)
from workflow_loading import REPO_ROOT


@pytest.mark.parametrize("module", NETWORK_FREE_MODULES)
def test_release_sources_import_no_network_client(module: str) -> None:
    """Keep policy and non-transport release sources free of network clients."""
    source = (REPO_ROOT / module).read_text(encoding="utf-8")
    offenders = network_offenders(source)
    assert not offenders, (
        f"{module} imports {offenders!r}, so the release source is no longer "
        "network-free; move the request into the workflow step"
    )


@pytest.mark.parametrize("module", SOURCE_MODULES)
def test_every_process_spawner_is_a_named_decision(module: str) -> None:
    """Require each source that launches a process to say so explicitly.

    ``subprocess`` is not a network module: the command it runs decides that.
    Banning it outright would misreport the packaging seam, and banning it by
    silence would let a new spawner reach a credentialled service unnoticed.
    So each spawner is named in :data:`PROCESS_MODULES` with its reason, and
    that named set is asserted to be exactly the set of sources that import
    one.
    """
    source = (REPO_ROOT / module).read_text(encoding="utf-8")
    offenders = process_offenders(source)
    if module in PROCESS_MODULES:
        assert offenders, (
            f"{module} is listed in PROCESS_MODULES but imports no process "
            "module, so its recorded reason is stale"
        )
    else:
        assert not offenders, (
            f"{module} imports {offenders!r}, so it can launch a process. "
            "Add it to PROCESS_MODULES in release_publication_modules.py with "
            "the reason it must, or keep the work in a workflow step"
        )


@pytest.mark.parametrize("module", TRANSPORT_MODULES)
def test_transport_sources_are_the_only_network_sources(module: str) -> None:
    """Keep urllib within the bounded registry transport boundary."""
    source = (REPO_ROOT / module).read_text(encoding="utf-8")
    assert network_offenders(source), (
        f"{module} must own an explicit part of registry HTTP transport"
    )


def test_the_network_import_detector_has_positive_and_negative_controls() -> None:
    """Prove the import detector finds clients and accepts ordinary modules."""
    positives = (
        "import urllib.request",
        "from urllib.request import urlopen",
        "import http.client as client",
        "from socket import create_connection",
    )
    for source in positives:
        assert any_offender(imported_modules(source)), (
            f"the detector must flag {source!r}, or the no-network check is vacuous"
        )
    negatives = (
        "import release_digest",
        "from pathlib import Path",
        "import typing as typ",
    )
    for source in negatives:
        assert not any_offender(imported_modules(source)), (
            f"the detector must accept {source!r}, a non-network import"
        )


def test_the_process_import_detector_has_positive_and_negative_controls() -> None:
    """Prove the process detector finds spawners and ignores sockets."""
    positives = (
        "import subprocess",
        "import subprocess as sp",
        "from subprocess import run",
    )
    for source in positives:
        assert any_process_offender(imported_modules(source)), (
            f"the detector must flag {source!r}, or the spawner check is vacuous"
        )
    negatives = (
        "import socket",
        "import urllib.request",
        "from pathlib import Path",
    )
    for source in negatives:
        assert not any_process_offender(imported_modules(source)), (
            f"the detector must accept {source!r}, which launches no process"
        )


@pytest.mark.parametrize("module", POLICY_MODULES)
def test_policy_modules_keep_external_effects_in_adapters(module: str) -> None:
    """Keep file, serialization, HTTP, and provider formats at named seams."""
    source = (REPO_ROOT / module).read_text(encoding="utf-8")
    offenders = policy_boundary_offenders(source)
    assert not offenders, (
        f"{module} owns in-memory policy but contains boundary operations "
        f"{offenders!r}; move them to its named adapter or CLI"
    )


def test_every_release_source_is_classified_as_policy_or_adapter() -> None:
    """Require every tested source to belong to exactly one boundary side."""
    policy = set(POLICY_MODULES)
    adapters = set(ADAPTER_MODULES)
    sources = set(SOURCE_MODULES)
    assert not policy & adapters, "a source cannot be both policy and adapter"
    assert policy | adapters == sources, (
        "every release source must be assigned to exactly one side of the boundary"
    )


def test_the_policy_boundary_detector_has_positive_and_negative_controls() -> None:
    """Prove the boundary detector rejects effects and accepts pure policy."""
    positives = (
        "import json",
        "from pathlib import Path",
        "import http.client",
        "def load(path): return path.read_text()",
        "def decode(body): return json.loads(body)",
        "status_code = 404",
        'digest = "sha256:" + value',
    )
    for source in positives:
        assert policy_boundary_offenders(source), (
            f"the detector must flag {source!r}, or the boundary check is vacuous"
        )
    negatives = (
        "def compare(left, right): return left == right",
        "class Observation: checksum: str",
        "def publish(name): return name.endswith('.crate')",
        "from pathlib import PurePosixPath",
    )
    for source in negatives:
        assert not policy_boundary_offenders(source), (
            f"the detector must accept pure policy {source!r}"
        )
