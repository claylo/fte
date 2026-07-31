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

fn read_entry(archive: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Result<String> {
    let mut entry = archive.by_name(name)?;
    let mut buf = String::with_capacity(entry.size() as usize);
    entry.read_to_string(&mut buf)?;
    Ok(buf)
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
