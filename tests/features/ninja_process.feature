Feature: Ninja process execution

  Scenario: Ninja succeeds
    Given a fake ninja executable that exits with 0
    And the CLI is parsed with ""
    When the ninja process is run
    Then the command should succeed

  Scenario: Ninja fails
    Given a fake ninja executable that exits with 1
    And the CLI is parsed with ""
    When the ninja process is run
    Then the command should fail

  Scenario: Ninja missing
    Given no ninja executable is available
    And the CLI is parsed with ""
    When the ninja process is run
    Then the command should fail
