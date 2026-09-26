Feature: Step error propagation

  # Regression guard for the `rstest-bdd` 0.6.0 step-result change, described
  # under "Audit step return aliases" in
  # `docs/rstest-bdd-v0-6-0-migration-guide.md`: a step that aliases
  # `Result<T, E>` and returns `Err` must fail its scenario. Before 0.6.0 the
  # error was boxed as an unused payload and the scenario still passed.
  #
  # Every other scenario in this suite walks a green path whose steps return
  # `Ok`, so none of them would notice if that propagation regressed. This
  # scenario fails unless the error surfaces. It therefore lives outside
  # `tests/features`, which `scenarios!` sweeps in `tests/bdd_tests.rs`: a
  # deliberately failing scenario must not be collected as an ordinary one.

  Scenario: An error returned by a step fails its scenario
    Given a step that always returns an error
    Then this step must never run
