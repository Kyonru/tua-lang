#!/usr/bin/env python3
"""Create deterministic Tua release archives, checksums, and release notes."""

from __future__ import annotations

import argparse
import gzip
import hashlib
import io
import re
import tarfile
import zipfile
from pathlib import Path


WINDOWS_TARGETS = {"x86_64-pc-windows-msvc"}
VERSION_PATTERN = re.compile(r"^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$")


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(64 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def write_checksum(path: Path) -> Path:
    checksum_path = path.with_name(f"{path.name}.sha256")
    # Checksum manifests are consumed by Unix release jobs even when an entry
    # was produced on Windows. Write bytes so Windows cannot translate the
    # line ending to CRLF and break exact manifest matching downstream.
    checksum_path.write_bytes(f"{sha256(path)}  {path.name}\n".encode("utf-8"))
    return checksum_path


def release_entries(repo_root: Path, binary: Path, target: str) -> list[tuple[str, bytes, int]]:
    executable = "tua.exe" if target in WINDOWS_TARGETS else "tua"
    entries = [(executable, binary.read_bytes(), 0o755)]
    for name in ("README.md", "LICENSE", "CHANGELOG.md"):
        entries.append((name, (repo_root / name).read_bytes(), 0o644))
    return entries


def write_zip(path: Path, root_name: str, entries: list[tuple[str, bytes, int]]) -> None:
    with zipfile.ZipFile(path, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for name, contents, mode in entries:
            info = zipfile.ZipInfo(f"{root_name}/{name}", date_time=(1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.create_system = 3
            info.external_attr = mode << 16
            archive.writestr(info, contents)


def write_tar_gz(path: Path, root_name: str, entries: list[tuple[str, bytes, int]]) -> None:
    tar_buffer = io.BytesIO()
    with tarfile.open(fileobj=tar_buffer, mode="w", format=tarfile.PAX_FORMAT) as archive:
        for name, contents, mode in entries:
            info = tarfile.TarInfo(f"{root_name}/{name}")
            info.size = len(contents)
            info.mode = mode
            info.mtime = 0
            info.uid = 0
            info.gid = 0
            info.uname = ""
            info.gname = ""
            archive.addfile(info, io.BytesIO(contents))

    with path.open("wb") as output:
        with gzip.GzipFile(filename="", mode="wb", fileobj=output, mtime=0, compresslevel=9) as zipped:
            zipped.write(tar_buffer.getvalue())


def create_server_archive(
    repo_root: Path, binary: Path, version: str, target: str, output_dir: Path
) -> Path:
    if not VERSION_PATTERN.fullmatch(version):
        raise ValueError(f"invalid release version: {version}")
    if not binary.is_file():
        raise FileNotFoundError(f"server binary does not exist: {binary}")

    output_dir.mkdir(parents=True, exist_ok=True)
    root_name = f"tua-{version}-{target}"
    suffix = ".zip" if target in WINDOWS_TARGETS else ".tar.gz"
    archive_path = output_dir / f"{root_name}{suffix}"
    entries = release_entries(repo_root, binary, target)

    if suffix == ".zip":
        write_zip(archive_path, root_name, entries)
    else:
        write_tar_gz(archive_path, root_name, entries)

    write_checksum(archive_path)
    return archive_path


def changelog_notes(changelog: str, version: str) -> str:
    heading = re.compile(rf"^## \[{re.escape(version)}] - .+$", re.MULTILINE)
    match = heading.search(changelog)
    if not match:
        raise ValueError(f"CHANGELOG.md has no release section for {version}")

    next_heading = re.search(r"^## \[", changelog[match.end() :], re.MULTILINE)
    end = match.end() + next_heading.start() if next_heading else len(changelog)
    body = changelog[match.end() : end].strip()
    if not body:
        raise ValueError(f"CHANGELOG.md release section for {version} is empty")
    summary = re.search(
        r"^### Release summary[ \t]*(?:\n|$)(.*?)(?=^### |\Z)",
        body, re.MULTILINE | re.DOTALL,
    )
    if summary:
        body = summary.group(1).strip()
        if not body:
            raise ValueError(f"CHANGELOG.md release summary for {version} is empty")
        body = re.sub(r"^#### ", "## ", body, flags=re.MULTILINE)
        body += (
            f"\n\nSee the [full changelog](https://github.com/Kyonru/tua-lang/"
            f"blob/v{version}/CHANGELOG.md) for every change in this release."
            "\n\nDownload the CLI archive or platform-specific VS Code extension below. "
            "Checksums are available in `SHA256SUMS`."
        )
    return f"# Tua {version}\n\n{body}\n"


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    subcommands = result.add_subparsers(dest="command", required=True)

    archive = subcommands.add_parser("archive", help="create a server archive and checksum")
    archive.add_argument("--repo-root", type=Path, default=Path.cwd())
    archive.add_argument("--binary", type=Path, required=True)
    archive.add_argument("--version", required=True)
    archive.add_argument("--target", required=True)
    archive.add_argument("--output", type=Path, required=True)

    checksum = subcommands.add_parser("checksum", help="write SHA-256 files")
    checksum.add_argument("paths", type=Path, nargs="+")

    notes = subcommands.add_parser("notes", help="extract release notes from CHANGELOG.md")
    notes.add_argument("--changelog", type=Path, required=True)
    notes.add_argument("--version", required=True)
    notes.add_argument("--output", type=Path, required=True)

    return result


def main() -> None:
    args = parser().parse_args()
    if args.command == "archive":
        archive = create_server_archive(
            args.repo_root.resolve(),
            args.binary.resolve(),
            args.version,
            args.target,
            args.output.resolve(),
        )
        print(archive)
    elif args.command == "checksum":
        for path in args.paths:
            print(write_checksum(path.resolve()))
    else:
        notes = changelog_notes(args.changelog.read_text(encoding="utf-8"), args.version)
        args.output.write_text(notes, encoding="utf-8")
        print(args.output)


if __name__ == "__main__":
    main()
