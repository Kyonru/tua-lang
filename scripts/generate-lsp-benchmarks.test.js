"use strict";

const assert = require("assert");
const fs = require("fs");
const os = require("os");
const path = require("path");
const test = require("node:test");

const { generateRepresentativeFixtures } = require("./generate-lsp-benchmarks");

test("representative LSP fixtures have exact deterministic sizes and hashes", () => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), "tua-lsp-benchmarks-"));
  const firstRoot = path.join(tempRoot, "first");
  const secondRoot = path.join(tempRoot, "second");

  try {
    const first = generateRepresentativeFixtures(firstRoot);
    const second = generateRepresentativeFixtures(secondRoot);
    const comparable = (manifest) =>
      manifest.fixtures.map(({ name, kind, sourceFileCount, sourceLineCount, contentHash }) => ({
        name,
        kind,
        sourceFileCount,
        sourceLineCount,
        contentHash,
      }));

    assert.deepStrictEqual(comparable(first), comparable(second));
    assert.deepStrictEqual(
      comparable(first).map(({ name, sourceFileCount, sourceLineCount }) => ({
        name,
        sourceFileCount,
        sourceLineCount,
      })),
      [
        { name: "single-1000", sourceFileCount: 1, sourceLineCount: 1000 },
        { name: "single-5000", sourceFileCount: 1, sourceLineCount: 5000 },
        { name: "project-100", sourceFileCount: 100, sourceLineCount: 1401 },
        { name: "project-1000", sourceFileCount: 1000, sourceLineCount: 14001 },
      ],
    );
    assert.match(
      fs.readFileSync(path.join(firstRoot, "single-5000", "src", "main.tua"), "utf8"),
      /profile_completion_probe = profile_primary\.label/,
    );
  } finally {
    fs.rmSync(tempRoot, { force: true, recursive: true });
  }
});

test("representative fixture selection rejects unknown names", () => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), "tua-lsp-benchmarks-select-"));

  try {
    assert.throws(
      () => generateRepresentativeFixtures(tempRoot, ["missing"]),
      /unknown representative fixture/,
    );
  } finally {
    fs.rmSync(tempRoot, { force: true, recursive: true });
  }
});
