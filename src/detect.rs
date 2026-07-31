use std::fmt;
use std::path::Path;

use crate::config::{Config, PublisherProfile};

/// Detected format of a fulltext file.
#[derive(Debug, Clone)]
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
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    match ext {
        "epub" => Format::Epub,
        "xml" => detect_xml(content),
        "html" => detect_html(content, config),
        _ => Format::Unknown,
    }
}

fn detect_xml(content: &str) -> Format {
    let head: &str = if content.len() > 2048 {
        &content[..2048]
    } else {
        content
    };

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
    let sample_len = content.len().min(150_000);
    let head = &content[..sample_len];

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
