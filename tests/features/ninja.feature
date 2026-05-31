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
    Then shlex splitting the command yields "cat, in file, >, out file"

  Scenario: Edge-case paths are shell-quoted
    When the manifest file "tests/data/quote.yml" is compiled to IR
    And the ninja file is generated
    Then shlex splitting command 3 yields "printf, %s, -in file, >, o'utfile"

  Scenario: Missing action is reported
    When the manifest file "tests/data/rules.yml" is compiled to IR
    And an action is removed from the graph
    And the ninja file is generated
    Then ninja generation fails mentioning the removed action id

  Scenario: Target and action deps become implicit Ninja dependencies
    When the manifest file "tests/data/implicit_deps.yml" is compiled to IR
    Then the graph target "out/app" has inputs "src/main.c"
    And the graph target "out/app" has implicit deps "include/config.h, generated/stamp"
    And the graph target "regenerate" has inputs ""
    And the graph target "regenerate" has implicit deps "schemas/user.yml, tools/generator"
    When the ninja file is generated
    Then the ninja file contains "build out/app: "
    And the ninja file contains " src/main.c | include/config.h generated/stamp"
    And the ninja file contains "build regenerate: "
    And the ninja file contains " | schemas/user.yml tools/generator"
    And the ninja file contains "command = echo src/main.c src/main.c > out/app"
