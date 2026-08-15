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
    let digest = hasher.finalize();
    // `sha2` 0.11's output type no longer implements `LowerHex`
    // (generic-array's blanket impl didn't carry over to hybrid-array), so
    // hex-encode manually instead of `{:x}`.
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    out.push_str(&format!("sha256 {hex}\n"));
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
            out.push_str(&format!(
                "chapter-start {}\n",
                rest.trim_end_matches("-->").trim()
            ));
        } else if let Some(rest) = trimmed.strip_prefix(END_PREFIX) {
            out.push_str(&format!(
                "chapter-end {}\n",
                rest.trim_end_matches("-->").trim()
            ));
        } else if trimmed.starts_with('#') {
            let level = trimmed.chars().take_while(|&c| c == '#').count();
            let text = trimmed.trim_start_matches('#').trim();
            out.push_str(&format!("h{level} {text}\n"));
        } else if is_toc_entry(trimmed) {
            let depth = (line.len() - line.trim_start().len()) / 2;
            let slug = trimmed
                .rsplit_once("(#")
                .map_or("", |(_, s)| s.trim_end_matches(')'));
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
