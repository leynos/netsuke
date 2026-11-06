Feature: Terminal rendering on a pseudo-terminal

  Background:
    Given the "TERM" environment variable is "xterm-256color"

  Scenario: Accessible mode never redraws on a terminal
    Given a minimal Netsuke workspace
    When netsuke is run on a terminal with arguments "--accessibility on generate"
    Then the command should succeed
    And the terminal output should contain no redraw sequence
    And the terminal output should contain "Stage 6/6"

  Scenario: Standard mode draws live progress on a terminal
    Given a minimal Netsuke workspace
    When netsuke is run on a terminal with arguments "--accessibility off generate"
    Then the command should succeed
    And the terminal output should contain a redraw sequence
