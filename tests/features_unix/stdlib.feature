@unix
Feature: Unix stdlib filters
  Scenario: realpath resolves symlinks
    Given a stdlib workspace
    When I render "{{ path | realpath }}" with stdlib path "link"
    Then the stdlib output is the workspace path "file"
