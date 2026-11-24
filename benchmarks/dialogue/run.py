#!/usr/bin/env python3
"""Reproduce dialogue M0 baseline with the selected compiler; writes only to output/temp dirs."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(command, cwd, env=None, timeout=120):
    result = subprocess.run(command, cwd=cwd, env=env, capture_output=True, text=True, timeout=timeout)
    if result.returncode:
        raise RuntimeError(f"{command}: {result.stdout}\n{result.stderr}")
    return result.stdout + result.stderr


def metrics(log):
    result = []
    for line in log.splitlines():
        if line.startswith("METRIC\t"):
            _, workload, unit, median, p95, samples, minimum, maximum = line.split("\t")
            result.append(dict(workload=workload, unit=unit, median=float(median), p95=float(p95),
                               samples=int(samples), minimum=float(minimum), maximum=float(maximum)))
    if not result:
        raise RuntimeError("No benchmark metrics returned")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tua", type=Path, default=ROOT / "target/release/tua")
    parser.add_argument("--love", type=Path, default=Path("/Applications/love.app/Contents/MacOS/love"))
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    args.tua = args.tua.resolve()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    report = {
        "schema_version": 1,
        "repository_revision": run(["git", "rev-parse", "HEAD"], ROOT).strip(),
        "repository_dirty": bool(run(["git", "status", "--porcelain"], ROOT).strip()),
        "compiler": json.loads(run([str(args.tua), "--version", "--json"], ROOT)),
        "host": platform.platform(),
        "machine": platform.machine(),
        "fixture_hashes": {str(p.relative_to(ROOT)): digest(p) for p in sorted((HERE / "fixture").rglob("*"))
                           if p.is_file() and "build" not in p.parts},
        "harness_hashes": {str(p.relative_to(ROOT)): digest(p) for p in
                           [HERE / name for name in ["run.py", "runtime.lua", "love.lua", "ownership.lua", "editor.js"]]
                           + [ROOT / "editors/vscode/scripts/lsp-profile-common.js"]},
        "cpu_policy": "os.clock process CPU, GC running/default tunables, no profiler; 200 warmup operations; 7 trials in alternating workload order; batch-average p95 is not per-frame p95",
        "allocation_policy": "separate 7 x 200-operation batches, GC stopped; Lua heap growth proxy excludes C/GPU allocations; retained delta after full collection may include noise",
        "render_policy": "synthetic 16-panel reference renderer, 60 warmup + 180 measured callbacks, fixed 1/60 simulation, CPU callbacks exclude GPU completion/presentation; not FPS",
        "probes": [], "runs": [], "unavailable": [],
    }
    with tempfile.TemporaryDirectory(prefix="tua-dialogue-m0-") as directory:
        project = Path(directory)
        shutil.copytree(HERE / "fixture", project, dirs_exist_ok=True)
        original = (project / "src/main.tua").read_text()
        for probe in sorted((project / "probes").glob("*.tua")):
            (project / "src/main.tua").write_text(probe.read_text())
            checked = subprocess.run([str(args.tua), "check"], cwd=project, capture_output=True, text=True, timeout=120)
            diagnostics = (checked.stdout + checked.stderr).replace(str(project.resolve()), "<fixture>").replace(str(project), "<fixture>")
            report["probes"].append({"name": probe.stem, "accepted": checked.returncode == 0, "diagnostics": diagnostics})
        (project / "src/main.tua").write_text(original)
        run([str(args.tua), "check"], project)
        run([str(args.tua), "build"], project)
        build = project / "build"
        emitted = build / "tuadialogue.lua"
        assert emitted.read_bytes() == (ROOT / "vendor/runtime/tuadialogue.lua").read_bytes(), "Selected compiler emits a stale dialogue runtime"
        report["emitted_runtime_sha256"] = digest(emitted)
        shutil.copyfile(HERE / "runtime.lua", build / "runtime_benchmark.lua")
        shutil.copyfile(HERE / "ownership.lua", build / "ownership.lua")
        shutil.copyfile(build / "main.lua", build / "compatibility.lua")
        (build / "run.lua").write_text('print("RUNTIME\\t" .. _VERSION .. "\\t" .. tostring(jit and jit.version) .. "\\t" .. tostring(jit and jit.status()))\ndofile("compatibility.lua")\ndofile("ownership.lua")\nrequire("runtime_benchmark").run()\n')
        for interpreter in ["lua", "luajit"]:
            binary = shutil.which(interpreter)
            if not binary:
                report["unavailable"].append(interpreter)
                continue
            log = run([binary, "run.lua"], build)
            (output / f"{interpreter}.log").write_text(log)
            report["runs"].append({"host": interpreter, "binary": binary, "version": run([binary, "-v"], build).strip(), "binary_sha256": digest(Path(binary)),
                                   "runtime": next(line for line in log.splitlines() if line.startswith("RUNTIME\t")), "metrics": metrics(log)})
        if args.love.is_file():
            (build / "conf.lua").write_text('function love.conf(t)\n  t.window.width = 960\n  t.window.height = 600\n  t.window.vsync = 0\n  t.modules.audio = false\nend\n')
            shutil.copyfile(HERE / "love.lua", build / "main.lua")
            for mode in ["off", "on"]:
                destination = output / f"love-jit-{mode}"
                destination.mkdir()
                env = dict(os.environ, TUA_DIALOGUE_BASELINE_OUT=str(destination), TUA_DIALOGUE_BASELINE_JIT=mode)
                log = run([str(args.love), str(build)], build, env=env, timeout=60)
                assert "RENDER_OK\t240" in log, log
                assert (destination / "render.png").is_file()
                (destination / "runtime.log").write_text(log)
                runtime = next(line for line in log.splitlines() if line.startswith("RUNTIME\t"))
                assert runtime.endswith("true" if mode == "on" else "false"), runtime
                report["runs"].append({"host": f"love-jit-{mode}", "binary": str(args.love), "binary_sha256": digest(args.love),
                                       "runtime": runtime, "love_version": next(line.split("\t")[1] for line in log.splitlines() if line.startswith("LOVE\t")), "metrics": metrics(log)})
        else:
            report["unavailable"].append(str(args.love))
        node = shutil.which("node")
        if node:
            run([node, str(HERE / "editor.js"), str(args.tua), str(project), str(output / "editor.json")], ROOT)
            report["editor"] = json.loads((output / "editor.json").read_text())
            report["editor"]["node_version"] = run([node, "--version"], ROOT).strip()
        else:
            report["unavailable"].append("node/editor workload")
    (output / "baseline.json").write_text(json.dumps(report, indent=2) + "\n")
    print(output / "baseline.json")


if __name__ == "__main__":
    main()
