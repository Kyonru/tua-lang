#!/bin/sh
set -eu
HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO=$(CDPATH= cd -- "$HERE/../../../../.." && pwd)
"$HERE/run.sh" smoke
SMOKE="$REPO/target/game-platformer/smoke"
VISUAL="$REPO/target/game-platformer/visual"
mkdir -p "$SMOKE" "$VISUAL"
cp "$REPO/target/game-platformer/project/build/world.lua" "$SMOKE/world.lua"
cp "$HERE/smoke.lua" "$SMOKE/test.lua"
cat > "$SMOKE/conf.lua" <<'LUA'
function love.conf(t)
 t.window=false
 t.modules.graphics=false
 t.modules.audio=false
 t.modules.joystick=false
 t.modules.physics=false
end
LUA
cat > "$SMOKE/main.lua" <<'LUA'
function love.load()
 local ok,err=pcall(require,'test')
 if not ok then print(err) end
 os.exit(ok and 0 or 1)
end
LUA
"${LOVE_BIN:-/Applications/love.app/Contents/MacOS/love}" "$SMOKE"
cp "$REPO/target/game-platformer/project/build/"*.lua "$VISUAL/"
cat >> "$VISUAL/main.lua" <<'LUA'
local originalUpdate = love.update
local frames = 0
function love.update(dt)
 originalUpdate(dt)
 frames=frames+1
 if frames==30 then print('PASS 30 update/draw frames');love.event.quit(0) end
end
LUA
"${LOVE_BIN:-/Applications/love.app/Contents/MacOS/love}" "$VISUAL"
