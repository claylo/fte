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
    // Pinned to `--format text`: assert_cmd pipes stdout, so unforced
    // `auto` format resolves to JSON and `wants_json_errors()` would be
    // true. Before I6 this test passed for the wrong reason — matching
    // "extract"/"split" as substrings inside a JSON-escaped `message`
    // field containing the whole help screen, not a real help render.
    // Forcing text mode makes the assertion mean what it says: a plain
    // help screen on stderr, not a JSON envelope.
    let assert = fte()
        .args(["--format", "text"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("extract"))
        .stderr(predicate::str::contains("split"));

    let stderr = String::from_utf8(assert.get_output().stderr.clone()).unwrap();
    assert!(
        serde_json::from_str::<serde_json::Value>(stderr.trim()).is_err(),
        "help screen must not be a JSON envelope: {stderr}"
    );
}

#[test]
fn bare_invocation_prints_real_help_even_in_json_mode() {
    // I6: help text is exempted from the JSON error envelope entirely. No
    // `--format` flag here: assert_cmd pipes stdout, so `wants_json_errors()`
    // would resolve to JSON by the same `auto` rule as everywhere else —
    // this proves the help path bypasses that envelope regardless, rather
    // than stuffing ~700 characters of help into a `message` field.
    let out = fte().assert().code(2).get_output().stderr.clone();
    let stderr = String::from_utf8(out).unwrap();
    assert!(stderr.contains("extract"));
    assert!(stderr.contains("split"));
    assert!(
        serde_json::from_str::<serde_json::Value>(stderr.trim()).is_err(),
        "help screen must not be a JSON envelope: {stderr}"
    );
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
    // Forced text mode: `--stdout` has no JSON items channel, so a missing
    // input's only diagnostic is this stderr line, printed in text mode
    // only (I5) — assert_cmd pipes stdout, so unforced `auto` resolves to
    // JSON and the line would not appear at all.
    fte()
        .args([
            "extract",
            "--format",
            "text",
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
    // assert_cmd pipes stdout, so `auto` format resolves to JSON unless
    // forced — this test is specifically about the text-mode "Done:" line.
    let tmp = tempfile::tempdir().unwrap();
    fte()
        .args([
            "extract",
            "--format",
            "text",
            "--indir",
            "tests/golden/input",
            "--outdir",
        ])
        .arg(tmp.path())
        .args(["bmc-short", "definitely-not-a-real-id"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("1 extracted"))
        .stderr(predicate::str::contains("1 failed"));
}

#[test]
fn quiet_suppresses_progress_but_not_failures() {
    // Forced text mode: the failure itself now lives in the row on stdout
    // (JSON mode reports it the same way, via `status: "failed"`), so -q's
    // job is only to keep OK/Done chatter off stderr.
    let tmp = tempfile::tempdir().unwrap();
    fte()
        .args([
            "extract",
            "-q",
            "--force",
            "--format",
            "text",
            "--indir",
            "tests/golden/input",
            "--outdir",
        ])
        .arg(tmp.path())
        .args(["bmc-short", "definitely-not-a-real-id"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("not found"))
        .stderr(predicate::str::contains("OK").not())
        .stderr(predicate::str::contains("Done:").not());
}

#[test]
fn verbose_reports_detected_format() {
    // Forced text mode: unforced `auto` resolves to JSON here (assert_cmd
    // pipes stdout), and `-v` chatter is text-mode-only (I5).
    let tmp = tempfile::tempdir().unwrap();
    fte()
        .args([
            "extract",
            "-v",
            "--force",
            "--format",
            "text",
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
    // assert_cmd pipes stdout, so `auto` format resolves to JSON unless
    // forced — the warning is deliberately text-mode-only chatter.
    let tmp = tempfile::tempdir().unwrap();
    fte()
        .current_dir(tmp.path())
        .args(["extract", "--format", "text"])
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "warning: input directory ref/epub does not exist",
        ));
}

#[test]
fn an_io_error_on_a_path_containing_exists_is_not_output_exists() {
    let tmp = tempfile::tempdir().unwrap();
    let locked = tmp.path().join("exists");
    std::fs::create_dir(&locked).unwrap();

    let mut perms = std::fs::metadata(&locked).unwrap().permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(&locked, perms).unwrap();

    let assert = fte()
        .args(["split", "--outdir"])
        .arg(locked.join("out"))
        .arg("tests/golden/input/books/frankenstein-pg.epub")
        .assert()
        .failure();

    // Restore permissions so tempdir cleanup succeeds regardless of outcome.
    let mut perms = std::fs::metadata(&locked).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    perms.set_readonly(false);
    std::fs::set_permissions(&locked, perms).unwrap();

    // io_error (7), not output_exists (6): the path merely contains the word.
    assert.code(7);
}

#[test]
fn detect_emits_an_items_envelope_in_json_mode() {
    let out = fte()
        .args([
            "detect",
            "--format",
            "json",
            "tests/golden/input/bmc-short.html",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let parsed: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert!(parsed["items"].is_array());
    assert_eq!(parsed["items"][0]["id"], "bmc-short");
}

#[test]
fn help_and_version_still_succeed() {
    fte().arg("--help").assert().success();
    fte().arg("--version").assert().success();
}

#[test]
fn a_usage_error_emits_the_structured_envelope() {
    let out = fte()
        .args(["extract", "--nope", "--format", "json"])
        .assert()
        .code(2)
        .get_output()
        .stderr
        .clone();

    let text = String::from_utf8(out).unwrap();
    let last = text.lines().last().expect("stderr is not empty");
    let parsed: serde_json::Value =
        serde_json::from_str(last).expect("last stderr line is a JSON envelope");
    assert_eq!(parsed["kind"], "usage");
}

#[test]
fn json_mode_puts_no_plain_text_on_stderr() {
    let out = fte()
        .args([
            "detect",
            "--indir",
            "tests/golden/input",
            "--format",
            "json",
            "definitely-not-a-real-id",
        ])
        .assert()
        .code(1)
        .get_output()
        .stderr
        .clone();

    let text = String::from_utf8(out).unwrap();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        assert!(
            serde_json::from_str::<serde_json::Value>(line).is_ok(),
            "non-JSON line on stderr in JSON mode: {line}"
        );
    }
}

#[test]
fn a_failed_extraction_is_reported_exactly_once() {
    // I3: the same failure used to print once on stdout (via the row) and
    // once on stderr (via a separate eprintln under a different
    // identifier — "sage-html-article.md" vs "sage-html-article"). The row
    // is now the sole report; stderr carries only progress/summary chatter.
    let tmp = tempfile::tempdir().unwrap();
    let assert = fte()
        .args(["extract", "--format", "text", "--outdir"])
        .arg(tmp.path())
        .arg("tests/golden/input/sage-html-article.html")
        .assert()
        .failure();

    let out = assert.get_output();
    let stdout = String::from_utf8(out.stdout.clone()).unwrap();
    let stderr = String::from_utf8(out.stderr.clone()).unwrap();

    let stdout_fails = stdout.lines().filter(|l| l.contains("FAIL")).count();
    assert_eq!(
        stdout_fails, 1,
        "expected exactly one FAIL row on stdout:\n{stdout}"
    );
    assert!(
        !stderr.contains("FAIL"),
        "the failure must not also appear on stderr:\n{stderr}"
    );
    // The reason (I4) survives even without the old stderr line.
    assert!(stdout.contains("sage-html-article"));
    assert!(stdout.contains("abstract-only format"));
}

#[test]
fn a_resolved_unsupported_file_does_not_report_not_found() {
    // N1 regression: the text-mode row used to discriminate "missing input"
    // from "failed extraction" by testing `source_format == "unknown"", but
    // a resolved file with an unclassifiable format also has
    // `source_format: "unknown"` — so the row lied and said "not found" for
    // a file that plainly exists and was given as an explicit path.
    let indir = tempfile::tempdir().unwrap();
    let input = indir.path().join("note.txt");
    std::fs::write(&input, "hi").unwrap();

    let outdir = tempfile::tempdir().unwrap();
    let assert = fte()
        .args(["extract", "--format", "text", "--outdir"])
        .arg(outdir.path())
        .arg(&input)
        .assert()
        .failure();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert!(
        !stdout.contains("not found"),
        "a resolved file must not be reported as not found:\n{stdout}"
    );
    assert!(
        stdout.contains("unknown format"),
        "the real reason must appear:\n{stdout}"
    );
}

#[test]
fn extract_verbose_json_mode_puts_no_plain_text_on_stderr() {
    // I5: `-v` had no `render` check and leaked "  detect id: format" onto
    // stderr even in JSON mode. Regression per the review's suggestion to
    // extend `json_mode_puts_no_plain_text_on_stderr` beyond `detect`.
    let tmp = tempfile::tempdir().unwrap();
    let out = fte()
        .args([
            "extract",
            "-v",
            "--force",
            "--format",
            "json",
            "--indir",
            "tests/golden/input",
            "--outdir",
        ])
        .arg(tmp.path())
        .arg("bmc-short")
        .assert()
        .success()
        .get_output()
        .stderr
        .clone();

    let text = String::from_utf8(out).unwrap();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        assert!(
            serde_json::from_str::<serde_json::Value>(line).is_ok(),
            "non-JSON line on stderr in JSON mode: {line}"
        );
    }
}

#[test]
fn extract_stdout_json_mode_puts_no_plain_text_on_stderr() {
    // The `--stdout` per-item diagnostic (I5's smaller leak, extract.rs:64)
    // is text-mode-only; JSON mode gets nothing on stderr for a per-item
    // failure since --stdout has no items channel to report it through.
    let out = fte()
        .args([
            "extract",
            "--stdout",
            "--format",
            "json",
            "--indir",
            "tests/golden/input",
            "definitely-not-a-real-id",
        ])
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();

    let text = String::from_utf8(out).unwrap();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        assert!(
            serde_json::from_str::<serde_json::Value>(line).is_ok(),
            "non-JSON line on stderr in JSON mode: {line}"
        );
    }
}

#[test]
fn split_json_mode_puts_no_plain_text_on_stderr() {
    let tmp = tempfile::tempdir().unwrap();
    let out = fte()
        .args(["split", "--format", "json", "--outdir"])
        .arg(tmp.path())
        .arg("tests/golden/input/books/frankenstein-pg.epub")
        .assert()
        .success()
        .get_output()
        .stderr
        .clone();

    let text = String::from_utf8(out).unwrap();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        assert!(
            serde_json::from_str::<serde_json::Value>(line).is_ok(),
            "non-JSON line on stderr in JSON mode: {line}"
        );
    }
}

#[test]
fn a_failed_row_carries_a_reason_in_json_mode() {
    // I4: a JSON consumer could see status: "failed" with no way to tell
    // why. `reason` makes the cause part of the structured envelope.
    let tmp = tempfile::tempdir().unwrap();
    let out = fte()
        .args(["extract", "--format", "json", "--outdir"])
        .arg(tmp.path())
        .arg("tests/golden/input/sage-html-article.html")
        .assert()
        .failure()
        .get_output()
        .stdout
        .clone();

    let parsed: serde_json::Value = serde_json::from_slice(&out).unwrap();
    let items = parsed["items"].as_array().expect("items array");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["status"], "failed");
    assert!(
        items[0]["reason"].is_string() && items[0]["reason"] != "",
        "failed row must carry a non-empty reason: {items:?}"
    );
}

#[test]
fn a_missing_input_appears_as_a_failed_row() {
    let out = fte()
        .args([
            "detect",
            "--indir",
            "tests/golden/input",
            "--format",
            "json",
            "bmc-short",
            "definitely-not-a-real-id",
        ])
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();

    let parsed: serde_json::Value = serde_json::from_slice(&out).unwrap();
    let items = parsed["items"].as_array().expect("items array");
    assert_eq!(items.len(), 2, "both inputs should be reported");
    assert!(
        items
            .iter()
            .any(|i| i["status"] == "failed" && i["id"] == "definitely-not-a-real-id"),
        "the missing input needs a failed row"
    );
}

#[test]
fn stdout_stays_raw_markdown_even_when_piped() {
    let out = fte()
        .args(["extract", "--stdout", "tests/golden/input/bmc-short.html"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let text = String::from_utf8(out).unwrap();
    assert!(
        text.starts_with("---\n"),
        "expected raw markdown frontmatter"
    );
    assert!(serde_json::from_str::<serde_json::Value>(&text).is_err());
}
