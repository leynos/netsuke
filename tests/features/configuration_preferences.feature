Feature: Configuration preferences

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

  Scenario: Unsupported output format fails during configuration merge
    Given an empty workspace
    And the Netsuke config file sets output format to "json"
    When the CLI is parsed and merged with ""
    Then an error should be returned
    And the merge error should contain "output_format = "

  Scenario: no_emoji compatibility alias resolves to the ASCII theme
    Given a minimal Netsuke workspace
    And the Netsuke config file sets no_emoji to true
    When the CLI is parsed and merged with ""
    And merged output preferences are resolved
    And the success prefix is rendered
    Then parsing succeeds
    And the merged theme is ascii
    And the prefix contains no non-ASCII characters

  Scenario: CLI theme flag overrides configuration file
    Given an empty workspace
    And the Netsuke config file sets theme to "unicode"
    When the CLI is parsed and merged with "--theme ascii"
    Then parsing succeeds
    And the merged theme is ascii

  Scenario: CLI theme flag overrides environment variable
    Given an empty workspace
    And the NETSUKE_THEME environment variable is "unicode"
    When the CLI is parsed and merged with "--theme ascii"
    Then parsing succeeds
    And the merged theme is ascii

  Scenario: CLI theme flag has highest precedence over env and config
    Given an empty workspace
    And the Netsuke config file sets theme to "unicode"
    And the NETSUKE_THEME environment variable is "auto"
    When the CLI is parsed and merged with "--theme ascii"
    Then parsing succeeds
    And the merged theme is ascii

  Scenario: CLI colour policy flag overrides configuration file
    Given an empty workspace
    And the Netsuke config file sets colour policy to "never"
    When the CLI is parsed and merged with "--colour-policy always"
    Then parsing succeeds
    And the merged colour policy is always

  Scenario: CLI colour policy flag overrides environment variable
    Given an empty workspace
    And the NETSUKE_COLOUR_POLICY environment variable is "never"
    When the CLI is parsed and merged with "--colour-policy always"
    Then parsing succeeds
    And the merged colour policy is always

  Scenario: CLI spinner mode flag overrides configuration file
    Given an empty workspace
    And the Netsuke config file sets spinner mode to "disabled"
    When the CLI is parsed and merged with "--spinner-mode enabled"
    Then parsing succeeds
    And the merged spinner mode is enabled

  Scenario: CLI spinner mode flag overrides environment variable
    Given an empty workspace
    And the NETSUKE_SPINNER_MODE environment variable is "disabled"
    When the CLI is parsed and merged with "--spinner-mode enabled"
    Then parsing succeeds
    And the merged spinner mode is enabled

  Scenario: Environment variable overrides configuration for theme
    Given an empty workspace
    And the Netsuke config file sets theme to "ascii"
    And the NETSUKE_THEME environment variable is "unicode"
    When the CLI is parsed and merged with ""
    Then parsing succeeds
    And the merged theme is unicode

  Scenario: Environment variable overrides configuration for colour policy
    Given an empty workspace
    And the Netsuke config file sets colour policy to "auto"
    And the NETSUKE_COLOUR_POLICY environment variable is "always"
    When the CLI is parsed and merged with ""
    Then parsing succeeds
    And the merged colour policy is always
