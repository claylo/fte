# Design: CLI restructure, chapter markers, and CLIspec 0.3

**Date:** 2026-08-14
**Status:** Approved for planning
**Branch:** main

## Context

`fte` currently exposes one implicit command: a bare invocation with a variadic
positional that extracts every file in the configured `input_dir`. librebar
injects `schema` and `completions` as real clap subcommands alongside that
positional, so `fte schema` works by luck — a file named `schema` in the input
directory would be shadowed.

Four refinements land together because they depend on each other:

1. Chapter boundary markers in book-length markdown output.
2. Extraction moved under `fte extract`; bare `fte` shows help.
3. A new `fte split` that turns marked-up book markdown into per-chapter files.
4. Movement toward CLIspec 0.3 (release candidate).

Items 2 and 4 are the same work seen from two angles. `clispec score` reports
**11/24 (45%, "Needs Work")** against the current binary, and five failing
checks give the identical reason: `no subcommand to test`. The scorer probes
real subcommands to verify JSON output, format precedence, and stream
separation. A bare-positional CLI is unscoreable.

## Goals

- Bare `fte` shows help and exits 2.
- Book markdown carries machine-readable chapter boundaries.
- `fte split` produces per-chapter files with user-configurable naming.
- `clispec score` reaches 18/24, with the remaining 6 parked: 5 blocked on
  librebar 0.6 emitter gaps, 1 (*Structured errors*) blocked on this repo's
  own outcome-vs-error design. Both are documented in `.justfile`.
- The book extraction path gains golden coverage it has never had.

## Non-goals

- Changing librebar. The emitter gaps are real and are written up below as a
  librebar 0.7 follow-up for a separate session.
- Changing paper (non-book) extraction output. Every existing golden file stays
  byte-identical.
- Backwards compatibility for the bare-positional form. The repo has no git
  tags; nothing has shipped.

## 1. Command surface

```
fte                        help on stderr, exit 2 (usage)
fte extract [INPUTS...]    today's extraction, semantics unchanged
fte detect  [INPUTS...]    promoted from --detect-only
fte split   <FILE>         new; accepts .md or .epub
fte schema                 librebar-injected, unchanged
fte completions <SHELL>    librebar-injected, unchanged
```

`fte extract` with no positionals still walks the configured `input_dir`. That
behavior moves under a subcommand; it does not go away.

`--detect-only` becomes `fte detect`. Under CLIspec 0.3 every command declares
`effects` and `output_kind`. Detection is `read_only` and returns format names;
extraction writes files. A flag that silently reclassifies a command's effects
is precisely what 0.3 exists to prevent, so the two separate.

### Flag placement

Per-subcommand, not global — global args inflate `global_args` in the schema
and imply a generality that does not exist.

| Command | Flags |
|---------|-------|
| `extract` | `--indir`, `-o/--outdir`, `--stdout`, `--force` |
| `detect` | `--indir` |
| `split` | `-o/--outdir`, `--subdir`, `--name`, `--front`, `--no-front`, `--force` |

librebar's `CommonArgs` (`-C`, `-c`, `-q`, `-v`, `--color`, `--format`,
`--version-only`) stay global, as today.

`-o` remains bound to `--outdir`. CLIspec permits `--format` as the output
selector when `-o` is taken, which is what librebar already provides.

## 2. Chapter markers

Emitted by the book path only (`extract_book` in `src/epub.rs`).

```markdown
<!-- fte:chapter-start id="ch03" title="Walking in the Dark" src="OEBPS/ch03.xhtml" -->

## Walking in the Dark

Body text…

<!-- fte:chapter-end id="ch03" -->
```

### Attributes

| Attribute | Source |
|-----------|--------|
| `id` | Sequential: `ch01`, `ch02`, … |
| `title` | Nav label, or the first heading found in the chapter body |
| `src` | Spine entry path, sanitized (see Sanitization below) |

`id` is sequential rather than derived from the source stem. Book epubs
routinely use obfuscated filenames (`sgPhzGILRlKrLKg2DMvpew1`, `c0`, `cP`) —
`src` preserves the real path for provenance while `id` stays legible and
stable.

`src` goes through the same sanitization as `title` (below) because it sits
inside the same HTML comment attribute — an unsanitized `--` or `"` in a
spine path is exactly as much of a corruption hazard as one in a title. This
means `src` is provenance, not a guaranteed-exact round-trip key: a spine
path containing `--` (e.g. `OEBPS/part--one.xhtml`) round-trips with an en
dash substituted. Use `id` when exact identity matters.

### Sanitization

`--` cannot appear inside an HTML comment. Attribute values are sanitized
before emission:

- `--` collapses to an en dash (`–`)
- `>` is stripped
- Newlines collapse to a single space

Em dashes rendered as `--` are common in converter output, so this is a live
hazard rather than a theoretical one. An unsanitized title would produce a
comment some parsers treat as unterminated, silently swallowing the chapter.

### Placement

The `# Title` heading and `## Contents` block are emitted before the first
marker and belong to no chapter. `fte split` routes them to the frontmatter
file. Content between a `chapter-end` and the next `chapter-start` is discarded
by `split` — paired markers exist so converter cruft cannot silently attach
itself to the preceding chapter.

Markers are always on for books. They are invisible in rendered markdown, and
`fte split` depends on them.

## 3. `fte split`

### Input

- `.md` — a document produced by `fte extract` containing chapter markers.
- `.epub` — run the extraction pipeline in memory, then split. No combined
  `.md` is written; use `fte extract` for that.

A document with no chapter markers is an error (`no_chapters`, exit 8), not a
silent single-file copy. An academic-paper epub passed to `split` therefore
fails with a clear reason.

### Naming templates

Configuration, not hardcoded styles:

```yaml
split:
  subdir: "{book}"              # "" writes flat into outdir
  file:   "{n}-{slug}.md"
  front:  "{n}-frontmatter.md"  # "" suppresses the frontmatter file
  pad:    2
```

| Token | Expands to |
|-------|-----------|
| `{book}` | Source file stem (`.md` or `.epub`) |
| `{n}` | Index, zero-padded to `pad`. Frontmatter is 0; chapters start at 1 |
| `{slug}` | GFM slug of the chapter title (reuses `heading_slug`) |
| `{title}` | Raw chapter title |
| `{src}` | Source spine-entry stem, from the marker's `src` attribute |

An unrecognized `{token}` is a `config_error`, not a literal passthrough.

The default produces:

```
oconnor-2022/
  00-frontmatter.md
  01-dedication.md
  02-walking-in-the-dark.md
```

An alternate convention is configuration alone, with no dedicated code path:

```yaml
split:
  subdir: ""
  file:   "{book}-ch{n}.md"
  front:  "{book}-ch{n}-frontmatter.md"
```

```
oconnor-2022-ch00-frontmatter.md
oconnor-2022-ch01.md
oconnor-2022-ch02.md
```

CLI flags map 1:1 onto the config keys: `--subdir`, `--name`, `--front` /
`--no-front`.

### Frontmatter propagation

Each chapter file inherits the book's YAML frontmatter and adds:

```yaml
chapter: 3
chapter_title: "Walking in the Dark"
source: "OEBPS/ch03.xhtml"
```

The frontmatter file receives the book frontmatter unchanged, plus the `# Title`
heading and `## Contents` block.

### Collisions

Two chapters sharing a title produce the same `{slug}`. `split` detects the
collision and appends the chapter index (`walking-in-the-dark-2`) rather than
overwriting. Existing files are refused without `--force`, consistent with
`extract`.

## 4. CLIspec 0.3

### Error kinds

Declared through librebar's existing `ErrorMetadata`. When `--format` resolves
to json, failures print a single JSON line to stderr as the last line:

```json
{"kind":"not_found","message":"input 'foo.html' not found","hint":"check --indir"}
```

| Kind | Exit | Retryable | Raised when |
|------|------|-----------|-------------|
| `usage` | 2 | false | Bad arguments, unknown subcommand, bare `fte` |
| `not_found` | 3 | false | Input file or ID does not resolve; `input_dir` missing |
| `unsupported_format` | 4 | false | No handler matches the detected format |
| `extraction_failed` | 5 | false | Parsing produced no usable body content |
| `output_exists` | 6 | false | Target exists and `--force` was not given |
| `io_error` | 7 | true | Read or write failure |
| `no_chapters` | 8 | false | `split` on a document with no chapter markers |
| `config_error` | 9 | false | Malformed config or unknown template token |

### Exit 1 is an outcome

A run where 3 of 10 inputs fail currently exits 1, indistinguishable from a
hard failure. CLIspec 0.3 separates errors from outcomes, and librebar already
supports `OutcomeMetadata`:

```
outcomes: [{ code: 1, name: "partial_failure" }]
```

Exit 1 keeps its current meaning and gains a name. Whole-run failures use the
error kinds above.

### Structured output

Each command emits a `{"items": [...]}` envelope. 0.3 requires the wrapper over
a bare array so a consumer can add fields without a breaking change.

| Command | `output_kind` | `cardinality` | `effects` | Fields |
|---------|---------------|---------------|-----------|--------|
| `extract` | data | bounded | idempotent | `id`, `source_format`, `output_path`, `bytes`, `status`, `reason` |
| `detect` | data | bounded | read_only | `id`, `path`, `format`, `status`, `reason` |
| `split` | data | bounded | idempotent | `index`, `title`, `path`, `bytes` |

This table describes the target 0.3 shape; only the `Fields` column is
actually shipped. `output_kind`, `cardinality`, and `effects` are not
declared anywhere — `SchemaMetadata` in librebar 0.6 has no fields for them
(see the librebar 0.7 follow-up below), and `fte schema` still reports
`"clispec": "0.2"`. `status` and `reason` were not in the original design;
both were added during the final review pass (I4) so a JSON consumer can
tell not just *that* a row failed but *why*.

Cardinality is `bounded` because the item count is driven by caller input —
explicit positionals or the contents of `input_dir`. No pagination is required.

`extract` is `idempotent`: rerunning skips existing outputs and converges. Its
items carry a `status` field (`extracted` / `skipped` / `failed`), which
satisfies 0.3's expectation that idempotent commands report whether anything
changed.

### `--stdout` is opaque

`fte extract --stdout paper.html` emits raw markdown regardless of `--format`.

This is deliberate. `--format auto` resolves to json when stdout is not a TTY,
so `fte extract --stdout paper.html > out.md` would otherwise JSON-wrap the
markdown — a trap that only becomes reachable once JSON output exists at all.

The *behavior* is shipped and golden-tested
(`tests/cli.rs::stdout_stays_raw_markdown_even_when_piped`). The
`output_kind: opaque` / `media_type: text/markdown` *declaration* was not:
librebar 0.6's `SchemaMetadata` has no field for either, the same gap that
blocks `cardinality` and `effects` below. Until librebar carries them, this
is a documented behavioral contract, not a machine-readable one — a
JSON-consuming caller has to know `--stdout` is special from this doc, not
from `fte schema`.

`split` has no `--stdout`; it writes N files.

### Text mode

In text mode every command prints its result rows to stdout and nothing else.
Progress lines (`OK  foo.md (12KB)`), warnings, and the `Done:` summary go to
stderr, as they do today. This is already correct in `main.rs`; the CLIspec
check *Messages on stderr only* currently fails only because the scorer has no
subcommand to probe.

### Score

18/24 (75%), up from 11/24 (45%). Six checks are parked, not failing for lack
of effort: five are blocked by librebar 0.6 emitter gaps (below), and a
sixth, *Structured errors*, is blocked by this repo's own outcome-vs-error
design — `partial_failure` (exit 1) is documented as an outcome, not a
fault, so it reports through the `items` envelope on stdout and puts nothing
on stderr, which the scorer's error-envelope check doesn't have a category
for. `.justfile`'s `clispec` recipe carries the current, authoritative list
of both; this section summarizes it.

This section originally projected 20/24 before implementation. The measured
score came in lower because a sixth check (*Structured errors*) turned out to
be blocked too, on a design decision rather than a librebar gap — see above.

## librebar 0.7 follow-up

Five checks cannot pass from this repo. Verified against the published
`v0.3.json`:

- `properties.clispec` is `{"const": "0.3"}`. librebar hardcodes `"0.2"`
  (`src/cli/schema.rs:11`), so schema validation fails on the first field.
- `$defs.command.required` is `["name", "description", "effects"]`. librebar's
  `CommandSchema` has no `effects` field; it emits the 0.2 `mutating` boolean
  that 0.3 deprecates.
- `cardinality`, `output_kind`, `media_type`, `stream_format`, and
  `stdout_schema` have no representation in `SchemaMetadata`.

Blocked checks: *Validates against clispec v0.3*, *Effects on all commands*,
*Effects declarations*, *Cardinality declarations*, *Output fields declared*.

A sixth check, *Structured errors*, is blocked separately — not by librebar,
but by this repo's own outcome-vs-error design (see Score, above). Forcing it
to pass would mean either duplicating a partial-failure's rows as a stderr
error line or reclassifying the outcome as an error, undoing the exit-1
design this section argues for. It stays parked on purpose.

### The version must be selectable, not switched

The obvious fix — retarget `CLI_SPEC_VERSION` to `"0.3"` — is wrong, and this
is the load-bearing constraint on that work.

Moving 0.2 → 0.3 is not an emitter-only change. `$defs.command` requires
`effects` on every command, and only the application knows whether a command is
`read_only`, `idempotent`, or `non_idempotent`. `cardinality` is the same. So a
librebar release that flips the constant would make every consumer emit a
document *claiming* 0.3 conformance while failing 0.3 validation — strictly
worse than the valid 0.2 document it emits today, and silent. `cargo update`
would break `schema` output in repos nobody touched.

librebar 0.7 should therefore emit both versions and let the application pick:

- `SchemaMetadata::spec_version(SpecVersion::V0_3)`, defaulting to `V0_2`.
- Requesting `V0_3` without `effects` on every command is a `SchemaError`, not a
  silently invalid document. librebar already validates metadata against the
  command tree in `validate_metadata`, so this is the existing mechanism.
- `V0_3` emission drops the deprecated `mutating` boolean and gains `effects`,
  `cardinality`, `output_kind`, `media_type`, `stream_format`, and
  `stdout_schema`.

The selector is permanent infrastructure, not RC-window scaffolding. CLIspec
versions fast — v0.2 took five amendments in two months before freezing, and
0.3 arrived three months later — and every bump has the same shape: new
required fields that only the application can supply. There will be a 0.4.
`SpecVersion` should be `#[non_exhaustive]` and the emitter structured for N
versions, not as a two-way toggle.

What *is* disposable is the `V0_2` arm. **Once CLIspec 0.3 freezes,
re-assess:** if librebar still has no external consumers, flip the default to
`V0_3`, sweep the fleet repos to declare `effects` in the same pass, ship one
release, and drop `V0_2` when nothing selects it. The default tracks the newest
frozen version the fleet has migrated to.

The `SchemaError` above is what makes each such flip a one-pass job — a
consumer that has not declared the new version's required fields fails loudly
instead of quietly emitting a document that lies about its own version. That
property is worth more at 0.4 than it is at 0.3, because by then the fleet is
larger.

The zero-reverse-dependencies fact does not license skipping the selector
today. It means no *external* crate breaks — it says nothing about the fleet
repos that would start emitting invalid schemas on upgrade. It is the reason
the eventual flip is a one-session job rather than a deprecation cycle.

This belongs in a librebar session; recorded in the fleet hub, not here.

## Book golden tests

The book path currently has fixtures (`tests/golden/input/books/`) but no
expected output, so the exact path gaining chapter markers is unprotected. This
work wires it up — but not with the paper harness's full-text comparison.

### Why structural, not full text

`check_golden` renders a complete line diff into the panic message
(`tests/golden.rs:53-66`). At paper scale that is the right call. Frankenstein
extracts to roughly 400KB of markdown and A Tale of Two Cities to roughly
800KB; a one-line regression would print an entire novel as test output.

Repo weight compounds it. `tale-two-cities-pg.epub` is 7.9MB on its own, which
the previous handoff already flags. Another ~1.2MB of expected markdown buys
little, because the paper goldens already cover body-text rendering byte for
byte.

### Skeleton format

`tests/golden/expected/books/<id>.skeleton.txt`, one line per structural event:

```
sha256 f3a91c…
bytes  412883
---
frontmatter id=frankenstein-pg format=epub title="Frankenstein; Or, The Modern Prometheus" authors=1
h1 Frankenstein; Or, The Modern Prometheus
h2 Contents
toc 27
chapter-start ch01 title="Letter 1" src="OEBPS/ch01.xhtml"
h2 Letter 1
text 1204w
chapter-end ch01
chapter-start ch02 title="Letter 2" src="OEBPS/ch02.xhtml"
…
```

Prose collapses to a word count. Headings, markers, and frontmatter render in
full — those are the structure the book path is responsible for, and the part
chapter markers change.

The `sha256` line covers the complete output, so a body-text-only regression
still fails. It reports as "hash changed, structure identical", which localizes
the problem immediately rather than burying it in a diff.

`UPDATE_GOLDEN=1` regenerates skeletons, matching the existing harness
convention.

### Coverage

| Fixture | Exercises |
|---------|-----------|
| `frankenstein-pg.epub` | Gutenberg boilerplate, `div.chapter` splitting, 31 spine files |
| `tale-two-cities-pg.epub` | Illustrations, 47 spine files, long novel |

Both were used to build the book capability, per the previous handoff.

`fte split` reuses the same renderer, but its golden coverage is narrower
than originally planned here. `tests/split.rs::one_shot_epub_split_matches_the_two_step_result`
compares exactly one file — `00-frontmatter.md` — between the one-shot and
two-step runs. `common::skeleton` embeds a full sha256, so that one file is
compared byte for byte, but the ~30 chapter files are not compared at all,
and there is no file-tree golden. Widening it to walk the output directory
and compare skeletons pairwise is a real gap (final review M19) but was
judged deferrable rather than must-fix for this branch; it remains open
follow-up work.

## Testing

Unit:

- Marker rendering, including `--` and `>` sanitization in titles.
- Template expansion across every token, with padding widths 1–4.
- Unknown template token produces `config_error`.
- Slug collision disambiguation.

Integration:

- Book skeleton goldens for both Gutenberg fixtures (see above).
- Round-trip `extract` → `split` on `frankenstein-pg.epub` under both the
  default and the flat naming conventions.
- One-shot `split book.epub` matches the two-step result.
- `split` on a paper epub fails with `no_chapters` and exit 8.
- Bare `fte` exits 2 with help on stderr.
- One assertion per error kind confirming its exit code.
- Existing golden files pass unchanged, confirming the paper path is untouched.

Tooling:

- `just clispec` wraps `clispec score` and fails below a floor, so the score is
  a checked artifact rather than a number re-derived by hand.

## Landmines

- **Skeleton goldens trade diff precision for legibility.** A body-text change
  reports as a hash mismatch with no indication of what moved. That is the
  intended trade — the alternative prints a novel — but expect to re-extract by
  hand when a hash-only failure appears.
- **`split` on an epub duplicates extraction cost.** Running `extract` then
  `split` parses the epub twice. Acceptable — the one-shot exists for
  convenience, not throughput.
- **`{src}` is absent for injected chapters.** If a future code path emits a
  chapter with no backing spine file, `{src}` has nothing to expand to. Emit an
  empty string and let the template author notice.
- **The 20/24 estimate was a projection; it undercounted the parked checks.**
  It assumed only librebar-blocked checks would be parked. The measured score
  is 18/24 (75%) — five checks blocked by librebar 0.6, plus one
  (*Structured errors*) blocked by this repo's own outcome-vs-error design,
  which the original estimate didn't anticipate. See Score, above. `just
  clispec` was run early, per this note, which is exactly how the gap was
  caught before merge instead of after.
