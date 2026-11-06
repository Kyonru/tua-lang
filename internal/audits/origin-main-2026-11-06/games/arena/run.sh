#!/bin/sh
set -eu
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo=$(CDPATH= cd -- "$here/../../../../.." && pwd)
trial="$repo/target/game-arena"
mkdir -p "$trial"
cp -R "$here/src" "$trial/"
cp "$here/tua.toml" "$trial/"
cd "$trial"
"$repo/target/release/tua" check
"$repo/target/release/tua" build
if [ "${1:-}" = smoke ]; then
  mkdir -p smoke
  cp "$here/runtime-smoke.lua" smoke/main.lua
  cat > smoke/conf.lua <<'LUA'
function love.conf(t)
  t.window=false
  t.modules.audio=false
  t.modules.graphics=false
  t.modules.window=false
end
LUA
  TUA_ARENA_BUILD="$trial/build" "${LOVE_BIN:-/Applications/love.app/Contents/MacOS/love}" smoke
else
  "${LOVE_BIN:-/Applications/love.app/Contents/MacOS/love}" build
fi
