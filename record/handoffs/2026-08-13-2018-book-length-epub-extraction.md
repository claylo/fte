# Handoff: book-length ePub extraction

**Date:** 2026-08-13
**Branch:** main
**State:** Green — 100/100 tests pass (nextest), clippy clean (`-D warnings`, toolchain 1.97.1)

## What was done

### Book-length epub support

fte now handles full-length book epubs (not just academic papers). The detection heuristic: >5 spine files with an EPUB3 navigation document triggers the book path. Academic paper epubs (≤5 spine files) continue through the existing concatenate-and-parse path, unchanged.

### What the book path does

1. **Nav TOC parsing** — reads the EPUB3 navigation document (`nav.xhtml`) to build a hierarchical chapter map. Each `<a>` in the TOC `<ol>` maps a file path to a chapter label.

2. **Per-chapter extraction** — processes each spine file independently rather than concatenating into one blob. Skips structural cruft (cover page, title page, table of contents). Everything else — front matter, chapters, notes, references, index — is included.

3. **Heading injection** — when a chapter's XHTML has no `<h1>`–`<h3>` elements (common in converter output), injects a heading from the nav label. When the XHTML does have headings, skips injection to avoid doubling.

4. **Junk title detection** — recognizes obfuscated file stems used as `<title>` content (e.g. `sgPhzGILRlKrLKg2DMvpew1`, `c0`, `cP`) and falls back to OPF `dc:title`. Uses a heuristic: no spaces + contains digits or ≤3 chars = junk; real single-word titles like "Dedication" pass through.

5. **CSS subheading recovery** — scans the epub's stylesheets for CSS classes that look like subheadings: `text-align: center` with `font-size` between 1.1em and the max heading size, and not bold. Rewrites matching `<p>` tags to `<h3>` before parsing. Correctly distinguishes within-chapter subheadings (1.1em) from chapter titles (1.5em+).

6. **Table of contents** — renders a markdown TOC with GFM anchor links at the top of the output, derived from the nav hierarchy. Skips structural cruft entries. Nested chapters under Part headings are indented.

7. **Title heading** — emits `# Title` from OPF `dc:title` at the top of book output (the paper path already did this via `extract_epub_metadata`).

### Test fixtures

Added two Project Gutenberg epubs as book-length test fixtures:
- `tests/golden/input/books/frankenstein-pg.epub` — Shelley, 31 spine files
- `tests/golden/input/books/tale-two-cities-pg.epub` — Dickens, 47 spine files with illustrations

These are not wired as golden tests (no expected output files) — they're for manual testing and future golden expansion.

### README update

Documented book epub support with a new "Book-length ePubs" subsection. Updated the format table to show both academic and book epub rows. Frames the feature as best-effort with honest caveats about converter variability.

## Changed files

```
M  src/epub.rs                   — book detection, nav parsing, per-chapter extraction,
                                   subheading recovery, TOC rendering, junk title detection
M  README.md                     — document book epub support, update format table
A  tests/golden/input/books/     — Gutenberg epub fixtures (Frankenstein, Tale of Two Cities)
```

## Tested against

Four book-length epubs covering three converter/publisher variants:

| Book | Source | Spine files | Key challenge |
|------|--------|-------------|---------------|
| The Grieving Brain (O'Connor) | HarperCollins via converter | 27 | Subheadings in styled `<p>` tags, obfuscated filenames |
| Shattered Assumptions (Janoff-Bulman) | Simon & Schuster via converter | 25 | No heading tags at all, chapter titles in `<p>` tags |
| Frankenstein (Shelley) | Project Gutenberg | 31 | PG header/footer boilerplate, `div.chapter` splitting |
| A Tale of Two Cities (Dickens) | Project Gutenberg | 47 | Illustrations, many spine files, long novel |

The first two (converter output from purchased books) live outside this repo in `~/source/claylo/failing-to-die/ref/epub-queue/`.

## What's next

Nothing pressing — this is in good shape. Possible future work:

1. **Per-chapter file splitting** — the heading structure is clean enough to split on `## Chapter` headings in post-processing, but a `--split-chapters` flag could do it natively and produce `book-id/01-chapter-name.md` files.

2. **Standard Ebooks fixtures** — SE uses proper EPUB3 semantic markup (`epub:type` attributes, semantic HTML5). Would exercise a different code path than Gutenberg/converter output. Their download endpoint requires JavaScript; would need browser automation or building from their GitHub repos.

3. **Anchor link accuracy** — TOC anchors don't resolve when the XHTML splits a heading across multiple elements (e.g. `<h2>Chapter 1</h2>` + `<h2>Walking in the Dark</h2>` as two headings, but the nav label is "1. Walking in the Dark"). Fixing this would require matching nav labels to heading text at extraction time and emitting combined headings.

4. **Items from earlier handoffs still open:**
   - Inert `-q`/`-v` flags (wired to `CommonArgs` but not plumbed)
   - No CI
   - Missing `ref/` directory structure
   - Stale audit

## Landmines

- **Book detection threshold is 5 spine files.** An academic paper with supplementary materials and a nav document could theoretically cross this threshold. In practice, academic publisher epubs have 1–3 spine files and rarely include a nav document, so this hasn't been a problem.
- **Subheading CSS heuristic requires a size gap.** If a book's stylesheet uses the same font-size for chapter titles and subheadings (distinguishing only by weight or margin), the heuristic won't find subheadings. The bold/weight filter also means bold subheadings won't be promoted.
- **Gutenberg epub fixtures are ~8MB combined.** They're binary files in `tests/golden/input/books/`. If repo size becomes a concern, they could move to Git LFS or a download script.
