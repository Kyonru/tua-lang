#!/bin/sh
set -eu
HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO=$(CDPATH= cd -- "$HERE/../../../../.." && pwd)
DEST="$REPO/target/game-platformer/project"
mkdir -p "$DEST"
cp "$HERE/tua.toml" "$DEST/tua.toml"
mkdir -p "$DEST/src"
cp "$HERE"/src/*.tua "$DEST/src/"
cd "$DEST"
"$REPO/target/release/tua" check
"$REPO/target/release/tua" build
if [ "${1:-}" = smoke ]; then
  cd build
  lua -e 'package.path="./?.lua;"..package.path; dofile(arg[1])' "$HERE/smoke.lua"
else
  exec "${LOVE_BIN:-/Applications/love.app/Contents/MacOS/love}" build
fi
