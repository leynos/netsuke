@unix
Feature: Graph subcommand execution

  The `graph` subcommand renders the build graph in-process; it no longer
  spawns `ninja -t graph`. These scenarios assert the dispatch and CLI
  parsing for `--output` and `--html`. End-to-end DOT content is covered by
  `tests/runner_graph_tests.rs`.

  Scenario: Graph runs in-process and succeeds without ninja
    Given no ninja executable is available
    And the CLI is parsed with "graph --output -"
    And the CLI uses the temporary directory
    When the graph process is run
    Then the command should succeed

  Scenario: Graph writes DOT to the specified file
    Given no ninja executable is available
    And the CLI is parsed with "graph --output graph.dot"
    And the CLI uses the temporary directory
    When the graph process is run
    Then the command should succeed

  Scenario: Graph HTML contains well-formed SVG
    Given no ninja executable is available
    And the CLI is parsed with "graph --html --output graph.html"
    And the CLI uses the temporary directory
    When the graph process is run
    Then the command should succeed
    And the graph HTML file "graph.html" should contain well-formed SVG
