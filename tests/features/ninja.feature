Feature: Ninja file generation

  Scenario: Generate build statements
    When the manifest file "tests/data/rules.yml" is compiled to IR
    And the ninja file is generated
    Then the ninja file contains "rule"
    And the ninja file contains "build hello.o:"

  Scenario: Phony target runs its command
    When the manifest file "tests/data/phony.yml" is compiled to IR
    And the ninja file is generated
    Then the ninja file contains "build clean:"
    And the ninja file contains "rm -rf build"

  Scenario: Inputs and outputs are shell-quoted
    When the manifest file "tests/data/quote.yml" is compiled to IR
    And the ninja file is generated
    Then the ninja file contains "command = cat in' file' > out' file'"
