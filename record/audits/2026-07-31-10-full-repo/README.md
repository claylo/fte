---
audit_date: 2026-07-31
project: fte
commit: abb332f7ea069ef190953021e2f87e34b3f0267a
scope: Full repository audit of the fte Rust CLI, source, configuration, and dependency graph
auditor: Codex (GPT-5), cased with crustoleum Rust surfaces
findings:
  critical: 0
  significant: 7
  moderate: 7
  advisory: 7
  note: 0
---

# Audit: fte

fte is a compact, readable extractor with no unsafe code and no known
vulnerable dependencies. **The Untrusted Document Boundary Surface** permits
valid Unicode, nesting depth, and ePub metadata to terminate the process.
**The Extraction Fidelity Surface** and **The Batch CLI Contract Surface** can
turn omitted content or failed work into plausible success. **The Performance,
Supply Chain, Static Analysis, and Type Design Surfaces** are fundamentally
sound but carry a focused cleanup list. The priority is to make incomplete or
unbounded extraction impossible to report as success.

---

## The Untrusted Document Boundary Surface

*Document bytes cross into string slicing, archive allocation, and recursive
traversal without consistent boundary checks.*

### utf8-prefix-byte-slicing-panics

UTF-8 prefix sampling can panic at arbitrary byte boundaries.

**significant** · `src/detect.rs:56-61` · effort: small ·
<img src="assets/sparkline-utf8-prefix-byte-slicing-panics.svg" height="14" alt="commit activity" />

XML and HTML detection derive prefix lengths in bytes and index `str` values
directly. A valid document whose multi-byte character crosses byte 2,048 or
150,000 makes the index cease to be a character boundary, panicking before
normal extraction error handling runs.

```rust
fn detect_xml(content: &str) -> Format {
    let head: &str = if content.len() > 2048 {
        &content[..2048]
    } else {
        content
    };
```

> I do not need malformed markup. I only need an ordinary multi-byte character
> to straddle a fixed sampling offset.

Related: [unbounded-recursive-document-walks](#unbounded-recursive-document-walks),
[epub-declared-size-drives-capacity](#epub-declared-size-drives-capacity).

**Remediation:** Clamp the prefix to a character boundary, or search the
encoded byte sequence of each configured marker. Cover multi-byte characters
spanning both limits.

<div>&hairsp;</div>

### unbounded-recursive-document-walks

Untrusted document nesting can exhaust the process stack.

**significant** · `src/html.rs:245-251` · effort: medium ·
<img src="assets/sparkline-unbounded-recursive-document-walks.svg" height="14" alt="commit activity" />

HTML, ePub XHTML, JATS, and Wiley XML all reach recursive tree walkers whose
depth is controlled by document nesting. The HTML depth parameter changes
rendering behavior but never stops recursion; XML caps heading depth, not call
depth.

```rust
    // Recurse into child elements
    for child in el.children() {
        match child.value() {
            Node::Element(_) => {
                if let Some(child_el) = ElementRef::wrap(child) {
                    walk_element(&child_el, cruft, out, depth + 1);
                }
```

> Deeply nested but syntactically valid input turns a document parser into a
> stack-exhaustion switch.

Related: [recursive-renderers-allocate-per-node](#recursive-renderers-allocate-per-node).

**Remediation:** Use an explicit heap-backed work stack, or enforce one shared
maximum depth and return a contextual error across every recursive walker.

<div>&hairsp;</div>

### epub-declared-size-drives-capacity

Untrusted ePub size metadata drives an unchecked allocation.

**significant** · `src/epub.rs:60-64` · effort: medium ·
<img src="assets/sparkline-epub-declared-size-drives-capacity.svg" height="14" alt="commit activity" />

The archive-controlled uncompressed `u64` size is cast to `usize` and passed
straight to `String::with_capacity`. Oversized metadata can cause a capacity
panic, allocation abort, or narrowing-target truncation before the entry is
read.

```rust
fn read_entry(archive: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Result<String> {
    let mut entry = archive.by_name(name)?;
    let mut buf = String::with_capacity(entry.size() as usize);
    entry.read_to_string(&mut buf)?;
    Ok(buf)
```

> The central directory tells the allocator how ambitious to be. It currently
> has no upper bound.

Related: [epub-overflow-read-errors-are-silently-discarded](#epub-overflow-read-errors-are-silently-discarded).

**Remediation:** Set a maximum expanded-entry size, use `usize::try_from` and
`try_reserve`, and enforce the same limit with a bounded streaming read.

<div>&hairsp;</div>

### declared-document-encodings-unsupported

HTML, XML, and ePub inputs are restricted to UTF-8 without documentation.

**moderate** · `src/main.rs:124-128` · effort: medium ·
<img src="assets/sparkline-declared-document-encodings-unsupported.svg" height="14" alt="commit activity" />

`read_to_string` rejects non-UTF-8 bytes before detection or parsing, and ePub
entries use the equivalent path. XML commonly declares UTF-16, while publisher
HTML may use BOM or charset declarations; the README does not disclose this
narrower contract.

```rust
// HTML/XML: read to string, detect, extract
let content =
    fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;

let format = detect::detect_format(path, &content, &cfg);
```

**Remediation:** Read bytes, detect BOM and XML/HTML encoding declarations,
decode to Unicode, and apply the same handling to ePub XHTML members.

*Verdict: memory-safe parsing is not yet resource-safe parsing; these four
findings should be remediated as one input-boundary hardening project.*

---

## The Extraction Fidelity Surface

*Several failure paths return plausible Markdown while silently omitting
configured or source content.*

### ref-selector-has-no-effect

The documented `ref_selector` profile field is never applied.

**significant** · `src/html.rs:57-63` · effort: small ·
<img src="assets/sparkline-ref-selector-has-no-effect.svg" height="14" alt="commit activity" />

Five built-in profiles populate `ref_selector`, and the README documents it as
the references subtree to strip. Extraction compiles only `cruft_selectors`, so
the field has no behavioral reader and silently does nothing.

```rust
let cruft_sels: Vec<Selector> = profile
    .cruft_selectors
    .iter()
    .filter_map(|s| Selector::parse(s).ok())
    .collect();

walk_element(&body_el, &cruft_sels, &mut body, 0);
```

> A configuration key that parses cleanly but is never consumed looks exactly
> like a working feature until someone compares the output.

Related: [invalid-css-selectors-are-silently-discarded](#invalid-css-selectors-are-silently-discarded).

**Remediation:** Compile `ref_selector` into the removal set, propagate invalid
selector errors, and prove with a profile fixture that the references subtree
is excluded.

<div>&hairsp;</div>

### xml-block-content-silently-omitted

JATS and Wiley extraction omit lists and nested block content.

**significant** · `src/jats.rs:142-169` · effort: medium ·
<img src="assets/sparkline-xml-block-content-silently-omitted.svg" height="14" alt="commit activity" />

The section walkers emit direct paragraphs, tables, figures, and subsections.
They have no branch or generic recursion for lists, definition lists, boxed
text, quotations, or statements, so those nodes disappear without an error.

```rust
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
```

> The output is syntactically tidy, which makes missing source blocks harder
> to notice than a hard failure would be.

**Remediation:** Replace the whitelist with an order-preserving block walker
covering lists, definition lists, boxes, quotations, nested paragraphs,
tables, figures, and subsections for both XML dialects.

<div>&hairsp;</div>

### invalid-css-selectors-are-silently-discarded

Invalid configured CSS selectors lose their parse errors.

**moderate** · `src/html.rs:39-42` · effort: small ·
<img src="assets/sparkline-invalid-css-selectors-are-silently-discarded.svg" height="14" alt="commit activity" />

`.ok()` erases user-configuration parse failures. Invalid body selectors become
indistinguishable from valid selectors that matched nothing, while invalid
cruft selectors are dropped and extraction may succeed with unwanted content.

```rust
    let body_el = profile
        .body_selectors
        .iter()
        .find_map(|sel| Selector::parse(sel).ok().and_then(|s| doc.select(&s).next()));
```

Related: [css-selectors-recompiled-per-document](#css-selectors-recompiled-per-document).

**Remediation:** Compile and validate selectors when loading each profile, then
return an error naming the publisher, selector class, selector text, and parser
cause.

<div>&hairsp;</div>

### epub-overflow-read-errors-are-silently-discarded

Corrupt ePub overflow members produce incomplete successful output.

**moderate** · `src/epub.rs:29-36` · effort: small ·
<img src="assets/sparkline-epub-overflow-read-errors-are-silently-discarded.svg" height="14" alt="commit activity" />

Table and figure members are read with `if let Ok` and no error branch.
Decompression, CRC, I/O, or UTF-8 errors therefore omit content without warning
while extraction proceeds as a success.

```rust
    for name in &table_names {
        if let Ok(content) = read_entry(&mut archive, name) {
            // Extract just the <body> content from overflow files
            main_xhtml.push_str("\n<!-- overflow: ");
            main_xhtml.push_str(name);
            main_xhtml.push_str(" -->\n");
            main_xhtml.push_str(&content);
        }
```

**Remediation:** Propagate member failures with archive and member context. If
members are optional, surface structured warnings and do not present the result
as unqualified success.

*Verdict: extraction needs an explicit completeness contract; silently omitting
known structures or errors undermines the tool's primary promise.*

---

## The Batch CLI Contract Surface

*Several batch outcomes diverge from their command-line contract while still
ending in process success.*

### stdout-skipped-when-output-exists

stdout mode silently skips files whose output already exists.

**significant** · `src/main.rs:135-144` · effort: small ·
<img src="assets/sparkline-stdout-skipped-when-output-exists.svg" height="14" alt="commit activity" />

`--stdout` is evaluated only after the normal output-file existence check. If
`<id>.md` already exists, fte emits nothing and counts the input as skipped
unless the unrelated `--force` option is supplied. The ePub path repeats this
ordering.

```rust
let out_path = output_dir.join(format!("{id}.md"));
if !cli.force && out_path.exists() {
    skip += 1;
    continue;
}

match extract::extract(&format, id, &content) {
    Ok(md) => {
        if cli.stdout {
            println!("{md}");
```

**Remediation:** Bypass output-file existence checks whenever `cli.stdout` is
true in both document branches, with a regression case using a pre-existing
output file.

<div>&hairsp;</div>

### documented-user-config-path-wrong-on-macos

The documented user config path is not discovered on macOS.

**significant** · `README.md:50-58` · effort: small ·
<img src="assets/sparkline-documented-user-config-path-wrong-on-macos.svg" height="14" alt="commit activity" />

fte delegates user configuration to librebar's `ProjectDirs`. On macOS that
resolves beneath `~/Library/Application Support/fte`, while the README directs
users to `~/.config/fte/config.yaml`. A valid file at the documented path is
silently ignored.

```markdown
## Configuration

fte uses [librebar](https://crates.io/crates/librebar) for config discovery. Config files are found by walking up from the working directory, checking for `.fte.yaml`, `.config/fte.yaml`, or `fte.yaml` (also `.toml` and `.json`). User-level config lives at `~/.config/fte/config.yaml`.

Merge order (lowest to highest precedence):
1. Compiled defaults (all 8 publishers built in)
2. User config (`~/.config/fte/config.yaml`)
3. Project config (`.fte.yaml` in repo root)
4. CLI flags (`--indir`, `--outdir`)
```

**Remediation:** Document the actual platform-specific paths, or explicitly
support the cross-platform path promised by the README. A config-inspection
command should report loaded sources.

<div>&hairsp;</div>

### directory-entry-errors-are-silently-discarded

Directory scan errors silently remove inputs from the batch.

**moderate** · `src/main.rs:170-178` · effort: small ·
<img src="assets/sparkline-directory-entry-errors-are-silently-discarded.svg" height="14" alt="commit activity" />

`ReadDir` can fail after the directory opens. Converting each result with
`.ok()` makes affected entries disappear from both the input set and the final
failure count.

```rust
        let mut paths: Vec<PathBuf> = fs::read_dir(input_dir)
            .with_context(|| format!("reading {}", input_dir.display()))?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.extension()
                    .is_some_and(|ext| ext == "html" || ext == "xml" || ext == "epub")
            })
            .collect();
```

**Remediation:** Collect directory entries through `Result`; either propagate
the error or report each failed entry and include it in the failure total.

<div>&hairsp;</div>

### extraction-failures-return-success

Per-file extraction failures still produce a successful process exit.

**moderate** · `src/main.rs:153-164` · effort: trivial ·
<img src="assets/sparkline-extraction-failures-return-success.svg" height="14" alt="commit activity" />

The batch correctly continues after a handler failure, but the accumulated
failure count never affects `main`'s result. Shell scripts and CI receive exit
status zero even when every requested extraction failed.

```rust
Err(e) => {
    eprintln!("  FAIL {id}: {e}");
    fail += 1;
}
}
}

if !cli.stdout && !cli.detect_only {
    eprintln!("\nDone: {ok} extracted, {skip} skipped, {fail} failed");
}

Ok(())
```

**Remediation:** Finish the batch and print the summary, then return nonzero
when `fail > 0`. Document and test all-success, partial-failure, and all-failure
exit behavior.

*Verdict: the batch workflow is usable interactively but not yet trustworthy
as an automation boundary.*

---

## The Performance Surface

*Parsing costs are dominated by avoidable full-document copies and per-node
buffer churn rather than the parsers themselves.*

### publisher-profile-cloned-per-document

HTML detection deep-clones immutable publisher profiles.

**moderate** · `src/detect.rs:75-93` · effort: small ·
<img src="assets/sparkline-publisher-profile-cloned-per-document.svg" height="14" alt="commit activity" />

Each profile contains four `Vec<String>` fields and optional strings. Detection
deep-clones that data for every HTML input, despite process-wide immutability,
and inflates the `Format` enum enough for Clippy to report a 224-byte variant.

```rust
fn detect_html(content: &str, config: &Config) -> Format {
    let sample_len = content.len().min(150_000);
    let head = &content[..sample_len];

    // Iterate publishers in order, first match wins
    for (name, profile) in &config.publishers {
        if matches_profile(head, profile) {
            return Format::Html {
                name: name.clone(),
                profile: profile.clone(),
            };
        }
    }

    // No match — use fallback
    Format::Html {
        name: "other".into(),
        profile: config.fallback.clone(),
    }
```

**Remediation:** Borrow the publisher name and profile, or return a compact key
and resolve the profile during extraction.

<div>&hairsp;</div>

### recursive-renderers-allocate-per-node

Recursive text renderers allocate and recopy at every nested node.

**moderate** · `src/jats.rs:270-299` · effort: medium ·
<img src="assets/sparkline-recursive-renderers-allocate-per-node.svg" height="14" alt="commit activity" />

Each recursive call creates and grows a fresh `String`, returns it, and copies
its bytes into the parent. HTML, JATS, and Wiley share the pattern, making
allocation count scale with element count and text copying with nesting depth.

```rust
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
```

**Remediation:** Add accumulator variants such as
`inline_text_into(node, &mut String)` and recurse into one shared buffer.

<div>&hairsp;</div>

### jats-no-doctype-copies-entire-input

JATS parsing copies the entire input when no DOCTYPE exists.

**advisory** · `src/jats.rs:337-348` · effort: trivial ·
<img src="assets/sparkline-jats-no-doctype-copies-entire-input.svg" height="14" alt="commit activity" />

The unchanged path copies the input into a new `String`, so ordinary JATS files
without a DOCTYPE keep two raw-document buffers alive during extraction.

```rust
/// Strip DOCTYPE declaration from XML since roxmltree doesn't support DTDs.
fn strip_doctype(content: &str) -> String {
    if let Some(start) = content.find("<!DOCTYPE") {
        if let Some(end) = content[start..].find('>') {
            let mut out = String::with_capacity(content.len());
            out.push_str(&content[..start]);
            out.push_str(&content[start + end + 1..]);
            return out;
        }
    }
    content.to_string()
}
```

**Remediation:** Return `Cow<'_, str>`, borrowing unchanged input and allocating
only when a declaration is removed.

<div>&hairsp;</div>

### normalize-text-reallocates-result

Whitespace normalization reallocates its completed buffer.

**advisory** · `src/markdown.rs:60-75` · effort: trivial ·
<img src="assets/sparkline-normalize-text-reallocates-result.svg" height="14" alt="commit activity" />

The helper builds one capacity-sized `String`, then `trim().to_string()`
allocates and copies it again. It runs for paragraphs, headings, metadata,
citations, and table cells.

```rust
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
```

**Remediation:** Remove the possible trailing normalized space in place, then
return the existing buffer.

<div>&hairsp;</div>

### css-selectors-recompiled-per-document

Static CSS selectors are parsed again for every document.

**advisory** · `src/html.rs:38-63` · effort: medium ·
<img src="assets/sparkline-css-selectors-recompiled-per-document.svg" height="14" alt="commit activity" />

Publisher selector strings are immutable, but each HTML extraction reparses
body selectors and rebuilds the cruft set. ePub likewise recompiles a fixed
selector list, repeating parsing and allocation across directory runs.

```rust
    // Find the body content container
    let body_el = profile
        .body_selectors
        .iter()
        .find_map(|sel| Selector::parse(sel).ok().and_then(|s| doc.select(&s).next()));

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
```

**Remediation:** Compile runtime profiles once after configuration loading and
cache or lazily initialize ePub's fixed selectors as well.

*Verdict: shared accumulators and one-time configuration compilation deliver
the meaningful gains; the remaining copies are localized polish.*

---

## The Supply Chain Surface

*The dependency graph has no known vulnerabilities or unused direct
dependencies, but it carries one unmaintained transitive crate and avoidable
feature weight.*

### unmaintained-fxhash-through-scraper

HTML parsing retains an unmaintained hashing crate.

**advisory** · `.crustoleum/audit.txt:6-9` · effort: small ·
<img src="assets/sparkline-unmaintained-fxhash-through-scraper.svg" height="14" alt="commit activity" />

`scraper` depends on `selectors`, which retains `fxhash` 0.2.1. No vulnerability
is reported, but this expands maintenance and monitoring burden on the
publisher-controlled HTML path.

```text
Crate: fxhash 0.2.1
ID: RUSTSEC-2025-0057
Status: unmaintained
Path: fxhash -> selectors -> scraper -> fte
```

**Remediation:** Upgrade when an upstream release removes `fxhash`; until then,
document a time-bounded advisory exception and monitor RUSTSEC-2025-0057.

<div>&hairsp;</div>

### scraper-default-cli-feature

scraper's default CLI feature pulls an unused argument parser.

**advisory** · `Cargo.toml:15` · effort: trivial ·
<img src="assets/sparkline-scraper-default-cli-feature.svg" height="14" alt="commit activity" />

The default `main` feature brings `getopts` and `unicode-width` for scraper's
standalone executable. fte uses only library APIs.

```toml
scraper = "0.22"
```

**Remediation:** Set `default-features = false`; enable only `errors` if fte
later consumes scraper's parse-error collection.

<div>&hairsp;</div>

### zip-deflate-includes-unused-zopfli

zip's aggregate deflate feature compiles an unused encoder.

**advisory** · `Cargo.toml:17` · effort: trivial ·
<img src="assets/sparkline-zip-deflate-includes-unused-zopfli.svg" height="14" alt="commit activity" />

The aggregate feature enables both flate2/zlib-rs and zopfli. fte opens
`ZipArchive` entries but never constructs a writer, leaving zopfli unreachable.

```toml
zip = { version = "4.0", default-features = false, features = ["deflate"] }
```

**Remediation:** Use `deflate-flate2-zlib-rs`, retaining DEFLATE decompression
without the unused encoder.

*Verdict: the graph is vulnerability-clean; feature trimming and upstream
monitoring are sufficient.*

---

## The Static Analysis Surface

*The crate compiles, but its strict Clippy baseline currently fails before it
can act as a quality gate.*

### clippy-baseline-fails

Strict Clippy analysis fails with 28 denied warnings.

**advisory** · `.crustoleum/clippy.txt:101-129` · effort: small

Most failures are mechanical control-flow suggestions, but a 224-byte enum and
manual derivable `Default` are also present. A strict CI job would fail, and new
warnings cannot be distinguished from existing debt.

```text
src/markdown.rs:1:1: error: empty line after doc comment
src/detect.rs:8:1: error: large size difference between variants: the entire enum is at least 224 bytes
src/epub.rs:112:5: error: this `if` statement can be collapsed
src/epub.rs:122:5: error: this `if` statement can be collapsed
src/epub.rs:123:9: error: this `if` statement can be collapsed
src/epub.rs:147:5: error: this `if` statement can be collapsed
src/epub.rs:148:9: error: this `if` statement can be collapsed
src/epub.rs:149:13: error: this `if` statement can be collapsed
src/epub.rs:163:5: error: this `if` statement can be collapsed
src/epub.rs:179:5: error: this `if` statement can be collapsed
src/epub.rs:189:5: error: this `if` statement can be collapsed
src/epub.rs:196:5: error: this `if` statement can be collapsed
src/epub.rs:203:5: error: this `if` statement can be collapsed
src/html.rs:18:5: error: this `if` statement can be collapsed
src/html.rs:23:5: error: this `if` statement can be collapsed
src/html.rs:95:17: error: this `if` can be collapsed into the outer `match`
src/html.rs:104:5: error: this `if` statement can be collapsed
src/html.rs:105:9: error: matching on `Some` with `ok()` is redundant
src/html.rs:105:9: error: this `if` statement can be collapsed
src/html.rs:192:17: error: this `if` statement can be collapsed
src/html.rs:255:17: error: this `if` can be collapsed into the outer `match`
src/jats.rs:130:5: error: this `if` statement can be collapsed
src/jats.rs:339:5: error: this `if` statement can be collapsed
src/markdown.rs:44:1: error: this `impl` can be derived
src/wiley_xml.rs:19:5: error: this `if` statement can be collapsed
src/wiley_xml.rs:115:13: error: this `if` statement can be collapsed
src/wiley_xml.rs:124:13: error: this `if` statement can be collapsed
src/wiley_xml.rs:195:5: error: this `if` statement can be collapsed
error: could not compile `fte` (bin "fte" test) due to 28 previous errors
```

**Remediation:** Resolve the enum representation and derived `Default`
deliberately, apply the localized control-flow suggestions, then keep
`cargo clippy --all-targets -- -D warnings` clean in CI or a Justfile check.

*Verdict: this is existing lint debt rather than a correctness failure, but the
gate should be restored before the next feature tranche.*

---

## The Type Design Surface

*The private binary modules use borrowing and standard traits conventionally,
with no caller-facing type-safety defect worth elevating beyond the findings
already captured elsewhere.*

The profile ownership issue is tracked under performance, and the manual
`Default` implementation is captured by the Clippy baseline. No separate public
API remediation is warranted for this binary crate.

*Verdict: clean surface.*

---

## Remediation Ledger

| Finding | Concern | Location | Effort | Chains |
|---------|---------|----------|--------|--------|
| **The Untrusted Document Boundary Surface** | | | | |
| [utf8-prefix-byte-slicing-panics](#utf8-prefix-byte-slicing-panics) | significant | `src/detect.rs:56-61` | small | related: recursion, ePub size |
| [unbounded-recursive-document-walks](#unbounded-recursive-document-walks) | significant | `src/html.rs:245-251` | medium | related: renderer allocation |
| [epub-declared-size-drives-capacity](#epub-declared-size-drives-capacity) | significant | `src/epub.rs:60-64` | medium | related: overflow reads |
| [declared-document-encodings-unsupported](#declared-document-encodings-unsupported) | moderate | `src/main.rs:124-128` | medium | related: prefix slicing |
| **The Extraction Fidelity Surface** | | | | |
| [ref-selector-has-no-effect](#ref-selector-has-no-effect) | significant | `src/html.rs:57-63` | small | related: selector errors |
| [xml-block-content-silently-omitted](#xml-block-content-silently-omitted) | significant | `src/jats.rs:142-169` | medium | related: ePub omissions |
| [invalid-css-selectors-are-silently-discarded](#invalid-css-selectors-are-silently-discarded) | moderate | `src/html.rs:39-42` | small | related: selector compilation |
| [epub-overflow-read-errors-are-silently-discarded](#epub-overflow-read-errors-are-silently-discarded) | moderate | `src/epub.rs:29-36` | small | related: ePub size |
| **The Batch CLI Contract Surface** | | | | |
| [stdout-skipped-when-output-exists](#stdout-skipped-when-output-exists) | significant | `src/main.rs:135-144` | small | related: exit status |
| [documented-user-config-path-wrong-on-macos](#documented-user-config-path-wrong-on-macos) | significant | `README.md:50-58` | small | — |
| [directory-entry-errors-are-silently-discarded](#directory-entry-errors-are-silently-discarded) | moderate | `src/main.rs:170-178` | small | related: exit status |
| [extraction-failures-return-success](#extraction-failures-return-success) | moderate | `src/main.rs:153-164` | trivial | related: stdout, enumeration |
| **The Performance Surface** | | | | |
| [publisher-profile-cloned-per-document](#publisher-profile-cloned-per-document) | moderate | `src/detect.rs:75-93` | small | related: selectors, Clippy |
| [recursive-renderers-allocate-per-node](#recursive-renderers-allocate-per-node) | moderate | `src/jats.rs:270-299` | medium | related: recursion, normalization |
| [jats-no-doctype-copies-entire-input](#jats-no-doctype-copies-entire-input) | advisory | `src/jats.rs:337-348` | trivial | related: renderer allocation |
| [normalize-text-reallocates-result](#normalize-text-reallocates-result) | advisory | `src/markdown.rs:60-75` | trivial | related: renderer allocation |
| [css-selectors-recompiled-per-document](#css-selectors-recompiled-per-document) | advisory | `src/html.rs:38-63` | medium | related: profiles, validation |
| **The Supply Chain Surface** | | | | |
| [unmaintained-fxhash-through-scraper](#unmaintained-fxhash-through-scraper) | advisory | `.crustoleum/audit.txt:6-9` | small | related: scraper feature |
| [scraper-default-cli-feature](#scraper-default-cli-feature) | advisory | `Cargo.toml:15` | trivial | related: fxhash |
| [zip-deflate-includes-unused-zopfli](#zip-deflate-includes-unused-zopfli) | advisory | `Cargo.toml:17` | trivial | — |
| **The Static Analysis Surface** | | | | |
| [clippy-baseline-fails](#clippy-baseline-fails) | advisory | `.crustoleum/clippy.txt:101-129` | small | related: profile ownership |

<sub>
Generated 2026-07-31 at commit abb332f. Intermediate artifacts:
recon.yaml, findings.yaml. Primary report: report.html.
</sub>
