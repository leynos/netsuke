"""Hold the scope wrapper's payload to the shape that keeps failures visible.

The wrapper's guarantee is not the scope alone. A failing verifier is reported
only when the payload sets `set -o pipefail` *first* and then pipes the verifier
into a capture, because a pipeline's status is otherwise `tee`'s — zero
whenever the capture succeeded.

Three ways of writing that look right and are not: printing the setting,
setting it after the pipeline, and mentioning the capture without making one.
They are why these checks look at the payload's structure rather than searching
it for words.

Split from ``kani_scope_wrapper_docs_test.py`` to keep every module inside the
repository's 400-line limit. The document contract lives there; this module
owns the payload half.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import kani_scope_wrapper_blocks as blocks
import pytest
from workflow_loading import REPO_ROOT

if typ.TYPE_CHECKING:
    from pathlib import Path

#: The documents that publish the wrapper, paired with their case ids.
DOCUMENTS = (
    pytest.param(REPO_ROOT / "docs" / "developers-guide.md", id="developers-guide"),
    pytest.param(
        REPO_ROOT
        / "docs"
        / "execplans"
        / "4-2-2-kani-harnesses-for-cycle-canonicalization.md",
        id="plan-4-2-2",
    ),
    pytest.param(
        REPO_ROOT
        / "docs"
        / "execplans"
        / "4-2-3-kani-harnesses-for-command-interpolation.md",
        id="plan-4-2-3",
    ),
)


def _relative(path: Path) -> str:
    """Return ``path`` relative to the repository root, POSIX-style.

    Returns
    -------
    str
        The repository-relative path, so a failure names the document
        unambiguously.
    """
    return path.relative_to(REPO_ROOT).as_posix()


@pytest.mark.parametrize("path", DOCUMENTS)
def test_routine_examples_keep_pipefail_and_tee_inside_the_payload(path: Path) -> None:
    """Require `set -o pipefail` and `tee` inside the `bash -c` argument.

    A pipeline written outside the scope cannot carry `pipefail` into it, so
    the status the caller sees would be `tee`'s — zero whenever the capture
    succeeded — and a failing verifier would be masked. The capture must also
    travel as the scope's payload, so it ends where the cap ends.

    The check is made against the quoted argument, not the rest of the block.
    Everything after `bash -c` includes the outer pipeline and any trailing
    comment, so a wrapper ending `bash -c 'make kani-ir' | tee capture # set -o
    pipefail` would satisfy a suffix check while masking the verifier exactly
    as the contract forbids.

    Within the payload the documented shape is checked structurally rather than
    searched: the setting must be the first statement and the capture must be a
    pipeline stage after it.
    """
    for block in blocks.routine_blocks(path):
        payload = blocks.payload_argument(block)
        assert payload is not None, (
            f"{_relative(path)}: a routine example must pass `bash -c` a quoted "
            f"argument, so the payload can be identified:\n{block}"
        )
        assert blocks.payload_is_guarded(payload), (
            f"{_relative(path)}: the `bash -c` payload must set `set -o pipefail` "
            f"as its first statement and then pipe the verifier into `tee`. "
            f"Printing the setting, setting it after the pipeline, or mentioning "
            f"the capture without making one are all rejected, because none of "
            f"them keeps a failing verifier from being masked. The payload was "
            f"{payload!r}:\n{block}"
        )


def test_the_payload_check_rejects_an_outer_pipeline() -> None:
    """Prove the payload check fires on the shape it exists to reject.

    The control keeps `set -o pipefail` and `tee` in the block but moves the
    pipeline outside the quoted argument, which is precisely the wrapper the
    contract forbids. A check that only searched the block's suffix would pass
    it, so this case is what makes the check above meaningful rather than
    merely present.
    """
    masked = (
        "systemd-run --user --scope --collect --unit example "
        "bash -c 'make kani-ir' | tee capture # set -o pipefail"
    )
    payload = blocks.payload_argument(masked)
    assert payload == "make kani-ir", (
        "the payload must be the quoted argument alone, so the outer pipeline "
        f"and the comment are outside it; got {payload!r}"
    )
    assert "set -o pipefail" not in payload, (
        "a `pipefail` written after the closing quote must not count as being "
        "inside the payload"
    )
    assert "tee " not in payload, (
        "a `tee` outside the scope must not count as the scope's capture"
    )


def test_the_payload_check_rejects_the_shapes_that_mask_a_failure() -> None:
    """Prove the payload check rejects every text-but-not-effect shape.

    Each case contains the words the documented payload contains, and none of
    them keeps a failing verifier's status, so each must be rejected.
    """
    misleading = {
        "printed setting": 'echo "set -o pipefail"; make kani-ir 2>&1 | tee cap',
        "setting after the pipeline": "make kani-ir 2>&1 | tee cap; set -o pipefail",
        "printed capture": 'set -o pipefail; echo "tee cap"; make kani-ir',
        "quoted semicolons": 'echo "x;set -o pipefail;y"; make kani-ir 2>&1 | tee cap',
        "quoted pipeline": 'set -o pipefail; echo "make kani-ir | tee /tmp/x"',
        "success after the capture": (
            "set -o pipefail; make kani-ir 2>&1 | tee cap; true"
        ),
    }
    for name, payload in misleading.items():
        assert "set -o pipefail" in payload or "tee " in payload, (
            f"the {name} control must contain the text it is testing"
        )
        assert not blocks.payload_is_guarded(payload), (
            f"a payload that only mentions the shape ({name}) must be rejected; "
            f"got {payload!r}"
        )
    documented = "set -o pipefail; make kani-ir 2>&1 | tee cap"
    assert blocks.payload_is_guarded(documented), (
        f"the documented shape must still be accepted; got {documented!r}"
    )
