Feature: Manifest parsing

  Scenario: Parse minimal manifest
    When the manifest file "tests/data/minimal.yml" is parsed
    Then the manifest version is "1.0.0"
    And the first target name is "hello"

  Scenario: Invalid manifest version
    When the manifest file "tests/data/invalid_version.yml" is parsed
    Then manifest parsing should fail
    And the manifest error message should contain "version"
