Feature: Progress output

  Scenario: Standard mode shows six stage summaries
    Given a minimal Netsuke workspace
    When netsuke is run with arguments "--accessible false --progress true manifest -"
    Then the command should succeed
    And stderr should contain "Stage 1/6"
    And stderr should contain "Stage 6/6"
    And stderr should contain "Manifest complete."

  Scenario: Stage summaries localize to Spanish
    Given a minimal Netsuke workspace
    When netsuke is run with arguments "--accessible false --locale es-ES --progress true manifest -"
    Then the command should succeed
    And stderr should contain "Etapa 1/6"
    And stderr should contain "Etapa 6/6"
    And stderr should contain "Manifiesto completo."

  Scenario: Accessible mode still uses static stage labels
    Given a minimal Netsuke workspace
    When netsuke is run with arguments "--accessible true --progress true manifest -"
    Then the command should succeed
    And stderr should contain "Stage 1/6"
    And stderr should contain "Stage 6/6"

  Scenario: Progress output can be disabled in standard mode
    Given a minimal Netsuke workspace
    When netsuke is run with arguments "--accessible false --progress false manifest -"
    Then the command should succeed
    And stderr should not contain "Stage 1/6"

  Scenario: Failed runs mark the active stage as failed
    Given an empty workspace
    When netsuke is run with arguments "--accessible false --progress true"
    Then the command should fail
    And stderr should contain "Stage 1/6"
    And stderr should contain "failed"
