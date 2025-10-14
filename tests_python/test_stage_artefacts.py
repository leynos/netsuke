"""Tests asserting the public staging package surface remains consistent."""

from __future__ import annotations


def test_staging_package_exports_public_api(
    stage_common: object,
    staging_package: object,
    staging_pipeline: object,
    staging_output: object,
) -> None:
    """The staging package should expose the streamlined public surface."""

    assert staging_package.stage_artefacts is stage_common.stage_artefacts
    assert staging_package.StageResult is staging_pipeline.StageResult
    assert staging_package.StageResult is stage_common.StageResult
    assert staging_package.StagedArtefact is staging_pipeline.StagedArtefact
    assert staging_package.write_github_output is staging_output.write_github_output
    assert (
        staging_package.RESERVED_OUTPUT_KEYS
        == staging_output.RESERVED_OUTPUT_KEYS
    )
