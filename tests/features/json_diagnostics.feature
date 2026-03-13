Feature: JSON diagnostics mode

  Scenario: JSON diagnostics report a missing manifest without polluting stdout
    Given an empty workspace
    When netsuke is run with arguments "--diag-json graph"
    Then the command should fail
    And stdout should be empty
    And stderr should be valid diagnostics json
    And stderr diagnostics code should be "netsuke::runner::manifest_not_found"

  Scenario: JSON diagnostics keep successful manifest output on stdout
    Given a minimal Netsuke workspace
    When netsuke is run with arguments "--diag-json manifest -"
    Then the command should succeed
    And stderr should be empty
    And stdout should contain "build hello: "

  Scenario: JSON diagnostics suppress verbose tracing noise
    Given an empty workspace
    When netsuke is run with arguments "--diag-json --verbose graph"
    Then the command should fail
    And stdout should be empty
    And stderr should be valid diagnostics json
    And stderr should not contain "ERROR"
