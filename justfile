# ------------------------------------------------------------------------------
# just — development recipes mirroring what CI runs.
#
# Install `just` with `cargo install just`; `just` alone lists the recipes.
# Recipes shell out through `bash -euo pipefail`, so any failing step aborts.
# ------------------------------------------------------------------------------

set shell := ["bash", "-euo", "pipefail", "-c"]

# List the available recipes.
default:
    @just --list

# Install the git hooks (lefthook.yml) and the docs toolchain; run once after cloning.
setup:
    lefthook install
    uv sync --group docs

# ------------------------------------------------------------------------------
# Formatting & linting — the cheap checks to run before every commit.
# ------------------------------------------------------------------------------

# Format Rust and TOML sources in place.
fmt:
    cargo fmt --all
    taplo fmt

# Verify formatting without writing anything.
fmt-check:
    cargo fmt --all --check
    taplo fmt --check

# Lint with clippy, treating warnings as errors.
clippy:
    cargo clippy --workspace --all-targets --all-features -- -D warnings

# ------------------------------------------------------------------------------
# Tests — unit, integration and documentation tests, plus the docs build.
# ------------------------------------------------------------------------------

# Run the whole test suite with nextest, falling back to cargo test.
test:
    cargo nextest run --workspace --all-features || cargo test --workspace --all-features

# Run the doctests — nextest does not run them.
doctest:
    cargo test --workspace --all-features --doc

# Build the API docs exactly as docs.rs would.
doc:
    RUSTDOCFLAGS="--cfg docsrs -D warnings" cargo +nightly doc --workspace --all-features --no-deps

# ------------------------------------------------------------------------------
# Supply chain & compatibility — dependency policy, feature combinations, MSRV.
# ------------------------------------------------------------------------------

# Check advisories, licences, bans and sources with cargo-deny.
deny:
    cargo deny check

# Check that every feature combination compiles.
hack:
    cargo hack check --workspace --feature-powerset --depth 2 --no-dev-deps

# Verify the crates build on the MSRV declared in Cargo.toml.
msrv:
    cargo +$(awk -F'"' '/^rust-version/ {print $2; exit}' Cargo.toml) check --workspace --all-features

# ------------------------------------------------------------------------------
# Documentation site — Zensical (Material theme), driven through uv.
# ------------------------------------------------------------------------------

# Serve the site with live reload on http://127.0.0.1:8000.
docs-serve:
    uv run --group docs zensical serve

# Build the site in strict mode from a clean cache: a broken link or a
# missing snippet fails.
docs-build:
    uv run --group docs zensical build --strict --clean

# ------------------------------------------------------------------------------
# Everything CI runs — the aggregate gate, plus the example.
# ------------------------------------------------------------------------------

# Run every check CI runs, locally.
ci: fmt-check clippy test doctest deny examples docs-build

# Run the quickstart and the relations examples.
example:
    cargo run -p turso-orm --example quickstart
    cargo run -p turso-orm --example relations

# Format, lint and test each example workspace, and run the console one.
# Each example is its own workspace, so the root recipes do not reach it.
examples:
    for manifest in examples/*/Cargo.toml; do \
        cargo fmt --all --manifest-path "$manifest" -- --check; \
        cargo clippy --workspace --all-targets --manifest-path "$manifest" -- -D warnings; \
        cargo test --workspace --manifest-path "$manifest"; \
    done
    cargo run --manifest-path examples/basic/Cargo.toml
