"use strict";

const assert = require("assert/strict");
const test = require("node:test");
const {
  MACOS_EXECUTABLES,
  localVSCodeExecutable,
} = require("../vscode-executable");

test("an explicit VS Code executable overrides platform detection", () => {
  assert.equal(
    localVSCodeExecutable({
      env: { VSCODE_EXECUTABLE_PATH: "/custom/code" },
      existsSync: () => false,
      platform: "darwin",
    }),
    "/custom/code",
  );
});

test("macOS detection prefers the current Code executable", () => {
  assert.equal(
    localVSCodeExecutable({
      env: {},
      existsSync: (candidate) => candidate === MACOS_EXECUTABLES[0],
      platform: "darwin",
    }),
    "/Applications/Visual Studio Code.app/Contents/MacOS/Code",
  );
});

test("macOS detection retains the legacy Electron fallback", () => {
  assert.equal(
    localVSCodeExecutable({
      env: {},
      existsSync: (candidate) => candidate === MACOS_EXECUTABLES[1],
      platform: "darwin",
    }),
    "/Applications/Visual Studio Code.app/Contents/MacOS/Electron",
  );
});

test("unsupported platforms let @vscode/test-electron select its executable", () => {
  assert.equal(
    localVSCodeExecutable({ env: {}, existsSync: () => true, platform: "linux" }),
    undefined,
  );
});
