Feature: BuildGraph

  Scenario: New BuildGraph is empty
    When a new BuildGraph is created
    Then the graph has 0 actions
    And the graph has 0 targets
    And the graph has 0 default targets


  Scenario: BuildGraph from manifest
    When the manifest file "tests/data/rules.yml" is compiled to IR
    Then the graph has 1 actions
    And the graph has 1 targets

  Scenario: Duplicate rules are deduplicated
    When the manifest file "tests/data/duplicate_rules.yml" is compiled to IR
    Then the graph has 2 actions
    And the graph has 2 targets

  Scenario: Rule not found during IR generation
    When the manifest file "tests/data/missing_rule.yml" is compiled to IR
    Then IR generation fails

  Scenario: Multiple rules specified for target
    When the manifest file "tests/data/multiple_rules_per_target.yml" is compiled to IR
    Then IR generation fails

  Scenario: Duplicate target outputs
    When the manifest file "tests/data/duplicate_outputs.yml" is compiled to IR
    Then IR generation fails

  Scenario: Circular dependency detection
    When the manifest file "tests/data/circular.yml" is compiled to IR
    Then IR generation fails
