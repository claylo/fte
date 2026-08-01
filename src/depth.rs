//! Limits on document nesting.
//!
//! Input documents are untrusted: they arrive from publisher sites and
//! archives, and nothing guarantees their nesting is sane. Both of fte's
//! parsing paths descend recursively, so deep enough nesting exhausts the
//! process stack — which is a `SIGABRT`, not a catchable panic, so no amount
//! of error handling downstream can recover from it.
//!
//! The two paths fail in different places, which is why they get different
//! limits:
//!
//! * **XML** (JATS, Wiley) — `roxmltree::Document::parse` is itself a
//!   recursive-descent parser and overflows before any fte code runs. A depth
//!   check inside fte's walkers cannot help; the input has to be rejected
//!   *before* it reaches the parser.
//! * **HTML** — `html5ever`'s tree builder is iterative and handles very deep
//!   input fine. The overflow is in fte's own `walk_element`, whose frames are
//!   much cheaper than the XML parser's, so it tolerates far more nesting.
//!
//! Measured on this crate's toolchain, worst case (debug build, 2 MB thread):
//! `roxmltree` aborts between depth 110 and 128; `walk_element` aborts between
//! 5,000 and 20,000. Release builds on the 8 MB main thread survive roughly an
//! order of magnitude more. The limits below are set for the *worst* case, so
//! behavior does not change between `cargo test` and a release binary.
//!
//! For scale: JATS section nesting in real papers is typically under 10, and
//! total element nesting under 30.

/// Maximum element nesting accepted from XML input.
///
/// Deliberately well under the measured 110-deep failure point of the XML
/// parser, and still several times deeper than any real paper.
pub const MAX_XML_DEPTH: usize = 64;

/// Maximum element nesting descended when rendering HTML.
///
/// Publisher HTML nests far deeper than XML — wrapper divs, nested tables and
/// inline spans accumulate — so this is generous, while staying two orders of
/// magnitude below the measured failure point.
pub const MAX_HTML_DEPTH: usize = 512;

// `roxmltree` aborts between depth 110 and 128 in the worst measured
// configuration. A limit at or above that makes the guard useless, so this is
// checked at compile time rather than left to a test that could be skipped.
const _: () = assert!(MAX_XML_DEPTH < 110);

/// Byte-substring search. Avoids `str` slicing so no index can land mid-character.
fn find_from(haystack: &[u8], start: usize, needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    (start..=haystack.len() - needle.len()).find(|&i| &haystack[i..i + needle.len()] == needle)
}

/// Whether `xml` nests elements more than `limit` deep.
///
/// This runs *before* the real parser, so it must not itself recurse and must
/// not trust the input. It scans bytes once, tracking the constructs that can
/// legally contain a bare `<` or `>` — comments, CDATA, processing
/// instructions, declarations, and quoted attribute values — so those cannot
/// be miscounted as elements.
///
/// Malformed input (an unterminated tag or comment) returns `false`: this
/// function's only job is bounding depth, and the real parser will reject
/// malformed documents with a useful message.
pub fn xml_depth_exceeds(xml: &str, limit: usize) -> bool {
    let b = xml.as_bytes();
    let mut i = 0usize;
    let mut depth = 0usize;

    while i < b.len() {
        if b[i] != b'<' {
            i += 1;
            continue;
        }

        // Constructs that may contain characters which would otherwise scan as
        // markup. Each skips to its own terminator. Order matters: the CDATA
        // and comment openers both start with `<!`, so they precede the
        // general declaration case.
        let skip = if b[i..].starts_with(b"<!--") {
            Some((4usize, &b"-->"[..]))
        } else if b[i..].starts_with(b"<![CDATA[") {
            Some((9, &b"]]>"[..]))
        } else if b[i..].starts_with(b"<?") {
            Some((2, &b"?>"[..]))
        } else if b[i..].starts_with(b"<!") {
            // `<!DOCTYPE ...>` and friends: not elements.
            Some((2, &b">"[..]))
        } else {
            None
        };
        if let Some((open_len, close)) = skip {
            match find_from(b, i + open_len, close) {
                Some(k) => i = k + close.len(),
                None => return false, // unterminated; parser's problem
            }
            continue;
        }

        let closing = b.get(i + 1) == Some(&b'/');

        // Walk to the tag's '>', ignoring any inside quoted attribute values.
        let mut j = i + 1;
        let mut quote: Option<u8> = None;
        let mut self_closing = false;
        while j < b.len() {
            let c = b[j];
            match quote {
                Some(q) if c == q => quote = None,
                Some(_) => {}
                None if c == b'"' || c == b'\'' => quote = Some(c),
                None if c == b'>' => {
                    self_closing = j > i + 1 && b[j - 1] == b'/';
                    break;
                }
                None => {}
            }
            j += 1;
        }
        if j >= b.len() {
            return false; // unterminated tag; parser's problem
        }

        if closing {
            depth = depth.saturating_sub(1);
        } else if !self_closing {
            depth += 1;
            if depth > limit {
                return true;
            }
        }

        i = j + 1;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nested(depth: usize) -> String {
        let mut s = String::new();
        for _ in 0..depth {
            s.push_str("<a>");
        }
        s.push('x');
        for _ in 0..depth {
            s.push_str("</a>");
        }
        s
    }

    #[test]
    fn counts_plain_nesting() {
        assert!(!xml_depth_exceeds(&nested(10), 10));
        assert!(xml_depth_exceeds(&nested(11), 10));
    }

    #[test]
    fn siblings_do_not_accumulate_depth() {
        // 100 siblings are depth 2, not depth 101.
        let mut s = String::from("<root>");
        for _ in 0..100 {
            s.push_str("<child>x</child>");
        }
        s.push_str("</root>");
        assert!(!xml_depth_exceeds(&s, 4));
    }

    #[test]
    fn self_closing_tags_do_not_increase_depth() {
        let mut s = String::from("<root>");
        for _ in 0..100 {
            s.push_str("<br/>");
        }
        s.push_str("</root>");
        assert!(!xml_depth_exceeds(&s, 2));
    }

    #[test]
    fn angle_brackets_in_attributes_are_not_markup() {
        let s = r#"<root><a title="a > b"><b alt='x < y'>t</b></a></root>"#;
        assert!(!xml_depth_exceeds(s, 3));
        assert!(xml_depth_exceeds(s, 2));
    }

    #[test]
    fn comments_and_cdata_are_skipped() {
        let s = "<root><!-- <a><a><a> --><![CDATA[<b><b><b>]]></root>";
        assert!(!xml_depth_exceeds(s, 1));
    }

    #[test]
    fn declarations_and_processing_instructions_are_skipped() {
        let s = r#"<?xml version="1.0"?><!DOCTYPE article SYSTEM "jats.dtd"><root><a>x</a></root>"#;
        assert!(!xml_depth_exceeds(s, 2));
        assert!(xml_depth_exceeds(s, 1));
    }

    #[test]
    fn unterminated_markup_defers_to_the_real_parser() {
        assert!(!xml_depth_exceeds("<root><!-- never closed", 1));
        assert!(!xml_depth_exceeds("<root><a", 1));
    }
}
