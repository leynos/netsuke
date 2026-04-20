Feature: Configuration file discovery and precedence
  Netsuke discovers configuration files automatically from project and user scopes,
  with environment variables and CLI flags overriding discovered values.

  Scenario: Project config file is discovered and applied
    Given a temporary workspace
    And a project config file ".netsuke.toml" with theme "unicode" and jobs 8
    When the CLI is parsed with "--jobs 12"
    Then parsing succeeds
    And the theme preference is "unicode"
    And the jobs setting is 12

  Scenario: Environment variables override project config
    Given a temporary workspace
    And a project config file ".netsuke.toml" with theme "ascii"
    And the environment variable "NETSUKE_THEME" is set to "unicode"
    When the CLI is parsed with no additional arguments
    Then parsing succeeds
    And the theme preference is "unicode"

  Scenario: CLI flags override environment and config
    Given a temporary workspace
    And a project config file ".netsuke.toml" with theme "ascii" and output format "human"
    And the environment variable "NETSUKE_THEME" is set to "unicode"
    When the CLI is parsed with "--theme ascii --output-format json"
    Then parsing succeeds
    And the theme preference is "ascii"
    And the output format is "json"

  Scenario: List fields append across config, environment, and CLI
    Given a temporary workspace
    And a project config file ".netsuke.toml" with default targets "fmt, lint"
    And the environment variable "NETSUKE_DEFAULT_TARGETS" is set to "test"
    When the CLI is parsed with "--default-target build"
    Then parsing succeeds
    And the default targets are "fmt, lint, test, build"

  Scenario: NETSUKE_CONFIG_PATH overrides automatic discovery
    Given a temporary workspace
    And a project config file ".netsuke.toml" with theme "ascii"
    And a custom config file "custom.toml" with theme "unicode"
    And the environment variable "NETSUKE_CONFIG_PATH" points to "custom.toml"
    When the CLI is parsed with no additional arguments
    Then parsing succeeds
    And the theme preference is "unicode"

  Scenario: Explicit config file overrides project discovery
    Given a temporary workspace
    And a project config file ".netsuke.toml" with theme "ascii"
    And a custom config file "custom.toml" with theme "unicode"
    When the CLI is parsed with "--config custom.toml"
    Then parsing succeeds
    And the theme preference is "unicode"

  Scenario: NETSUKE_CONFIG environment variable selects config file
    Given a temporary workspace
    And a project config file ".netsuke.toml" with theme "ascii"
    And a custom config file "override.toml" with theme "unicode"
    And the environment variable "NETSUKE_CONFIG" points to "override.toml"
    When the CLI is parsed with no additional arguments
    Then parsing succeeds
    And the theme preference is "unicode"

  Scenario: NETSUKE_CONFIG takes precedence over NETSUKE_CONFIG_PATH
    Given a temporary workspace
    And a project config file ".netsuke.toml" with theme "ascii"
    And a custom config file "new.toml" with theme "unicode"
    And a custom config file "legacy.toml" with theme "ascii"
    And the environment variable "NETSUKE_CONFIG" points to "new.toml"
    And the environment variable "NETSUKE_CONFIG_PATH" points to "legacy.toml"
    When the CLI is parsed with no additional arguments
    Then parsing succeeds
    And the theme preference is "unicode"

  Scenario: CLI config flag takes precedence over NETSUKE_CONFIG
    Given a temporary workspace
    And a project config file ".netsuke.toml" with theme "ascii"
    And a custom config file "cli.toml" with theme "unicode"
    And a custom config file "env.toml" with theme "ascii"
    And the environment variable "NETSUKE_CONFIG" points to "env.toml"
    When the CLI is parsed with "--config cli.toml"
    Then parsing succeeds
    And the theme preference is "unicode"
