"""Check/build the showcase, run headless outcomes and optionally capture native replay."""
import argparse
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parent
repo = root.parents[2]
parser = argparse.ArgumentParser()
parser.add_argument("--tua", default=str(repo / "target/debug/tua"))
parser.add_argument("--love")
parser.add_argument("--output", type=Path)
parser.add_argument("--video", action="store_true")
args = parser.parse_args()
tua = str(Path(args.tua).resolve())
for command in [
    [tua, "fmt", "--check"],
    [tua, "fmt", "--check", "src/conf.lua", "tests/controller.lua", "tests/replay.lua"],
    [tua, "check"],
    [tua, "build"],
]:
    subprocess.run(command, cwd=root, check=True)
subprocess.run(["lua", "../tests/controller.lua"], cwd=root / "build", check=True)
if args.love:
    output = args.output
    if output is None:
        output = Path(tempfile.mkdtemp(prefix="tua-showcase-evidence-")) / "replay"
    command = [
        "python3", str(repo / "scripts/love_replay.py"), "--project", str(root),
        "--tua", tua, "--love", args.love, "--scenario", str(root / "tests/replay.lua"),
        "--frames", "180", "--fps", "30", "--seed", "71", "--output", str(output),
    ]
    if args.video:
        command.append("--video")
    subprocess.run(command, check=True)
