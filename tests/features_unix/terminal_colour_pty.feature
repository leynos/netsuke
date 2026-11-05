Feature: Terminal colour policy on a pseudo-terminal

  Background:
    Given the "TERM" environment variable is "xterm-256color"

  Scenario: Colour never removes colour from terminal diagnostics
    Given an empty workspace
    When netsuke is run on a terminal with arguments "--color never generate"
    Then the command should fail
    And the terminal output should contain no SGR sequence
    And the terminal output should contain "Manifest 'Netsukefile' not found"

  Scenario: NO_COLOR removes colour from terminal logs
    Given a minimal Netsuke workspace
    And the "NO_COLOR" environment variable is "1"
    When netsuke is run on a terminal with arguments "--color auto --verbose generate"
    Then the command should succeed
    And the terminal output should contain no SGR sequence
    And the terminal output should contain "Stage 6/6"

  Scenario: Empty NO_COLOR keeps generate output live
    Given a minimal Netsuke workspace
    And the "NO_COLOR" environment variable is ""
    When netsuke is run on a terminal with arguments "--color auto generate"
    Then the command should succeed
    And the terminal output should contain a redraw sequence
    And the terminal output should contain "Stage 6/6"

  Scenario: Empty NO_COLOR permits verbose log colour
    Given a minimal Netsuke workspace
    And the "NO_COLOR" environment variable is ""
    When netsuke is run on a terminal with arguments "--color auto --verbose generate"
    Then the command should succeed
    And the terminal output should contain an SGR sequence
    And the terminal output should contain "Stage 6/6"

  Scenario: Empty NO_COLOR permits coloured help
    Given a minimal Netsuke workspace
    And the "NO_COLOR" environment variable is ""
    When netsuke is run on a terminal with arguments "--color auto --help"
    Then the command should succeed
    And the terminal output should contain an SGR sequence
    And the terminal output should contain "Usage"
