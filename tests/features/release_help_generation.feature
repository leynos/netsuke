Feature: Release help generation
  As a release maintainer,
  I want release help generated outside build.rs,
  So that man pages and Windows help are explicit release artefacts.

  Scenario: A release build generates and stages a manual page from cargo orthohelp
    Given the release help workflow files are available
    Then the build workflow installs cargo-orthohelp before generating help
    And the build workflow generates help under target/orthohelp
    And release staging declares the orthohelp manual page
    And Linux packaging consumes the staged manual page

  Scenario: A Windows release build generates and stages PowerShell MAML help
    Given the release help workflow files are available
    Then the build workflow generates PowerShell help for Windows targets
    And release staging declares the Windows PowerShell help files

  Scenario: Invalid SOURCE_DATE_EPOCH falls back to 1970-01-01
    Given the release help workflow files are available
    Then invalid SOURCE_DATE_EPOCH handling falls back to the epoch date with a warning

  Scenario: Missing generated help files fail the release-help step
    Given the release help workflow files are available
    Then missing help outputs fail with release help errors

  Scenario: Release help no longer relies on build.rs output
    Given the release help workflow files are available
    Then the workflow no longer references build.rs generated help paths
