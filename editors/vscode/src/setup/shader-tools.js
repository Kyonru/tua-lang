"use strict";

const { execFile } = require("node:child_process");

function shaderEnvironment(workspace, base = process.env) {
  const env = { ...base };
  if (!workspace.isTrusted) return env;
  const config = workspace.getConfiguration("tua");
  const glslang = config.inspect("shaders.glslangPath")?.globalValue;
  const love = config.get("love.command");
  if (typeof glslang === "string" && glslang.trim()) env.TUA_GLSLANG = glslang.trim();
  if (typeof love === "string" && love.trim() && love.trim() !== "love") env.TUA_LOVE = love.trim();
  return env;
}

function probeShaderTools(command, env) {
  return new Promise((resolve, reject) => {
    execFile(command, ["shader-tools"], {
      env, timeout: 35000, maxBuffer: 256 * 1024, windowsHide: true,
    }, (error, stdout) => {
      if (error) return reject(error);
      try {
        const report = JSON.parse(stdout);
        if (typeof report.love?.canValidate !== "boolean" || typeof report.glslang?.canValidate !== "boolean") throw new Error("Invalid shader tool report; update the Tua server");
        resolve(report);
      } catch (error) { reject(error); }
    });
  });
}

module.exports = { shaderEnvironment, probeShaderTools };
