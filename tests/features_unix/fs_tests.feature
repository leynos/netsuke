# Unix-specific file tests
Feature: File-system tests
  Scenario: file system tests detect path types
    Given a file-type test workspace
    When the manifest file "tests/data/jinja_is.yml" is parsed
    Then the manifest has targets 7

  Scenario: file system tests return false for missing paths
    Given a file-type test workspace
    And the environment variable "DIR_PATH" is set to "/no/such"
    And the environment variable "FILE_PATH" is set to "/no/such"
    And the environment variable "SYMLINK_PATH" is set to "/no/such"
    And the environment variable "PIPE_PATH" is set to "/no/such"
    And the environment variable "BLOCK_DEVICE_PATH" is set to "/no/such"
    And the environment variable "CHAR_DEVICE_PATH" is set to "/no/such"
    And the environment variable "DEVICE_PATH" is set to "/no/such"
    When the manifest file "tests/data/jinja_is.yml" is parsed
    Then the manifest has targets 0
