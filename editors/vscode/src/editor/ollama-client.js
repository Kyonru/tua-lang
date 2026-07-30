"use strict";

const http = require("http");
const net = require("net");

const STOP_TOKENS = [
  "<|fim_prefix|>",
  "<|fim_suffix|>",
  "<|fim_middle|>",
  "<|endoftext|>",
  "<|im_end|>",
];

function loopbackGenerateUrl(endpoint) {
  let parsed;
  try {
    parsed = new URL(endpoint);
  } catch {
    throw new Error("Invalid Ollama endpoint URL.");
  }
  const hostname = parsed.hostname.replace(/^\[|\]$/g, "").toLowerCase();
  const loopback =
    hostname === "localhost" ||
    hostname === "::1" ||
    (net.isIP(hostname) === 4 && hostname.split(".")[0] === "127") ||
    hostname === "0:0:0:0:0:0:0:1";
  if (parsed.protocol !== "http:" || !loopback || parsed.username || parsed.password) {
    throw new Error("Tua Ollama endpoints must use unauthenticated HTTP on loopback.");
  }
  return new URL("/api/generate", parsed);
}

function generate(url, payload, timeoutMs, signal) {
  return new Promise((resolve, reject) => {
    const body = JSON.stringify(payload);
    const request = http.request(
      url,
      {
        method: "POST",
        headers: {
          "content-type": "application/json",
          "content-length": Buffer.byteLength(body),
        },
        signal,
      },
      (response) => {
        let responseBody = "";
        response.setEncoding("utf8");
        response.on("data", (chunk) => {
          responseBody += chunk;
        });
        response.on("end", () => {
          if (response.statusCode < 200 || response.statusCode >= 300) {
            const error = new Error(
              `Ollama returned HTTP ${response.statusCode}: ${responseBody.slice(0, 200)}`,
            );
            error.statusCode = response.statusCode;
            reject(error);
            return;
          }
          try {
            resolve(JSON.parse(responseBody));
          } catch {
            reject(new Error("Ollama returned invalid JSON."));
          }
        });
      },
    );
    const timeout = setTimeout(() => {
      const error = new Error(`Ollama request timed out after ${timeoutMs} ms.`);
      error.code = "TUA_OLLAMA_TIMEOUT";
      request.destroy(error);
    }, timeoutMs);
    request.on("close", () => clearTimeout(timeout));
    request.on("error", reject);
    request.end(body);
  });
}

function touchCache(cache, key, value, limit = 32) {
  cache.delete(key);
  cache.set(key, value);
  while (cache.size > limit) {
    cache.delete(cache.keys().next().value);
  }
}

function isCancellationError(error) {
  const code = Number(error?.code);
  const message = String(error?.message || "");

  return (
    error?.name === "AbortError" ||
    error?.code === "ABORT_ERR" ||
    code === -32800 ||
    code === -32801 ||
    /request cancelled|document changed while request was running/i.test(message)
  );
}

function generationTokenBudget(configuredMaximum, completionScope) {
  const maximum = Math.max(1, Number(configuredMaximum) || 1);
  const scopeLimit = completionScope === "function" ? 512 : 64;
  return Math.min(maximum, scopeLimit);
}

module.exports = {
  STOP_TOKENS,
  generate,
  generationTokenBudget,
  isCancellationError,
  loopbackGenerateUrl,
  touchCache,
};
