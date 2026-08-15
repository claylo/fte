# Final whole-branch review — `feat/cli-restructure-and-split` @ 997c394

Reviewer: final-review (opus). Scope: cross-task consistency and seam defects.
Not re-verified (taken as given): clean tree, `just check`, 157/157, clispec 18/24.

Every finding below was reproduced against `./target/debug/fte` built at 997c394
unless marked "by inspection".

---

## CRITICAL

### C1. `split` silently discards an unterminated chapter and exits 0

`src/chapter.rs:117-149`

`parse` accumulates a chunk on `START_PREFIX` and only pushes it to
`doc.chapters` when it sees a matching `END_PREFIX` (line 131-136). Two paths
lose content with no error:

1. **EOF with an open chunk.** The loop ends, `current` is dropped, the chapter
   is gone.
2. **A second `START_PREFIX` before the matching end** (line 120-129):
   `current = Some(...)` overwrites the in-progress chunk without pushing it.

Reproduced end to end:

```bash
# trunc.md: two chapters, the second missing its chapter-end
$ fte split --format text --outdir /tmp/t1 /tmp/trunc.md
Done: 2 files written
  OK   /tmp/t1/trunc/00-frontmatter.md (0KB)
  OK   /tmp/t1/trunc/01-one.md (0KB)
$ echo $?
0
$ grep -rl "body two IMPORTANT" /tmp/t1
# nothing — content lost silently
```

Failure scenario: a user truncates or hand-edits a book `.md` (or a future
`extract` bug drops a closing marker). `split` reports success, writes a short
tree, and the missing chapter is invisible. Exit 0 is the worst possible answer
here — a wrapper script has nothing to branch on.

The spec deliberately discards content *between* an end and the next start
(design lines 126-128). It says nothing about an unmatched start, and the code
picked silence.

**Fix:** in `parse`, either close the trailing chunk at EOF, or record the
imbalance on `Document` and have `cmd/split.rs` raise `Kind::ExtractionFailed`.
The second is better: an unbalanced document is corrupt input, not a shape to
paper over. Same treatment for the nested-start case. This also closes the Task 4
deferred minor (a raw `"` inside a hand-edited marker truncates at the quote) —
`parse` currently has no validation of any kind.

---

## IMPORTANT

### I2. The spec misdescribes the shipped code in six places

Repo rule: documentation is authoritative, so each of these is a defect. The
lead already knows about the score; the rest are new.

| Spec line | Claims | Reality |
|---|---|---|
| 33, 291, 456 | `clispec score` reaches **20/24** | 18/24. Line 456 still calls it "a projection". |
| 306-307 | **Four** librebar-blocked checks | Five librebar-blocked (+ *Output fields declared*) plus one fte-design-blocked (*Structured errors*). `.justfile` already carries the correct six; the spec does not. |
| 103 | `src` attribute is the "Spine entry path, **verbatim**" | `chapter.rs:45` runs `sanitize_attr(src)`. Also contradicts the spec's own line 112 ("Attribute values are sanitized before emission"). |
| 258 | `detect` fields are `id`, `path`, `format` | `DetectRow` and `schema_metadata()` also publish `status` (`detected \| failed`). |
| 268-277 | `--stdout` is "declared as `output_kind: opaque` with `media_type: text/markdown`" | Never built. librebar 0.6 has no such field (the spec admits this at line 303 but does not list it as blocked, so §"--stdout is opaque" reads as shipped). The *behavior* is correct and tested; only the declaration is fiction. |
| 416-417 | "`fte split` reuses the same renderer: its golden is the output file tree plus one skeleton per chapter file." | Never built. `tests/split.rs:154-158` compares one skeleton — `00-frontmatter.md` — between the one-shot and two-step runs. There is no file-tree golden and no per-chapter skeleton. |

Also stale but lower stakes: the table at line 254 has `output_kind` /
`cardinality` / `effects` columns for all three commands. None of the three are
emitted (`fte schema` still reports `"clispec": "0.2"`). Reads as declared work.

README, by contrast, is **clean** on the two things the lead flagged:

- Exit-code table (README:85-96) matches `errors.rs` code-for-code, all nine rows.
- `split` config keys (README:71-77) match `SplitConfig` exactly: `subdir`,
  `file`, `front`, `pad`, including the `""` semantics for `subdir` and `front`.
- Token list (README:79) matches `template.rs` exactly.
- `fte extract book.epub && fte split ref/epub-md/book.md` (README:66) is a
  correct path under the default `output_dir`.

One README gap, from an unfulfilled ruling — see M17.

### I3. `extract` double-prints failures in text mode

`src/cmd/extract.rs:136-154` and `:158-166`

This is the exact bug shape fixed for `split` in 19e6975, still live in
`extract`:

```
$ fte extract --format text --outdir /tmp/d tests/golden/input/sage-html-article.html
  FAIL sage-html-article.md              # stdout, from emit_items
  FAIL sage-html-article: abstract-only format   # stderr, from line 141

Done: 0 extracted, 0 skipped, 1 failed
```

The same failure is announced twice under two different identifiers
(`sage-html-article.md` vs `sage-html-article`). Worse, the two failure classes
inside the same command disagree: a *missing input* (line 60-74) prints only the
stdout row, while a *failed extraction* prints both. A user grepping `FAIL` on
either stream gets an inconsistent count.

Pick one: either the stderr line carries the reason and the stdout row is the
data (in which case the missing-input path needs a matching stderr line), or the
row is the sole report (in which case line 140-142 drops to `--stdout` only).
I recommend the latter, matching the split fix.

### I4. A failed row in JSON mode carries no reason

`src/cmd/extract.rs:136-154`

```
$ fte extract --format json --outdir /tmp/d2 tests/golden/input/sage-html-article.html
{"items":[{"bytes":0,"id":"sage-html-article","output_path":null,"source_format":"sage-html","status":"failed"}]}
# stderr: empty
```

`abstract-only format` — the whole point of the failure — exists only in text
mode. A JSON consumer sees `status: "failed"` and cannot tell whether the file
was abstract-only, unparseable, or a wrong format. Ledger line 160 defends
parking clispec's *Structured errors* check on the grounds that "the failures are
fully reported as rows in the stdout items envelope." They are not fully
reported; they are reported without a cause.

**Fix:** add an optional `reason: Option<String>` to `ExtractRow` (and
`DetectRow` for symmetry). This is additive, it is what makes the Task 10 ruling
true, and it does not touch the outcome-vs-error design.

### I5. `-v` puts plain text on stderr in JSON mode

`src/cmd/extract.rs:93-95`

```
$ fte extract -v --force --indir tests/golden/input --outdir /tmp/x1 --format json bmc-short
# stderr: "  detect bmc-short: springer-html"
```

The `if verbose` guard has no `render` check, unlike every other chatter site on
the branch (`inputs.rs:65`, `extract.rs:169`, `split.rs:94`). This directly
violates the Task 10 ruling (ledger:149): *"suppress ALL human chatter on stderr
when render == Json."* The ruling was implemented for `resolve`'s warning and the
`Done:` summaries but not for `-v`.

`tests/cli.rs::json_mode_puts_no_plain_text_on_stderr` asserts exactly this
invariant — but only for `detect`, which has no verbose path. Extending that test
to `extract -v` and `split` would have caught it.

Second, smaller leak in the same file: `extract.rs:64` prints `  FAIL {id}: not
found` to stderr in `--stdout` mode regardless of format.

### I6. Bare `fte` dumps the full help text into the JSON `message` field — and its test passes for the wrong reason

`src/main.rs:244-247`, `tests/cli.rs::bare_invocation_shows_help_and_exits_two`

```
$ fte 2>&1 >/dev/null
{"hint":"run with --help for usage","kind":"usage","message":"Extract clean markdown from publisher HTML/XML/ePub academic papers\n\nUsage: fte [OPTIONS] <COMMAND>\n\nCommands:\n  extract      Extract markdown...
```

`arg_required_else_help` produces clap's `DisplayHelpOnMissingArgumentOrSubcommand`,
which is not exempted at line 237, so the entire help screen becomes
`AppError::message` and gets JSON-escaped into a single ~700-character envelope
field. An agent consuming the envelope gets a help screen where it expects a
one-line diagnostic.

The test is the honesty problem the lead asked about. It asserts
`stderr.contains("extract")` and `stderr.contains("split")` — but `assert_cmd`
pipes stdout, so `wants_json_errors()` returns true and the assertion is matching
substrings *inside a JSON string literal*, not reading a help screen. It would
pass identically if the help text were mangled, double-escaped, or wrapped in
anything at all. It reads as a text-mode test; it is a JSON test. Same class as
the three repaired in Task 10 (ledger:161) — this one was missed because it was
*added* in this branch rather than inherited.

**Fix:** exempt `DisplayHelpOnMissingArgumentOrSubcommand` alongside
`DisplayHelp` (print help to stderr, exit `Kind::Usage.code()`), or keep the
error but set `message` to something short and print the help separately. Then
pin the test with `--format text`.

### I7. `anyhow` context drops the cause in `split`'s error mapping

`src/splitter.rs:68`, `src/splitter.rs:193`, consumed at `src/cmd/split.rs:81`

`with_context(|| format!("creating {}", dir.display()))` attaches a context
layer; the underlying `io::Error` becomes the *source*. `cmd/split.rs:81` calls
`e.to_string()`, which renders only the outermost layer. Result:

```
error: creating /path/to/out
```

No "Permission denied", no "No space left on device", no errno. The kind is
correctly `io_error` (7), so exit codes are fine — but the human message is
useless, and this is the same call site the `exists`-substring fix (08c4fe0)
already touched without noticing.

**Fix:** build the message with the chain, e.g.
`e.chain().map(ToString::to_string).collect::<Vec<_>>().join(": ")`.

---

## MINOR

**M8. Ruling on substring error mapping (ledger:118) — STANDS.** I traced every
message against every arm: `splitter.rs:49` → `no chapter markers` ✓,
`template.rs:36` → `unknown template token` ✓, `template.rs:39` → `unterminated`
✓, `splitter.rs:191` → `pass --force to overwrite` ✓. All four are matched, and
the `exists` narrowing was the right call. A typed-error refactor is still not
worth it now that the branch is done. But add a `// keep in sync with
cmd/split.rs` comment at each `bail!`/`Display` site — it costs three lines and
is the only thing standing between a reword and a silently wrong exit code. The
real defect in this area is I7, not the string matching.

**M9. `src` sanitization is lossy for spine paths containing `--`.** `chapter.rs:45`
runs `sanitize_attr` on `src`, so `OEBPS/part--one.xhtml` round-trips as
`part–one.xhtml`. That value then lands in each chapter file's `source:`
frontmatter (`splitter.rs:135-138`) claiming provenance it no longer has, and
feeds `{src}` template expansion. Sanitizing is correct (the marker must stay a
valid HTML comment); the fix is to acknowledge it — percent-encode `--` in `src`
specifically, or amend the spec's line 103 (see I2) to say `src` is sanitized and
therefore not a reliable round-trip key.

**M10. `nav_label == Some("")` defeats the title fallback.** `epub.rs:151-154`
uses `nav_label.map(str::to_owned).or_else(|| first_heading_text(...))`. `or_else`
only fires on `None`, so an empty or whitespace-only nav label yields
`title=""` → `slug=""` → a file named `01-.md` under the default template. Add
`.filter(|s| !s.trim().is_empty())` before the `or_else`.

**M11. `first_heading_text` matches inside fenced code blocks.** `chapter.rs:59-64`
takes the first line whose trimmed start is `#`. A chapter opening with a shell
listing gets `# install deps` as its title. Low frequency in book epubs; note it.

**M12. Inconsistent null convention across commands.** `detect` reports a missing
input's `path` as `""` (`cmd/detect.rs:63`); `extract` reports a missing output as
`null` (`Option<String>`, `cmd/extract.rs:69`). Same concept, two encodings, in
one release. Make `DetectRow.path` an `Option<String>`.

**M13. Row ordering differs.** `extract` emits missing-input rows *before*
resolved ones (`cmd/extract.rs:60`); `detect` emits them *after*
(`cmd/detect.rs:60`). Neither is documented as ordered, but a consumer diffing
the two commands' output will notice.

**M14. Templates expand raw values into path components.** `template.rs:70`
expands `{title}` (and `{book}`) verbatim. A chapter titled `Before/After` under
`--name '{title}.md'` produces `dir.join("Before/After.md")` and fails as an
`io_error` rather than a clean filename or a clear diagnostic. `sanitize_attr`
does not strip `/`. `{slug}` is safe (`heading_slug` drops it); `{title}` and
`{src}` are not. Either slugify path separators at expansion or document that
`{title}` is unsafe for untrusted books.

**M15. `splitter::split` has no rollback.** Files are written as the loop runs
(`splitter.rs:144`); a mid-run `output_exists` or `io_error` leaves a partially
written tree and exits nonzero. Low impact in practice because the frontmatter
file is written first and is the usual collision, so a plain re-run bails before
touching anything. Reachable with `--no-front`.

**M16. `just clispec` writes ~30 files into `ref/epub-md/`.** The declared `split`
example (`main.rs:169-172`) is
`["split", "tests/golden/input/books/frankenstein-pg.epub"]` with no `--outdir`,
and per ledger:153 the scorer *executes* declared examples. It therefore writes a
full chapter tree into the default `output_dir`. `ref/` is not in the repo
`.gitignore` — it is ignored only by Clay's personal `~/.gitignore:38`, so this is
clean on his machine and dirty on CI or any other checkout. Add
`"--outdir", "target/clispec-scratch"` to the example, or add `ref/` to
`.gitignore`. The example stays runnable either way.

**M17. Ledger:132's follow-through never happened.** That ruling ends "Document it
in the README" — meaning that `fte extract -o text` creates a *directory* named
`text` rather than selecting a format. README has no mention of `-o` at all
(`grep -n '\-o ' README.md` → nothing). Either add the note to README's exit-code
/ usage section or strike the commitment from the ledger.

**M18. `Kind::UnsupportedFormat` (exit 4) is published but unreachable.**
`grep -rn 'Kind::UnsupportedFormat' src/ --include='*.rs'` outside `errors.rs`
returns zero hits. `fte schema` and README:91 both advertise exit 4; no code path
produces it (format-detection failures surface as `ExtractionFailed` or as a
`status: "failed"` row). A declared error kind that can never occur is a schema
that lies. Either wire it up in `extract::extract`'s no-handler path or drop it
from `Kind::ALL` and README.

**M19. `one_shot_epub_split_matches_the_two_step_result` is narrower than its
name.** `tests/split.rs:154-158` compares the skeleton of exactly one file,
`00-frontmatter.md`. Task 7's review (ledger:111) correctly noted that
`common::skeleton` embeds a full sha256 — so that *one file* is compared byte for
byte, which is genuinely strong. But ~30 chapter files are not compared at all,
and the spec (line 416-417) asked for the whole tree. Widen it to walk the
directory and compare skeletons pairwise; it is a ten-line loop.

**M20. Public API notes.** `epub::heading_slug` was made `pub` (`epub.rs:377`) —
it is a generic GFM slug helper living in the ePub module and consumed by
`splitter.rs`; it belongs in `markdown.rs`. Everything else newly public is
documented and appropriate. `errors::Kind` is `#[non_exhaustive]` but
`Kind::ALL: [Self; 8]` pins the count in its public type, so adding a variant is
breaking regardless of the attribute — make it a `&'static [Self]` or drop the
attribute, since per memory breaking changes here are free anyway.

---

## Triage of the ledger's `minor (deferred)` items

| Ledger | Item | Verdict |
|---|---|---|
| :71 | frankenstein ch01 title has a trailing `;` | **Deferrable.** Cosmetic; `heading_slug` strips it so filenames are unaffected. |
| :78 | rustfmt reflow in `tests/book_golden.rs` | **Non-issue.** `fmt --check` passes. |
| :89 | brief header said `common: &CommonArgs` | **Non-issue.** Process artifact, not code. |
| :94 | raw `"` in a hand-edited marker truncates | **Fold into C1.** Same root cause: `parse` validates nothing. |
| :124 | no shared constant ties bail text to classification | **Deferrable** (see M8), but add the sync comments now. |
| :138 | JSON consumer cannot tell which inputs failed | **Closed** by Task 10's `resolve`-returns-missing fix. Verified: `detect --format json bmc-short bad-id` yields two rows, one `status: "failed"`. |
| :142 | `tests/split.rs:119` asserts stderr contains `exists` | **Deferrable.** The exit-code assertion (7 vs 6) in `tests/cli.rs` is the real guard. |
| :159 | schema examples use repo-relative fixture paths | **Deferrable with a doc requirement.** The measurement is sound and there is no self-contained non-empty invocation. But nothing in spec or README says the published examples are compliance probes rather than user docs — add one sentence. Also see M16: the `split` example has a side effect the other two do not. |

## Stress-test of the remaining rulings

- **:85** (missing *default* `input_dir` → exit 0 + warning; explicit `--indir` →
  exit 3) — **stands.** Verified both paths.
- **:103** (fix `disambiguate` rather than defer) — **stands, and was right.** The
  widening loop terminates by pigeonhole and the test genuinely fails against the
  old code.
- **:118** (substring mapping over typed errors) — **stands**, see M8.
- **:132** (`-o` stays bound to `--outdir`) — **stands**, but its documentation
  commitment was not honored (M17).
- **:137** (fix split's double print rather than defer) — **stands, and was
  right** — but it was applied to only one of the two commands with the problem
  (I3).
- **:149** (suppress ALL chatter on stderr in JSON mode) — **correct ruling,
  incompletely implemented** (I5).
- **:159** (keep fixture paths in examples) — **stands**, see triage above.
- **:160** (*Structured errors* parked as fte-design-blocked, not librebar) —
  **stands.** The outcome-vs-error distinction is CLIspec's own, the scorer's
  check does not honor it, and forcing it would either duplicate data or undo
  Task 8. Correct call. Its supporting claim ("failures are fully reported as
  rows") is currently false and I4 is what makes it true.

---

## Merge verdict

**Not ready.** The branch is in good shape — the architecture is sound, the
error/outcome split is right, and the seams are mostly clean — but four things
must land first.

**Must fix before merge**

1. **C1** — unterminated chapter causes silent data loss with exit 0.
2. **I2** — amend the spec: score 20→18, the blocked list (4→5+1), `src`
   "verbatim", `detect`'s `status` field, the unbuilt `output_kind: opaque`
   declaration, and the unbuilt split golden. In this repo a stale spec is a
   defect, and five of these six are ones the ledger never recorded.
3. **I3 + I5** — output discipline: `extract` double-prints failures in text
   mode, and `-v` leaks plain text onto stderr in JSON mode against an explicit
   ruling.
4. **I6** — exempt `DisplayHelpOnMissingArgumentOrSubcommand` so bare `fte`
   stops stuffing the help screen into a JSON field, and pin its test with
   `--format text` so it stops passing for the wrong reason.

**Strongly recommended in the same pass** (cheap, and each closes a gap the
ledger currently claims is closed): **I4** (`reason` on failed rows), **I7**
(error chain in `split`'s messages), **M16** (`--outdir` on the split example),
**M18** (unreachable `unsupported_format`).

Everything else in MINOR is genuinely deferrable.
