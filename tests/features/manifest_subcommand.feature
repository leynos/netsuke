Feature: Manifest subcommand

  Scenario: Manifest subcommand writes a Ninja file without invoking Ninja
    Given a minimal Netsuke workspace
    When the netsuke manifest subcommand is run with "out.ninja"
    Then the command should succeed
    And the file "out.ninja" should exist

  Scenario: Manifest subcommand streams a Ninja file to stdout
    Given a minimal Netsuke workspace
    When the netsuke manifest subcommand is run with "-"
    Then the command should succeed
    And stdout should contain "rule "
    And the file "-" should not exist

  Scenario: Manifest-time conditions select generated actions and targets
    Given a Netsuke workspace with conditional actions and targets
    When the netsuke manifest subcommand is run with "-"
    Then the command should succeed
    And stdout should contain "build action-kept:"
    And stdout should contain "build target-kept:"
    And stdout should not contain "action-skipped"
    And stdout should not contain "target-skipped"

  Scenario: Command availability selects the preferred top-level action
    Given a Netsuke workspace with a preferred command available
    When the netsuke manifest subcommand is run with "-"
    Then the command should succeed
    And stdout should contain "build preferred-action:"
    And stdout should not contain "fallback-action"

  Scenario: Manifest subcommand fails when output path is a directory
    Given a minimal Netsuke workspace
    And a directory named "out.ninja" exists
    When the netsuke manifest subcommand is run with "out.ninja"
    Then the command should fail
    And stderr should contain "Failed to create Ninja file"
