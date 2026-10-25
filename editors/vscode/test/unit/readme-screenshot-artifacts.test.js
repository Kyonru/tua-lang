"use strict";
const test = require("node:test"), assert = require("node:assert/strict");
const fs = require("node:fs"), os = require("node:os"), path = require("node:path");
const { NAMES, inspect, verify } = require("../../scripts/readme-screenshot-artifacts");
const source = path.resolve(__dirname, "../../docs/screenshots");
const readme = fs.readFileSync(path.resolve(__dirname, "../../README.md"), "utf8");
function fixture(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "tua-capture-artifacts-"));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  for (const name of NAMES) fs.copyFileSync(path.join(source, name + ".png"), path.join(root, name + ".png"));
  fs.writeFileSync(path.join(root, "capture.json"), JSON.stringify({ schemaVersion: 1, completed: true, images: inspect(root, readme) }));
  return root;
}
test("gallery validation accepts complete images and detects interrupted replacement", t => {
  const root = fixture(t);
  assert.equal(verify(root, readme).images.length, 5);
  fs.appendFileSync(path.join(root, "workspace.png"), "changed");
  assert.throws(() => verify(root, readme), /Images differ/);
});
test("gallery validation rejects missing links, missing images, and incorrect dimensions", t => {
  const root = fixture(t);
  assert.throws(() => verify(root, readme.replace("](docs/screenshots/workspace.png)", "")), /missing README image link/);
  const image = path.join(root, "workspace.png"), bytes = fs.readFileSync(image);
  bytes.writeUInt32BE(640, 16); fs.writeFileSync(image, bytes);
  assert.throws(() => inspect(root, readme), /unexpected width/);
  fs.unlinkSync(image);
  assert.throws(() => verify(root, readme), /ENOENT/);
});
