Feature: Missing manifest error handling

  Scenario: Running netsuke in directory without Netsukefile shows helpful error
    Given an empty workspace
    When netsuke is run without arguments
    Then the command should fail
    And stderr should contain "No `Netsukefile` found"
    And stderr should contain "netsuke --help"

  Scenario: Running netsuke with custom manifest path that does not exist
    Given an empty workspace
    When netsuke is run with arguments "--file nonexistent.yml"
    Then the command should fail
    And stderr should contain "No `nonexistent.yml` found"

  Scenario: Running netsuke in specified directory without manifest
    Given an empty workspace
    When netsuke is run with directory flag pointing to the workspace
    Then the command should fail
    And stderr should contain "No `Netsukefile` found"
