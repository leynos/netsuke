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

  Scenario: Manifest subcommand fails when output path is a directory
    Given a minimal Netsuke workspace
    And a directory named "out.ninja" exists
    When the netsuke manifest subcommand is run with "out.ninja"
    Then the command should fail
    And stderr should contain "Failed to create Ninja file"
