"""Check/build the showcase, run headless outcomes and optionally capture native replay."""
import argparse
import hashlib
import json
import struct
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
parser.add_argument("--scenario", choices=["lifecycle", "styles", "rich", "staging", "transcript"], default="lifecycle")
args = parser.parse_args()
tua = str(Path(args.tua).resolve())
portrait_root = root / "src/assets/portraits"
manifest = json.loads((portrait_root / "provenance.json").read_text())
portrait_hashes = set()
for asset in manifest["assets"]:
    data = (portrait_root / asset["file"]).read_bytes()
    digest = hashlib.sha256(data).hexdigest()
    assert data[:8] == b"\x89PNG\r\n\x1a\n" and struct.unpack(">II", data[16:24]) == (256, 256)
    assert digest == asset["sha256"], asset["file"]
    assert digest not in portrait_hashes, "expression images must differ"
    portrait_hashes.add(digest)

for command in [
    [tua, "fmt", "--check"],
    [tua, "fmt", "--check", "src/conf.lua", "tests/controller.lua", "tests/replay.lua", "tests/host_ui.lua", "tests/story.lua", "tests/styles.lua", "tests/portraits.lua", "tests/rich.lua", "tests/rich_replay.lua", "tests/staging.lua", "tests/staging_replay.lua", "tests/transcript_replay.lua"],
    [tua, "check"],
    [tua, "build"],
]:
    subprocess.run(command, cwd=root, check=True)
for test in ["controller", "host_ui", "story", "portraits", "rich", "staging"]:
    subprocess.run(["lua", f"../tests/{test}.lua"], cwd=root / "build", check=True)
if args.love:
    output = args.output
    if output is None:
        output = Path(tempfile.mkdtemp(prefix="tua-showcase-evidence-")) / "replay"
    scenario = {"lifecycle": ("replay", 180), "styles": ("styles", 520), "rich": ("rich_replay", 360), "staging": ("staging_replay", 240), "transcript": ("transcript_replay", 180)}[args.scenario]
    command = [
        "python3", str(repo / "scripts/love_replay.py"), "--project", str(root),
        "--tua", tua, "--love", args.love, "--scenario", str(root / f"tests/{scenario[0]}.lua"),
        "--frames", str(scenario[1]), "--fps", "30", "--seed", "71", "--output", str(output),
    ]
    if args.video:
        command.append("--video")
    subprocess.run(command, check=True)
