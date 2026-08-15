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
