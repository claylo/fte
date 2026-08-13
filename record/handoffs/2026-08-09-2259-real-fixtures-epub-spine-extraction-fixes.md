# Handoff: real fixtures, ePub spine discovery, extraction fixes

**Date:** 2026-08-09
**Branch:** main
**State:** Green — 92/92 tests pass (nextest), clippy clean (`-D warnings`, toolchain 1.97.1)

## What was done

### 1. Real publisher fixtures (all manually saved via browser)

Added 10 new golden tests covering 7 publishers across 3 formats:

| File | Publisher | Format | Detection |
|------|-----------|--------|-----------|
| bmcgenomics-passionfruit.html | BMC Genomics | HTML | `springer` profile (shared platform) |
| wiley-forest-defoliation.html | Wiley | HTML | `wiley-html` profile |
| oup-dbapis.html | OUP | HTML | `oup-journal` profile |
| cambridge-academic-publishing.html | Cambridge | HTML | `cambridge` profile |
| taylor-francis-readership-awareness.html | T&F | HTML | `tandf` profile |
| mdpi-polymer-waveguide-sensor.html | MDPI | HTML | `mdpi` profile |
| frontiers-angelshark-real.html | Frontiers (v4 Nuxt) | HTML | `frontiers` profile |
| frontiers-angelshark.xml | Frontiers | JATS XML | `jats` (NLM DTD v2.3) |
| tandf-epub-readership-awareness.epub | T&F | ePub | OPF spine discovery |
| frontiers-epub-angelshark.epub | Frontiers | ePub | OPF spine discovery |

The `_files/` companion directories from browser Save As are on disk but intentionally not committed — no test references them.

### 2. ePub spine discovery

Rewrote `epub.rs` to discover content files via the EPUB standard path: `META-INF/container.xml` → OPF rootfile → manifest + spine. Previously hardcoded `EPUB/xhtml/index.xhtml`, which only worked for T&F/Sage.

Added OPF Dublin Core metadata (`dc:title`, `dc:creator`, `dc:identifier`) as fallback when the XHTML lacks schema.org properties. The Frontiers ePub uses this path.

Added DOI fallback: scans any `<a>` linking to `doi.org` when `a[property="sameAs"]` isn't present.

### 3. Frontiers v4 profile update

Frontiers moved to a Nuxt SPA platform. Updated the `frontiers` profile:
- Added `div.ArticleContent` as the primary body selector (v4 DOM)
- Added `button.ArticleReference` to cruft selectors (inline citation popups)
- Moved `div.References` from `ref_selector` to `cruft_selectors`
- Kept old selectors (`JournalFullText`, `article-section`) for compatibility

### 4. Three extraction correctness fixes

**`<ol>` numbering (html.rs):** `el.children().enumerate()` counted all child nodes including whitespace text nodes between `<li>` elements, producing `2, 4, 6, 8` instead of `1, 2, 3, 4`. Fixed with a dedicated `li_index` counter.

**Duplicate paragraphs (jats.rs, wiley_xml.rs):** `extract_sections` called `collect_paragraphs` on each `<sec>`/`<section>`, then recursed into the same node — the `else if child.has_tag_name("p")` branch emitted every paragraph a second time. Unified into a single walk: heading → recurse (handles `<p>`, `<table-wrap>`, `<fig>`, nested `<sec>` in one pass). Net -355 lines of duplicate content removed from golden output.

**Dublin Core metadata fallback (html.rs):** Added `dc.Title` and `dc.Creator` as fallbacks when `citation_title`/`citation_author` are absent. T&F HTML now gets proper title (without "Full article:" prefix) and both authors. `citation_*` takes priority when present; `dc.Creator` values are normalized to collapse double spaces.

## Changed files

```
M  src/config.rs        — frontiers profile: ArticleContent selector, reference stripping
M  src/epub.rs          — OPF spine discovery, Dublin Core metadata fallback, DOI <a> scanner
M  src/html.rs          — ol numbering fix, dc.Title/dc.Creator metadata fallback
M  src/jats.rs          — deduplicate paragraphs in extract_sections
M  src/wiley_xml.rs     — deduplicate paragraphs in extract_sections
M  tests/golden.rs      — golden_epub() helper, 10 new test functions
M  tests/golden/SOURCES.md — updated with all fixture provenance
A  tests/golden/input/  — 7 HTML, 1 XML, 2 ePub real fixtures
M  tests/golden/expected/ — 10 new + 28 updated golden output files
```

## What's next

1. **Build a crawl script** that re-fetches SOURCES.md URLs and diffs against committed fixtures. Detects publisher platform changes before they silently break extraction.

2. **Items from earlier handoffs still open:**
   - Inert `-q`/`-v` flags (wired to `CommonArgs` but not plumbed)
   - No CI
   - Missing `ref/` directory structure
   - Stale audit

3. **Frontiers ePub body content quality.** The Frontiers ePub extracts but includes a header table ("ORIGINAL RESEARCH / published: ..."), author block, editor/reviewer block, and correspondence block before the article body. These could be stripped with additional cruft selectors in `parse_epub_xhtml`. The body content and references are otherwise complete.

4. **ePub metadata for Frontiers.** OPF `dc:creator` gives authors as `"Janušonis S, Metzler R and Vojta T"` (all in one field, abbreviated). The XHTML has full names but in a `<p class="author">` element with complex inline markup — not currently parsed. JATS XML or the HTML version give better author metadata for the same article.

## Landmines

- **`UPDATE_GOLDEN=1` regenerates ALL expected files.** Review the diff before committing. The three extraction fixes changed 28 golden files — all verified via spot-checking the diffs.
- **ePub fixtures share file stems with other formats.** `frontiers-angelshark.xml` and an ePub with the same stem would collide on `frontiers-angelshark.md`. The ePub fixtures use distinct stems (`frontiers-epub-angelshark.epub`, `tandf-epub-readership-awareness.epub`) to avoid this.
- **`_files/` directories are not committed.** The browser Save As companion directories are on disk in `tests/golden/input/` (~93MB total) but not in git. They're not needed for tests. If you `git clean -fd` you'll lose them.
