Feature: Graph subcommand execution

  Scenario: Graph invokes ninja with tool flag
    Given a fake ninja executable that expects the graph tool
    And the CLI is parsed with "graph"
    And the CLI uses the temporary directory
    When the graph process is run
    Then the command should succeed

  Scenario: Graph fails when ninja fails
    Given a fake ninja executable that exits with 1
    And the CLI is parsed with "graph"
    And the CLI uses the temporary directory
    When the graph process is run
    Then the command should fail with error "ninja exited"

  Scenario: Graph respects jobs flag
    Given a fake ninja executable that expects graph with 4 jobs
    And the CLI is parsed with "-j 4 graph"
    And the CLI uses the temporary directory
    When the graph process is run
    Then the command should succeed

  Scenario: Graph fails when ninja is missing
    Given no ninja executable is available
    And the CLI is parsed with "graph"
    When the graph process is run
    Then the command should fail with error "No such file or directory"
