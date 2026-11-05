#!/bin/sh
# horizon-ctl against the game tools/play.sh started.
ROOT=$(cd "$(dirname "$0")/.." && pwd)
export HORIZON_HOME=$ROOT/target/home HORIZON_CONTROL=$ROOT/target/ctl.sock
exec "$ROOT/target/game/horizon-ctl" "$@"
