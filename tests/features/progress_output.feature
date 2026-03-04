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

  Scenario: Verbose mode includes a completion timing summary
    Given a minimal Netsuke workspace
    When netsuke is run with arguments "--accessible false --progress true --verbose manifest -"
    Then the command should succeed
    And stderr should contain "Stage timing summary:"
    And stderr should contain "- Stage 1/6:"
    And stderr should contain "Total pipeline time:"

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

  Scenario: Failed verbose runs suppress timing summary lines
    Given an empty workspace
    When netsuke is run with arguments "--accessible false --progress true --verbose"
    Then the command should fail
    And stderr should not contain "Stage timing summary:"
    And stderr should not contain "Total pipeline time:"

  Scenario: Non-verbose runs omit completion timing summaries
    Given a minimal Netsuke workspace
    When netsuke is run with arguments "--accessible false --progress true manifest -"
    Then the command should succeed
    And stderr should not contain "Stage timing summary:"
    And stderr should not contain "Total pipeline time:"

  # Stream separation tests (roadmap 3.10.1)
  # These scenarios use stable machine markers (NINJA_STDOUT_MARKER, NINJA_STDERR_MARKER)
  # to verify stream separation without coupling to localized UI strings.

  Scenario: Subprocess stdout is separate from status messages
    Given a minimal Netsuke workspace
    And a fake ninja executable that emits stdout output
    When netsuke is run with arguments "--accessible true --progress true build"
    Then the command should succeed
    # Verify stdout contains only subprocess output markers
    And stdout should contain "NINJA_STDOUT_MARKER_LINE_1"
    And stdout should contain "NINJA_STDOUT_MARKER_LINE_2"
    And stdout should contain "NINJA_STDOUT_MARKER_LINE_1" before "NINJA_STDOUT_MARKER_LINE_2"
    # Verify stderr contains the subprocess stderr marker (stream routing)
    And stderr should contain "NINJA_STDERR_MARKER"
    # Verify task-progress messages reach stderr (not raw ninja [1/2] tokens)
    And stderr should contain "Task 1/2"
    # Verify stream exclusivity using stable markers
    And stdout should not contain "NINJA_STDERR_MARKER"
    # Verify formatted task-progress messages don't leak to stdout
    And stdout should not contain "Task 1/2"
    And stderr should not contain "NINJA_STDOUT_MARKER_LINE"

  Scenario: Status messages do not contaminate stdout in standard mode
    Given a minimal Netsuke workspace
    And a fake ninja executable that emits stdout output
    When netsuke is run with arguments "--accessible false --progress true build"
    Then the command should succeed
    # Verify subprocess stdout reaches stdout
    And stdout should contain "NINJA_STDOUT_MARKER_LINE_1"
    # Verify subprocess stderr marker reaches stderr (not stdout)
    And stderr should contain "NINJA_STDERR_MARKER"
    # Verify task-progress messages reach stderr
    And stderr should contain "Task 1/2"
    And stdout should not contain "NINJA_STDERR_MARKER"
    # Verify formatted task-progress messages don't leak to stdout
    And stdout should not contain "Task 1/2"
    And stderr should not contain "NINJA_STDOUT_MARKER_LINE"

  Scenario: Build artifacts can be captured via stdout redirection
    Given a minimal Netsuke workspace
    When netsuke is run with arguments "--progress true manifest -"
    Then the command should succeed
    # Verify manifest output goes to stdout
    And stdout should contain "rule "
    # Verify stdout is not contaminated with status by checking it lacks
    # typical status patterns (numeric progress indicators)
    And stdout should not contain "1/6"
