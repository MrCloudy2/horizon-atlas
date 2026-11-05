#!/bin/sh
# Builds the game with only this bundle (Horizon's `fast` profile) and lays
# out a playable copy of the installed version under target/game, its game
# binary replaced, for tools/play.sh. The launcher's own builds are untouched.
set -eu
ROOT=$(cd "$(dirname "$0")/.." && pwd)
L=$HOME/.local/share/horizon-launcher
"$ROOT/tools/check.sh" >/dev/null
export PATH=$L/rust/1.99.1-x86_64-unknown-linux-gnu/bin:$PATH CARGO_HOME=$L/cargo
export CARGO_TARGET_DIR=$ROOT/target/hcheck/target
cd "$ROOT/target/hcheck/stage"
cargo build --offline --profile fast -p horizon-client --bin horizon-client --message-format short
REV=$(python3 -c "import json;print(json.load(open('$ROOT/compatibility.json'))['horizon_revision'][:7])")
BASE=$(ls -d "$L"/versions/nightly-*-"$REV" | tail -1)
GAME=$ROOT/target/game
if [ ! -d "$GAME" ]; then
    cp -al "$BASE" "$GAME"
fi
rm -f "$GAME/horizon-client"
cp "$CARGO_TARGET_DIR/fast/horizon-client" "$GAME/horizon-client"
echo "game: $GAME/horizon-client"
