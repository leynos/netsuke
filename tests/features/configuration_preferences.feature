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
