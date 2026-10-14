"use strict";

const fs = require("fs");

const MACOS_EXECUTABLES = [
  "/Applications/Visual Studio Code.app/Contents/MacOS/Code",
  "/Applications/Visual Studio Code.app/Contents/MacOS/Electron",
];

function localVSCodeExecutable(options = {}) {
  const env = options.env || process.env;
  const platform = options.platform || process.platform;
  const existsSync = options.existsSync || fs.existsSync;

  if (env.VSCODE_EXECUTABLE_PATH) {
    return env.VSCODE_EXECUTABLE_PATH;
  }

  if (platform === "darwin") {
    return MACOS_EXECUTABLES.find((candidate) => existsSync(candidate));
  }

  return undefined;
}

module.exports = {
  MACOS_EXECUTABLES,
  localVSCodeExecutable,
};
