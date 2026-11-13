"""Lay out the tree one release producer holds when it records.

A producer's record is only meaningful if it was written over the files that
producer really built, in the directory the release later uploads from. The
names themselves belong to ``upload_declared_assets_staging.py``, which every
case shares; what this module adds is *where* each one sits while a producer
runs -- the staging directory the shared action creates, and the ``dist`` root
where the Linux glob and the macOS stamp step put their packages.

Run via ``make test-workflow-contracts``.
"""

import pathlib

import upload_declared_assets_staging as staging
from upload_declared_assets_staging import Facts, Target


def artifact_dir(facts: Facts, target: Target) -> pathlib.Path:
    """Return the directory the staging action hands the producer.

    The name reproduces the configuration's ``staging_dir_template``, which the
    shared staging action expands, so the producer's record lands where the
    workflow uploads it from.

    Returns
    -------
    pathlib.Path
        ``dist/<staging directory>`` for ``target``.
    """
    return pathlib.Path(staging.DIST) / staging.staging_directory(facts, target)


def package_paths(facts: Facts, target: Target) -> tuple[pathlib.Path, ...]:
    """Return the packages one target's producer stages, at the ``dist`` root.

    Every packaging action in the chain writes into ``dist`` and reports the
    file it made, so the operator's report and the tree agree even for the
    macOS package its stamp step renamed.

    Returns
    -------
    tuple[pathlib.Path, ...]
        One path per package format the target's platform stages.
    """
    return tuple(
        pathlib.Path(staging.DIST) / name
        for name in staging.package_names(facts, target)
    )


def stage_paths(facts: Facts, target: Target) -> dict[str, str]:
    """Return the step-output values the producer's ``env`` block names.

    Returns
    -------
    dict[str, str]
        ``artifact-dir``, ``binstall-archive-path``, and ``msi-path`` as the
        staging and packaging actions would report them.
    """
    directory = artifact_dir(facts, target)
    archive = staging.archive_name(facts, target)
    return {
        "artifact-dir": str(directory),
        "binstall-archive-path": str(directory / archive),
        # The Windows arm's output, which the producer reads only on Windows;
        # every platform reports *a* value so the expression resolver has one.
        "msi-path": str(package_paths(facts, target)[0]),
    }


def record_location(target: Target, facts: Facts) -> pathlib.Path:
    """Return where the step writes its record, relative to the workspace.

    The Linux arm writes beside the packages it globs, at the ``dist`` root;
    the Windows and macOS arms write into the directory the staging action
    reports. That difference is the step's own, so it is derived here the way
    the step derives it rather than assumed uniform.

    Returns
    -------
    pathlib.Path
        The directory the record is expected in.
    """
    if target.platform == "linux":
        return pathlib.Path(staging.DIST)
    return artifact_dir(facts, target)


def stage_producer_tree(workspace: pathlib.Path, facts: Facts, target: Target) -> None:
    """Lay out the tree one producer's job holds when its step runs."""
    directory = workspace / artifact_dir(facts, target)
    directory.mkdir(parents=True)
    # Retained material a checkout leaves beside the artefacts. The producer
    # names its own files rather than walking the tree, so none of this may
    # reach the record -- and the Linux arm's `dist/*.deb` glob must not reach
    # it either, which is why no retained file carries a package suffix.
    (workspace / staging.DIST / "LICENSE").write_bytes(b"retained licence\n")
    (directory / facts.bin_name).write_bytes(b"retained binary\n")
    archive = staging.archive_name(facts, target)
    content = f"{archive}\n".encode()
    (directory / archive).write_bytes(content)
    (directory / f"{archive}.sha256").write_bytes(staging.sidecar(archive, content))
    for relative in package_paths(facts, target):
        path = workspace / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(f"{path.name}\n".encode())
