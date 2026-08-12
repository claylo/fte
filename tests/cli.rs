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
fn extraction_success_exits_zero() {
    fte()
        .args(["--stdout", "tests/golden/input/bmc-short.html"])
        .assert()
        .success()
        .stdout(predicate::str::contains("---"));
}

#[test]
fn missing_input_id_exits_nonzero() {
    fte()
        .args([
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
        .args(["--indir", "tests/golden/input", "--outdir"])
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
        .args(["-q", "--force", "--indir", "tests/golden/input", "--outdir"])
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
        .args(["-v", "--force", "--indir", "tests/golden/input", "--outdir"])
        .arg(tmp.path())
        .arg("bmc-short")
        .assert()
        .success()
        .stderr(predicate::str::contains("detect bmc-short:"));
}
