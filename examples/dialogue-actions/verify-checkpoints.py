"""Check the typed example and checkpoint formats under separate native processes."""
import argparse
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import uuid

root = Path(__file__).resolve().parent
repo = root.parents[1]
parser = argparse.ArgumentParser()
parser.add_argument("--tua", default=str(repo / "target/debug/tua"))
parser.add_argument("--love")
args = parser.parse_args()
for command in [["fmt", "--check"], ["fmt", "--check", "tests/checkpoint-native.lua"],
                ["check"], ["build"]]:
    subprocess.run([args.tua, *command], cwd=root, check=True)
subprocess.run(["lua", "main.lua"], cwd=root / "build", check=True)
if args.love:
    with tempfile.TemporaryDirectory(prefix="tua-settled-checkpoint-") as directory:
        isolated = Path(directory)
        shutil.copytree(root / "build", isolated, dirs_exist_ok=True)
        for version, suffix in [(1, ""), (2, "_context"), (3, "_effects")]:
            shutil.copy2(repo / f"crates/tua-core/tests/fixtures/dialogue_checkpoint{suffix}.lua",
                         isolated / f"v{version}.lua")
        shutil.copy2(root / "tests/checkpoint-native.lua", isolated / "main.lua")
        (isolated / "conf.lua").write_text(
            "function love.conf(t)\n"
            "  t.identity = os.getenv(\"TUA_CHECKPOINT_ID\")\n"
            "  t.window = false\n  t.modules.graphics = false\n"
            "  t.modules.audio = false\nend\n"
        )
        for mode in ["on", "off"]:
            identity = "tua-settled-test-" + uuid.uuid4().hex
            for phase in ["fixtures", "write", "read"]:
                env = dict(os.environ, TUA_CHECKPOINT_JIT=mode,
                           TUA_CHECKPOINT_PHASE=phase, TUA_CHECKPOINT_ID=identity)
                subprocess.run([args.love, str(isolated)], env=env, check=True, timeout=30)
