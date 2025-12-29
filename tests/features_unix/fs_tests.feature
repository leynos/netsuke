# Unix-specific file tests
@unix
Feature: File-system tests
  Scenario: file system tests detect path types
    Given a file-type test workspace
    When the manifest file "tests/data/jinja_is.yml" is parsed
    And the manifest has targets named "is-dir, is-file, is-symlink, is-pipe, is-block-device, is-char-device, is-device"

  Scenario: file system tests return false for missing paths
    Given a file-type test workspace
    And the environment variable "MISSING_DIR_PATH" is set to "${WORKSPACE}/.missing/dir"
    And the environment variable "MISSING_FILE_PATH" is set to "${WORKSPACE}/.missing/file"
    And the environment variable "MISSING_SYMLINK_PATH" is set to "${WORKSPACE}/.missing/symlink"
    And the environment variable "MISSING_PIPE_PATH" is set to "${WORKSPACE}/.missing/pipe"
    And the environment variable "MISSING_BLOCK_DEVICE_PATH" is set to "${WORKSPACE}/.missing/block"
    And the environment variable "MISSING_CHAR_DEVICE_PATH" is set to "${WORKSPACE}/.missing/char"
    And the environment variable "MISSING_DEVICE_PATH" is set to "${WORKSPACE}/.missing/device"
    When the manifest file "tests/data/jinja_is_missing.yml" is parsed
    Then the manifest has 0 targets
