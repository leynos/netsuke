Feature: CLI help output

  Scenario: Display help
    Given netsuke is built
    When I run netsuke --help
    Then the process exits successfully
    And stdout contains "Usage"
