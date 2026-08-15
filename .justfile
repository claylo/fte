set shell := ["bash", "-c"]
set dotenv-load := true
toolchain := `taplo get -f rust-toolchain.toml toolchain.channel | tr -d '"'`
msrv := "1.89.0"

default:
  @just --list

fmt:
  cargo fmt --all -- --config-path .config/rustfmt.toml

clippy:
  cargo +{{toolchain}} clippy --all-targets --all-features --message-format=short -- -D warnings

fix:
  echo "Using toolchain {{toolchain}}"
  cargo +{{toolchain}} clippy --fix --allow-dirty --allow-staged -- -W clippy::all

# Check dependencies for security advisories and license compliance.
# `--all-features` walks the full dep tree so optional features 
# are covered — matches the CI invocation.
#
# NOTE: cargo-deny 0.20 moved graph-shaping flags (`--config`, `--all-features`,
# `--workspace`) to global options, ahead of the subcommand. Only report-shaping
# flags (`--deny`, `--warn`, `--allow`) remain on `check`.
deny:
  cargo deny --all-features --config .config/deny.toml check

test:
  cargo nextest run --all-features

test-ci:
  cargo nextest run --all-features --profile ci

# `cargo test --doc` hard-errors on a crate with no library target, which is
# how a binary-only crate like fte fails `check` for the wrong reason. Probe for
# a lib target first so the recipe skips instead, and still runs — and still
# fails loudly — the moment one is added.
doc-test:
  @if cargo metadata --no-deps --format-version 1 | grep -q '"kind":\[[^]]*"lib"'; then \
    cargo test --doc --all-features; \
  else \
    echo "doc-test: no library target in this crate, skipping"; \
  fi

cov:
  @cargo llvm-cov clean
  cargo llvm-cov nextest --no-report
  @cargo llvm-cov report --html
  @cargo llvm-cov report --summary-only --json --output-path target/llvm-cov/summary.json

check: fmt clippy deny test doc-test

# Check for outdated dependencies (root only, no transitive noise)
outdated:
    cargo outdated --root-deps-only

# Safe update: respects semver constraints, only touches Cargo.lock
#
# NOTE: no `--workspace` here. In `cargo update`, `--workspace` is shorthand
# for `-p <each workspace member>` — it re-resolves only the workspace's own
# packages, which is a no-op for a single-crate workspace. Bare `cargo update`
# is what actually walks the dependency tree.
update:
    cargo update --verbose

# Upgrade Cargo.toml to latest compatible versions
upgrade:
    cargo upgrade
    cargo update

# The nuclear option: upgrade to latest incompatible versions (breaking changes)
upgrade-breaking:
    cargo upgrade --incompatible
    cargo update

# See what WOULD update without doing it
check-updates:
    cargo update --dry-run

# Score the built binary against The CLI Spec.
#
# Five checks are blocked by librebar 0.6, which emits CLIspec 0.2, has no
# `effects` or `cardinality` fields, and unconditionally overwrites any
# CommandMetadata this crate declares for the reserved `schema`/`completions`
# commands (cli/parse.rs:116-128) — see
# record/superpowers/specs/2026-08-14-cli-restructure-and-chapter-splitting-design.md
#
# librebar-blocked: Validates against clispec v0.3, Effects on all commands,
# Effects declarations, Cardinality declarations, Output fields declared.
#
# A sixth, *Structured errors*, is blocked by our own design, not librebar:
# `partial_failure` (exit 1) is documented as an outcome, not a fault
# (errors::PARTIAL_FAILURE), so it reports through the `items` envelope on
# stdout and puts nothing on stderr. clispec's error-envelope check wants a
# JSON line on stderr for any nonzero exit, which this deliberately doesn't
# do. Forcing one would mean duplicating the failure or reclassifying the
# outcome as an error — parked rather than distorted.
clispec-floor := "18"

clispec:
  @cargo build --quiet
  @clispec score -o json ./target/debug/fte > target/clispec.json
  @jq -e '.score >= {{clispec-floor}}' target/clispec.json > /dev/null || { \
      echo "clispec score below floor of {{clispec-floor}}:" >&2; \
      jq -r '.principles[].checks[] | select(.passed == false) | "  FAIL \(.name): \(.detail // "no detail")"' target/clispec.json >&2; \
      exit 1; \
  }
  @jq -r '"clispec \(.score)/\(.max) (\(.percentage)%) \(.grade)"' target/clispec.json

