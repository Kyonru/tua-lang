#!/usr/bin/env node
"use strict";

const fs = require("fs");
const os = require("os");
const path = require("path");
const { spawnSync } = require("child_process");
const { CURSOR, tasks } = require("../benchmarks/llm-language/tasks.js");

const root = path.resolve(__dirname, "..");
const cardsDir = path.join(root, "benchmarks", "llm-language", "cards");

function parseArgs(argv) {
  const options = { provider: "reference", out: null, responses: null, emit: null, task: null };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--reference") options.provider = "reference";
    else if (arg === "--mutants") options.provider = "mutants";
    else if (arg === "--ollama") options.provider = "ollama";
    else if (arg === "--responses") options.responses = argv[++index];
    else if (arg === "--emit-requests") options.emit = argv[++index];
    else if (arg === "--out") options.out = argv[++index];
    else if (arg === "--task") options.task = argv[++index];
    else if (arg === "--tua") options.tua = argv[++index];
    else throw new Error(`unknown argument: ${arg}`);
  }
  if (options.responses) options.provider = "responses";
  return options;
}

function selectedTasks(taskId) {
  if (!taskId) return tasks;
  const selected = tasks.filter((item) => item.id === taskId);
  if (selected.length === 0) throw new Error(`unknown task: ${taskId}`);
  return selected;
}

function readCard(name) {
  return fs.readFileSync(path.join(cardsDir, `${name}.txt`), "utf8").trim();
}

function requestFor(item) {
  const [prefix, suffix] = item.source.split(CURSOR);
  const categoryCard = item.card === "core" ? "" : `\n\n${readCard(item.card)}`;
  return {
    id: item.id,
    category: item.category,
    instruction: item.instruction,
    prompt: `${readCard("core")}${categoryCard}\n\nTask: ${item.instruction}\n\n${prefix}`,
    suffix,
    responseFormat: "raw Tua insertion only",
  };
}

function writeJsonLines(destination, values) {
  const content = `${values.map((value) => JSON.stringify(value)).join("\n")}\n`;
  if (destination === "-") process.stdout.write(content);
  else {
    fs.mkdirSync(path.dirname(path.resolve(destination)), { recursive: true });
    fs.writeFileSync(destination, content);
  }
}

function readResponses(filename) {
  const entries = new Map();
  for (const [index, line] of fs.readFileSync(filename, "utf8").split(/\r?\n/).entries()) {
    if (!line.trim()) continue;
    const value = JSON.parse(line);
    if (typeof value.id !== "string" || typeof value.candidate !== "string") {
      throw new Error(`${filename}:${index + 1}: expected string id and candidate`);
    }
    if (entries.has(value.id)) throw new Error(`${filename}:${index + 1}: duplicate id ${value.id}`);
    entries.set(value.id, value);
  }
  return entries;
}

async function ollamaCandidate(item) {
  const model = process.env.TUA_OLLAMA_MODEL;
  if (!model) throw new Error("set TUA_OLLAMA_MODEL to an installed Ollama model");
  const endpoint = process.env.TUA_OLLAMA_ENDPOINT || "http://127.0.0.1:11434";
  const url = new URL("/api/generate", endpoint);
  if (!(["127.0.0.1", "localhost", "[::1]"].includes(url.hostname))) {
    throw new Error("TUA_OLLAMA_ENDPOINT must be loopback-only");
  }
  const request = requestFor(item);
  const started = performance.now();
  const response = await fetch(url, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      model,
      prompt: request.prompt,
      suffix: request.suffix,
      stream: false,
      keep_alive: "10m",
      options: { temperature: 0.1, top_p: 0.9, num_ctx: 4096, num_predict: 256 },
    }),
  });
  const body = await response.text();
  if (!response.ok) throw new Error(`Ollama returned HTTP ${response.status}: ${body}`);
  const parsed = JSON.parse(body);
  if (typeof parsed.response !== "string") throw new Error("Ollama response is missing response text");
  return { candidate: parsed.response, latencyMs: performance.now() - started, model };
}

function resolveTua(explicit) {
  const candidates = [explicit, process.env.TUA_BIN, path.join(root, "target", "debug", process.platform === "win32" ? "tua.exe" : "tua")].filter(Boolean);
  const found = candidates.find((candidate) => fs.existsSync(candidate));
  if (!found) throw new Error("Tua binary not found; run `cargo build -p tua-cli` or pass --tua");
  return path.resolve(found);
}

function structuralFailure(item, candidate) {
  if (!candidate.trim()) return "empty candidate";
  if (Buffer.byteLength(candidate) > 65536) return "candidate exceeds 65536 bytes";
  if (candidate.includes("\0")) return "candidate contains NUL";
  for (const source of item.mustMatch) {
    if (!new RegExp(source, "m").test(candidate)) return `missing required pattern: ${source}`;
  }
  for (const source of item.mustNotMatch) {
    if (new RegExp(source, "m").test(candidate)) return `contains forbidden pattern: ${source}`;
  }
  return null;
}

function evaluateCandidate(item, candidate, tua) {
  const structural = structuralFailure(item, candidate);
  if (structural) return { pass: false, stage: "structure", reason: structural };
  const parts = item.source.split(CURSOR);
  if (parts.length !== 2) return { pass: false, stage: "corpus", reason: "source must contain one cursor" };
  const temp = fs.mkdtempSync(path.join(os.tmpdir(), `tua-llm-${item.id}-`));
  try {
    const sourceDir = path.join(temp, "src");
    fs.mkdirSync(sourceDir, { recursive: true });
    fs.writeFileSync(path.join(temp, "tua.toml"), 'source = "src"\nout = "build"\nmain = "main.tua"\ntarget = "love11-luajit"\nemit_maps = false\n\n[diagnostics]\nrules = false\n');
    for (const [relative, content] of Object.entries(item.files)) {
      const destination = path.join(sourceDir, relative);
      fs.mkdirSync(path.dirname(destination), { recursive: true });
      fs.writeFileSync(destination, content);
    }
    fs.writeFileSync(path.join(sourceDir, "main.tua"), `${parts[0]}${candidate}${parts[1]}`);
    for (const command of ["check", "build"]) {
      const result = spawnSync(tua, [command], { cwd: temp, encoding: "utf8", timeout: 15000 });
      if (result.error) return { pass: false, stage: command, reason: result.error.message };
      if (result.status !== 0) {
        return { pass: false, stage: command, reason: `${result.stdout}${result.stderr}`.trim().slice(0, 2000) };
      }
    }
    return { pass: true, stage: "complete", reason: null };
  } finally {
    fs.rmSync(temp, { recursive: true, force: true });
  }
}

function summarize(results, provider) {
  const categories = {};
  for (const result of results) {
    const category = categories[result.category] || { passed: 0, total: 0 };
    category.total += 1;
    if (result.pass) category.passed += 1;
    categories[result.category] = category;
  }
  const passed = results.filter((result) => result.pass).length;
  return { version: 1, provider, passed, total: results.length, passRate: results.length ? passed / results.length : 0, categories, results };
}

async function run(options) {
  const chosen = selectedTasks(options.task);
  if (options.emit) {
    writeJsonLines(options.emit, chosen.map(requestFor));
    return { emitted: chosen.length };
  }
  const tua = resolveTua(options.tua);
  const responses = options.responses ? readResponses(options.responses) : null;
  const results = [];
  for (const item of chosen) {
    let response;
    if (options.provider === "reference") response = { candidate: item.answer, model: "reference" };
    else if (options.provider === "mutants") response = { candidate: item.mutant, model: "negative-control" };
    else if (options.provider === "ollama") response = await ollamaCandidate(item);
    else {
      response = responses.get(item.id);
      if (!response) response = { candidate: "", model: "missing-response" };
    }
    const evaluated = evaluateCandidate(item, response.candidate, tua);
    const result = options.provider === "mutants"
      ? { pass: !evaluated.pass, stage: evaluated.stage, reason: evaluated.pass ? "negative control was incorrectly accepted" : evaluated.reason }
      : evaluated;
    results.push({ id: item.id, category: item.category, ...result, latencyMs: response.latencyMs ?? null, model: response.model ?? null });
    const marker = result.pass ? "PASS" : "FAIL";
    console.error(`${marker} ${item.id}${result.pass ? "" : ` [${result.stage}] ${result.reason.split("\n")[0]}`}`);
  }
  const report = summarize(results, options.provider);
  const rendered = `${JSON.stringify(report, null, 2)}\n`;
  if (options.out) {
    fs.mkdirSync(path.dirname(path.resolve(options.out)), { recursive: true });
    fs.writeFileSync(options.out, rendered);
  } else process.stdout.write(rendered);
  if (report.passed !== report.total) process.exitCode = 1;
  return report;
}

if (require.main === module) {
  run(parseArgs(process.argv.slice(2))).catch((error) => {
    console.error(error.stack || error.message);
    process.exitCode = 1;
  });
}

module.exports = { evaluateCandidate, parseArgs, requestFor, run, structuralFailure, summarize };
