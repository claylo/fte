# fte — Fulltext Extraction

Extract clean markdown from publisher HTML, XML, and ePub academic papers.

Config-driven: add new publishers with YAML, no recompile needed.

## Install

```bash
cargo install --path .
```

## Usage

```bash
# Extract all files in a directory
fte --indir path/to/html-files --outdir path/to/output

# Extract specific files
fte paper.html chapter.xml article.epub

# Print to stdout
fte --stdout paper.html

# Detect format without extracting
fte --detect-only --indir path/to/html-files

# Overwrite existing output
fte --force --indir path/to/html-files --outdir path/to/output
```

When run without `--indir`, fte looks for a config file (`.fte.yaml`, `.config/fte.yaml`, etc.) walking up from the current directory to find input/output paths.

## Supported Formats

| Format | Detection | Handler |
|--------|-----------|---------|
| ePub (Sage XHTML) | `.epub` extension | Zip unpack + XHTML walk |
| JATS/NLM XML | `<article` + JATS/NLM markers | Structural XML parser |
| Wiley WML3G XML | `<component` + wiley marker | Structural XML parser |
| Springer HTML | `c-article-body` / `c-article-title` | Config-driven CSS |
| Wiley HTML | `article__content` / wiley URL | Config-driven CSS |
| Taylor & Francis | `hlFld-Fulltext` / tandfonline URL | Config-driven CSS |
| OUP Books | `chapter-title` AND oup URL | Config-driven CSS |
| OUP Journals | `widget-ArticleFulltext` / oup URL | Config-driven CSS |
| Cambridge | cambridge URL | Config-driven CSS |
| PLOS | plos URL / `artText` | Config-driven CSS |
| SAGE HTML | sagepub URL | Detected as abstract-only |

## Configuration

fte uses [librebar](https://crates.io/crates/librebar) for config discovery. Config files are found by walking up from the working directory, checking for `.fte.yaml`, `.config/fte.yaml`, or `fte.yaml` (also `.toml` and `.json`). User-level config lives at `~/.config/fte/config.yaml`.

Merge order (lowest to highest precedence):
1. Compiled defaults (all 8 publishers built in)
2. User config (`~/.config/fte/config.yaml`)
3. Project config (`.fte.yaml` in repo root)
4. CLI flags (`--indir`, `--outdir`)

### Minimal project config

```yaml
input_dir: ref/epub
output_dir: ref/epub-md
```

### Adding a publisher

```yaml
publishers:
  arxiv:
    detect_any:
      - "arxiv.org"
      - "ar5iv.labs"
    body_selectors:
      - "article.ltx_document"
      - "div.ltx_page_main"
    cruft_selectors:
      - "nav"
      - "footer"
      - "script"
      - "style"
```

### Publisher profile fields

| Field | Type | Description |
|-------|------|-------------|
| `detect` | `string[]` | AND logic — all must match in first 150KB |
| `detect_any` | `string[]` | OR logic — at least one must match |
| `body_selectors` | `string[]` | CSS selectors tried in order for content container |
| `cruft_selectors` | `string[]` | CSS selectors for elements to strip |
| `ref_selector` | `string?` | CSS selector for references section |
| `fulltext_required` | `string?` | Bail if this string is absent (e.g. T&F) |
| `no_content_marker` | `string?` | Bail if present without `content_marker` |
| `content_marker` | `string?` | Overrides `no_content_marker` bail |
| `abstract_only` | `bool` | Always bail (e.g. SAGE HTML) |

Detection runs against the first 150KB of the HTML. Profiles are tested in alphabetical order; first match wins. Unmatched HTML falls through to the `fallback` profile.

## What about PDFs?

fte handles structured markup (HTML, XML, ePub) — formats where the DOM gives you headings, paragraphs, tables, and metadata for free. PDFs are a fundamentally different problem (layout analysis, OCR, column detection). For PDF-to-markdown conversion, use [Datalab](https://www.datalab.to/) or [Marker](https://github.com/datalab-to/marker).

## Output

Each extracted file is written as markdown with YAML frontmatter:

```yaml
---
id: paper-id
source_format: springer-html
title: "Paper Title"
authors:
  - "First Author"
  - "Second Author"
doi: "10.1234/example"
journal: "Journal Name"
---

# Paper Title

## Abstract

...
```

## License

Apache-2.0 OR MIT
