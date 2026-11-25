"""Build typed sources, run emitted Lua tests, optionally test native LÖVE audio."""
import argparse
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

root = Path(__file__).resolve().parent
parser = argparse.ArgumentParser()
parser.add_argument("--tua", default=str(root.parents[2] / "target/debug/tua"))
parser.add_argument("--love")
parser.add_argument("--render", action="store_true")
args = parser.parse_args()
commands = [
    [args.tua, "fmt", "--check"],
    [args.tua, "fmt", "--check", "tests/controller.lua", "tests/backend.lua", "tests/native.lua"],
    [args.tua, "check"],
    [args.tua, "build"],
]
for command in commands:
    subprocess.run(command, cwd=root, check=True)
for test in ["controller", "backend"]:
    subprocess.run(["lua", f"../tests/{test}.lua"], cwd=root / "build", check=True)
if args.love:
    with tempfile.TemporaryDirectory(prefix="tua-dialogue-audio-") as directory:
        isolated = Path(directory)
        shutil.copytree(root / "build", isolated, dirs_exist_ok=True)
        (isolated / "main.lua").rename(isolated / "showcase.lua")
        shutil.copy2(root / "tests/native.lua", isolated / "main.lua")
        for test in ["controller", "backend"]:
            shutil.copy2(root / f"tests/{test}.lua", isolated / f"{test}_test.lua")
        if not args.render:
            (isolated / "conf.lua").write_text(
                "function love.conf(t)\n  t.window = false\n"
                "  t.modules.graphics = false\nend\n"
            )
        for mode in ["on", "off"]:
            env = dict(
                os.environ,
                TUA_AUDIO_JIT=mode,
                TUA_AUDIO_RENDER="1" if args.render else "0",
            )
            subprocess.run([args.love, str(isolated)], env=env, check=True, timeout=45)
