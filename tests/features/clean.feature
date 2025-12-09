Feature: Clean subcommand execution

  Scenario: Clean invokes ninja with tool flag
    Given a fake ninja executable that expects the clean tool
    And the CLI is parsed with "clean"
    And the CLI uses the temporary directory
    When the clean process is run
    Then the command should succeed

  Scenario: Clean fails when ninja fails
    Given a fake ninja executable that exits with 1
    And the CLI is parsed with "clean"
    And the CLI uses the temporary directory
    When the clean process is run
    Then the command should fail with error "ninja exited"

  Scenario: Clean respects working directory flag
    Given a fake ninja executable that expects the clean tool
    And the CLI is parsed with "-C work clean"
    And the CLI uses the temporary directory
    When the clean process is run
    Then the command should succeed

  Scenario: Clean respects jobs flag
    Given a fake ninja executable that expects the clean tool
    And the CLI is parsed with "-j 4 clean"
    And the CLI uses the temporary directory
    When the clean process is run
    Then the command should succeed
