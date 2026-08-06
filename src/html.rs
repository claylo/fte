use anyhow::Result;
use scraper::{ElementRef, Html, Node, Selector};

use crate::config::PublisherProfile;
use crate::depth::MAX_HTML_DEPTH;
use crate::markdown::{self, Metadata};

/// Core HTML extraction driven by a publisher profile.
pub fn extract_html(
    id: &str,
    content: &str,
    profile: &PublisherProfile,
    format_name: &str,
) -> Result<String> {
    // Pre-flight checks
    if profile.abstract_only {
        anyhow::bail!("abstract-only format");
    }
    if let Some(req) = &profile.fulltext_required
        && !content.contains(req.as_str())
    {
        anyhow::bail!("abstract-only (no fulltext)");
    }
    if let Some(marker) = &profile.no_content_marker
        && content.contains(marker.as_str())
    {
        if let Some(ok) = &profile.content_marker {
            if !content.contains(ok.as_str()) {
                anyhow::bail!("no-content page (abstract only)");
            }
        } else {
            anyhow::bail!("no-content page (abstract only)");
        }
    }

    let doc = Html::parse_document(content);
    let meta = extract_meta_tags(&doc);

    // Find the body content container
    let body_el = profile.body_selectors.iter().find_map(|sel| {
        Selector::parse(sel)
            .ok()
            .and_then(|s| doc.select(&s).next())
    });

    let body_el = match body_el {
        Some(el) => el,
        None => anyhow::bail!("no content container found"),
    };

    let mut body = String::new();

    // Title
    if let Some(t) = &meta.title {
        body.push_str(&format!("# {t}\n\n"));
    }

    // Walk the body DOM and emit markdown
    let cruft_sels: Vec<Selector> = profile
        .cruft_selectors
        .iter()
        .filter_map(|s| Selector::parse(s).ok())
        .collect();

    walk_element(&body_el, &cruft_sels, &mut body, 0);

    let body = markdown::collapse_blanks(&body);
    Ok(markdown::document(id, format_name, &meta, &body))
}

/// Extract metadata from <meta> tags (universal across publishers).
pub fn extract_meta_tags(doc: &Html) -> Metadata {
    let mut meta = Metadata::default();

    let meta_sel = Selector::parse("meta").unwrap();
    for el in doc.select(&meta_sel) {
        let name = el.value().attr("name").unwrap_or("");
        let content = el.value().attr("content").unwrap_or("");
        if content.is_empty() {
            continue;
        }
        match name {
            "citation_title" => {
                if meta.title.is_none() {
                    meta.title = Some(content.to_string());
                }
            }
            "citation_author" => {
                meta.authors.push(content.to_string());
            }
            "citation_doi" | "DOI" => {
                if meta.doi.is_none() {
                    meta.doi = Some(content.to_string());
                }
            }
            "citation_journal_title" if meta.journal.is_none() => {
                meta.journal = Some(content.to_string());
            }
            _ => {}
        }
    }

    // Fallback title from <title> tag
    if meta.title.is_none()
        && let Ok(title_sel) = Selector::parse("title")
        && let Some(title_el) = doc.select(&title_sel).next()
    {
        let t = element_text(&title_el);
        let t = markdown::normalize_text(&t);
        // Strip common suffixes
        let t = t
            .split(" | ")
            .next()
            .unwrap_or(&t)
            .split(" - ")
            .next()
            .unwrap_or(&t)
            .trim()
            .to_string();
        if !t.is_empty() {
            meta.title = Some(t);
        }
    }

    meta
}

/// Recursively walk an element and emit markdown.
pub fn walk_element(el: &ElementRef, cruft: &[Selector], out: &mut String, depth: usize) {
    // Untrusted nesting: stop descending rather than exhausting the stack.
    // `html5ever` parses arbitrarily deep input fine; this recursion is what
    // overflows, so the limit belongs here rather than at the parse boundary.
    if depth >= MAX_HTML_DEPTH {
        return;
    }

    // Skip cruft elements
    for sel in cruft {
        if sel.matches(el) {
            return;
        }
    }

    let tag = el.value().name();

    match tag {
        // Headings
        "h1" => {
            let text = markdown::normalize_text(&element_text(el));
            if !text.is_empty() {
                out.push_str(&format!("# {text}\n\n"));
            }
            return;
        }
        "h2" => {
            let text = markdown::normalize_text(&element_text(el));
            if !text.is_empty() {
                out.push_str(&format!("## {text}\n\n"));
            }
            return;
        }
        "h3" => {
            let text = markdown::normalize_text(&element_text(el));
            if !text.is_empty() {
                out.push_str(&format!("### {text}\n\n"));
            }
            return;
        }
        "h4" => {
            let text = markdown::normalize_text(&element_text(el));
            if !text.is_empty() {
                out.push_str(&format!("#### {text}\n\n"));
            }
            return;
        }
        "h5" | "h6" => {
            let text = markdown::normalize_text(&element_text(el));
            if !text.is_empty() {
                out.push_str(&format!("##### {text}\n\n"));
            }
            return;
        }

        // Paragraphs
        "p" => {
            let text = inline_markdown(el, cruft);
            let text = markdown::normalize_text(&text);
            if !text.is_empty() {
                out.push_str(&text);
                out.push_str("\n\n");
            }
            return;
        }

        // Lists
        "ul" | "ol" => {
            for (i, child) in el.children().enumerate() {
                if let Some(child_el) = ElementRef::wrap(child)
                    && child_el.value().name() == "li"
                {
                    let text = inline_markdown(&child_el, cruft);
                    let text = markdown::normalize_text(&text);
                    if !text.is_empty() {
                        if tag == "ol" {
                            out.push_str(&format!("{}. {text}\n", i + 1));
                        } else {
                            out.push_str(&format!("- {text}\n"));
                        }
                    }
                }
            }
            out.push('\n');
            return;
        }

        // Tables
        "table" => {
            extract_html_table(el, out);
            out.push('\n');
            return;
        }

        // Figures
        "figure" | "figcaption" if tag == "figcaption" => {
            let text = markdown::normalize_text(&element_text(el));
            if !text.is_empty() {
                out.push_str(&format!("*{text}*\n\n"));
            }
            return;
        }

        // Block quotes
        "blockquote" => {
            let text = markdown::normalize_text(&element_text(el));
            if !text.is_empty() {
                for line in text.lines() {
                    out.push_str(&format!("> {line}\n"));
                }
                out.push('\n');
            }
            return;
        }

        // Skip these entirely
        "script" | "style" | "noscript" | "iframe" | "svg" | "img" => return,

        // For everything else, recurse into children
        _ => {}
    }

    // Recurse into child elements
    for child in el.children() {
        match child.value() {
            Node::Element(_) => {
                if let Some(child_el) = ElementRef::wrap(child) {
                    walk_element(&child_el, cruft, out, depth + 1);
                }
            }
            Node::Text(text)
                // Only emit loose text at reasonable depth (avoid nav text etc.)
                if depth < 20 => {
                    let t = text.text.trim();
                    if !t.is_empty() && t.len() > 2 {
                        // Heuristic: skip very short text nodes (likely cruft)
                    }
                }
            _ => {}
        }
    }
}

/// Convert inline content to markdown (handles bold, italic, links, etc.)
pub fn inline_markdown(el: &ElementRef, cruft: &[Selector]) -> String {
    inline_markdown_at(el, cruft, 0)
}

fn inline_markdown_at(el: &ElementRef, cruft: &[Selector], depth: usize) -> String {
    // Untrusted nesting: stop descending rather than exhausting the stack.
    if depth >= MAX_HTML_DEPTH {
        return String::new();
    }
    let mut out = String::new();
    for child in el.children() {
        match child.value() {
            Node::Text(text) => {
                out.push_str(&text.text);
            }
            Node::Element(_) => {
                if let Some(child_el) = ElementRef::wrap(child) {
                    // Skip cruft
                    if cruft.iter().any(|s| s.matches(&child_el)) {
                        continue;
                    }

                    let tag = child_el.value().name();
                    match tag {
                        "b" | "strong" => {
                            out.push_str("**");
                            out.push_str(&inline_markdown_at(&child_el, cruft, depth + 1));
                            out.push_str("**");
                        }
                        "i" | "em" => {
                            out.push('*');
                            out.push_str(&inline_markdown_at(&child_el, cruft, depth + 1));
                            out.push('*');
                        }
                        "sup" => {
                            out.push('^');
                            out.push_str(&inline_markdown_at(&child_el, cruft, depth + 1));
                        }
                        "sub" => {
                            out.push('_');
                            out.push_str(&inline_markdown_at(&child_el, cruft, depth + 1));
                        }
                        "a" => {
                            // Just emit the link text, drop the URL
                            out.push_str(&inline_markdown_at(&child_el, cruft, depth + 1));
                        }
                        "span" | "div" => {
                            out.push_str(&inline_markdown_at(&child_el, cruft, depth + 1));
                        }
                        "br" => {
                            out.push('\n');
                        }
                        "script" | "style" | "svg" | "img" => {}
                        _ => {
                            out.push_str(&inline_markdown_at(&child_el, cruft, depth + 1));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    out
}

fn extract_html_table(el: &ElementRef, out: &mut String) {
    let thead_sel = Selector::parse("thead").unwrap();
    let tbody_sel = Selector::parse("tbody").unwrap();
    let tr_sel = Selector::parse("tr").unwrap();
    let th_sel = Selector::parse("th").unwrap();
    let td_sel = Selector::parse("td").unwrap();

    // Header
    if let Some(thead) = el.select(&thead_sel).next() {
        for tr in thead.select(&tr_sel) {
            let cells: Vec<String> = tr
                .select(&th_sel)
                .chain(tr.select(&td_sel))
                .map(|c| markdown::normalize_text(&element_text(&c)))
                .collect();
            if !cells.is_empty() {
                out.push_str(&format!("| {} |\n", cells.join(" | ")));
                out.push_str(&format!(
                    "| {} |\n",
                    cells.iter().map(|_| "---").collect::<Vec<_>>().join(" | ")
                ));
            }
        }
    }

    // Body
    if let Some(tbody) = el.select(&tbody_sel).next() {
        for tr in tbody.select(&tr_sel) {
            let cells: Vec<String> = tr
                .select(&td_sel)
                .chain(tr.select(&th_sel))
                .map(|c| markdown::normalize_text(&element_text(&c)))
                .collect();
            if !cells.is_empty() {
                out.push_str(&format!("| {} |\n", cells.join(" | ")));
            }
        }
    }
}

/// Get all visible text from an element, recursively.
pub fn element_text(el: &ElementRef) -> String {
    element_text_at(el, 0)
}

fn element_text_at(el: &ElementRef, depth: usize) -> String {
    // Untrusted nesting: stop descending rather than exhausting the stack.
    if depth >= MAX_HTML_DEPTH {
        return String::new();
    }
    let mut out = String::new();
    for child in el.children() {
        match child.value() {
            Node::Text(text) => out.push_str(&text.text),
            Node::Element(_) => {
                if let Some(child_el) = ElementRef::wrap(child) {
                    let tag = child_el.value().name();
                    if !matches!(tag, "script" | "style" | "svg") {
                        out.push_str(&element_text_at(&child_el, depth + 1));
                    }
                }
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nested_html(depth: usize) -> String {
        let mut h = String::from("<html><body>");
        for _ in 0..depth {
            h.push_str("<div>");
        }
        h.push_str("deep");
        for _ in 0..depth {
            h.push_str("</div>");
        }
        h.push_str("</body></html>");
        h
    }

    /// Depth 20,000 aborted the process before `walk_element` was bounded.
    /// A stack overflow is a SIGABRT, not a panic, so this test cannot assert
    /// on an error — reaching the assertion at all is the result.
    #[test]
    fn deeply_nested_html_does_not_exhaust_the_stack() {
        let doc = Html::parse_document(&nested_html(20_000));
        let sel = Selector::parse("body").unwrap();
        let body = doc.select(&sel).next().unwrap();

        let mut out = String::new();
        walk_element(&body, &[], &mut out, 0);
    }

    #[test]
    fn deeply_nested_inline_markup_does_not_exhaust_the_stack() {
        let mut h = String::from("<html><body><p>");
        for _ in 0..20_000 {
            h.push_str("<em>");
        }
        h.push_str("text");
        for _ in 0..20_000 {
            h.push_str("</em>");
        }
        h.push_str("</p></body></html>");

        let doc = Html::parse_document(&h);
        let sel = Selector::parse("p").unwrap();
        let p = doc.select(&sel).next().unwrap();

        let _ = inline_markdown(&p, &[]);
        let _ = element_text(&p);
    }

    #[test]
    fn ordinary_nesting_is_unaffected() {
        let doc = Html::parse_document("<html><body><p>hello <b>world</b></p></body></html>");
        let sel = Selector::parse("p").unwrap();
        let p = doc.select(&sel).next().unwrap();

        assert_eq!(element_text(&p), "hello world");
        assert_eq!(inline_markdown(&p, &[]), "hello **world**");
    }
}
