Feature: Options-carrying manifest loading

  The loader offers the same manifest two capability surfaces. The build
  surface keeps every helper; the test surface withholds the ones that reach
  the host. These scenarios pin the two ends of that contract: a manifest that
  needs nothing outside itself loads identically in both, and one that needs a
  process fails under test with a refusal that names what it wanted.

  Scenario: A pure manifest loads identically in build and test modes
    Given the manifest file "tests/data/manifest_load_options/boolean_interpolation.yml"
    And a fixed clock at "2026-06-08T12:00:00Z"
    When the manifest is loaded in build mode and in test mode
    Then both loads produce the same Ninja file

  Scenario: Test mode refuses a process helper
    Given the manifest file "tests/data/manifest_load_options/uses_shell.yml"
    When the manifest is loaded in test mode
    Then parsing the manifest fails
    And the error message contains "[netsuke::jinja::test_mode::unavailable]"
    And the error message contains "shell"