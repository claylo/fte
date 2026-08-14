# fte — Fulltext Extraction

Extract clean markdown from publisher HTML, XML, and ePub files — academic papers and book-length epubs.

Config-driven: add new publishers with YAML, no recompile needed.

## Install

```bash
cargo install fte
# or
brew install claylo/tap/fte
# or
npm install -g @claylo/fte
```

Prebuilt binaries for macOS, Linux, and Windows are on the
[releases page](https://github.com/claylo/fte/releases).

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

### Shell completions and machine-readable help

```bash
# Generate completions (bash, zsh, fish, elvish, powershell)
fte completions zsh > ~/.zfunc/_fte

# Print the CLI Spec schema for tooling and agents
fte schema
```

## Supported Formats

| Format | Detection | Handler |
|--------|-----------|---------|
| ePub (academic) | `.epub`, ≤5 spine files | OPF spine + XHTML walk |
| ePub (books) | `.epub`, >5 spine files + nav TOC | Per-chapter extraction with nav structure |
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

### Book-length ePubs

fte handles book-length epubs — full-length books with many chapters, not just single academic articles. When it detects a book (>5 spine files with a navigation document), it switches to a per-chapter extraction strategy:

- **Metadata** from OPF Dublin Core (`dc:title`, `dc:creator`), since per-file XHTML titles are often obfuscated converter artifacts.
- **Chapter structure** from the EPUB3 navigation document's table of contents. When chapters lack heading elements (common in converter output), fte injects headings from the nav labels.
- **Subheading recovery** via CSS heuristics. Some converters flatten subheadings into styled `<p>` tags — fte detects these by font-size and promotes them back to headings.
- **Table of contents** with anchor links, rendered at the top of the output.
- **All content included** — front matter, chapters, notes, references, index. The heading structure is clean enough to split by chapter or strip sections in post-processing.

This is best-effort. ePub books vary wildly in structure, especially after format conversion. fte handles the common patterns well (tested against Gutenberg, HarperCollins, and Simon & Schuster converter output), but edge cases exist. If a book's XHTML uses no heading tags and no identifiable CSS patterns for subheadings, you'll get flat paragraphs with chapter breaks.

## Configuration

Config files are found by walking up from the working directory, checking for `.fte.yaml`, `.config/fte.yaml`, or `fte.yaml` (also `.toml` and `.json`). User-level config lives at `~/.config/fte/config.yaml`.

Pass `-c/--config FILE` to load a specific file instead of relying on discovery. An explicit file is a deliberate selection, so it outranks both discovered files and environment variables.

Merge order (lowest to highest precedence):
1. Compiled defaults (all 8 publishers built in)
2. User config (`~/.config/fte/config.yaml`)
3. Project config (`.fte.yaml` in repo root)
4. Environment variables (`FTE_INPUT_DIR`, `FTE_OUTPUT_DIR`, …)
5. Explicit config file (`-c/--config`)
6. CLI flags (`--indir`, `--outdir`)

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

## AI disclosure

I build this with AI assistance — more of it than most disclosures admit.

- **Tools**: Claude Code (Anthropic), running locally with persistent project memory and a fleet-wide set of workflow rules shared across my repos.
- **Used for**: Design conversations, specs and plans, code, tests, CI workflows, refactors, docs. AI agents wrote substantial portions of this codebase, working from plans I approved.
- **Not used for**: Committing, merging, pushing, tagging, publishing, version numbers. An agent drafts the commit message. I read the diff and run the commit myself. Releases pass a ship gate I run by hand.
- **Verification**: Every change gets two independent AI review passes (spec compliance, then code quality) and the `just check` / `just test` gates. I read every diff before I commit it.
- **Limitations**: AI code can pass tests and still be subtly wrong. The review stack catches most of it. Bug reports are welcome and taken seriously.
- **Last reviewed**: 2026-08-11

## License

Apache-2.0 OR MIT
