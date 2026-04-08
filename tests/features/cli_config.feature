Feature: CLI config flags

  Background:
    Given an isolated CLI environment

  Scenario: Colour policy flag is parsed
    When the CLI is parsed with "--colour-policy always"
    Then parsing succeeds
    And the colour policy is "always"

  Scenario: Spinner mode flag is parsed
    When the CLI is parsed with "--spinner-mode disabled"
    Then parsing succeeds
    And the spinner mode is "disabled"
    And progress resolution is disabled

  Scenario: Output format flag is parsed
    When the CLI is parsed with "--output-format json"
    Then parsing succeeds
    And the output format is "json"
    And diagnostic JSON resolution is enabled

  Scenario: Default targets flag is parsed
    When the CLI is parsed with "--default-target lint --default-target test"
    Then parsing succeeds
    And the default targets are "lint, test"

  Scenario: Invalid colour policy value fails validation
    When the CLI is parsed with invalid arguments "--colour-policy loud"
    Then an error should be returned
    And the localized error contains "Invalid colour policy 'loud'"

  Scenario: Invalid spinner mode value fails validation
    When the CLI is parsed with invalid arguments "--spinner-mode paused"
    Then an error should be returned
    And the localized error contains "Invalid spinner mode 'paused'"

  Scenario: Invalid output format value fails validation
    When the CLI is parsed with invalid arguments "--output-format tap"
    Then an error should be returned
    And the localized error contains "Invalid output format 'tap'"
