Feature: JSON diagnostics mode

  Scenario: JSON diagnostics report a missing manifest without polluting stdout
    Given an empty workspace
    When netsuke is run with arguments "--json graph"
    Then the command should fail
    And stdout should be empty
    And stderr should be valid diagnostics json
    And stderr diagnostics code should be "netsuke::runner::manifest_not_found"

  Scenario: JSON mode wraps successful generate output
    Given a minimal Netsuke workspace
    When netsuke is run with arguments "--json generate"
    Then the command should succeed
    And stderr should be empty
    And stdout should be one generate result json document

  Scenario: JSON mode wraps successful clean output
    Given a minimal Netsuke workspace
    And a fake ninja executable that succeeds without output
    When netsuke is run with arguments "--json clean"
    Then the command should succeed
    And stderr should be empty
    And stdout should be one clean result json document

  Scenario: JSON diagnostics suppress verbose tracing noise
    Given an empty workspace
    When netsuke is run with arguments "--json --verbose graph"
    Then the command should fail
    And stdout should be empty
    And stderr should be valid diagnostics json
    And stderr should not contain "ERROR"
