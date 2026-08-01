use std::io::Read;
use std::path::Path;

use anyhow::{Context, Result};
use scraper::{ElementRef, Html, Selector};

use crate::html;
use crate::markdown::{self, Metadata};

/// Extract markdown from a Sage ePub file.
pub fn extract(id: &str, path: &Path) -> Result<String> {
    let file = std::fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut archive =
        zip::ZipArchive::new(file).with_context(|| format!("reading zip: {}", path.display()))?;

    // Read the main content file
    let mut main_xhtml = read_entry(&mut archive, "EPUB/xhtml/index.xhtml")
        .context("reading EPUB/xhtml/index.xhtml")?;

    // Append any table overflow files (table*.xhtml)
    let table_names: Vec<String> = archive
        .file_names()
        .filter(|n| {
            n.starts_with("EPUB/xhtml/table") && n.ends_with(".xhtml")
        })
        .map(String::from)
        .collect();

    for name in &table_names {
        if let Ok(content) = read_entry(&mut archive, name) {
            // Extract just the <body> content from overflow files
            main_xhtml.push_str("\n<!-- overflow: ");
            main_xhtml.push_str(name);
            main_xhtml.push_str(" -->\n");
            main_xhtml.push_str(&content);
        }
    }

    // Also grab figure overflow files
    let fig_names: Vec<String> = archive
        .file_names()
        .filter(|n| {
            n.starts_with("EPUB/xhtml/fig") && n.ends_with(".xhtml")
        })
        .map(String::from)
        .collect();

    for name in &fig_names {
        if let Ok(content) = read_entry(&mut archive, name) {
            main_xhtml.push_str("\n<!-- overflow: ");
            main_xhtml.push_str(name);
            main_xhtml.push_str(" -->\n");
            main_xhtml.push_str(&content);
        }
    }

    parse_epub_xhtml(id, &main_xhtml)
}

/// Largest uncompressed entry fte will read out of an ePub archive.
///
/// Generous for a chapter of XHTML; a single entry past this is a malformed or
/// hostile archive, not a paper.
const MAX_ENTRY_BYTES: u64 = 64 * 1024 * 1024;

fn read_entry(archive: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Result<String> {
    read_entry_limited(archive, name, MAX_ENTRY_BYTES)
}

/// Read one archive entry as UTF-8, refusing anything larger than `max` bytes.
///
/// Both halves of the size are untrusted and are checked separately:
///
/// * The *declared* uncompressed size comes from the archive's central
///   directory. It was previously cast straight to `usize` and handed to
///   `String::with_capacity`, so a crafted declaration could trigger a
///   capacity-overflow panic or an allocation abort before a single byte was
///   read.
/// * The *actual* stream need not match that declaration — a small declared
///   size can expand without bound. So the read is capped as well, and a
///   stream that reaches the cap is rejected rather than silently truncated.
fn read_entry_limited(
    archive: &mut zip::ZipArchive<std::fs::File>,
    name: &str,
    max: u64,
) -> Result<String> {
    let mut entry = archive.by_name(name)?;

    let declared = entry.size();
    if declared > max {
        anyhow::bail!("{name}: declared uncompressed size {declared} exceeds the limit of {max}");
    }

    // Now that `declared` is bounded, it is safe as a capacity hint.
    // `try_reserve` reports allocation failure instead of aborting.
    let hint = usize::try_from(declared).unwrap_or(0);
    let mut buf = Vec::new();
    buf.try_reserve_exact(hint)
        .with_context(|| format!("reserving {hint} bytes for {name}"))?;

    // Read one byte past the cap so "exactly at the limit" stays distinguishable
    // from "the declaration lied and this kept going".
    entry
        .by_ref()
        .take(max + 1)
        .read_to_end(&mut buf)
        .with_context(|| format!("reading {name}"))?;

    if buf.len() as u64 > max {
        anyhow::bail!("{name}: uncompressed data exceeds the limit of {max}");
    }

    String::from_utf8(buf).with_context(|| format!("{name} is not valid UTF-8"))
}

fn parse_epub_xhtml(id: &str, xhtml: &str) -> Result<String> {
    let doc = Html::parse_document(xhtml);

    // Extract metadata from schema.org properties
    let meta = extract_epub_metadata(&doc);

    let mut body = String::new();

    // Title
    if let Some(t) = &meta.title {
        body.push_str(&format!("# {t}\n\n"));
    }

    // Find body content: section#bodymatter > article, or article fallback
    let body_el = find_body_element(&doc);
    let body_el = match body_el {
        Some(el) => el,
        None => anyhow::bail!("no body content found in ePub"),
    };

    // Selectors to strip
    let strip_sels: Vec<Selector> = [
        "section#frontmatter",
        "section#backmatter",
        "nav",
        "div.authors",
        "div.article-notes",
        "div#keywords",
        "script",
        "style",
    ]
    .iter()
    .filter_map(|s| Selector::parse(s).ok())
    .collect();

    html::walk_element(&body_el, &strip_sels, &mut body, 0);

    let body = markdown::collapse_blanks(&body);
    Ok(markdown::document(id, "epub", &meta, &body))
}

fn extract_epub_metadata(doc: &Html) -> Metadata {
    let mut meta = Metadata::default();

    // Title from h1[property="name"] or <title>
    if let Ok(sel) = Selector::parse(r#"h1[property="name"]"#) {
        if let Some(el) = doc.select(&sel).next() {
            let t = markdown::normalize_text(&html::element_text(&el));
            if !t.is_empty() {
                meta.title = Some(t);
            }
        }
    }

    // Fallback: <title> tag
    if meta.title.is_none() {
        if let Ok(sel) = Selector::parse("title") {
            if let Some(el) = doc.select(&sel).next() {
                let t = markdown::normalize_text(&html::element_text(&el));
                if !t.is_empty() {
                    meta.title = Some(t);
                }
            }
        }
    }

    // Authors from span[property="author"]
    if let Ok(sel) = Selector::parse(r#"span[property="author"]"#) {
        for el in doc.select(&sel) {
            let given = extract_property(&el, "givenName");
            let family = extract_property(&el, "familyName");
            let name = format!("{} {}", given.trim(), family.trim());
            let name = name.trim().to_string();
            if !name.is_empty() {
                meta.authors.push(name);
            }
        }
    }

    // DOI from a[property="sameAs"] containing doi.org
    if let Ok(sel) = Selector::parse(r#"a[property="sameAs"]"#) {
        if let Some(el) = doc.select(&sel).next() {
            if let Some(href) = el.value().attr("href") {
                if href.contains("doi.org") {
                    // Extract DOI from URL
                    let doi = href
                        .strip_prefix("https://doi.org/")
                        .or_else(|| href.strip_prefix("http://doi.org/"))
                        .unwrap_or(href);
                    meta.doi = Some(doi.to_string());
                }
            }
        }
    }

    // Journal from span[property="name"] inside span[property="isPartOf"][typeof="Periodical"]
    if let Ok(sel) =
        Selector::parse(r#"span[typeof="Periodical"] span[property="name"]"#)
    {
        if let Some(el) = doc.select(&sel).next() {
            let j = markdown::normalize_text(&html::element_text(&el));
            if !j.is_empty() {
                meta.journal = Some(j);
            }
        }
    }

    meta
}

fn extract_property(el: &ElementRef, prop: &str) -> String {
    let sel_str = format!(r#"span[property="{prop}"]"#);
    if let Ok(sel) = Selector::parse(&sel_str) {
        if let Some(child) = el.select(&sel).next() {
            return html::element_text(&child);
        }
    }
    String::new()
}

fn find_body_element(doc: &Html) -> Option<ElementRef<'_>> {
    // Try section#bodymatter first
    if let Ok(sel) = Selector::parse("section#bodymatter") {
        if let Some(el) = doc.select(&sel).next() {
            return Some(el);
        }
    }

    // Fallback: article
    if let Ok(sel) = Selector::parse("article") {
        if let Some(el) = doc.select(&sel).next() {
            return Some(el);
        }
    }

    // Last resort: body
    if let Ok(sel) = Selector::parse("body") {
        if let Some(el) = doc.select(&sel).next() {
            return Some(el);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_zip(tag: &str, name: &str, data: &[u8]) -> std::path::PathBuf {
        let path = std::env::temp_dir()
            .join(format!("fte-epub-{}-{}.zip", std::process::id(), tag));
        let f = std::fs::File::create(&path).unwrap();
        let mut w = zip::ZipWriter::new(f);
        w.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        w.write_all(data).unwrap();
        w.finish().unwrap();
        path
    }

    fn open(path: &std::path::Path) -> zip::ZipArchive<std::fs::File> {
        zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap()
    }

    #[test]
    fn an_entry_within_the_limit_reads_normally() {
        let p = temp_zip("ok", "a.xhtml", b"<html>hi</html>");
        let mut a = open(&p);

        let got = read_entry_limited(&mut a, "a.xhtml", 1024).expect("should read");
        assert_eq!(got, "<html>hi</html>");

        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn an_oversized_declaration_is_refused_before_allocating() {
        let big = vec![b'x'; 4096];
        let p = temp_zip("big", "a.xhtml", &big);
        let mut a = open(&p);

        let err = read_entry_limited(&mut a, "a.xhtml", 100)
            .expect_err("declared size over the limit must be refused");
        assert!(
            err.to_string().contains("declared uncompressed size"),
            "should name the declared-size check: {err}"
        );

        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn an_entry_exactly_at_the_limit_is_still_accepted() {
        let exact = vec![b'y'; 100];
        let p = temp_zip("exact", "a.xhtml", &exact);
        let mut a = open(&p);

        let got = read_entry_limited(&mut a, "a.xhtml", 100).expect("exactly at the cap is fine");
        assert_eq!(got.len(), 100);

        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn non_utf8_content_is_an_error_not_a_panic() {
        let p = temp_zip("utf8", "a.xhtml", &[0xff, 0xfe, 0x00]);
        let mut a = open(&p);

        let err = read_entry_limited(&mut a, "a.xhtml", 1024).expect_err("invalid UTF-8");
        assert!(
            err.to_string().contains("not valid UTF-8"),
            "should name the encoding problem: {err}"
        );

        let _ = std::fs::remove_file(&p);
    }
}
