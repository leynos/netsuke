Feature: Canonical value identity

  Scenario: Reordered maps have the same canonical key
    Given two string-keyed maps with identical members in different orders
    When their canonical keys are derived
    Then the keys are equal

  Scenario: Deduplication keeps the first equal value
    Given a materialized sequence with repeated canonical values
    When stable deduplication is applied
    Then the original first values appear once in first-appearance order

  Scenario: Unsupported values fail before coercion
    Given a value outside the canonical JSON domain
    When its canonical key is requested
    Then a typed error identifies its value kind
