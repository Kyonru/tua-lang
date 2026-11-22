#!/usr/bin/env python3
"""Build and run an opt-in native LÖVE scenario, optionally recording frames/video."""

import argparse
import hashlib
import json
import math
import os
import re
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import tomllib
import uuid


RUNTIME = Path(__file__).with_name("love_replay") / "runtime.lua"


def executable(value):
    found = shutil.which(value)
    if not found:
        raise ValueError(f"Executable unavailable: {value}")
    return str(Path(found).resolve())


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def built_root(project, config):
    result = (project / config.get("out", "build")).resolve()
    if result == project or not result.is_relative_to(project):
        raise ValueError("Replay requires a build output directory inside the project")
    return result


def cleanup_save(log, identity):
    for line in log.splitlines():
        if line.startswith("TUA_REPLAY_SAVE\t"):
            path = Path(line.split("\t", 1)[1])
            if path.is_absolute() and path.name == identity and not path.is_symlink():
                if path.is_dir():
                    shutil.rmtree(path)


def run(args):
    if not 1 <= args.frames <= 18000 or not 1 <= args.fps <= 240:
        raise ValueError("frames must be 1–18000 and fps 1–240")
    if not math.isfinite(args.timeout) or args.timeout <= 0:
        raise ValueError("timeout must be finite and positive")
    if not 0 <= args.seed <= 2147483647:
        raise ValueError("seed must be a nonnegative 31-bit integer")
    tua, love = executable(args.tua), executable(args.love)
    ffmpeg = executable(args.ffmpeg) if args.video else None
    project = args.project.resolve()
    config = tomllib.loads((project / "tua.toml").read_text())
    emitted = built_root(project, config)
    output = args.output.resolve()
    if output.exists():
        raise ValueError("Output must be a new directory; existing evidence is never overwritten")
    if output == emitted or output.is_relative_to(emitted):
        raise ValueError("Evidence must be outside compiler output")
    scenario = args.scenario.resolve() if args.scenario else None
    if scenario and (not scenario.is_file() or scenario.suffix != ".lua"):
        raise ValueError("External scenario must be an authored .lua file")
    if args.scenario_module and not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*(?:\.[A-Za-z_][A-Za-z0-9_]*)*", args.scenario_module):
        raise ValueError("Scenario module must be a dotted Lua module name")
    output.mkdir(parents=True)
    identity = "tua-replay-" + uuid.uuid4().hex
    capture = args.capture or args.video
    metadata = {
        "status": "running", "compiler": tua, "compiler_sha256": digest(Path(tua)),
        "seed": args.seed, "frames": args.frames, "fps": args.fps,
        "dt": 1 / args.fps, "gc_policy": "project-controlled / unknown",
        "timing": "fixed-step functional replay; not a CPU, FPS or latency benchmark",
        "audio_captured": False, "identity": identity,
        "scenario": str(scenario) if scenario else args.scenario_module,
        "runner_sha256": digest(Path(__file__)),
        "runtime_wrapper_sha256": digest(RUNTIME),
        "love_executable": love, "love_sha256": digest(Path(love)),
    }
    log = ""
    try:
        version = subprocess.run([tua, "--version", "--json"], capture_output=True, text=True, check=True)
        metadata["compiler_provenance"] = json.loads(version.stdout)
        revision = subprocess.run(["git", "-C", str(project), "rev-parse", "HEAD"], capture_output=True, text=True)
        metadata["revision"] = revision.stdout.strip() if revision.returncode == 0 else None
        dirty = subprocess.run(["git", "-C", str(project), "status", "--porcelain"], capture_output=True, text=True)
        metadata["worktree_dirty"] = bool(dirty.stdout) if dirty.returncode == 0 else None
        with (output / "build.log").open("w") as build_log:
            subprocess.run([tua, "build"], cwd=project, stdout=build_log, stderr=subprocess.STDOUT, check=True)
        with tempfile.TemporaryDirectory(prefix="tua-replay-") as directory:
            game = Path(directory) / "game"
            shutil.copytree(emitted, game)
            for name in ("__tua_replay_game.lua", "__tua_replay_runtime.lua", "__tua_replay_scenario.lua", "__tua_replay_conf.lua"):
                if (game / name).exists():
                    raise ValueError(f"Reserved replay filename already exists: {name}")
            (game / "main.lua").rename(game / "__tua_replay_game.lua")
            shutil.copy2(RUNTIME, game / "__tua_replay_runtime.lua")
            (game / "main.lua").write_text('require("__tua_replay_runtime")\n')
            conf = game / "conf.lua"
            prefix = ""
            if conf.exists():
                conf.rename(game / "__tua_replay_conf.lua")
                prefix = 'require("__tua_replay_conf")\n'
            isolation = ('io.stdout:setvbuf("no")\n'
                         f'love.filesystem.setIdentity("{identity}")\n'
                         'local setIdentity = love.filesystem.setIdentity\n'
                         f'love.filesystem.setIdentity = function() return setIdentity("{identity}") end\n'
                         'print("TUA_REPLAY_SAVE\\t" .. love.filesystem.getSaveDirectory())\n')
            conf.write_text(isolation + prefix + 'local original = love.conf\nfunction love.conf(t)\n'
                            '  if original then original(t) end\n'
                            f'  t.identity = "{identity}"\nend\n')
            module = args.scenario_module
            if scenario:
                shutil.copy2(scenario, game / "__tua_replay_scenario.lua")
                module = "__tua_replay_scenario"
                metadata["scenario_sha256"] = digest(scenario)
            else:
                metadata["scenario_sha256"] = digest(game / (module.replace(".", "/") + ".lua"))
            if capture:
                (output / "frames").mkdir()
            environment = dict(os.environ, TUA_REPLAY_IDENTITY=identity,
                               TUA_REPLAY_OUTPUT=str(output), TUA_REPLAY_FRAMES=str(args.frames),
                               TUA_REPLAY_FPS=str(args.fps), TUA_REPLAY_SEED=str(args.seed),
                               TUA_REPLAY_CAPTURE="1" if capture else "0", TUA_REPLAY_SCENARIO=module)
            command = [love, str(game)]
            if sys.platform == "darwin":
                command.extend(["-ApplePersistenceIgnoreState", "YES"])
            with (output / "runtime.log").open("w") as runtime_log:
                subprocess.run(command, env=environment, stdout=runtime_log,
                               stderr=subprocess.STDOUT, timeout=args.timeout, check=True)
            log = (output / "runtime.log").read_text()
            if f"TUA_REPLAY_OK\t{args.frames}" not in log.splitlines():
                raise ValueError("Runtime exited without completing scenario verification")
            if capture and len(list((output / "frames").glob("*.png"))) != args.frames:
                raise ValueError("Incomplete frame capture")
            if ffmpeg:
                subprocess.run([ffmpeg, "-hide_banner", "-loglevel", "error", "-framerate", str(args.fps),
                                "-start_number", "1", "-i", str(output / "frames/%06d.png"),
                                "-an", "-vf", "pad=ceil(iw/2)*2:ceil(ih/2)*2", "-c:v", "libx264",
                                "-pix_fmt", "yuv420p", "-crf", "20", "-movflags", "+faststart",
                                str(output / "replay.mp4")], check=True)
            metadata["status"] = "passed"
    except Exception as error:
        metadata.update(status="failed", error=str(error))
        raise
    finally:
        if (output / "runtime.log").exists():
            log = (output / "runtime.log").read_text(errors="replace")
        for line in log.splitlines():
            if line.startswith("TUA_REPLAY_RUNTIME\t"):
                fields = line.split("\t")
                if len(fields) == 4:
                    metadata.update(love_version=fields[1], lua_version=fields[2],
                                    jit_enabled=fields[3] == "true")
        try:
            cleanup_save(log, identity)
        except OSError as error:
            metadata["cleanup_error"] = str(error)
        (output / "provenance.json").write_text(json.dumps(metadata, indent=2) + "\n")
    return output


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--project", type=Path, required=True)
    parser.add_argument("--tua", default="tua")
    parser.add_argument("--love", default="love")
    parser.add_argument("--output", type=Path, required=True)
    selection = parser.add_mutually_exclusive_group(required=True)
    selection.add_argument("--scenario", type=Path)
    selection.add_argument("--scenario-module", help="typed scenario module emitted by this project")
    parser.add_argument("--frames", type=int, default=180)
    parser.add_argument("--fps", type=int, default=30)
    parser.add_argument("--seed", type=int, default=1)
    parser.add_argument("--timeout", type=float, default=60)
    parser.add_argument("--capture", action="store_true")
    parser.add_argument("--video", action="store_true")
    parser.add_argument("--ffmpeg", default="ffmpeg")
    print(run(parser.parse_args()))


if __name__ == "__main__":
    main()
