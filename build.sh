#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

wasm-pack build --release --target web
cp pkg/breakout_game.js ./breakout_game.js
cp pkg/breakout_game_bg.wasm ./breakout_game_bg.wasm

echo "Built and copied breakout_game.js + breakout_game_bg.wasm to repo root."