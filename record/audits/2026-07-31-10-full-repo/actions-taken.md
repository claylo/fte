---
audit: 2026-07-31-10-full-repo
last_updated: 2026-07-31
status:
  fixed: 3
  mitigated: 0
  accepted: 0
  disputed: 0
  deferred: 0
  open: 18
---

# Actions Taken: Full repository audit of the fte Rust CLI, source, configuration, and dependency graph

Summary of remediation status for the [2026-07-31 Full repository audit of the fte Rust CLI, source, configuration, and dependency graph audit](README.md).

---

## 2026-07-31 — Bound untrusted document nesting, size, and prefix slicing

**Disposition:** fixed
**Addresses:** [utf8-prefix-byte-slicing-panics](README.md#utf8-prefix-byte-slicing-panics), [unbounded-recursive-document-walks](README.md#unbounded-recursive-document-walks), [epub-declared-size-drives-capacity](README.md#epub-declared-size-drives-capacity)
**Commit:** PENDING — staged, `commit.txt` written, awaiting `gtxt`
**Author:** Claude (Opus 5), directed by Clay Loveless

All three findings in the Untrusted Document Boundary Surface that carry a
`significant` concern level, addressed together because they share one entry
point: a document fte was asked to extract. Two of the three abort the process
rather than returning an error.

`utf8-prefix-byte-slicing-panics` — format detection sampled a fixed byte count
and indexed the `&str` directly. A new `char_safe_prefix` helper returns a
`&str` rather than a length, which makes the unsafe slice unrepresentable at
the call site; `detect_html` previously computed a bounded length with `min()`
and then indexed anyway, which is why the bug survived a reading. Hand-rolled
rather than using `str::floor_char_boundary`, which stabilized in Rust 1.91
while this crate's `rust-version` floor is 1.89.

`unbounded-recursive-document-walks` — **the finding's stated mechanism is
correct for HTML and wrong for XML, and the difference determines the fix.**
The finding locates the overflow in fte's tree walkers across all three
parsers. Measured on this toolchain, worst case (debug build, 2 MB thread):
`html5ever` parses 20,000-deep input without trouble and `walk_element` aborts
between 5,000 and 20,000 — fte's own recursion, as described. But `roxmltree`
aborts inside `Document::parse` between depth 110 and 128, on a 2.7 KB
document, before any fte code runs. A depth cap in `extract_sections`,
`text_content`, or `inline_text` would therefore have been dead code.

The XML paths instead get a pre-parse byte scanner (`xml_depth_exceeds`) at
both `extract()` entry points, which transitively bounds all three XML
walkers. The HTML path gets the depth cap the finding describes, applied to
`walk_element`, `inline_markdown`, and `element_text`; public signatures are
unchanged, so `epub.rs` needed no edits. A stack overflow is `SIGABRT` rather
than a panic, so nothing downstream can catch it — the guard has to run before
the parser, not around it.

`epub-declared-size-drives-capacity` — `read_entry` passed the archive's
declared uncompressed size straight to `String::with_capacity`. Both halves of
that size are untrusted and are now checked separately: the declaration is
rejected against a cap and reserved with `try_reserve_exact`, and the read
itself is capped at `max + 1` so a central directory that understates the real
stream cannot expand past the limit either.

Limits live in the new `src/depth.rs` alongside the measurements that justify
them, set for the worst build configuration so `cargo test` and a release
binary behave identically:

    ~~~rust src/depth.rs
    pub const MAX_XML_DEPTH: usize = 64;
    pub const MAX_HTML_DEPTH: usize = 512;

    // `roxmltree` aborts between depth 110 and 128 in the worst measured
    // configuration. A limit at or above that makes the guard useless, so this is
    // checked at compile time rather than left to a test that could be skipped.
    const _: () = assert!(MAX_XML_DEPTH < 110);
    ~~~

fte had no test infrastructure before this work. `just test` now runs 23 tests,
all passing. One takes 14.4s and the other 22 are milliseconds — the slow one
parses 20,000 nested divs in debug, which is the depth that actually aborted,
so shrinking it would weaken the regression.

Two things this entry does not claim. `just clippy` still fails with the 28
pre-existing errors tracked as `clippy-baseline-fails`; this change adds none
of them and fixes none of them, so `just check` cannot pass yet. And the case
where a zip's central directory *understates* the expanded size is guarded in
code but not covered by a test, since constructing that archive requires
hand-built bytes rather than the `zip` writer.

---
