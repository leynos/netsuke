@unix
Feature: Unix stdlib filters
  Scenario: realpath resolves symlinks
    Given a stdlib workspace
    When I render template "{{ path | realpath }}" at stdlib path "link"
    Then the stdlib output matches the workspace path "file"
