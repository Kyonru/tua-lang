"""Collect the modified MPL formatter sources for binary distributions."""
from pathlib import Path
import argparse


def formatter_source_entries(repo_root: Path) -> list[tuple[str, bytes, int]]:
    vendor = repo_root / "vendor"
    files = [vendor / "FORMATTER.md"]
    for name in ("tua-full-moon", "tua-stylua"):
        package = vendor / name
        for required in ("Cargo.toml", "LICENSE.md", "src/lib.rs"):
            if not (package / required).is_file():
                raise FileNotFoundError(f"missing formatter source: {package / required}")
        files.extend(path for path in package.rglob("*")
                     if path.is_file()
                     and not {"target", ".git"}.intersection(path.relative_to(package).parts)
                     and path.name not in {"Cargo.lock", ".cargo_vcs_info.json", ".cargo-checksum.json"}
                     and not path.name.endswith(".snap.new"))
    return [(f"third-party/formatter/{path.relative_to(vendor).as_posix()}", path.read_bytes(), 0o644)
            for path in sorted(files)]


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("destination", type=Path)
    args = parser.parse_args()
    repo_root = Path(__file__).resolve().parent.parent
    for name, content, _ in formatter_source_entries(repo_root):
        output = args.destination / name
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_bytes(content)


if __name__ == "__main__":
    main()
