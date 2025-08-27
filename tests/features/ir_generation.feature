Feature: Intermediate Representation (IR) Generation
  As a developer,
  I want to compile a manifest into a valid build graph,
  So that I can detect configuration errors before execution.

  Scenario: A new, empty BuildGraph has no content
    Given a new BuildGraph is created
    When its contents are checked
    Then the graph has 0 actions
    And the graph has 0 targets
    And the graph has 0 default targets

  Scenario: Compiling a valid manifest with one rule and one target
    Given the manifest file "tests/data/rules.yml" is compiled to IR
    When the graph contents are checked
    Then the graph has 1 actions
    And the graph has 1 targets

  Scenario: Identical rules are deduplicated during IR generation
    Given the manifest file "tests/data/duplicate_rules.yml" is compiled to IR
    When the graph contents are checked
    Then the graph has 2 actions
    And the graph has 2 targets

  Scenario: IR generation fails if a target references a rule that does not exist
    Given the manifest file "tests/data/missing_rule.yml" is compiled to IR
    When the generation result is checked
    Then IR generation fails

  Scenario: IR generation fails if a target specifies multiple rules
    Given the manifest file "tests/data/multiple_rules_per_target.yml" is compiled to IR
    When the generation result is checked
    Then IR generation fails

  Scenario: IR generation fails if multiple targets produce the same output file
    Given the manifest file "tests/data/duplicate_outputs.yml" is compiled to IR
    When the generation result is checked
    Then IR generation fails

  Scenario: IR generation fails if there is a circular dependency between targets
    Given the manifest file "tests/data/circular.yml" is compiled to IR
    When the generation result is checked
    Then IR generation fails
