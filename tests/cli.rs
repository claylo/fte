//! Binary-level tests: exit codes, failure accounting, and -q/-v behavior.
//!
//! The golden tests exercise extraction through the library; these run the
//! actual binary because exit codes and stderr chatter only exist there.

use assert_cmd::Command;
use predicates::prelude::*;

fn fte() -> Command {
    Command::cargo_bin("fte").expect("fte binary builds")
}

#[test]
fn bare_invocation_shows_help_and_exits_two() {
    fte()
        .assert()
        .code(2)
        .stderr(predicate::str::contains("extract"))
        .stderr(predicate::str::contains("split"));
}

#[test]
fn extraction_success_exits_zero() {
    fte()
        .args(["extract", "--stdout", "tests/golden/input/bmc-short.html"])
        .assert()
        .success()
        .stdout(predicate::str::contains("---"));
}

#[test]
fn missing_input_id_exits_nonzero() {
    fte()
        .args([
            "extract",
            "--indir",
            "tests/golden/input",
            "--stdout",
            "definitely-not-a-real-id",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn missing_input_is_counted_in_summary() {
    let tmp = tempfile::tempdir().unwrap();
    fte()
        .args(["extract", "--indir", "tests/golden/input", "--outdir"])
        .arg(tmp.path())
        .args(["bmc-short", "definitely-not-a-real-id"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("1 extracted"))
        .stderr(predicate::str::contains("1 failed"));
}

#[test]
fn quiet_suppresses_progress_but_not_failures() {
    let tmp = tempfile::tempdir().unwrap();
    fte()
        .args([
            "extract",
            "-q",
            "--force",
            "--indir",
            "tests/golden/input",
            "--outdir",
        ])
        .arg(tmp.path())
        .args(["bmc-short", "definitely-not-a-real-id"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"))
        .stderr(predicate::str::contains("OK").not())
        .stderr(predicate::str::contains("Done:").not());
}

#[test]
fn verbose_reports_detected_format() {
    let tmp = tempfile::tempdir().unwrap();
    fte()
        .args([
            "extract",
            "-v",
            "--force",
            "--indir",
            "tests/golden/input",
            "--outdir",
        ])
        .arg(tmp.path())
        .arg("bmc-short")
        .assert()
        .success()
        .stderr(predicate::str::contains("detect bmc-short:"));
}

#[test]
fn detect_is_its_own_command() {
    fte()
        .args(["detect", "tests/golden/input/bmc-short.html"])
        .assert()
        .success()
        .stdout(predicate::str::contains("bmc-short"));
}

#[test]
fn schema_still_works_alongside_subcommands() {
    fte()
        .arg("schema")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"clispec\""));
}
