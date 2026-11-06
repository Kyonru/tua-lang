"use strict";

const { spawn } = require("node:child_process");

function stopProcess(child, force = false) {
  if (!child.pid) return;
  if (process.platform === "win32") {
    const killer = spawn("taskkill", ["/pid", String(child.pid), "/T", "/F"], { windowsHide: true, stdio: "ignore" });
    killer.on("error", () => child.kill());
  } else {
    try { process.kill(-child.pid, force ? "SIGKILL" : "SIGTERM"); } catch { child.kill(force ? "SIGKILL" : "SIGTERM"); }
  }
}

function runProcess(command, args, { cwd, token, timeoutMs = 0, truncateOutput = false } = {}) {
  if (token?.isCancellationRequested) return Promise.resolve({ cancelled: true, code: null, stdout: "", stderr: "" });
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, { cwd, shell: false, windowsHide: true, detached: process.platform !== "win32", stdio: ["ignore", "pipe", "pipe"] });
    let stdout = "", stderr = "", bytes = 0, failure, truncated = false;
    let cancelled = false, killTimer;
    const stop = () => {
      stopProcess(child);
      // Test code may install a SIGTERM handler. Cancellation and output/time
      // limits must still release the serialized Test Explorer queue.
      if (!killTimer) killTimer = setTimeout(() => stopProcess(child, true), 1000);
    };
    const cancel = () => { cancelled = true; stop(); };
    const subscription = token?.onCancellationRequested(cancel);
    const timeout = timeoutMs ? setTimeout(() => { failure = new Error("Test discovery timed out"); stop(); }, timeoutMs) : undefined;
    const receive = target => chunk => {
      bytes += Buffer.byteLength(chunk);
      if (bytes > 16 * 1024 * 1024) {
        if (truncateOutput) truncated = true;
        else { failure = new Error("Test output exceeded 16 MiB"); stop(); }
        return;
      }
      if (target === "stdout") stdout += chunk.toString(); else stderr += chunk.toString();
    };
    child.stdout.setEncoding("utf8");
    child.stderr.setEncoding("utf8");
    child.stdout.on("data", receive("stdout"));
    child.stderr.on("data", receive("stderr"));
    const cleanup = () => { clearTimeout(timeout); clearTimeout(killTimer); subscription?.dispose(); };
    child.on("error", error => { cleanup(); reject(error); });
    child.on("close", code => {
      cleanup();
      if (truncated) stdout += "\n[Test output truncated at 16 MiB; see structured results for test status.]\n";
      if (failure) reject(failure); else resolve({ code, stdout, stderr, cancelled });
    });
  });
}

module.exports = { runProcess };
