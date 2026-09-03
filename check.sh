#!/usr/bin/env bash
# Verification gate: must pass before every commit (see CLAUDE.md).
# Ordered cheapest-and-likeliest-to-fail first, so a failure costs seconds.
set -euo pipefail
cd "$(dirname "$0")"

for crate in probe-sim probe-protocol probe-agents probe-game probe-server; do cargo fmt --check -p "$crate"; done
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace --all-targets
# The look feature is off by default; build it so the tool always compiles.
cargo build -p probe-game --features look --all-targets
# Hosting is on by default and its dependencies are native-only, so the
# browser build below is what proves the game builds without the server.
cargo test --workspace
# The playable's own headless drive, which needs a GPU and an X display.
xvfb-run -a cargo test -p probe-game --features look --bin probe-game
# The game ships in the browser, and the sim with it.
cargo build -p probe-game --target wasm32-unknown-unknown
