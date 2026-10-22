"use strict";

const fs = require("node:fs");
const path = require("node:path");

const repositoryRoot = path.resolve(__dirname, "..", "..", "..");

function serverExecutableName(platform = process.platform) {
  return platform === "win32" ? "tua.exe" : "tua";
}

function serverPaths(root = repositoryRoot, platform = process.platform) {
  const executable = serverExecutableName(platform);
  return {
    source: path.join(root, "target", "release", executable),
    destination: path.join(root, "editors", "vscode", "bin", executable),
    destinationDirectory: path.join(root, "editors", "vscode", "bin"),
  };
}

function stageServer({ root = repositoryRoot, platform = process.platform } = {}) {
  const paths = serverPaths(root, platform);
  if (!fs.existsSync(paths.source)) {
    throw new Error(
      `Release Tua server not found at ${paths.source}. Build it with cargo build --release --package tua-cli first.`,
    );
  }

  fs.mkdirSync(paths.destinationDirectory, { recursive: true });
  fs.copyFileSync(paths.source, paths.destination);
  if (platform !== "win32") {
    fs.chmodSync(paths.destination, 0o755);
  }
  return paths.destination;
}

function cleanStagedServer({ root = repositoryRoot } = {}) {
  const { destinationDirectory } = serverPaths(root);
  fs.rmSync(destinationDirectory, { force: true, recursive: true });
  return destinationDirectory;
}

function main(args = process.argv.slice(2)) {
  if (args.length === 0) {
    console.log(`Staged Tua server at ${stageServer()}`);
    return;
  }
  if (args.length === 1 && args[0] === "--clean") {
    console.log(`Removed staged Tua server directory ${cleanStagedServer()}`);
    return;
  }
  throw new Error(`Usage: node ${path.basename(__filename)} [--clean]`);
}

if (require.main === module) {
  try {
    main();
  } catch (error) {
    console.error(error instanceof Error ? error.message : error);
    process.exitCode = 1;
  }
}

module.exports = {
  cleanStagedServer,
  serverExecutableName,
  serverPaths,
  stageServer,
};
