#!/bin/sh
set -eu
trial=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo=$(CDPATH= cd -- "$trial/../../../../.." && pwd)
mkdir -p "$repo/target/game-puzzle/runtime"
cat > "$repo/target/game-puzzle/runtime/main.lua" <<'LUA'
function love.load()
 local ok, err = pcall(dofile, os.getenv('TUA_PUZZLE_SMOKE'))
 if not ok then print(err); love.event.quit(1) end
end
LUA
export TUA_PUZZLE_BUILD="$trial/build"
export TUA_PUZZLE_SMOKE="$trial/smoke.lua"
exec "${LOVE_BIN:-/Applications/love.app/Contents/MacOS/love}" "$repo/target/game-puzzle/runtime"
