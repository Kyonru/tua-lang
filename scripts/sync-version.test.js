"use strict";

const assert = require("assert");
const test = require("node:test");

const {
  semanticVersion,
  synchronizedCargoLock,
  synchronizedJson,
  updatedWorkspaceVersion,
  workspaceVersion,
} = require("./sync-version");

test("workspace version is read only from workspace package metadata", () => {
  const contents = `
[workspace]
members = []

[workspace.package]
edition = "2021"
version = "1.2.3-beta.1"

[profile.release]
strip = true
`;

  assert.strictEqual(workspaceVersion(contents), "1.2.3-beta.1");
});

test("manifest and lockfile roots receive the workspace version", () => {
  const manifest = synchronizedJson('{"name":"tua","version":"0.0.1"}\n', "0.2.0");
  const lockfile = synchronizedJson(
    '{"name":"tua","version":"0.0.1","packages":{"":{"version":"0.0.1"}}}\n',
    "0.2.0",
    true,
  );

  assert.strictEqual(JSON.parse(manifest).version, "0.2.0");
  assert.strictEqual(JSON.parse(lockfile).version, "0.2.0");
  assert.strictEqual(JSON.parse(lockfile).packages[""].version, "0.2.0");
});

test("workspace manifest and Cargo lock receive a requested version", () => {
  const manifest = `[workspace.package]\r\nedition = "2021"\r\nversion = "0.2.8"\r\n`;
  const lockfile = `version = 4

[[package]]
name = "dependency"
version = "0.2.8"

[[package]]
name = "tua-cli"
version = "0.2.8"

[[package]]
name = "tua-core"
version = "0.2.8"

[[package]]
name = "tua-lsp"
version = "0.2.8"
`;
  const updatedManifest = updatedWorkspaceVersion(manifest, "0.2.9");
  const updatedLockfile = synchronizedCargoLock(lockfile, "0.2.9");

  assert.strictEqual(workspaceVersion(updatedManifest), "0.2.9");
  assert.match(updatedManifest, /version = "0\.2\.9"\r\n$/);
  assert.match(updatedLockfile, /name = "dependency"\nversion = "0\.2\.8"/);
  assert.strictEqual((updatedLockfile.match(/version = "0\.2\.9"/g) || []).length, 3);
  assert.strictEqual(synchronizedCargoLock(updatedLockfile, "0.2.9"), updatedLockfile);
});

test("version updates reject invalid versions and incomplete Cargo locks", () => {
  assert.throws(() => semanticVersion("next"), /semantic version/);
  assert.throws(
    () => synchronizedCargoLock('[[package]]\nname = "tua-core"\nversion = "0.2.8"\n', "0.2.9"),
    /missing workspace packages: tua-cli, tua-lsp/,
  );
});

test("invalid or missing workspace versions are rejected", () => {
  assert.throws(() => workspaceVersion('[workspace.package]\nversion = "next"\n'), /semantic version/);
  assert.throws(() => workspaceVersion('[package]\nversion = "1.0.0"\n'), /workspace.package/);
});
