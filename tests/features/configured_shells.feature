Feature: Trusted configured shell definitions

  Only operator configuration may define a shell. A project file, an explicit
  configuration selector, and an environment selector are all untrusted, so
  each is rejected before any manifest is compiled.

  Scenario: Operator layers append different definitions
    Given a system configuration defining shell "dash"
    And the selected user configuration extends that system configuration
    And that user configuration defines shell "custom"
    When Netsuke loads its configuration
    Then both definitions retain their fixed arguments in layer order

  Scenario: A project cannot define an interpreter
    Given a project configuration containing a shell definition
    When Netsuke loads its configuration
    Then loading fails at configuration key "shells"
    And no manifest is compiled

  Scenario: Explicit selection does not grant authority
    Given an otherwise untrusted file defining shell "custom"
    When that file is selected with --config
    Then loading fails at configuration key "shells"
    And no manifest is compiled

  Scenario: Layered names must be unique
    Given two trusted layers defining shell "dash"
    When Netsuke loads its configuration
    Then loading fails at the duplicate shell name
    And no manifest is compiled