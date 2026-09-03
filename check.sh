#!/usr/bin/env bash
# Verification gate: must pass before every commit (see CLAUDE.md).
# Ordered cheapest-and-likeliest-to-fail first, so a failure costs seconds.
set -euo pipefail
cd "$(dirname "$0")"

cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace --all-targets
# The look feature is off by default; build it so the tool always compiles.
cargo build -p probe-game --features look --all-targets
cargo test --workspace
# The playable's own headless drive, which needs a GPU and an X display.
xvfb-run -a cargo test -p probe-game --features look --bin probe-game
# The game ships in the browser, and the sim with it.
cargo build -p probe-game --target wasm32-unknown-unknown
