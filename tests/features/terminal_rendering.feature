Feature: Terminal rendering policy contracts

  Scenario: Emoji never keeps status glyphs ASCII
    Given a minimal Netsuke workspace
    When netsuke is run with arguments "--emoji never --accessibility off --progress always generate"
    Then the command should succeed
    And stderr status glyphs should be ASCII
    And every stderr status line should carry a text label

  Scenario: Emoji always selects Unicode status glyphs
    Given a minimal Netsuke workspace
    When netsuke is run with arguments "--emoji always --accessibility on --progress always generate"
    Then the command should succeed
    And stderr status glyphs should be Unicode
    And every stderr status line should carry a text label

  Scenario: Emoji auto on a piped run selects Unicode glyphs
    Given a minimal Netsuke workspace
    When netsuke is run with arguments "--emoji auto --accessibility off --progress always generate"
    Then the command should succeed
    And stderr status glyphs should be Unicode
    And every stderr status line should carry a text label

  Scenario: Accessibility mode keeps status glyphs ASCII under emoji auto
    Given a minimal Netsuke workspace
    When netsuke is run with arguments "--emoji auto --accessibility on --progress always generate"
    Then the command should succeed
    And stderr status glyphs should be ASCII
    And every stderr status line should carry a text label

  Scenario: Progress never suppresses stage reporting
    Given a minimal Netsuke workspace
    When netsuke is run with arguments "--progress never generate"
    Then the command should succeed
    And stderr should not contain "Stage 1/6"
    And stdout should contain "rule"
