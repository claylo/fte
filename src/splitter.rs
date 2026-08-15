//! Writing a chapter-marked document out as per-chapter files.

use anyhow::{Context, Result, bail};
use std::collections::HashSet;
use std::path::PathBuf;

use crate::chapter::Document;
use crate::epub::heading_slug;
use crate::template::{self, Vars};

/// Where and how to write split output.
#[derive(Debug, Clone)]
pub struct Options {
    /// Base output directory.
    pub outdir: PathBuf,
    /// Subdirectory template; empty writes flat into `outdir`.
    pub subdir: String,
    /// Chapter filename template.
    pub file: String,
    /// Frontmatter filename template; `None` suppresses the file.
    pub front: Option<String>,
    /// Zero-padding width for `{n}`.
    pub pad: usize,
    /// Overwrite existing files.
    pub force: bool,
}

/// One file written by [`split`].
#[derive(Debug, Clone)]
pub struct Written {
    /// Chapter index; 0 is the frontmatter file.
    pub index: usize,
    /// Chapter title, or the book title for the frontmatter file.
    pub title: String,
    /// Path written.
    pub path: PathBuf,
    /// Size in bytes.
    pub bytes: usize,
}

/// Write a chapter-marked document out as per-chapter files.
///
/// # Errors
///
/// Fails when the document has no chapter markers, a template is invalid, a
/// target exists without `force`, or a write fails.
pub fn split(book: &str, doc: &Document, opts: &Options) -> Result<Vec<Written>> {
    if doc.chapters.is_empty() {
        bail!("no chapter markers in {book}; only book-length ePub output carries them");
    }

    let dir = if opts.subdir.is_empty() {
        opts.outdir.clone()
    } else {
        let sub = template::render(
            &opts.subdir,
            &Vars {
                book,
                n: 0,
                slug: "",
                title: "",
                src: "",
                pad: opts.pad,
            },
        )?;
        opts.outdir.join(sub)
    };
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;

    let mut written = Vec::new();
    let mut used: HashSet<PathBuf> = HashSet::new();

    if let Some(front_tpl) = &opts.front {
        let name = template::render(
            front_tpl,
            &Vars {
                book,
                n: 0,
                slug: "frontmatter",
                title: "frontmatter",
                src: "",
                pad: opts.pad,
            },
        )?;
        let path = dir.join(name);
        let mut content = String::new();
        if !doc.frontmatter.is_empty() {
            content.push_str("---\n");
            content.push_str(&doc.frontmatter);
            content.push_str("---\n\n");
        }
        content.push_str(&doc.preamble);
        content.push('\n');
        write_file(&path, &content, opts.force, &mut used)?;
        written.push(Written {
            index: 0,
            title: book.to_string(),
            path,
            bytes: content.len(),
        });
    }

    for (i, chunk) in doc.chapters.iter().enumerate() {
        let n = i + 1;
        let slug = heading_slug(&chunk.title);
        let src_stem = chunk
            .src
            .rsplit('/')
            .next()
            .and_then(|f| f.rsplit_once('.').map(|(stem, _)| stem))
            .unwrap_or("");

        let name = template::render(
            &opts.file,
            &Vars {
                book,
                n,
                slug: &slug,
                title: &chunk.title,
                src: src_stem,
                pad: opts.pad,
            },
        )?;
        let path = disambiguate(dir.join(name), n, &used);

        let mut content = String::new();
        content.push_str("---\n");
        content.push_str(&doc.frontmatter);
        content.push_str(&format!("chapter: {n}\n"));
        content.push_str(&format!(
            "chapter_title: \"{}\"\n",
            chunk.title.replace('\\', "\\\\").replace('"', "\\\"")
        ));
        if !chunk.src.is_empty() {
            content.push_str(&format!("source: \"{}\"\n", chunk.src));
        }
        content.push_str("---\n\n");
        content.push_str(chunk.body.trim());
        content.push('\n');

        write_file(&path, &content, opts.force, &mut used)?;
        written.push(Written {
            index: n,
            title: chunk.title.clone(),
            path,
            bytes: content.len(),
        });
    }

    Ok(written)
}

/// Append the chapter index when two chapters share a title.
fn disambiguate(path: PathBuf, n: usize, used: &HashSet<PathBuf>) -> PathBuf {
    if !used.contains(&path) {
        return path;
    }
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("chapter");
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("md");
    path.with_file_name(format!("{stem}-{n}.{ext}"))
}

fn write_file(
    path: &PathBuf,
    content: &str,
    force: bool,
    used: &mut HashSet<PathBuf>,
) -> Result<()> {
    if !force && path.exists() {
        bail!("{} exists; pass --force to overwrite", path.display());
    }
    std::fs::write(path, content).with_context(|| format!("writing {}", path.display()))?;
    used.insert(path.clone());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chapter;

    const SAMPLE: &str = r#"---
id: demo
title: "A Book"
---

# A Book

<!-- fte:chapter-start id="ch01" title="One" src="OEBPS/c1.xhtml" -->

## One

First body.

<!-- fte:chapter-end id="ch01" -->

<!-- fte:chapter-start id="ch02" title="One" src="OEBPS/c2.xhtml" -->

## One

Second body.

<!-- fte:chapter-end id="ch02" -->
"#;

    fn opts(dir: &std::path::Path) -> Options {
        Options {
            outdir: dir.to_path_buf(),
            subdir: "{book}".into(),
            file: "{n}-{slug}.md".into(),
            front: Some("{n}-frontmatter.md".into()),
            pad: 2,
            force: false,
        }
    }

    #[test]
    fn default_convention_writes_index_and_chapters() {
        let tmp = tempfile::tempdir().unwrap();
        let doc = chapter::parse(SAMPLE);
        let written = split("demo", &doc, &opts(tmp.path())).unwrap();

        assert_eq!(written.len(), 3);
        assert!(tmp.path().join("demo/00-frontmatter.md").exists());
        assert!(tmp.path().join("demo/01-one.md").exists());
        assert!(tmp.path().join("demo/02-one.md").exists());
    }

    #[test]
    fn colliding_slugs_are_disambiguated_by_index() {
        let tmp = tempfile::tempdir().unwrap();
        let doc = chapter::parse(SAMPLE);
        let o = Options {
            file: "{slug}.md".into(),
            front: None,
            ..opts(tmp.path())
        };
        split("demo", &doc, &o).unwrap();

        // Both chapters in SAMPLE are titled "One", so the second must not
        // overwrite the first.
        assert!(tmp.path().join("demo/one.md").exists());
        assert!(tmp.path().join("demo/one-2.md").exists());
    }

    // CONTROLLER AMENDMENT (preflight ruling, FINDING 1): the plan as written
    // asserted `02-one-2.md` under the default `{n}-{slug}.md` template. That
    // template embeds the chapter index, so every filename is unique by
    // construction and `disambiguate` can never fire — the assertion could not
    // pass. The default-template test now asserts the real output, and
    // `colliding_slugs_are_disambiguated_by_index` above exercises collision
    // handling with `{slug}.md`, the only shape where two chapters actually
    // collide. Implement `disambiguate` exactly as specified below.

    #[test]
    fn flat_convention_writes_alongside() {
        let tmp = tempfile::tempdir().unwrap();
        let doc = chapter::parse(SAMPLE);
        let o = Options {
            subdir: String::new(),
            file: "{book}-ch{n}.md".into(),
            front: Some("{book}-ch{n}-frontmatter.md".into()),
            ..opts(tmp.path())
        };
        split("demo", &doc, &o).unwrap();

        assert!(tmp.path().join("demo-ch00-frontmatter.md").exists());
        assert!(tmp.path().join("demo-ch01.md").exists());
        assert!(tmp.path().join("demo-ch02.md").exists());
    }

    #[test]
    fn chapter_files_inherit_and_extend_frontmatter() {
        let tmp = tempfile::tempdir().unwrap();
        let doc = chapter::parse(SAMPLE);
        split("demo", &doc, &opts(tmp.path())).unwrap();

        let body = std::fs::read_to_string(tmp.path().join("demo/01-one.md")).unwrap();
        assert!(body.starts_with("---\n"));
        assert!(body.contains("id: demo"));
        assert!(body.contains("chapter: 1"));
        assert!(body.contains(r#"chapter_title: "One""#));
        assert!(body.contains(r#"source: "OEBPS/c1.xhtml""#));
        assert!(body.contains("First body."));
    }

    #[test]
    fn front_none_suppresses_the_index_file() {
        let tmp = tempfile::tempdir().unwrap();
        let doc = chapter::parse(SAMPLE);
        let o = Options {
            front: None,
            ..opts(tmp.path())
        };
        let written = split("demo", &doc, &o).unwrap();

        assert_eq!(written.len(), 2);
        assert!(!tmp.path().join("demo/00-frontmatter.md").exists());
    }

    #[test]
    fn existing_files_are_refused_without_force() {
        let tmp = tempfile::tempdir().unwrap();
        let doc = chapter::parse(SAMPLE);
        split("demo", &doc, &opts(tmp.path())).unwrap();

        let err = split("demo", &doc, &opts(tmp.path())).unwrap_err();
        assert!(err.to_string().contains("exists"));

        let forced = Options {
            force: true,
            ..opts(tmp.path())
        };
        assert!(split("demo", &doc, &forced).is_ok());
    }

    #[test]
    fn a_document_with_no_chapters_is_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let doc = chapter::parse("---\nid: paper\n---\n\n# Paper\n\nBody.\n");
        let err = split("paper", &doc, &opts(tmp.path())).unwrap_err();
        assert!(err.to_string().contains("no chapter markers"));
    }
}
