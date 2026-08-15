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
fn an_io_failure_message_carries_the_root_cause() {
    // I7: `with_context(|| "creating {dir}")` used to render as just that
    // outer layer via `e.to_string()`, dropping the io::Error source that
    // actually explains the failure (e.g. "Permission denied").
    let tmp = tempfile::tempdir().unwrap();
    let locked = tmp.path().join("locked");
    std::fs::create_dir(&locked).unwrap();
    let mut perms = std::fs::metadata(&locked).unwrap().permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(&locked, perms).unwrap();

    let assert = fte()
        .args(["split", "--outdir"])
        .arg(locked.join("out"))
        .arg(BOOK)
        .assert()
        .failure();

    let mut perms = std::fs::metadata(&locked).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    perms.set_readonly(false);
    std::fs::set_permissions(&locked, perms).unwrap();

    let out = assert.get_output();
    let stderr = String::from_utf8(out.stderr.clone()).unwrap();
    assert!(
        stderr.to_lowercase().contains("permission denied")
            || stderr.to_lowercase().contains("denied"),
        "expected the io::Error cause in the message, got: {stderr}"
    );
    assert!(
        stderr.contains("creating"),
        "outer context should survive too: {stderr}"
    );
}

#[test]
fn an_unterminated_trailing_chapter_fails_instead_of_vanishing() {
    // Regression for C1: two chapters, the second missing its chapter-end.
    // The old behavior silently dropped chapter two and exited 0.
    let src = tempfile::tempdir().unwrap();
    let input = src.path().join("trunc.md");
    std::fs::write(
        &input,
        "---\nid: trunc\n---\n\n# T\n\n\
         <!-- fte:chapter-start id=\"ch01\" title=\"One\" src=\"a.xhtml\" -->\n\n\
         ## One\n\nbody one\n\n\
         <!-- fte:chapter-end id=\"ch01\" -->\n\n\
         <!-- fte:chapter-start id=\"ch02\" title=\"Two\" src=\"b.xhtml\" -->\n\n\
         ## Two\n\nbody two IMPORTANT\n",
    )
    .unwrap();

    let tmp = tempfile::tempdir().unwrap();
    fte()
        .args(["split", "--format", "text", "--outdir"])
        .arg(tmp.path())
        .arg(&input)
        .assert()
        .failure()
        .code(5)
        .stderr(predicate::str::contains("ch02"));

    // No file anywhere in the output tree may contain the lost chapter's body.
    let leaked = walkdir_contains(tmp.path(), "body two IMPORTANT");
    assert!(!leaked, "chapter two's content leaked despite the failure");
}

fn walkdir_contains(dir: &std::path::Path, needle: &str) -> bool {
    if !dir.exists() {
        return false;
    }
    for entry in std::fs::read_dir(dir).unwrap().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_dir() {
            if walkdir_contains(&path, needle) {
                return true;
            }
        } else if let Ok(content) = std::fs::read_to_string(&path)
            && content.contains(needle)
        {
            return true;
        }
    }
    false
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
