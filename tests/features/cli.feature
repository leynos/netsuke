Feature: CLI parsing

  Scenario: Build is the default command
    When the CLI is parsed with ""
    Then parsing succeeds
    And the command is build

  Scenario: Clean command runs
    When the CLI is parsed with "-C work clean"
    Then parsing succeeds
    And the command is clean
    And the working directory is "work"

  Scenario: Graph command with jobs
    When the CLI is parsed with "-j 2 graph"
    Then parsing succeeds
    And the command is graph
    And the job count is 2

  Scenario: Manifest file can be overridden
    When the CLI is parsed with "--file alt.yml build target"
    Then parsing succeeds
    And the manifest path is "alt.yml"
    And the first target is "target"

  Scenario: Unknown command fails
    When the CLI is parsed with invalid arguments "unknown"
    Then an error should be returned
    And the error message should contain "unknown"

  Scenario: Missing file argument value
    When the CLI is parsed with invalid arguments "--file"
    Then an error should be returned
    And the error message should contain "--file"

  Scenario: Directory flag sets working directory
    When the CLI is parsed with "-C work build"
    Then parsing succeeds
    And the working directory is "work"

  Scenario: Jobs flag sets parallelism
    When the CLI is parsed with "-j 4"
    Then parsing succeeds
    And the job count is 4

  Scenario: Missing directory argument value
    When the CLI is parsed with invalid arguments "-C"
    Then an error should be returned
    And the error message should contain "--directory"

  Scenario: Missing jobs argument value
    When the CLI is parsed with invalid arguments "-j"
    Then an error should be returned
    And the error message should contain "--jobs"

  Scenario: Non-numeric jobs value
    When the CLI is parsed with invalid arguments "-j notanumber"
    Then an error should be returned
    And the error message should contain "notanumber"
