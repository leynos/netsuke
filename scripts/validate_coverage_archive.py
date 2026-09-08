#!/usr/bin/env python3
"""Validate and safely materialize a hostile LCOV artifact ZIP archive.

The trusted workflow downloads an untrusted artifact without decompression.
This command validates the archive directory and ZIP metadata before writing
the sole validated ``lcov.info`` member to a trusted output directory.
"""

import argparse
import importlib.util
import pathlib
import sys
import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    import types

REPOSITORY_ROOT = pathlib.Path(__file__).resolve().parents[1]
VALIDATOR_PATH = REPOSITORY_ROOT / "scripts" / "validate_coverage_artifact.py"
ARCHIVE_HELPER_PATH = REPOSITORY_ROOT / "scripts" / "coverage_artifact_archive.py"


class ArchiveCommandError(ValueError):
    """Describe hostile archive data rejected by the archive command."""


def _load_error(path: pathlib.Path) -> RuntimeError:
    """Build one explicit checked-in module loading error."""
    return RuntimeError(f"unable to load {path.name}")


def _load_module(name: str, path: pathlib.Path) -> types.ModuleType:
    """Load one checked-in archive-validation module through an explicit seam."""
    specification = importlib.util.spec_from_file_location(name, path)
    if specification is None or specification.loader is None:
        raise _load_error(path)
    module = importlib.util.module_from_spec(specification)
    sys.modules[specification.name] = module
    specification.loader.exec_module(module)
    return module


def _sole_archive_path(
    members: list[pathlib.Path], helper: types.ModuleType
) -> pathlib.Path:
    """Return the sole outer archive member or reject an unexpected directory."""
    if len(members) != 1:
        raise helper.ArchiveValidationError(helper.ArchiveIssue.MEMBERS)
    return members[0]


def validate_archive(
    archive_directory: pathlib.Path, output_directory: pathlib.Path
) -> None:
    """Validate an archive directory before materializing the coverage member.

    Parameters
    ----------
    archive_directory
        Download destination containing exactly one raw ZIP file.
    output_directory
        Empty trusted destination for the validated ``lcov.info`` data.

    Raises
    ------
    ArchiveCommandError
        If hostile outer-directory or ZIP data violates the archive contract.

    Notes
    -----
    Checks outer-directory member count, path containment, and regular
    non-link type before ZIP metadata is examined or any bytes are extracted.
    """
    validator = _load_module("validate_coverage_artifact", VALIDATOR_PATH)
    helper = _load_module("coverage_artifact_archive", ARCHIVE_HELPER_PATH)
    try:
        directory = validator._validated_directory(archive_directory)
        members = validator._bounded_directory_members(directory)
        archive_path = _sole_archive_path(members, helper)
        validator._reject_symlink_member(archive_path)
        archive_path = validator._resolve_contained_regular_member(
            archive_path, directory
        )
        contract = helper.ArchiveContract(
            validator.EXPECTED_MEMBER,
            validator.MAXIMUM_COVERAGE_BYTES,
            validator._validate_lcov_text,
        )
        helper.validate_and_materialize(archive_path, output_directory, contract)
    except (validator.ValidationError, helper.ArchiveValidationError) as error:
        raise ArchiveCommandError(str(error)) from error


def main(argv: cabc.Sequence[str] | None = None) -> int:
    """Run hostile archive validation and return its documented exit status.

    Parameters
    ----------
    argv
        Command arguments. ``None`` uses the process arguments.

    Returns
    -------
    int
        ``0`` after safe materialization, ``1`` for hostile data, or ``2``
        when the filesystem cannot be inspected.
    """
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive-dir", type=pathlib.Path, required=True)
    parser.add_argument("--output-dir", type=pathlib.Path, required=True)
    arguments = parser.parse_args(argv)
    try:
        validate_archive(arguments.archive_dir, arguments.output_dir)
    except OSError as error:
        print(f"error: unable to inspect artefact: {error}", file=sys.stderr)
        return 2
    except ArchiveCommandError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    print("ok: validated and materialized hostile LCOV artefact")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
