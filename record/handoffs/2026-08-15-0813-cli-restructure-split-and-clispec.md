# Handoff: CLI restructure, chapter splitting, CLIspec 0.3

**Date:** 2026-08-15
**Branch:** `feat/cli-restructure-and-split` — 21 commits off `main` at `9cf523e`, not merged, not pushed
**State:** Green — 170/170 tests (was 100), clippy clean `-D warnings`, `just check` passes, `clispec score` 18/24

Most of the detail lives in `record/followups/` — this is the map.

## What shipped

| | |
|---|---|
| **Chapter markers** | Book ePub output wraps each chapter in paired `<!-- fte:chapter-start id … -->` / `chapter-end` comments. Attribute values sanitized: `--` → en dash, `>` stripped, `"` → `'`. Book path only; paper output byte-identical. |
| **Subcommands** | `fte extract` / `fte detect` / `fte split`. Bare `fte` prints help, exits 2. `--detect-only` is gone. |
| **`fte split`** | Takes `.md` or `.epub`. Filename templates in config (`split.subdir/file/front/pad`) with `{book}` `{n}` `{slug}` `{title}` `{src}`. Unknown token is a hard error. |
| **CLIspec 0.3** | Seven declared error kinds with distinct exit codes, `partial_failure` as an outcome at exit 1, `{"items":[…]}` envelopes, `just clispec` gate at floor 18. |
| **Book goldens** | `tests/golden/expected/books/*.skeleton.txt` — structural summary plus a sha256, because a full-text diff on a novel is unreadable. |

## Read these

- `record/followups/2026-08-15-elsevier-html-no-body-text.md` — **the live bug.** `scratch/leach-2018.html` extracts headings but no prose. Root cause is a dead `if` branch in `walk_element` (`src/html.rs:272-280`) that binds a text node, tests it, and does nothing. Latent for any publisher whose body text is not in `<p>`; Elsevier uses `<div>`. Recommended fix and its blast radius are in the doc.
- `record/followups/2026-08-15-deferred-from-cli-restructure.md` — non-blocking items triaged by the final review.
- `record/followups/2026-08-15-cli-restructure-final-review.md` — the whole-branch review: 1 Critical, 6 Important, all fixed.
- `record/followups/2026-08-15-cli-restructure-execution-ledger.md` — every ruling made during the run, including three I reversed after measuring.

## Uncommitted

Four files staged in `record/followups/`, `commit.txt` written for them. They want to land before any merge — they are the only surviving copy of the scratch workspace.

## Landmines

- **`clispec score` is 18/24 and that is the ceiling.** Six checks parked, documented in `.justfile`: five blocked on librebar 0.6 emitter gaps, one (*Structured errors*) on this repo's own outcome-vs-error design — the scorer wants a JSON line on stderr for any non-zero exit, which does not honor CLIspec's own distinction. Do not chase it; forcing it would undo the design. librebar work is tracked in the fleet hub as `librebar-clispec-0-3` and `librebar-cli-adoption`.
- **The schema's `example` invocations point at repo fixtures** (`tests/golden/input/bmc-short.html`). Deliberate: `clispec` executes them, and no self-contained invocation produces non-empty output. Measured — bare `["detect"]` scores 17, not 18. They are compliance probes, not user docs.
- **`cmd/split.rs` classifies errors by matching substrings of `anyhow` messages built in `src/splitter.rs`.** Only sync comments tie the two sites together. Reword one and exit codes silently misroute. The typed-error refactor was ruled out as too costly mid-branch; revisit if the mapping grows.
- **`-o` is `--outdir`, not `--output`.** `fte extract -o text` creates a directory named `text`. Coherent for the flag, surprising with CLIspec muscle memory. Changing it needs librebar to own `-o` as the format selector.
- **Book fixtures are ~8MB.** Unchanged from the last handoff, now with skeleton goldens guarding them.
