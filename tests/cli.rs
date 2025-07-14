//! Unit tests for CLI argument parsing.

use assert_cmd::Command;
use predicates::prelude::*;
use rstest::rstest;

#[rstest]
#[case::default_invocation(vec!["netsuke"])]
#[case::targets(vec!["netsuke", "build", "foo", "bar"])]
#[case::file_and_jobs(vec!["netsuke", "--file", "Custom", "-j", "4"])]
fn cli_runs(#[case] args: Vec<&str>) {
    let mut cmd = Command::cargo_bin("netsuke").expect("binary exists");
    cmd.args(args.iter().skip(1)).assert().success();
}

#[test]
fn cli_help() {
    let mut cmd = Command::cargo_bin("netsuke").expect("binary exists");
    cmd.arg("--help")
        .assert()
        .stdout(predicate::str::contains("Usage"));
}
