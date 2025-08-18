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
    Then the target 1 is phony
    And the target 1 is always rebuilt

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

  Scenario: Rendering Jinja conditionals in a manifest (disabled)
    Given the manifest file "tests/data/jinja_if_disabled.yml" is parsed
    When the manifest is checked
    Then the first target name is "hello"
    And the first target command is "echo off"

  Scenario: Generating targets with foreach
    Given the manifest file "tests/data/foreach.yml" is parsed
    When the manifest is checked
    Then the manifest has 2 targets
    And the target 1 name is "foo"
    And the target 1 command is "echo 'foo'"
    And the target 1 index is 0
    And the target 2 name is "bar"
    And the target 2 command is "echo 'bar'"
    And the target 2 index is 1

  Scenario: Parsing fails when a foreach expression is not iterable
    Given the manifest file "tests/data/foreach_invalid.yml" is parsed
    When the parsing result is checked
    Then parsing the manifest fails

  Scenario: Rendering all target fields
    Given the manifest file "tests/data/render_target.yml" is parsed
    When the manifest is checked
    Then the target 1 name is "base1"
    And the target 1 has source "base1.src"
    And the target 1 has dep "base1.dep"
    And the target 1 has order-only dep "base1.ord"
    And the target 1 command is "echo base1"
    And the target 2 script is "run base.sh"
    And the target 3 rule is "base-rule"

  Scenario Outline: Targets default flags are false
    Given the manifest file "tests/data/target_defaults.yml" is parsed
    When the manifest is checked
    Then the manifest has 3 targets
    And the target <index> is not phony
    And the target <index> is not always rebuilt

    Examples:
      | index |
      | 1     |
      | 2     |
      | 3     |

  Scenario: Parsing fails when rule and command are both defined
    Given the manifest file "tests/data/rule_command_conflict.yml" is parsed
    When the parsing result is checked
    Then parsing the manifest fails
