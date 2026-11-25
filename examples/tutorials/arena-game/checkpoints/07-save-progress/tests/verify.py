"""Build typed source and verify emitted persistence across two isolated LÖVE processes."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import uuid

root = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser()
parser.add_argument("--tua", type=Path, required=True)
parser.add_argument("--love", type=Path, required=True)
parser.add_argument("--output", type=Path, required=True)
args = parser.parse_args()
tua, love = args.tua.resolve(), args.love.resolve()
output = args.output.resolve()
output.mkdir(parents=True, exist_ok=False)
for command in [
    [str(tua), "fmt", "--check"],
    [str(tua), "fmt", "--check", "tests/persistence.lua", "tests/save-replay.lua"],
    [str(tua), "check", "--deny-warnings"],
    [str(tua), "build"],
]:
    subprocess.run(command, cwd=root, check=True)
identity = "tua-arena-save-test-" + uuid.uuid4().hex
metadata = {
    "compiler": json.loads(subprocess.check_output([str(tua), "--version", "--json"], text=True)),
    "identity": identity,
    "love_executable": str(love),
    "love_sha256": hashlib.sha256(love.read_bytes()).hexdigest(),
    "test_sha256": hashlib.sha256((root / "tests/persistence.lua").read_bytes()).hexdigest(),
    "timing": "functional persistence checks, not performance measurements",
}
with tempfile.TemporaryDirectory(prefix="tua-save-processes-") as directory:
    game = Path(directory) / "game"
    shutil.copytree(root / "build", game)
    shutil.copyfile(root / "tests/persistence.lua", game / "test_persistence.lua")
    (game / "conf.lua").write_text(
        f'function love.conf(t)\n  t.identity = "{identity}"\n  t.window = false\n  t.modules.audio = false\nend\n'
    )
    (game / "main.lua").write_text("""
io.stdout:setvbuf("no")
function love.errorhandler(message)
  print(debug.traceback(tostring(message), 2))
  return function() return 1 end
end
function love.load()
  if jit then jit.off() end
  local major, minor, revision = love.getVersion()
  print("SAVE_RUNTIME", string.format("%d.%d.%d", major, minor, revision),
    jit and jit.version or _VERSION, jit and jit.status() or false)
  print("SAVE_DIRECTORY", love.filesystem.getSaveDirectory())
  require("test_persistence").run(assert(os.getenv("TUA_SAVE_PHASE")))
  love.event.quit()
end
""")
    for phase in ("1", "2"):
        result = subprocess.run([str(love), str(game)], env=dict(os.environ, TUA_SAVE_PHASE=phase),
                                text=True, capture_output=True, timeout=30)
        log = result.stdout + result.stderr
        (output / f"phase-{phase}.log").write_text(log)
        if result.returncode or f"SAVE_PHASE_{phase}_OK" not in log:
            raise SystemExit(f"Persistence phase {phase} failed:\n{log}")
        print(result.stdout.strip())
        for line in result.stdout.splitlines():
            if line.startswith("SAVE_RUNTIME\t"):
                _, love_version, lua_version, jit_enabled = line.split("\t")
                metadata["runtime"] = {"love": love_version, "lua": lua_version, "jit_enabled": jit_enabled == "true"}
        if phase == "2":
            for line in result.stdout.splitlines():
                if line.startswith("SAVE_DIRECTORY\t"):
                    savedir = Path(line.split("\t", 1)[1])
                    if savedir.name == identity:
                        # Remove only the unique identity created by this invocation.
                        shutil.rmtree(savedir)
    metadata["status"] = "passed"
(output / "provenance.json").write_text(json.dumps(metadata, indent=2) + "\n")
print("SAVE_PERSISTENCE_OK two independent LÖVE processes")
