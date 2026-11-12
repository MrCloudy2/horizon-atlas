#!/bin/sh
# Map sharing over LAN, end to end: a host world (sharing set to Everyone)
# where the host walks far apart, opened to LAN, and a second game in a home
# of its own that joins it and stays at spawn. The guest's world map should
# show what only the host saw. Both offscreen and muted.
#
#   tools/share-test.sh OUT_DIR
set -eu
ROOT=$(cd "$(dirname "$0")/.." && pwd)
OUT=${1:?OUT_DIR}
mkdir -p "$OUT"
GAME=$ROOT/target/game
PORT=25611
WORLD=atlas-share-host
H="env HORIZON_HOME=$ROOT/target/home HORIZON_CONTROL=$ROOT/target/ctl.sock $GAME/horizon-ctl"
G="env HORIZON_HOME=$ROOT/target/home2 HORIZON_CONTROL=$ROOT/target/ctl2.sock $GAME/horizon-ctl"
DATA=horizon-mods/.horizon-mod-data/atlas

$H quit >/dev/null 2>&1 || true
$G quit >/dev/null 2>&1 || true
sleep 2

# The host shares with everyone; a fresh world each run.
rm -rf "$ROOT/target/home/saves/$WORLD"
S=$ROOT/target/home/$DATA/settings.txt
if [ -f "$S" ]; then
    sed -i '/^share_map=/d' "$S"
    printf 'share_map=all\n' >> "$S"
fi
# The guest: a home of its own, muted, nothing of Atlas's yet.
rm -rf "$ROOT/target/home2"
mkdir -p "$ROOT/target/home2"
echo 'soundCategory_master:0.0' > "$ROOT/target/home2/options.txt"

echo "host: starting"
HORIZON_HOME=$ROOT/target/home HORIZON_CONTROL=$ROOT/target/ctl.sock \
    "$GAME/horizon-ctl" launch --game "$GAME/horizon-client" --offscreen \
    --world "$WORLD" --seed 424242 --creative --wait >/dev/null
$H --timeout 120 wait-ready >/dev/null
# Walk far apart so the server loads (and the share keeps) those chunks.
for at in "0 120 0" "400 120 0" "400 120 400" "-400 120 -300" "0 120 0"; do
    $H tp $at >/dev/null
    $H wait 140 >/dev/null
done
$H lan open $PORT >/dev/null
echo "host: open on $PORT"
ls "$ROOT/target/home/saves/$WORLD/atlas-share" 2>/dev/null | sed 's/^/host share: /'

echo "guest: starting"
HORIZON_HOME=$ROOT/target/home2 HORIZON_CONTROL=$ROOT/target/ctl2.sock \
    "$GAME/horizon-ctl" launch --game "$GAME/horizon-client" --offscreen >/dev/null
sleep 5
$G connect "127.0.0.1:$PORT" Guest >/dev/null
$G --timeout 120 wait-ready >/dev/null
echo "guest: joined"
# Let the catch-up arrive and be coloured.
$G wait 2400 >/dev/null
$G key m tap >/dev/null
$G wait 20 >/dev/null
for _ in 1 2 3; do $G scroll-at 320 180 -1 >/dev/null; $G wait 5 >/dev/null; done
$G wait 40 >/dev/null
$G screenshot "$OUT/guest-map.png" >/dev/null
$H key m tap >/dev/null
$H wait 20 >/dev/null
for _ in 1 2 3; do $H scroll-at 320 180 -1 >/dev/null; $H wait 5 >/dev/null; done
$H wait 40 >/dev/null
$H screenshot "$OUT/host-map.png" >/dev/null
$G key esc tap >/dev/null
$G wait 300 >/dev/null
find "$ROOT/target/home2/$DATA/maps" -name 'shared.txt' -exec sh -c 'echo "guest $1: $(cat "$1")"' _ {} \; 2>/dev/null
$G quit >/dev/null 2>&1 || true
$H quit >/dev/null 2>&1 || true
echo done
