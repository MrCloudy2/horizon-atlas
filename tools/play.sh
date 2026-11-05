#!/bin/sh
# Starts the game built by tools/build-game.sh offscreen, under control, in a
# home of its own (target/home: its own saves and options, sound off), in a
# creative test world. Then drive it with tools/ctl.sh (screenshot, key, ...).
set -eu
ROOT=$(cd "$(dirname "$0")/.." && pwd)
GAME=$ROOT/target/game
export HORIZON_HOME=$ROOT/target/home HORIZON_CONTROL=$ROOT/target/ctl.sock
mkdir -p "$HORIZON_HOME"
# Muted: master volume 0.
if ! grep -q '^soundCategory_master:' "$HORIZON_HOME/options.txt" 2>/dev/null; then
    echo 'soundCategory_master:0.0' >> "$HORIZON_HOME/options.txt"
fi
exec "$GAME/horizon-ctl" launch --game "$GAME/horizon-client" --offscreen \
    --world "${ATLAS_WORLD:-atlas-test}" --seed "${ATLAS_SEED:-8675309}" --creative --wait "$@"
