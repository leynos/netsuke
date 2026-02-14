Feature: Accessibility preferences

  Scenario: NETSUKE_NO_EMOJI disables emoji
    Given the simulated NETSUKE_NO_EMOJI is "1"
    When output preferences are resolved with no explicit setting
    Then emoji is disabled

  Scenario: NO_COLOR disables emoji
    Given the simulated NO_COLOR is "1"
    When output preferences are resolved with no explicit setting
    Then emoji is disabled

  Scenario: Explicit no-emoji overrides NO_COLOR absence
    When output preferences are resolved with no_emoji set to true
    Then emoji is disabled

  Scenario: Explicit emoji-on overrides NO_COLOR
    Given the simulated NO_COLOR is "1"
    When output preferences are resolved with no_emoji set to false
    Then emoji is enabled

  Scenario: Default allows emoji
    When output preferences are resolved with no explicit setting
    Then emoji is enabled

  Scenario: Error prefix includes text in no-emoji mode
    Given emoji is suppressed
    When the error prefix is rendered
    Then the prefix contains "Error:"
    And the prefix contains no non-ASCII characters

  Scenario: Error prefix includes emoji glyph in standard mode
    Given emoji is allowed
    When the error prefix is rendered
    Then the prefix contains "Error:"

  Scenario: Success prefix includes text in no-emoji mode
    Given emoji is suppressed
    When the success prefix is rendered
    Then the prefix contains "Success:"
    And the prefix contains no non-ASCII characters

  Scenario: Warning prefix includes text in no-emoji mode
    Given emoji is suppressed
    When the warning prefix is rendered
    Then the prefix contains "Warning:"
    And the prefix contains no non-ASCII characters

  Scenario: Warning prefix includes emoji glyph in standard mode
    Given emoji is allowed
    When the warning prefix is rendered
    Then the prefix contains "Warning:"

  Scenario: CLI parses no-emoji true
    When the CLI is parsed with "--no-emoji true"
    Then parsing succeeds
    And no emoji mode is enabled

  Scenario: CLI parses no-emoji false
    When the CLI is parsed with "--no-emoji false"
    Then parsing succeeds
    And no emoji mode is disabled
