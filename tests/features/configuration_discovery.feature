Feature: Configuration file discovery and precedence
  Netsuke discovers configuration files automatically from project and user scopes,
  with environment variables and CLI flags overriding discovered values.

  Scenario: Project config file is discovered and applied
    Given a temporary workspace
    And a project config file ".netsuke.toml" with emoji "always" and jobs 8
    When the CLI is parsed with "--jobs 12"
    Then parsing succeeds
    And the emoji policy is "always"
    And the jobs setting is 12

  Scenario: Environment variables override project config
    Given a temporary workspace
    And a project config file ".netsuke.toml" with emoji "never"
    And the environment variable "NETSUKE_EMOJI" is set to "always"
    When the CLI is parsed with no additional arguments
    Then parsing succeeds
    And the emoji policy is "always"

  Scenario: CLI flags override environment and config
    Given a temporary workspace
    And a project config file ".netsuke.toml" with emoji "never" and JSON false
    And the environment variable "NETSUKE_EMOJI" is set to "always"
    When the CLI is parsed with "--emoji never --json"
    Then parsing succeeds
    And the emoji policy is "never"
    And JSON output is enabled

  Scenario: List fields append across config, environment, and CLI
    Given a temporary workspace
    And a project config file ".netsuke.toml" with default targets "fmt, lint"
    And the environment variable "NETSUKE_DEFAULT_TARGETS" is set to "test"
    When the CLI is parsed with "--default-target build"
    Then parsing succeeds
    And the default targets are "fmt, lint, test, build"

  Scenario: Explicit config file overrides project discovery
    Given a temporary workspace
    And a project config file ".netsuke.toml" with emoji "never"
    And a custom config file "custom.toml" with emoji "always"
    When the CLI is parsed with "--config custom.toml"
    Then parsing succeeds
    And the emoji policy is "always"

  Scenario: NETSUKE_CONFIG environment variable selects config file
    Given a temporary workspace
    And a project config file ".netsuke.toml" with emoji "never"
    And a custom config file "override.toml" with emoji "always"
    And the environment variable "NETSUKE_CONFIG" points to "override.toml"
    When the CLI is parsed with no additional arguments
    Then parsing succeeds
    And the emoji policy is "always"

  Scenario: CLI config flag takes precedence over NETSUKE_CONFIG
    Given a temporary workspace
    And a project config file ".netsuke.toml" with emoji "never"
    And a custom config file "cli.toml" with emoji "always"
    And a custom config file "env.toml" with emoji "never"
    And the environment variable "NETSUKE_CONFIG" points to "env.toml"
    When the CLI is parsed with "--config cli.toml"
    Then parsing succeeds
    And the emoji policy is "always"
