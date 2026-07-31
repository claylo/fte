use anyhow::Result;

use crate::detect::Format;
use crate::{html, jats, wiley_xml};

/// Dispatch extraction to the appropriate format handler.
pub fn extract(format: &Format, id: &str, content: &str) -> Result<String> {
    match format {
        Format::Jats => jats::extract(id, content),
        Format::WileyXml => wiley_xml::extract(id, content),
        Format::Html { name, profile } => {
            // Profile keys like "springer" become "springer-html",
            // but keys already ending in "-html" stay as-is.
            let format_name = if name.ends_with("-html") {
                name.clone()
            } else {
                format!("{name}-html")
            };
            html::extract_html(id, content, profile, &format_name)
        }
        Format::Epub => anyhow::bail!("ePub should be handled before extract dispatch"),
        Format::Unknown => anyhow::bail!("unknown format"),
    }
}
