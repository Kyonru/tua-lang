#!/usr/bin/env node
"use strict";
const path = require("node:path");
if (process.argv.includes("--version")) {
  console.log("LOVE 11.5 (test fixture)");
} else {
  console.log("[warn] [combat] low health");
  console.log("[warn] [combat] low health");
  console.error(`[error] ${path.join(process.cwd(), "src/intelligence.tua")}:7:1 fixture failure`);
  setTimeout(() => { process.exitCode = 1; }, 500);
}
