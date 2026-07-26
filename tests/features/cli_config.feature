Feature: Canonical CLI policy flags

  Background:
    Given an isolated CLI environment

  Scenario: Color policy flag is parsed
    When the CLI is parsed with "--color always"
    Then parsing succeeds
    And the color policy is "always"

  Scenario: Emoji policy flag is parsed
    When the CLI is parsed with "--emoji never"
    Then parsing succeeds
    And the emoji policy is "never"

  Scenario: Progress policy flag is parsed
    When the CLI is parsed with "--progress never"
    Then parsing succeeds
    And the progress policy is "never"
    And progress resolution is disabled

  Scenario: Accessibility policy flag is parsed
    When the CLI is parsed with "--accessibility on"
    Then parsing succeeds
    And the accessibility policy is "on"

  Scenario: JSON flag is parsed
    When the CLI is parsed with "--json"
    Then parsing succeeds
    And JSON output is enabled

  Scenario: Default targets flag is parsed
    When the CLI is parsed with "--default-target lint --default-target test"
    Then parsing succeeds
    And the default targets are "lint, test"

  Scenario: Invalid color policy value fails validation
    When the CLI is parsed with invalid arguments "--color loud"
    Then an error should be returned
    And the localized error contains "Invalid color policy 'loud'"

  Scenario: Invalid emoji policy value fails validation
    When the CLI is parsed with invalid arguments "--emoji sometimes"
    Then an error should be returned
    And the localized error contains "Invalid emoji policy 'sometimes'"

  Scenario: Invalid progress policy value fails validation
    When the CLI is parsed with invalid arguments "--progress paused"
    Then an error should be returned
    And the localized error contains "Invalid progress policy 'paused'"

  Scenario: Invalid accessibility policy value fails validation
    When the CLI is parsed with invalid arguments "--accessibility yes"
    Then an error should be returned
    And the localized error contains "Invalid accessibility policy 'yes'"
