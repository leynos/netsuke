Feature: Progress output

  Scenario: Standard mode reports task updates from Ninja status lines
    Given a minimal Netsuke workspace
    And a fake ninja executable that emits task status lines
    When netsuke is run with arguments "--accessible false --progress true build"
    Then the command should succeed
    And stderr should contain "Task 1/2"
    And stderr should contain "Task 2/2"

  Scenario: Accessible mode emits textual task updates
    Given a minimal Netsuke workspace
    And a fake ninja executable that emits task status lines
    When netsuke is run with arguments "--accessible true --progress true build"
    Then the command should succeed
    And stderr should contain "Task 1/2"
    And stderr should contain "Task 2/2"

  Scenario: Malformed Ninja status lines are ignored safely
    Given a minimal Netsuke workspace
    And a fake ninja executable that emits malformed task status lines
    When netsuke is run with arguments "--accessible false --progress true build"
    Then the command should succeed
    And stderr should not contain "Task 1/"

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
    And a fake ninja executable that emits task status lines
    When netsuke is run with arguments "--accessible false --progress false build"
    Then the command should succeed
    And stderr should not contain "Stage 1/6"
    And stderr should not contain "Task 1/2"

  Scenario: Progress output can be disabled in accessible mode
    Given a minimal Netsuke workspace
    And a fake ninja executable that emits task status lines
    When netsuke is run with arguments "--accessible true --progress false build"
    Then the command should succeed
    And stderr should not contain "Stage 1/6"
    And stderr should not contain "Task 1/2"

  Scenario: Failed runs mark the active stage as failed
    Given an empty workspace
    When netsuke is run with arguments "--accessible false --progress true"
    Then the command should fail
    And stderr should contain "Stage 1/6"
    And stderr should contain "failed"
