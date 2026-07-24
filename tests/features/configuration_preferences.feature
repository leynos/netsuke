Feature: Canonical configuration preferences

  Scenario: Configured build targets become the default build command targets
    Given a minimal Netsuke workspace
    And the Netsuke config file sets build targets to "hello"
    When the CLI is parsed and merged with ""
    Then parsing succeeds
    And the merged CLI uses build target "hello"

  Scenario: CLI locale and verbose flags override configuration and environment
    Given an empty workspace
    And the Netsuke config file sets locale to "es-ES"
    And the NETSUKE_LOCALE environment variable is "fr-FR"
    When the CLI is parsed and merged with "--locale en-US --verbose"
    Then parsing succeeds
    And the merged locale is "en-US"
    And verbose mode is enabled in the merged CLI

  Scenario: CLI color policy overrides configuration
    Given an empty workspace
    And the Netsuke config file sets color to "never"
    When the CLI is parsed and merged with "--color always"
    Then parsing succeeds
    And the merged color policy is "always"

  Scenario: Environment emoji policy overrides configuration
    Given an empty workspace
    And the Netsuke config file sets emoji to "never"
    And the "NETSUKE_EMOJI" environment variable is "always"
    When the CLI is parsed and merged with ""
    Then parsing succeeds
    And the merged emoji policy is "always"

  Scenario: CLI progress policy overrides configuration
    Given an empty workspace
    And the Netsuke config file sets progress to "never"
    When the CLI is parsed and merged with "--progress always"
    Then parsing succeeds
    And the merged progress policy is "always"

  Scenario: CLI accessibility policy overrides configuration
    Given an empty workspace
    And the Netsuke config file sets accessibility to "off"
    When the CLI is parsed and merged with "--accessibility on"
    Then parsing succeeds
    And the merged accessibility policy is "on"

  Scenario: Interactive configuration is rejected
    Given an empty workspace
    And the Netsuke config file disables no-input
    When the CLI is parsed and merged with ""
    Then an error should be returned
    And the merge error should contain "no_input = false is unsupported"
