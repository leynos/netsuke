Feature: Template stdlib filters
  Background:
    Given a stdlib workspace

  Scenario: Rendering basename for a file path
    When I render "{{ path | basename }}" with stdlib path "file"
    Then the stdlib output is "file"

  Scenario: Dirname resolves to the workspace root
    When I render "{{ path | dirname }}" with stdlib path "file"
    Then the stdlib output equals the workspace root

  Scenario: relative_to returns the child component
    Given the stdlib file "nested/file.txt" contains "nested"
    When I render "{{ path | relative_to(path | dirname) }}" with stdlib path "nested/file.txt"
    Then the stdlib output is "file.txt"

  Scenario: with_suffix rewrites extensions
    When I render "{{ path | with_suffix('.log') }}" with stdlib path "file.tar.gz"
    Then the stdlib output is the workspace path "file.tar.log"

  Scenario: expanduser expands the home directory
    Given HOME points to the stdlib workspace root
    When I render "{{ path | expanduser }}" with stdlib path "~/workspace"
    Then the stdlib output is the workspace path "workspace"

  Scenario: contents reads a file
    When I render "{{ path | contents }}" with stdlib path "file"
    Then the stdlib output is "data"

  Scenario: linecount counts newline-delimited lines
    When I render "{{ path | linecount }}" with stdlib path "lines.txt"
    Then the stdlib output is "3"

  Scenario: size returns the byte length
    When I render "{{ path | size }}" with stdlib path "file"
    Then the stdlib output is "4"

  Scenario: Size filter reports errors for missing files
    When I render "{{ path | size }}" with stdlib path "missing"
    Then the stdlib error contains "not found"

  Scenario: hash computes the sha256 digest
    When I render "{{ path | hash('sha256') }}" with stdlib path "file"
    Then the stdlib output is "3a6eb0790f39ac87c94f3856b2dd2c5d110e6811602261a9a923d3bb23adc8b7"

  Scenario: digest truncates the hash output
    When I render "{{ path | digest(8, 'sha256') }}" with stdlib path "file"
    Then the stdlib output is "3a6eb079"

  Scenario: uniq removes duplicates
    When I render "{{ ['a', 'a', 'b'] | uniq | join(',') }}" with stdlib path "file"
    Then the stdlib output is "a,b"

  Scenario: flatten merges nested lists
    When I render "{{ [['a'], ['b', 'c']] | flatten | join(',') }}" with stdlib path "file"
    Then the stdlib output is "a,b,c"

  Scenario: group_by clusters items by attribute
    When I render "{{ ([{'name': 'one', 'kind': 'tool'}, {'name': 'two', 'kind': 'tool'}, {'name': 'three', 'kind': 'material'}] | group_by('kind')).tool | length }}" with stdlib path "file"
    Then the stdlib output is "2"

  Scenario: group_by reports errors for missing attributes
    When I render "{{ ([{'name': 'one'}] | group_by('kind')) }}" with stdlib path "file"
    Then the stdlib error contains "could not resolve"

