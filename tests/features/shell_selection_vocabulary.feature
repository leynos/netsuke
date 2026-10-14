Feature: Structured-command shell selection vocabulary
  A structured command's shell selector records the author's intent without
  resolving or executing a shell.

  Scenario Outline: A Boolean or a valid name decodes to the author's intent
    Given the shell selector YAML <yaml>
    When the shell selector is decoded
    Then the shell selection is <intent>

    Examples:
      | yaml   | intent                  |
      | false  | direct execution        |
      | true   | the platform default    |
      | bash   | the named shell "bash"  |
      | dash   | the named shell "dash"  |
      | "bash" | the named shell "bash"  |

  Scenario Outline: Any other selector is rejected
    Given the shell selector YAML <yaml>
    When the shell selector is decoded
    Then decoding the shell selector fails

    Examples:
      | yaml      |
      | 1         |
      | null      |
      | [bash]    |
      | /bin/bash |
      | Bash      |
      | ""        |
      | "true"    |
      | "yes"     |
