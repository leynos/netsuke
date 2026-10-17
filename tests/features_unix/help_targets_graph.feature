@unix
Feature: Hermetic help-target graph export

  Scenario: Graph export preserves raw recipes and edges without Ninja
    Given no ninja executable is available
    And a Netsuke workspace with graph export edges
    When the netsuke help targets graph export is run
    Then the command should succeed
    And the JSON help-target graph should contain raw recipes and edge kinds
