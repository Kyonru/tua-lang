"use strict";

const { spawn } = require("node:child_process");

function stopProcess(child) {
  if (!child.pid) return;
  if (process.platform === "win32") {
    const killer = spawn("taskkill", ["/pid", String(child.pid), "/T", "/F"], { windowsHide: true, stdio: "ignore" });
    killer.on("error", () => child.kill());
  } else {
    try { process.kill(-child.pid, "SIGTERM"); } catch { child.kill(); }
  }
}

function runProcess(command, args, { cwd, token, timeoutMs = 0 } = {}) {
  if (token?.isCancellationRequested) return Promise.resolve({ cancelled: true, code: null, stdout: "", stderr: "" });
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, { cwd, shell: false, windowsHide: true, detached: process.platform !== "win32", stdio: ["ignore", "pipe", "pipe"] });
    let stdout = "", stderr = "", bytes = 0, failure;
    let cancelled = false;
    const cancel = () => { cancelled = true; stopProcess(child); };
    const subscription = token?.onCancellationRequested(cancel);
    const timeout = timeoutMs ? setTimeout(() => { failure = new Error("Test discovery timed out"); stopProcess(child); }, timeoutMs) : undefined;
    const receive = target => chunk => {
      bytes += Buffer.byteLength(chunk);
      if (bytes > 16 * 1024 * 1024) { failure = new Error("Test output exceeded 16 MiB"); stopProcess(child); return; }
      if (target === "stdout") stdout += chunk.toString(); else stderr += chunk.toString();
    };
    child.stdout.setEncoding("utf8");
    child.stderr.setEncoding("utf8");
    child.stdout.on("data", receive("stdout"));
    child.stderr.on("data", receive("stderr"));
    const cleanup = () => { clearTimeout(timeout); subscription?.dispose(); };
    child.on("error", error => { cleanup(); reject(error); });
    child.on("close", code => { cleanup(); if (failure) reject(failure); else resolve({ code, stdout, stderr, cancelled }); });
  });
}

module.exports = { runProcess };
