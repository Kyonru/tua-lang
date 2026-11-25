"""Qualify independent arena checkpoints from fresh tracked-file copies.

No performance conclusions: native runs are deterministic functional replays.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

repo = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser()
parser.add_argument("--tua", type=Path, required=True)
parser.add_argument("--love", type=Path)
parser.add_argument("--lua", default="lua", help="Lua interpreter executable or path")
parser.add_argument("--output", type=Path, required=True)
args = parser.parse_args()
lua = shutil.which(args.lua)
if lua is None:
    parser.error(f"Lua interpreter not found: {args.lua}")
lua = str(Path(lua).resolve())
tua = args.tua.resolve()
love = args.love.resolve() if args.love else None
output = args.output.resolve()
output.mkdir(parents=True, exist_ok=False)
chapters = repo / "examples/tutorials/arena-game/checkpoints"
result = {
    "compiler": json.loads(subprocess.check_output([str(tua), "--version", "--json"], text=True)),
    "revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repo, text=True).strip(),
    "worktreeDirty": bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=repo)),
    "scope": "agent-led clean-copy functional qualification; not a human walkthrough or benchmark",
    "native": "requested" if love else "not requested",
    "lua": {"executable": lua, "version": subprocess.check_output(
        [lua, "-v"], text=True, stderr=subprocess.STDOUT).strip()},
    "chapters": [],
}
if love:
    result["loveSha256"] = hashlib.sha256(love.read_bytes()).hexdigest()

def run(command, cwd, log, env=None):
    process = subprocess.run(command, cwd=cwd, env=env, text=True, capture_output=True, timeout=180)
    with log.open("a") as file:
        file.write("$ " + " ".join(str(part) for part in command) + "\n")
        file.write(process.stdout + process.stderr)
    if process.returncode:
        raise RuntimeError(f"Command failed; see {log}\n{process.stdout}{process.stderr}")

with tempfile.TemporaryDirectory(prefix="tua-arena-path-") as directory:
    clean = Path(directory)
    binary = clean / "bin"
    binary.mkdir()
    (binary / "tua").symlink_to(tua)
    env = dict(os.environ, PATH=str(binary) + os.pathsep + os.environ.get("PATH", ""))
    for original in sorted(chapters.iterdir()):
        if not original.is_dir():
            continue
        relative = original.relative_to(repo)
        project = clean / relative
        tracked = subprocess.check_output(["git", "ls-files", "-z", str(relative)], cwd=repo).decode().split("\0")
        digest = hashlib.sha256()
        for name in filter(None, tracked):
            source = repo / name
            destination = clean / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, destination)
            digest.update(name.encode() + b"\0" + source.read_bytes())
        assert not (project / "build").exists()
        log = output / (original.name + ".log")
        for command in (["tua", "fmt", "--check"], ["tua", "check", "--deny-warnings"], ["tua", "build"]):
            run(command, project, log, env)
        lua_files = sorted(str(file.relative_to(project)) for file in project.rglob("*.lua")
                           if "build" not in file.relative_to(project).parts)
        if lua_files:
            run(["tua", "fmt", "--check", *lua_files], project, log, env)
        reference = project / "tests/reference-combat.tua"
        if reference.exists():
            run(["tua", "fmt", "--check", "tests/reference-combat.tua"], project, log, env)
        suites = []
        for suite in ("movement", "session", "combat", "decisions", "soundscape", "conversation"):
            if (project / f"tests/{suite}.lua").exists():
                run([lua, f"../tests/{suite}.lua"], project / "build", log)
                suites.append(suite)
        native = []
        if love:
            frames = {"replay": 125 if original.name[:2] in ("01", "02") else 350,
                      "shader-replay": 50, "dialogue-replay": 60, "save-replay": 40, "workload-replay": 45}
            for scenario, count in frames.items():
                path = project / f"tests/{scenario}.lua"
                if not path.exists():
                    continue
                destination = output / f"{original.name}-{scenario}"
                run(["python3", str(repo / "scripts/love_replay.py"), "--project", str(project),
                     "--tua", str(tua), "--love", str(love), "--scenario", str(path), "--frames", str(count),
                     "--fps", "30", "--seed", "71", "--output", str(destination), "--capture"], repo, log)
                native.append({"scenario": scenario, "frames": count,
                               "provenance": json.loads((destination / "provenance.json").read_text())})
            if (project / "tests/verify.py").exists():
                destination = output / (original.name + "-persistence")
                run(["python3", str(project / "tests/verify.py"), "--tua", str(tua),
                     "--love", str(love), "--output", str(destination)], repo, log)
                native.append({"scenario": "two-process persistence",
                               "provenance": json.loads((destination / "provenance.json").read_text())})
        result["chapters"].append({"chapter": original.name, "trackedSourceSha256": digest.hexdigest(),
                                   "headlessSuites": suites, "native": native, "status": "passed"})
        print(original.name + " passed", flush=True)
result["status"] = "passed"
(output / "qualification.json").write_text(json.dumps(result, indent=2) + "\n")
print("ARENA_PATH_OK clean copies, formatting, checks, builds and selected runtime suites")
print("Qualification report: " + str(output / "qualification.json"))
