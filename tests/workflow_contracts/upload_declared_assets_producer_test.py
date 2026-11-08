"""Hand each producer's own record to publication, and assert what it does.

The action's execution cases trust a staged tree they were given. Nothing so
far executes the producer step that writes ``producer-provenance.json``, so a
record placed in a directory the artifact upload never carries, or a macOS
package the stamp step names differently from the recorder's guess, would keep
every structural check green while publishing bytes no producer vouched for.

These cases run that step's real body, read from ``build-and-package.yml``,
for every configured target, and then hand the sealed union to the action's
own run bodies. The hand-off is asserted in both directions: the complete
union uploads exactly the declared names, and a union whose producers disagree
is refused before the network is touched.

Run via ``make test-workflow-contracts``.
"""

import json
import pathlib
import shutil
import sys
import typing as typ

import pytest
import upload_declared_assets_producer as producer
import upload_declared_assets_producer_env as producer_env
import upload_declared_assets_producer_tree as tree
import upload_declared_assets_staging as staging
from upload_declared_assets_harness import execute
from upload_declared_assets_manifest import release_tag

if typ.TYPE_CHECKING:
    from upload_declared_assets_harness import ActionRun
    from upload_declared_assets_staging import Facts, Target


@pytest.fixture(name="facts")
def facts_fixture() -> Facts:
    """Return the release facts every case is bound to."""
    return staging.release_facts()


def _runs(tmp_path: pathlib.Path, facts: Facts) -> tuple[producer.ProducerRun, ...]:
    """Execute every configured target's producer step.

    Returns
    -------
    tuple[ProducerRun, ...]
        One completed run per configured target, in key order.
    """
    return tuple(
        producer.run_producer(tmp_path, facts, target) for target in facts.targets
    )


def _sealed(
    tmp_path: pathlib.Path, facts: Facts, runs: tuple[producer.ProducerRun, ...]
) -> tuple[pathlib.Path, ActionRun]:
    """Seal the producers' trees and run the action's steps over the result.

    Returns
    -------
    tuple[pathlib.Path, ActionRun]
        The workspace the action ran in, and what it did.
    """
    publication = producer.assemble_publication(tmp_path, facts, runs)
    assert publication.hoisted == 0, "the release's own hoist must accept the tree"
    return publication.workspace, execute(tmp_path, publication.workspace, facts)


def _refused(run: ActionRun) -> str:
    """Assert selection stopped the run, and return its diagnosis.

    Returns
    -------
    str
        The failing step's standard error.
    """
    assert len(run.completed) == 1, "selection must be the failing step"
    assert run.completed[0].returncode == 1, run.completed[0].stdout
    assert not run.gh_log, "a refused tree must not reach the network"
    return run.completed[0].stderr


def _uploads(run: ActionRun) -> list[list[str]]:
    """Return each recorded upload as its argument list.

    Returns
    -------
    list[list[str]]
        One argument list per ``gh release upload`` invocation.
    """
    return [
        line.split()
        for line in run.gh_log.splitlines()
        if line.startswith("release upload")
    ]


def _record_files(workspace: pathlib.Path) -> list[pathlib.Path]:
    """Return every producer record below a sealed tree, in path order.

    Returns
    -------
    list[pathlib.Path]
        The record paths.
    """
    return sorted((workspace / staging.DIST).rglob(staging.RECORD_NAME))


def test_every_producer_records_the_tree_its_platform_builds(
    tmp_path: pathlib.Path, facts: Facts
) -> None:
    """Record exactly the artefacts one producer staged, and nothing retained.

    The record is what publication trusts, so a producer that walked its tree
    would declare the retained ``LICENSE`` and the staged binary as release
    assets. Asserted per target rather than over the union, because the Linux
    arm globs where the others name their packages.
    """
    for run in _runs(tmp_path, facts):
        assert run.completed.returncode == 0, run.completed.stderr
        expected = sorted(staging.published_names(facts, run.target))
        assert sorted(run.claimed) == expected, (
            f"{run.target.key} must record exactly its own published names"
        )


def test_each_producer_writes_beside_what_it_records(
    tmp_path: pathlib.Path, facts: Facts
) -> None:
    """Put the record where the producer's own artifact upload carries it.

    Linux uploads ``dist`` wholesale, so a record at the ``dist`` root travels.
    The other platforms upload the staging directory and the package path, so
    a record at the ``dist`` root would be left behind -- which is why the
    step derives its own directory instead of using one for every platform.
    """
    for run in _runs(tmp_path, facts):
        assert run.completed.returncode == 0, run.completed.stderr
        if run.target.platform == "linux":
            assert run.record_dir == pathlib.Path(staging.DIST), run.record_dir
            continue
        assert run.record_dir == tree.artifact_dir(facts, run.target), (
            f"{run.target.key} must record inside the directory it uploads"
        )


def test_every_producer_record_is_bound_to_the_candidate(
    tmp_path: pathlib.Path, facts: Facts
) -> None:
    """Claim the release candidate, the run, and the target that produced it.

    A record is the only account of which bytes this candidate produced, so
    every identity field it carries must be the workflow's, not a default.
    """
    for run in _runs(tmp_path, facts):
        assert run.completed.returncode == 0, run.completed.stderr
        assert run.record["commit"] == staging.COMMIT, run.record
        assert run.record["run_id"] == staging.RUN_ID, run.record
        assert run.record["target"] == run.target.key, run.record
        assert run.record["platform"] == run.target.platform, run.record


def test_every_producer_invokes_uv_exactly_once(
    tmp_path: pathlib.Path, facts: Facts
) -> None:
    """Reach the recorder through the one ``uv`` invocation the step declares.

    The ``uv`` stand-in refuses anything but ``run --no-project --python``, so
    a recorded value is itself the evidence that the step kept that shape. What
    is left to assert is that the recorder ran once per producer and read the
    interpreter the runner supplied through ``UV_PYTHON``, rather than
    restarting ``uv`` or falling back to a bare ``python``.
    """
    for run in _runs(tmp_path, facts):
        assert run.completed.returncode == 0, run.completed.stderr
        assert run.baselines == (sys.executable,), (
            f"{run.target.key} must run the recorder once under UV_PYTHON, "
            f"got {run.baselines}"
        )


def test_an_unrecorded_platform_is_refused(
    tmp_path: pathlib.Path, facts: Facts
) -> None:
    """Refuse a platform the step has no arm for, rather than record nothing.

    The step's own ``*)`` arm is what stops a target added to the staging
    configuration from publishing artefacts no record covers. Without it the
    case would fall through with an empty asset list and be refused for the
    wrong reason.
    """
    target: Target = facts.targets[0]
    workspace = staging.stage_repository(tmp_path, name="unrecorded")
    tree.stage_producer_tree(workspace, facts, target)
    baseline_log = tmp_path / "uv-unrecorded.txt"
    baseline_log.touch()
    environment = producer_env.step_environment(facts, target) | {
        "PLATFORM": "solaris",
        "UV_BASELINE_LOG": str(baseline_log),
    }
    completed = producer.run_producer_step(workspace, environment, label="solaris")
    assert completed.returncode == 1, completed.stdout
    assert "::error title=Unrecorded platform::solaris" in completed.stdout, (
        completed.stdout
    )
    assert not _record_files(workspace), (
        "an unrecorded platform must leave no record behind"
    )


def test_the_producers_record_reaches_publication(
    tmp_path: pathlib.Path, facts: Facts
) -> None:
    """Upload exactly the declared names from a tree the producers wrote.

    This is the hand-off the issue exists for: nothing in this case stages an
    artefact, so every uploaded byte and every digest the action trusts came
    from a producer's own run.
    """
    _workspace, run = _sealed(tmp_path, facts, _runs(tmp_path, facts))
    expected = staging.declared_names(facts)
    message = run.outputs.get("error-message")
    assert run.outputs.get("upload-error") == "false", message
    assert tuple(sorted(path.name for path in run.payload.iterdir())) == expected, (
        "publication must upload exactly what the producers declared"
    )
    uploads = _uploads(run)
    assert len(uploads) == 1, run.gh_log
    assert uploads[0][2] == release_tag(facts), uploads[0][2]
    assert {pathlib.PurePosixPath(arg).name for arg in uploads[0][3:]} == set(expected)
    assert "--clobber" not in run.gh_log, "publication must never clobber"


def test_a_replaced_archive_is_refused_before_upload(
    tmp_path: pathlib.Path, facts: Facts
) -> None:
    """Refuse a hoisted archive whose bytes no longer match its record.

    The archives travel outside their producer's directory, so selection
    cannot compare them by position. It compares them by recorded digest,
    which is the only thing standing between a substituted archive and a
    release.
    """
    runs = _runs(tmp_path, facts)
    publication = producer.assemble_publication(tmp_path, facts, runs)
    archive = (
        publication.workspace
        / staging.DIST
        / staging.archive_name(facts, runs[0].target)
    )
    archive.write_bytes(b"substituted\n")
    diagnosis = _refused(execute(tmp_path, publication.workspace, facts))
    assert "holds bytes its producer did not record" in diagnosis, diagnosis


def test_a_record_that_misses_a_target_is_refused(
    tmp_path: pathlib.Path, facts: Facts
) -> None:
    """Refuse a download one producer's artifact never reached.

    A producer whose job failed uploads no artifact, so its target arrives
    with no record and the target's packages are simply absent. Publication
    must refuse that rather than publish a release missing a platform. The
    tree is sealed from every producer first, then one target's download
    directory is removed, because the release's own hoist refuses a union
    that never held every configured archive and would otherwise be the thing
    that failed.
    """
    runs = _runs(tmp_path, facts)
    publication = producer.assemble_publication(tmp_path, facts, runs)
    assert publication.hoisted == 0, "the release's own hoist must accept the tree"
    shutil.rmtree(publication.workspace / staging.DIST / runs[0].target.key)
    diagnosis = _refused(execute(tmp_path, publication.workspace, facts))
    assert "no producer record covers targets" in diagnosis, diagnosis
    assert runs[0].target.key in diagnosis, (
        f"the refusal must name the uncovered target, got {diagnosis}"
    )


def test_a_producer_from_another_commit_is_refused(
    tmp_path: pathlib.Path, facts: Facts
) -> None:
    """Refuse a tree whose records were written for an earlier candidate.

    A re-run against a moved ``main`` downloads whatever the producers of that
    run uploaded. Comparing the recorded commit is what stops those bytes
    being published under this candidate's name.
    """
    runs = _runs(tmp_path, facts)
    stale = runs[0]
    record = dict(stale.record) | {"commit": "b" * 40}
    (stale.workspace / stale.record_dir / staging.RECORD_NAME).write_text(
        json.dumps(record, indent=2) + "\n", encoding="utf-8"
    )
    _workspace, run = _sealed(tmp_path, facts, runs)
    diagnosis = _refused(run)
    assert "not candidate" in diagnosis, diagnosis


def test_a_target_the_configuration_omits_is_refused(
    tmp_path: pathlib.Path, facts: Facts
) -> None:
    """Refuse a staged target the release configuration does not declare.

    A producer for a target removed from the configuration still uploads, and
    its package is still release-shaped. Publication must refuse the surplus
    rather than attach a file for a platform the release does not serve.
    """
    workspace, _run = _sealed(tmp_path, facts, _runs(tmp_path, facts))
    extra = workspace / staging.DIST / "solaris-x86_64"
    extra.mkdir()
    (extra / "netsuke_0.1.0_solaris.deb").write_bytes(b"orphan\n")
    diagnosis = _refused(execute(tmp_path, workspace, facts))
    assert "release-shaped but not declared" in diagnosis, diagnosis


def test_a_duplicated_record_is_refused(tmp_path: pathlib.Path, facts: Facts) -> None:
    """Refuse a tree in which two records claim the same target.

    Two records for one target means two producers disagreed about what it
    built. Whichever is chosen, the other's bytes are lost, so neither may be.
    """
    workspace, _run = _sealed(tmp_path, facts, _runs(tmp_path, facts))
    first, second = _record_files(workspace)[:2]
    shutil.copy2(first, second)
    diagnosis = _refused(execute(tmp_path, workspace, facts))
    assert "more than one producer record covers targets" in diagnosis, diagnosis
