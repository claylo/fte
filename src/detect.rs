use std::fmt;
use std::path::Path;

use crate::config::{Config, PublisherProfile};

/// Detected format of a fulltext file.
///
/// `Html` carries an owned `PublisherProfile`, which makes it much larger than
/// the unit variants. Boxing it would trade a one-time 224-byte move for a heap
/// allocation on a value constructed once per input file, alongside parsing
/// hundreds of kilobytes of HTML — not a trade worth making here.
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum Format {
    /// ePub (Sage structured XHTML)
    Epub,
    /// NLM/JATS XML
    Jats,
    /// Wiley WML3G XML
    WileyXml,
    /// HTML with a matched publisher profile
    Html {
        name: String,
        profile: PublisherProfile,
    },
    /// Unknown format
    Unknown,
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Epub => write!(f, "epub"),
            Self::Jats => write!(f, "jats-xml"),
            Self::WileyXml => write!(f, "wiley-xml"),
            Self::Html { name, .. } => {
                if name.ends_with("-html") {
                    write!(f, "{name}")
                } else {
                    write!(f, "{name}-html")
                }
            }
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

pub fn detect_format(path: &Path, content: &str, config: &Config) -> Format {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    match ext {
        "epub" => Format::Epub,
        "xml" => detect_xml(content),
        "html" => detect_html(content, config),
        _ => Format::Unknown,
    }
}

/// Longest prefix of `s` that is at most `max` bytes and ends on a character
/// boundary.
///
/// Detection samples a fixed byte count from the head of a document, but the
/// document is untrusted: a multi-byte character straddling that offset would
/// make `&s[..max]` panic before any extraction error handling runs.
///
/// `str::floor_char_boundary` does this in std, but it stabilized in 1.91 and
/// this crate's `rust-version` floor is 1.89. Swap to it if that floor moves.
fn char_safe_prefix(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    // UTF-8 code points are at most 4 bytes, so this steps back at most 3 times.
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

fn detect_xml(content: &str) -> Format {
    let head = char_safe_prefix(content, 2048);

    if head.contains("<component") && head.contains("wiley") {
        Format::WileyXml
    } else if head.contains("<article") && (head.contains("JATS") || head.contains("NLM")) {
        Format::Jats
    } else if head.contains("<article") {
        // Fallback: assume JATS for any <article> root
        Format::Jats
    } else {
        Format::Unknown
    }
}

fn detect_html(content: &str, config: &Config) -> Format {
    let head = char_safe_prefix(content, 150_000);

    // Iterate publishers in order, first match wins
    for (name, profile) in &config.publishers {
        if matches_profile(head, profile) {
            return Format::Html {
                name: name.clone(),
                profile: profile.clone(),
            };
        }
    }

    // No match — use fallback
    Format::Html {
        name: "other".into(),
        profile: config.fallback.clone(),
    }
}

/// Test whether a profile's detect/detect_any markers match the HTML head.
fn matches_profile(head: &str, profile: &PublisherProfile) -> bool {
    // If detect is non-empty, ALL must match
    if !profile.detect.is_empty() && !profile.detect.iter().all(|d| head.contains(d.as_str())) {
        return false;
    }

    // If detect_any is non-empty, at least one must match
    if !profile.detect_any.is_empty()
        && !profile.detect_any.iter().any(|d| head.contains(d.as_str()))
    {
        return false;
    }

    // At least one detection group must be non-empty
    !profile.detect.is_empty() || !profile.detect_any.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A string whose final character is multi-byte and straddles `boundary`,
    /// so slicing at exactly `boundary` splits a UTF-8 code point.
    fn straddling(boundary: usize) -> String {
        // 'é' is 2 bytes: placing it at boundary-1 puts its second byte at
        // `boundary`, which is therefore not a character boundary.
        let mut s = "a".repeat(boundary - 1);
        s.push('é');
        s
    }

    #[test]
    fn xml_detection_survives_a_character_spanning_the_sample_boundary() {
        let content = straddling(2048);
        assert!(!content.is_char_boundary(2048), "test fixture is wrong");

        // Must not panic. Content is not XML, so Unknown is the right answer.
        assert!(matches!(detect_xml(&content), Format::Unknown));
    }

    #[test]
    fn html_detection_survives_a_character_spanning_the_sample_boundary() {
        let content = straddling(150_000);
        assert!(!content.is_char_boundary(150_000), "test fixture is wrong");

        let config = Config::default();
        // Must not panic. No publisher matches, so the fallback profile is used.
        assert!(matches!(
            detect_html(&content, &config),
            Format::Html { .. }
        ));
    }

    #[test]
    fn xml_detection_still_reads_markers_inside_the_sample() {
        let content = format!(
            r#"<?xml version="1.0"?><article xmlns:jats="JATS">{}"#,
            "x".repeat(4096)
        );
        assert!(matches!(detect_xml(&content), Format::Jats));
    }

    #[test]
    fn char_safe_prefix_never_splits_a_code_point() {
        // 'é' occupies bytes 1..3, so byte 2 is mid-character.
        let s = "aéb";
        assert_eq!(char_safe_prefix(s, 2), "a");
        assert_eq!(char_safe_prefix(s, 3), "aé");
        assert_eq!(char_safe_prefix(s, 0), "");
        // Shorter than the cap returns the whole string.
        assert_eq!(char_safe_prefix(s, 999), s);
    }

    #[test]
    fn char_safe_prefix_handles_a_four_byte_character() {
        // An emoji is 4 bytes; every interior index must floor to 0.
        let s = "😀";
        for i in 0..4 {
            assert_eq!(
                char_safe_prefix(s, i),
                "",
                "index {i} should floor to empty"
            );
        }
        assert_eq!(char_safe_prefix(s, 4), s);
    }
}
