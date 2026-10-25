"use strict";

const fs = require("fs");
const path = require("path");

const repoRoot = path.resolve(__dirname, "..");
const cargoPath = path.join(repoRoot, "Cargo.toml");
const cargoLockPath = path.join(repoRoot, "Cargo.lock");
const fuzzCargoLockPath = path.join(repoRoot, "fuzz", "Cargo.lock");
const packagePath = path.join(repoRoot, "editors", "vscode", "package.json");
const packageLockPath = path.join(repoRoot, "editors", "vscode", "package-lock.json");
const semanticVersionPattern = /^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/;
const workspacePackages = new Set(["tua-cli", "tua-core", "tua-lsp"]);

function semanticVersion(version) {
  if (!semanticVersionPattern.test(version)) {
    throw new Error(`version must be a semantic version, got '${version}'`);
  }

  return version;
}

function workspaceVersionRange(contents) {
  let inWorkspacePackage = false;
  let offset = 0;

  for (const lineWithEnding of contents.match(/[^\r\n]*(?:\r\n|\n|$)/g) || []) {
    const line = lineWithEnding.replace(/\r?\n$/, "");
    const section = line.match(/^\s*\[([^\]]+)]\s*$/)?.[1];
    if (section) {
      inWorkspacePackage = section === "workspace.package";
    } else if (inWorkspacePackage) {
      const match = line.match(/^(\s*version\s*=\s*")([^"]+)("\s*)$/);
      if (match) {
        const start = offset + match[1].length;

        return { start, end: start + match[2].length, version: match[2] };
      }
    }

    offset += lineWithEnding.length;
  }

  throw new Error("Cargo.toml must define a semantic version in [workspace.package]");
}

function workspaceVersion(contents) {
  const version = workspaceVersionRange(contents).version;

  try {
    return semanticVersion(version);
  } catch {
    throw new Error("Cargo.toml must define a semantic version in [workspace.package]");
  }
}

function updatedWorkspaceVersion(contents, version) {
  semanticVersion(version);
  const range = workspaceVersionRange(contents);

  return `${contents.slice(0, range.start)}${version}${contents.slice(range.end)}`;
}

function synchronizedCargoLock(contents, version, packages = workspacePackages) {
  semanticVersion(version);
  const found = new Set();
  const generated = contents
    .split(/(?=^\[\[package]]\s*$)/m)
    .map((block) => {
      const name = block.match(/^name = "([^"]+)"\s*$/m)?.[1];
      if (!packages.has(name)) {
        return block;
      }
      if (found.has(name)) {
        throw new Error(`Cargo.lock contains duplicate workspace package '${name}'`);
      }
      if (!/^version = "[^"]+"[ \t]*$/m.test(block)) {
        throw new Error(`Cargo.lock package '${name}' is missing its version`);
      }

      found.add(name);
      return block.replace(/^version = "[^"]+"[ \t]*$/m, `version = "${version}"`);
    })
    .join("");
  const missing = [...packages].filter((name) => !found.has(name));

  if (missing.length > 0) {
    throw new Error(`Cargo.lock is missing workspace packages: ${missing.join(", ")}`);
  }

  return generated;
}

function synchronizedJson(contents, version, lockfile = false) {
  const value = JSON.parse(contents);

  value.version = version;
  if (lockfile) {
    if (!value.packages?.[""]) {
      throw new Error("editors/vscode/package-lock.json is missing its root package entry");
    }
    value.packages[""].version = version;
  }

  return `${JSON.stringify(value, null, 2)}\n`;
}

function synchronizeFile(file, generated, checkOnly) {
  const current = fs.readFileSync(file, "utf8");

  if (current === generated) {
    return true;
  }
  if (checkOnly) {
    console.error(`${path.relative(repoRoot, file)} does not use the workspace version`);
    return false;
  }

  fs.writeFileSync(file, generated, "utf8");
  return true;
}

function main(args = process.argv.slice(2)) {
  let mode = "sync";
  let requestedVersion;

  if (args.length === 1 && ["--check", "--print"].includes(args[0])) {
    mode = args[0].slice(2);
  } else if (args.length === 2 && args[0] === "--set") {
    mode = "set";
    requestedVersion = semanticVersion(args[1]);
  } else if (args.length > 0) {
    throw new Error("usage: node scripts/sync-version.js [--check | --print | --set VERSION]");
  }

  const cargo = fs.readFileSync(cargoPath, "utf8");
  const version = requestedVersion || workspaceVersion(cargo);
  if (mode === "print") {
    console.log(version);
    return;
  }

  const checkOnly = mode === "check";
  const cargoLock = synchronizedCargoLock(fs.readFileSync(cargoLockPath, "utf8"), version);
  const fuzzCargoLock = synchronizedCargoLock(
    fs.readFileSync(fuzzCargoLockPath, "utf8"), version, new Set(["tua-core", "tua-lsp"]),
  );
  const packageJson = synchronizedJson(fs.readFileSync(packagePath, "utf8"), version);
  const packageLock = synchronizedJson(fs.readFileSync(packageLockPath, "utf8"), version, true);
  const files = [
    ...(mode === "set" ? [[cargoPath, updatedWorkspaceVersion(cargo, version)]] : []),
    [cargoLockPath, cargoLock],
    [fuzzCargoLockPath, fuzzCargoLock],
    [packagePath, packageJson],
    [packageLockPath, packageLock],
  ];
  const synchronized = files
    .map(([file, generated]) => synchronizeFile(file, generated, checkOnly))
    .every(Boolean);

  if (!synchronized) {
    process.exitCode = 1;
    return;
  }

  console.log(
    checkOnly
      ? `package versions are synchronized (${version})`
      : mode === "set"
        ? `updated workspace version (${version})`
        : `synchronized package versions (${version})`,
  );
}

if (require.main === module) {
  main();
}

module.exports = {
  semanticVersion,
  synchronizedCargoLock,
  synchronizedJson,
  updatedWorkspaceVersion,
  workspaceVersion,
};
