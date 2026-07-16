"use strict";

const fs = require("fs");
const path = require("path");

const repoRoot = path.resolve(__dirname, "..");
const cargoPath = path.join(repoRoot, "Cargo.toml");
const packagePath = path.join(repoRoot, "editors", "vscode", "package.json");
const packageLockPath = path.join(repoRoot, "editors", "vscode", "package-lock.json");

function workspaceVersion(contents) {
  let inWorkspacePackage = false;
  let version;

  for (const line of contents.split(/\r?\n/)) {
    const section = line.match(/^\s*\[([^\]]+)]\s*$/)?.[1];
    if (section) {
      inWorkspacePackage = section === "workspace.package";
      continue;
    }
    if (inWorkspacePackage) {
      version ||= line.match(/^\s*version\s*=\s*"([^"]+)"\s*$/)?.[1];
    }
  }

  if (!version || !/^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(version)) {
    throw new Error("Cargo.toml must define a semantic version in [workspace.package]");
  }

  return version;
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
  const known = new Set(["--check", "--print"]);
  const unknown = args.filter((argument) => !known.has(argument));
  if (unknown.length > 0 || (args.includes("--check") && args.includes("--print"))) {
    throw new Error("usage: node scripts/sync-version.js [--check | --print]");
  }

  const version = workspaceVersion(fs.readFileSync(cargoPath, "utf8"));
  if (args.includes("--print")) {
    console.log(version);
    return;
  }

  const checkOnly = args.includes("--check");
  const packageJson = synchronizedJson(fs.readFileSync(packagePath, "utf8"), version);
  const packageLock = synchronizedJson(fs.readFileSync(packageLockPath, "utf8"), version, true);
  const synchronized = [
    synchronizeFile(packagePath, packageJson, checkOnly),
    synchronizeFile(packageLockPath, packageLock, checkOnly),
  ].every(Boolean);

  if (!synchronized) {
    process.exitCode = 1;
    return;
  }

  console.log(
    checkOnly
      ? `package versions are synchronized (${version})`
      : `synchronized package versions (${version})`,
  );
}

if (require.main === module) {
  main();
}

module.exports = {
  synchronizedJson,
  workspaceVersion,
};
