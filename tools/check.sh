#!/bin/sh
# Applies the bundle to the pinned Horizon source with the launcher's
# horizon-mixins, then type-checks the game (`cargo check`), all under
# target/hcheck. Uses the launcher's Rust and crate cache (offline).
set -eu
ROOT=$(cd "$(dirname "$0")/.." && pwd)
L=$HOME/.local/share/horizon-launcher
REV=$(python3 -c "import json;print(json.load(open('$ROOT/compatibility.json'))['horizon_revision'])")
MIXREV=$(python3 -c "import json;print(json.load(open('$ROOT/compatibility.json'))['horizon_mixins_revision'])")
SRC=$L/native/source/$REV
TOOL=$L/native/tools/$(echo "$MIXREV" | cut -c1-12)/bin/horizon-mixins
OUT=$ROOT/target/hcheck
[ -d "$SRC" ] || { echo "no Horizon source for $REV (build it once in the launcher)"; exit 1; }
[ -x "$TOOL" ] || { echo "no horizon-mixins $MIXREV in the launcher"; exit 1; }
rm -rf "$OUT/stage"
mkdir -p "$OUT"
"$TOOL" apply --workspace "$SRC" --mods "$ROOT/mixins" --out "$OUT/stage"
export PATH=$L/rust/1.99.1-x86_64-unknown-linux-gnu/bin:$PATH CARGO_HOME=$L/cargo
export CARGO_TARGET_DIR=$OUT/target
cd "$OUT/stage"
cargo check --offline -p horizon-client --message-format short "$@"
