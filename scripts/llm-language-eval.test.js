"use strict";

const assert = require("assert");
const test = require("node:test");
const { CURSOR, tasks } = require("../benchmarks/llm-language/tasks.js");
const { isLoopbackHostname, parseArgs, requestFor, structuralFailure } = require("./llm-language-eval.js");

test("LLM language corpus has forty-three unique, well-formed tasks", () => {
  assert.strictEqual(tasks.length, 43);
  assert.strictEqual(new Set(tasks.map((item) => item.id)).size, tasks.length);
  for (const item of tasks) {
    assert.match(item.id, /^[a-z0-9-]+$/);
    assert.strictEqual(item.source.split(CURSOR).length, 2, `${item.id} must have exactly one cursor`);
    assert(item.answer.trim(), `${item.id} needs a reference answer`);
    assert(item.mutant.trim(), `${item.id} needs a negative control`);
  }
});

test("exported model requests never disclose answers or controls", () => {
  for (const item of tasks) {
    const request = requestFor(item);
    assert(!Object.hasOwn(request, "answer"));
    assert(!Object.hasOwn(request, "mutant"));
    assert(!request.prompt.includes(CURSOR));
    assert.strictEqual(request.responseFormat, "raw Tua insertion only");
  }
});

test("compact core contract stays within its intended learning budget", () => {
  const core = requestFor(tasks[0]).prompt.split("\n\nTask:")[0];
  const words = core.match(/\b[\w`|?:.+-]+\b/g) || [];
  assert(words.length >= 300, `core contract is too small: ${words.length} words`);
  assert(words.length <= 500, `core contract is too large: ${words.length} words`);
});

test("model evaluation describes supported generics and exercises current game features", () => {
  const core = requestFor(tasks[0]).prompt.split("\n\nTask:")[0];
  assert.match(core, /supports generic aliases, free functions, and classes/);
  assert.doesNotMatch(core, /do not emit[^\n]*generics/i);
  for (const id of ["class-generic", "class-protocol", "love-behavior-helper", "trap-no-callsite-generics"]) {
    assert(tasks.some((item) => item.id === id), `${id} is missing from the corpus`);
  }
});

test("default negative controls reach the compiler oracle", () => {
  for (const item of tasks.filter((entry) => entry.mutant.includes("__benchmark_type_error"))) {
    assert.strictEqual(structuralFailure(item, item.mutant), null, `${item.id} control never reached compilation`);
  }
});

test("argument parsing rejects missing option values", () => {
  assert.throws(() => parseArgs(["--out"]), /expects a value/);
  assert.throws(() => parseArgs(["--task", "--reference"]), /expects a value/);
});

test("Ollama host policy accepts loopback names only", () => {
  assert(isLoopbackHostname("localhost"));
  assert(isLoopbackHostname("127.0.0.1"));
  assert(isLoopbackHostname("127.12.34.56"));
  assert(isLoopbackHostname("[::1]"));
  assert(!isLoopbackHostname("127.999.0.1"));
  assert(!isLoopbackHostname("example.com"));
  assert(!isLoopbackHostname("10.0.0.1"));
});
