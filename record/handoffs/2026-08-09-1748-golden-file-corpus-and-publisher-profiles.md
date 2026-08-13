# Handoff: golden-file corpus and publisher profiles

**Date:** 2026-08-09
**Branch:** main (uncommitted — see Changed Files below)
**State:** Green — 82/82 tests pass (nextest), clippy clean (`-D warnings`, toolchain 1.97.1)

## What was done

### 1. Golden-file test corpus

Built the extraction fidelity test infrastructure called out as gap #1 in the previous handoff. 59 golden tests covering every processor and profile.

**Harness:** `tests/golden.rs` — loads input fixtures, runs them through detect → extract, compares against committed expected output. Set `UPDATE_GOLDEN=1` to accept new output. Diffs on failure use the `diff` crate for readable output.

**Lib/bin split:** Created `src/lib.rs` re-exporting all modules; `src/main.rs` now uses the library crate. Required for integration tests to import the pipeline.

**Fixture breakdown:**
- 42 synthetic HTML fixtures (minimal but targeted)
- 9 synthetic XML fixtures (JATS and Wiley WML3G)
- 4 real PLOS/Springer HTML fetched from publisher sites
- 4 real JATS XML fetched from PubMed Central API
- 3 bail tests (sage-html abstract-only, tandf abstract-only, cambridge no-content)

### 2. Six new publisher profiles

Added to `default_publishers()` in `config.rs`:

| Profile | Detection markers | Notes |
|---------|------------------|-------|
| `acs` | `pubs.acs.org`, `pubs-acs-org`, `NLM_sec_level` | DOM marker + domain |
| `elsevier` | `sciencedirect.com`, `sciencedirect-com`, `sd-article` | Tightened — see Bugs |
| `frontiers` | `frontiersin.org`, `frontiersin-org` | Frontiers moved to Nuxt SPA; profile may need selector updates |
| `ieee` | `ieeexplore.ieee.org`, `ieeexplore-ieee-org` | |
| `mdpi` | `mdpi.com`, `mdpi-com` | |
| `sage-html` | (already existed) | |

**Removed:** `bmc` profile. BMC articles use the same Springer Nature platform markup (`c-article-body`) so they match the existing `springer` profile. The standalone `bmc` profile's `biomedcentral.com` marker collided with Springer pages that reference BMC in JavaScript — see Bugs.

### 3. Bugs found

Real publisher HTML exposed detection collisions that synthetic fixtures missed:

1. **Elsevier detection collision (fixed).** `elsevier.com` appeared in PLOS article text (a Scopus link). Elsevier sorted before PLOS alphabetically and stole detection. Fix: replaced `elsevier.com`/`elsevier-com` markers with `sd-article` (Elsevier DOM class).

2. **BMC detection collision (fixed).** `biomedcentral.com` appeared in Springer Nature's consent/tracking JavaScript. BMC sorted before Springer and stole detection. Fix: removed BMC profile entirely since BMC shares Springer's DOM.

3. **Duplicate paragraphs in JATS and Wiley XML (pinned, not fixed).** Both `jats.rs` and `wiley_xml.rs` emit paragraphs twice — once from `extract_sections` walking `<p>` children, once from the parent-level paragraph handler. Visible in golden output. Root cause: `extract_sections` recurses into `<sec>` children and also handles direct `<p>` children, but the paragraph text appears both in the section walk and in the parent's direct-child pass.

4. **Wrong `<ol>` numbering (pinned, not fixed).** `html.rs:197` uses `el.children().enumerate()` which counts all child nodes including whitespace text nodes between `<li>` elements. Produces `2. A`, `4. B`, `6. C` instead of `1. A`, `2. B`, `3. C`.

5. **T&F metadata gap (noted).** Taylor & Francis uses `dc.Title` and `dc.Creator` meta tags instead of `citation_title` and `citation_author`. The extractor only reads `citation_*` tags. Title falls back to `<title>` tag stripping, but authors are lost.

### 4. URL manifest

`tests/golden/SOURCES.md` records the source URL, publisher, and fetch date for every real fixture. Also lists publisher URLs that were verified in-browser but couldn't be saved programmatically (Cloudflare blocks curl; Chrome blocked JS-triggered downloads from HTTPS pages to localhost).

## Changed files

```
M  Cargo.lock           — added diff 0.1.13 dev-dependency
M  Cargo.toml           — [dev-dependencies] diff = "0.1.13"
M  src/config.rs        — 6 new profiles, removed bmc, tightened elsevier markers
M  src/main.rs          — replaced mod declarations with `use fte::{...}`
A  src/lib.rs            — pub mod re-exports for integration tests
A  tests/golden.rs       — 59 golden tests
A  tests/golden/SOURCES.md
A  tests/golden/input/   — 59 fixture files
A  tests/golden/expected/ — 56 expected output files (3 fixtures are bail tests)
```

## What's next

1. **Grab real HTML for remaining publishers.** SOURCES.md lists URLs for Wiley, OUP, Cambridge, T&F, MDPI, IEEE, ACS that were verified in-browser but need manual "Save As" to get the HTML on disk. Once saved, copy to `tests/golden/input/`, add test functions, run with `UPDATE_GOLDEN=1`. Frontiers moved to a Nuxt SPA that redirects `/full` to `/abstract` — may need a different approach.

2. **Fix the `<ol>` numbering bug.** `html.rs:197` — filter `el.children()` to only count `li` elements for the index. The fix is trivial but changes golden output for every fixture with ordered lists.

3. **Fix duplicate paragraphs in JATS/Wiley XML.** Root cause is in `extract_sections` — paragraphs are emitted both when encountered as direct children of `<body>` and when recursing into `<sec>`. Needs careful scoping so each `<p>` is emitted exactly once.

4. **Add `dc.Title`/`dc.Creator` metadata fallback.** T&F and possibly other publishers use Dublin Core meta tags. `extract_meta_tags` should try `dc.Title` when `citation_title` is absent, and `dc.Creator` when no `citation_author` tags are found.

5. **Build a crawl script** that re-fetches SOURCES.md URLs and diffs against committed fixtures. Detects publisher platform changes before they silently break extraction.

6. **Items 2–5 from the previous handoff** remain open: inert `-q`/`-v` flags, no CI, missing `ref/` directory, stale audit.

## Landmines

- **`UPDATE_GOLDEN=1` regenerates ALL expected files**, not just new ones. Safe to run, but review the diff before committing — a code change that alters extraction output will update every affected golden file silently.
- **Detection is first-match-wins in BTreeMap order.** Adding a new profile with broad domain-name markers can steal detection from existing profiles. Real HTML fixtures are the only reliable way to catch this — synthetic fixtures with isolated markers won't reproduce cross-publisher collisions.
- **Real fixtures are large.** The Springer Nature Comms HTML is 645KB. Four real JATS XMLs total ~350KB. This is fine for a test corpus but will grow as more publishers are added.
- **Frontiers has changed its platform.** Their Nuxt SPA doesn't serve full-text HTML at `/full` anymore — it redirects to `/abstract`. The `frontiers` profile selectors (`JournalFullText`, `article-section`) may no longer match their rendered DOM. Needs investigation with a successfully-loaded full-text page.
