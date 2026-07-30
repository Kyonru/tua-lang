"use strict";

const assert = require("assert");

const model = process.env.TUA_OLLAMA_MODEL;
if (!model) {
  console.error("error: set TUA_OLLAMA_MODEL to an installed Ollama model");
  process.exit(1);
}

const endpoint = process.env.TUA_OLLAMA_ENDPOINT || "http://127.0.0.1:11434";
const url = new URL("/api/generate", endpoint);

fetch(url, {
  method: "POST",
  headers: { "content-type": "application/json" },
  body: JSON.stringify({
    model,
    prompt: "local speed: number = 10\nlocal x: number = 0\nx += ",
    suffix: "\nprint(x)",
    stream: false,
    keep_alive: "10m",
    options: {
      temperature: 0.1,
      top_p: 0.9,
      num_ctx: 4096,
      num_predict: 48,
      stop: [
        "<|fim_prefix|>",
        "<|fim_suffix|>",
        "<|fim_middle|>",
        "<|endoftext|>",
      ],
    },
  }),
})
  .then(async (response) => {
    const body = await response.text();
    assert(response.ok, `Ollama returned HTTP ${response.status}: ${body}`);
    const result = JSON.parse(body);
    assert.strictEqual(typeof result.response, "string");
    assert(result.response.length > 0, "Ollama returned an empty completion");
    console.log(`Ollama ${model} completion: ${JSON.stringify(result.response)}`);
  })
  .catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
