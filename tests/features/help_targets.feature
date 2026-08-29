Feature: Help targets subcommand

  Scenario: Help targets prints described actions and targets
    Given a Netsuke workspace with described actions and targets
    When the netsuke help targets subcommand is run
    Then the command should succeed
    And stdout should contain "Actions:"
    And stdout should contain "Targets:"
    And stdout should contain "Run rustdoc, Clippy, and Whitaker"
    And stdout should contain "Build the optimized release binary"

  Scenario: Help targets lists build-only conditional actions
    Given a Netsuke workspace with a conditional action
    When the netsuke help targets subcommand is run
    Then the command should succeed
    And stdout should contain "Run tests with cargo-nextest"
    And stdout should contain "[◇ conditional]"
