"""Pinned installer contract for the actionlint workflow linter.

The Linux CI job installs actionlint through ``scripts/ci/install_actionlint.py``,
a Cyclopts helper that reuses the gate-cached binary when it reports the
pinned version and otherwise downloads the pinned Linux amd64 release archive,
verifies its SHA-256, and extracts the single ``actionlint`` member. The pins
live beside the step as ``INPUT_*`` environment variables so a silent change
to the version or checksum fails a test rather than shipping; the download,
verification, reuse, and failure behaviours are covered by
``scripts/tests/test_ci_install_actionlint.py``. ``github_actions_validation_test.py``
consumes these constants; this module holds no tests of its own.

Run via ``make test-workflow-contracts``.
"""

ACTIONLINT_VERSION = "1.7.12"
ACTIONLINT_SHA256 = "8aca8db96f1b94770f1b0d72b6dddcb1ebb8123cb3712530b08cc387b349a3d8"
ACTIONLINT_INSTALL_SCRIPT = "scripts/ci/install_actionlint.py"
ACTIONLINT_INSTALL_COMMAND = f"uv run --script {ACTIONLINT_INSTALL_SCRIPT}"
#: The pins the step must pass to the installer, and the concern each protects.
ACTIONLINT_STEP_ENV_CONTRACTS = (
    (
        "INPUT_ACTIONLINT_VERSION",
        ACTIONLINT_VERSION,
        "the actionlint step must pin the expected release version",
    ),
    (
        "INPUT_ACTIONLINT_SHA256",
        ACTIONLINT_SHA256,
        "the actionlint step must pin the expected release archive checksum",
    ),
)
