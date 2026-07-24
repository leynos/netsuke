Feature: CLI parsing

  Background:
    Given an isolated CLI environment

  Scenario: Build is the default command
    When the CLI is parsed with ""
    Then parsing succeeds
    And the command is build

  Scenario: Clean command runs
    When the CLI is parsed with "-C work clean"
    Then parsing succeeds
    And the command is clean
    And the working directory is "work"

  Scenario: Graph command with jobs
    When the CLI is parsed with "-j 2 graph"
    Then parsing succeeds
    And the command is graph
    And the job count is 2

  Scenario: Graph command accepts --output
    When the CLI is parsed with "graph --output build.dot"
    Then parsing succeeds
    And the command is graph
    And the graph output path is "build.dot"

  Scenario: Graph command --output accepts stdout sentinel
    When the CLI is parsed with "graph --output -"
    Then parsing succeeds
    And the command is graph
    And the graph output path is "-"

  Scenario: Graph command accepts --html
    When the CLI is parsed with "graph --html"
    Then parsing succeeds
    And the command is graph
    And the graph html flag is set

  Scenario: Graph command accepts --html with --output
    When the CLI is parsed with "graph --html --output graph.html"
    Then parsing succeeds
    And the command is graph
    And the graph html flag is set
    And the graph output path is "graph.html"

  Scenario: Manifest file can be overridden
    When the CLI is parsed with "--file alt.yml build target"
    Then parsing succeeds
    And the manifest path is "alt.yml"
    And the first target is "target"

  Scenario: Generate command writes Ninja file
    When the CLI is parsed with "generate --output out.ninja"
    Then parsing succeeds
    And the command is generate
    And the generate output path is "out.ninja"

  Scenario: Generate subcommand streams to stdout by default
    When the CLI is parsed with "generate"
    Then parsing succeeds
    And the command is generate

  Scenario: Unknown command fails
    When the CLI is parsed with invalid arguments "unknown"
    Then an error should be returned
    And the error message should contain "Unknown subcommand"

  Scenario: Unknown command is localised in Spanish
    When the CLI is parsed with invalid arguments "--locale es-ES unknown"
    Then an error should be returned
    And the error message should contain "Subcomando desconocido"

  Scenario: Missing file argument value
    When the CLI is parsed with invalid arguments "--file"
    Then an error should be returned
    And the error message should contain "--file"

  Scenario: Directory flag sets working directory
    When the CLI is parsed with "-C work build"
    Then parsing succeeds
    And the working directory is "work"

  Scenario: Jobs flag sets parallelism
    When the CLI is parsed with "-j 4"
    Then parsing succeeds
    And the job count is 4

  Scenario: Missing directory argument value
    When the CLI is parsed with invalid arguments "-C"
    Then an error should be returned
    And the error message should contain "--directory"

  Scenario: Missing jobs argument value
    When the CLI is parsed with invalid arguments "-j"
    Then an error should be returned
    And the error message should contain "--jobs"

  Scenario: Non-numeric jobs value
    When the CLI is parsed with invalid arguments "-j notanumber"
    Then an error should be returned
    And the error message should contain "notanumber"

  Scenario: Invalid emoji value fails validation
    When the CLI is parsed with invalid arguments "--emoji neon"
    Then an error should be returned
    And the error message should contain "Invalid emoji policy 'neon'"

  Scenario: Invalid emoji value is localised in Spanish
    When the CLI is parsed with invalid arguments "--locale es-ES --emoji neon"
    Then an error should be returned
    And the error message should contain "Política de emoji no válida 'neon'"

  Scenario: Blocklist overrides allowlist for network policy flags
    When the CLI is parsed with "--fetch-allow-host example.com --fetch-block-host example.com"
    Then parsing succeeds
    And the CLI network policy rejects "https://example.com" with "blocked by policy"

  Scenario: CLI parses single-quoted argument with space
    When the CLI is parsed with "--file 'my manifest.yml' generate"
    Then parsing succeeds
    And the command is generate
    And the manifest path is "my manifest.yml"

  Scenario: CLI parses double-quoted argument with space
    When the CLI is parsed with '--file "my manifest.yml" generate'
    Then parsing succeeds
    And the command is generate
    And the manifest path is "my manifest.yml"

  Scenario: CLI parses argument with escaped space
    When the CLI is parsed with "--file my\ manifest.yml generate"
    Then parsing succeeds
    And the command is generate
    And the manifest path is "my manifest.yml"

  Scenario: CLI parses multiple single-quoted arguments
    When the CLI is parsed with "--directory 'work dir' --file 'other.yml' build"
    Then parsing succeeds
    And the command is build
    And the working directory is "work dir"
