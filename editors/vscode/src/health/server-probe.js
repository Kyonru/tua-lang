"use strict";

const childProcess = require("node:child_process");

// Freshly installed binaries can take longer on their first launch. Keep the
// version probe bounded without rejecting a valid server after five seconds.
function checkServerCommand(command, cwd) {
  return new Promise((resolve) => {
    let settled = false;
    let stderr = "";
    let stdout = "";
    const child = childProcess.spawn(command, ["--version"], {
      cwd,
      stdio: ["ignore", "pipe", "pipe"],
      windowsHide: true,
    });

    const timeout = setTimeout(() => {
      child.kill();
      finish({
        ok: false,
        error: new Error(`\`${command} --version\` timed out`),
      });
    }, 30000);

    child.stdout.on("data", (chunk) => {
      stdout += chunk.toString("utf8");
    });

    child.stderr.on("data", (chunk) => {
      stderr += chunk.toString("utf8");
    });

    child.once("error", (error) => {
      finish({ ok: false, error });
    });

    child.once("exit", (code) => {
      if (code === 0) {
        finish({ ok: true, version: firstLine(stdout) || firstLine(stderr) });
      } else {
        finish({
          ok: false,
          error: new Error(
            `\`${command} --version\` exited with status ${code}${stderr ? `: ${stderr.trim()}` : ""}`,
          ),
        });
      }
    });

    function finish(result) {
      if (settled) {
        return;
      }

      settled = true;
      clearTimeout(timeout);
      resolve(result);
    }
  });
}

function firstLine(text) {
  return text
    .split(/\r?\n/)
    .map((line) => line.trim())
    .find(Boolean);
}

module.exports = { checkServerCommand };
