#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

wasm-pack build --release --target web
cp pkg/game.js ./game.js
cp pkg/game_bg.wasm ./game_bg.wasm

echo "Built and copied game.js + game_bg.wasm to repo root."
