/// Helpers for building clean markdown output.

/// Wrap extracted content in a markdown document with YAML frontmatter.
pub fn document(id: &str, format: &str, meta: &Metadata, body: &str) -> String {
    let mut out = String::with_capacity(body.len() + 512);

    out.push_str("---\n");
    out.push_str(&format!("id: {id}\n"));
    out.push_str(&format!("source_format: {format}\n"));
    if let Some(t) = &meta.title {
        out.push_str(&format!("title: \"{}\"\n", escape_yaml(t)));
    }
    if !meta.authors.is_empty() {
        out.push_str("authors:\n");
        for a in &meta.authors {
            out.push_str(&format!("  - \"{}\"\n", escape_yaml(a)));
        }
    }
    if let Some(d) = &meta.doi {
        out.push_str(&format!("doi: \"{d}\"\n"));
    }
    if let Some(j) = &meta.journal {
        out.push_str(&format!("journal: \"{}\"\n", escape_yaml(j)));
    }
    out.push_str("---\n\n");

    out.push_str(body);

    // Ensure trailing newline
    if !out.ends_with('\n') {
        out.push('\n');
    }

    out
}

pub struct Metadata {
    pub title: Option<String>,
    pub authors: Vec<String>,
    pub doi: Option<String>,
    pub journal: Option<String>,
}

impl Default for Metadata {
    fn default() -> Self {
        Self {
            title: None,
            authors: Vec::new(),
            doi: None,
            journal: None,
        }
    }
}

fn escape_yaml(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Normalize whitespace in text: collapse runs, trim.
pub fn normalize_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_was_space = true;
    for c in s.chars() {
        if c.is_whitespace() {
            if !last_was_space {
                out.push(' ');
                last_was_space = true;
            }
        } else {
            out.push(c);
            last_was_space = false;
        }
    }
    out.trim().to_string()
}

/// Collapse consecutive blank lines to at most two newlines.
pub fn collapse_blanks(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut blank_count = 0u32;
    for line in s.lines() {
        if line.trim().is_empty() {
            blank_count += 1;
            if blank_count <= 1 {
                out.push('\n');
            }
        } else {
            blank_count = 0;
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}
