"""Show that feature-examples-check validates inventory, not Tua syntax.

Run from the repository root: python3 internal/audits/feature-and-preview-2026-11-11/repro-feature-check-oracle.py
The temporary synthetic tree is removed on exit; repository files are never edited.
"""

from pathlib import Path
import json
import shutil
import subprocess
import tempfile

repo = Path(__file__).resolve().parents[3]
coverage = json.loads((repo / "examples/features/coverage.json").read_text())

with tempfile.TemporaryDirectory(prefix="tua-feature-check-oracle-") as name:
    root = Path(name)
    (root / "scripts").mkdir()
    (root / "examples/features/src").mkdir(parents=True)
    shutil.copy2(repo / "scripts/check-feature-examples.js", root / "scripts/check-feature-examples.js")
    shutil.copy2(repo / "zensical.toml", root / "zensical.toml")
    (root / "examples/features/coverage.json").write_text(json.dumps(coverage))

    modules = []
    for files in coverage.values():
        for relative in files:
            source = root / "examples/features" / relative
            source.write_text("-- Feature: deliberately invalid syntax\nlocal = ???\n")
            modules.append(source.stem)
    (root / "examples/features/src/main.tua").write_text(
        "".join(f'local {module} = require("{module}")\n' for module in modules)
    )

    check = subprocess.run(
        ["node", str(root / "scripts/check-feature-examples.js")],
        capture_output=True,
        text=True,
        check=False,
    )
    print(f"exit={check.returncode}")
    print(check.stdout.strip() or check.stderr.strip())
    assert check.returncode == 0, "the inventory check unexpectedly rejected invalid syntax"
