"use strict";

const assert = require("assert");
const http = require("http");
const {
  STOP_TOKENS,
  generate,
  isCancellationError,
  loopbackGenerateUrl,
  touchCache,
} = require("../src/editor/ollama-client");

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});

async function main() {
  assert(isCancellationError({ name: "AbortError" }));
  assert(isCancellationError({ code: "ABORT_ERR" }));
  assert(isCancellationError({ code: -32800, message: "request cancelled" }));
  assert(isCancellationError({ code: -32801, message: "content modified" }));
  assert(isCancellationError(new Error("document changed while request was running")));
  assert(!isCancellationError(new Error("connect ECONNREFUSED 127.0.0.1:11434")));

  assert.throws(
    () => loopbackGenerateUrl("https://127.0.0.1:11434"),
    /loopback/,
  );
  assert.throws(
    () => loopbackGenerateUrl("http://example.com:11434"),
    /loopback/,
  );
  assert.throws(
    () => loopbackGenerateUrl("http://127.example.com:11434"),
    /loopback/,
  );
  assert.strictEqual(
    loopbackGenerateUrl("http://localhost:11434/base").pathname,
    "/api/generate",
  );
  const cache = new Map();
  for (let index = 0; index < 33; index += 1) {
    touchCache(cache, `key-${index}`, index);
  }
  assert.strictEqual(cache.size, 32);
  assert(!cache.has("key-0"));
  touchCache(cache, "key-1", 100);
  assert.strictEqual(Array.from(cache.keys()).at(-1), "key-1");

  const requests = [];
  const server = http.createServer((request, response) => {
    let body = "";
    request.setEncoding("utf8");
    request.on("data", (chunk) => {
      body += chunk;
    });
    request.on("end", () => {
      const payload = JSON.parse(body);
      requests.push({ path: request.url, payload });
      if (payload.model === "missing") {
        response.writeHead(404, { "content-type": "application/json" });
        response.end(JSON.stringify({ error: "model not found" }));
      } else if (payload.model === "slow") {
        setTimeout(() => {
          response.writeHead(200, { "content-type": "application/json" });
          response.end(JSON.stringify({ response: "late" }));
        }, 100);
      } else {
        response.writeHead(200, { "content-type": "application/json" });
        response.end(JSON.stringify({ response: " value" }));
      }
    });
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const endpoint = loopbackGenerateUrl(
    `http://127.0.0.1:${server.address().port}`,
  );

  try {
    const payload = {
      model: "qwen2.5-coder:1.5b-base",
      prompt: "local value = ",
      suffix: "\nprint(value)",
      stream: false,
      keep_alive: "10m",
      options: {
        temperature: 0.1,
        top_p: 0.9,
        num_ctx: 4096,
        num_predict: 48,
        stop: STOP_TOKENS,
      },
    };
    const result = await generate(endpoint, payload, 1000);
    assert.strictEqual(result.response, " value");
    assert.deepStrictEqual(requests[0], {
      path: "/api/generate",
      payload,
    });

    await generate(
      endpoint,
      { model: payload.model, prompt: "", stream: false, keep_alive: "10m" },
      1000,
    );
    assert.strictEqual(requests[1].payload.prompt, "");

    await assert.rejects(
      generate(endpoint, { model: "missing" }, 1000),
      (error) => error.statusCode === 404,
    );
    await assert.rejects(
      generate(endpoint, { model: "slow" }, 10),
      (error) => error.code === "TUA_OLLAMA_TIMEOUT",
    );

    const controller = new AbortController();
    const cancelled = generate(
      endpoint,
      { model: "slow" },
      1000,
      controller.signal,
    );
    controller.abort();
    await assert.rejects(
      cancelled,
      (error) => error.name === "AbortError" || error.code === "ABORT_ERR",
    );
  } finally {
    await new Promise((resolve) => server.close(resolve));
  }
}
