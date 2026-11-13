"""Run one built LOVE fixture with a firm timeout and print its actual output."""

import os
import pathlib
import subprocess
import sys

project = pathlib.Path(sys.argv[1]).resolve()
env = os.environ.copy()
env["TUA_ALCHEMY_SMOKE"] = "1"
try:
    completed = subprocess.run(
        ["/Applications/love.app/Contents/MacOS/love", "build"],
        cwd=project,
        env=env,
        capture_output=True,
        text=True,
        timeout=8,
        check=False,
    )
except subprocess.TimeoutExpired as error:
    print(f"timeout after {error.timeout}s")
    if error.stdout:
        print(error.stdout.decode(errors="replace"))
    if error.stderr:
        print(error.stderr.decode(errors="replace"))
    raise SystemExit(124)

print(f"exit: {completed.returncode}")
print(f"stdout: {completed.stdout.rstrip()}")
print(f"stderr: {completed.stderr.rstrip()}")
raise SystemExit(completed.returncode)
