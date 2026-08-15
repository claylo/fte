# Follow-up: Elsevier HTML extracts headings but no body text

**Reported by Clay, 2026-08-15.** Fixture: `scratch/leach-2018.html` (1.5MB,
ScienceDirect via a proxy host). Pre-existing bug — not introduced by the CLI
restructure work. Reproduced on this branch.

## Symptom

`fte extract --stdout scratch/leach-2018.html` produces 48 lines: correct
frontmatter, correct `# title`, all 14 `<h2>`/`<h3>` headings, one figure
caption, one download list. **Zero paragraphs of prose.**

## Root cause

Two facts combine.

**1. Elsevier wraps paragraphs in `<div>`, not `<p>`.** In the body region of
this file:

| markup | count |
|--------|-------|
| `<div class="u-margin-s-bottom" id="pNNNN">` | 50 |
| `<h2>` | 14 |
| `<p>` | 7 |

The 50 divs are the article's paragraphs. The 7 `<p>` tags are figure captions
and boilerplate — which is exactly what survived extraction.

**2. `walk_element`'s loose-text branch is a stub that discards everything.**
`src/html.rs:272-280`:

```rust
Node::Text(text)
    // Only emit loose text at reasonable depth (avoid nav text etc.)
    if depth < 20 => {
        let t = text.text.trim();
        if !t.is_empty() && t.len() > 2 {
            // Heuristic: skip very short text nodes (likely cruft)
        }
    }
```

The `if` body is **empty**. It binds `t`, tests it, and does nothing. Every text
node not inside a recognized block element (`p`, `h1`–`h6`, `li`, `blockquote`,
`figcaption`, `table`) is silently dropped.

So the walk goes `div` → `_ => {}` → recurse → `span` → `_ => {}` → recurse →
`Node::Text` → discarded. Headings survive because `h2` is a recognized arm.

This is a latent bug for any publisher whose body text is not in `<p>`, not
just Elsevier.

## Fix options

**(a) Emit loose text during recursion.** Fill in the stub: accumulate text
nodes and flush as a paragraph. Fixes every publisher at once, but changes the
walker for ALL formats — every existing golden file would need re-verification,
and it risks pulling in nav/chrome text that the current silence happens to
suppress. High blast radius.

**(b) Config-driven paragraph selectors.** Add `para_selectors` to
`PublisherProfile`; a matching element is treated exactly like `<p>`. Elsevier
declares `div.u-margin-s-bottom`. Matches the project's stated design — the
README's first line is "Config-driven: add new publishers with YAML, no
recompile needed" — and touches no other publisher's output. Narrow blast
radius, and a user can fix a new publisher without a release.

**(c) Both.** (b) now for a correct Elsevier fix, and treat the dead branch in
(a) as its own decision, since leaving obviously-dead code with a misleading
comment is its own defect.

Recommendation: **(c)**, with (b) first. The dead-stub branch should at minimum
gain a comment saying it deliberately drops loose text, or be deleted — as
written it reads like an unfinished thought and will mislead the next reader.

## Verification for whichever fix lands

- `leach-2018.html` must produce ~50 paragraphs of prose under its 14 headings.
- Every existing golden file in `tests/golden/expected/` must stay
  byte-identical (that is the whole point of choosing (b) over (a)).
- Add `leach-2018.html` as a golden fixture so this cannot regress. Note the
  file is 1.5MB; consider trimming it to the article body first, the way the
  other HTML fixtures appear to be.
