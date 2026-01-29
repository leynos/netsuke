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

  Scenario: Unsupported locale falls back to English messages
    Given the configuration locale is "fr-FR"
    And the system locale is "fr-FR"
    When the runtime localizer is built
    Then the localized message contains "not found"
