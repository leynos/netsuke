"""Hold the documented Kani scope wrapper to one evidence contract.

Three documents publish the `systemd-run` wrapper a developer is told to run:
`docs/developers-guide.md` and the 4.2.2 and 4.2.3 ExecPlans. Copies drift. A
flag dropped from one of them is a run that behaves differently from the one
the suite exercises, and the resource caps are the difference between a bounded
verifier and an OOM-killed machine, so these contracts read every routine
example and require the documented shape.

The examples are located and split into arguments and payload by
``kani_scope_wrapper_blocks``; this module states what each part must contain.
The Makefile half lives in ``kani_scope_wrapper_makefile_test.py``, split out
to keep every module inside the repository's 400-line limit.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import kani_scope_wrapper_blocks as blocks
import pytest
from workflow_loading import REPO_ROOT

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
#: Every document that publishes the wrapper, paired with its case id.
#:
#: The ids are stated rather than derived from each stem, so a case reported as
#: ``...[developers-guide]`` keeps that name if a file is renamed.
DOCUMENTS = (
    pytest.param(DEVELOPERS_GUIDE, id="developers-guide"),
    pytest.param(PLAN_4_2_2, id="plan-4-2-2"),
    pytest.param(PLAN_4_2_3, id="plan-4-2-3"),
)

#: The flags every routine example must carry.
REQUIRED_FLAGS = (
    "--user",
    "--scope",
    "--collect",
    "--expand-environment=no",
    "--unit",
)

#: The bounded value each property must carry, as `Name=value`.
#:
#: Checking only that a property is *named* accepts `MemoryMax=infinity`,
#: which removes the cap the example exists to show. These are the documented
#: figures, so an example that weakens one fails here rather than at the point
#: where an uncapped verifier OOM-kills a machine.
#:
#: The values are checked against the *parsed* `-p` arguments, not the block
#: text: a block that runs `-p MemoryMax=16G` while a comment quotes
#: `MemoryMax=8G` would satisfy a text search and exceed the ceiling.
REQUIRED_PROPERTIES = {
    "RuntimeMaxSec": "8m",
    "TimeoutStopSec": "20s",
    "CPUQuota": "200%",
    "MemoryMax": "8G",
    "MemorySwapMax": "0",
    "TasksMax": "96",
    "IOWeight": "20",
}

#: The values that would silently remove a cap, one per property that has one.
#:
#: The negative control in
#: `test_the_property_check_rejects_an_unbounded_value` uses one of these to
#: prove the value check fires.
UNBOUNDED_VALUES = (
    "MemoryMax=infinity",
    "RuntimeMaxSec=infinity",
    "TimeoutStopSec=infinity",
    "CPUQuota=100%",
    "MemorySwapMax=infinity",
    "TasksMax=infinity",
    "IOWeight=0",
)

#: How the wrapper applies niceness, because `Nice=` is not accepted here.
#:
#: Three consecutive argument tokens rather than one string: the niceness is
#: an argument *sequence*, and searching for the phrase as text would neither
#: respect token boundaries nor exclude the payload. The first token is matched
#: on its basename because the documented form is `/usr/bin/nice`.
REQUIRED_NICE = ("nice", "-n", "15")

#: The single-token wrapper flags a routine example must contain.
REQUIRED_TOKENS = REQUIRED_FLAGS

#: The developer-guide headings the evidence contract lives under.
REQUIRED_SECTIONS = (
    "Routine collection and deferred collection",
    "Harvesting evidence from a routine run",
    "Reading a scope's `Result`",
    "Cleaning up a failed scope",
)


def _relative(path: Path) -> str:
    """Return ``path`` relative to the repository root, POSIX-style."""
    return path.relative_to(REPO_ROOT).as_posix()


@pytest.mark.parametrize("path", DOCUMENTS)
def test_every_document_publishes_a_routine_example(path: Path) -> None:
    """Require at least one routine example per document.

    Without this the flag and property checks below would pass vacuously on a
    document whose example had been deleted or reshaped out of recognition.
    """
    assert blocks.routine_blocks(path), (
        f"{_relative(path)} must publish a routine `systemd-run` example running "
        "`make kani-ir` or `make kani-full` through `bash -c`"
    )


@pytest.mark.parametrize("path", DOCUMENTS)
def test_routine_examples_keep_every_flag_property_and_niceness(path: Path) -> None:
    """Require the documented flags, bounded caps and niceness in every example.

    Options are looked for among the wrapper's own argument tokens, and
    properties by their documented values, so neither a comment nor an echoed
    string inside the payload can stand in for the real setting. An example
    that omits `--collect` while its payload prints it would satisfy a
    whole-text search; this does not.
    """
    for block in blocks.routine_blocks(path):
        arguments = blocks.argument_tokens(block)
        missing = [token for token in REQUIRED_TOKENS if token not in arguments]
        assert not missing, (
            f"{_relative(path)}: a routine example omits {missing}. The caps are "
            f"what keep an uncapped verifier from OOM-killing the machine, and "
            f"`--collect` is what stops a failed scope accumulating:\n{block}"
        )
        assert _has_sequence(arguments, REQUIRED_NICE), (
            f"{_relative(path)}: a routine example must run the verifier through "
            f"{' '.join(REQUIRED_NICE)}, so it cannot crowd the machine; the "
            f"arguments were {arguments}"
        )
        properties = blocks.property_arguments(block)
        wrong = [
            f"{name}={properties.get(name)!r} (want {value!r})"
            for name, value in REQUIRED_PROPERTIES.items()
            if properties.get(name) != value
        ]
        assert not wrong, (
            f"{_relative(path)}: a routine example does not set the documented "
            f"bounds {wrong}. These are the caps that keep an uncapped verifier "
            f"from OOM-killing the machine; reading them from the parsed "
            f"arguments is what stops a comment standing in for the setting:\n{block}"
        )


def test_the_property_check_rejects_an_unbounded_value() -> None:
    """Prove the value check fires on the weakening it exists to catch.

    The control keeps every property *name* and drops one bound, which is the
    shape a name-only check accepts.
    """
    weakened = (
        "systemd-run --user --scope --collect -p MemoryMax=infinity bash -c 'true'"
    )
    properties = blocks.property_arguments(weakened)
    assert properties.get("MemoryMax") == "infinity", (
        "the control must set the unbounded value on the argument, or it would "
        f"test something else; parsed {properties}"
    )
    assert properties.get("MemoryMax") != REQUIRED_PROPERTIES["MemoryMax"], (
        "the control must have dropped the documented bound, so the value check "
        "is the one that has to notice"
    )
    assert any(value in weakened for value in UNBOUNDED_VALUES), (
        "the unbounded table must name the value the control uses, or the check "
        "cannot fire on it"
    )


def test_the_property_check_reads_arguments_not_comments() -> None:
    """Prove a comment cannot satisfy the property check.

    The command here sets a 16G ceiling while a comment quotes the documented
    8G, which is the shape a block-text search accepts.
    """
    misleading = (
        "systemd-run --user --scope \\\n"
        "    -p MemoryMax=16G \\\n"
        "    bash -c 'true'  # documented as MemoryMax=8G"
    )
    assert "MemoryMax=8G" in misleading, (
        "the control must quote the documented value somewhere, or it would not "
        "exercise the comment trap"
    )
    properties = blocks.property_arguments(misleading)
    assert properties.get("MemoryMax") == "16G", (
        "the parser must read the argument and ignore the payload and any "
        f"comment beyond it; parsed {properties}"
    )
    assert properties.get("MemoryMax") != REQUIRED_PROPERTIES["MemoryMax"], (
        "the parsed value must be the one the check has to reject"
    )


def test_the_flag_check_ignores_options_inside_the_payload() -> None:
    """Prove a payload cannot supply an option the wrapper omits.

    The control prints `--collect` from inside the payload while the wrapper
    itself never passes it, which is the shape a whole-text search accepts.
    """
    misleading = "systemd-run --user --scope bash -c 'echo --collect'"
    arguments = blocks.argument_tokens(misleading)
    assert "--collect" in misleading, (
        "the control must contain the option text somewhere, or it would not "
        "exercise the payload trap"
    )
    assert "--collect" not in arguments, (
        "the option must be looked for among the wrapper's arguments only; "
        f"parsed {arguments}"
    )


def test_the_placeholder_template_collects_and_names_its_unit() -> None:
    """Require the 4.2.2 `systemd-run` template to carry the same options.

    The template is the one example that leaves the verifier command as
    `<kani-command>`, so it cannot carry the payload assertions above. It can,
    and must, carry the same invocation options.
    """
    templates = blocks.placeholder_blocks(PLAN_4_2_2)
    assert templates, "the 4.2.2 plan must keep its `<kani-command>` wrapper template"
    for block in templates:
        arguments = blocks.argument_tokens(block)
        missing = [token for token in REQUIRED_TOKENS if token not in arguments]
        assert not missing, (
            f"{_relative(PLAN_4_2_2)}: the placeholder template "
            f"omits {missing}:\n{block}"
        )
        assert _has_sequence(arguments, REQUIRED_NICE), (
            f"{_relative(PLAN_4_2_2)}: the placeholder template must run the "
            f"verifier through {' '.join(REQUIRED_NICE)}; the arguments were "
            f"{arguments}"
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
    for token in (
        ".status",
        "journalctl --user --sync",
        "journalctl --user -q -u",
        "LoadState",
    ):
        assert token in text, (
            f"docs/developers-guide.md must document `{token}`: the status sidecar, "
            "the journal sync, the bounded journal extract and the loaded-unit "
            "prerequisite are what make a collected run's evidence durable"
        )


def _has_sequence(arguments: list[str], sequence: tuple[str, ...]) -> bool:
    """Whether ``sequence`` appears as consecutive tokens in ``arguments``.

    The first token is compared on its basename, so `nice` and `/usr/bin/nice`
    both match `nice`. The rest must match exactly: `-n 15` is what sets the
    level, and a different level would be a different claim.

    Returns
    -------
    bool
        True when the tokens appear in order without interruption.
    """
    head, *rest = sequence
    width = len(sequence)
    return any(
        arguments[start].rsplit("/", 1)[-1] == head
        and arguments[start + 1 : start + width] == rest
        for start in range(len(arguments) - width + 1)
    )
