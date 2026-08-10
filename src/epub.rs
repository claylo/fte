use std::io::Read;
use std::path::Path;

use anyhow::{Context, Result};
use scraper::{ElementRef, Html, Selector};

use crate::html;
use crate::markdown::{self, Metadata};

/// Extract markdown from an ePub file.
///
/// Discovers content files via the OPF spine (the EPUB standard way) rather
/// than hardcoding paths. Falls back to scanning for .xhtml files if the
/// OPF is missing or unparseable.
pub fn extract(id: &str, path: &Path) -> Result<String> {
    let file = std::fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut archive =
        zip::ZipArchive::new(file).with_context(|| format!("reading zip: {}", path.display()))?;

    let (content_paths, opf_meta) = discover_content_paths(&mut archive)?;

    let mut combined_xhtml = String::new();
    for entry_path in &content_paths {
        match read_entry(&mut archive, entry_path) {
            Ok(content) => {
                if !combined_xhtml.is_empty() {
                    combined_xhtml.push_str("\n<!-- epub-part: ");
                    combined_xhtml.push_str(entry_path);
                    combined_xhtml.push_str(" -->\n");
                }
                combined_xhtml.push_str(&content);
            }
            Err(e) => {
                eprintln!("  WARN skipping {entry_path}: {e}");
            }
        }
    }

    if combined_xhtml.is_empty() {
        anyhow::bail!("no readable content files in ePub");
    }

    parse_epub_xhtml(id, &combined_xhtml, &opf_meta)
}

/// Discover content XHTML paths from the OPF spine, plus Dublin Core metadata.
fn discover_content_paths(
    archive: &mut zip::ZipArchive<std::fs::File>,
) -> Result<(Vec<String>, Metadata)> {
    let opf_path = find_opf_path(archive)?;
    let opf_dir = opf_path
        .rfind('/')
        .map(|i| &opf_path[..=i])
        .unwrap_or("");

    let opf_xml = read_entry(archive, &opf_path)
        .with_context(|| format!("reading {opf_path}"))?;
    let doc = roxmltree::Document::parse(&opf_xml)
        .with_context(|| format!("parsing {opf_path}"))?;

    let opf_meta = extract_opf_metadata(&doc);

    // Build manifest: id → href (resolved relative to OPF directory)
    let mut manifest = std::collections::HashMap::new();
    let mut nav_ids = std::collections::HashSet::new();

    for node in doc.descendants() {
        if node.tag_name().name() == "item"
            && let (Some(item_id), Some(href)) = (node.attribute("id"), node.attribute("href"))
        {
            let props = node.attribute("properties").unwrap_or("");
            if props.contains("nav") {
                nav_ids.insert(item_id.to_string());
            }
            let full_path = format!("{opf_dir}{href}");
            manifest.insert(item_id.to_string(), full_path);
        }
    }

    // Walk spine in order, collecting content items
    let mut paths = Vec::new();
    for node in doc.descendants() {
        if node.tag_name().name() == "itemref"
            && let Some(idref) = node.attribute("idref")
        {
            if nav_ids.contains(idref) {
                continue;
            }
            let linear = node.attribute("linear").unwrap_or("yes");
            if linear == "no" {
                continue;
            }
            if let Some(href) = manifest.get(idref) {
                let lower = href.to_lowercase();
                if lower.ends_with("cover.xhtml") || lower.ends_with("cover.html") {
                    continue;
                }
                paths.push(href.clone());
            }
        }
    }

    if paths.is_empty() {
        anyhow::bail!("OPF spine contains no content items");
    }

    // Also grab overflow files (tables, figures) not in the spine
    let xhtml_dir = paths
        .first()
        .and_then(|p| p.rfind('/').map(|i| &p[..=i]))
        .unwrap_or("");
    let overflow_names: Vec<String> = archive
        .file_names()
        .filter(|n| {
            n.starts_with(xhtml_dir)
                && n.ends_with(".xhtml")
                && !paths.contains(&n.to_string())
        })
        .filter(|n| {
            let lower = n.to_lowercase();
            let fname = lower.rsplit('/').next().unwrap_or(&lower);
            fname.starts_with("table") || fname.starts_with("fig")
        })
        .map(String::from)
        .collect();
    paths.extend(overflow_names);

    Ok((paths, opf_meta))
}

/// Extract Dublin Core metadata from the OPF document.
fn extract_opf_metadata(doc: &roxmltree::Document) -> Metadata {
    let mut meta = Metadata::default();

    for node in doc.descendants() {
        let name = node.tag_name().name();
        match name {
            "title" if node.tag_name().namespace().is_some() => {
                if let Some(text) = node.text() {
                    let t = text.trim();
                    if !t.is_empty() {
                        meta.title = Some(t.to_string());
                    }
                }
            }
            "creator" if node.tag_name().namespace().is_some() => {
                if let Some(text) = node.text() {
                    let t = text.trim();
                    if !t.is_empty() {
                        meta.authors.push(t.to_string());
                    }
                }
            }
            "identifier" if node.tag_name().namespace().is_some() => {
                if let Some(text) = node.text() {
                    let t = text.trim();
                    if t.starts_with("10.") && t.contains('/') {
                        meta.doi = Some(t.to_string());
                    }
                }
            }
            _ => {}
        }
    }

    meta
}

/// Find the OPF file path from META-INF/container.xml.
fn find_opf_path(archive: &mut zip::ZipArchive<std::fs::File>) -> Result<String> {
    let container = read_entry(archive, "META-INF/container.xml")
        .context("reading META-INF/container.xml")?;
    let doc = roxmltree::Document::parse(&container)
        .context("parsing META-INF/container.xml")?;

    for node in doc.descendants() {
        if node.tag_name().name() == "rootfile"
            && let Some(path) = node.attribute("full-path")
        {
            return Ok(path.to_string());
        }
    }

    // Fallback: scan for .opf file
    let opf = archive
        .file_names()
        .find(|n| n.ends_with(".opf"))
        .map(String::from);

    opf.ok_or_else(|| anyhow::anyhow!("no OPF file found in ePub"))
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

fn parse_epub_xhtml(id: &str, xhtml: &str, opf_meta: &Metadata) -> Result<String> {
    let doc = Html::parse_document(xhtml);

    // Extract metadata from schema.org properties, falling back to OPF
    let mut meta = extract_epub_metadata(&doc);
    if meta.title.is_none() {
        meta.title.clone_from(&opf_meta.title);
    }
    if meta.authors.is_empty() {
        meta.authors.clone_from(&opf_meta.authors);
    }
    if meta.doi.is_none() {
        meta.doi.clone_from(&opf_meta.doi);
    }
    if meta.journal.is_none() {
        meta.journal.clone_from(&opf_meta.journal);
    }

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
    if let Ok(sel) = Selector::parse(r#"h1[property="name"]"#)
        && let Some(el) = doc.select(&sel).next()
    {
        let t = markdown::normalize_text(&html::element_text(&el));
        if !t.is_empty() {
            meta.title = Some(t);
        }
    }

    // Fallback: <title> tag
    if meta.title.is_none()
        && let Ok(sel) = Selector::parse("title")
        && let Some(el) = doc.select(&sel).next()
    {
        let t = markdown::normalize_text(&html::element_text(&el));
        if !t.is_empty() {
            meta.title = Some(t);
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
    if let Ok(sel) = Selector::parse(r#"a[property="sameAs"]"#)
        && let Some(el) = doc.select(&sel).next()
        && let Some(href) = el.value().attr("href")
        && href.contains("doi.org")
    {
        let doi = href
            .strip_prefix("https://doi.org/")
            .or_else(|| href.strip_prefix("http://doi.org/"))
            .unwrap_or(href);
        meta.doi = Some(doi.to_string());
    }

    // Fallback DOI: any <a> linking to doi.org
    if meta.doi.is_none()
        && let Ok(sel) = Selector::parse("a")
    {
        for el in doc.select(&sel) {
            if let Some(href) = el.value().attr("href")
                && let Some(doi) = href
                    .strip_prefix("https://doi.org/")
                    .or_else(|| href.strip_prefix("http://doi.org/"))
                && doi.starts_with("10.")
            {
                meta.doi = Some(doi.to_string());
                break;
            }
        }
    }

    // Journal from span[property="name"] inside span[property="isPartOf"][typeof="Periodical"]
    if let Ok(sel) = Selector::parse(r#"span[typeof="Periodical"] span[property="name"]"#)
        && let Some(el) = doc.select(&sel).next()
    {
        let j = markdown::normalize_text(&html::element_text(&el));
        if !j.is_empty() {
            meta.journal = Some(j);
        }
    }

    meta
}

fn extract_property(el: &ElementRef, prop: &str) -> String {
    let sel_str = format!(r#"span[property="{prop}"]"#);
    if let Ok(sel) = Selector::parse(&sel_str)
        && let Some(child) = el.select(&sel).next()
    {
        return html::element_text(&child);
    }
    String::new()
}

fn find_body_element(doc: &Html) -> Option<ElementRef<'_>> {
    // Try section#bodymatter first
    if let Ok(sel) = Selector::parse("section#bodymatter")
        && let Some(el) = doc.select(&sel).next()
    {
        return Some(el);
    }

    // Fallback: article
    if let Ok(sel) = Selector::parse("article")
        && let Some(el) = doc.select(&sel).next()
    {
        return Some(el);
    }

    // Last resort: body
    if let Ok(sel) = Selector::parse("body")
        && let Some(el) = doc.select(&sel).next()
    {
        return Some(el);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_zip(tag: &str, name: &str, data: &[u8]) -> std::path::PathBuf {
        let path =
            std::env::temp_dir().join(format!("fte-epub-{}-{}.zip", std::process::id(), tag));
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
