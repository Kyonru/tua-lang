"use strict";

const childProcess = require("child_process");
const fs = require("fs");
const path = require("path");

const repoRoot = path.resolve(__dirname, "..");
const files = childProcess
  .execFileSync("git", ["ls-files", "--", "*.json"], {
    cwd: repoRoot,
    encoding: "utf8",
  })
  .split(/\r?\n/)
  .filter(Boolean);

for (const file of files) {
  const fullPath = path.join(repoRoot, file);

  try {
    JSON.parse(fs.readFileSync(fullPath, "utf8"));
  } catch (error) {
    console.error(`${file}: ${error.message}`);
    process.exitCode = 1;
  }
}

if (!process.exitCode) {
  console.log(`validated ${files.length} tracked JSON files`);
}
