#!/usr/bin/env python3
"""Check complete documentation samples in disposable projects."""
from pathlib import Path
import re
import subprocess
import tempfile

repo = Path(__file__).resolve().parents[3]
binary = repo / "target/release/tua"
with tempfile.TemporaryDirectory(prefix="tua-doc-contracts-") as temporary:
    for name, indices in [("comments.md", [0, 1, 2, 3]), ("classes.md", [0])]:
        blocks = re.findall(r"```tua\n(.*?)\n```", (repo / "docs" / name).read_text(), re.S)
        for index in indices:
            project = Path(temporary) / (name + str(index))
            (project / "src").mkdir(parents=True)
            (project / "tua.toml").write_text('source = "src"\nout = "build"\n[diagnostics]\nrules = false\n')
            (project / "src/main.tua").write_text(blocks[index] + "\n")
            result = subprocess.run([str(binary), "check"], cwd=project, capture_output=True, text=True)
            print(f"{name} block {index}: exit {result.returncode}\n{result.stdout}{result.stderr}")
            result.check_returncode()
