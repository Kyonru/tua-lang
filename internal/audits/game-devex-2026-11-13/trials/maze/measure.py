"""Small-project CLI timings; never substitutes for visible editor latency."""

import json
import statistics
import subprocess
import time
from pathlib import Path

root = Path(__file__).resolve().parent
tua = Path(__file__).resolve().parents[5] / "target/debug/tua"
result = {}
for label, args in (
    ("check", ["check"]),
    ("format_check", ["fmt", "--check"]),
    ("build", ["build"]),
):
    samples = []
    for _ in range(5):
        start = time.perf_counter()
        run = subprocess.run([str(tua), *args], cwd=root, capture_output=True, text=True)
        samples.append(round((time.perf_counter() - start) * 1000, 1))
        if run.returncode:
            raise SystemExit(f"{label} failed: {run.stderr or run.stdout}")
    result[label] = {"samples_ms": samples, "median_ms": statistics.median(samples)}
print(json.dumps(result, indent=2))
