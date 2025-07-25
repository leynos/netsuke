Feature: Manifest parsing

  Scenario: Parse minimal manifest
    When the manifest file "tests/data/minimal.yml" is parsed
    Then the manifest version is "1.0.0"
    And the first target name is "hello"

  Scenario: Parse phony and always flags
    When the manifest file "tests/data/phony.yml" is parsed
    Then the first target is phony
    And the first target is always rebuilt

  Scenario: Actions are always treated as phony
    When the manifest file "tests/data/actions.yml" is parsed
    Then the first action is phony

  Scenario: Invalid action fails to parse
    When the manifest file "tests/data/action_invalid.yml" is parsed
    Then parsing the manifest fails
