use anyhow::{Context, Result};
use roxmltree::{Document, Node};

use crate::markdown::{self, Metadata};

pub fn extract(id: &str, content: &str) -> Result<String> {
    let doc = Document::parse(content).context("parsing Wiley XML")?;
    let root = doc.root_element();

    let meta = extract_metadata(&root);
    let mut body = String::new();

    // Title
    if let Some(t) = &meta.title {
        body.push_str(&format!("# {t}\n\n"));
    }

    // Abstract
    if let Some(abs_group) = find_desc(&root, "abstractGroup") {
        if let Some(abs) = find_desc(&abs_group, "abstract") {
            body.push_str("## Abstract\n\n");
            // Wiley abstracts can be sectioned or flat
            let sections: Vec<Node> = abs
                .children()
                .filter(|n| n.has_tag_name("section"))
                .collect();
            if sections.is_empty() {
                body.push_str(&collect_paragraphs(&abs));
            } else {
                for sec in sections {
                    if let Some(title) = find_child(&sec, "title") {
                        let t = text_content(&title);
                        if !t.is_empty() {
                            body.push_str(&format!("**{t}** "));
                        }
                    }
                    body.push_str(&collect_paragraphs(&sec));
                }
            }
            body.push('\n');
        }
    }

    // Body sections
    if let Some(body_el) = find_child(&root, "body") {
        extract_sections(&body_el, 2, &mut body);

        // Wiley puts bibliography inside <body>
        if let Some(bib) = find_desc(&body_el, "bibliography") {
            body.push_str("## References\n\n");
            for bib_entry in bib.children().filter(|n| n.has_tag_name("bib")) {
                let cite_text = bib_entry
                    .children()
                    .find(|n| n.has_tag_name("citation"))
                    .map(|c| format_wiley_citation(&c))
                    .unwrap_or_else(|| markdown::normalize_text(&text_content(&bib_entry)));
                if !cite_text.is_empty() {
                    body.push_str(&format!("- {cite_text}\n"));
                }
            }
            body.push('\n');
        }
    }

    let body = markdown::collapse_blanks(&body);
    Ok(markdown::document(id, "wiley-xml", &meta, &body))
}

fn extract_metadata(root: &Node) -> Metadata {
    let mut meta = Metadata::default();

    // Navigate: component > header > contentMeta
    let header = find_child(root, "header");

    if let Some(h) = &header {
        // Content metadata
        if let Some(cm) = find_child(h, "contentMeta") {
            // Title
            if let Some(tg) = find_child(&cm, "titleGroup") {
                for title in tg.children().filter(|n| n.has_tag_name("title")) {
                    if title.attribute("type") == Some("main") {
                        let t = markdown::normalize_text(&text_content(&title));
                        if !t.is_empty() {
                            meta.title = Some(t);
                        }
                    }
                }
            }

            // Authors
            if let Some(creators) = find_child(&cm, "creators") {
                for creator in creators.children().filter(|n| n.has_tag_name("creator")) {
                    if creator.attribute("creatorRole") != Some("author") {
                        continue;
                    }
                    if let Some(pn) = find_child(&creator, "personName") {
                        let given = find_child(&pn, "givenNames")
                            .map(|n| text_content(&n))
                            .unwrap_or_default();
                        let family = find_child(&pn, "familyName")
                            .map(|n| text_content(&n))
                            .unwrap_or_default();
                        let name = format!("{} {}", given.trim(), family.trim());
                        let name = name.trim().to_string();
                        if !name.is_empty() {
                            meta.authors.push(name);
                        }
                    }
                }
            }
        }

        // DOI from publication meta (unit level)
        for pm in h.children().filter(|n| n.has_tag_name("publicationMeta")) {
            if pm.attribute("level") == Some("unit") {
                if let Some(doi) = find_child(&pm, "doi") {
                    let d = text_content(&doi).trim().to_string();
                    if !d.is_empty() {
                        meta.doi = Some(d);
                    }
                }
            }
            // Journal title from product level
            if pm.attribute("level") == Some("product") {
                if let Some(tg) = find_child(&pm, "titleGroup") {
                    for t in tg.children().filter(|n| n.has_tag_name("title")) {
                        if t.attribute("type") == Some("main") {
                            let jt = text_content(&t).trim().to_string();
                            if !jt.is_empty() {
                                meta.journal = Some(jt);
                            }
                        }
                    }
                }
            }
        }
    }

    meta
}

fn extract_sections(parent: &Node, depth: u8, out: &mut String) {
    for child in parent.children() {
        if child.has_tag_name("section") {
            // Section heading
            if let Some(title) = find_child(&child, "title") {
                let t = markdown::normalize_text(&text_content(&title));
                if !t.is_empty() {
                    let hashes = "#".repeat(depth as usize);
                    out.push_str(&format!("{hashes} {t}\n\n"));
                }
            }

            // Direct paragraphs
            out.push_str(&collect_paragraphs(&child));

            // Tables (Wiley uses <tabular>)
            for tab in child.children().filter(|n| n.has_tag_name("tabular")) {
                extract_tabular(&tab, out);
            }

            // Figures
            for fig in child.children().filter(|n| n.has_tag_name("figure")) {
                extract_figure(&fig, out);
            }

            // Recurse
            let next_depth = (depth + 1).min(6);
            extract_sections(&child, next_depth, out);
        } else if child.has_tag_name("p") {
            let text = markdown::normalize_text(&inline_text(&child));
            if !text.is_empty() {
                out.push_str(&text);
                out.push_str("\n\n");
            }
        } else if child.has_tag_name("bibliography") {
            // Handled separately in the main extract function
        }
    }
}

fn extract_tabular(tab: &Node, out: &mut String) {
    let label = find_child(tab, "label")
        .map(|n| text_content(&n))
        .unwrap_or_default();
    let title = find_child(tab, "title")
        .map(|n| markdown::normalize_text(&text_content(&n)))
        .unwrap_or_default();

    if !label.is_empty() || !title.is_empty() {
        out.push_str(&format!("**{label}** {title}\n\n", label = label.trim()));
    }

    // CALS table model: table > tgroup > thead/tbody
    if let Some(table) = find_desc(tab, "table") {
        if let Some(tgroup) = find_child(&table, "tgroup") {
            if let Some(thead) = find_child(&tgroup, "thead") {
                for row in thead.children().filter(|n| n.has_tag_name("row")) {
                    let cells: Vec<String> = row
                        .children()
                        .filter(|n| n.has_tag_name("entry"))
                        .map(|c| markdown::normalize_text(&text_content(&c)))
                        .collect();
                    out.push_str(&format!("| {} |\n", cells.join(" | ")));
                    out.push_str(&format!(
                        "| {} |\n",
                        cells.iter().map(|_| "---").collect::<Vec<_>>().join(" | ")
                    ));
                }
            }
            if let Some(tbody) = find_child(&tgroup, "tbody") {
                for row in tbody.children().filter(|n| n.has_tag_name("row")) {
                    let cells: Vec<String> = row
                        .children()
                        .filter(|n| n.has_tag_name("entry"))
                        .map(|c| markdown::normalize_text(&text_content(&c)))
                        .collect();
                    out.push_str(&format!("| {} |\n", cells.join(" | ")));
                }
            }
        }
    }
    out.push('\n');
}

fn extract_figure(fig: &Node, out: &mut String) {
    let label = find_child(fig, "label")
        .map(|n| text_content(&n))
        .unwrap_or_default();
    let caption = find_desc(fig, "caption")
        .map(|n| markdown::normalize_text(&text_content(&n)))
        .unwrap_or_default();
    if !label.is_empty() || !caption.is_empty() {
        out.push_str(&format!(
            "**{label}** {caption}\n\n",
            label = label.trim()
        ));
    }
}

fn format_wiley_citation(citation: &Node) -> String {
    // Build a readable citation string from structured elements
    let mut parts = Vec::new();

    // Authors
    let authors: Vec<String> = citation
        .children()
        .filter(|n| n.has_tag_name("author"))
        .map(|a| {
            let family = find_child(&a, "familyName")
                .map(|n| text_content(&n))
                .unwrap_or_default();
            let given = find_child(&a, "givenNames")
                .map(|n| text_content(&n))
                .unwrap_or_default();
            format!("{}, {}", family.trim(), given.trim())
        })
        .collect();
    if !authors.is_empty() {
        parts.push(authors.join(", "));
    }

    // Year
    if let Some(year) = find_child(citation, "pubYear") {
        parts.push(format!(
            "({})",
            year.attribute("year")
                .unwrap_or(&text_content(&year))
        ));
    }

    // Article/chapter/book title
    let title = find_child(citation, "articleTitle")
        .or_else(|| find_child(citation, "chapterTitle"))
        .or_else(|| find_child(citation, "bookTitle"))
        .map(|n| markdown::normalize_text(&text_content(&n)))
        .unwrap_or_default();
    if !title.is_empty() {
        parts.push(title);
    }

    // Journal title
    if let Some(jt) = find_child(citation, "journalTitle") {
        let j = markdown::normalize_text(&text_content(&jt));
        if !j.is_empty() {
            parts.push(format!("*{j}*"));
        }
    }

    // Volume, pages
    let vol = find_child(citation, "vol")
        .map(|n| text_content(&n).trim().to_string())
        .unwrap_or_default();
    let fp = find_child(citation, "pageFirst")
        .map(|n| text_content(&n).trim().to_string())
        .unwrap_or_default();
    let lp = find_child(citation, "pageLast")
        .map(|n| text_content(&n).trim().to_string())
        .unwrap_or_default();
    if !vol.is_empty() {
        if !fp.is_empty() && !lp.is_empty() {
            parts.push(format!("{vol}, {fp}-{lp}"));
        } else {
            parts.push(vol);
        }
    }

    if parts.is_empty() {
        // Fallback: just grab all text
        markdown::normalize_text(&text_content(citation))
    } else {
        parts.join(". ") + "."
    }
}

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

fn inline_text(node: &Node) -> String {
    let mut out = String::new();
    for child in node.children() {
        if child.is_text() {
            out.push_str(child.text().unwrap_or(""));
        } else if child.is_element() {
            if child.has_tag_name("link") {
                // Wiley <link href="#..."/> — citation markers, skip the element
                // (surrounding text has the author name)
            } else if child.has_tag_name("i") || child.has_tag_name("italic") {
                out.push('*');
                out.push_str(&inline_text(&child));
                out.push('*');
            } else if child.has_tag_name("b") || child.has_tag_name("bold") {
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

fn find_child<'a>(node: &'a Node, tag: &str) -> Option<Node<'a, 'a>> {
    node.children().find(|n| n.has_tag_name(tag))
}

fn find_desc<'a>(node: &'a Node, tag: &str) -> Option<Node<'a, 'a>> {
    node.descendants().find(|n| n.has_tag_name(tag))
}
