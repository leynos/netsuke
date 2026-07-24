Feature: Advanced usage workflows

  # --- Generate subcommand ---

  Scenario: Generate subcommand streams to stdout
    Given a minimal Netsuke workspace
    When netsuke is run with arguments "generate"
    Then the command should succeed
    And stdout should contain "rule "

  Scenario: Generate subcommand writes to file
    Given a minimal Netsuke workspace
    And a directory named "output" exists
    When the netsuke generate subcommand is run with "output/build.ninja"
    Then the command should succeed
    And the file "output/build.ninja" should exist

  # --- Clean subcommand ---

  Scenario: Clean without manifest reports missing manifest
    Given an empty workspace
    When netsuke is run with arguments "clean"
    Then the command should fail
    And stderr should contain "Manifest"

  # --- Graph subcommand ---

  Scenario: Graph without manifest reports missing manifest
    Given an empty workspace
    When netsuke is run with arguments "graph"
    Then the command should fail
    And stderr should contain "Manifest"

  Scenario: Graph with invalid manifest reports parse error
    Given an empty workspace
    When netsuke is run with arguments "--json graph"
    Then the command should fail
    And stderr should be valid diagnostics json
    And stdout should be empty

  # --- Configuration layering ---

  Scenario: Invalid config value reports validation error
    Given a minimal Netsuke workspace
    And a workspace with config file setting color to loud
    When netsuke is run with arguments "generate"
    Then the command should fail
    And stderr should contain "loud"

  # --- JSON diagnostics ---

  Scenario: JSON diagnostics on error
    Given an empty workspace
    When netsuke is run with arguments "--json build"
    Then the command should fail
    And stderr should be valid diagnostics json
    And stdout should be empty

  Scenario: JSON diagnostics with generate subcommand
    Given a minimal Netsuke workspace
    When netsuke is run with arguments "--json generate"
    Then the command should succeed
    And stdout should contain "rule "
    And stderr should be empty
