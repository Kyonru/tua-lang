#!/usr/bin/env python3
"""Measure bundled save payloads through a real LÖVE filesystem backend."""

import os
from pathlib import Path
import shutil
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[1]
LOVE = os.environ.get("TUA_LOVE_BIN") or "/Applications/LOVE.app/Contents/MacOS/love"


def main() -> None:
    if not Path(LOVE).is_file():
        raise SystemExit(f"LÖVE executable unavailable: {LOVE}")
    with tempfile.TemporaryDirectory(prefix="tua-save-bench-") as directory:
        project = Path(directory)
        for name in ("tuasave.lua", "tuasave_json.lua"):
            shutil.copyfile(ROOT / "vendor" / "runtime" / name, project / name)
        identity = f"tua-save-bench-{os.getpid()}"
        (project / "conf.lua").write_text(
            f'function love.conf(t) t.identity = "{identity}"; t.window = false end\n'
        )
        (project / "main.lua").write_text(
            """local Save = require('tuasave')
function love.load()
  for _, size in ipairs({ 10 * 1024, 100 * 1024, 1024 * 1024 }) do
    local slot = 'payload' .. size
    local schema = Save.record({ data = Save.string('') })
    local profile = Save.contract({slot = slot, version = 1,
      schema = schema, max_bytes = 4 * 1024 * 1024})
    local snapshot = {data = string.rep('x', size)}
    local saveMs, loadMs = 0, 0
    for _ = 1, 3 do
      local started = love.timer.getTime()
      local ok, problem = profile:save(snapshot)
      assert(ok, problem and problem.message)
      saveMs = saveMs + (love.timer.getTime() - started) * 1000
      local fresh = Save.contract({slot = slot, version = 1,
        schema = schema, max_bytes = 4 * 1024 * 1024})
      started = love.timer.getTime()
      local loaded, issue = fresh:load()
      assert(loaded and loaded.data == snapshot.data and not issue)
      loadMs = loadMs + (love.timer.getTime() - started) * 1000
    end
    print(string.format('payload=%d bytes save=%.2f ms load=%.2f ms (mean of 3)',
      size, saveMs / 3, loadMs / 3))
    love.filesystem.remove('__tua_saves/' .. slot .. '.a')
    love.filesystem.remove('__tua_saves/' .. slot .. '.b')
  end
  love.filesystem.remove('__tua_saves')
  love.event.quit()
end
"""
        )
        env = os.environ.copy()
        env.setdefault("SDL_VIDEODRIVER", "dummy")
        result = subprocess.run(
            [LOVE, str(project)], env=env, capture_output=True, timeout=60, check=False
        )
        output = result.stdout.decode("utf-8", "replace")
        errors = result.stderr.decode("utf-8", "replace")
        if result.returncode or "payload=1048576" not in output:
            raise SystemExit(f"LÖVE save benchmark failed ({result.returncode}):\n{output}\n{errors}")
        print(output.strip())


if __name__ == "__main__":
    main()
