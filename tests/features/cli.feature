Feature: CLI parsing

  Scenario: Build is the default command
    When the CLI is parsed with ""
    Then parsing succeeds
    And the command is build

  Scenario: Manifest file can be overridden
    When the CLI is parsed with "--file alt.yml build target"
    Then parsing succeeds
    And the manifest path is "alt.yml"
    And the first target is "target"
