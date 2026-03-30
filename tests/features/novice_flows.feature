Feature: Novice CLI flows

  Scenario: First run succeeds in a minimal workspace
    Given a minimal Netsuke workspace
    And a fake ninja executable that emits task status lines
    When netsuke is run without arguments
    Then the command should succeed
    And stderr should contain "Stage 6/6"
    And stderr should contain "Task 1/2"
    And stderr should contain "Build complete."

  Scenario: Missing manifest shows guided failure
    Given an empty workspace
    When netsuke is run without arguments
    Then the command should fail
    And stderr should contain "Manifest 'Netsukefile' not found in the current directory."
    And stderr should contain "Ensure the manifest exists or pass `--file` with the correct path."

  Scenario: Help flag output matches the documented journey
    Given an empty workspace
    When netsuke is run with arguments "--help"
    Then the command should succeed
    And stdout should contain "build     Build targets defined in the manifest (default)."
    And stdout should contain "clean     Remove build artefacts via Ninja."
    And stdout should contain "graph     Emit the dependency graph in DOT format."
    And stdout should contain "manifest  Write the generated Ninja manifest without running Ninja."

  Scenario: Help subcommand output matches the flag form
    Given an empty workspace
    When netsuke is run with arguments "help"
    Then the command should succeed
    And stdout should contain "build     Build targets defined in the manifest (default)."
    And stdout should contain "clean     Remove build artefacts via Ninja."
    And stdout should contain "graph     Emit the dependency graph in DOT format."
    And stdout should contain "manifest  Write the generated Ninja manifest without running Ninja."
