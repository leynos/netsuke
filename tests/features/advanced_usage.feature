Feature: Advanced usage workflows

  Scenario: Manifest subcommand streams to stdout
    Given a minimal Netsuke workspace
    When netsuke is run with arguments "manifest -"
    Then the command should succeed
    And stdout should contain "rule "

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
