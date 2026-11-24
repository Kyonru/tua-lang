"""Qualify typed recovery, emitted Lua faults and separate native restart processes."""
import argparse
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import uuid

root = Path(__file__).resolve().parent
parser = argparse.ArgumentParser()
parser.add_argument("--tua", default=str(root.parents[1] / "target/debug/tua"))
parser.add_argument("--love")
args = parser.parse_args()
for command in [
    ["fmt", "--check"],
    ["fmt", "--check", "tests/memory.lua", "tests/recovery.lua", "tests/native.lua"],
    ["check"],
    ["build"],
]:
    subprocess.run([args.tua, *command], cwd=root, check=True)
subprocess.run(["lua", "../tests/recovery.lua"], cwd=root / "build", check=True)
if args.love:
    with tempfile.TemporaryDirectory(prefix="tua-dialogue-durable-") as directory:
        isolated = Path(directory)
        shutil.copytree(root / "build", isolated, dirs_exist_ok=True)
        for name in ["memory", "recovery"]:
            shutil.copy2(root / f"tests/{name}.lua", isolated / f"{name}.lua")
        shutil.copy2(root / "tests/native.lua", isolated / "main.lua")
        (isolated / "conf.lua").write_text(
            "function love.conf(t)\n"
            "  t.identity = os.getenv(\"TUA_DURABLE_TEST_ID\")\n"
            "  t.window = false\n  t.modules.graphics = false\n"
            "  t.modules.audio = false\nend\n"
        )
        for mode in ["on", "off"]:
            identity = "tua-durable-test-" + uuid.uuid4().hex
            for phase in ["fixtures", "prepare", "commit", "replay", "verify"]:
                env = dict(os.environ, TUA_DURABLE_JIT=mode,
                           TUA_DURABLE_PHASE=phase, TUA_DURABLE_TEST_ID=identity)
                subprocess.run([args.love, str(isolated)], env=env,
                               check=True, timeout=30)
