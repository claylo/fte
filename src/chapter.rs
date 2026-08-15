//! Chapter boundary markers in generated book markdown.
//!
//! Book output wraps each chapter in a paired HTML comment so `fte split`
//! can recover chapter boundaries without re-parsing the source ePub.

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
    /// Set when the document could not be cleanly decomposed: a
    /// `chapter-start` with no matching `chapter-end`, a `chapter-start`
    /// that arrives before the previous chapter's `chapter-end`, or a marker
    /// whose attribute values could not be parsed unambiguously. `split`
    /// treats any of these as corrupt input and refuses to emit a silently
    /// truncated tree.
    pub unterminated: Option<String>,
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
            if let Some(chunk) = current.take() {
                // A second start arrived before this chunk's chapter-end:
                // the in-progress chapter never closed and its body is lost.
                doc.unterminated.get_or_insert_with(|| {
                    format!("chapter \"{}\" has no matching chapter-end", chunk.id)
                });
            }
            if !well_formed(trimmed) {
                doc.unterminated
                    .get_or_insert_with(|| format!("malformed chapter-start marker: {trimmed}"));
            }
            current = Some(Chunk {
                id: attr(trimmed, "id").unwrap_or_default(),
                title: attr(trimmed, "title").unwrap_or_default(),
                src: attr(trimmed, "src").unwrap_or_default(),
                body: String::new(),
            });
            continue;
        }

        if trimmed.starts_with(END_PREFIX) {
            if !well_formed(trimmed) {
                doc.unterminated
                    .get_or_insert_with(|| format!("malformed chapter-end marker: {trimmed}"));
            }
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

    if let Some(chunk) = current {
        // EOF with an open chunk: the trailing chapter never closed.
        doc.unterminated
            .get_or_insert_with(|| format!("chapter \"{}\" has no matching chapter-end", chunk.id));
    }

    doc.preamble = preamble.trim().to_string();
    doc
}

/// Whether a marker line's attribute values can be parsed unambiguously.
///
/// Every generated marker has an even number of `"`: three quoted attributes
/// (six quotes) on a `chapter-start`, one (two quotes) on a `chapter-end`.
/// `sanitize_attr` never emits a raw `"` in a value, but a human hand-editing
/// generated markdown can introduce one — `attr` would then silently
/// truncate at the stray quote instead of the real terminator. An odd count
/// is the cheap, reliable signal that happened.
fn well_formed(line: &str) -> bool {
    line.matches('"').count().is_multiple_of(2)
}

/// Read a double-quoted attribute value out of a marker line.
fn attr(line: &str, name: &str) -> Option<String> {
    let needle = format!("{name}=\"");
    let start = line.find(&needle)? + needle.len();
    let rest = &line[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

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
        assert!(doc.unterminated.is_none());
    }

    #[test]
    fn a_well_formed_document_has_no_unterminated_flag() {
        assert!(parse(SAMPLE).unterminated.is_none());
    }

    #[test]
    fn eof_with_an_open_chunk_is_flagged_unterminated() {
        // Two chapters; the second is missing its chapter-end.
        let md = r#"<!-- fte:chapter-start id="ch01" title="One" src="a.xhtml" -->

## One

body one

<!-- fte:chapter-end id="ch01" -->

<!-- fte:chapter-start id="ch02" title="Two" src="b.xhtml" -->

## Two

body two
"#;
        let doc = parse(md);
        assert_eq!(
            doc.chapters.len(),
            1,
            "the unterminated chapter must not appear as a chapter"
        );
        assert!(!doc.chapters.iter().any(|c| c.body.contains("body two")));
        let reason = doc
            .unterminated
            .expect("EOF with an open chunk must be flagged");
        assert!(reason.contains("ch02"));
    }

    #[test]
    fn a_second_start_before_the_matching_end_is_flagged_unterminated() {
        let md = r#"<!-- fte:chapter-start id="ch01" title="One" src="a.xhtml" -->

## One

body one

<!-- fte:chapter-start id="ch02" title="Two" src="b.xhtml" -->

## Two

body two

<!-- fte:chapter-end id="ch02" -->
"#;
        let doc = parse(md);
        // ch01's body is overwritten mid-parse; only ch02 survives as a chapter.
        assert_eq!(doc.chapters.len(), 1);
        assert_eq!(doc.chapters[0].id, "ch02");
        assert!(!doc.chapters.iter().any(|c| c.body.contains("body one")));
        let reason = doc
            .unterminated
            .expect("a start before the matching end must be flagged");
        assert!(reason.contains("ch01"));
    }

    #[test]
    fn a_raw_quote_in_a_hand_edited_marker_is_flagged_unterminated() {
        // A single unescaped `"` inside the title value (not a matched pair)
        // leaves an odd number of quotes on the line, which is exactly the
        // condition under which `attr` truncates a value at the stray quote
        // instead of the real terminator.
        let md = r#"<!-- fte:chapter-start id="ch01" title="A "Quoted Title" src="a.xhtml" -->

## One

body one

<!-- fte:chapter-end id="ch01" -->
"#;
        let doc = parse(md);
        let reason = doc
            .unterminated
            .expect("an odd number of quotes on a marker line must be flagged");
        assert!(reason.contains("malformed"));
    }
}
