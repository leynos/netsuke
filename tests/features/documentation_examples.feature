Feature: User-facing documentation examples

  Scenario: README first-run example builds successfully
    Given a workspace from documentation example "readme-first-build-manifest"
    And a fake ninja executable that emits task status lines
    When netsuke is run without arguments
    Then the command should succeed
    And stderr should contain "Stage 6/6"
    And stderr should contain "Build complete."

  Scenario: User's guide first-run example builds successfully
    Given a workspace from documentation example "guide-first-build-manifest"
    And a fake ninja executable that emits task status lines
    When netsuke is run without arguments
    Then the command should succeed
    And stderr should contain "Stage 6/6"
    And stderr should contain "Build complete."
