"""Hold the documented Kani scope wrapper to one evidence contract.

Three documents publish the `systemd-run` wrapper a developer is told to run:
`docs/developers-guide.md` and the 4.2.2 and 4.2.3 ExecPlans. Copies drift. A
flag dropped from one of them is a run that behaves differently from the one
the suite exercises, and the resource caps are the difference between a bounded
verifier and an OOM-killed machine, so these contracts read every routine
example and require the documented shape.

They also require the sections that explain what survives collection, and pin
the Makefile's strict-mode default: a lane that stopped exporting it would
still report green while skipping every scenario it exists to prove.

Run via ``make test-workflow-contracts``.
"""

import re
import typing as typ

import pytest
from workflow_loading import MAKEFILE_PATH, REPO_ROOT

if typ.TYPE_CHECKING:
    from pathlib import Path

#: The documents that publish the wrapper, in the order the table below uses.
DEVELOPERS_GUIDE = REPO_ROOT / "docs" / "developers-guide.md"
PLAN_4_2_2 = (
    REPO_ROOT
    / "docs"
    / "execplans"
    / "4-2-2-kani-harnesses-for-cycle-canonicalization.md"
)
PLAN_4_2_3 = (
    REPO_ROOT
    / "docs"
    / "execplans"
    / "4-2-3-kani-harnesses-for-command-interpolation.md"
)
DOCUMENTS = (DEVELOPERS_GUIDE, PLAN_4_2_2, PLAN_4_2_3)

#: One fenced block, capturing its language tag and body.
#:
#: The fence may be indented: the 4.2.2 plan carries its template inside a list
#: item, so requiring a fence at column zero would silently skip it.
FENCE = re.compile(
    r"^[ \t]*```(?P<language>[\w+-]*)\n(?P<body>.*?)^[ \t]*```",
    re.MULTILINE | re.DOTALL,
)

#: The flags every routine example must carry.
REQUIRED_FLAGS = (
    "--user",
    "--scope",
    "--collect",
    "--expand-environment=no",
    "--unit",
)

#: The properties every routine example must pin.
#:
#: `RuntimeMaxSec` and `TimeoutStopSec` bound the run; the rest cap CPU,
#: memory, swap, task count and I/O weight. None is optional.
REQUIRED_PROPERTIES = (
    "RuntimeMaxSec=",
    "TimeoutStopSec=",
    "CPUQuota=",
    "MemoryMax=",
    "MemorySwapMax=",
    "TasksMax=",
    "IOWeight=",
)

#: How the wrapper applies niceness, because `Nice=` is not accepted here.
REQUIRED_NICE = "nice -n 15"

#: Everything a routine example must contain, in one tuple for the diff.
REQUIRED_TOKENS = (*REQUIRED_FLAGS, *REQUIRED_PROPERTIES, REQUIRED_NICE)

#: The developer-guide headings the evidence contract lives under.
REQUIRED_SECTIONS = (
    "Routine collection and deferred collection",
    "Harvesting evidence from a routine run",
    "Reading a scope's `Result`",
    "Cleaning up a failed scope",
)


def _document_id(path: Path) -> str:
    """Name one document in a parametrized test identifier."""
    return path.stem


def _relative(path: Path) -> str:
    """Return ``path`` relative to the repository root, POSIX-style."""
    return path.relative_to(REPO_ROOT).as_posix()


def _fenced_blocks(path: Path) -> list[str]:
    """Return the body of every fenced code block in ``path``."""
    return [
        match.group("body")
        for match in FENCE.finditer(path.read_text(encoding="utf-8"))
    ]


def _wrapper_blocks(path: Path) -> list[str]:
    """Return the fenced blocks of ``path`` that invoke ``systemd-run``."""
    return [block for block in _fenced_blocks(path) if "systemd-run" in block]


def _routine_blocks(path: Path) -> list[str]:
    """Return the routine examples: a named scope running a piped `make`."""
    return [
        block
        for block in _wrapper_blocks(path)
        if "bash -c" in block and ("make kani-ir" in block or "make kani-full" in block)
    ]


def _placeholder_blocks(path: Path) -> list[str]:
    """Return the template examples, which leave the command as a placeholder."""
    return [block for block in _wrapper_blocks(path) if "<kani-command>" in block]


@pytest.mark.parametrize("path", DOCUMENTS, ids=_document_id)
def test_every_document_publishes_a_routine_example(path: Path) -> None:
    """Require at least one routine example per document.

    Without this the flag and property checks below would pass vacuously on a
    document whose example had been deleted or reshaped out of recognition.
    """
    assert _routine_blocks(path), (
        f"{_relative(path)} must publish a routine `systemd-run` example running "
        "`make kani-ir` or `make kani-full` through `bash -c`"
    )


@pytest.mark.parametrize("path", DOCUMENTS, ids=_document_id)
def test_routine_examples_keep_every_flag_property_and_niceness(path: Path) -> None:
    """Require the documented flags, caps and niceness in every routine example."""
    for block in _routine_blocks(path):
        missing = [token for token in REQUIRED_TOKENS if token not in block]
        assert not missing, (
            f"{_relative(path)}: a routine example omits {missing}. The caps are "
            f"what keep an uncapped verifier from OOM-killing the machine, and "
            f"`--collect` is what stops a failed scope accumulating:\n{block}"
        )


@pytest.mark.parametrize("path", DOCUMENTS, ids=_document_id)
def test_routine_examples_keep_pipefail_and_tee_inside_the_payload(path: Path) -> None:
    """Require `set -o pipefail` and `tee` inside the `bash -c` payload.

    A pipeline written outside the scope cannot carry `pipefail` into it, so
    the status the caller sees would be `tee`'s — zero whenever the capture
    succeeded — and a failing verifier would be masked. The capture must also
    travel as the scope's payload, so it ends where the cap ends.
    """
    for block in _routine_blocks(path):
        payload = block.split("bash -c", 1)[1]
        assert "set -o pipefail" in payload, (
            f"{_relative(path)}: `set -o pipefail` must be set inside the "
            f"`bash -c` payload, not in the shell that launches the scope:\n{block}"
        )
        assert "tee " in payload, (
            f"{_relative(path)}: `tee` must write the capture from inside the "
            f"scope's payload:\n{block}"
        )


def test_the_placeholder_template_collects_and_names_its_unit() -> None:
    """Require the 4.2.2 `systemd-run` template to carry the same options.

    The template is the one example that leaves the verifier command as
    `<kani-command>`, so it cannot carry the payload assertions above. It can,
    and must, carry the same invocation options.
    """
    blocks = _placeholder_blocks(PLAN_4_2_2)
    assert blocks, "the 4.2.2 plan must keep its `<kani-command>` wrapper template"
    for block in blocks:
        missing = [token for token in REQUIRED_TOKENS if token not in block]
        assert not missing, (
            f"{_relative(PLAN_4_2_2)}: the placeholder template "
            f"omits {missing}:\n{block}"
        )
        assert block.index("set -o pipefail") < block.index("systemd-run"), (
            "the placeholder template sets `pipefail` in the invoking shell; the "
            "composed example is the one that carries the pipeline inside the scope"
        )


def test_developer_guide_keeps_the_evidence_contract_sections() -> None:
    """Require the guide to keep the sections the contract is stated in."""
    text = DEVELOPERS_GUIDE.read_text(encoding="utf-8")
    missing = [heading for heading in REQUIRED_SECTIONS if heading not in text]
    assert not missing, (
        "docs/developers-guide.md must keep these evidence-contract sections: "
        f"{missing}"
    )


def test_developer_guide_documents_the_durable_evidence() -> None:
    """Require the guide to name the sidecar, the journal extract and `LoadState`."""
    text = DEVELOPERS_GUIDE.read_text(encoding="utf-8")
    for token in (".status", "journalctl --user -u", "LoadState"):
        assert token in text, (
            f"docs/developers-guide.md must document `{token}`: the status sidecar, "
            "the bounded journal extract and the loaded-unit prerequisite are what "
            "make a collected run's evidence durable"
        )


def test_makefile_defaults_strict_mode_and_exports_it() -> None:
    """Require the strict default and the export that turns skips into failures.

    A lane that lost the export would run the suite with no strict mode: every
    scenario would skip on a host without a per-user manager and the lane would
    still report green.
    """
    text = MAKEFILE_PATH.read_text(encoding="utf-8")
    assert re.search(r"^test-kani-scope-wrapper:", text, re.MULTILINE), (
        "the Makefile must keep the `test-kani-scope-wrapper` target"
    )
    assert re.search(r"^KANI_SCOPE_WRAPPER_STRICT\s*\?=\s*1$", text, re.MULTILINE), (
        "`KANI_SCOPE_WRAPPER_STRICT` must default to 1"
    )
    assert "NETSUKE_KANI_SCOPE_WRAPPER_STRICT=$(KANI_SCOPE_WRAPPER_STRICT)" in text, (
        "the target must export the variable the suite reads as strict mode"
    )
