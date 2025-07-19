Feature: Manifest parsing

  Scenario: Parse minimal manifest
    When the manifest file "tests/data/minimal.yml" is parsed
    Then the manifest version is "1.0"
    And the first target name is "hello"
