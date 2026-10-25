#!/usr/bin/env node
"use strict";
const path = require("node:path");
const fs = require("node:fs");
if (process.argv.includes("--version")) {
  console.log("LOVE 11.5 (test fixture)");
} else {
  console.log("[warn] [combat] low health");
  console.log("[warn] [combat] low health");
  console.error(`[error] ${path.join(process.cwd(), "src/intelligence.tua")}:7:1 fixture failure`);
  if (fs.existsSync(path.join(process.cwd(), ".tua-test-keep-running"))) {
    fs.writeFileSync(path.join(process.cwd(), ".tua-test-process.pid"), String(process.pid));
    setInterval(() => {}, 1000);
  } else setTimeout(() => { process.exitCode = 1; }, 500);
}
