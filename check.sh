#!/usr/bin/env bash
# Verification gate: must pass before every commit (see CLAUDE.md).
# Ordered cheapest-and-likeliest-to-fail first, so a failure costs seconds.
set -euo pipefail
cd "$(dirname "$0")"

for crate in neumannarch-sim neumannarch-protocol neumannarch-agents neumannarch-game neumannarch-server; do cargo fmt --check -p "$crate"; done
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p neumannarch-game --features look --all-targets -- -D warnings
cargo build --workspace --all-targets
# The look feature is off by default; build it so the tool always compiles.
cargo build -p neumannarch-game --features look --all-targets
# Hosting is on by default and its dependencies are native-only, so the
# browser build below is what proves the game builds without the server.
cargo test --workspace
# The bot's guarantees play a six-minute match, seven seconds in release.
cargo run --release -p neumannarch-agents --bin harness -- verify 6
# The playable's own headless drive, which needs a GPU and an X display.
xvfb-run -a cargo test -p neumannarch-game --features look --bin neumannarch-game
# The game ships in the browser, and the sim with it.
cargo build -p neumannarch-game --target wasm32-unknown-unknown
