Feature: Advanced usage workflows

  # --- Manifest subcommand ---

  Scenario: Manifest subcommand streams to stdout
    Given a minimal Netsuke workspace
    When netsuke is run with arguments "manifest -"
    Then the command should succeed
    And stdout should contain "rule "

  Scenario: Manifest subcommand writes to file
    Given a minimal Netsuke workspace
    And a directory named "output" exists
    When the netsuke manifest subcommand is run with "output/build.ninja"
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
    When netsuke is run with arguments "--diag-json graph"
    Then the command should fail
    And stderr should be valid diagnostics json
    And stdout should be empty

  # --- Configuration layering ---

  Scenario: Invalid config value reports validation error
    Given a minimal Netsuke workspace
    And a workspace with config file setting colour_policy to loud
    When netsuke is run with arguments "manifest -"
    Then the command should fail
    And stderr should contain "loud"

  # --- JSON diagnostics ---

  Scenario: JSON diagnostics on error
    Given an empty workspace
    When netsuke is run with arguments "--diag-json build"
    Then the command should fail
    And stderr should be valid diagnostics json
    And stdout should be empty

  Scenario: JSON diagnostics with manifest subcommand
    Given a minimal Netsuke workspace
    When netsuke is run with arguments "--diag-json manifest -"
    Then the command should succeed
    And stdout should contain "rule "
    And stderr should be empty
