"use strict";

const assert = require("assert");
const childProcess = require("child_process");
const fs = require("fs");
const os = require("os");
const path = require("path");
const test = require("node:test");

const {
  compareVersions,
  releasedChangelog,
  synchronizedReleaseDocs,
  versionParts,
} = require("./release-version");

function run(cwd, command, args) {
  return childProcess.execFileSync(command, args, { cwd, encoding: "utf8" }).trim();
}

test("release versions must increase using semantic version ordering", () => {
  assert(compareVersions("0.2.9", "0.2.8") > 0);
  assert(compareVersions("0.3.0-beta.1", "0.2.9") > 0);
  assert(compareVersions("0.3.0", "0.3.0-beta.1") > 0);
  assert(compareVersions("0.3.0-beta.2", "0.3.0-beta.10") < 0);
  assert.throws(() => versionParts("next"), /semantic version/);
});

test("release notes move from Unreleased into a dated version and update links", () => {
  const changelog = `# Changelog

## [Unreleased]

### Fixed

- Portable release automation.

## [0.2.8] - 2026-07-31

- Previous release.

[Unreleased]: https://github.com/Kyonru/tua-lang/compare/v0.2.8...HEAD
[0.2.8]: https://github.com/Kyonru/tua-lang/releases/tag/v0.2.8
`;
  const released = releasedChangelog(changelog, "0.2.8", "0.2.9", "2026-08-01");

  assert.match(released, /## \[Unreleased]\n\n## \[0\.2\.9] - 2026-08-01/);
  assert.match(released, /## \[0\.2\.9][\s\S]*Portable release automation/);
  assert.match(released, /compare\/v0\.2\.9\.\.\.HEAD/);
  assert.match(released, /compare\/v0\.2\.8\.\.\.v0\.2\.9/);
});

test("release notes and artifact examples are required", () => {
  const empty = `## [Unreleased]\n\n## [0.2.8]\n\nPrevious.\n`;

  assert.throws(
    () => releasedChangelog(empty, "0.2.8", "0.2.9", "2026-08-01"),
    /must contain release notes/,
  );
  assert.strictEqual(
    synchronizedReleaseDocs("tua-0.2.8-darwin-arm64.vsix", "0.2.8", "0.2.9"),
    "tua-0.2.9-darwin-arm64.vsix",
  );
  assert.throws(
    () => synchronizedReleaseDocs("no artifact", "0.2.8", "0.2.9"),
    /contains no 0.2.8 artifact/,
  );
});

test("release command commits synchronized metadata and creates an annotated tag", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "tua-release-version-test-"));

  try {
    fs.mkdirSync(path.join(root, "scripts"));
    fs.mkdirSync(path.join(root, "docs"));
    fs.mkdirSync(path.join(root, "fuzz"));
    fs.writeFileSync(path.join(root, "fuzz", "Cargo.lock"),
      'version = 4\n\n[[package]]\nname = "tua-core"\nversion = "0.2.8"\n\n[[package]]\nname = "tua-lsp"\nversion = "0.2.8"\n');
    fs.mkdirSync(path.join(root, "editors", "vscode"), { recursive: true });
    for (const script of ["release-version.js", "sync-version.js", "package_release.py"]) {
      fs.copyFileSync(path.join(__dirname, script), path.join(root, "scripts", script));
    }
    fs.writeFileSync(
      path.join(root, "Cargo.toml"),
      '[workspace]\nmembers = []\n\n[workspace.package]\nversion = "0.2.8"\n',
    );
    fs.writeFileSync(
      path.join(root, "Cargo.lock"),
      `version = 4

[[package]]
name = "tua-cli"
version = "0.2.8"

[[package]]
name = "tua-core"
version = "0.2.8"

[[package]]
name = "tua-lsp"
version = "0.2.8"
`,
    );
    fs.writeFileSync(
      path.join(root, "CHANGELOG.md"),
      `# Changelog

## [Unreleased]

### Added

- Automated releases.

## [0.2.8] - 2026-07-31

- Previous release.

[Unreleased]: https://github.com/Kyonru/tua-lang/compare/v0.2.8...HEAD
[0.2.8]: https://github.com/Kyonru/tua-lang/releases/tag/v0.2.8
`,
    );
    fs.writeFileSync(
      path.join(root, "docs", "releases.md"),
      "shasum -a 256 tua-0.2.8-darwin-arm64.vsix\n",
    );
    fs.writeFileSync(
      path.join(root, "editors", "vscode", "package.json"),
      '{"name":"tua","version":"0.2.8"}\n',
    );
    fs.writeFileSync(
      path.join(root, "editors", "vscode", "package-lock.json"),
      '{"name":"tua","version":"0.2.8","packages":{"":{"version":"0.2.8"}}}\n',
    );
    fs.writeFileSync(
      path.join(root, "Makefile"),
      "ci-check:\n\tnode scripts/sync-version.js --check\n",
    );

    run(root, "git", ["init", "-q"]);
    run(root, "git", ["config", "user.name", "Tua Release Test"]);
    run(root, "git", ["config", "user.email", "release-test@example.invalid"]);
    run(root, "git", ["config", "commit.gpgSign", "false"]);
    run(root, "git", ["config", "tag.gpgSign", "false"]);
    run(root, "git", ["add", "."]);
    run(root, "git", ["commit", "-qm", "baseline"]);
    run(root, "git", ["tag", "-a", "v0.2.8", "-m", "Tua 0.2.8"]);

    const output = run(root, process.execPath, ["scripts/release-version.js", "0.2.9"]);

    assert.match(output, /created release commit and annotated tag v0\.2\.9/);
    assert.strictEqual(run(root, "git", ["describe", "--tags", "--exact-match"]), "v0.2.9");
    assert.strictEqual(run(root, "git", ["log", "-1", "--pretty=%s"]), "release: v0.2.9");
    assert.strictEqual(run(root, "git", ["status", "--porcelain"]), "");
    assert.match(fs.readFileSync(path.join(root, "Cargo.toml"), "utf8"), /0\.2\.9/);
    assert.match(fs.readFileSync(path.join(root, "CHANGELOG.md"), "utf8"), /## \[0\.2\.9]/);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});
