"""Test ShaderScan include invalidation in an isolated copy of the built game."""

import os
import pathlib
import selectors
import shutil
import subprocess
import sys
import tempfile

project = pathlib.Path(__file__).resolve().parent
if not (project / "build" / "main.lua").is_file():
    raise SystemExit("build the alchemy project first")

with tempfile.TemporaryDirectory(prefix="tua-alchemy-hot-") as directory:
    game = pathlib.Path(directory) / "build"
    shutil.copytree(project / "build", game)
    include = game / "assets" / "lib" / "pulse.glsl"
    environment = os.environ.copy()
    environment["TUA_ALCHEMY_HOT_SMOKE"] = "1"
    process = subprocess.Popen(
        ["/Applications/love.app/Contents/MacOS/love", str(game)],
        cwd=directory,
        env=environment,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    selector = selectors.DefaultSelector()
    selector.register(process.stdout, selectors.EVENT_READ)
    if not selector.select(timeout=5):
        process.kill()
        stdout, stderr = process.communicate()
        print("did not receive shader loaded signal within 5s")
        print(stdout)
        print(stderr, file=sys.stderr)
        raise SystemExit(124)
    first_line = process.stdout.readline()
    if "HOT: initial shader loaded" not in first_line:
        process.kill()
        stdout, stderr = process.communicate()
        print("unexpected first line:", first_line.rstrip())
        print(stdout)
        print(stderr, file=sys.stderr)
        raise SystemExit(1)
    include.write_text(include.read_text() + "\n// changed by hot reload smoke\n")
    try:
        stdout, stderr = process.communicate(timeout=8)
    except subprocess.TimeoutExpired:
        process.kill()
        stdout, stderr = process.communicate()
        print("timeout after 8s")
        print(stdout)
        print(stderr, file=sys.stderr)
        raise SystemExit(124)

    print(f"exit: {process.returncode}")
    print(f"stdout: {(first_line + stdout).rstrip()}")
    print(f"stderr: {stderr.rstrip()}")
    raise SystemExit(process.returncode)
