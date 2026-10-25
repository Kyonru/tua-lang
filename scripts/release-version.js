"use strict";

const childProcess = require("child_process");
const fs = require("fs");
const os = require("os");
const path = require("path");

const { semanticVersion, workspaceVersion } = require("./sync-version");

const repoRoot = path.resolve(__dirname, "..");
const releaseFiles = [
  "Cargo.toml",
  "Cargo.lock",
  "fuzz/Cargo.lock",
  "CHANGELOG.md",
  "docs/releases.md",
  "editors/vscode/package.json",
  "editors/vscode/package-lock.json",
];

function versionParts(version) {
  semanticVersion(version);
  const withoutBuild = version.split("+", 1)[0];
  const prereleaseStart = withoutBuild.indexOf("-");
  const core = prereleaseStart === -1 ? withoutBuild : withoutBuild.slice(0, prereleaseStart);
  const prerelease = prereleaseStart === -1 ? undefined : withoutBuild.slice(prereleaseStart + 1);

  return {
    core: core.split(".").map(Number),
    prerelease: prerelease?.split(".") ?? [],
  };
}

function compareVersions(left, right) {
  const leftParts = versionParts(left);
  const rightParts = versionParts(right);

  for (let index = 0; index < leftParts.core.length; index += 1) {
    if (leftParts.core[index] !== rightParts.core[index]) {
      return Math.sign(leftParts.core[index] - rightParts.core[index]);
    }
  }

  if (leftParts.prerelease.length === 0 || rightParts.prerelease.length === 0) {
    return Math.sign(rightParts.prerelease.length - leftParts.prerelease.length);
  }

  const length = Math.max(leftParts.prerelease.length, rightParts.prerelease.length);
  for (let index = 0; index < length; index += 1) {
    const leftPart = leftParts.prerelease[index];
    const rightPart = rightParts.prerelease[index];
    if (leftPart === undefined || rightPart === undefined) {
      return leftPart === undefined ? -1 : 1;
    }
    if (leftPart === rightPart) {
      continue;
    }

    const leftNumber = /^\d+$/.test(leftPart) ? Number(leftPart) : undefined;
    const rightNumber = /^\d+$/.test(rightPart) ? Number(rightPart) : undefined;
    if (leftNumber !== undefined && rightNumber !== undefined) {
      return Math.sign(leftNumber - rightNumber);
    }
    if (leftNumber !== undefined || rightNumber !== undefined) {
      return leftNumber !== undefined ? -1 : 1;
    }
    return leftPart.localeCompare(rightPart);
  }

  return 0;
}

function releasedChangelog(contents, currentVersion, nextVersion, date) {
  if (contents.includes(`## [${nextVersion}]`)) {
    throw new Error(`CHANGELOG.md already contains a ${nextVersion} release section`);
  }

  const unreleased = contents.match(/^## \[Unreleased]\s*\n([\s\S]*?)(?=^## \[)/m);
  if (!unreleased) {
    throw new Error("CHANGELOG.md must contain [Unreleased] before a versioned section");
  }

  const notes = unreleased[1].trim();
  if (!notes) {
    throw new Error("CHANGELOG.md [Unreleased] must contain release notes before tagging");
  }

  const currentLink =
    `[Unreleased]: https://github.com/Kyonru/tua-lang/compare/v${currentVersion}...HEAD`;
  if (!contents.includes(currentLink)) {
    throw new Error(`CHANGELOG.md [Unreleased] link must start at v${currentVersion}`);
  }

  const released = contents.replace(
    unreleased[0],
    `## [Unreleased]\n\n## [${nextVersion}] - ${date}\n\n${notes}\n\n`,
  );

  return released.replace(
    currentLink,
    `[Unreleased]: https://github.com/Kyonru/tua-lang/compare/v${nextVersion}...HEAD\n` +
      `[${nextVersion}]: https://github.com/Kyonru/tua-lang/compare/` +
      `v${currentVersion}...v${nextVersion}`,
  );
}

function synchronizedReleaseDocs(contents, currentVersion, nextVersion) {
  const currentName = `tua-${currentVersion}-`;
  if (!contents.includes(currentName)) {
    throw new Error(`docs/releases.md contains no ${currentVersion} artifact example`);
  }
  return contents.replaceAll(currentName, `tua-${nextVersion}-`);
}

function run(command, args, options = {}) {
  const result = childProcess.spawnSync(command, args, {
    cwd: repoRoot,
    encoding: "utf8",
    stdio: options.capture ? "pipe" : "inherit",
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    const output = (result.stderr || result.stdout || "").trim();
    const detail = options.capture && output ? `: ${output}` : "";
    throw new Error(`${command} ${args.join(" ")} failed${detail}`);
  }
  const stdout = result.stdout || "";
  return options.preserveLeading ? stdout.trimEnd() : stdout.trim();
}

function assertCleanWorktree() {
  const status = run("git", ["status", "--porcelain=v1", "--untracked-files=all"], {
    capture: true,
    preserveLeading: true,
  });
  if (status) {
    throw new Error("release versioning requires a clean worktree; commit release notes first");
  }
}

function assertTagMissing(tag) {
  const result = childProcess.spawnSync(
    "git",
    ["show-ref", "--verify", "--quiet", `refs/tags/${tag}`],
    { cwd: repoRoot },
  );
  if (result.error) {
    throw result.error;
  }
  if (result.status === 0) {
    throw new Error(`tag ${tag} already exists`);
  }
  if (result.status !== 1) {
    throw new Error(`could not check whether tag ${tag} exists`);
  }
}

function changedFiles() {
  const status = run("git", ["status", "--porcelain=v1", "--untracked-files=all"], {
    capture: true,
    preserveLeading: true,
  });
  return status ? status.split("\n").map((line) => line.slice(3)) : [];
}

function assertExpectedChanges() {
  const expected = new Set(releaseFiles);
  const unexpected = changedFiles().filter((file) => !expected.has(file));
  if (unexpected.length > 0) {
    throw new Error(`release checks changed unexpected files: ${unexpected.join(", ")}`);
  }
}

function main(args = process.argv.slice(2)) {
  if (args.length !== 1) {
    throw new Error("usage: node scripts/release-version.js <version>");
  }

  const nextVersion = args[0];
  versionParts(nextVersion);
  assertCleanWorktree();

  const cargoPath = path.join(repoRoot, "Cargo.toml");
  const currentVersion = workspaceVersion(fs.readFileSync(cargoPath, "utf8"));
  if (compareVersions(nextVersion, currentVersion) <= 0) {
    throw new Error(`release version ${nextVersion} must be newer than ${currentVersion}`);
  }

  const tag = `v${nextVersion}`;
  assertTagMissing(tag);

  const files = Object.fromEntries(
    releaseFiles.map((relative) => {
      const absolute = path.join(repoRoot, relative);
      return [relative, fs.readFileSync(absolute, "utf8")];
    }),
  );
  const date = new Date().toISOString().slice(0, 10);

  const generated = {
    "CHANGELOG.md": releasedChangelog(
      files["CHANGELOG.md"],
      currentVersion,
      nextVersion,
      date,
    ),
    "docs/releases.md": synchronizedReleaseDocs(
      files["docs/releases.md"],
      currentVersion,
      nextVersion,
    ),
  };

  for (const [relative, contents] of Object.entries(generated)) {
    fs.writeFileSync(path.join(repoRoot, relative), contents, "utf8");
  }

  try {
    run(process.execPath, ["scripts/sync-version.js", "--set", nextVersion]);
    assertExpectedChanges();
    run("make", ["ci-check"]);

    const temp = fs.mkdtempSync(path.join(os.tmpdir(), "tua-release-version-"));
    try {
      run("python3", [
        "scripts/package_release.py",
        "notes",
        "--changelog",
        "CHANGELOG.md",
        "--version",
        nextVersion,
        "--output",
        path.join(temp, "release-notes.md"),
      ]);
    } finally {
      fs.rmSync(temp, { recursive: true, force: true });
    }
    assertExpectedChanges();
  } catch (error) {
    for (const [relative, contents] of Object.entries(files)) {
      fs.writeFileSync(path.join(repoRoot, relative), contents, "utf8");
    }
    throw error;
  }

  run("git", ["add", "--", ...releaseFiles]);
  run("git", ["commit", "-m", `release: ${tag}`]);
  run("git", ["tag", "-a", tag, "-m", `Tua ${nextVersion}`]);

  console.log(`created release commit and annotated tag ${tag}`);
  console.log(`push when ready: git push origin HEAD ${tag}`);
}

if (require.main === module) {
  try {
    main();
  } catch (error) {
    console.error(`release version failed: ${error.message}`);
    process.exitCode = 1;
  }
}

module.exports = {
  compareVersions,
  releasedChangelog,
  synchronizedReleaseDocs,
  versionParts,
};
