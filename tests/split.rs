//! End-to-end behavior of `fte split`.

mod common;

use assert_cmd::Command;
use predicates::prelude::*;

fn fte() -> Command {
    Command::cargo_bin("fte").expect("fte binary builds")
}

const BOOK: &str = "tests/golden/input/books/frankenstein-pg.epub";
const PAPER: &str = "tests/golden/input/frontiers-epub-angelshark.epub";

#[test]
fn splitting_an_epub_writes_numbered_chapters() {
    let tmp = tempfile::tempdir().unwrap();
    fte()
        .args(["split", "--outdir"])
        .arg(tmp.path())
        .arg(BOOK)
        .assert()
        .success();

    let dir = tmp.path().join("frankenstein-pg");
    assert!(dir.join("00-frontmatter.md").exists());

    let mut chapters: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| !n.starts_with("00-"))
        .collect();
    chapters.sort();
    assert!(chapters.len() > 20, "got {} chapters", chapters.len());
    assert!(chapters[0].starts_with("01-"));
}

#[test]
fn the_flat_convention_writes_alongside() {
    let tmp = tempfile::tempdir().unwrap();
    fte()
        .args(["split", "--outdir"])
        .arg(tmp.path())
        .args([
            "--subdir",
            "",
            "--name",
            "{book}-ch{n}.md",
            "--front",
            "{book}-ch{n}-frontmatter.md",
            BOOK,
        ])
        .assert()
        .success();

    assert!(
        tmp.path()
            .join("frankenstein-pg-ch00-frontmatter.md")
            .exists()
    );
    assert!(tmp.path().join("frankenstein-pg-ch01.md").exists());
}

#[test]
fn no_front_suppresses_the_index() {
    let tmp = tempfile::tempdir().unwrap();
    fte()
        .args(["split", "--no-front", "--outdir"])
        .arg(tmp.path())
        .arg(BOOK)
        .assert()
        .success();

    assert!(
        !tmp.path()
            .join("frankenstein-pg/00-frontmatter.md")
            .exists()
    );
}

#[test]
fn a_paper_epub_has_no_chapters_to_split() {
    let tmp = tempfile::tempdir().unwrap();
    fte()
        .args(["split", "--outdir"])
        .arg(tmp.path())
        .arg(PAPER)
        .assert()
        .failure()
        .stderr(predicate::str::contains("no chapter markers"));
}

#[test]
fn an_unknown_template_token_fails_loudly() {
    let tmp = tempfile::tempdir().unwrap();
    fte()
        .args(["split", "--name", "{chapter}.md", "--outdir"])
        .arg(tmp.path())
        .arg(BOOK)
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown template token"));
}

#[test]
fn existing_output_is_refused_without_force() {
    let tmp = tempfile::tempdir().unwrap();
    let run = || {
        let mut c = fte();
        c.args(["split", "--outdir"]).arg(tmp.path()).arg(BOOK);
        c
    };

    run().assert().success();
    run()
        .assert()
        .failure()
        .stderr(predicate::str::contains("exists"));

    fte()
        .args(["split", "--force", "--outdir"])
        .arg(tmp.path())
        .arg(BOOK)
        .assert()
        .success();
}

#[test]
fn one_shot_epub_split_matches_the_two_step_result() {
    let one_shot = tempfile::tempdir().unwrap();
    let two_step = tempfile::tempdir().unwrap();

    fte()
        .args(["split", "--outdir"])
        .arg(one_shot.path())
        .arg(BOOK)
        .assert()
        .success();

    fte()
        .args(["extract", "--outdir"])
        .arg(two_step.path())
        .arg(BOOK)
        .assert()
        .success();
    fte()
        .args(["split", "--outdir"])
        .arg(two_step.path())
        .arg(two_step.path().join("frankenstein-pg.md"))
        .assert()
        .success();

    let a =
        std::fs::read_to_string(one_shot.path().join("frankenstein-pg/00-frontmatter.md")).unwrap();
    let b =
        std::fs::read_to_string(two_step.path().join("frankenstein-pg/00-frontmatter.md")).unwrap();
    assert_eq!(common::skeleton(&a), common::skeleton(&b));
}
