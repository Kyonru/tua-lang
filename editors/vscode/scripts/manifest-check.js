"use strict";

const fs = require("fs");
const path = require("path");

const packagePath = path.resolve(__dirname, "..", "package.json");
const manifest = JSON.parse(fs.readFileSync(packagePath, "utf8"));
const commands = new Set((manifest.contributes?.commands || []).map((command) => command.command));
const expected = [
  "tua.restartServer",
  "tua.output.focus",
  "tua.openSettings",
  "tua.selectServer",
  "tua.copyDebugInfo",
  "tua.copyCacheStats",
  "tua.saveCacheStats",
  "tua.saveCpuProfile",
  "tua.saveHeapSummary",
  "tua.reportIssue",
];
const missing = expected.filter((command) => !commands.has(command));

if (missing.length > 0) {
  console.error(`Missing contributed commands: ${missing.join(", ")}`);
  process.exit(1);
}
