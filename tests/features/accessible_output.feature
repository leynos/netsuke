Feature: Accessible output mode

  Scenario: Accessible mode is auto-detected from TERM=dumb
    Given the simulated TERM is "dumb"
    When the output mode is resolved with no explicit setting
    Then the output mode is accessible

  Scenario: Accessible mode is auto-detected from NO_COLOR
    Given the simulated NO_COLOR is "1"
    When the output mode is resolved with no explicit setting
    Then the output mode is accessible

  Scenario: Explicit accessible flag overrides TERM
    Given the simulated TERM is "xterm-256color"
    When the output mode is resolved with accessible set to true
    Then the output mode is accessible

  Scenario: Explicit non-accessible overrides NO_COLOR
    Given the simulated NO_COLOR is "1"
    When the output mode is resolved with accessible set to false
    Then the output mode is standard

  Scenario: Default output mode is standard
    When the output mode is resolved with no explicit setting
    Then the output mode is standard

  Scenario: CLI parses accessible true
    When the CLI is parsed with "--accessible true"
    Then parsing succeeds
    And accessible mode is enabled

  Scenario: CLI parses accessible false
    When the CLI is parsed with "--accessible false"
    Then parsing succeeds
    And accessible mode is disabled
