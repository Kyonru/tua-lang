"use strict";

const fs = require("fs");
const path = require("path");

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

// test-electron assumes Microsoft's CLI filename even for a VSCodium app.
function localVSCodeCliArgs(executable, options = {}) {
  const resolve = options.resolve || require("@vscode/test-electron").resolveCliArgsFromVSCodeExecutablePath;
  const existsSync = options.existsSync || fs.existsSync;
  const args = resolve(executable);
  if (!existsSync(args[0]) && path.basename(args[0]) === "code") {
    const codium = path.join(path.dirname(args[0]), "codium");
    if (existsSync(codium)) return [codium, ...args.slice(1)];
  }
  return args;
}

module.exports = {
  localVSCodeCliArgs,
  MACOS_EXECUTABLES,
  localVSCodeExecutable,
};
