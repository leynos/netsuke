"""Pinned installer contract for the actionlint workflow linter.

The Linux CI job downloads actionlint through a reviewed installer script at a
fixed revision, feeding it an archive whose SHA-256 the job verifies itself.
These constants spell out that script line by line so a silent change to the
version, endpoint, or checksum fails a test rather than shipping.
``github_actions_validation_test.py`` consumes them; this module holds no
tests of its own.

Run via ``make test-workflow-contracts``.
"""


def shell_variable(name: str) -> str:
    """Return a shell variable expansion for script contract expectations."""
    return f"${{{name}}}"


ACTIONLINT_VERSION = "1.7.12"
ACTIONLINT_SHA256 = "8aca8db96f1b94770f1b0d72b6dddcb1ebb8123cb3712530b08cc387b349a3d8"
ACTIONLINT_INSTALLER_COMMIT = "914e7df21a07ef503a81201c76d2b11c789d3fca"
ACTIONLINT_ARCHIVE = (
    f"actionlint_{shell_variable('ACTIONLINT_VERSION')}_linux_amd64.tar.gz"
)
ACTIONLINT_RAW_BASE = "https://raw.githubusercontent.com/rhysd/actionlint"
ACTIONLINT_SCRIPT = "scripts/download-actionlint.bash"
ACTIONLINT_RELEASE_ROOT = "https://github.com/rhysd/actionlint/releases/download"

ACTIONLINT_INSTALL_COMMAND = (
    f'bash "{shell_variable("ACTIONLINT_INSTALLER_PATH")}" '
    f'"{shell_variable("ACTIONLINT_VERSION")}"'
)
ACTIONLINT_CHECKSUM_COMMAND = (
    f"printf '%s  %s\\n' \"{shell_variable('ACTIONLINT_SHA256')}\" "
    f'"{shell_variable("ACTIONLINT_ARCHIVE_PATH")}" | sha256sum --check --'
)
ACTIONLINT_SCRIPT_CONTRACTS = (
    (
        f"readonly ACTIONLINT_VERSION='{ACTIONLINT_VERSION}'",
        "the actionlint installer must pin the expected release version",
    ),
    (
        f"readonly ACTIONLINT_SHA256='{ACTIONLINT_SHA256}'",
        "the actionlint installer must pin the expected release archive checksum",
    ),
    (
        f"readonly ACTIONLINT_INSTALLER_COMMIT='{ACTIONLINT_INSTALLER_COMMIT}'",
        "the actionlint installer must pin its reviewed installer revision",
    ),
    (
        f'readonly ACTIONLINT_ARCHIVE="{ACTIONLINT_ARCHIVE}"',
        "the actionlint installer must request the published Linux amd64 archive",
    ),
    (
        f"readonly ACTIONLINT_RAW_BASE='{ACTIONLINT_RAW_BASE}'",
        "the actionlint installer must own its immutable raw-content endpoint",
    ),
    (
        f"readonly ACTIONLINT_SCRIPT='{ACTIONLINT_SCRIPT}'",
        "the actionlint installer must pin its downloader script path",
    ),
    (
        (
            'readonly ACTIONLINT_INSTALLER_URL="'
            f"{shell_variable('ACTIONLINT_RAW_BASE')}/"
            f"{shell_variable('ACTIONLINT_INSTALLER_COMMIT')}/"
            f'{shell_variable("ACTIONLINT_SCRIPT")}"'
        ),
        "the actionlint installer URL must be constructed from its pinned inputs",
    ),
    (
        f"readonly ACTIONLINT_RELEASE_ROOT='{ACTIONLINT_RELEASE_ROOT}'",
        "the actionlint installer must own its release endpoint",
    ),
    (
        (
            'readonly ACTIONLINT_RELEASE_BASE="'
            f"{shell_variable('ACTIONLINT_RELEASE_ROOT')}/"
            f'v{shell_variable("ACTIONLINT_VERSION")}"'
        ),
        "the actionlint release base must select the pinned version",
    ),
    (
        (
            'readonly ACTIONLINT_RELEASE_URL="'
            f"{shell_variable('ACTIONLINT_RELEASE_BASE')}/"
            f'{shell_variable("ACTIONLINT_ARCHIVE")}"'
        ),
        ("the actionlint release URL must be constructed from the pinned archive"),
    ),
    (
        (
            "command curl --fail --location --show-error --output "
            f'"{shell_variable("ACTIONLINT_INSTALLER_PATH")}" \\\n'
            f'  "{shell_variable("ACTIONLINT_INSTALLER_URL")}"'
        ),
        "the actionlint installer download must use the installer endpoint",
    ),
    (
        (
            "command curl --fail --location --show-error --output "
            f'"{shell_variable("ACTIONLINT_ARCHIVE_PATH")}" \\\n'
            f'  "{shell_variable("ACTIONLINT_RELEASE_URL")}"'
        ),
        "the actionlint archive download must use the release endpoint",
    ),
    (
        ACTIONLINT_CHECKSUM_COMMAND,
        "the actionlint archive checksum must verify the downloaded archive",
    ),
)
