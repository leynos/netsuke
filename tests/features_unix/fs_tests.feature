# Unix-specific file tests
Feature: File-system tests
  Scenario: file system tests detect path types
    Given a file-type test workspace
    When the manifest file "tests/data/jinja_is.yml" is parsed
    And the manifest has targets named "is-dir, is-file, is-symlink, is-pipe, is-block-device, is-char-device, is-device"

  Scenario: file system tests return false for missing paths
    Given a file-type test workspace
    And the environment variable "DIR_PATH" is set to "${WORKSPACE}/__missing__/dir"
    And the environment variable "FILE_PATH" is set to "${WORKSPACE}/__missing__/file"
    And the environment variable "SYMLINK_PATH" is set to "${WORKSPACE}/__missing__/symlink"
    And the environment variable "PIPE_PATH" is set to "${WORKSPACE}/__missing__/pipe"
    And the environment variable "BLOCK_DEVICE_PATH" is set to "${WORKSPACE}/__missing__/block"
    And the environment variable "CHAR_DEVICE_PATH" is set to "${WORKSPACE}/__missing__/char"
    And the environment variable "DEVICE_PATH" is set to "${WORKSPACE}/__missing__/device"
    When the manifest file "tests/data/jinja_is.yml" is parsed
    Then the manifest has targets 0
