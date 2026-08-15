# CLI Restructure and Chapter Splitting Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give `fte` a subcommand surface (`extract`/`detect`/`split`), emit chapter boundary markers in book-length markdown, split books into per-chapter files with configurable naming, and raise `clispec score` from 11/24 to 20/24.

**Architecture:** Extraction moves from a bare positional into `fte extract`, which unblocks the CLIspec scorer (five checks currently fail with `no subcommand to test`). The book extraction path wraps each chapter in paired HTML-comment markers; `fte split` parses those markers back out and writes per-chapter files using config-driven filename templates. A thin error/output layer gives every command declared exit codes and a JSON envelope.

**Tech Stack:** Rust 2024 edition, clap 4 via `librebar::cli`, `scraper` for HTML, `zip` for epub, `serde_json` for output, `sha2` (dev-only) for golden hashes, `cargo nextest` for tests, `clispec` CLI for conformance scoring.

**Spec:** `record/superpowers/specs/2026-08-14-cli-restructure-and-chapter-splitting-design.md`

## Global Constraints

- **Edition 2024, MSRV 1.89.** Toolchain pinned in `rust-toolchain.toml` (currently 1.97.1).
- **Clippy must pass with `-D warnings`** under `--all-targets --all-features`. Run `just clippy`.
- **Formatting uses the repo config:** `cargo fmt --all -- --config-path .config/rustfmt.toml`. Run `just fmt`, never bare `cargo fmt`.
- **`just check` = fmt + clippy + deny + test + doc-test.** New dependencies must pass `cargo deny` license and advisory checks.
- **Paper (non-book) extraction output must not change.** Every file in `tests/golden/expected/` stays byte-identical. Chapter markers are emitted by the book path only.
- **Never manually edit `version` in `Cargo.toml`.**
- **Tests run with `cargo nextest run --all-features`** (`just test`).
- **`UPDATE_GOLDEN=1`** is the existing convention for regenerating golden files. New golden harnesses honor it.
- **Error kinds and exit codes are fixed by the spec.** Copy this table verbatim; do not invent new codes:

  | Kind | Exit | Retryable |
  |------|------|-----------|
  | `usage` | 2 | false |
  | `not_found` | 3 | false |
  | `unsupported_format` | 4 | false |
  | `extraction_failed` | 5 | false |
  | `output_exists` | 6 | false |
  | `io_error` | 7 | true |
  | `no_chapters` | 8 | false |
  | `config_error` | 9 | false |

  Exit 1 is the `partial_failure` **outcome**, not an error.

---

## File Structure

**New library modules** (add to `src/lib.rs`):

| File | Responsibility |
|------|----------------|
| `src/chapter.rs` | Chapter marker rendering *and* parsing. Both directions live together because the format is one contract. |
| `src/template.rs` | Filename template expansion (`{book}`, `{n}`, `{slug}`, `{title}`, `{src}`). |
| `src/splitter.rs` | Turns a parsed chapter document into files on disk. |
| `src/errors.rs` | Error kinds, exit codes, structured stderr rendering. |
| `src/output.rs` | `{"items": [...]}` envelope rendering for text and JSON. |
| `src/inputs.rs` | Input resolution shared by `extract` and `detect` (moved out of `main.rs`). |

**New binary modules** (declared by `src/main.rs`):

| File | Responsibility |
|------|----------------|
| `src/cmd/mod.rs` | Re-exports the three command modules. |
| `src/cmd/extract.rs` | `fte extract` |
| `src/cmd/detect.rs` | `fte detect` |
| `src/cmd/split.rs` | `fte split` |

**Modified:**

| File | Change |
|------|--------|
| `src/main.rs` | Becomes arg definitions + dispatch only. |
| `src/epub.rs` | `extract_book` wraps chapters in markers. |
| `src/config.rs` | Adds `SplitConfig`. |
| `Cargo.toml` | Adds `serde_json`; dev-adds `sha2`. |
| `.justfile` | Adds `clispec` recipe. |
| `README.md` | Documents the new surface. |

**New test files:**

| File | Responsibility |
|------|----------------|
| `tests/common/mod.rs` | Skeleton renderer shared by book and split goldens. Not its own test binary. |
| `tests/book_golden.rs` | Structural goldens for the two Gutenberg books. |
| `tests/split.rs` | `fte split` behavior and goldens. |
| `tests/golden/expected/books/` | Skeleton files. |

---

## Task 1: Chapter marker rendering

**Files:**
- Create: `src/chapter.rs`
- Modify: `src/lib.rs`
- Modify: `src/epub.rs:77-154` (`extract_book`)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces:
  - `fte::chapter::chapter_id(index: usize) -> String` — `1` → `"ch01"`
  - `fte::chapter::sanitize_attr(value: &str) -> String`
  - `fte::chapter::start_marker(id: &str, title: &str, src: &str) -> String`
  - `fte::chapter::end_marker(id: &str) -> String`
  - `fte::chapter::first_heading_text(md: &str) -> Option<String>`
  - Constants `fte::chapter::START_PREFIX` and `END_PREFIX`.

**Background:** An HTML comment cannot contain `--` anywhere in its body — `-->` terminates it, and `--` is illegal even mid-comment per the HTML spec. Book epubs frequently render em dashes as `--` in chapter titles, so an unsanitized title silently truncates or corrupts the marker. A `"` inside an attribute value would likewise break the quoted-value scanner in Task 4.

- [ ] **Step 1: Write the failing tests**

Create `src/chapter.rs` with only the test module for now:

```rust
//! Chapter boundary markers in generated book markdown.
//!
//! Book output wraps each chapter in a paired HTML comment so `fte split`
//! can recover chapter boundaries without re-parsing the source ePub.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chapter_ids_are_zero_padded() {
        assert_eq!(chapter_id(1), "ch01");
        assert_eq!(chapter_id(9), "ch09");
        assert_eq!(chapter_id(10), "ch10");
        assert_eq!(chapter_id(100), "ch100");
    }

    #[test]
    fn double_hyphens_cannot_reach_the_comment() {
        assert_eq!(sanitize_attr("Walking--in the Dark"), "Walking–in the Dark");
        assert_eq!(sanitize_attr("a----b"), "a––b");
    }

    #[test]
    fn comment_terminators_and_quotes_are_neutralized() {
        assert_eq!(sanitize_attr("close --> here"), "close – here");
        assert_eq!(sanitize_attr(r#"say "hi""#), "say 'hi'");
    }

    #[test]
    fn whitespace_collapses_to_single_spaces() {
        assert_eq!(sanitize_attr("Letter\n  One  "), "Letter One");
    }

    #[test]
    fn markers_round_trip_their_attributes() {
        let start = start_marker("ch03", "Walking in the Dark", "OEBPS/ch03.xhtml");
        assert_eq!(
            start,
            r#"<!-- fte:chapter-start id="ch03" title="Walking in the Dark" src="OEBPS/ch03.xhtml" -->"#
        );
        assert_eq!(end_marker("ch03"), r#"<!-- fte:chapter-end id="ch03" -->"#);
    }

    #[test]
    fn first_heading_text_finds_the_leading_heading() {
        assert_eq!(
            first_heading_text("## Letter 1\n\nbody"),
            Some("Letter 1".to_string())
        );
        assert_eq!(first_heading_text("no heading here"), None);
        assert_eq!(
            first_heading_text("plain\n\n### Deep\n"),
            Some("Deep".to_string())
        );
    }
}
```

Add `pub mod chapter;` to `src/lib.rs`, keeping the list alphabetical:

```rust
pub mod chapter;
pub mod config;
pub mod depth;
pub mod detect;
pub mod epub;
pub mod extract;
pub mod html;
pub mod jats;
pub mod markdown;
pub mod wiley_xml;
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run --all-features chapter::`
Expected: FAIL — `cannot find function 'chapter_id' in this scope` and similar for each helper.

- [ ] **Step 3: Write the implementation**

Insert above the `#[cfg(test)]` block in `src/chapter.rs`:

```rust
/// Opening delimiter of a chapter-start marker.
pub const START_PREFIX: &str = "<!-- fte:chapter-start ";
/// Opening delimiter of a chapter-end marker.
pub const END_PREFIX: &str = "<!-- fte:chapter-end ";

/// Format a sequential chapter identifier: `1` becomes `ch01`.
///
/// Sequential rather than derived from the source filename: book ePubs
/// routinely use obfuscated stems (`sgPhzGILRlKrLKg2DMvpew1`, `c0`), which the
/// `src` attribute preserves for provenance while this stays legible.
#[must_use]
pub fn chapter_id(index: usize) -> String {
    format!("ch{index:02}")
}

/// Make a value safe to place inside a double-quoted HTML comment attribute.
///
/// `--` is illegal anywhere inside an HTML comment, and `-->` terminates it.
/// Converter output renders em dashes as `--` often enough that an
/// unsanitized chapter title is a live corruption hazard, not a theoretical
/// one.
#[must_use]
pub fn sanitize_attr(value: &str) -> String {
    let mut out = crate::markdown::normalize_text(value);
    out = out.replace('>', "");
    out = out.replace('"', "'");
    while out.contains("--") {
        out = out.replace("--", "\u{2013}");
    }
    out.trim().to_string()
}

/// Render the opening marker for a chapter.
#[must_use]
pub fn start_marker(id: &str, title: &str, src: &str) -> String {
    format!(
        "{START_PREFIX}id=\"{}\" title=\"{}\" src=\"{}\" -->",
        sanitize_attr(id),
        sanitize_attr(title),
        sanitize_attr(src)
    )
}

/// Render the closing marker for a chapter.
#[must_use]
pub fn end_marker(id: &str) -> String {
    format!("{END_PREFIX}id=\"{}\" -->", sanitize_attr(id))
}

/// The text of the first markdown heading in a fragment, if there is one.
///
/// Used as the chapter title when the nav TOC has no label for a spine file.
#[must_use]
pub fn first_heading_text(md: &str) -> Option<String> {
    md.lines()
        .find(|line| line.trim_start().starts_with('#'))
        .map(|line| line.trim_start().trim_start_matches('#').trim().to_string())
        .filter(|text| !text.is_empty())
}
```

Note the ordering inside `sanitize_attr`: whitespace collapses first so that a
newline between two hyphens cannot produce a `--` after collapsing, then `>` is
removed, then `--` is folded repeatedly until none remain. Each pass strictly
shortens the string, so the loop terminates.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo nextest run --all-features chapter::`
Expected: PASS, 6 tests.

- [ ] **Step 5: Wire markers into `extract_book`**

The current loop pushes an injected heading directly onto `body` *before* it
knows whether the chapter has content (`src/epub.rs:128-145`). The marker has to
enclose that heading, so the chapter's markdown is built into a local buffer
first and wrapped once it is known to be non-empty.

Replace the body of the `for entry_path in content_paths` loop in
`src/epub.rs`, from the `let has_real_heading` line through the closing brace of
the `if !trimmed.is_empty()` block, with:

```rust
        let has_real_heading = has_heading_in_body(&doc);

        let mut chapter_md = String::new();
        if !has_real_heading && let Some(label) = nav_label {
            let heading = nav_label_to_heading(label);
            if !heading.is_empty() {
                chapter_md.push_str(&heading);
                chapter_md.push_str("\n\n");
            }
        }

        let mut walked = String::new();
        html::walk_element(&body_el, &strip_sels, &mut walked, 0);
        let walked = markdown::collapse_blanks(&walked);
        let walked = walked.trim();

        if walked.is_empty() {
            continue;
        }
        chapter_md.push_str(walked);

        chapter_index += 1;
        let id = chapter::chapter_id(chapter_index);
        let title = nav_label
            .map(str::to_owned)
            .or_else(|| chapter::first_heading_text(&chapter_md))
            .unwrap_or_default();

        body.push_str(&chapter::start_marker(&id, &title, entry_path));
        body.push_str("\n\n");
        body.push_str(&chapter_md);
        body.push_str("\n\n");
        body.push_str(&chapter::end_marker(&id));
        body.push_str("\n\n");
        included_any = true;
```

Declare the counter next to `included_any` near `src/epub.rs:88`:

```rust
    let mut included_any = false;
    let mut chapter_index = 0usize;
```

Add the import at the top of `src/epub.rs`:

```rust
use crate::chapter;
```

`markdown::collapse_blanks` runs over the whole body afterwards and only
collapses blank lines, so it leaves marker lines untouched.

- [ ] **Step 6: Verify paper output is untouched**

Run: `just test`
Expected: PASS. Every existing golden file in `tests/golden/expected/` must still
match — markers are emitted only by `extract_book`, and paper epubs
(≤5 spine files, or no nav document) go through `extract_paper`.

If any paper golden fails, stop: the book-detection branch has been changed by
mistake. Do not run `UPDATE_GOLDEN=1`.

- [ ] **Step 7: Eyeball a real book**

Run:
```bash
cargo run --quiet -- --stdout tests/golden/input/books/frankenstein-pg.epub 2>/dev/null | grep -c 'fte:chapter-start'
cargo run --quiet -- --stdout tests/golden/input/books/frankenstein-pg.epub 2>/dev/null | grep -m2 'fte:chapter-start'
```
Expected: a count above 20, and marker lines with populated `title=` and `src=`
attributes. Note this uses the *current* bare-positional CLI; Task 3 changes the
invocation to `fte extract --stdout ...`.

- [ ] **Step 8: Run the full check and commit**

Run: `just fmt && just clippy && just test`
Expected: all pass.

```bash
git add src/chapter.rs src/lib.rs src/epub.rs
git commit -m "feat(epub): wrap book chapters in paired boundary markers"
```

---

## Task 2: Structural golden tests for books

**Files:**
- Create: `tests/common/mod.rs`
- Create: `tests/book_golden.rs`
- Create: `tests/golden/expected/books/frankenstein-pg.skeleton.txt` (generated)
- Create: `tests/golden/expected/books/tale-two-cities-pg.skeleton.txt` (generated)
- Modify: `Cargo.toml`

**Interfaces:**
- Consumes: `fte::chapter::{START_PREFIX, END_PREFIX}` from Task 1.
- Produces: `common::skeleton(md: &str) -> String` and `common::check_skeleton(name: &str, md: &str)`, used again by Task 7.

**Background:** `tests/golden.rs:53-66` renders a full line diff into the panic
message. Frankenstein extracts to roughly 400KB of markdown and A Tale of Two
Cities to roughly 800KB, so a one-line regression would print an entire novel.
The skeleton records structure in full and collapses prose to a word count, with
a `sha256` header so a body-only change still fails — reported as "hash changed,
structure identical", which localizes the problem instead of burying it.

A file at `tests/common/mod.rs` is a module directory, not a test target, so
Cargo does not compile it as its own test binary. Both `tests/book_golden.rs` and
`tests/split.rs` declare `mod common;` to share it.

- [ ] **Step 1: Add the dev-dependency**

In `Cargo.toml`, under `[dev-dependencies]`, keeping entries alphabetical:

```toml
[dev-dependencies]
assert_cmd = "2.2"
diff = "0.1.13"
predicates = "3.1"
sha2 = "0.11"
tempfile = "3.27"
```

Run: `cargo deny --all-features --config .config/deny.toml check`
Expected: PASS. `sha2` is MIT OR Apache-2.0.

- [ ] **Step 2: Write the skeleton renderer**

Create `tests/common/mod.rs`:

```rust
//! Shared helpers for structural golden tests.
//!
//! Book-length markdown is too large for the full-text golden comparison in
//! `tests/golden.rs`, whose failure path prints a complete line diff. These
//! helpers render a compact skeleton instead: structure in full, prose as a
//! word count, plus a hash of the complete output so body-only changes still
//! fail.

use sha2::{Digest, Sha256};
use std::path::Path;

use fte::chapter::{END_PREFIX, START_PREFIX};

/// Render a compact structural summary of a markdown document.
pub fn skeleton(md: &str) -> String {
    let mut out = String::new();

    let mut hasher = Sha256::new();
    hasher.update(md.as_bytes());
    out.push_str(&format!("sha256 {:x}\n", hasher.finalize()));
    out.push_str(&format!("bytes  {}\n", md.len()));
    out.push_str("---\n");

    let mut in_frontmatter = false;
    let mut frontmatter = Vec::new();
    let mut words = 0usize;

    for (i, line) in md.lines().enumerate() {
        let trimmed = line.trim();

        if i == 0 && trimmed == "---" {
            in_frontmatter = true;
            continue;
        }
        if in_frontmatter {
            if trimmed == "---" {
                in_frontmatter = false;
                out.push_str(&format!("frontmatter {}\n", frontmatter.join(" ")));
                frontmatter.clear();
            } else if !trimmed.is_empty() {
                frontmatter.push(trimmed.replace(' ', "\u{a0}"));
            }
            continue;
        }

        let structural = trimmed.starts_with(START_PREFIX)
            || trimmed.starts_with(END_PREFIX)
            || trimmed.starts_with('#')
            || is_toc_entry(trimmed);

        if structural && words > 0 {
            out.push_str(&format!("text {words}w\n"));
            words = 0;
        }

        if let Some(rest) = trimmed.strip_prefix(START_PREFIX) {
            out.push_str(&format!("chapter-start {}\n", rest.trim_end_matches("-->").trim()));
        } else if let Some(rest) = trimmed.strip_prefix(END_PREFIX) {
            out.push_str(&format!("chapter-end {}\n", rest.trim_end_matches("-->").trim()));
        } else if trimmed.starts_with('#') {
            let level = trimmed.chars().take_while(|&c| c == '#').count();
            let text = trimmed.trim_start_matches('#').trim();
            out.push_str(&format!("h{level} {text}\n"));
        } else if is_toc_entry(trimmed) {
            let depth = (line.len() - line.trim_start().len()) / 2;
            let slug = trimmed.rsplit_once("(#").map_or("", |(_, s)| s.trim_end_matches(')'));
            out.push_str(&format!("toc-entry depth={depth} slug={slug}\n"));
        } else {
            words += trimmed.split_whitespace().count();
        }
    }

    if words > 0 {
        out.push_str(&format!("text {words}w\n"));
    }

    out
}

fn is_toc_entry(trimmed: &str) -> bool {
    trimmed.starts_with("- [") && trimmed.contains("](#")
}

/// Compare a document's skeleton against its golden file.
///
/// Honors `UPDATE_GOLDEN=1`, matching the convention in `tests/golden.rs`.
pub fn check_skeleton(name: &str, md: &str) {
    let path = Path::new("tests/golden/expected/books").join(format!("{name}.skeleton.txt"));
    let actual = skeleton(md);

    if std::env::var("UPDATE_GOLDEN").is_ok() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("creating skeleton directory");
        }
        std::fs::write(&path, &actual)
            .unwrap_or_else(|e| panic!("writing {}: {e}", path.display()));
        return;
    }

    let expected = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "reading {}: {e}\n\nRun with UPDATE_GOLDEN=1 to create it",
            path.display()
        )
    });

    if actual == expected {
        return;
    }

    let structure_only = |s: &str| {
        s.lines()
            .filter(|l| !l.starts_with("sha256 ") && !l.starts_with("bytes  "))
            .collect::<Vec<_>>()
            .join("\n")
    };

    if structure_only(&actual) == structure_only(&expected) {
        panic!(
            "{name}: hash changed, structure identical.\n\n\
             Body text changed without altering headings, chapter markers, or the TOC.\n\
             Re-extract and diff by hand, then run UPDATE_GOLDEN=1 to accept."
        );
    }

    let mut diff = String::new();
    for change in diff::lines(&expected, &actual) {
        match change {
            diff::Result::Left(l) => diff.push_str(&format!("- {l}\n")),
            diff::Result::Right(r) => diff.push_str(&format!("+ {r}\n")),
            diff::Result::Both(b, _) => diff.push_str(&format!("  {b}\n")),
        }
    }
    panic!("skeleton mismatch for {name}:\n\n{diff}\n\nRun with UPDATE_GOLDEN=1 to accept");
}
```

The frontmatter line replaces spaces with non-breaking spaces so a multi-word
title stays on one skeleton line without needing quoting.

- [ ] **Step 3: Write the book golden tests**

Create `tests/book_golden.rs`:

```rust
//! Structural goldens for book-length ePub extraction.
//!
//! Unlike `tests/golden.rs`, these compare a compact skeleton rather than full
//! text — see `tests/common/mod.rs` for why.

mod common;

use std::path::Path;

fn book(name: &str) {
    let path = Path::new("tests/golden/input/books").join(format!("{name}.epub"));
    let md = fte::epub::extract(name, &path)
        .unwrap_or_else(|e| panic!("extracting {name}: {e}"));
    common::check_skeleton(name, &md);
}

#[test]
fn frankenstein_structure() {
    book("frankenstein-pg");
}

#[test]
fn tale_of_two_cities_structure() {
    book("tale-two-cities-pg");
}
```

- [ ] **Step 4: Run to verify they fail**

Run: `cargo nextest run --all-features --test book_golden`
Expected: FAIL, both tests, with `Run with UPDATE_GOLDEN=1 to create it`.

- [ ] **Step 5: Generate the skeletons and inspect them**

Run:
```bash
UPDATE_GOLDEN=1 cargo nextest run --all-features --test book_golden
head -30 tests/golden/expected/books/frankenstein-pg.skeleton.txt
wc -l tests/golden/expected/books/*.skeleton.txt
```

Read the output before accepting it. Confirm:
- Every `chapter-start` has a non-empty `title=` and a plausible `src=`.
- `chapter-start`/`chapter-end` counts match.
- `h1`/`h2` lines look like real chapter titles, not junk stems.
- The skeleton files are on the order of hundreds of lines, not tens of thousands.

If titles are empty or ids are out of sequence, fix Task 1 rather than accepting
a bad baseline.

- [ ] **Step 6: Run to verify they now pass**

Run: `cargo nextest run --all-features --test book_golden`
Expected: PASS, 2 tests.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml Cargo.lock tests/common tests/book_golden.rs tests/golden/expected/books
git commit -m "test(epub): add structural golden coverage for book extraction"
```

---

## Task 3: Subcommand restructure

**Files:**
- Create: `src/inputs.rs`
- Create: `src/cmd/mod.rs`, `src/cmd/extract.rs`, `src/cmd/detect.rs`
- Modify: `src/main.rs` (full rewrite)
- Modify: `src/lib.rs`
- Modify: `tests/cli.rs`

**Interfaces:**
- Consumes: nothing from Tasks 1–2.
- Produces:
  - `fte::inputs::resolve(inputs: &[String], input_dir: &Path) -> anyhow::Result<(Vec<PathBuf>, u32)>`
  - `cmd::extract::run(args: &ExtractArgs, cfg: &Config, common: &CommonArgs) -> anyhow::Result<ExitCode>`
  - `cmd::detect::run(args: &DetectArgs, cfg: &Config) -> anyhow::Result<ExitCode>`
  - `ExtractArgs` / `DetectArgs` / `SplitArgs` clap structs in `src/main.rs`.

**Background:** librebar injects `schema` and `completions` as real clap
subcommands (`librebar/src/cli/parse.rs:48`) and returns early for both
*before* calling `T::from_arg_matches_mut` (`parse.rs:110-144`). A required
subcommand enum on `Cli` is therefore safe — `fte schema` never reaches the
enum. librebar also rejects an application that defines those two names itself,
so do not add them.

`arg_required_else_help = true` makes bare `fte` produce
`ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand`, which clap exits with
code 2 and prints to stderr. That is exactly the spec's "help on stderr,
exit 2".

- [ ] **Step 1: Write the failing CLI tests**

Replace the contents of `tests/cli.rs`:

```rust
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
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo nextest run --all-features --test cli`
Expected: FAIL — `bare_invocation_shows_help_and_exits_two` and
`detect_is_its_own_command` fail, and every `extract`-prefixed test fails because
`extract` is currently parsed as an input filename.

- [ ] **Step 3: Move input resolution into its own module**

Create `src/inputs.rs` by moving `resolve_inputs` out of `src/main.rs:153-193`
unchanged except for the name and visibility:

```rust
//! Resolving CLI inputs to concrete paths.

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// Resolve CLI inputs to concrete paths, plus a count of requested inputs
/// that don't exist anywhere — the caller folds that count into its failure
/// total so the summary and exit code reflect them.
pub fn resolve(inputs: &[String], input_dir: &Path) -> Result<(Vec<PathBuf>, u32)> {
    if inputs.is_empty() {
        let mut paths: Vec<PathBuf> = fs::read_dir(input_dir)
            .with_context(|| format!("reading {}", input_dir.display()))?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.extension()
                    .is_some_and(|ext| ext == "html" || ext == "xml" || ext == "epub")
            })
            .collect();
        paths.sort();
        return Ok((paths, 0));
    }

    let mut paths = Vec::new();
    let mut missing = 0u32;
    for input in inputs {
        let p = Path::new(input);
        if p.exists() {
            paths.push(p.to_path_buf());
        } else {
            let epub = input_dir.join(format!("{input}.epub"));
            let xml = input_dir.join(format!("{input}.xml"));
            let html = input_dir.join(format!("{input}.html"));
            if epub.exists() {
                paths.push(epub);
            } else if xml.exists() {
                paths.push(xml);
            } else if html.exists() {
                paths.push(html);
            } else {
                eprintln!("  FAIL {input}: not found");
                missing += 1;
            }
        }
    }
    Ok((paths, missing))
}
```

Add `pub mod inputs;` to `src/lib.rs` in alphabetical position (after `html`).

- [ ] **Step 4: Write the command modules**

Create `src/cmd/mod.rs`:

```rust
//! Subcommand implementations.

pub mod detect;
pub mod extract;
```

Create `src/cmd/extract.rs`, moving the extraction loop out of `main.rs`:

```rust
//! `fte extract` — publisher markup to markdown.

use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use fte::{config::Config, detect, epub, extract, inputs};

use crate::ExtractArgs;

pub fn run(args: &ExtractArgs, cfg: &Config, quiet: bool, verbose: bool) -> Result<ExitCode> {
    let input_dir = args
        .indir
        .clone()
        .unwrap_or_else(|| PathBuf::from(&cfg.input_dir));
    let output_dir = args
        .outdir
        .clone()
        .unwrap_or_else(|| PathBuf::from(&cfg.output_dir));

    if !args.stdout {
        fs::create_dir_all(&output_dir)?;
    }

    let (paths, mut fail) = inputs::resolve(&args.inputs, &input_dir)?;

    let mut ok = 0u32;
    let mut skip = 0u32;

    for path in &paths {
        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        let (format, content) = if ext == "epub" {
            (detect::Format::Epub, None)
        } else {
            let text =
                fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
            let format = detect::detect_format(path, &text, cfg);
            (format, Some(text))
        };

        if verbose {
            eprintln!("  detect {id}: {format}");
        }

        let out_path = output_dir.join(format!("{id}.md"));
        if !args.force && !args.stdout && out_path.exists() {
            skip += 1;
            continue;
        }

        let result = match &content {
            None => epub::extract(id, path),
            Some(text) => extract::extract(&format, id, text),
        };

        match result {
            Ok(md) => {
                if args.stdout {
                    println!("{md}");
                } else {
                    fs::write(&out_path, &md)
                        .with_context(|| format!("writing {}", out_path.display()))?;
                    if !quiet {
                        let kb = md.len() / 1024;
                        eprintln!("  OK   {id}.md ({kb}KB)");
                    }
                }
                ok += 1;
            }
            Err(e) => {
                eprintln!("  FAIL {id}: {e}");
                fail += 1;
            }
        }
    }

    if !quiet && !args.stdout {
        eprintln!("\nDone: {ok} extracted, {skip} skipped, {fail} failed");
    }

    Ok(if fail > 0 {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}
```

Note the one behavior fix folded in: the skip-if-exists check now also requires
`!args.stdout`. Previously `--stdout` still consulted `out_path.exists()`, which
could silently suppress output.

Create `src/cmd/detect.rs`:

```rust
//! `fte detect` — report the detected format without extracting.

use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use fte::{config::Config, detect, inputs};

use crate::DetectArgs;

pub fn run(args: &DetectArgs, cfg: &Config) -> Result<ExitCode> {
    let input_dir = args
        .indir
        .clone()
        .unwrap_or_else(|| PathBuf::from(&cfg.input_dir));

    let (paths, fail) = inputs::resolve(&args.inputs, &input_dir)?;

    for path in &paths {
        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        let format = if ext == "epub" {
            detect::Format::Epub
        } else {
            let text =
                fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
            detect::detect_format(path, &text, cfg)
        };

        println!("{id}: {format}");
    }

    Ok(if fail > 0 {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}
```

- [ ] **Step 5: Rewrite `main.rs`**

Replace `src/main.rs` entirely:

```rust
use anyhow::{Context, Result};
use librebar::cli::clap::{self, Args, Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

mod cmd;

use fte::config;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(
    version,
    about = "Extract clean markdown from publisher HTML/XML/ePub academic papers",
    arg_required_else_help = true
)]
struct Cli {
    #[command(flatten)]
    common: librebar::cli::CommonArgs,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Extract markdown from publisher markup
    Extract(ExtractArgs),
    /// Report the detected format without extracting
    Detect(DetectArgs),
    /// Split book markdown into per-chapter files
    Split(SplitArgs),
}

#[derive(Args)]
pub struct ExtractArgs {
    /// Input files or IDs (looked up in input_dir)
    pub inputs: Vec<String>,

    /// Input directory to scan (overrides config)
    #[arg(long)]
    pub indir: Option<PathBuf>,

    /// Output directory (overrides config)
    #[arg(short, long)]
    pub outdir: Option<PathBuf>,

    /// Print to stdout instead of writing files
    #[arg(long)]
    pub stdout: bool,

    /// Overwrite existing output files
    #[arg(long)]
    pub force: bool,
}

#[derive(Args)]
pub struct DetectArgs {
    /// Input files or IDs (looked up in input_dir)
    pub inputs: Vec<String>,

    /// Input directory to scan (overrides config)
    #[arg(long)]
    pub indir: Option<PathBuf>,
}

#[derive(Args)]
pub struct SplitArgs {
    /// Book markdown (.md) or ePub (.epub) to split
    pub input: PathBuf,

    /// Output directory (overrides config)
    #[arg(short, long)]
    pub outdir: Option<PathBuf>,

    /// Subdirectory template (overrides config)
    #[arg(long)]
    pub subdir: Option<String>,

    /// Chapter filename template (overrides config)
    #[arg(long)]
    pub name: Option<String>,

    /// Frontmatter filename template (overrides config)
    #[arg(long, conflicts_with = "no_front")]
    pub front: Option<String>,

    /// Do not write a frontmatter file
    #[arg(long)]
    pub no_front: bool,

    /// Overwrite existing output files
    #[arg(long)]
    pub force: bool,
}

fn main() -> Result<ExitCode> {
    let cli: Cli = librebar::cli::parse();

    if cli.common.apply(VERSION)?.is_exit() {
        return Ok(ExitCode::SUCCESS);
    }

    let cwd = std::env::current_dir()?;
    let cwd_utf8 = cwd.to_str().context("cwd is not valid UTF-8")?;

    // Load config: struct defaults → user config → project config.
    // An explicit `-c/--config` file is layered on top of whatever discovery finds.
    let mut loader = librebar::config::ConfigLoader::new("fte").with_project_search(cwd_utf8);
    if let Some(path) = cli.common.config_path()? {
        loader = loader.with_file(&path);
    }
    let (cfg, _sources) = loader.load::<config::Config>()?;

    let quiet = cli.common.quiet;
    let verbose = cli.common.verbose > 0;

    match &cli.command {
        Commands::Extract(args) => cmd::extract::run(args, &cfg, quiet, verbose),
        Commands::Detect(args) => cmd::detect::run(args, &cfg),
        Commands::Split(_) => anyhow::bail!("split is not implemented yet"),
    }
}
```

`Commands::Split` is stubbed here and filled in by Task 6. The `SplitArgs`
struct is defined now so `--help` and `fte schema` already describe the final
surface.

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo nextest run --all-features --test cli`
Expected: PASS, 8 tests.

If `schema_still_works_alongside_subcommands` fails with a clap error about a
reserved name, the `Commands` enum has gained a `Schema` or `Completions`
variant — remove it; librebar owns those.

- [ ] **Step 7: Update the README usage section**

In `README.md`, replace the fenced block at lines 22-37 and the sentence at
line 39:

````markdown
```bash
# Extract all files in a directory
fte extract --indir path/to/html-files --outdir path/to/output

# Extract specific files
fte extract paper.html chapter.xml article.epub

# Print to stdout
fte extract --stdout paper.html

# Detect format without extracting
fte detect --indir path/to/html-files

# Overwrite existing output
fte extract --force --indir path/to/html-files --outdir path/to/output

# Split a book into per-chapter files
fte split book.epub
```

When run without `--indir`, `fte extract` looks for a config file (`.fte.yaml`,
`.config/fte.yaml`, etc.) walking up from the current directory to find
input/output paths. A bare `fte` prints help.
````

- [ ] **Step 8: Full check and commit**

Run: `just check`
Expected: all pass.

```bash
git add src/main.rs src/cmd src/inputs.rs src/lib.rs tests/cli.rs README.md
git commit -m "feat(cli)!: move extraction under 'fte extract', promote 'fte detect'"
```

---

## Task 4: Chapter marker parsing

**Files:**
- Modify: `src/chapter.rs`

**Interfaces:**
- Consumes: `START_PREFIX`, `END_PREFIX` from Task 1.
- Produces:
  - `fte::chapter::Chunk { id: String, title: String, src: String, body: String }`
  - `fte::chapter::Document { frontmatter: String, preamble: String, chapters: Vec<Chunk> }`
  - `fte::chapter::parse(md: &str) -> Document`

**Background:** Paired markers exist so content between a `chapter-end` and the
next `chapter-start` can be dropped. Converter output leaves stray fragments
there, and a single-delimiter format would silently attach them to the preceding
chapter.

- [ ] **Step 1: Write the failing tests**

Append to the `mod tests` block in `src/chapter.rs`:

```rust
    const SAMPLE: &str = r#"---
id: demo
title: "A Book"
---

# A Book

## Contents

- [One](#one)

<!-- fte:chapter-start id="ch01" title="One" src="OEBPS/c1.xhtml" -->

## One

First body.

<!-- fte:chapter-end id="ch01" -->

stray converter cruft

<!-- fte:chapter-start id="ch02" title="Two" src="OEBPS/c2.xhtml" -->

## Two

Second body.

<!-- fte:chapter-end id="ch02" -->
"#;

    #[test]
    fn parse_splits_frontmatter_preamble_and_chapters() {
        let doc = parse(SAMPLE);
        assert!(doc.frontmatter.contains("id: demo"));
        assert!(doc.frontmatter.contains(r#"title: "A Book""#));
        assert!(!doc.frontmatter.contains("---"));
        assert!(doc.preamble.contains("# A Book"));
        assert!(doc.preamble.contains("- [One](#one)"));
        assert_eq!(doc.chapters.len(), 2);
    }

    #[test]
    fn parse_reads_marker_attributes() {
        let doc = parse(SAMPLE);
        assert_eq!(doc.chapters[0].id, "ch01");
        assert_eq!(doc.chapters[0].title, "One");
        assert_eq!(doc.chapters[0].src, "OEBPS/c1.xhtml");
        assert_eq!(doc.chapters[1].id, "ch02");
    }

    #[test]
    fn parse_keeps_chapter_bodies_and_drops_cruft() {
        let doc = parse(SAMPLE);
        assert_eq!(doc.chapters[0].body.trim(), "## One\n\nFirst body.");
        assert_eq!(doc.chapters[1].body.trim(), "## Two\n\nSecond body.");
        assert!(!doc.chapters[0].body.contains("stray converter cruft"));
        assert!(!doc.chapters[1].body.contains("stray converter cruft"));
    }

    #[test]
    fn parse_of_an_unmarked_document_yields_no_chapters() {
        let doc = parse("---\nid: paper\n---\n\n# Paper\n\nBody.\n");
        assert!(doc.chapters.is_empty());
        assert!(doc.preamble.contains("# Paper"));
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo nextest run --all-features chapter::`
Expected: FAIL — `cannot find function 'parse' in this scope`.

- [ ] **Step 3: Write the implementation**

Add to `src/chapter.rs`, above the test module:

```rust
/// One chapter recovered from a marked-up document.
#[derive(Debug, Clone)]
pub struct Chunk {
    /// Sequential marker id, e.g. `ch03`.
    pub id: String,
    /// Chapter title from the marker.
    pub title: String,
    /// Source spine entry path from the marker.
    pub src: String,
    /// Markdown between the paired markers.
    pub body: String,
}

/// A markdown document decomposed along its chapter markers.
#[derive(Debug, Default, Clone)]
pub struct Document {
    /// YAML frontmatter body, without the `---` delimiters.
    pub frontmatter: String,
    /// Content before the first chapter marker: title heading and TOC.
    pub preamble: String,
    /// Chapters in document order.
    pub chapters: Vec<Chunk>,
}

/// Decompose a document along its chapter markers.
///
/// Content between a `chapter-end` and the next `chapter-start` is discarded:
/// paired markers exist so stray converter output cannot silently attach itself
/// to the preceding chapter.
#[must_use]
pub fn parse(md: &str) -> Document {
    let mut doc = Document::default();
    let mut lines = md.lines().peekable();

    if lines.peek().is_some_and(|l| l.trim() == "---") {
        lines.next();
        let mut front = String::new();
        for line in lines.by_ref() {
            if line.trim() == "---" {
                break;
            }
            front.push_str(line);
            front.push('\n');
        }
        doc.frontmatter = front;
    }

    let mut preamble = String::new();
    let mut current: Option<Chunk> = None;
    let mut seen_first_marker = false;

    for line in lines {
        let trimmed = line.trim();

        if trimmed.starts_with(START_PREFIX) {
            seen_first_marker = true;
            current = Some(Chunk {
                id: attr(trimmed, "id").unwrap_or_default(),
                title: attr(trimmed, "title").unwrap_or_default(),
                src: attr(trimmed, "src").unwrap_or_default(),
                body: String::new(),
            });
            continue;
        }

        if trimmed.starts_with(END_PREFIX) {
            if let Some(chunk) = current.take() {
                doc.chapters.push(chunk);
            }
            continue;
        }

        if let Some(chunk) = current.as_mut() {
            chunk.body.push_str(line);
            chunk.body.push('\n');
        } else if !seen_first_marker {
            preamble.push_str(line);
            preamble.push('\n');
        }
    }

    doc.preamble = preamble.trim().to_string();
    doc
}

/// Read a double-quoted attribute value out of a marker line.
fn attr(line: &str, name: &str) -> Option<String> {
    let needle = format!("{name}=\"");
    let start = line.find(&needle)? + needle.len();
    let rest = &line[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}
```

The `seen_first_marker` flag is what makes cruft-dropping work: once any marker
has been seen, unenclosed lines belong to neither the preamble nor a chapter, so
they fall through both branches and are discarded.

- [ ] **Step 4: Run to verify they pass**

Run: `cargo nextest run --all-features chapter::`
Expected: PASS, 10 tests.

- [ ] **Step 5: Commit**

```bash
git add src/chapter.rs
git commit -m "feat(chapter): parse chapter markers back out of book markdown"
```

---

## Task 5: Filename templates and split config

**Files:**
- Create: `src/template.rs`
- Modify: `src/lib.rs`
- Modify: `src/config.rs:8-13` and `src/config.rs:39-48`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `fte::template::Vars<'a> { book, n, slug, title, src, pad }`
  - `fte::template::render(tpl: &str, vars: &Vars<'_>) -> Result<String, TemplateError>`
  - `fte::template::TemplateError` (implements `std::error::Error`)
  - `fte::config::SplitConfig { subdir, file, front, pad }`

- [ ] **Step 1: Write the failing tests**

Create `src/template.rs` with the test module only:

```rust
//! Filename template expansion for `fte split`.

#[cfg(test)]
mod tests {
    use super::*;

    fn vars() -> Vars<'static> {
        Vars {
            book: "oconnor-2022",
            n: 3,
            slug: "walking-in-the-dark",
            title: "Walking in the Dark",
            src: "ch03",
            pad: 2,
        }
    }

    #[test]
    fn every_token_expands() {
        assert_eq!(
            render("{book}/{n}-{slug}.md", &vars()).unwrap(),
            "oconnor-2022/03-walking-in-the-dark.md"
        );
        assert_eq!(render("{title}", &vars()).unwrap(), "Walking in the Dark");
        assert_eq!(render("{src}.md", &vars()).unwrap(), "ch03.md");
    }

    #[test]
    fn padding_is_configurable() {
        for (pad, expected) in [(1, "3"), (2, "03"), (3, "003"), (4, "0003")] {
            let v = Vars { pad, ..vars() };
            assert_eq!(render("{n}", &v).unwrap(), expected);
        }
    }

    #[test]
    fn the_flat_convention_expands() {
        assert_eq!(
            render("{book}-ch{n}.md", &vars()).unwrap(),
            "oconnor-2022-ch03.md"
        );
    }

    #[test]
    fn literal_text_passes_through() {
        assert_eq!(render("plain.md", &vars()).unwrap(), "plain.md");
        assert_eq!(render("", &vars()).unwrap(), "");
    }

    #[test]
    fn an_unknown_token_is_an_error() {
        let err = render("{chapter}.md", &vars()).unwrap_err();
        assert!(matches!(err, TemplateError::UnknownToken(ref t) if t == "chapter"));
        assert!(err.to_string().contains("chapter"));
    }

    #[test]
    fn an_unterminated_token_is_an_error() {
        assert!(matches!(
            render("{book.md", &vars()).unwrap_err(),
            TemplateError::Unterminated
        ));
    }
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo nextest run --all-features template::`
Expected: FAIL — `cannot find type 'Vars' in this scope`.

- [ ] **Step 3: Write the implementation**

Add above the test module in `src/template.rs`:

```rust
use std::fmt;

/// Values a filename template can reference.
#[derive(Debug, Clone, Copy)]
pub struct Vars<'a> {
    /// Source document stem.
    pub book: &'a str,
    /// Chapter index; 0 is the frontmatter file.
    pub n: usize,
    /// GFM slug of the chapter title.
    pub slug: &'a str,
    /// Raw chapter title.
    pub title: &'a str,
    /// Source spine-entry stem.
    pub src: &'a str,
    /// Zero-padding width applied to `n`.
    pub pad: usize,
}

/// A template that could not be expanded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateError {
    /// The template names a token that does not exist.
    UnknownToken(String),
    /// A `{` was opened and never closed.
    Unterminated,
}

impl fmt::Display for TemplateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownToken(token) => write!(
                f,
                "unknown template token `{{{token}}}`; \
                 expected one of book, n, slug, title, src"
            ),
            Self::Unterminated => write!(f, "unterminated `{{` in template"),
        }
    }
}

impl std::error::Error for TemplateError {}

/// Expand a filename template.
///
/// An unrecognized token is an error rather than a literal passthrough: a
/// typo'd `{chapter}` should fail loudly, not write a file with braces in its
/// name.
///
/// # Errors
///
/// Returns [`TemplateError`] for unknown tokens and unterminated braces.
pub fn render(tpl: &str, vars: &Vars<'_>) -> Result<String, TemplateError> {
    let mut out = String::with_capacity(tpl.len());
    let mut rest = tpl;

    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let close = after.find('}').ok_or(TemplateError::Unterminated)?;
        let token = &after[..close];

        match token {
            "book" => out.push_str(vars.book),
            "n" => out.push_str(&format!("{:0width$}", vars.n, width = vars.pad)),
            "slug" => out.push_str(vars.slug),
            "title" => out.push_str(vars.title),
            "src" => out.push_str(vars.src),
            other => return Err(TemplateError::UnknownToken(other.to_string())),
        }

        rest = &after[close + 1..];
    }

    out.push_str(rest);
    Ok(out)
}
```

Add `pub mod template;` to `src/lib.rs` (after `markdown`).

- [ ] **Step 4: Run to verify they pass**

Run: `cargo nextest run --all-features template::`
Expected: PASS, 6 tests.

- [ ] **Step 5: Add `SplitConfig`**

In `src/config.rs`, add the field to `Config` (lines 8-13):

```rust
pub struct Config {
    pub input_dir: String,
    pub output_dir: String,
    pub split: SplitConfig,
    pub publishers: BTreeMap<String, PublisherProfile>,
    pub fallback: PublisherProfile,
}

/// Naming rules for `fte split` output.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct SplitConfig {
    /// Subdirectory template; empty writes flat into the output directory.
    pub subdir: String,
    /// Chapter filename template.
    pub file: String,
    /// Frontmatter filename template; empty suppresses the file.
    pub front: String,
    /// Zero-padding width for `{n}`.
    pub pad: usize,
}

impl Default for SplitConfig {
    fn default() -> Self {
        Self {
            subdir: "{book}".into(),
            file: "{n}-{slug}.md".into(),
            front: "{n}-frontmatter.md".into(),
            pad: 2,
        }
    }
}
```

Add `split: SplitConfig::default(),` to `Config::default()` (lines 39-48),
after `output_dir`.

- [ ] **Step 6: Verify config still loads**

Run: `just test`
Expected: PASS. `#[serde(default)]` on `Config` means existing config files
without a `split:` key still deserialize.

- [ ] **Step 7: Commit**

```bash
git add src/template.rs src/lib.rs src/config.rs
git commit -m "feat(split): add filename templates and split config"
```

---

## Task 6: `fte split`

**Files:**
- Create: `src/splitter.rs`
- Create: `src/cmd/split.rs`
- Modify: `src/lib.rs`, `src/cmd/mod.rs`, `src/main.rs`
- Modify: `src/epub.rs` (make `heading_slug` public)

**Interfaces:**
- Consumes: `chapter::{Document, Chunk, parse}` (Task 4), `template::{Vars, render}` and `config::SplitConfig` (Task 5), `epub::extract` (existing).
- Produces:
  - `fte::splitter::Options { outdir, subdir, file, front, pad, force }`
  - `fte::splitter::Written { index, title, path, bytes }`
  - `fte::splitter::split(book: &str, doc: &Document, opts: &Options) -> anyhow::Result<Vec<Written>>`
  - `cmd::split::run(args: &SplitArgs, cfg: &Config, quiet: bool) -> anyhow::Result<ExitCode>`

- [ ] **Step 1: Expose `heading_slug`**

In `src/epub.rs:360`, change the signature and add a doc comment:

```rust
/// Generate a GFM-compatible anchor slug from a heading string.
#[must_use]
pub fn heading_slug(text: &str) -> String {
```

- [ ] **Step 2: Write the failing tests**

Create `src/splitter.rs` with the test module only:

```rust
//! Writing a chapter-marked document out as per-chapter files.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chapter;

    const SAMPLE: &str = r#"---
id: demo
title: "A Book"
---

# A Book

<!-- fte:chapter-start id="ch01" title="One" src="OEBPS/c1.xhtml" -->

## One

First body.

<!-- fte:chapter-end id="ch01" -->

<!-- fte:chapter-start id="ch02" title="One" src="OEBPS/c2.xhtml" -->

## One

Second body.

<!-- fte:chapter-end id="ch02" -->
"#;

    fn opts(dir: &std::path::Path) -> Options {
        Options {
            outdir: dir.to_path_buf(),
            subdir: "{book}".into(),
            file: "{n}-{slug}.md".into(),
            front: Some("{n}-frontmatter.md".into()),
            pad: 2,
            force: false,
        }
    }

    #[test]
    fn default_convention_writes_index_and_chapters() {
        let tmp = tempfile::tempdir().unwrap();
        let doc = chapter::parse(SAMPLE);
        let written = split("demo", &doc, &opts(tmp.path())).unwrap();

        assert_eq!(written.len(), 3);
        assert!(tmp.path().join("demo/00-frontmatter.md").exists());
        assert!(tmp.path().join("demo/01-one.md").exists());
        assert!(tmp.path().join("demo/02-one-2.md").exists());
    }

    #[test]
    fn flat_convention_writes_alongside() {
        let tmp = tempfile::tempdir().unwrap();
        let doc = chapter::parse(SAMPLE);
        let o = Options {
            subdir: String::new(),
            file: "{book}-ch{n}.md".into(),
            front: Some("{book}-ch{n}-frontmatter.md".into()),
            ..opts(tmp.path())
        };
        split("demo", &doc, &o).unwrap();

        assert!(tmp.path().join("demo-ch00-frontmatter.md").exists());
        assert!(tmp.path().join("demo-ch01.md").exists());
        assert!(tmp.path().join("demo-ch02.md").exists());
    }

    #[test]
    fn chapter_files_inherit_and_extend_frontmatter() {
        let tmp = tempfile::tempdir().unwrap();
        let doc = chapter::parse(SAMPLE);
        split("demo", &doc, &opts(tmp.path())).unwrap();

        let body = std::fs::read_to_string(tmp.path().join("demo/01-one.md")).unwrap();
        assert!(body.starts_with("---\n"));
        assert!(body.contains("id: demo"));
        assert!(body.contains("chapter: 1"));
        assert!(body.contains(r#"chapter_title: "One""#));
        assert!(body.contains(r#"source: "OEBPS/c1.xhtml""#));
        assert!(body.contains("First body."));
    }

    #[test]
    fn front_none_suppresses_the_index_file() {
        let tmp = tempfile::tempdir().unwrap();
        let doc = chapter::parse(SAMPLE);
        let o = Options {
            front: None,
            ..opts(tmp.path())
        };
        let written = split("demo", &doc, &o).unwrap();

        assert_eq!(written.len(), 2);
        assert!(!tmp.path().join("demo/00-frontmatter.md").exists());
    }

    #[test]
    fn existing_files_are_refused_without_force() {
        let tmp = tempfile::tempdir().unwrap();
        let doc = chapter::parse(SAMPLE);
        split("demo", &doc, &opts(tmp.path())).unwrap();

        let err = split("demo", &doc, &opts(tmp.path())).unwrap_err();
        assert!(err.to_string().contains("exists"));

        let forced = Options {
            force: true,
            ..opts(tmp.path())
        };
        assert!(split("demo", &doc, &forced).is_ok());
    }

    #[test]
    fn a_document_with_no_chapters_is_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let doc = chapter::parse("---\nid: paper\n---\n\n# Paper\n\nBody.\n");
        let err = split("paper", &doc, &opts(tmp.path())).unwrap_err();
        assert!(err.to_string().contains("no chapter markers"));
    }
}
```

`tempfile` is currently a dev-dependency only, which is what these unit tests
need. Do not move it.

- [ ] **Step 3: Run to verify they fail**

Run: `cargo nextest run --all-features splitter::`
Expected: FAIL — `cannot find type 'Options' in this scope`.

- [ ] **Step 4: Write the implementation**

Add above the test module in `src/splitter.rs`:

```rust
use anyhow::{Context, Result, bail};
use std::collections::HashSet;
use std::path::PathBuf;

use crate::chapter::Document;
use crate::epub::heading_slug;
use crate::template::{self, Vars};

/// Where and how to write split output.
#[derive(Debug, Clone)]
pub struct Options {
    /// Base output directory.
    pub outdir: PathBuf,
    /// Subdirectory template; empty writes flat into `outdir`.
    pub subdir: String,
    /// Chapter filename template.
    pub file: String,
    /// Frontmatter filename template; `None` suppresses the file.
    pub front: Option<String>,
    /// Zero-padding width for `{n}`.
    pub pad: usize,
    /// Overwrite existing files.
    pub force: bool,
}

/// One file written by [`split`].
#[derive(Debug, Clone)]
pub struct Written {
    /// Chapter index; 0 is the frontmatter file.
    pub index: usize,
    /// Chapter title, or the book title for the frontmatter file.
    pub title: String,
    /// Path written.
    pub path: PathBuf,
    /// Size in bytes.
    pub bytes: usize,
}

/// Write a chapter-marked document out as per-chapter files.
///
/// # Errors
///
/// Fails when the document has no chapter markers, a template is invalid, a
/// target exists without `force`, or a write fails.
pub fn split(book: &str, doc: &Document, opts: &Options) -> Result<Vec<Written>> {
    if doc.chapters.is_empty() {
        bail!("no chapter markers in {book}; only book-length ePub output carries them");
    }

    let dir = if opts.subdir.is_empty() {
        opts.outdir.clone()
    } else {
        let sub = template::render(
            &opts.subdir,
            &Vars {
                book,
                n: 0,
                slug: "",
                title: "",
                src: "",
                pad: opts.pad,
            },
        )?;
        opts.outdir.join(sub)
    };
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;

    let mut written = Vec::new();
    let mut used: HashSet<PathBuf> = HashSet::new();

    if let Some(front_tpl) = &opts.front {
        let name = template::render(
            front_tpl,
            &Vars {
                book,
                n: 0,
                slug: "frontmatter",
                title: "frontmatter",
                src: "",
                pad: opts.pad,
            },
        )?;
        let path = dir.join(name);
        let mut content = String::new();
        if !doc.frontmatter.is_empty() {
            content.push_str("---\n");
            content.push_str(&doc.frontmatter);
            content.push_str("---\n\n");
        }
        content.push_str(&doc.preamble);
        content.push('\n');
        write_file(&path, &content, opts.force, &mut used)?;
        written.push(Written {
            index: 0,
            title: book.to_string(),
            path,
            bytes: content.len(),
        });
    }

    for (i, chunk) in doc.chapters.iter().enumerate() {
        let n = i + 1;
        let slug = heading_slug(&chunk.title);
        let src_stem = chunk
            .src
            .rsplit('/')
            .next()
            .and_then(|f| f.rsplit_once('.').map(|(stem, _)| stem))
            .unwrap_or("");

        let name = template::render(
            &opts.file,
            &Vars {
                book,
                n,
                slug: &slug,
                title: &chunk.title,
                src: src_stem,
                pad: opts.pad,
            },
        )?;
        let path = disambiguate(dir.join(name), n, &used);

        let mut content = String::new();
        content.push_str("---\n");
        content.push_str(&doc.frontmatter);
        content.push_str(&format!("chapter: {n}\n"));
        content.push_str(&format!(
            "chapter_title: \"{}\"\n",
            chunk.title.replace('\\', "\\\\").replace('"', "\\\"")
        ));
        if !chunk.src.is_empty() {
            content.push_str(&format!("source: \"{}\"\n", chunk.src));
        }
        content.push_str("---\n\n");
        content.push_str(chunk.body.trim());
        content.push('\n');

        write_file(&path, &content, opts.force, &mut used)?;
        written.push(Written {
            index: n,
            title: chunk.title.clone(),
            path,
            bytes: content.len(),
        });
    }

    Ok(written)
}

/// Append the chapter index when two chapters share a title.
fn disambiguate(path: PathBuf, n: usize, used: &HashSet<PathBuf>) -> PathBuf {
    if !used.contains(&path) {
        return path;
    }
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("chapter");
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("md");
    path.with_file_name(format!("{stem}-{n}.{ext}"))
}

fn write_file(
    path: &PathBuf,
    content: &str,
    force: bool,
    used: &mut HashSet<PathBuf>,
) -> Result<()> {
    if !force && path.exists() {
        bail!("{} exists; pass --force to overwrite", path.display());
    }
    std::fs::write(path, content).with_context(|| format!("writing {}", path.display()))?;
    used.insert(path.clone());
    Ok(())
}
```

Add `pub mod splitter;` to `src/lib.rs` (after `markdown`, before `template`).

- [ ] **Step 5: Run to verify they pass**

Run: `cargo nextest run --all-features splitter::`
Expected: PASS, 6 tests.

- [ ] **Step 6: Write the command module**

Create `src/cmd/split.rs`:

```rust
//! `fte split` — book markdown into per-chapter files.

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::ExitCode;

use fte::{chapter, config::Config, epub, splitter};

use crate::SplitArgs;

pub fn run(args: &SplitArgs, cfg: &Config, quiet: bool) -> Result<ExitCode> {
    let book = args
        .input
        .file_stem()
        .and_then(|s| s.to_str())
        .context("input has no usable filename")?
        .to_string();

    let ext = args
        .input
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    // An ePub is extracted in memory rather than round-tripped through disk;
    // `fte extract` is what writes the combined document.
    let md = if ext == "epub" {
        epub::extract(&book, &args.input)?
    } else {
        std::fs::read_to_string(&args.input)
            .with_context(|| format!("reading {}", args.input.display()))?
    };

    let doc = chapter::parse(&md);

    let front = if args.no_front {
        None
    } else {
        let tpl = args.front.clone().unwrap_or_else(|| cfg.split.front.clone());
        if tpl.is_empty() { None } else { Some(tpl) }
    };

    let opts = splitter::Options {
        outdir: args
            .outdir
            .clone()
            .unwrap_or_else(|| PathBuf::from(&cfg.output_dir)),
        subdir: args.subdir.clone().unwrap_or_else(|| cfg.split.subdir.clone()),
        file: args.name.clone().unwrap_or_else(|| cfg.split.file.clone()),
        front,
        pad: cfg.split.pad,
        force: args.force,
    };

    let written = splitter::split(&book, &doc, &opts)?;

    if !quiet {
        for w in &written {
            let kb = w.bytes / 1024;
            eprintln!("  OK   {} ({kb}KB)", w.path.display());
        }
        eprintln!("\nDone: {} files written", written.len());
    }

    Ok(ExitCode::SUCCESS)
}
```

Add `pub mod split;` to `src/cmd/mod.rs`, keeping it alphabetical:

```rust
pub mod detect;
pub mod extract;
pub mod split;
```

Replace the stub arm in `src/main.rs`:

```rust
        Commands::Split(args) => cmd::split::run(args, &cfg, quiet),
```

- [ ] **Step 7: Verify end-to-end by hand**

Run:
```bash
cargo run --quiet -- split --outdir /tmp/fte-split tests/golden/input/books/frankenstein-pg.epub
ls /tmp/fte-split/frankenstein-pg | head
head -12 /tmp/fte-split/frankenstein-pg/01-*.md
```
Expected: a `frankenstein-pg/` directory containing `00-frontmatter.md` and
numbered chapter files, each opening with inherited frontmatter plus `chapter:`,
`chapter_title:`, and `source:`.

- [ ] **Step 8: Full check and commit**

Run: `just check`
Expected: all pass.

```bash
git add src/splitter.rs src/cmd/split.rs src/cmd/mod.rs src/main.rs src/lib.rs src/epub.rs
git commit -m "feat(split): add 'fte split' for per-chapter book output"
```

---

## Task 7: Split integration tests and goldens

**Files:**
- Create: `tests/split.rs`

**Interfaces:**
- Consumes: `common::skeleton` (Task 2), the `fte split` binary (Task 6).
- Produces: nothing later tasks depend on.

- [ ] **Step 1: Write the tests**

Create `tests/split.rs`:

```rust
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

    assert!(tmp.path().join("frankenstein-pg-ch00-frontmatter.md").exists());
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
    run().assert().failure().stderr(predicate::str::contains("exists"));

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

    let a = std::fs::read_to_string(
        one_shot.path().join("frankenstein-pg/00-frontmatter.md"),
    )
    .unwrap();
    let b = std::fs::read_to_string(
        two_step.path().join("frankenstein-pg/00-frontmatter.md"),
    )
    .unwrap();
    assert_eq!(common::skeleton(&a), common::skeleton(&b));
}
```

- [ ] **Step 2: Run the tests**

Run: `cargo nextest run --all-features --test split`
Expected: PASS, 7 tests.

If `a_paper_epub_has_no_chapters_to_split` fails because the fixture is
detected as a book, pick a different paper epub from `tests/golden/input/` —
`tandf-epub-readership-awareness.epub` is the alternative.

- [ ] **Step 3: Commit**

```bash
git add tests/split.rs
git commit -m "test(split): cover naming conventions, collisions, and one-shot epub split"
```

---

## Task 8: Declared error kinds and exit codes

**Files:**
- Create: `src/errors.rs`
- Modify: `src/lib.rs`, `src/main.rs`, `src/cmd/*.rs`
- Modify: `tests/cli.rs`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `fte::errors::Kind` with `code()`, `as_str()`, `retryable()`
  - `fte::errors::AppError { kind, message, hint }` implementing `std::error::Error`
  - `fte::errors::AppError::new(kind, message)` and `.hint(text)`
  - `fte::errors::emit(err: &AppError, json: bool)`
  - `fte::errors::PARTIAL_FAILURE: u8 = 1`

**Background:** CLIspec 0.3 requires an explicit exit code per error kind, and
forbids sharing a code between an error and an outcome. Exit 1 is reserved for
the `partial_failure` outcome — a run where some inputs succeeded and others
did not — so error kinds start at 2.

- [ ] **Step 1: Write the failing tests**

Create `src/errors.rs` with the test module only:

```rust
//! Declared error kinds, exit codes, and structured error rendering.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_has_a_distinct_code_above_the_outcome() {
        let kinds = Kind::ALL;
        let mut codes: Vec<u8> = kinds.iter().map(|k| k.code()).collect();
        codes.sort_unstable();
        let unique = codes.len();
        codes.dedup();
        assert_eq!(codes.len(), unique, "duplicate exit codes");
        assert!(
            codes.iter().all(|&c| c > PARTIAL_FAILURE),
            "an error code collides with the partial_failure outcome"
        );
    }

    #[test]
    fn codes_match_the_spec() {
        assert_eq!(Kind::Usage.code(), 2);
        assert_eq!(Kind::NotFound.code(), 3);
        assert_eq!(Kind::UnsupportedFormat.code(), 4);
        assert_eq!(Kind::ExtractionFailed.code(), 5);
        assert_eq!(Kind::OutputExists.code(), 6);
        assert_eq!(Kind::IoError.code(), 7);
        assert_eq!(Kind::NoChapters.code(), 8);
        assert_eq!(Kind::ConfigError.code(), 9);
    }

    #[test]
    fn kind_names_are_snake_case() {
        for kind in Kind::ALL {
            let name = kind.as_str();
            assert!(
                name.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
                "{name} is not snake_case"
            );
        }
    }

    #[test]
    fn only_io_errors_are_retryable() {
        assert!(Kind::IoError.retryable());
        assert!(!Kind::NotFound.retryable());
        assert!(!Kind::Usage.retryable());
    }

    #[test]
    fn json_rendering_is_a_single_line_envelope() {
        let err = AppError::new(Kind::NotFound, "input 'foo.html' not found")
            .hint("check --indir");
        let line = err.to_json_line();
        assert!(!line.contains('\n'));

        let parsed: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(parsed["kind"], "not_found");
        assert_eq!(parsed["message"], "input 'foo.html' not found");
        assert_eq!(parsed["hint"], "check --indir");
    }

    #[test]
    fn a_hintless_error_omits_the_field() {
        let line = AppError::new(Kind::IoError, "disk full").to_json_line();
        let parsed: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert!(parsed.get("hint").is_none());
    }
}
```

- [ ] **Step 2: Add the runtime dependency**

In `Cargo.toml`, under `[dependencies]`, alphabetically after `serde`:

```toml
serde_json = "1.0"
```

`serde_json` 1.0.151 is already in `Cargo.lock` via librebar, so this adds no
new transitive dependency.

Run: `cargo deny --all-features --config .config/deny.toml check`
Expected: PASS.

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo nextest run --all-features errors::`
Expected: FAIL — `cannot find type 'Kind' in this scope`.

- [ ] **Step 4: Write the implementation**

Add above the test module in `src/errors.rs`:

```rust
use std::fmt;

/// Exit code for the `partial_failure` outcome.
///
/// CLIspec 0.3 separates outcomes from errors: a run where some inputs
/// succeeded and others failed is data, not a fault, so it keeps exit 1 and
/// error kinds start at 2.
pub const PARTIAL_FAILURE: u8 = 1;

/// A declared error kind.
///
/// Every kind carries a stable snake_case name and a distinct exit code, both
/// published through `fte schema`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Kind {
    /// Bad arguments, unknown subcommand, or a bare invocation.
    Usage,
    /// An input file or ID did not resolve.
    NotFound,
    /// No handler matches the detected format.
    UnsupportedFormat,
    /// Parsing produced no usable body content.
    ExtractionFailed,
    /// A target file exists and `--force` was not given.
    OutputExists,
    /// A read or write failed.
    IoError,
    /// `split` was given a document with no chapter markers.
    NoChapters,
    /// Malformed config, or an unknown filename-template token.
    ConfigError,
}

impl Kind {
    /// Every declared kind, for schema emission and exhaustiveness tests.
    pub const ALL: [Self; 8] = [
        Self::Usage,
        Self::NotFound,
        Self::UnsupportedFormat,
        Self::ExtractionFailed,
        Self::OutputExists,
        Self::IoError,
        Self::NoChapters,
        Self::ConfigError,
    ];

    /// The process exit code for this kind.
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::Usage => 2,
            Self::NotFound => 3,
            Self::UnsupportedFormat => 4,
            Self::ExtractionFailed => 5,
            Self::OutputExists => 6,
            Self::IoError => 7,
            Self::NoChapters => 8,
            Self::ConfigError => 9,
        }
    }

    /// The stable machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Usage => "usage",
            Self::NotFound => "not_found",
            Self::UnsupportedFormat => "unsupported_format",
            Self::ExtractionFailed => "extraction_failed",
            Self::OutputExists => "output_exists",
            Self::IoError => "io_error",
            Self::NoChapters => "no_chapters",
            Self::ConfigError => "config_error",
        }
    }

    /// Whether retrying the same request can succeed.
    #[must_use]
    pub const fn retryable(self) -> bool {
        matches!(self, Self::IoError)
    }

    /// Human-readable description, published in the schema.
    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Usage => "Invalid arguments, unrecognized command, or bad flag value.",
            Self::NotFound => "An input file or ID could not be found.",
            Self::UnsupportedFormat => "No extraction handler matches the detected format.",
            Self::ExtractionFailed => "Parsing produced no usable body content.",
            Self::OutputExists => "An output file already exists and --force was not given.",
            Self::IoError => "A read or write operation failed.",
            Self::NoChapters => "The document contains no chapter markers to split on.",
            Self::ConfigError => "Configuration or a filename template is invalid.",
        }
    }
}

/// A failure carrying a declared kind.
#[derive(Debug, Clone)]
pub struct AppError {
    /// Declared kind; determines the exit code.
    pub kind: Kind,
    /// Human-readable message.
    pub message: String,
    /// Optional actionable hint.
    pub hint: Option<String>,
}

impl AppError {
    /// Create an error of the given kind.
    pub fn new(kind: Kind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            hint: None,
        }
    }

    /// Attach an actionable hint.
    #[must_use]
    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Render the CLIspec error envelope as a single JSON line.
    #[must_use]
    pub fn to_json_line(&self) -> String {
        let mut map = serde_json::Map::new();
        map.insert("kind".into(), self.kind.as_str().into());
        map.insert("message".into(), self.message.clone().into());
        if let Some(hint) = &self.hint {
            map.insert("hint".into(), hint.clone().into());
        }
        serde_json::Value::Object(map).to_string()
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)?;
        if let Some(hint) = &self.hint {
            write!(f, " ({hint})")?;
        }
        Ok(())
    }
}

impl std::error::Error for AppError {}

/// Print a failure to stderr in the selected format.
///
/// JSON mode prints the envelope as the last line of stderr, per CLIspec 0.3.
pub fn emit(err: &AppError, json: bool) {
    if json {
        eprintln!("{}", err.to_json_line());
    } else {
        eprintln!("error: {err}");
    }
}
```

Add `pub mod errors;` to `src/lib.rs` (after `epub`).

- [ ] **Step 5: Run to verify they pass**

Run: `cargo nextest run --all-features errors::`
Expected: PASS, 6 tests.

- [ ] **Step 6: Route command failures through `AppError`**

In `src/main.rs`, replace the `match &cli.command` block and the `main`
signature so failures map to declared codes:

```rust
fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(err) => {
            let json = false;
            fte::errors::emit(&err, json);
            ExitCode::from(err.kind.code())
        }
    }
}

fn run() -> std::result::Result<ExitCode, fte::errors::AppError> {
    let cli: Cli = librebar::cli::parse();

    if cli
        .common
        .apply(VERSION)
        .map_err(|e| AppError::new(Kind::ConfigError, e.to_string()))?
        .is_exit()
    {
        return Ok(ExitCode::SUCCESS);
    }

    let cwd = std::env::current_dir()
        .map_err(|e| AppError::new(Kind::IoError, format!("reading current directory: {e}")))?;
    let cwd_utf8 = cwd
        .to_str()
        .ok_or_else(|| AppError::new(Kind::ConfigError, "cwd is not valid UTF-8"))?;

    let mut loader = librebar::config::ConfigLoader::new("fte").with_project_search(cwd_utf8);
    if let Some(path) = cli
        .common
        .config_path()
        .map_err(|e| AppError::new(Kind::ConfigError, e.to_string()))?
    {
        loader = loader.with_file(&path);
    }
    let (cfg, _sources) = loader
        .load::<config::Config>()
        .map_err(|e| AppError::new(Kind::ConfigError, e.to_string()))?;

    let quiet = cli.common.quiet;
    let verbose = cli.common.verbose > 0;

    match &cli.command {
        Commands::Extract(args) => cmd::extract::run(args, &cfg, quiet, verbose),
        Commands::Detect(args) => cmd::detect::run(args, &cfg),
        Commands::Split(args) => cmd::split::run(args, &cfg, quiet),
    }
}
```

Add the imports to `src/main.rs`:

```rust
use fte::errors::{AppError, Kind};
```

Change each command module's return type from `anyhow::Result<ExitCode>` to
`std::result::Result<ExitCode, AppError>` and map their failures:

- `src/cmd/extract.rs` and `src/cmd/detect.rs`: `inputs::resolve` failure →
  `Kind::NotFound`; `fs::read_to_string` / `fs::write` / `create_dir_all`
  failure → `Kind::IoError`. The per-file `Err(e)` arm inside the loop keeps
  printing `FAIL {id}: {e}` and incrementing `fail`; it does not abort.
- `src/cmd/split.rs`: `epub::extract` failure → `Kind::ExtractionFailed`;
  `read_to_string` failure → `Kind::IoError`; a `splitter::split` error whose
  message contains `no chapter markers` → `Kind::NoChapters`; one containing
  `unknown template token` or `unterminated` → `Kind::ConfigError`; one
  containing `exists` → `Kind::OutputExists`; anything else → `Kind::IoError`.

Keep `ExitCode::from(fte::errors::PARTIAL_FAILURE)` as the extract/detect return
when `fail > 0`, replacing `ExitCode::FAILURE`.

- [ ] **Step 7: Add exit-code assertions**

Append to `tests/cli.rs`:

```rust
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
```

- [ ] **Step 8: Run and commit**

Run: `just check`
Expected: all pass.

```bash
git add Cargo.toml Cargo.lock src/errors.rs src/lib.rs src/main.rs src/cmd tests/cli.rs
git commit -m "feat(cli): declare error kinds with distinct exit codes"
```

---

## Task 9: JSON output envelopes

**Files:**
- Create: `src/output.rs`
- Modify: `src/lib.rs`, `src/main.rs`, `src/cmd/*.rs`
- Modify: `tests/cli.rs`

**Interfaces:**
- Consumes: `errors::AppError` (Task 8).
- Produces:
  - `fte::output::Render` (`Text` | `Json`)
  - `fte::output::emit_items<T: Serialize>(rows: &[T], render: Render, text: impl Fn(&T) -> String)`

**Background:** CLIspec 0.3 requires collections to be wrapped in
`{"items": [...]}` rather than returned as a bare array, so fields can be added
to the envelope without a breaking change.

`--stdout` is deliberately exempt. librebar's `--format auto` resolves to JSON
when stdout is not a terminal, so `fte extract --stdout paper.html > out.md`
would otherwise wrap the markdown in JSON — a trap that only becomes reachable
once JSON output exists at all. `--stdout` is declared `output_kind: opaque`
with `media_type: text/markdown` and always emits raw markdown.

- [ ] **Step 1: Write the failing test**

Create `src/output.rs` with the test module only:

```rust
//! Rendering command results as text or a CLIspec `items` envelope.

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;

    #[derive(Serialize)]
    struct Row {
        id: String,
        format: String,
    }

    #[test]
    fn json_wraps_rows_in_an_items_envelope() {
        let rows = vec![Row {
            id: "paper".into(),
            format: "jats".into(),
        }];
        let json = to_json(&rows);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed["items"].is_array());
        assert_eq!(parsed["items"][0]["id"], "paper");
    }

    #[test]
    fn an_empty_result_is_still_an_envelope() {
        let rows: Vec<Row> = Vec::new();
        let parsed: serde_json::Value = serde_json::from_str(&to_json(&rows)).unwrap();
        assert!(parsed["items"].is_array());
        assert_eq!(parsed["items"].as_array().unwrap().len(), 0);
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo nextest run --all-features output::`
Expected: FAIL — `cannot find function 'to_json' in this scope`.

- [ ] **Step 3: Write the implementation**

Add above the test module in `src/output.rs`:

```rust
use serde::Serialize;

/// Which rendering a command should produce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Render {
    /// Human-readable lines.
    Text,
    /// A CLIspec `items` envelope.
    Json,
}

/// Serialize rows into the CLIspec collection envelope.
///
/// The wrapper is required rather than a bare array so the envelope can gain
/// fields without breaking consumers.
#[must_use]
pub fn to_json<T: Serialize>(rows: &[T]) -> String {
    let mut map = serde_json::Map::new();
    map.insert(
        "items".into(),
        serde_json::to_value(rows).unwrap_or(serde_json::Value::Array(Vec::new())),
    );
    serde_json::Value::Object(map).to_string()
}

/// Print rows to stdout in the selected rendering.
pub fn emit_items<T: Serialize>(rows: &[T], render: Render, text: impl Fn(&T) -> String) {
    match render {
        Render::Json => println!("{}", to_json(rows)),
        Render::Text => {
            for row in rows {
                println!("{}", text(row));
            }
        }
    }
}
```

Add `pub mod output;` to `src/lib.rs` (after `markdown`).

- [ ] **Step 4: Run to verify it passes**

Run: `cargo nextest run --all-features output::`
Expected: PASS, 2 tests.

- [ ] **Step 5: Resolve the render mode in `main.rs`**

In `run()`, after computing `verbose`:

```rust
    use librebar::cli::ResolvedOutputFormat;
    let render = match cli.common.output_format() {
        ResolvedOutputFormat::Json => fte::output::Render::Json,
        _ => fte::output::Render::Text,
    };
```

Pass `render` to each command's `run`, and use it in the error path so a JSON
run gets a JSON error envelope. Restructure `main` to resolve the format before
dispatch:

```rust
fn main() -> ExitCode {
    let json_errors = matches!(
        std::env::args().any(|a| a == "--json" || a == "--format=json"),
        true
    ) || !std::io::IsTerminal::is_terminal(&std::io::stdout());

    match run() {
        Ok(code) => code,
        Err(err) => {
            fte::errors::emit(&err, json_errors);
            ExitCode::from(err.kind.code())
        }
    }
}
```

The pre-parse sniff in `main` exists because `run()` can fail before clap has
produced a `CommonArgs` — a config error, for instance. Inside `run()`, prefer
the resolved `render` value.

- [ ] **Step 6: Emit rows from each command**

In `src/cmd/detect.rs`, collect rows instead of printing inline:

```rust
#[derive(serde::Serialize)]
struct DetectRow {
    id: String,
    path: String,
    format: String,
}
```

Build a `Vec<DetectRow>` in the loop, then:

```rust
    output::emit_items(&rows, render, |r| format!("{}: {}", r.id, r.format));
```

In `src/cmd/extract.rs`, add:

```rust
#[derive(serde::Serialize)]
struct ExtractRow {
    id: String,
    source_format: String,
    output_path: Option<String>,
    bytes: usize,
    status: &'static str,
}
```

`status` is `"extracted"`, `"skipped"`, or `"failed"`. Push a row per input.
When `args.stdout` is set, skip the envelope entirely and keep the existing
`println!("{md}")` — that is the declared opaque path. Otherwise call
`output::emit_items` after the loop, with the text closure returning the
existing `  OK   {id}.md ({kb}KB)` line. Move that progress printing out of the
loop so text mode prints rows once, from one place.

In `src/cmd/split.rs`, add:

```rust
#[derive(serde::Serialize)]
struct SplitRow {
    index: usize,
    title: String,
    path: String,
    bytes: usize,
}
```

Map `splitter::Written` into it and emit.

- [ ] **Step 7: Add output-shape tests**

Append to `tests/cli.rs`:

```rust
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
fn stdout_stays_raw_markdown_even_when_piped() {
    let out = fte()
        .args(["extract", "--stdout", "tests/golden/input/bmc-short.html"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let text = String::from_utf8(out).unwrap();
    assert!(text.starts_with("---\n"), "expected raw markdown frontmatter");
    assert!(serde_json::from_str::<serde_json::Value>(&text).is_err());
}
```

`assert_cmd` runs the binary with piped stdout, so
`stdout_stays_raw_markdown_even_when_piped` is a genuine regression test for the
auto-format trap.

Add `serde_json = "1.0"` to `[dev-dependencies]` if the test file needs it
independently — it is already a runtime dependency, so integration tests can use
it via `fte`'s dependency only if re-exported. Add it explicitly to
`[dev-dependencies]` to keep the test file self-contained.

- [ ] **Step 8: Run and commit**

Run: `just check`
Expected: all pass.

```bash
git add Cargo.toml Cargo.lock src/output.rs src/lib.rs src/main.rs src/cmd tests/cli.rs
git commit -m "feat(cli): emit CLIspec items envelopes in JSON mode"
```

---

## Task 10: Schema metadata and the clispec gate

**Files:**
- Modify: `src/main.rs`
- Modify: `.justfile`
- Modify: `README.md`

**Interfaces:**
- Consumes: `errors::Kind::ALL` (Task 8).
- Produces: nothing later tasks depend on.

**Background:** `librebar::cli::parse()` uses empty schema metadata.
`parse_with(metadata)` is the same entry point with application facts attached
(`librebar/src/cli/parse.rs:166`). librebar 0.6 supports `errors`, `outcomes`,
and per-command `mutating` / `stability` / `output_fields` / `example`. It does
**not** support `effects`, `cardinality`, or `output_kind` — those are the four
scorer checks parked on librebar 0.7 and documented in the spec. Do not attempt
to work around this by hand-rolling a schema document.

- [ ] **Step 1: Declare schema metadata**

In `src/main.rs`, replace `librebar::cli::parse()` with a metadata-carrying call:

```rust
use librebar::cli::{CommandExample, CommandMetadata, ErrorMetadata, OutcomeMetadata, OutputField, SchemaMetadata, Stability};

fn schema_metadata() -> SchemaMetadata {
    let mut metadata = SchemaMetadata::new();

    for kind in fte::errors::Kind::ALL {
        metadata = metadata.error(
            ErrorMetadata::new(kind.as_str())
                .exit_code(kind.code())
                .retryable(kind.retryable())
                .description(kind.description()),
        );
    }

    metadata = metadata.outcome(
        OutcomeMetadata::new(fte::errors::PARTIAL_FAILURE, "partial_failure")
            .description("Some inputs were processed and others failed."),
    );

    metadata
        .command(
            "extract",
            CommandMetadata::new()
                .mutating(true)
                .stability(Stability::Stable)
                .example(CommandExample::new(["extract", "--stdout", "paper.html"]))
                .output_field(OutputField::new("id", "string").description("Input file stem"))
                .output_field(
                    OutputField::new("source_format", "string")
                        .description("Detected publisher format"),
                )
                .output_field(
                    OutputField::new("output_path", "string")
                        .description("Path written, absent with --stdout"),
                )
                .output_field(OutputField::new("bytes", "integer").description("Markdown size"))
                .output_field(
                    OutputField::new("status", "string")
                        .description("extracted | skipped | failed"),
                ),
        )
        .command(
            "detect",
            CommandMetadata::new()
                .mutating(false)
                .stability(Stability::Stable)
                .example(CommandExample::new(["detect", "paper.html"]))
                .output_field(OutputField::new("id", "string").description("Input file stem"))
                .output_field(OutputField::new("path", "string").description("Resolved path"))
                .output_field(
                    OutputField::new("format", "string").description("Detected format name"),
                ),
        )
        .command(
            "split",
            CommandMetadata::new()
                .mutating(true)
                .stability(Stability::Stable)
                .example(CommandExample::new(["split", "book.epub"]))
                .output_field(
                    OutputField::new("index", "integer")
                        .description("Chapter number; 0 is the frontmatter file"),
                )
                .output_field(OutputField::new("title", "string").description("Chapter title"))
                .output_field(OutputField::new("path", "string").description("Path written"))
                .output_field(OutputField::new("bytes", "integer").description("File size")),
        )
}
```

Then in `run()`:

```rust
    let cli: Cli = librebar::cli::parse_with(schema_metadata());
```

Verified against librebar 0.6: `CommandMetadata` exposes exactly `new`,
`mutating`, `stability`, `output_field` (singular, called once per field), and
`example`. Every type imported above is re-exported from `librebar::cli`
(`librebar/src/cli.rs:28-35`). There is no `effects`, `cardinality`, or
`output_kind` builder — that absence is the librebar 0.7 gap, not a lookup
failure.

- [ ] **Step 2: Verify the schema declares errors**

Run:
```bash
cargo build --quiet
./target/debug/fte schema | grep -c '"kind"'
./target/debug/fte schema | grep -o '"partial_failure"'
```
Expected: 8 error kinds and one `partial_failure` outcome.

- [ ] **Step 3: Add the clispec recipe**

Append to `.justfile`:

```just
# Score the built binary against The CLI Spec.
#
# Four checks are blocked by librebar 0.6, which emits CLIspec 0.2 and has no
# `effects` or `cardinality` fields — see
# record/superpowers/specs/2026-08-14-cli-restructure-and-chapter-splitting-design.md
clispec-floor := "20"

clispec:
  @cargo build --quiet
  @clispec score -o json ./target/debug/fte > target/clispec.json
  @jq -e '.score >= {{clispec-floor}}' target/clispec.json > /dev/null || { \
      echo "clispec score below floor of {{clispec-floor}}:" >&2; \
      jq -r '.principles[].checks[] | select(.passed == false) | "  FAIL \(.name): \(.detail // "no detail")"' target/clispec.json >&2; \
      exit 1; \
  }
  @jq -r '"clispec \(.score)/\(.max) (\(.percentage)%) \(.grade)"' target/clispec.json
```

This stays out of `just check` deliberately: it needs the `clispec` binary
(`brew install`) and `jq`, neither of which CI installs today.

- [ ] **Step 4: Run the gate**

Run: `just clispec`
Expected: `clispec 20/24 (83%) Good` or better.

If the score is below 20, read the failing checks the recipe prints. Common
causes, in order of likelihood:
- A command still prints progress chatter to stdout — move it to stderr.
- `--format json` is not producing an `items` envelope for some command.
- The structured error envelope is not the last line of stderr.

If the only failures are *Validates against clispec v0.3*, *Effects on all
commands*, *Effects declarations*, and *Cardinality declarations*, the target is
met — those four are librebar's.

- [ ] **Step 5: Document the new surface**

In `README.md`, after the "Shell completions and machine-readable help" section,
add:

````markdown
### Splitting books into chapters

Book-length ePub output carries chapter boundary markers, so it can be split
into per-chapter files:

```bash
# One shot: epub straight to chapter files
fte split book.epub

# Or split a document you already extracted
fte extract book.epub && fte split ref/epub-md/book.md
```

Filenames come from templates, configurable in `.fte.yaml`:

```yaml
split:
  subdir: "{book}"              # "" writes flat into the output directory
  file: "{n}-{slug}.md"
  front: "{n}-frontmatter.md"   # "" suppresses it
  pad: 2
```

Available tokens are `{book}`, `{n}`, `{slug}`, `{title}`, and `{src}`. An
unrecognized token is an error, not a literal. `--subdir`, `--name`, `--front`,
and `--no-front` override the config per run.

### Exit codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | `partial_failure` — some inputs succeeded, others failed |
| 2 | `usage` |
| 3 | `not_found` |
| 4 | `unsupported_format` |
| 5 | `extraction_failed` |
| 6 | `output_exists` |
| 7 | `io_error` |
| 8 | `no_chapters` |
| 9 | `config_error` |

With `--format json`, failures print a single-line envelope to stderr:

```json
{"kind":"not_found","message":"input 'foo.html' not found","hint":"check --indir"}
```
````

Also update the "Book-length ePubs" subsection to mention that output carries
chapter markers.

- [ ] **Step 6: Final check and commit**

Run: `just check && just clispec`
Expected: all pass, score ≥ 20.

```bash
git add src/main.rs .justfile README.md
git commit -m "feat(cli): declare schema metadata and add clispec score gate"
```

---

## Self-Review Notes

**Spec coverage:**

| Spec section | Task |
|--------------|------|
| 1. Command surface | 3 |
| 2. Chapter markers (attributes, sanitization, placement) | 1 |
| 3. `fte split` — input, templates, frontmatter, collisions | 4, 5, 6 |
| 4. Error kinds, partial_failure outcome | 8 |
| 4. Structured output, `--stdout` opaque, text mode | 9 |
| 4. Schema metadata, expected score | 10 |
| Book golden tests | 2 |
| Testing — unit, integration, tooling | 1, 4, 5, 6, 7, 8, 9, 10 |
| librebar 0.7 follow-up | Out of scope by design; recorded in the fleet hub |

**API verification:** every librebar call in this plan was checked against
librebar 0.6 source, not recalled: `parse_with` (`cli/parse.rs:166`),
`SchemaMetadata::{error, outcome, command}` (`cli/schema.rs:319-352`),
`CommandMetadata::{mutating, stability, output_field, example}`
(`cli/schema.rs:363-400`), `ErrorMetadata::{exit_code, retryable, description}`
(`cli/schema.rs:245-277`), and the `librebar::cli` re-export list
(`cli.rs:28-35`). The early return for `schema`/`completions` before
`T::from_arg_matches_mut` (`cli/parse.rs:110-144`) is what makes a required
subcommand enum safe in Task 3.

**Type consistency:** `chapter::Document`/`Chunk` (Task 4) are consumed by
`splitter::split` (Task 6) and `cmd::split` (Task 6). `template::Vars`/`render`
(Task 5) are consumed by `splitter` (Task 6). `errors::Kind::ALL` (Task 8) is
consumed by `schema_metadata` (Task 10). `common::skeleton` (Task 2) is consumed
by `tests/split.rs` (Task 7). `heading_slug` becomes public in Task 6 Step 1
before `splitter` uses it in Step 4.

**Ordering constraint:** Task 3 rewrites `main.rs` and Tasks 8–9 rewrite parts
of it again. Do not reorder — Task 3 establishes the subcommand surface the
scorer needs, and Tasks 8–9 layer error and output handling onto it. Running
`just clispec` after Task 3 should already show a jump from 11/24 to roughly
16/24; that is the checkpoint confirming the restructure did its job.
