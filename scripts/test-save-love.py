#!/usr/bin/env python3
"""Exercise the bundled save backend under a real LÖVE 11 host when installed."""

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
    with tempfile.TemporaryDirectory(prefix="tua-save-love-") as directory:
        project = Path(directory)
        for name in ("tuasave.lua", "tuasave_json.lua"):
            shutil.copyfile(ROOT / "vendor" / "runtime" / name, project / name)
        identity = f"tua-save-smoke-{os.getpid()}"
        (project / "conf.lua").write_text(
            f'function love.conf(t) t.identity = "{identity}"; t.window = false end\n'
        )
        (project / "main.lua").write_text(
            """local Save = require("tuasave")
function love.load()
  local schema = Save.record({ score = Save.number(0), name = Save.string("Ada") })
  local profile = Save.contract({ slot = "profile", version = 1, schema = schema })
  local prior, issue = profile:load()
  assert(prior == nil and issue == nil)
  local state = profile:defaults(); state.score = 7
  local saved, problem = profile:save(state)
  assert(saved, problem and problem.message)
  local again = Save.contract({ slot = "profile", version = 1, schema = schema })
  assert(again:load().score == 7)
  assert(love.filesystem.remove("__tua_saves/profile.a"))
  assert(love.filesystem.remove("__tua_saves"))
  print("love-save-ok")
  love.event.quit()
end
"""
        )
        env = os.environ.copy()
        env.setdefault("SDL_VIDEODRIVER", "dummy")
        result = subprocess.run(
            [LOVE, str(project)],
            env=env,
            capture_output=True,
            timeout=20,
            check=False,
        )
        output = result.stdout.decode("utf-8", "replace")
        errors = result.stderr.decode("utf-8", "replace")
        if result.returncode or "love-save-ok" not in output:
            raise SystemExit(
                f"LÖVE save smoke failed ({result.returncode}):\n{output}\n{errors}"
            )
        print("love-save-ok")


if __name__ == "__main__":
    main()
