import tempfile
import tarfile
import unittest
import zipfile
from pathlib import Path

from package_release import changelog_notes, create_server_archive, sha256


class PackageReleaseTests(unittest.TestCase):
    def test_archives_are_deterministic_and_include_release_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            for name in ("README.md", "LICENSE", "CHANGELOG.md"):
                (root / name).write_text(name, encoding="utf-8")
            binary = root / "tua.exe"
            binary.write_bytes(b"server")

            first = create_server_archive(
                root, binary, "1.2.3", "x86_64-pc-windows-msvc", root / "first"
            )
            second = create_server_archive(
                root, binary, "1.2.3", "x86_64-pc-windows-msvc", root / "second"
            )

            self.assertEqual(sha256(first), sha256(second))
            self.assertTrue(first.with_name(f"{first.name}.sha256").is_file())
            with zipfile.ZipFile(first) as archive:
                self.assertEqual(
                    sorted(archive.namelist()),
                    [
                        "tua-1.2.3-x86_64-pc-windows-msvc/CHANGELOG.md",
                        "tua-1.2.3-x86_64-pc-windows-msvc/LICENSE",
                        "tua-1.2.3-x86_64-pc-windows-msvc/README.md",
                        "tua-1.2.3-x86_64-pc-windows-msvc/tua.exe",
                    ],
                )

    def test_tar_archives_are_deterministic_and_executable(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            for name in ("README.md", "LICENSE", "CHANGELOG.md"):
                (root / name).write_text(name, encoding="utf-8")
            binary = root / "tua"
            binary.write_bytes(b"server")

            first = create_server_archive(
                root, binary, "1.2.3", "aarch64-apple-darwin", root / "first"
            )
            second = create_server_archive(
                root, binary, "1.2.3", "aarch64-apple-darwin", root / "second"
            )

            self.assertEqual(sha256(first), sha256(second))
            with tarfile.open(first, "r:gz") as archive:
                executable = archive.getmember("tua-1.2.3-aarch64-apple-darwin/tua")
                self.assertEqual(executable.mode, 0o755)
                self.assertEqual(executable.mtime, 0)

    def test_release_notes_are_extracted_by_exact_version(self) -> None:
        changelog = """# Changelog

## [Unreleased]

Future work.

## [1.2.3] - 2026-07-16

### Added

- Release behavior.

## [1.2.2] - 2026-07-01

- Previous behavior.
"""

        notes = changelog_notes(changelog, "1.2.3")

        self.assertIn("# Tua 1.2.3", notes)
        self.assertIn("Release behavior", notes)
        self.assertNotIn("Previous behavior", notes)


if __name__ == "__main__":
    unittest.main()
