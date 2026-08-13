# Handoff: dependency sweep and librebar 0.6 migration

**Date:** 2026-08-06
**Branch:** main (clean, `.vale.ini` untracked and deliberately left alone)
**State:** Green — `just check` passes end to end for the first time in this repo

> Green = `just fmt` + `just clippy` (`-D warnings`, toolchain 1.97.1) + `just deny` (advisories/bans/licenses/sources ok) + 23/23 nextest + `just doc-test`, all passing on `main`. There is no CI, so "green" means green *locally* — see Landmines.

## What's next

1. **Build a golden-file corpus for extraction fidelity.** This is the real gap. All 23 tests cover depth guards, archive size limits, and UTF-8 boundaries — **none assert that publisher HTML produces correct markdown.** html5ever moved 0.29 → 0.39 under a tool whose entire job is HTML→markdown, and the only functional verification was a trivial `<article>` document. A dozen real Springer/Wiley/OUP/Cambridge/PLOS files with committed expected output would retire this permanently and make the next parser bump a non-event.
2. **Decide what `-q/--quiet` and `-v/--verbose` should do.** They are in `--help` and do nothing. fte reports progress with bare `eprintln!` and builds without librebar's `logging` feature. Either wire `--quiet` to suppress the `OK`/`Done:` chatter, or take the flags off the struct. Leaving advertised flags inert is the worst of the three.
3. **There is no CI.** No `.github/workflows/` at all. Every gate in this handoff was run by hand on one machine. `.config/scrat.toml` exists with `no_publish = true`, so release tooling is configured but nothing enforces the gate on push.
4. **Consider whether `ref/` should be in the repo.** `input_dir` defaults to `ref/epub`, which does not exist — only the empty `ref/epub-md` does. A bare `fte` with no config or flags fails with `reading ref/epub: No such file or directory`. Either ship the directory, change the default, or give the error a better hint.
5. **Refresh `record/audits/2026-07-31-10-full-repo`.** It predates all four commits and the `.crustoleum/` scratch files (`clippy.txt`, `deny.txt`, `machete.txt`, …) are from that same run, so they describe the pre-sweep tree.

## Landmines

- **`just check` green is a *local* claim.** No CI exists. The gate depends on toolchain 1.97.1 from `rust-toolchain.toml` and on `cargo-deny`, `cargo-nextest`, and `taplo` being installed.
- **A missing `taplo` degrades `just clippy` confusingly rather than failing clearly.** `.justfile` line 3 derives `toolchain` from `` `taplo get -f rust-toolchain.toml toolchain.channel` ``. `just` evaluates that backtick lazily, so `just --list` and the recipes that ignore `{{toolchain}}` still work. But `clippy` and `fix` interpolate it, and with taplo absent it expands to the empty string — `cargo +{{toolchain}} clippy` becomes `cargo + clippy`, which dies with `invalid toolchain name ''` and a Rust backtrace. Verified by running with taplo off `PATH` and cargo still on it. The `taplo: command not found` line does appear, but above a backtrace that points nowhere near the cause.
- **MSRV is declared but never tested.** `rust-version = "1.89"` with `channel = "1.97.1"`; nothing checks that fte still compiles on 1.89. `detect.rs` has a comment noting `str::floor_char_boundary` was avoided because it stabilized in 1.91 — so the floor is load-bearing and unverified.
- **`cargo deny` passes but emits two duplicate-crate warnings** (`supports-color`, `syn`). Non-blocking today; they become blocking if `deny.toml` ever tightens `multiple-versions`. A third, `getrandom`, was resolved by the upgrades.
- **The `schema`/`completions` subcommands share a namespace with input IDs.** `fte schema` runs the schema command, not an extraction of `schema.html`. Harmless for DOI-shaped IDs, surprising if anyone ever passes a bare word.
- **`-c/--config` outranks `FTE_*` environment variables**, which is librebar's documented precedence and is now in the README, but inverts the intuition that env beats files. Verified empirically, not assumed.
