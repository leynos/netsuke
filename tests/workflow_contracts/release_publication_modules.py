"""Keep release-publication source, suite, and boundary ownership explicit.

The Makefile contract compares its arguments with ``SECTION_MODULES`` and
``SOURCE_MODULES``. The policy boundary contract partitions every source into
``POLICY_MODULES`` or ``ADAPTER_MODULES`` so adding a source requires an
explicit decision about which side owns its effects.
"""

#: Test suites collected by ``make test-release-publication``.
SECTION_MODULES = (
    "scripts/tests/test_release_candidate.py",
    "scripts/tests/test_release_candidate_cli.py",
    "scripts/tests/test_package_crate_cli.py",
    "scripts/tests/test_package_crate_cli_errors.py",
    "scripts/tests/test_publish_crates_io_cli.py",
    "scripts/tests/test_verify_release_inventory_cli.py",
    "scripts/tests/test_verify_release_inventory_command.py",
    "scripts/tests/test_finalize_github_release_cli.py",
    "scripts/tests/test_release_inventory.py",
    "scripts/tests/test_release_identity_record.py",
    "scripts/tests/test_release_semver.py",
    "scripts/tests/test_release_digest.py",
    "scripts/tests/test_release_digest_files.py",
    "scripts/tests/test_verify_crates_io_bytes.py",
    "scripts/tests/test_release_crates_io_adapter.py",
    "scripts/tests/test_release_github_assets_adapter.py",
    "scripts/tests/test_release_reconcile_crates.py",
    "scripts/tests/test_release_reconcile_crates_properties.py",
    "scripts/tests/test_release_reconcile_github.py",
    "scripts/tests/test_release_declaration.py",
    "scripts/tests/test_release_declaration_cli.py",
    "scripts/tests/test_release_digest_record.py",
    "scripts/tests/test_release_digest_record_files.py",
    "scripts/tests/test_release_staged_digests.py",
)

#: Source modules named by ``--doctest-modules`` in the Makefile target.
SOURCE_MODULES = (
    "scripts/release_candidate.py",
    "scripts/release_candidate_cli.py",
    "scripts/release_identity_record.py",
    "scripts/package_crate_cli.py",
    "scripts/publish_crates_io_cli.py",
    "scripts/release_crates_io_adapter.py",
    "scripts/release_github_assets_adapter.py",
    "scripts/verify_release_inventory_cli.py",
    "scripts/finalize_github_release_cli.py",
    "scripts/release_inventory.py",
    "scripts/release_semver.py",
    "scripts/release_digest.py",
    "scripts/release_digest_files.py",
    "scripts/verify_crates_io_bytes.py",
    "scripts/release_registry_response.py",
    "scripts/release_reconcile_crates.py",
    "scripts/release_reconcile_github.py",
    "scripts/release_asset_names.py",
    "scripts/release_package_formats.py",
    "scripts/release_declaration.py",
    "scripts/release_declaration_cli.py",
    "scripts/release_digest_record.py",
    "scripts/release_digest_record_files.py",
    "scripts/release_staged_digests.py",
)

#: Sources that decide from already parsed, in-memory values.
POLICY_MODULES = (
    "scripts/release_candidate.py",
    "scripts/release_identity_record.py",
    "scripts/release_semver.py",
    "scripts/release_asset_names.py",
    "scripts/release_package_formats.py",
    "scripts/release_declaration.py",
    "scripts/release_inventory.py",
    "scripts/release_digest.py",
    "scripts/release_digest_record.py",
    "scripts/release_reconcile_crates.py",
    "scripts/release_reconcile_github.py",
    "scripts/verify_crates_io_bytes.py",
)

#: Sources that parse formats, touch files, or compose policy at the CLI edge.
ADAPTER_MODULES = (
    "scripts/release_candidate_cli.py",
    "scripts/package_crate_cli.py",
    "scripts/publish_crates_io_cli.py",
    "scripts/release_crates_io_adapter.py",
    "scripts/verify_release_inventory_cli.py",
    "scripts/finalize_github_release_cli.py",
    "scripts/release_digest_files.py",
    "scripts/release_registry_response.py",
    "scripts/release_declaration_cli.py",
    "scripts/release_github_assets_adapter.py",
    "scripts/release_digest_record_files.py",
    "scripts/release_staged_digests.py",
)


__all__ = [
    "ADAPTER_MODULES",
    "POLICY_MODULES",
    "SECTION_MODULES",
    "SOURCE_MODULES",
]
