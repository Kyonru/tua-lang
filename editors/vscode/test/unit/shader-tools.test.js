"use strict";
const assert = require("node:assert/strict");
const test = require("node:test");
const { shaderEnvironment } = require("../../src/setup/shader-tools");
test("shader tools use trusted settings without replacing inherited defaults", () => {
  const config = { inspect: () => ({ globalValue: " /usr/bin/glslang " }), get: () => "/Applications/love.app/Contents/MacOS/love" };
  const workspace = { isTrusted: false, getConfiguration: () => config };
  const base = { PATH: "/usr/bin", TUA_LOVE: "inherited" };
  assert.deepEqual(shaderEnvironment(workspace, base), base);
  workspace.isTrusted = true;
  assert.deepEqual(shaderEnvironment(workspace, base), { ...base, TUA_GLSLANG: "/usr/bin/glslang", TUA_LOVE: "/Applications/love.app/Contents/MacOS/love" });
  config.get = () => "love";
  config.inspect = () => ({ workspaceValue: "/untrusted/tool" });
  assert.deepEqual(shaderEnvironment(workspace, base), base);
});
