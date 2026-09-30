use std::io::Read;
use std::path::Path;

use anyhow::{Context, Result};
use scraper::{ElementRef, Html, Selector};

use std::collections::BTreeMap;

use crate::chapter;
use crate::config::EpubConfig;
use crate::depth::{MAX_XML_DEPTH, xml_depth_exceeds};
use crate::html;
use crate::markdown::{self, Metadata};

/// Extract markdown from an ePub file.
///
/// Discovers content files via the OPF spine (the EPUB standard way) rather
/// than hardcoding paths. Falls back to scanning for .xhtml files if the
/// OPF is missing or unparseable.
///
/// For book-length epubs (many spine files, no schema.org metadata), switches
/// to a per-chapter extraction strategy using the nav TOC for structure. An
/// EPUB2 book has no nav document, so its NCX supplies the same structure.
pub fn extract(id: &str, path: &Path, cfg: &EpubConfig) -> Result<String> {
    let file = std::fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut archive =
        zip::ZipArchive::new(file).with_context(|| format!("reading zip: {}", path.display()))?;

    let package = discover_content_paths(&mut archive)?;
    let nav_entries = package
        .toc
        .nav
        .and_then(|p| parse_nav_toc(&mut archive, &p).ok())
        .filter(|entries| !entries.is_empty())
        .or_else(|| {
            package
                .toc
                .ncx
                .and_then(|p| parse_ncx_toc(&mut archive, &p).ok())
        })
        .unwrap_or_default();

    let is_book = package.content_paths.len() > 5 && !nav_entries.is_empty();

    if is_book {
        let subheading_classes = detect_subheading_classes(&mut archive);
        let heading_classes = package
            .publisher
            .as_deref()
            .and_then(|p| cfg.heading_classes.get(p));
        extract_book(
            id,
            &mut archive,
            &package.content_paths,
            &package.meta,
            &nav_entries,
            &subheading_classes,
            heading_classes,
        )
    } else {
        extract_paper(id, &mut archive, &package.content_paths, &package.meta)
    }
}

/// Paper-length extraction: concatenate all spine files, parse as one document.
fn extract_paper(
    id: &str,
    archive: &mut zip::ZipArchive<std::fs::File>,
    content_paths: &[String],
    opf_meta: &Metadata,
) -> Result<String> {
    let mut combined_xhtml = String::new();
    for entry_path in content_paths {
        match read_entry(archive, entry_path) {
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

    parse_epub_xhtml(id, &combined_xhtml, opf_meta)
}

/// Book-length extraction: use nav TOC to classify and structure chapters.
fn extract_book(
    id: &str,
    archive: &mut zip::ZipArchive<std::fs::File>,
    content_paths: &[String],
    opf_meta: &Metadata,
    nav_entries: &[NavEntry],
    subheading_classes: &[String],
    heading_classes: Option<&BTreeMap<String, u8>>,
) -> Result<String> {
    let file_to_nav = build_file_nav_map(nav_entries);
    let toc_levels = toc_heading_levels(nav_entries);

    let mut body = String::new();
    let mut included_any = false;
    let mut chapter_index = 0usize;

    // Title heading from OPF metadata
    if let Some(t) = &opf_meta.title {
        body.push_str(&format!("# {t}\n\n"));
    }

    // Table of contents with anchor links
    body.push_str(&render_toc(nav_entries));
    body.push('\n');

    for entry_path in content_paths {
        let nav_label = file_to_nav.get(entry_path.as_str()).map(|s| s.as_str());
        let role = classify_spine_file(entry_path, nav_label);

        if role == SpineRole::Skip {
            continue;
        }

        let xhtml = match read_entry(archive, entry_path) {
            Ok(content) => content,
            Err(e) => {
                eprintln!("  WARN skipping {entry_path}: {e}");
                continue;
            }
        };

        // A paragraph the TOC links to is a section head, styled or not, and
        // so is one whose class the config maps to a level for this publisher.
        let by_id = toc_levels.get(entry_path.as_str());
        let xhtml = if by_id.is_some() || heading_classes.is_some() {
            promote_paragraphs(&xhtml, |id, class| {
                let from_class = class.zip(heading_classes).and_then(|(class, map)| {
                    class.split_whitespace().find_map(|c| map.get(c).copied())
                });
                from_class.or_else(|| id.zip(by_id).and_then(|(id, m)| m.get(id).copied()))
            })
        } else {
            xhtml
        };

        // Promote styled <p> subheadings to <h3> before parsing
        let xhtml = promote_subheadings(&xhtml, subheading_classes);

        let doc = Html::parse_document(&xhtml);
        let body_el = match find_body_element(&doc) {
            Some(el) => el,
            None => continue,
        };

        let strip_sels = book_strip_selectors();

        let has_real_heading = has_heading_in_body(&doc);

        let mut chapter_md = String::new();
        if !has_real_heading && let Some(label) = nav_label {
            let heading = nav_label_to_heading(label);
            if !heading.is_empty() {
                chapter_md.push_str(&heading);
                chapter_md.push_str("\n\n");
            }
        }

        let mut walked = String::new();
        html::walk_element(&body_el, &strip_sels, &mut walked, 0);
        let walked = markdown::collapse_blanks(&walked);
        let walked = walked.trim();

        if walked.is_empty() {
            continue;
        }
        chapter_md.push_str(walked);

        chapter_index += 1;
        let id = chapter::chapter_id(chapter_index);
        let title = nav_label
            .map(str::to_owned)
            .or_else(|| chapter::first_heading_text(&chapter_md))
            .unwrap_or_default();

        body.push_str(&chapter::start_marker(&id, &title, entry_path));
        body.push_str("\n\n");
        body.push_str(&chapter_md);
        body.push_str("\n\n");
        body.push_str(&chapter::end_marker(&id));
        body.push_str("\n\n");
        included_any = true;
    }

    if !included_any {
        anyhow::bail!("no body content found in ePub");
    }

    let body = markdown::collapse_blanks(&body);
    Ok(markdown::document(id, "epub", opf_meta, &body))
}

// ---------------------------------------------------------------------------
// Nav TOC parsing
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct NavEntry {
    label: String,
    /// Spine file the entry points at, without its fragment.
    href: String,
    /// The `#id` part of the entry's link, when it points inside a file.
    fragment: Option<String>,
    children: Vec<NavEntry>,
}

/// Split a TOC link into the file it points at and its fragment, if any.
fn split_href(href: &str) -> (&str, Option<&str>) {
    match href.split_once('#') {
        Some((file, frag)) if !frag.is_empty() => (file, Some(frag)),
        Some((file, _)) => (file, None),
        None => (href, None),
    }
}

/// Parse the EPUB3 navigation document's TOC.
fn parse_nav_toc(
    archive: &mut zip::ZipArchive<std::fs::File>,
    nav_path: &str,
) -> Result<Vec<NavEntry>> {
    let nav_dir = nav_path.rfind('/').map(|i| &nav_path[..=i]).unwrap_or("");
    let content = read_entry(archive, nav_path)?;
    let doc = Html::parse_document(&content);

    let nav_sel = Selector::parse(r#"nav[epub\:type="toc"], nav[epub:type="toc"]"#)
        .unwrap_or_else(|_| Selector::parse("nav").unwrap());
    let nav_el = doc.select(&nav_sel).next();

    // Fallback: try any nav element
    let nav_el = nav_el.or_else(|| {
        let any_nav = Selector::parse("nav").ok()?;
        doc.select(&any_nav).next()
    });

    let nav_el = match nav_el {
        Some(el) => el,
        None => return Ok(Vec::new()),
    };

    let ol_sel = Selector::parse("ol").unwrap();
    let top_ol = match nav_el.select(&ol_sel).next() {
        Some(el) => el,
        None => return Ok(Vec::new()),
    };

    Ok(parse_nav_ol(&top_ol, nav_dir))
}

fn parse_nav_ol(ol: &ElementRef, nav_dir: &str) -> Vec<NavEntry> {
    let li_sel = Selector::parse("li").unwrap();
    let a_sel = Selector::parse("a").unwrap();
    let ol_sel = Selector::parse("ol").unwrap();

    let mut entries = Vec::new();

    for li in ol.select(&li_sel) {
        // Only direct-child <li>s (skip nested ones from sub-<ol>)
        if !is_direct_child_of(li, ol) {
            continue;
        }

        if let Some(a) = li.select(&a_sel).next() {
            let label = markdown::normalize_text(&html::element_text(&a));
            let href = a.value().attr("href").unwrap_or("");
            let (file_href, fragment) = split_href(href);
            let full_href = format!("{nav_dir}{file_href}");

            let children = li
                .select(&ol_sel)
                .next()
                .map(|child_ol| parse_nav_ol(&child_ol, nav_dir))
                .unwrap_or_default();

            entries.push(NavEntry {
                label,
                href: full_href,
                fragment: fragment.map(str::to_owned),
                children,
            });
        }
    }

    entries
}

/// Parse an EPUB2 NCX `navMap` into the entries the EPUB3 nav TOC yields.
fn parse_ncx_toc(
    archive: &mut zip::ZipArchive<std::fs::File>,
    ncx_path: &str,
) -> Result<Vec<NavEntry>> {
    let ncx_dir = ncx_path.rfind('/').map(|i| &ncx_path[..=i]).unwrap_or("");
    let content = read_entry(archive, ncx_path)?;

    // roxmltree recurses while parsing, so over-deep input aborts inside the
    // parser where no error handling can reach it (see crate::depth).
    if xml_depth_exceeds(&content, MAX_XML_DEPTH) {
        anyhow::bail!("{ncx_path} nests deeper than {MAX_XML_DEPTH} elements; refusing to parse");
    }

    // Publisher NCX files routinely declare the NISO ncx-2005-1 DTD, and
    // roxmltree refuses any DTD unless told otherwise. It never fetches the
    // external subset, so allowing the declaration only means parsing it.
    let opts = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..Default::default()
    };
    let doc = roxmltree::Document::parse_with_options(&content, opts)
        .with_context(|| format!("parsing {ncx_path}"))?;

    let Some(nav_map) = doc.descendants().find(|n| n.tag_name().name() == "navMap") else {
        return Ok(Vec::new());
    };

    Ok(parse_nav_points(nav_map, ncx_dir))
}

fn parse_nav_points(parent: roxmltree::Node, ncx_dir: &str) -> Vec<NavEntry> {
    parent
        .children()
        .filter(|n| n.tag_name().name() == "navPoint")
        .map(|point| {
            let label = point
                .children()
                .find(|n| n.tag_name().name() == "navLabel")
                .map(|l| {
                    let text: String = l
                        .descendants()
                        .filter(|n| n.is_text())
                        .filter_map(|n| n.text())
                        .collect();
                    markdown::normalize_text(&text)
                })
                .unwrap_or_default();

            let src = point
                .children()
                .find(|n| n.tag_name().name() == "content")
                .and_then(|c| c.attribute("src"));
            let (href, fragment) = match src.map(split_href) {
                Some((file_href, fragment)) => {
                    (format!("{ncx_dir}{file_href}"), fragment.map(str::to_owned))
                }
                None => (String::new(), None),
            };

            NavEntry {
                label,
                href,
                fragment,
                children: parse_nav_points(point, ncx_dir),
            }
        })
        .collect()
}

fn is_direct_child_of(child: ElementRef, parent: &ElementRef) -> bool {
    child
        .parent()
        .map(|p| p.id() == parent.id())
        .unwrap_or(false)
}

/// Heading levels for the elements a TOC links to by fragment, keyed by
/// spine file and then element id.
///
/// A level is relative to the file's shallowest TOC entry: a section listed
/// one level under its chapter is `2` (`##`), below the chapter title's `#`.
fn toc_heading_levels(
    entries: &[NavEntry],
) -> std::collections::HashMap<String, std::collections::HashMap<String, u8>> {
    use std::collections::HashMap;

    fn walk<'a>(entries: &'a [NavEntry], depth: usize, out: &mut Vec<(&'a NavEntry, usize)>) {
        for entry in entries {
            out.push((entry, depth));
            walk(&entry.children, depth + 1, out);
        }
    }
    let mut flat = Vec::new();
    walk(entries, 0, &mut flat);

    let mut shallowest: HashMap<&str, usize> = HashMap::new();
    for (entry, depth) in &flat {
        if !entry.href.is_empty() {
            let d = shallowest.entry(entry.href.as_str()).or_insert(*depth);
            *d = (*d).min(*depth);
        }
    }

    let mut levels: HashMap<String, HashMap<String, u8>> = HashMap::new();
    for (entry, depth) in &flat {
        let (Some(fragment), Some(&top)) = (&entry.fragment, shallowest.get(entry.href.as_str()))
        else {
            continue;
        };
        let level = u8::try_from(depth - top + 1).unwrap_or(6).min(6);
        levels
            .entry(entry.href.clone())
            .or_default()
            .entry(fragment.clone())
            .or_insert(level);
    }
    levels
}

/// Build a map from file path → nav label (flattening the TOC tree).
fn build_file_nav_map(entries: &[NavEntry]) -> std::collections::HashMap<&str, String> {
    let mut map = std::collections::HashMap::new();
    collect_nav_labels(entries, &mut map);
    map
}

fn collect_nav_labels<'a>(
    entries: &'a [NavEntry],
    map: &mut std::collections::HashMap<&'a str, String>,
) {
    for entry in entries {
        if !entry.href.is_empty() && !map.contains_key(entry.href.as_str()) {
            map.insert(&entry.href, entry.label.clone());
        }
        collect_nav_labels(&entry.children, map);
    }
}

// ---------------------------------------------------------------------------
// Spine file classification
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq)]
enum SpineRole {
    Body,
    Skip,
}

fn classify_spine_file(path: &str, nav_label: Option<&str>) -> SpineRole {
    let lower_path = path.to_lowercase();
    let fname = lower_path.rsplit('/').next().unwrap_or(&lower_path);

    // Cover pages
    if fname.starts_with("cover") || fname.starts_with("wrap") {
        return SpineRole::Skip;
    }

    if let Some(label) = nav_label {
        let lower = label.to_lowercase();

        // Skip only the barest structural cruft — cover, title page, TOC
        if is_structural_cruft(&lower) {
            return SpineRole::Skip;
        }
    }

    // Gutenberg license is not book content
    if let Some(label) = nav_label
        && label.to_lowercase().contains("project gutenberg")
    {
        return SpineRole::Skip;
    }

    SpineRole::Body
}

/// Structural cruft that adds no readable content — cover images, title pages,
/// and the table of contents (which is just a list of links in an e-reader).
fn is_structural_cruft(lower: &str) -> bool {
    matches!(
        lower,
        "cover" | "cover page" | "title page" | "contents" | "table of contents"
    )
}

/// Convert a nav label into a markdown heading.
fn nav_label_to_heading(label: &str) -> String {
    let trimmed = label.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // "Part One", "Part I", etc. → #
    let lower = trimmed.to_lowercase();
    if lower.starts_with("part ") {
        return format!("# {trimmed}");
    }

    // Default: ## for chapter-level content
    format!("## {trimmed}")
}

// ---------------------------------------------------------------------------
// Table of contents
// ---------------------------------------------------------------------------

/// Render a markdown table of contents with anchor links from the nav TOC.
fn render_toc(entries: &[NavEntry]) -> String {
    let mut toc = String::from("## Contents\n\n");
    render_toc_entries(entries, &mut toc, 0);
    toc
}

fn render_toc_entries(entries: &[NavEntry], toc: &mut String, depth: usize) {
    let indent = "  ".repeat(depth);
    for entry in entries {
        let lower = entry.label.to_lowercase();

        // Skip structural cruft and Gutenberg license
        if is_structural_cruft(&lower) || lower.contains("project gutenberg") {
            continue;
        }

        let slug = heading_slug(&entry.label);
        toc.push_str(&format!(
            "{indent}- [{label}](#{slug})\n",
            label = entry.label
        ));

        if !entry.children.is_empty() {
            render_toc_entries(&entry.children, toc, depth + 1);
        }
    }
}

/// Generate a GFM-compatible anchor slug from a heading string.
#[must_use]
pub fn heading_slug(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c
            } else if c == ' ' || c == '-' {
                '-'
            } else {
                // Drop punctuation
                '\0'
            }
        })
        .filter(|&c| c != '\0')
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

// ---------------------------------------------------------------------------
// CSS-based subheading detection
// ---------------------------------------------------------------------------

/// Scan epub stylesheets for CSS classes that look like subheadings:
/// centered text with a font-size bump (>= 1.1em).
fn detect_subheading_classes(archive: &mut zip::ZipArchive<std::fs::File>) -> Vec<String> {
    let css_names: Vec<String> = archive
        .file_names()
        .filter(|n| n.ends_with(".css"))
        .map(String::from)
        .collect();

    let mut classes = Vec::new();
    for name in css_names {
        let css = match read_entry(archive, &name) {
            Ok(c) => c,
            Err(_) => continue,
        };
        classes.extend(find_subheading_classes_in_css(&css));
    }
    classes
}

fn find_subheading_classes_in_css(css: &str) -> Vec<String> {
    // First pass: find the largest centered font-size (chapter/part heading level)
    let mut max_size: f32 = 0.0;
    for block in css.split('}') {
        if block.contains("text-align")
            && block.contains("center")
            && let Some(size) = extract_font_size_em(block)
            && size > max_size
        {
            max_size = size;
        }
    }

    // Second pass: collect classes that are centered with a font-size bump
    // but smaller than the chapter/part heading level. This distinguishes
    // within-chapter subheadings (e.g. 1.1em) from chapter titles (1.5em).
    let mut result = Vec::new();
    for block in css.split('}') {
        let has_center = block.contains("text-align") && block.contains("center");
        if !has_center {
            continue;
        }
        let size = match extract_font_size_em(block) {
            Some(s) if s >= 1.1 && s < max_size => s,
            _ => continue,
        };
        // Must also not be bold/heavy (those are chapter-level)
        if block.contains("font-weight") && (block.contains("bold") || block.contains("800")) {
            continue;
        }
        let _ = size;
        if let Some(class_name) = extract_css_class_name(block)
            && class_name.starts_with("class")
        {
            result.push(class_name);
        }
    }
    result
}

fn extract_font_size_em(css_block: &str) -> Option<f32> {
    let idx = css_block.find("font-size")?;
    let rest = &css_block[idx..];
    let colon = rest.find(':')?;
    let after_colon = rest[colon + 1..].trim_start();
    let em_idx = after_colon.find("em")?;
    after_colon[..em_idx].trim().parse().ok()
}

fn extract_css_class_name(css_block: &str) -> Option<String> {
    let dot = css_block.find('.')?;
    let rest = &css_block[dot + 1..];
    let end = rest
        .find(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
        .unwrap_or(rest.len());
    let name = &rest[..end];
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

/// Rewrite `<p …>…</p>` as `<hN …>…</hN>` wherever `level_for` names a level
/// for the paragraph's `id` and `class` attributes, keeping the attributes.
///
/// Some publishers style section heads as paragraphs. Like
/// `promote_subheadings`, this works on the raw XHTML before parsing; a `<p>`
/// cannot contain another `<p>`, so the next `</p>` closes it.
fn promote_paragraphs(
    xhtml: &str,
    level_for: impl Fn(Option<&str>, Option<&str>) -> Option<u8>,
) -> String {
    const CLOSE: &str = "</p>";
    let mut out = String::with_capacity(xhtml.len());
    let mut rest = xhtml;

    while let Some(start) = find_p_start(rest) {
        let Some(tag_len) = start_tag_len(&rest[start..]) else {
            break;
        };
        let after = start + tag_len;
        let tag = &rest[start..after];
        let self_closing = tag.ends_with("/>");

        let level = if self_closing {
            None
        } else {
            level_for(tag_attr(tag, "id"), tag_attr(tag, "class"))
        };
        match level.zip(rest[after..].find(CLOSE)) {
            Some((level, close)) => {
                // HTML has h1-h6; anything else would parse as an unknown
                // element and the walker would drop its text.
                let level = level.clamp(1, 6);
                out.push_str(&rest[..start]);
                out.push_str(&format!("<h{level}"));
                out.push_str(&tag[2..]);
                out.push_str(&rest[after..after + close]);
                out.push_str(&format!("</h{level}>"));
                rest = &rest[after + close + CLOSE.len()..];
            }
            None => {
                out.push_str(&rest[..after]);
                rest = &rest[after..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Offset of the next `<p` start tag (not `<pre`, `<param`, …).
fn find_p_start(s: &str) -> Option<usize> {
    s.match_indices("<p").map(|(i, _)| i).find(|&i| {
        matches!(
            s.as_bytes().get(i + 2),
            Some(b' ' | b'\t' | b'\n' | b'\r' | b'>' | b'/')
        )
    })
}

/// Length of the start tag at the beginning of `s`, through its `>`, skipping
/// any `>` inside a quoted attribute value.
fn start_tag_len(s: &str) -> Option<usize> {
    let mut quote = None;
    for (i, b) in s.bytes().enumerate() {
        match (quote, b) {
            (None, b'"' | b'\'') => quote = Some(b),
            (Some(q), _) if b == q => quote = None,
            (None, b'>') => return Some(i + 1),
            _ => {}
        }
    }
    None
}

/// Value of attribute `name` in a start tag such as `<p class="x" id='y'>`.
fn tag_attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let mut rest = tag.trim_start_matches('<');
    // Skip the element name.
    rest = rest.trim_start_matches(|c: char| !c.is_whitespace() && c != '>' && c != '/');
    loop {
        rest = rest.trim_start();
        let name_end =
            rest.find(|c: char| c == '=' || c.is_whitespace() || c == '>' || c == '/')?;
        if name_end == 0 {
            return None;
        }
        let attr = &rest[..name_end];
        rest = rest[name_end..].trim_start();
        let value = if let Some(after_eq) = rest.strip_prefix('=') {
            let after_eq = after_eq.trim_start();
            let (value, tail) = match after_eq.chars().next()? {
                q @ ('"' | '\'') => {
                    let body = &after_eq[1..];
                    let end = body.find(q)?;
                    (&body[..end], &body[end + 1..])
                }
                _ => {
                    let end = after_eq
                        .find(|c: char| c.is_whitespace() || c == '>')
                        .unwrap_or(after_eq.len());
                    (&after_eq[..end], &after_eq[end..])
                }
            };
            rest = tail;
            Some(value)
        } else {
            None
        };
        if attr.eq_ignore_ascii_case(name) {
            return value;
        }
    }
}

/// Rewrite `<p class="SUBHEADING_CLASS">...</p>` to `<h3>...</h3>` so that
/// `walk_element` emits them as markdown headings.
fn promote_subheadings(xhtml: &str, classes: &[String]) -> String {
    if classes.is_empty() {
        return xhtml.to_string();
    }
    let mut result = xhtml.to_string();
    for class in classes {
        let open_pattern = format!(r#"<p class="{class}">"#);
        let close = "</p>";
        // Replace each <p class="X">...</p> with <h3>...</h3>
        while let Some(start) = result.find(&open_pattern) {
            let after_open = start + open_pattern.len();
            if let Some(close_offset) = result[after_open..].find(close) {
                let close_start = after_open + close_offset;
                let close_end = close_start + close.len();
                let inner = result[after_open..close_start].to_string();
                result = format!(
                    "{}<h3>{}</h3>{}",
                    &result[..start],
                    inner,
                    &result[close_end..]
                );
            } else {
                break;
            }
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Body element detection
// ---------------------------------------------------------------------------

fn has_heading_in_body(doc: &Html) -> bool {
    for tag in ["h1", "h2", "h3"] {
        if let Ok(sel) = Selector::parse(tag) {
            for el in doc.select(&sel) {
                let text = markdown::normalize_text(&html::element_text(&el));
                if !text.is_empty() && !looks_like_junk_title(&text) {
                    return true;
                }
            }
        }
    }
    false
}

fn book_strip_selectors() -> Vec<Selector> {
    [
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
    .collect()
}

// ---------------------------------------------------------------------------
// Shared helpers (paper path + utilities)
// ---------------------------------------------------------------------------

/// Returns true if a title string looks like an obfuscated ID rather than a
/// real title. Some epub converters produce filenames like "sgPhzGILRlKrLKg2DMvpew1"
/// or "c0" as <title> content.
fn looks_like_junk_title(t: &str) -> bool {
    let t = t.trim();
    if t.is_empty() {
        return true;
    }
    let lower = t.to_lowercase();
    // Generic non-titles
    if lower == "cover" || lower == "nav" || lower == "title page" {
        return true;
    }
    // No spaces: distinguish real single-word headings ("Dedication") from
    // obfuscated file stems ("sgPhzGILRlKrLKg2DMvpew1", "c0", "cP").
    // Real words are all-alpha; hashes/IDs contain digits or mixed case runs.
    if !t.contains(' ') {
        let has_digit = t.chars().any(|c| c.is_ascii_digit());
        let all_alpha = t.chars().all(|c| c.is_alphabetic());
        if has_digit || !all_alpha {
            return true;
        }
        // Very short all-alpha without spaces still looks like an ID (e.g. "cP")
        if t.len() <= 3 {
            return true;
        }
    }
    false
}

/// Where an ePub keeps its table of contents: an EPUB3 nav document, an
/// EPUB2 NCX, or both (EPUB3 files often ship an NCX for older readers).
#[derive(Debug, Default)]
struct TocPaths {
    nav: Option<String>,
    ncx: Option<String>,
}

const NCX_MEDIA_TYPE: &str = "application/x-dtbncx+xml";

/// What the OPF package document says about an ePub.
struct Package {
    /// Spine content files in reading order, plus overflow table/figure files.
    content_paths: Vec<String>,
    meta: Metadata,
    toc: TocPaths,
    /// `dc:publisher`, trimmed.
    publisher: Option<String>,
}

/// Discover content XHTML paths from the OPF spine, plus Dublin Core metadata,
/// plus the table-of-contents document paths (if any).
fn discover_content_paths(archive: &mut zip::ZipArchive<std::fs::File>) -> Result<Package> {
    let opf_path = find_opf_path(archive)?;
    let opf_dir = opf_path.rfind('/').map(|i| &opf_path[..=i]).unwrap_or("");

    let opf_xml = read_entry(archive, &opf_path).with_context(|| format!("reading {opf_path}"))?;
    let doc =
        roxmltree::Document::parse(&opf_xml).with_context(|| format!("parsing {opf_path}"))?;

    let opf_meta = extract_opf_metadata(&doc);

    // Build manifest: id → href (resolved relative to OPF directory)
    let mut manifest = std::collections::HashMap::new();
    let mut nav_ids = std::collections::HashSet::new();
    let mut toc = TocPaths::default();

    for node in doc.descendants() {
        if node.tag_name().name() == "item"
            && let (Some(item_id), Some(href)) = (node.attribute("id"), node.attribute("href"))
        {
            let props = node.attribute("properties").unwrap_or("");
            let full_path = format!("{opf_dir}{href}");
            if props.contains("nav") {
                nav_ids.insert(item_id.to_string());
                toc.nav = Some(full_path.clone());
            }
            if node.attribute("media-type") == Some(NCX_MEDIA_TYPE) {
                toc.ncx = Some(full_path.clone());
            }
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
            n.starts_with(xhtml_dir) && n.ends_with(".xhtml") && !paths.contains(&n.to_string())
        })
        .filter(|n| {
            let lower = n.to_lowercase();
            let fname = lower.rsplit('/').next().unwrap_or(&lower);
            fname.starts_with("table") || fname.starts_with("fig")
        })
        .map(String::from)
        .collect();
    paths.extend(overflow_names);

    let publisher = doc
        .descendants()
        .find(|n| n.tag_name().name() == "publisher" && n.tag_name().namespace().is_some())
        .and_then(|n| n.text())
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(str::to_owned);

    Ok(Package {
        content_paths: paths,
        meta: opf_meta,
        toc,
        publisher,
    })
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
    let container =
        read_entry(archive, "META-INF/container.xml").context("reading META-INF/container.xml")?;
    let doc = roxmltree::Document::parse(&container).context("parsing META-INF/container.xml")?;

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
    if meta.title.is_none()
        || meta
            .title
            .as_ref()
            .is_some_and(|t| looks_like_junk_title(t))
    {
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

    // Fallback: <title> tag (skip if it looks like a junk ID)
    if meta.title.is_none()
        && let Ok(sel) = Selector::parse("title")
        && let Some(el) = doc.select(&sel).next()
    {
        let t = markdown::normalize_text(&html::element_text(&el));
        if !t.is_empty() && !looks_like_junk_title(&t) {
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

    fn nested_ncx(depth: usize) -> String {
        let mut x = String::from(r#"<ncx xmlns="http://www.daisy.org/z3986/2005/ncx/"><navMap>"#);
        for _ in 0..depth {
            x.push_str(r#"<navPoint><navLabel><text>x</text></navLabel><content src="a.html"/>"#);
        }
        for _ in 0..depth {
            x.push_str("</navPoint>");
        }
        x.push_str("</navMap></ncx>");
        x
    }

    #[test]
    fn a_deeply_nested_ncx_is_refused_rather_than_aborting() {
        let p = temp_zip("deep-ncx", "OEBPS/toc.ncx", nested_ncx(5_000).as_bytes());
        let mut a = open(&p);

        let err = parse_ncx_toc(&mut a, "OEBPS/toc.ncx")
            .expect_err("deep input must be refused, not parsed");
        assert!(
            err.to_string().contains("nests deeper"),
            "error should name the depth limit: {err}"
        );

        let _ = std::fs::remove_file(&p);
    }

    fn by_id(id: Option<&str>, _class: Option<&str>) -> Option<u8> {
        (id == Some("s1")).then_some(2)
    }

    #[test]
    fn a_quoted_gt_does_not_end_the_start_tag() {
        let got = promote_paragraphs(r#"<p title="a>b" id="s1"><b>X</b></p>"#, by_id);
        assert_eq!(got, r#"<h2 title="a>b" id="s1"><b>X</b></h2>"#);
    }

    #[test]
    fn only_the_named_attribute_matches() {
        let html = r#"<p data-id="s1">X</p><p id='s1'>Y</p>"#;
        assert_eq!(
            promote_paragraphs(html, by_id),
            r#"<p data-id="s1">X</p><h2 id='s1'>Y</h2>"#
        );
    }

    #[test]
    fn pre_and_self_closing_paragraphs_are_left_alone() {
        let html = r#"<pre id="s1">x</pre><p id="s1"/><p>z</p>"#;
        assert_eq!(promote_paragraphs(html, by_id), html);
    }

    #[test]
    fn junk_titles_are_detected() {
        assert!(looks_like_junk_title("sgPhzGILRlKrLKg2DMvpew1"));
        assert!(looks_like_junk_title("c0"));
        assert!(looks_like_junk_title("cP"));
        assert!(looks_like_junk_title("Cover"));
        assert!(looks_like_junk_title("nav"));
        assert!(looks_like_junk_title(""));
        assert!(looks_like_junk_title("item42"));

        // Real titles (single-word and multi-word)
        assert!(!looks_like_junk_title("Dedication"));
        assert!(!looks_like_junk_title("Introduction"));
        assert!(!looks_like_junk_title("Acknowledgments"));
        assert!(!looks_like_junk_title(
            "Shattered Assumptions: Towards a New Psychology of Trauma"
        ));
        assert!(!looks_like_junk_title(
            "Frankenstein; or, the modern prometheus"
        ));
        assert!(!looks_like_junk_title("The Grieving Brain"));
    }

    #[test]
    fn heading_slugs_are_gfm_compatible() {
        assert_eq!(heading_slug("Dedication"), "dedication");
        assert_eq!(heading_slug("Chapter 1"), "chapter-1");
        assert_eq!(
            heading_slug("1. Walking in the Dark"),
            "1-walking-in-the-dark"
        );
        assert_eq!(
            heading_slug("Part One: The Painful Loss of Here, Now, and Close"),
            "part-one-the-painful-loss-of-here-now-and-close"
        );
        assert_eq!(
            heading_slug("How Does the Brain Understand Loss?"),
            "how-does-the-brain-understand-loss"
        );
        assert_eq!(
            heading_slug("Then Suddenly, Out of Nowhere . . ."),
            "then-suddenly-out-of-nowhere"
        );
    }

    #[test]
    fn structural_cruft_is_classified() {
        assert!(is_structural_cruft("cover page"));
        assert!(is_structural_cruft("title page"));
        assert!(is_structural_cruft("contents"));
        assert!(is_structural_cruft("table of contents"));
        assert!(!is_structural_cruft("chapter 1"));
        assert!(!is_structural_cruft("introduction"));
        assert!(!is_structural_cruft("notes"));
        assert!(!is_structural_cruft("index"));
        assert!(!is_structural_cruft("acknowledgments"));
    }
}
