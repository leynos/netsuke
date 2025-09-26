Feature: Template stdlib filters
  Scenario: Rendering basename for a file path
    Given a stdlib workspace
    When I render "{{ path | basename }}" with stdlib path "file"
    Then the stdlib output is "file"

  Scenario: Size filter reports errors for missing files
    Given a stdlib workspace
    When I render "{{ path | size }}" with stdlib path "missing"
    Then the stdlib error contains "not found"
