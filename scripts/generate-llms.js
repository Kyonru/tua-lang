"use strict";

const fs = require("fs");
const path = require("path");

const root = path.resolve(__dirname, "..");
const sourcePath = path.join(root, "docs", "llms.txt");
const outputPath = path.join(root, "llms.txt");
const expected = fs.readFileSync(sourcePath, "utf8");

if (process.argv.includes("--check")) {
  const actual = fs.existsSync(outputPath)
    ? fs.readFileSync(outputPath, "utf8")
    : "";
  if (actual !== expected) {
    console.error("llms.txt is stale; run `make generate-llms`");
    process.exit(1);
  }
  console.log("llms.txt is synchronized");
} else {
  fs.writeFileSync(outputPath, expected);
  console.log("generated llms.txt");
}
