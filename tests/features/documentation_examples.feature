Feature: User-facing documentation examples

  Scenario: README first-run example builds successfully
    Given a workspace from documentation example "readme-first-build-manifest"
    And a fake ninja executable that emits task status lines and builds hello.txt
    When netsuke is run without arguments
    Then the command should succeed
    And stderr should contain "Stage 6/6"
    And stderr should contain "Build complete."
    And the file "hello.txt" should exist
    And the documentation file "hello.txt" should contain "Hello from Netsuke!"

  Scenario: User's guide first-run example builds successfully
    Given a workspace from documentation example "guide-first-build-manifest"
    And a fake ninja executable that emits task status lines and builds hello.txt
    When netsuke is run without arguments
    Then the command should succeed
    And stderr should contain "Stage 6/6"
    And stderr should contain "Build complete."
    And the file "hello.txt" should exist
    And the documentation file "hello.txt" should contain "Hello from Netsuke!"
