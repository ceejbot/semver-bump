//! Integration tests that exercise the built `semver-bump` binary end to end:
//! argument parsing, stdout output, and error / exit-code behavior. These complement
//! the unit tests in `src/main.rs`, which cover the bump logic directly.

use assert_cmd::Command;
use predicates::str::contains;

fn semver_bump() -> Command {
    Command::cargo_bin("semver-bump").expect("the semver-bump binary should be built")
}

#[test]
fn patch_writes_bumped_version_to_stdout() {
    semver_bump()
        .args(["patch", "1.2.3"])
        .assert()
        .success()
        .stdout("1.2.4\n");
}

#[test]
fn major_strips_prerelease_and_build_metadata() {
    semver_bump()
        .args(["major", "1.2.3-alpha.1+build.5"])
        .assert()
        .success()
        .stdout("2.0.0\n");
}

#[test]
fn prerelease_increments_trailing_number() {
    semver_bump()
        .args(["prerelease", "1.2.3-alpha.4"])
        .assert()
        .success()
        .stdout("1.2.3-alpha.5\n");
}

#[test]
fn prerelease_replacement_preserves_build_metadata() {
    semver_bump()
        .args(["prerelease", "1.2.3-four+4", "beta.2"])
        .assert()
        .success()
        .stdout("1.2.3-beta.2+4\n");
}

#[test]
fn build_increments_trailing_number() {
    semver_bump()
        .args(["build", "1.0.3-rc.2+build-4"])
        .assert()
        .success()
        .stdout("1.0.3-rc.2+build-5\n");
}

#[test]
fn missing_prerelease_suffix_is_a_usage_error() {
    semver_bump()
        .args(["prerelease", "1.0.0"])
        .assert()
        .failure()
        .stderr(contains("does not have a prerelease suffix"));
}

#[test]
fn invalid_version_input_fails() {
    semver_bump().args(["patch", "not-a-version"]).assert().failure();
}

#[test]
fn missing_subcommand_prints_usage_and_fails() {
    semver_bump().assert().failure().stderr(contains("Usage"));
}
