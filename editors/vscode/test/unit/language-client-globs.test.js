"use strict";
const assert = require("node:assert/strict");
const Module = require("node:module");
const path = require("node:path");
const test = require("node:test");

// Exercise the actual dependency consumers: packaging alone did not catch a
// minimatch major override removing the makeRe API used by the language client.
const clientRoot = path.resolve(path.dirname(require.resolve("vscode-languageclient/node")), "../..");
const load = Module._load;
let matchGlobPattern, DidCreateFilesFeature;
try {
  Module._load = function (name, ...args) {
    if (name === "vscode") {
      return { workspace: { onDidCreateFiles: () => ({ dispose() {} }) } };
    }
    return load.call(this, name, ...args);
  };
  ({ matchGlobPattern } = require(path.join(clientRoot, "lib/common/utils/globPattern.js")));
  ({ DidCreateFilesFeature } = require(path.join(clientRoot, "lib/common/fileOperations.js")));
} finally {
  Module._load = load;
}

test("language-client glob filters retain brace, wildcard and literal extglob behavior", () => {
  for (const [pattern, fsPath, expected] of [
    ["**/*.{tua,lua}", "/project/src/main.tua", true],
    ["**/*.{tua,lua}", "/project/src/helper.lua", true],
    ["**/*.{tua,lua}", "/project/src/readme.md", false],
    ["**/assets/**", "/project/assets/icons/icon.png", true],
    ["**/assets/**", "/project/src/main.tua", false],
    ["**/main?.tua", "/project/src/main1.tua", true],
    ["**/main?.tua", "/project/src/main12.tua", false],
    ["**/test+(x).tua", "/project/src/test+(x).tua", true],
    ["**/test+(x).tua", "/project/src/testxx.tua", false],
  ]) {
    assert.equal(matchGlobPattern(pattern, { fsPath }), expected, `${pattern}: ${fsPath}`);
  }
});

test("language-client file operation filters preserve case, dotfiles, schemes and Windows paths", async () => {
  const feature = new DidCreateFilesFeature({ code2ProtocolConverter: { asDidCreateFilesParams: value => value } });
  feature.register({ id: "tua-files", registerOptions: { filters: [
    { scheme: "file", pattern: { glob: "**/*.{tua,lua}", options: { ignoreCase: true } } },
  ] } });
  try {
    const files = [
      { fsPath: "/project/src/main.tua", scheme: "file" },
      { fsPath: "C:\\project\\src\\MAIN.LUA", scheme: "file" },
      { fsPath: "/project/.hidden.tua", scheme: "file" },
      { fsPath: "/project/main.tua", scheme: "untitled" },
      { fsPath: "/project/README.md", scheme: "file" },
    ];
    const result = await feature.filter({ files }, uri => uri);
    assert.deepEqual(result.files, files.slice(0, 3));
  } finally {
    feature.clear();
  }
});
