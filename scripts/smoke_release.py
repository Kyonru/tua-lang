#!/usr/bin/env python3
"""Exercise server bytes extracted from the native archive and VSIX."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import zipfile


def smoke(binary: Path, version: str, project: Path) -> None:
    binary.chmod(0o755)
    result = subprocess.run([str(binary), "--version"], check=True, capture_output=True, text=True, timeout=15)
    if result.stdout.strip() != f"tua {version}":
        raise ValueError("packaged server version mismatch")
    project.mkdir()
    subprocess.run([str(binary), "init"], cwd=project, check=True, timeout=15)
    (project / "src/main.tua").write_text("local count: number = 1\nprint(count)\n", encoding="utf-8")
    for command in ["check", "build"]:
        subprocess.run([str(binary), command], cwd=project, check=True, timeout=30)
    if not (project / "build/main.lua").is_file():
        raise ValueError("packaged server did not produce Lua")
    (project / "src/main.tua").write_text('local count: number = "wrong"\n', encoding="utf-8")
    result = subprocess.run([str(binary), "check"], cwd=project, capture_output=True, timeout=30)
    if result.returncode == 0 or b"TL1003" not in result.stderr:
        raise ValueError("packaged server failed to diagnose an invalid program")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", type=Path, required=True)
    parser.add_argument("--vsix", type=Path, required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--target", required=True)
    args = parser.parse_args()
    executable = "tua.exe" if os.name == "nt" else "tua"
    member = f"tua-{args.version}-{args.target}/{executable}"
    if args.archive.suffix == ".zip":
        with zipfile.ZipFile(args.archive) as archive:
            if archive.testzip() is not None:
                raise ValueError("corrupt server archive")
            standalone = archive.read(member)
    else:
        with tarfile.open(args.archive) as archive:
            info = archive.getmember(member)
            if not info.isfile():
                raise ValueError("server archive member is not a regular file")
            standalone = archive.extractfile(info).read()
    with zipfile.ZipFile(args.vsix) as archive:
        if archive.testzip() is not None:
            raise ValueError("corrupt VSIX")
        manifest = json.loads(archive.read("extension/package.json"))
        if manifest["version"] != args.version:
            raise ValueError("VSIX version mismatch")
        archive.getinfo("extension/" + manifest["main"].removeprefix("./"))
        bundled = archive.read("extension/bin/" + executable)
    if standalone != bundled:
        raise ValueError("standalone and bundled server differ")
    # Extract only known member bytes, never arbitrary archive paths or links.
    with tempfile.TemporaryDirectory(prefix="tua-artifact-smoke-") as temp:
        root = Path(temp)
        for name, contents in [("standalone", standalone), ("vsix", bundled)]:
            folder = root / name
            folder.mkdir()
            binary = folder / executable
            binary.write_bytes(contents)
            smoke(binary, args.version, folder / "project")
    print("Packaged standalone and VSIX servers passed")


if __name__ == "__main__":
    main()
