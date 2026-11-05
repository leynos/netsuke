Feature: Terminal colour policy

  Scenario: Colour never keeps verbose logs plain
    Given a minimal Netsuke workspace
    When netsuke is run with arguments "--color never --verbose generate"
    Then the command should succeed
    And stderr should contain no SGR sequence

  Scenario: Colour always styles piped verbose logs
    Given a minimal Netsuke workspace
    When netsuke is run with arguments "--color always --verbose generate"
    Then the command should succeed
    And stderr should contain an SGR sequence
