#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."

engine="../../mirage-engine"
out="dist"
port="${1:-8000}"
wasm="target/wasm32-unknown-unknown/release/neumannarch-game.wasm"

required=$(sed -n '/^name = "wasm-bindgen"$/{n;s/^version = "\(.*\)"$/\1/p;q;}' Cargo.lock)
if ! command -v wasm-bindgen >/dev/null; then
    echo "wasm-bindgen is missing: cargo install wasm-bindgen-cli --version ${required}" >&2
    exit 1
fi
installed=$(wasm-bindgen --version | awk '{ print $2 }')
if [ "${installed}" != "${required}" ]; then
    echo "wasm-bindgen ${installed} cannot process code built with wasm-bindgen ${required}:" >&2
    echo "  cargo install wasm-bindgen-cli --version ${required}" >&2
    exit 1
fi

cargo build --release -p neumannarch-game --target wasm32-unknown-unknown

rm -rf "${out}"
wasm-bindgen --target web --no-typescript --out-dir "${out}" --out-name game "${wasm}"
cp "${engine}/web/index.html" "${out}/index.html"

echo "serving http://localhost:${port} — a WebGPU browser is required; Ctrl-C to stop"
python3 -m http.server "${port}" --bind 127.0.0.1 --directory "${out}"
