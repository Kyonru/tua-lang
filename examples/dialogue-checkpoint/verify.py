"""Check typed persistence, fault handling and separate native restart processes."""
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
    ["fmt", "--check", "tests/memory.lua", "tests/persistence.lua", "tests/native.lua"],
    ["check"],
    ["build"],
]:
    subprocess.run([args.tua, *command], cwd=root, check=True)
for script in ["main.lua", "../tests/persistence.lua"]:
    subprocess.run(["lua", script], cwd=root / "build", check=True)
if args.love:
    with tempfile.TemporaryDirectory(prefix="tua-dialogue-checkpoint-") as directory:
        isolated = Path(directory)
        shutil.copytree(root / "build", isolated, dirs_exist_ok=True)
        shutil.copy2(root / "tests/native.lua", isolated / "main.lua")
        (isolated / "conf.lua").write_text(
            "function love.conf(t)\n"
            "  t.identity = os.getenv(\"TUA_CHECKPOINT_TEST_ID\")\n"
            "  t.window = false\n  t.modules.graphics = false\n"
            "  t.modules.audio = false\nend\n"
        )
        for mode in ["on", "off"]:
            identity = "tua-checkpoint-test-" + uuid.uuid4().hex
            for phase in ["line", "choice", "wait", "finish"]:
                env = dict(os.environ, TUA_CHECKPOINT_JIT=mode,
                           TUA_CHECKPOINT_PHASE=phase, TUA_CHECKPOINT_TEST_ID=identity)
                subprocess.run([args.love, str(isolated)], env=env,
                               check=True, timeout=30)
