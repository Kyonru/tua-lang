"use strict";

const assert = require("assert");
const test = require("node:test");
const { CURSOR, tasks } = require("../benchmarks/llm-language/tasks.js");
const { requestFor, structuralFailure } = require("./llm-language-eval.js");

test("LLM language corpus has forty unique, well-formed tasks", () => {
  assert.strictEqual(tasks.length, 40);
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

test("default negative controls reach the compiler oracle", () => {
  for (const item of tasks.filter((entry) => entry.mutant.includes("__benchmark_type_error"))) {
    assert.strictEqual(structuralFailure(item, item.mutant), null, `${item.id} control never reached compilation`);
  }
});
