#!/usr/bin/env node
// Fetches the pinned third-party LOVE projects used as the external Lua test
// set for inline-completion evaluation (see benchmarks/inline-eval).
//
// Each project is checked out at its pinned commit into
// target/inline-eval/external/<name>. The script verifies the commit ID, the
// license file, and a SHA-256 digest of the .lua files the evaluator reads,
// then writes target/inline-eval/external/lock.json. A digest left empty in
// external.toml is printed so it can be recorded. Network access is required;
// ordinary CI never runs this.
//
//   node scripts/fetch-inline-eval-projects.js [--print-digests]

"use strict";

const { execFileSync } = require("node:child_process");
const crypto = require("node:crypto");
const fs = require("node:fs");
const path = require("node:path");

const root = path.resolve(__dirname, "..");
const manifestPath = path.join(root, "benchmarks/inline-eval/external.toml");
const destination = path.join(root, "target/inline-eval/external");

function parseManifest(text) {
  const projects = [];
  let current = null;
  for (const raw of text.split("\n")) {
    const line = raw.trim();
    if (line === "" || line.startsWith("#")) continue;
    if (line === "[[project]]") {
      current = {};
      projects.push(current);
      continue;
    }
    const match = /^([A-Za-z0-9_]+)\s*=\s*"([^"]*)"$/.exec(line);
    if (!match || !current) throw new Error(`unsupported line in external.toml: ${line}`);
    current[match[1]] = match[2];
  }
  for (const project of projects) {
    for (const key of ["name", "url", "commit", "license", "license_file"]) {
      if (!project[key]) throw new Error(`project ${project.name ?? "?"} is missing ${key}`);
    }
    if (!/^[0-9a-f]{40}$/.test(project.commit)) {
      throw new Error(`${project.name}: commit must be a full 40-character SHA`);
    }
    if (!/^[a-z0-9-]+$/.test(project.name)) {
      throw new Error(`${project.name}: names must be lowercase letters, digits, and hyphens`);
    }
  }
  return projects;
}

function git(cwd, ...args) {
  return execFileSync("git", args, { cwd, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] }).trim();
}

function checkout(project) {
  const directory = path.join(destination, project.name);
  if (!directory.startsWith(destination + path.sep)) throw new Error(`unsafe path for ${project.name}`);

  if (fs.existsSync(path.join(directory, ".git"))) {
    try {
      if (git(directory, "rev-parse", "HEAD") === project.commit) return directory;
    } catch {
      // Fall through to a fresh checkout.
    }
  }

  fs.rmSync(directory, { recursive: true, force: true });
  fs.mkdirSync(directory, { recursive: true });
  git(directory, "init", "--quiet");
  git(directory, "fetch", "--quiet", "--depth", "1", project.url, project.commit);
  git(directory, "checkout", "--quiet", "--detach", "FETCH_HEAD");
  return directory;
}

function luaFiles(directory) {
  const files = [];
  const walk = (current) => {
    for (const entry of fs.readdirSync(current, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
      if (entry.name.startsWith(".")) continue;
      const full = path.join(current, entry.name);
      if (entry.isDirectory()) walk(full);
      else if (entry.isFile() && entry.name.endsWith(".lua")) files.push(full);
    }
  };
  walk(directory);
  return files;
}

// Digest of every evaluated file's relative path and content, so the test set
// is verified byte for byte without depending on GitHub archive formats.
function contentDigest(directory) {
  const hash = crypto.createHash("sha256");
  const files = luaFiles(directory);
  for (const file of files) {
    const relative = path.relative(directory, file).split(path.sep).join("/");
    const content = crypto.createHash("sha256").update(fs.readFileSync(file)).digest("hex");
    hash.update(`${relative}\0${content}\n`);
  }
  return { digest: hash.digest("hex"), files: files.length };
}

function main() {
  const printDigests = process.argv.includes("--print-digests");
  const projects = parseManifest(fs.readFileSync(manifestPath, "utf8"));
  fs.mkdirSync(destination, { recursive: true });

  const lock = [];
  let failed = false;
  for (const project of projects) {
    const directory = checkout(project);
    const head = git(directory, "rev-parse", "HEAD");
    if (head !== project.commit) {
      console.error(`${project.name}: checked out ${head}, expected ${project.commit}`);
      failed = true;
      continue;
    }
    if (!fs.existsSync(path.join(directory, project.license_file))) {
      console.error(`${project.name}: license file ${project.license_file} is missing`);
      failed = true;
      continue;
    }
    const { digest, files } = contentDigest(directory);
    if (project.content_sha256 && project.content_sha256 !== digest) {
      console.error(`${project.name}: content digest ${digest} does not match the pinned ${project.content_sha256}`);
      failed = true;
      continue;
    }
    if (!project.content_sha256 || printDigests) {
      console.log(`${project.name}: content_sha256 = "${digest}" (${files} .lua files)`);
    }
    lock.push({ ...project, content_sha256: digest, lua_files: files });
  }

  fs.writeFileSync(path.join(destination, "lock.json"), `${JSON.stringify(lock, null, 2)}\n`);
  if (failed) process.exit(1);
  console.log(`fetched ${lock.length} project(s) into ${path.relative(root, destination)}`);
}

main();
