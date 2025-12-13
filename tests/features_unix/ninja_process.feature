@unix
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
    Then the command should fail with error "ninja exited with exit status: 1"

  Scenario: Ninja missing
    Given no ninja executable is available
    And the CLI is parsed with ""
    When the ninja process is run
    Then the command should fail with error "No such file or directory"

  Scenario: Build file missing
    Given a fake ninja executable that checks for the build file
    And the CLI is parsed with ""
    And the CLI uses the temporary directory
    When the ninja process is run
    Then the command should fail with error "ninja exited with exit status: 1"

  Scenario: Build file is not a regular file
    Given a fake ninja executable that checks for the build file
    And the CLI is parsed with ""
    And the CLI uses the temporary directory
    And a directory named build.ninja exists
    When the ninja process is run
    Then the command should fail with error "ninja exited with exit status: 1"
