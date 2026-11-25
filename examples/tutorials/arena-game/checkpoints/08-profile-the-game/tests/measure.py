"""Compare emitted baseline/candidate in independent native LÖVE processes."""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import shutil
import statistics
import subprocess
import tempfile

project = Path(__file__).resolve().parents[1]
repo = project.parents[4]
parser = argparse.ArgumentParser()
parser.add_argument("--tua", type=Path, required=True)
parser.add_argument("--love", type=Path, required=True)
parser.add_argument("--output", type=Path, required=True)
parser.add_argument("--repetitions", type=int, default=5)
args = parser.parse_args()
assert args.repetitions >= 3
compiler, love = args.tua.resolve(), args.love.resolve()
args.output.mkdir(parents=True, exist_ok=False)
hash_bytes = lambda data: hashlib.sha256(data).hexdigest()
hash_file = lambda path: hash_bytes(path.read_bytes())
def percentile(values, fraction):
    values = sorted(values)
    return values[math.ceil(len(values) * fraction) - 1]

def run(game, mode, enemies):
    result = subprocess.run([str(love), str(game)], env=dict(os.environ,
        ARENA_MODE=mode, ARENA_ENEMIES=str(enemies)), capture_output=True, text=True, timeout=60)
    if result.returncode:
        raise RuntimeError(result.stdout + result.stderr)
    records = {}
    for line in result.stdout.splitlines():
        if ":" in line:
            key, value = line.split(":", 1)
            if key in ("TIMES", "OUTCOME", "MEMORY", "RUNTIME"):
                records[key] = value
    assert "OUTCOME" in records and "RUNTIME" in records, result.stdout + result.stderr
    trial = {"runtime": records["RUNTIME"], "outcomeSha256": hash_bytes(records["OUTCOME"].encode())}
    if mode == "cpu":
        times = list(map(float, records["TIMES"].split(",")))
        assert len(times) == 600
        trial.update(samplesMs=times, medianMs=statistics.median(times), p95Ms=percentile(times, .95))
    else:
        allocated, retained = map(float, records["MEMORY"].split(","))
        trial.update(allocatedKiB=allocated, retainedDeltaKiB=retained)
    return trial

subprocess.run([str(compiler), "build"], cwd=project, check=True)
provenance = {
    "compiler": json.loads(subprocess.check_output([str(compiler), "--version", "--json"], text=True)),
    "revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repo, text=True).strip(),
    "worktreeDirty": bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=repo)),
    "host": platform.platform(), "machine": platform.machine(), "processor": platform.processor(),
    "loveSha256": hash_file(love), "harnessSha256": hash_file(project / "tests/benchmark.lua"),
    "baselineSourceSha256": hash_file(project / "tests/reference-combat.tua"),
    "candidateSourceSha256": hash_file(project / "src/combat.tua"),
    "workload": "arena-one-second-episodes-v2", "seed": 71,
    "warmupUpdates": 120, "measuredUpdates": 600, "repetitions": args.repetitions,
    "simulationDt": 1 / 60, "gcCpu": "running, default runtime settings",
    "gcAllocation": "separate 60-update probe with GC stopped; no reset inside measured block",
    "timing": "wall duration around simulation update only; excludes reset, draw, sleep and profiler",
    "order": "alternating baseline/candidate independent processes",
}
if platform.system() == "Darwin":
    provenance["cpu"] = subprocess.check_output(["sysctl", "-n", "machdep.cpu.brand_string"], text=True).strip()
results = {"provenance": provenance, "workloads": []}
with tempfile.TemporaryDirectory(prefix="tua-arena-compare-") as directory:
    temporary = Path(directory)
    reference = temporary / "reference"
    shutil.copytree(project, reference, ignore=shutil.ignore_patterns("build"))
    shutil.copyfile(project / "tests/reference-combat.tua", reference / "src/combat.tua")
    subprocess.run([str(compiler), "build"], cwd=reference, check=True)
    games = {}
    for variant, build in [("baseline", reference / "build"), ("candidate", project / "build")]:
        game = temporary / variant
        shutil.copytree(build, game)
        shutil.copyfile(project / "tests/benchmark.lua", game / "main.lua")
        (game / "conf.lua").write_text('io.stdout:setvbuf("no")\n'
            'function love.conf(t)\n  t.window = false\n  t.modules.audio = false\nend\n')
        games[variant] = game
        provenance[variant + "CombatLuaSha256"] = hash_file(game / "combat.lua")
    shutil.copyfile(games["baseline"] / "combat.lua", games["candidate"] / "reference.lua")
    subprocess.run(["lua", str(project / "tests/differential.lua")], cwd=games["candidate"], check=True)
    for enemies in (8, 32, 128):
        trials = {"baseline": [], "candidate": []}
        for repetition in range(args.repetitions):
            order = ("baseline", "candidate") if repetition % 2 == 0 else ("candidate", "baseline")
            for variant in order:
                trials[variant].append(run(games[variant], "cpu", enemies))
            assert trials["baseline"][-1]["outcomeSha256"] == trials["candidate"][-1]["outcomeSha256"]
        memories = {variant: run(game, "allocation", enemies) for variant, game in games.items()}
        assert memories["baseline"]["outcomeSha256"] == memories["candidate"]["outcomeSha256"]
        summaries = {}
        for variant in trials:
            medians = [trial["medianMs"] for trial in trials[variant]]
            summaries[variant] = {"medianMs": statistics.median(medians),
                "p95Ms": statistics.median(trial["p95Ms"] for trial in trials[variant]),
                "trialMedianRangeMs": [min(medians), max(medians)]}
        results["workloads"].append({"enemies": enemies, "trials": trials, "memory": memories, "summary": summaries})
        print(enemies, json.dumps(summaries), flush=True)
(args.output / "results.json").write_text(json.dumps(results, indent=2) + "\n")
print("ARENA_MEASURE_OK exact outcomes match; simulation timing is not rendered FPS")
