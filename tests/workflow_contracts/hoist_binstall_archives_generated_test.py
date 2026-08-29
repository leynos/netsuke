"""Hypothesis generated-layout suite for the cargo-binstall hoist step.

Complements the example-based suite in ``hoist_binstall_archives_test.py``
by generating package names, versions, target sets, staging states, and
nested-layout shapes, then asserting the hoist's all-or-none and
name-derivation invariants hold for every generated case. Shared fixtures
and helpers live in ``conftest.py``.

Run via ``make test-workflow-contracts``.
"""

import dataclasses
import operator
import tempfile
from pathlib import Path

# conftest.py inserts scripts/ onto sys.path before this module is imported.
import hoist_binstall_archives as hoist_mod
from hypothesis import given, settings
from hypothesis import strategies as st

TARGET_POOL = [
    "x86_64-unknown-linux-gnu",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
    "riscv64gc-unknown-linux-gnu",
]
STATE_POOL = ["ok", "missing-archive", "missing-sidecar", "duplicate", "collision"]


@dataclasses.dataclass(frozen=True, slots=True)
class GeneratedHoistCase:
    """One generated hoist scenario for the property test.

    Parameters
    ----------
    package
        Generated Cargo package name.
    version
        Generated semantic-style release version.
    targets_and_states
        Unique target triples paired with the staging state to apply.
    layout_seed
        Seed varying the generated nested staging-directory layout.
    """

    package: str
    version: str
    targets_and_states: tuple[tuple[str, str], ...]
    layout_seed: int

    def archive_name(self, target: str) -> str:
        """Return the expected archive name for ``target``.

        Parameters
        ----------
        target
            Target triple to interpolate.

        Returns
        -------
        str
            The ``{package}-{version}-{target}.tar.gz`` archive name.
        """
        return f"{self.package}-{self.version}-{target}.tar.gz"

    @property
    def expects_failure(self) -> bool:
        """Whether any target is in a state that must fail validation."""
        return any(state != "ok" for _target, state in self.targets_and_states)


GENERATED_CASES = st.builds(
    GeneratedHoistCase,
    package=st.from_regex(r"[a-z][a-z0-9-]{0,12}", fullmatch=True),
    version=st.from_regex(r"[0-9]{1,2}\.[0-9]{1,2}\.[0-9]{1,2}", fullmatch=True),
    targets_and_states=st.lists(
        st.tuples(st.sampled_from(TARGET_POOL), st.sampled_from(STATE_POOL)),
        min_size=1,
        max_size=4,
        unique_by=operator.itemgetter(0),
    ).map(tuple),
    layout_seed=st.integers(min_value=0, max_value=7),
)


def build_generated_workspace(root: Path, case: GeneratedHoistCase) -> dict[str, Path]:
    """Write the generated staging and manifest TOML and create ``dist``.

    Parameters
    ----------
    root
        Fresh directory to build the workspace in.
    case
        Generated scenario supplying the targets and package name.

    Returns
    -------
    dict[str, Path]
        Paths keyed by ``staging``, ``manifest``, and ``dist``.
    """
    staging = root / "staging.toml"
    staging.write_text(
        "".join(
            f'[targets.t{index}]\ntarget = "{target}"\n'
            for index, (target, _state) in enumerate(case.targets_and_states)
        ),
        encoding="utf-8",
    )
    manifest = root / "Cargo.toml"
    manifest.write_text(f'[package]\nname = "{case.package}"\n', encoding="utf-8")
    dist = root / "dist"
    dist.mkdir()
    return {"staging": staging, "manifest": manifest, "dist": dist}


def stage_generated_target(
    dist: Path, case: GeneratedHoistCase, index: int
) -> list[Path]:
    """Stage one target in its generated state below ``dist``.

    Parameters
    ----------
    dist
        Dist root to stage below.
    case
        Generated scenario supplying the name, layout seed, and state.
    index
        Position of the target within ``case.targets_and_states``.

    Returns
    -------
    list[Path]
        The staged paths that must remain unchanged when validation fails.
    """
    target, state = case.targets_and_states[index]
    name = case.archive_name(target)
    staged: list[Path] = []
    nested = dist / f"artefact-{index}" / f"stage-{(index + case.layout_seed) % 3}"
    if state != "missing-archive":
        nested.mkdir(parents=True, exist_ok=True)
        (nested / name).write_bytes(f"archive:{name}".encode())
        staged.append(nested / name)
        if state != "missing-sidecar":
            (nested / f"{name}.sha256").write_text("c", encoding="utf-8")
            staged.append(nested / f"{name}.sha256")
    if state == "duplicate":
        other = dist / f"artefact-{index}-dup" / "stage"
        other.mkdir(parents=True)
        (other / name).write_bytes(b"dup")
        staged.append(other / name)
    if state == "collision":
        (dist / name).write_bytes(b"occupied")
    return staged


def assert_invalid_generated_outcome(
    dist: Path,
    case: GeneratedHoistCase,
    staged_paths: list[Path],
    status: int,
) -> None:
    """Assert an invalid generated case failed before moving anything.

    Parameters
    ----------
    dist
        Dist root the hoist ran against.
    case
        Generated scenario supplying the collision names.
    staged_paths
        Every staged path that must remain unchanged.
    status
        Exit status returned by the hoist.
    """
    assert status == 1, "any invalid target state must fail validation"
    for staged in staged_paths:
        assert staged.exists(), (
            f"invalid sets must leave staged file {staged} unchanged"
        )
    for target, state in case.targets_and_states:
        if state == "collision":
            occupied = dist / case.archive_name(target)
            assert occupied.read_bytes() == b"occupied", (
                "pre-existing destination entries must be untouched"
            )


def assert_valid_generated_outcome(
    dist: Path, case: GeneratedHoistCase, status: int
) -> None:
    """Assert a valid generated case hoisted every derived pair.

    Parameters
    ----------
    dist
        Dist root the hoist ran against.
    case
        Generated scenario the archive names derive from.
    status
        Exit status returned by the hoist.
    """
    assert status == 0, "a complete, unique, collision-free set must succeed"
    for target, _state in case.targets_and_states:
        name = case.archive_name(target)
        assert (dist / name).is_file(), (
            f"{name} must be derived from the generated inputs and hoisted"
        )
        assert (dist / f"{name}.sha256").is_file(), (
            f"{name}.sha256 must accompany its archive at the root"
        )


@settings(max_examples=25, deadline=None, derandomize=True)
@given(case=GENERATED_CASES)
def test_hoist_invariants_hold_for_generated_layouts(
    case: GeneratedHoistCase,
) -> None:
    """All-or-none and name-derivation invariants hold across generated cases."""
    # Each example needs a pristine root. A temporary directory managed here
    # avoids depending on a function-scoped pytest fixture, which Hypothesis
    # would otherwise reuse across every example of a single test call.
    with tempfile.TemporaryDirectory(prefix="hoist-property-") as root:
        workspace = build_generated_workspace(Path(root), case)
        staged_paths = [
            path
            for index in range(len(case.targets_and_states))
            for path in stage_generated_target(workspace["dist"], case, index)
        ]

        status = hoist_mod.hoist(
            workspace["dist"], workspace["staging"], workspace["manifest"], case.version
        )

        if case.expects_failure:
            assert_invalid_generated_outcome(
                workspace["dist"], case, staged_paths, status
            )
        else:
            assert_valid_generated_outcome(workspace["dist"], case, status)
