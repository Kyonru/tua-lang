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
        { name: "graph-chain-256", sourceFileCount: 258, sourceLineCount: 783 },
        { name: "graph-diamond-256", sourceFileCount: 34, sourceLineCount: 368 },
        { name: "graph-cycles-64", sourceFileCount: 65, sourceLineCount: 205 },
      ],
    );
    assert.match(
      fs.readFileSync(path.join(firstRoot, "single-5000", "src", "main.tua"), "utf8"),
      /profile_completion_probe = profile_primary\.label/,
    );
    assert.match(
      fs.readFileSync(
        path.join(firstRoot, "graph-chain-256", "src", "graph", "layer_064.tua"),
        "utf8",
      ),
      /require\("graph\.layer_065"\)/,
    );
    assert.match(
      fs.readFileSync(
        path.join(
          firstRoot,
          "graph-diamond-256",
          "src",
          "graph",
          "branches",
          "branch_00.tua",
        ),
        "utf8",
      ),
      /leaf_15/,
    );
    assert.match(
      fs.readFileSync(
        path.join(
          firstRoot,
          "graph-cycles-64",
          "src",
          "graph",
          "types",
          "node_31.d.tua",
        ),
        "utf8",
      ),
      /import type Next from "\.\/node_00"/,
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
