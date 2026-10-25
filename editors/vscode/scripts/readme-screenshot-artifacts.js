"use strict";
const fs = require("node:fs");
const path = require("node:path");
const crypto = require("node:crypto");
const assert = require("node:assert/strict");
const NAMES = ["workspace", "state-machine", "sprite-animation", "module-graph", "runtime-inspector"];
const hash = data => crypto.createHash("sha256").update(data).digest("hex");
function inspect(directory, readme) {
  return NAMES.map(name => {
    const file = name + ".png", bytes = fs.readFileSync(path.join(directory, file));
    assert(bytes.length > 24 && bytes.subarray(0, 8).equals(Buffer.from([137,80,78,71,13,10,26,10])), `${file}: invalid PNG`);
    assert.equal(bytes.toString("ascii", 12, 16), "IHDR", `${file}: missing PNG header`);
    assert.equal(bytes.readUInt32BE(16), 1440, `${file}: unexpected width`);
    assert.equal(bytes.readUInt32BE(20), 960, `${file}: unexpected height`);
    assert(readme.includes(`](docs/screenshots/${file})`), `${file}: missing README image link`);
    return { file, width: 1440, height: 960, bytes: bytes.length, sha256: hash(bytes) };
  });
}
function verify(directory, readme) {
  const manifest = JSON.parse(fs.readFileSync(path.join(directory, "capture.json"), "utf8"));
  assert.equal(manifest.schemaVersion, 1);
  assert.equal(manifest.completed, true, "Capture did not complete");
  assert.deepEqual(inspect(directory, readme), manifest.images, "Images differ from the completed capture; regenerate the gallery");
  return manifest;
}
module.exports = { NAMES, hash, inspect, verify };
if (require.main === module) {
  const extension = path.resolve(__dirname, "..");
  const directory = process.env.TUA_CAPTURE_OUTPUT || path.join(extension, "docs/screenshots");
  const manifest = verify(directory, fs.readFileSync(path.join(extension, "README.md"), "utf8"));
  console.log(`Verified ${manifest.images.length} README screenshots captured ${manifest.capturedAt} (extension ${manifest.extensionVersion}, ${manifest.compilerVersion}). Visual review is still required.`);
}
