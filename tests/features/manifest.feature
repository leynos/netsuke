Feature: Manifest Parsing
  As a user,
  I want to define my build in a YAML manifest,
  So that Netsuke can understand and execute it.

  Scenario: Parsing a minimal valid manifest
    Given the manifest file "tests/data/minimal.yml" is parsed
    When the version is checked
    Then the manifest version is "1.0.0"
    And the first target name is "hello"

  Scenario: Parsing a manifest with phony and always flags
    Given the manifest file "tests/data/phony.yml" is parsed
    When the flags are checked
    Then the first target is phony
    And the first target is always rebuilt

  Scenario: A target in the 'actions' block is implicitly phony
    Given the manifest file "tests/data/actions.yml" is parsed
    When the flags are checked
    Then the first action is phony

  Scenario: Parsing a manifest with rules
    Given the manifest file "tests/data/rules.yml" is parsed
    When the rules are checked
    Then the first rule name is "compile"
    And the first target name is "hello.o"

  Scenario: Parsing fails for a manifest with an unknown top-level field
    Given the manifest file "tests/data/unknown_field.yml" is parsed
    When the parsing result is checked
    Then parsing the manifest fails

  Scenario: Parsing fails for a manifest with an invalid version string
    Given the manifest file "tests/data/invalid_version.yml" is parsed
    When the parsing result is checked
    Then parsing the manifest fails

  Scenario: Parsing fails for a target that is missing a recipe
    Given the manifest file "tests/data/missing_recipe.yml" is parsed
    When the parsing result is checked
    Then parsing the manifest fails

  Scenario: Parsing fails for an action that is missing a recipe
    Given the manifest file "tests/data/action_invalid.yml" is parsed
    When the parsing result is checked
    Then parsing the manifest fails

  Scenario: Rendering Jinja variables in a manifest
    Given the manifest file "tests/data/jinja_vars.yml" is parsed
    When the manifest is checked
    Then the first target command is "echo world"

  Scenario: Parsing fails when a Jinja variable is undefined
    Given the manifest file "tests/data/jinja_undefined.yml" is parsed
    When the parsing result is checked
    Then parsing the manifest fails

  Scenario: Rendering Jinja conditionals in a manifest
    Given the manifest file "tests/data/jinja_if.yml" is parsed
    When the manifest is checked
    Then the first target name is "hello"
    And the first target command is "echo on"

  Scenario: Rendering Jinja loops in a manifest
    Given the manifest file "tests/data/jinja_for.yml" is parsed
    When the manifest is checked
    Then the manifest has 2 targets
    And the target 1 name is "foo"
    And the target 1 command is "echo foo"
    And the target 2 name is "bar"
    And the target 2 command is "echo bar"

  Scenario: Parsing fails when a Jinja loop iterates over a non-list
    Given the manifest file "tests/data/jinja_for_invalid.yml" is parsed
    When the parsing result is checked
    Then parsing the manifest fails
