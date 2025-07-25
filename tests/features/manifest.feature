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

  Scenario: Manifest with rules parses correctly
    When the manifest file "tests/data/rules.yml" is parsed
    Then the first rule name is "compile"
    And the first target name is "hello.o"

  Scenario: Unknown field fails to parse
    When the manifest file "tests/data/unknown_field.yml" is parsed
    Then parsing the manifest fails

  Scenario: Invalid version fails to parse
    When the manifest file "tests/data/invalid_version.yml" is parsed
    Then parsing the manifest fails

  Scenario: Missing recipe fails to parse
    When the manifest file "tests/data/missing_recipe.yml" is parsed
    Then parsing the manifest fails
