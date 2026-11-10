"use strict";

const fs = require("node:fs");
const path = require("node:path");

const root = path.resolve(__dirname, "..");
const featureRoot = path.join(root, "examples", "features");
const navigation = fs.readFileSync(path.join(root, "zensical.toml"), "utf8");
const section = navigation.match(/\{ "Features" = \[([\s\S]*?)\n  \] \},/);
if (!section) throw new Error("Could not find the Features documentation navigation");

const pages = [...section[1].matchAll(/\{ "[^"]+" = "([^"]+\.md)" \}/g)].map((match) => match[1]);
const coverage = JSON.parse(fs.readFileSync(path.join(featureRoot, "coverage.json"), "utf8"));
const missing = pages.filter((page) => !Object.hasOwn(coverage, page));
const stale = Object.keys(coverage).filter((page) => !pages.includes(page));
if (missing.length || stale.length) {
  throw new Error(`Feature example coverage differs from docs navigation: missing [${missing.join(", ")}], stale [${stale.join(", ")}]`);
}

const entry = fs.readFileSync(path.join(featureRoot, "src", "main.tua"), "utf8");
let count = 0;
const used = new Set();
for (const [page, files] of Object.entries(coverage)) {
  if (!Array.isArray(files) || files.length === 0) throw new Error(`${page} needs at least one example`);
  for (const relative of files) {
    if (!/^src\/[a-z_]+\.tua$/.test(relative)) throw new Error(`${page} has an invalid feature example path: ${relative}`);
    if (used.has(relative)) throw new Error(`${relative} is assigned to more than one feature guide`);
    used.add(relative);
    const source = fs.readFileSync(path.join(featureRoot, relative), "utf8");
    if (!source.startsWith("-- Feature:")) throw new Error(`${relative} needs a feature description on its first line`);
    const moduleName = path.basename(relative, ".tua");
    if (!entry.includes(`require("${moduleName}")`)) throw new Error(`${relative} is not included in src/main.tua`);
    count += 1;
  }
}

console.log(`validated ${pages.length} feature guides against ${count} feature-tour source files`);
