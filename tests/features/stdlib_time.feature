Feature: Template time helpers
  Netsuke exposes deterministic time primitives so manifests can reason about
  file ages without shelling out to external tools.

  Scenario: Rendering now() yields a UTC timestamp
    Given a stdlib workspace
    When I render the stdlib template "{{ now() }}"
    Then the stdlib output is an ISO8601 UTC timestamp

  Scenario: Rendering now() with an offset preserves the offset
    Given a stdlib workspace
    When I render the stdlib template "{{ now(offset='+02:00').iso8601 }}"
    Then the stdlib output offset is "+02:00"

  Scenario: Timedelta composes multiple components
    Given a stdlib workspace
    When I render the stdlib template "{{ timedelta(days=1, hours=2, minutes=30, seconds=5, milliseconds=750, microseconds=250).iso8601 }}"
    Then the stdlib output is "P1DT2H30M5.75025S"

  Scenario: Timedelta captures nanosecond precision
    Given a stdlib workspace
    When I render the stdlib template "{{ timedelta(nanoseconds=1).iso8601 }}"
    Then the stdlib output is "PT0.000000001S"

  Scenario: Timedelta supports negative values
    Given a stdlib workspace
    When I render the stdlib template "{{ timedelta(hours=-1).iso8601 }}"
    Then the stdlib output is "-PT1H"

  Scenario: Timedelta overflow surfaces an error
    Given a stdlib workspace
    When I render the stdlib template "{{ timedelta(days=9223372036854775807) }}"
    Then the stdlib error contains "overflow"

  Scenario: now() rejects invalid offsets
    Given a stdlib workspace
    When I render the stdlib template "{{ now(offset='bogus') }}"
    Then the stdlib error contains "invalid"
