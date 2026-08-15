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

#[test]
fn a_missing_input_exits_with_partial_failure() {
    let tmp = tempfile::tempdir().unwrap();
    fte()
        .args(["extract", "--indir", "tests/golden/input", "--outdir"])
        .arg(tmp.path())
        .arg("definitely-not-a-real-id")
        .assert()
        .code(1);
}

#[test]
fn a_bad_flag_exits_with_the_usage_code() {
    fte().args(["extract", "--nope"]).assert().code(2);
}

#[test]
fn an_unknown_template_token_exits_with_the_config_code() {
    let tmp = tempfile::tempdir().unwrap();
    fte()
        .args(["split", "--name", "{chapter}.md", "--outdir"])
        .arg(tmp.path())
        .arg("tests/golden/input/books/frankenstein-pg.epub")
        .assert()
        .code(9);
}

#[test]
fn splitting_a_paper_exits_with_the_no_chapters_code() {
    let tmp = tempfile::tempdir().unwrap();
    fte()
        .args(["split", "--outdir"])
        .arg(tmp.path())
        .arg("tests/golden/input/frontiers-epub-angelshark.epub")
        .assert()
        .code(8);
}

#[test]
fn an_explicit_missing_indir_exits_with_the_not_found_code() {
    fte()
        .args(["extract", "--indir", "tests/golden/does-not-exist"])
        .assert()
        .code(3)
        .stderr(predicate::str::contains("check --indir"));
}

#[test]
fn a_missing_default_indir_exits_zero_with_a_warning() {
    let tmp = tempfile::tempdir().unwrap();
    fte()
        .current_dir(tmp.path())
        .arg("extract")
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "warning: input directory ref/epub does not exist",
        ));
}
