use anyhow::{Context, Result, bail};
use roxmltree::{Document, Node};

use crate::depth::{MAX_XML_DEPTH, xml_depth_exceeds};
use crate::markdown::{self, Metadata};

pub fn extract(id: &str, content: &str) -> Result<String> {
    // Strip DOCTYPE declarations — roxmltree doesn't handle DTDs
    let content = strip_doctype(content);

    // Must precede `Document::parse`: roxmltree recurses per element, so deep
    // input aborts the process inside the parser, where no error handling can
    // reach it.
    if xml_depth_exceeds(&content, MAX_XML_DEPTH) {
        bail!("JATS XML nests deeper than {MAX_XML_DEPTH} elements; refusing to parse");
    }

    let doc = Document::parse(&content).context("parsing JATS XML")?;
    let root = doc.root_element();

    let meta = extract_metadata(&root);
    let mut body = String::new();

    // Title
    if let Some(t) = &meta.title {
        body.push_str(&format!("# {t}\n\n"));
    }

    // Abstract
    if let Some(abs) = find_descendant(&root, "abstract") {
        body.push_str("## Abstract\n\n");
        // Handle sectioned abstracts
        let secs: Vec<Node> = abs.children().filter(|n| n.has_tag_name("sec")).collect();
        if secs.is_empty() {
            body.push_str(&collect_text_blocks(&abs));
        } else {
            for sec in secs {
                if let Some(title) = find_child(&sec, "title") {
                    let t = text_content(&title);
                    if !t.is_empty() {
                        body.push_str(&format!("**{t}** "));
                    }
                }
                body.push_str(&collect_paragraphs(&sec));
                body.push('\n');
            }
        }
        body.push('\n');
    }

    // Body sections
    if let Some(body_el) = find_descendant(&root, "body") {
        extract_sections(&body_el, 2, &mut body);
    }

    // References
    if let Some(ref_list) = find_descendant(&root, "ref-list") {
        body.push_str("## References\n\n");
        for ref_el in ref_list.children().filter(|n| n.has_tag_name("ref")) {
            let label = find_child(&ref_el, "label")
                .map(|n| text_content(&n))
                .unwrap_or_default();
            // Get citation text from mixed-citation or element-citation
            let cite = ref_el
                .children()
                .find(|n| n.has_tag_name("mixed-citation") || n.has_tag_name("element-citation"))
                .map(|n| markdown::normalize_text(&text_content(&n)))
                .unwrap_or_else(|| markdown::normalize_text(&text_content(&ref_el)));

            if !cite.is_empty() {
                if label.is_empty() {
                    body.push_str(&format!("- {cite}\n"));
                } else {
                    body.push_str(&format!("{label}. {cite}\n"));
                }
            }
        }
        body.push('\n');
    }

    let body = markdown::collapse_blanks(&body);
    Ok(markdown::document(id, "jats-xml", &meta, &body))
}

fn extract_metadata(root: &Node) -> Metadata {
    let mut meta = Metadata::default();

    // Navigate: article > front > article-meta
    let article_meta = find_descendant(root, "article-meta");

    if let Some(am) = &article_meta {
        // Title
        if let Some(tg) = find_descendant(am, "article-title") {
            let t = markdown::normalize_text(&text_content(&tg));
            if !t.is_empty() {
                meta.title = Some(t);
            }
        }

        // DOI
        for aid in am.children().filter(|n| n.has_tag_name("article-id")) {
            if aid.attribute("pub-id-type") == Some("doi") {
                let d = text_content(&aid).trim().to_string();
                if !d.is_empty() {
                    meta.doi = Some(d);
                }
            }
        }

        // Authors
        if let Some(cg) = find_descendant(am, "contrib-group") {
            for contrib in cg.children().filter(|n| n.has_tag_name("contrib")) {
                if contrib.attribute("contrib-type") != Some("author") {
                    continue;
                }
                // Try string-name first, then name
                let name_node =
                    find_child(&contrib, "string-name").or_else(|| find_child(&contrib, "name"));
                if let Some(nn) = name_node {
                    let given = find_child(&nn, "given-names")
                        .map(|n| text_content(&n))
                        .unwrap_or_default();
                    let surname = find_child(&nn, "surname")
                        .map(|n| text_content(&n))
                        .unwrap_or_default();
                    let name = format!("{} {}", given.trim(), surname.trim());
                    let name = name.trim().to_string();
                    if !name.is_empty() {
                        meta.authors.push(name);
                    }
                }
            }
        }
    }

    // Journal title
    if let Some(jm) = find_descendant(root, "journal-meta")
        && let Some(jt) = find_descendant(&jm, "journal-title")
    {
        let t = text_content(&jt).trim().to_string();
        if !t.is_empty() {
            meta.journal = Some(t);
        }
    }

    meta
}

fn extract_sections(parent: &Node, depth: u8, out: &mut String) {
    for child in parent.children() {
        if child.has_tag_name("sec") {
            // Section heading
            if let Some(title) = find_child(&child, "title") {
                let t = markdown::normalize_text(&text_content(&title));
                if !t.is_empty() {
                    let hashes = "#".repeat(depth as usize);
                    out.push_str(&format!("{hashes} {t}\n\n"));
                }
            }

            // Direct paragraphs in this section
            out.push_str(&collect_paragraphs(&child));

            // Tables
            for tw in child.children().filter(|n| n.has_tag_name("table-wrap")) {
                extract_table(&tw, out);
            }

            // Figures
            for fig in child.children().filter(|n| n.has_tag_name("fig")) {
                extract_figure(&fig, out);
            }

            // Recurse into subsections
            let next_depth = (depth + 1).min(6);
            extract_sections(&child, next_depth, out);
        } else if child.has_tag_name("p") {
            // Paragraphs directly under body (no section wrapper)
            let text = markdown::normalize_text(&inline_text(&child));
            if !text.is_empty() {
                out.push_str(&text);
                out.push_str("\n\n");
            }
        }
    }
}

fn extract_table(tw: &Node, out: &mut String) {
    let label = find_child(tw, "label")
        .map(|n| text_content(&n))
        .unwrap_or_default();
    let caption = find_descendant(tw, "caption")
        .map(|c| {
            find_child(&c, "p")
                .map(|p| markdown::normalize_text(&text_content(&p)))
                .unwrap_or_else(|| markdown::normalize_text(&text_content(&c)))
        })
        .unwrap_or_default();

    if !label.is_empty() || !caption.is_empty() {
        out.push_str(&format!("**{label}** {caption}\n\n", label = label.trim()));
    }

    // Extract table as simple markdown
    if let Some(table) = find_descendant(tw, "table") {
        extract_html_table(&table, out);
    }
    out.push('\n');
}

fn extract_html_table(table: &Node, out: &mut String) {
    // Find thead rows
    let thead = find_child(table, "thead");
    let tbody = find_child(table, "tbody");

    if let Some(th) = &thead {
        for tr in th.children().filter(|n| n.has_tag_name("tr")) {
            let cells: Vec<String> = tr
                .children()
                .filter(|n| n.has_tag_name("th") || n.has_tag_name("td"))
                .map(|c| markdown::normalize_text(&text_content(&c)))
                .collect();
            out.push_str(&format!("| {} |\n", cells.join(" | ")));
            out.push_str(&format!(
                "| {} |\n",
                cells.iter().map(|_| "---").collect::<Vec<_>>().join(" | ")
            ));
        }
    }

    if let Some(tb) = &tbody {
        for tr in tb.children().filter(|n| n.has_tag_name("tr")) {
            let cells: Vec<String> = tr
                .children()
                .filter(|n| n.has_tag_name("td") || n.has_tag_name("th"))
                .map(|c| markdown::normalize_text(&text_content(&c)))
                .collect();
            out.push_str(&format!("| {} |\n", cells.join(" | ")));
        }
    }
}

fn extract_figure(fig: &Node, out: &mut String) {
    let label = find_child(fig, "label")
        .map(|n| text_content(&n))
        .unwrap_or_default();
    let caption = find_descendant(fig, "caption")
        .map(|c| {
            find_child(&c, "p")
                .map(|p| markdown::normalize_text(&text_content(&p)))
                .unwrap_or_else(|| markdown::normalize_text(&text_content(&c)))
        })
        .unwrap_or_default();
    if !label.is_empty() || !caption.is_empty() {
        out.push_str(&format!("**{label}** {caption}\n\n", label = label.trim()));
    }
}

/// Get all text from a node, recursing into children.
/// Strips xref/link markup but preserves their text content.
fn text_content(node: &Node) -> String {
    let mut out = String::new();
    for child in node.children() {
        if child.is_text() {
            out.push_str(child.text().unwrap_or(""));
        } else if child.is_element() {
            out.push_str(&text_content(&child));
        }
    }
    out
}

/// Get inline text, converting xref[@ref-type="bibr"] to (Author, Year) style.
fn inline_text(node: &Node) -> String {
    let mut out = String::new();
    for child in node.children() {
        if child.is_text() {
            out.push_str(child.text().unwrap_or(""));
        } else if child.is_element() {
            if child.has_tag_name("xref") {
                // Preserve citation text inline
                out.push_str(&text_content(&child));
            } else if child.has_tag_name("italic") || child.has_tag_name("i") {
                out.push('*');
                out.push_str(&inline_text(&child));
                out.push('*');
            } else if child.has_tag_name("bold") || child.has_tag_name("b") {
                out.push_str("**");
                out.push_str(&inline_text(&child));
                out.push_str("**");
            } else if child.has_tag_name("sup") {
                out.push('^');
                out.push_str(&inline_text(&child));
            } else if child.has_tag_name("sub") {
                out.push('_');
                out.push_str(&inline_text(&child));
            } else {
                out.push_str(&inline_text(&child));
            }
        }
    }
    out
}

/// Collect paragraphs (direct <p> children) from a node.
fn collect_paragraphs(node: &Node) -> String {
    let mut out = String::new();
    for child in node.children() {
        if child.has_tag_name("p") {
            let text = markdown::normalize_text(&inline_text(&child));
            if !text.is_empty() {
                out.push_str(&text);
                out.push_str("\n\n");
            }
        }
    }
    out
}

/// Collect all text blocks (paragraphs and raw text) from a node.
fn collect_text_blocks(node: &Node) -> String {
    let mut out = String::new();
    for child in node.children() {
        if child.has_tag_name("p") {
            let text = markdown::normalize_text(&inline_text(&child));
            if !text.is_empty() {
                out.push_str(&text);
                out.push_str("\n\n");
            }
        } else if child.is_text() {
            let text = markdown::normalize_text(child.text().unwrap_or(""));
            if !text.is_empty() {
                out.push_str(&text);
                out.push_str("\n\n");
            }
        }
    }
    out
}

/// Strip DOCTYPE declaration from XML since roxmltree doesn't support DTDs.
fn strip_doctype(content: &str) -> String {
    if let Some(start) = content.find("<!DOCTYPE")
        && let Some(end) = content[start..].find('>')
    {
        let mut out = String::with_capacity(content.len());
        out.push_str(&content[..start]);
        out.push_str(&content[start + end + 1..]);
        return out;
    }
    content.to_string()
}

fn find_child<'a>(node: &'a Node, tag: &str) -> Option<Node<'a, 'a>> {
    node.children().find(|n| n.has_tag_name(tag))
}

fn find_descendant<'a>(node: &'a Node, tag: &str) -> Option<Node<'a, 'a>> {
    node.descendants().find(|n| n.has_tag_name(tag))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nested_jats(depth: usize) -> String {
        let mut s = String::from("<article><body>");
        for _ in 0..depth {
            s.push_str("<sec>");
        }
        s.push_str("<p>deep</p>");
        for _ in 0..depth {
            s.push_str("</sec>");
        }
        s.push_str("</body></article>");
        s
    }

    /// Before the guard, this aborted the process inside `roxmltree::parse`
    /// with a stack overflow — reaching an `Err` at all is the fix.
    #[test]
    fn deeply_nested_jats_is_rejected_rather_than_aborting() {
        let err = extract("test", &nested_jats(5_000))
            .expect_err("deep input must be refused, not parsed");
        assert!(
            err.to_string().contains("nests deeper"),
            "error should name the depth limit: {err}"
        );
    }

    #[test]
    fn the_guard_runs_before_the_parser_sees_the_document() {
        // Depth beyond roxmltree's own failure point but well-formed: if the
        // guard ran after parsing, this test would abort instead of failing.
        assert!(extract("test", &nested_jats(200)).is_err());
    }

    #[test]
    fn ordinary_jats_is_not_rejected_for_depth() {
        let xml = r#"<article><front><article-meta><title-group><article-title>T</article-title></title-group></article-meta></front><body><sec><title>S</title><p>hello</p></sec></body></article>"#;
        match extract("test", xml) {
            Ok(md) => assert!(md.contains("hello"), "body should survive: {md}"),
            Err(e) => assert!(
                !e.to_string().contains("nests deeper"),
                "ordinary depth must not trip the guard: {e}"
            ),
        }
    }
}
