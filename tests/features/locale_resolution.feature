Feature: Locale resolution

  Scenario: CLI locale overrides environment and system defaults
    Given the system locale is "es_ES.UTF-8"
    And the environment locale is "fr-FR"
    When the startup locale is resolved for "--locale en-US"
    Then the resolved locale is "en-US"

  Scenario: Environment locale is used when no CLI locale is provided
    Given the system locale is "es_ES.UTF-8"
    And the environment locale is "fr-FR"
    When the startup locale is resolved for ""
    Then the resolved locale is "fr-FR"

  Scenario: Invalid CLI locale falls back to the environment locale
    Given the system locale is "en_US.UTF-8"
    And the environment locale is "fr-FR"
    When the startup locale is resolved for "--locale @@@"
    Then the resolved locale is "fr-FR"

  Scenario: Invalid environment locale falls back to system defaults
    Given the system locale is "en_US.UTF-8"
    And the environment locale is "bad locale"
    When the startup locale is resolved for ""
    Then the resolved locale is "en-US"

  Scenario: Configuration locale is used for runtime when no overrides exist
    Given the configuration locale is "es-ES"
    And the system locale is "en_US"
    When the runtime locale is resolved
    Then the resolved locale is "es-ES"

  Scenario: Invalid configuration locale falls back to system defaults
    Given the configuration locale is "bad locale"
    And the system locale is "en_US.UTF-8"
    When the runtime locale is resolved
    Then the resolved locale is "en-US"

  Scenario: No valid locale available returns nothing
    Given the configuration locale is "bad locale"
    And the system locale is "also bad"
    When the runtime locale is resolved
    Then no locale is resolved

  Scenario: Unsupported locale falls back to English messages
    Given the configuration locale is "fr-FR"
    And the system locale is "fr-FR"
    When the runtime localiser is built
    Then the localised message contains "not found"
