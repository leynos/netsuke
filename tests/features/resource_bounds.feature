Feature: Bounded materialization for standard-library helpers

  Scenario: Over-ceiling combinatorial request is rejected naming cardinality and ceiling
    Given a product request over operand lengths 316 and 317
    When the request is admitted
    Then admission is refused with cardinality 100172 and ceiling 100000

  Scenario: An unrepresentable cardinality reports overflow rather than a count
    Given a product request over operand lengths 4294967296 and 4294967296
    When the request is admitted
    Then admission is refused as overflow against ceiling 100000

  Scenario: Permutations are checked against the lower ceiling
    Given a permutations request choosing 8 of 8 items
    When the request is admitted
    Then admission is refused with cardinality 40320 and ceiling 10000