Feature: BuildGraph

  Scenario: New BuildGraph is empty
    When a new BuildGraph is created
    Then the graph has 0 actions
    And the graph has 0 targets
    And the graph has 0 default targets

