"use strict";

const childProcess = require("child_process");
const fs = require("fs");
const path = require("path");

const extensionRoot = path.resolve(__dirname, "..");
const roots = ["src", "scripts", "test"];
const files = roots
  .flatMap((root) => javascriptFiles(path.join(extensionRoot, root)))
  .sort((left, right) => left.localeCompare(right));

for (const file of files) {
  const result = childProcess.spawnSync(process.execPath, ["--check", file], {
    stdio: "inherit",
  });

  if (result.error) {
    throw result.error;
  }

  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

console.log(`syntax-checked ${files.length} extension JavaScript files`);

function javascriptFiles(root) {
  return fs.readdirSync(root, { withFileTypes: true }).flatMap((entry) => {
    const fullPath = path.join(root, entry.name);

    if (entry.isDirectory()) {
      return javascriptFiles(fullPath);
    }

    return entry.isFile() && entry.name.endsWith(".js") ? [fullPath] : [];
  });
}
