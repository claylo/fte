# Deferred items from the CLI restructure branch

Carried out of `feat/cli-restructure-and-split` (merged 2026-08-15). None
blocking; all triaged by the final whole-branch review.

## Behavioral

1. **Marker quote validation is odd-count only.** `chapter::parse`'s
   `well_formed` check catches an odd number of stray `"` in a hand-edited
   marker. An even number still truncates a title silently. No content loss —
   the chapter body is unaffected.
2. **`--quiet` does not suppress the missing-input-directory warning.**
   `inputs::resolve` emits it in every command, and `cmd::detect::run` never
   receives `quiet` at all.
3. **`fte split`'s golden covers one file, not the whole output tree.** The
   spec now records this as open rather than promising it as done.

## Test hygiene

4. **`bare_invocation_shows_help_and_exits_two` is honest but
   non-discriminating** — it would have passed pre-fix too. Add
   `assert!(!stderr.starts_with("error:"))` to make it bite.

## Design tension

5. **`errors::Kind` is `#[non_exhaustive]` while `Kind::ALL` is `[Self; 7]`.**
   The array has to change whenever a variant is added, which is most of what
   `#[non_exhaustive]` exists to avoid. Survived the M18 removal unchanged.
6. **`cmd/split.rs` classifies errors by matching substrings of `anyhow`
   messages built in `src/splitter.rs`**, with only sync comments tying the two
   sites together. A reword in one file silently misroutes exit codes. The
   typed-error refactor was ruled out mid-branch as too costly; revisit if the
   mapping grows.

## Not a defect

7. `record/superpowers/plans/2026-08-14-*.md` still references
   `UnsupportedFormat`. That is correct — the plan is a point-in-time artifact.
   Noted only so nobody re-greps and panics.

## Known CLIspec gaps

`clispec score` is 18/24. Six checks are parked, documented in `.justfile`:
five blocked on librebar 0.6 emitter gaps (tracked in the fleet hub as
`librebar-clispec-0-3` and `librebar-cli-adoption`), and one — *Structured
errors* — blocked on this repo's own outcome-vs-error design, which is
deliberate and correct.
